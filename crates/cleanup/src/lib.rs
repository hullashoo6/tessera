//! Review-first cleanup.
//!
//! Discover → Select → Review (`build_plan`) → Confirm → Move to Trash
//! (`execute`) → Verify (`Report`). There is no permanent deletion. Every
//! item is re-validated immediately before it is moved; anything that cannot
//! be verified is refused rather than guessed.

pub mod protect;
mod trash;

pub use protect::Protection;
pub use trash::move_to_trash;

use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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

#[derive(Debug, Clone)]
pub struct Outcome {
    pub id: NodeId,
    pub path: PathBuf,
    pub logical: u64,
    pub allocated: u64,
    /// `Err` holds the reason the item was left in place.
    pub result: Result<(), String>,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
    pub moved: usize,
    pub failed: usize,
    pub moved_logical: u64,
    pub moved_allocated: u64,
}

/// Moves every planned item with `mover`, re-validating each one first.
/// Items that fail are reported and never retried another way.
pub fn execute(
    plan: &Plan,
    protection: &Protection,
    mover: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Report {
    let mut report = Report::default();
    for c in &plan.items {
        let mut result = revalidate(c, &plan.root, protection).and_then(|()| {
            mover(&c.path).map_err(|e| format!("It could not be moved to the Trash: {e}"))
        });
        if result.is_ok() && std::fs::symlink_metadata(&c.path).is_ok() {
            result = Err("It is still in place after the move.".into());
        }
        match &result {
            Ok(()) => {
                report.moved += 1;
                report.moved_logical += c.logical;
                report.moved_allocated += c.allocated;
            }
            Err(_) => report.failed += 1,
        }
        report.outcomes.push(Outcome {
            id: c.id,
            path: c.path.clone(),
            logical: c.logical,
            allocated: c.allocated,
            result,
        });
    }
    report
}
