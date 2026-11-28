//! Review-first cleanup.
//!
//! Discover → Select → Review (`build_plan`) → Confirm → Remove (`run`) →
//! Verify (`Report`). Items go to the Recycle Bin / Trash, or, only when the
//! person explicitly chooses it, are deleted permanently. Every item is
//! re-validated immediately before it is removed; anything that cannot be
//! verified is refused rather than guessed. Failures carry a plain cause
//! (`Failure`), including which app holds a file open where Windows can tell.

mod failure;
pub mod inuse;
mod permanent;
pub mod protect;
mod trash;

pub use permanent::delete_permanently;

pub use failure::{FailKind, Failure, classify};
pub use protect::Protection;
pub mod cloud;
pub mod history;
pub use trash::{
    delete_many_permanently, fits_in_trash, move_many_to_trash, move_to_trash, put_back,
    recycle_limit,
};

use std::collections::{HashMap, HashSet};
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use tessera_scan::{NodeId, NodeKind, ROOT, Tree, flags};

/// What an item looked like when the user reviewed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub is_dir: bool,
    pub len: u64,
    pub modified: Option<SystemTime>,
}

impl Stamp {
    fn of(md: &Metadata) -> Stamp {
        Stamp {
            is_dir: md.is_dir(),
            len: if md.is_dir() { 0 } else { md.len() },
            modified: md.modified().ok(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: NodeId,
    pub path: PathBuf,
    pub kind: NodeKind,
    /// Bytes counted for this item in the scan (what removing it may free).
    pub logical: u64,
    pub allocated: u64,
    /// Files and folders inside it (0 for a file): how much work removing it
    /// is, so a large folder gets an operation of its own.
    pub entries: u64,
    pub stamp: Stamp,
    /// Shown to the user during review; never blocks.
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Rejection {
    pub id: NodeId,
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub root: PathBuf,
    pub items: Vec<Candidate>,
    pub rejected: Vec<Rejection>,
    /// Selected items already covered by a selected folder.
    pub merged: usize,
    pub total_logical: u64,
    pub total_allocated: u64,
}

/// Builds the review list for the selected node ids.
pub fn build_plan(tree: &Tree, selected: &[NodeId], protection: &Protection) -> Plan {
    let mut ids: Vec<NodeId> = selected
        .iter()
        .copied()
        .filter(|&i| (i as usize) < tree.len())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut plan = Plan {
        root: tree.root_path().to_path_buf(),
        ..Plan::default()
    };
    let mut accepted: Vec<Candidate> = Vec::new();

    // Judge every selected item on its own first, so a refused folder can
    // never swallow acceptable items inside it.
    for id in ids {
        let n = tree.node(id);
        let path = tree.path_of(id);
        let reject = |reason: &str, plan: &mut Plan| {
            plan.rejected.push(Rejection {
                id,
                path: path.clone(),
                reason: reason.into(),
            })
        };
        if id == ROOT {
            reject(
                "The scanned location itself is not removed. Choose items inside it.",
                &mut plan,
            );
            continue;
        }
        if matches!(n.kind, NodeKind::Link | NodeKind::Other) {
            reject(
                "Links and special files are not removed by Tessera.",
                &mut plan,
            );
            continue;
        }
        if n.has(flags::PENDING) {
            reject("This folder has not been scanned yet.", &mut plan);
            continue;
        }
        if let Err(reason) = protection.check(&path) {
            reject(&reason, &mut plan);
            continue;
        }
        let md = match std::fs::symlink_metadata(&path) {
            Ok(md) => md,
            Err(_) => {
                reject(
                    "This item no longer exists at its scanned location.",
                    &mut plan,
                );
                continue;
            }
        };
        if md.file_type().is_symlink() {
            reject(
                "This item has been replaced by a link since the scan.",
                &mut plan,
            );
            continue;
        }
        let warning = if n.has(flags::HARD_LINKED) {
            Some("This file has other names (hard links). Removing this one frees no space while another remains.".to_string())
        } else if n.has(flags::SUBTREE_INCOMPLETE) || n.has(flags::UNREADABLE) {
            Some(
                "Part of this folder could not be read, so it may contain more than shown."
                    .to_string(),
            )
        } else if n.has(flags::CLOUD_ONLY) || n.has(flags::SUBTREE_CLOUD) {
            Some("Contains online-only files. Removing them may also remove them from your cloud storage.".to_string())
        } else {
            None
        };
        let (logical, allocated) = n.counted();
        accepted.push(Candidate {
            id,
            path,
            kind: n.kind,
            logical,
            allocated,
            entries: u64::from(n.files) + u64::from(n.dirs),
            stamp: Stamp::of(&md),
            warning,
        });
    }

    // Then drop accepted items already covered by an accepted folder.
    let accepted_ids: std::collections::HashSet<NodeId> = accepted.iter().map(|c| c.id).collect();
    for c in accepted {
        if tree.ancestors(c.id).any(|a| accepted_ids.contains(&a)) {
            plan.merged += 1;
            continue;
        }
        plan.total_logical += c.logical;
        plan.total_allocated += c.allocated;
        plan.items.push(c);
    }
    plan
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('\\', "/");
        let s = s
            .strip_prefix("//?/")
            .unwrap_or(&s)
            .trim_end_matches('/')
            .to_string();
        if cfg!(any(windows, target_os = "macos")) {
            s.to_lowercase()
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

/// Checks, immediately before moving, that `c` is still exactly what was
/// reviewed and that its path does not pass through a link.
pub fn revalidate(c: &Candidate, root: &Path, protection: &Protection) -> Result<(), String> {
    protection.check(&c.path)?;
    if !c.path.starts_with(root) {
        return Err("This item is outside the scanned location.".into());
    }
    let md = std::fs::symlink_metadata(&c.path).map_err(|_| "It no longer exists.".to_string())?;
    if md.file_type().is_symlink() {
        return Err("It has been replaced by a link.".into());
    }
    let now = Stamp::of(&md);
    if now.is_dir != c.stamp.is_dir {
        return Err("It has been replaced by a different kind of item.".into());
    }
    if !now.is_dir && now != c.stamp {
        return Err("It has changed since you reviewed it.".into());
    }
    // No folder between the scan root and the item may have become a link.
    let mut p = c.path.parent();
    while let Some(dir) = p {
        if !dir.starts_with(root) {
            break;
        }
        let md = std::fs::symlink_metadata(dir)
            .map_err(|_| "A folder on its path no longer exists.".to_string())?;
        if md.file_type().is_symlink() || !md.is_dir() {
            return Err("A folder on its path has been replaced by a link.".into());
        }
        if dir == root {
            break;
        }
        p = dir.parent();
    }
    // The resolved location must be the same place. Links *above* the scan
    // root (e.g. macOS /var -> /private/var, Windows 8.3 short names) are
    // part of the location the user chose, so compare below the root only.
    let unverifiable = || "Its location could not be verified.".to_string();
    let real = std::fs::canonicalize(&c.path).map_err(|_| unverifiable())?;
    let real_root = std::fs::canonicalize(root).map_err(|_| unverifiable())?;
    let rel = c.path.strip_prefix(root).map_err(|_| unverifiable())?;
    if !same_path(&real, &real_root.join(rel)) {
        return Err("Its path now leads somewhere else.".into());
    }
    Ok(())
}

/// How the chosen items are removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Method {
    /// To the Recycle Bin / Trash, from where they can be put back.
    #[default]
    Trash,
    /// Deleted permanently. Only when the person chose it explicitly.
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Now in the Recycle Bin / Trash (where, when the system says).
    Moved(Option<PathBuf>),
    /// Deleted permanently.
    Deleted,
    /// It was no longer there: nothing left to do.
    Gone,
    /// Left in place, and why.
    Failed(Failure),
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub id: NodeId,
    pub path: PathBuf,
    pub logical: u64,
    pub allocated: u64,
    pub status: Status,
}

impl Outcome {
    /// Moved, deleted or already gone: no longer needs to be in the list.
    pub fn done(&self) -> bool {
        !matches!(self.status, Status::Failed(_))
    }

    /// Where it sits in the Recycle Bin / Trash, if it was moved there.
    pub fn recycled(&self) -> Option<&PathBuf> {
        match &self.status {
            Status::Moved(p) => p.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
    pub moved: usize,
    pub deleted: usize,
    pub gone: usize,
    pub failed: usize,
    /// Bytes counted in the scan for the items moved or deleted.
    pub removed_logical: u64,
    pub removed_allocated: u64,
    /// The person pressed Stop; `not_started` items were left untouched.
    pub stopped: bool,
    pub not_started: usize,
}

/// Where a cleanup run has got to.
#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    /// Bytes on disk of the items removed so far.
    pub bytes: u64,
    /// Name of the item being worked on now (the first of its operation),
    /// so a long removal visibly is not stuck. `None` once finished.
    pub current: Option<String>,
}

pub struct RunOptions<'a> {
    /// Remove only these planned items (the rest of the review is kept).
    pub only: Option<&'a HashSet<NodeId>>,
    pub method: Method,
    /// Items per operation (Windows moves many items far faster together).
    pub batch: usize,
    /// Files and folders inside the items of one operation: a large folder
    /// gets an operation of its own, so progress and Stop never wait for a
    /// whole group of large folders (28 Nov 2026: 17 folders deleted as one
    /// operation showed "0 of 17" for minutes).
    pub entries_per_batch: u64,
    /// Checked between operations (and, when deleting permanently, between
    /// the entries of a folder): stop early, leaving the rest untouched.
    pub stop: Option<&'a AtomicBool>,
}

impl Default for RunOptions<'_> {
    fn default() -> Self {
        RunOptions {
            only: None,
            method: Method::Trash,
            batch: 25,
            entries_per_batch: 2_000,
            stop: None,
        }
    }
}

/// Removes items, one result per path in order: `Ok(where it landed)`.
pub type Remover<'a> = dyn FnMut(&[&Path]) -> Vec<io::Result<Option<PathBuf>>> + 'a;

/// The system remover for `method`.
pub fn system_remover(method: Method) -> impl FnMut(&[&Path]) -> Vec<io::Result<Option<PathBuf>>> {
    system_remover_with_stop(method, None)
}

/// The system remover for `method`; permanent deletion of a folder checks
/// `stop` between its entries.
pub fn system_remover_with_stop(
    method: Method,
    stop: Option<&AtomicBool>,
) -> impl FnMut(&[&Path]) -> Vec<io::Result<Option<PathBuf>>> + '_ {
    move |paths: &[&Path]| match method {
        Method::Trash => move_many_to_trash(paths),
        Method::Permanent => paths
            .iter()
            .map(|p| permanent::delete_permanently(p, stop).map(|()| None))
            .collect(),
    }
}

/// Splits the chosen items into operations: at most `batch` items, and a
/// new operation once the current one holds `entries` files and folders.
fn batches<'c>(items: &[&'c Candidate], batch: usize, entries: u64) -> Vec<Vec<&'c Candidate>> {
    let mut out: Vec<Vec<&Candidate>> = Vec::new();
    let mut cur: Vec<&Candidate> = Vec::new();
    let mut weight = 0u64;
    for &c in items {
        let w = 1 + c.entries;
        if !cur.is_empty() && (cur.len() >= batch.max(1) || weight + w > entries.max(1)) {
            out.push(std::mem::take(&mut cur));
            weight = 0;
        }
        cur.push(c);
        weight += w;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Removes the planned items (or `opts.only` of them) in batches,
/// re-validating each just before its batch. Failures are explained, never
/// retried another way; items already gone count as done.
pub fn run(
    plan: &Plan,
    protection: &Protection,
    opts: &RunOptions,
    progress: &mut dyn FnMut(&Progress),
    remover: &mut Remover,
) -> Report {
    let chosen: Vec<&Candidate> = plan
        .items
        .iter()
        .filter(|c| opts.only.is_none_or(|o| o.contains(&c.id)))
        .collect();
    let mut report = Report::default();
    let mut p = Progress {
        total: chosen.len(),
        ..Progress::default()
    };
    let stopped = || opts.stop.is_some_and(|s| s.load(Ordering::Relaxed));
    let mut started = 0usize;
    for chunk in batches(&chosen, opts.batch, opts.entries_per_batch) {
        p.done = report.outcomes.len();
        p.bytes = report.removed_allocated;
        p.current = chunk
            .first()
            .and_then(|c| c.path.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        progress(&p);
        if stopped() {
            report.stopped = true;
            report.not_started = chosen.len() - started;
            break;
        }
        started += chunk.len();
        let mut ready: Vec<&Candidate> = Vec::new();
        for &c in &chunk {
            if std::fs::symlink_metadata(&c.path).is_err() {
                record(&mut report, c, Status::Gone);
            } else if let Err(reason) = revalidate(c, &plan.root, protection) {
                let kind = if protection.check(&c.path).is_err() {
                    FailKind::Protected
                } else {
                    FailKind::Changed
                };
                record(&mut report, c, Status::Failed(Failure::new(kind, reason)));
            } else if opts.method == Method::Trash && !trash::fits_in_trash(&c.path, c.logical) {
                // Windows would delete it permanently, without asking, rather
                // than keep it in the Recycle Bin: it is never handed over.
                record(
                    &mut report,
                    c,
                    Status::Failed(classify(&trash::too_big(), &c.path)),
                );
            } else {
                ready.push(c);
            }
        }
        let paths: Vec<&Path> = ready.iter().map(|c| c.path.as_path()).collect();
        let results = if paths.is_empty() {
            Vec::new()
        } else {
            remover(&paths)
        };
        for (c, r) in ready
            .iter()
            .zip(results.into_iter().chain(std::iter::repeat_with(|| {
                Err(io::Error::other("no result was reported"))
            })))
        {
            let status = match r {
                Ok(_) if std::fs::symlink_metadata(&c.path).is_ok() => Status::Failed(
                    Failure::new(FailKind::Other, "It is still in place after the move."),
                ),
                Ok(landed) => match opts.method {
                    // On Windows the Recycle Bin always reports (or Tessera
                    // finds) where a moved item is. Gone but not there means
                    // Windows deleted it: say so, and never offer Undo.
                    Method::Trash if landed.is_none() && cfg!(windows) => Status::Deleted,
                    Method::Trash => Status::Moved(landed),
                    Method::Permanent => Status::Deleted,
                },
                Err(e) if e.kind() == io::ErrorKind::NotFound => Status::Gone,
                Err(e) => Status::Failed(classify(&e, &c.path)),
            };
            record(&mut report, c, status);
        }
    }
    // Stop pressed during the last operation still counts as stopped (a
    // folder being deleted permanently ends part way).
    report.stopped |= stopped();
    p.done = report.outcomes.len();
    p.bytes = report.removed_allocated;
    p.current = None;
    progress(&p);
    report
}

fn record(report: &mut Report, c: &Candidate, status: Status) {
    match &status {
        Status::Moved(_) => report.moved += 1,
        Status::Deleted => report.deleted += 1,
        Status::Gone => report.gone += 1,
        Status::Failed(_) => report.failed += 1,
    }
    if matches!(status, Status::Moved(_) | Status::Deleted) {
        report.removed_logical += c.logical;
        report.removed_allocated += c.allocated;
    }
    report.outcomes.push(Outcome {
        id: c.id,
        path: c.path.clone(),
        logical: c.logical,
        allocated: c.allocated,
        status,
    });
}

/// Moves every planned item with a one-item `mover`, one at a time, to the
/// Recycle Bin / Trash. (Kept for callers and tests that move single items.)
pub fn execute(
    plan: &Plan,
    protection: &Protection,
    mover: &mut dyn FnMut(&Path) -> io::Result<Option<PathBuf>>,
) -> Report {
    let opts = RunOptions {
        batch: 1,
        ..RunOptions::default()
    };
    run(
        plan,
        protection,
        &opts,
        &mut |_| {},
        &mut |paths: &[&Path]| paths.iter().map(|p| mover(p)).collect(),
    )
}

/// What `remove_except_in_use` did with a folder that could not be removed
/// whole because files in it are open.
#[derive(Debug, Clone, Default)]
pub struct PartialReport {
    /// Items inside the folder that were removed (with where each landed).
    pub removed: Vec<(PathBuf, Option<PathBuf>)>,
    pub failed: Vec<(PathBuf, Failure)>,
    /// Files left because they are in use.
    pub in_use: Vec<PathBuf>,
    /// False when the folder was too big to check for open files in time,
    /// in which case nothing was removed.
    pub checked: bool,
}

/// Removes everything inside `dir` except the files in use and the folders
/// that lead to them. For caches and similar folders that an open app keeps
/// a few files of. `dir` must pass the protection rules, as must everything
/// removed.
pub fn remove_except_in_use(
    dir: &Path,
    protection: &Protection,
    remover: &mut Remover,
) -> PartialReport {
    let mut out = PartialReport::default();
    if protection.check(dir).is_err() || !dir.is_dir() {
        return out;
    }
    let locked = inuse::locked_files_in(dir, 10_000, 500_000, Duration::from_secs(5));
    if !locked.complete {
        return out;
    }
    out.checked = true;
    // Folders that contain an open file, up to `dir` itself.
    let mut keep: HashSet<PathBuf> = HashSet::new();
    for f in &locked.files {
        let mut p = f.parent();
        while let Some(d) = p {
            if !keep.insert(d.to_path_buf()) || d == dir {
                break;
            }
            p = d.parent();
        }
    }
    let open: HashSet<&PathBuf> = locked.files.iter().collect();
    let mut movable: Vec<PathBuf> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if open.contains(&path) {
                out.in_use.push(path);
            } else if ft.is_dir() && !ft.is_symlink() && keep.contains(&path) {
                stack.push(path);
            } else if protection.check(&path).is_ok() {
                movable.push(path);
            }
        }
    }
    let mut landed: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    for chunk in movable.chunks(50) {
        let refs: Vec<&Path> = chunk.iter().map(|p| p.as_path()).collect();
        for (p, r) in chunk.iter().zip(remover(&refs)) {
            match r {
                Ok(l) if std::fs::symlink_metadata(p).is_err() => {
                    landed.insert(p.clone(), l);
                }
                Ok(_) => out.failed.push((
                    p.clone(),
                    Failure::new(FailKind::Other, "It is still in place after the move."),
                )),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => out.failed.push((p.clone(), classify(&e, p))),
            }
        }
    }
    out.removed = landed.into_iter().collect();
    out.removed.sort();
    out
}
