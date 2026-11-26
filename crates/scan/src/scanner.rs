//! Parallel, cancellable, read-only directory scanner.
//!
//! Design:
//! * A fixed pool of worker threads (bounded, default = available cores,
//!   capped at 8) takes directory jobs from a queue, lists each directory
//!   with `std::fs::read_dir`, and sends the whole listing back.
//! * One aggregator (the calling thread) owns all tree mutation. It appends
//!   each listing's entries contiguously, queues subdirectories, and adds
//!   the listing's bytes to every ancestor, so totals are correct at every
//!   moment and the tree can be displayed while the scan runs.
//! * Links, junctions and mount points are never followed, and other
//!   filesystems are not entered, so traversal cannot loop. On Unix,
//!   directories are additionally de-duplicated by (device, inode).
//! * Errors never abort the scan: they are recorded per node and summarised.

use crate::model::{NO_NODE, Node, NodeId, NodeKind, ROOT, Tree, flags};
use crate::platform::{self, EntryInfo, VolumeContext};
use crossbeam_channel as chan;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// How hard links are accounted for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardLinkMode {
    /// macOS/Unix: every file is checked (identity is free with `lstat`).
    /// Windows: files of at least `ScanOptions::hard_link_min_size` bytes are
    /// checked; reading identity there requires opening the file, so small
    /// files are skipped to keep scans fast.
    Auto,
    /// Check every non-empty file on every platform. Accurate, but on
    /// Windows considerably slower.
    Thorough,
    /// Never de-duplicate: every path counts its full size.
    Off,
}

/// Which files were checked for hard links in a finished scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardLinkCoverage {
    NotChecked,
    /// Only files at least this many bytes long were checked.
    FilesAtLeast(u64),
    AllFiles,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    /// Worker threads; 0 chooses automatically.
    pub threads: usize,
    /// Descend into directories on other filesystems (Unix mount points).
    pub cross_filesystems: bool,
    pub hard_links: HardLinkMode,
    /// Windows `Auto` mode threshold (see `HardLinkMode::Auto`).
    pub hard_link_min_size: u64,
    /// How often the progress callback runs.
    pub progress_interval: Duration,
    /// Cap on individually recorded issues (the total is always counted).
    pub max_recorded_issues: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            threads: 0,
            cross_filesystems: false,
            hard_links: HardLinkMode::Auto,
            hard_link_min_size: 64 * 1024,
            progress_interval: Duration::from_millis(100),
            max_recorded_issues: 50_000,
        }
    }
}

/// Cooperative cancellation flag shared with a running scan.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub files: u64,
    pub dirs: u64,
    pub links: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub issues: u64,
    /// Directories discovered but not yet read.
    pub dirs_pending: u64,
    /// Entries read by workers (may run ahead of the tree).
    pub entries_listed: u64,
    pub elapsed: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    PermissionDenied,
    NotFound,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueStage {
    /// The directory could not be opened.
    OpenDirectory,
    /// Listing stopped part-way through.
    ReadDirectory,
    /// One entry's metadata could not be read.
    ReadMetadata,
}

/// A problem encountered at a node. Stores ids, not paths, so summaries can
/// be kept or logged without exposing file names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanIssue {
    pub node: NodeId,
    pub kind: IssueKind,
    pub stage: IssueStage,
    pub os_code: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct ScanSummary {
    pub progress: Progress,
    pub cancelled: bool,
    pub elapsed: Duration,
    pub time_to_first_result: Option<Duration>,
    pub issues: Vec<ScanIssue>,
    pub issues_not_recorded: u64,
    /// Which files were checked for (and de-duplicated by) hard links.
    pub hard_links: HardLinkCoverage,
    pub threads: usize,
    /// Allocation unit used for estimates, if any.
    pub cluster_size: Option<u64>,
    /// A worker thread failed unexpectedly; results are incomplete.
    pub internal_errors: u32,
}

#[derive(Debug)]
pub enum ScanError {
    RootNotFound(PathBuf),
    RootNotDirectory(PathBuf),
    RootUnreadable(PathBuf, io::Error),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::RootNotFound(_) => write!(f, "the selected location does not exist"),
            ScanError::RootNotDirectory(_) => write!(f, "the selected location is not a folder"),
            ScanError::RootUnreadable(_, e) => write!(f, "the selected folder cannot be read: {e}"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Resolves and validates a scan root, producing the absolute path the tree
/// will be rooted at. The root itself may be a link the user chose; links
/// *inside* it are not followed.
pub fn prepare_root(path: &Path) -> Result<PathBuf, ScanError> {
    let abs = std::path::absolute(path).map_err(|e| ScanError::RootUnreadable(path.into(), e))?;
    match std::fs::metadata(&abs) {
        Ok(m) if m.is_dir() => Ok(abs),
        Ok(_) => Err(ScanError::RootNotDirectory(abs)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ScanError::RootNotFound(abs)),
        Err(e) => Err(ScanError::RootUnreadable(abs, e)),
    }
}

/// Scans `root` and returns the finished tree. Blocks until done or
/// cancelled.
pub fn scan(
    root: &Path,
    opts: &ScanOptions,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<(Tree, ScanSummary), ScanError> {
    let root = prepare_root(root)?;
    let shared = Arc::new(RwLock::new(Tree::new(root)));
    let summary = scan_shared(&shared, opts, cancel, on_progress)?;
    let tree = match Arc::try_unwrap(shared) {
        Ok(lock) => lock.into_inner().unwrap_or_else(|e| e.into_inner()),
        Err(arc) => arc.read().unwrap_or_else(|e| e.into_inner()).clone(),
    };
    Ok((tree, summary))
}

/// Scans into a tree shared with other threads (e.g. the UI), which may take
/// read locks at any time to display partial results. The tree must be
/// freshly created with `Tree::new(prepare_root(..)?)`.
pub fn scan_shared(
    tree: &Arc<RwLock<Tree>>,
    opts: &ScanOptions,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<ScanSummary, ScanError> {
    let started = Instant::now();
    let root_path = tree
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .root_path()
        .to_path_buf();
    let root_meta = match std::fs::metadata(&root_path) {
        Ok(m) if m.is_dir() => m,
        Ok(_) => return Err(ScanError::RootNotDirectory(root_path)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(ScanError::RootNotFound(root_path));
        }
        Err(e) => return Err(ScanError::RootUnreadable(root_path, e)),
    };
    if let Err(e) = std::fs::read_dir(&root_path) {
        return Err(ScanError::RootUnreadable(root_path, e));
    }

    let identity_min_size = match opts.hard_links {
        HardLinkMode::Off => None,
        HardLinkMode::Thorough => Some(1),
        HardLinkMode::Auto => Some(opts.hard_link_min_size.max(1)),
    };
    let ctx = VolumeContext::new(
        &root_path,
        &root_meta,
        opts.cross_filesystems,
        identity_min_size,
    );
    let coverage = match ctx.identity_min_size {
        None => HardLinkCoverage::NotChecked,
        Some(n) if n <= 1 => HardLinkCoverage::AllFiles,
        Some(n) => HardLinkCoverage::FilesAtLeast(n),
    };
    let threads = resolve_threads(opts.threads);
    let listed = AtomicU64::new(0);

    let (job_tx, job_rx) = chan::unbounded::<Job>();
    let (res_tx, res_rx) = chan::bounded::<Listing>(threads * 32);
    job_tx
        .send(Job {
            dir: ROOT,
            path: root_path,
        })
        .expect("receiver alive");

    let mut agg = Aggregator::new(opts, &ctx, started);

    let internal_errors = std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(threads);
        for i in 0..threads {
            let jobs = job_rx.clone();
            let results = res_tx.clone();
            let ctx = &ctx;
            let listed = &listed;
            let handle = std::thread::Builder::new()
                .name(format!("tessera-scan-{i}"))
                .spawn_scoped(s, move || worker(jobs, results, ctx, cancel, listed))
                .expect("spawn scanner thread");
            handles.push(handle);
        }
        drop(res_tx);
        drop(job_rx);

        agg.run(tree, job_tx, &res_rx, cancel, &listed, on_progress);
        drop(res_rx);

        handles
            .into_iter()
            .map(|h| h.join())
            .filter(Result::is_err)
            .count() as u32
    });

    let mut t = tree.write().unwrap_or_else(|e| e.into_inner());
    t.finalize();
    let progress = agg.snapshot(&t, &listed);
    drop(t);
    on_progress(&progress);

    Ok(ScanSummary {
        progress,
        cancelled: agg.cancelled,
        elapsed: started.elapsed(),
        time_to_first_result: agg.first_result,
        issues: agg.issues,
        issues_not_recorded: agg.issues_not_recorded,
        hard_links: coverage,
        threads,
        cluster_size: ctx.cluster_size,
        internal_errors: internal_errors + agg.lost_workers,
    })
}

fn resolve_threads(requested: usize) -> usize {
    if requested > 0 {
        return requested.min(64);
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8)
}

struct Job {
    dir: NodeId,
    path: PathBuf,
}

struct Entry {
    name: OsString,
    info: EntryInfo,
}

struct Listing {
    dir: NodeId,
    path: PathBuf,
    /// `Err` when the directory could not be opened.
    entries: Result<Vec<Entry>, io::Error>,
    /// Indices of entries whose metadata failed, with the error.
    entry_errors: Vec<(usize, io::Error)>,
    /// Listing ended early with this error.
    read_error: Option<io::Error>,
    /// Listing was cut short by cancellation.
    truncated: bool,
}

fn worker(
    jobs: chan::Receiver<Job>,
    results: chan::Sender<Listing>,
    ctx: &VolumeContext,
    cancel: &CancelToken,
    listed: &AtomicU64,
) {
    while let Ok(job) = jobs.recv() {
        if cancel.is_cancelled() {
            break;
        }
        let listing = read_listing(job, ctx, cancel, listed);
        if results.send(listing).is_err() {
            break;
        }
    }
}

fn read_listing(
    job: Job,
    ctx: &VolumeContext,
    cancel: &CancelToken,
    listed: &AtomicU64,
) -> Listing {
    let mut listing = Listing {
        dir: job.dir,
        path: job.path,
        entries: Ok(Vec::new()),
        entry_errors: Vec::new(),
        read_error: None,
        truncated: false,
    };
    let rd = match std::fs::read_dir(&listing.path) {
        Ok(rd) => rd,
        Err(e) => {
            listing.entries = Err(e);
            return listing;
        }
    };
    let mut entries = Vec::new();
    let mut unreported = 0u64;
    for item in rd {
        match item {
            Ok(de) => {
                let name = de.file_name();
                let info = match platform::describe(&de, &listing.path, ctx) {
                    Ok(info) => info,
                    Err(e) => {
                        listing.entry_errors.push((entries.len(), e));
                        EntryInfo::unknown(platform::guess_kind(&de))
                    }
                };
                entries.push(Entry { name, info });
                unreported += 1;
                if unreported == 512 {
                    listed.fetch_add(unreported, Ordering::Relaxed);
                    unreported = 0;
                    if cancel.is_cancelled() {
                        listing.truncated = true;
                        break;
                    }
                }
            }
            Err(e) => {
                listing.read_error = Some(e);
                break;
            }
        }
    }
    listed.fetch_add(unreported, Ordering::Relaxed);
    listing.entries = Ok(entries);
    listing
}

struct Aggregator {
    started: Instant,
    interval: Duration,
    max_issues: usize,
    dedupe_files: bool,
    seen_files: HashSet<(u64, u64)>,
    seen_dirs: HashSet<(u64, u64)>,
    pending: u64,
    links: u64,
    issue_count: u64,
    issues: Vec<ScanIssue>,
    issues_not_recorded: u64,
    first_result: Option<Duration>,
    cancelled: bool,
    lost_workers: u32,
}

impl Aggregator {
    fn new(opts: &ScanOptions, ctx: &VolumeContext, started: Instant) -> Self {
        Aggregator {
            started,
            interval: opts.progress_interval.max(Duration::from_millis(10)),
            max_issues: opts.max_recorded_issues,
            dedupe_files: ctx.identity_min_size.is_some(),
            seen_files: HashSet::new(),
            seen_dirs: HashSet::new(),
            pending: 1,
            links: 0,
            issue_count: 0,
            issues: Vec::new(),
            issues_not_recorded: 0,
            first_result: None,
            cancelled: false,
            lost_workers: 0,
        }
    }

    fn run(
        &mut self,
        tree: &Arc<RwLock<Tree>>,
        job_tx: chan::Sender<Job>,
        res_rx: &chan::Receiver<Listing>,
        cancel: &CancelToken,
        listed: &AtomicU64,
        on_progress: &mut dyn FnMut(&Progress),
    ) {
        let mut last_report = Instant::now();
        let mut batch: Vec<Listing> = Vec::with_capacity(64);
        while self.pending > 0 {
            if cancel.is_cancelled() {
                self.cancelled = true;
                break;
            }
            match res_rx.recv_timeout(self.interval) {
                Ok(first) => {
                    batch.push(first);
                    while batch.len() < 64 {
                        match res_rx.try_recv() {
                            Ok(l) => batch.push(l),
                            Err(_) => break,
                        }
                    }
                    let mut t = tree.write().unwrap_or_else(|e| e.into_inner());
                    for listing in batch.drain(..) {
                        self.apply(&mut t, listing, &job_tx);
                    }
                    if self.first_result.is_none() {
                        self.first_result = Some(self.started.elapsed());
                    }
                }
                Err(chan::RecvTimeoutError::Timeout) => {}
                Err(chan::RecvTimeoutError::Disconnected) => {
                    // Every worker stopped while work remained.
                    self.lost_workers += 1;
                    break;
                }
            }
            if last_report.elapsed() >= self.interval {
                let snap = {
                    let t = tree.read().unwrap_or_else(|e| e.into_inner());
                    self.snapshot(&t, listed)
                };
                on_progress(&snap);
                last_report = Instant::now();
            }
        }
        if cancel.is_cancelled() {
            self.cancelled = true;
        }
        drop(job_tx);
    }

    fn record(&mut self, node: NodeId, err: &io::Error, stage: IssueStage) {
        self.issue_count += 1;
        if self.issues.len() >= self.max_issues {
            self.issues_not_recorded += 1;
            return;
        }
        let kind = match err.kind() {
            io::ErrorKind::PermissionDenied => IssueKind::PermissionDenied,
            io::ErrorKind::NotFound => IssueKind::NotFound,
            _ => IssueKind::Other,
        };
        self.issues.push(ScanIssue {
            node,
            kind,
            stage,
            os_code: err.raw_os_error(),
        });
    }

    fn apply(&mut self, t: &mut Tree, listing: Listing, job_tx: &chan::Sender<Job>) {
        self.pending -= 1;
        let dir = listing.dir;
        let entries = match listing.entries {
            Ok(e) => e,
            Err(err) => {
                let n = &mut t.nodes[dir as usize];
                n.flags = (n.flags & !flags::PENDING) | flags::UNREADABLE;
                self.record(dir, &err, IssueStage::OpenDirectory);
                return;
            }
        };

        let first = t.nodes.len() as NodeId;
        let count = entries.len() as u32;
        let (mut add_l, mut add_a, mut add_f, mut add_d) = (0u64, 0u64, 0u32, 0u32);

        for e in entries {
            let id = t.nodes.len() as NodeId;
            let info = e.info;
            let mut f = info.flags;
            match info.kind {
                NodeKind::File => {
                    add_f += 1;
                    if self.dedupe_files
                        && info.nlink > 1
                        && let Some(key) = info.identity
                    {
                        if self.seen_files.insert(key) {
                            f |= flags::HARD_LINKED;
                        } else {
                            f |= flags::COUNTED_ELSEWHERE | flags::HARD_LINKED;
                        }
                    }
                    if f & flags::COUNTED_ELSEWHERE == 0 {
                        add_l += info.logical;
                        add_a += info.allocated;
                    }
                }
                NodeKind::Dir => {
                    add_d += 1;
                    let revisit = info.identity.is_some_and(|k| !self.seen_dirs.insert(k));
                    if info.other_fs {
                        f |= flags::OTHER_FILESYSTEM;
                    } else if revisit {
                        f |= flags::COUNTED_ELSEWHERE;
                    } else if f & flags::METADATA_UNAVAILABLE == 0 {
                        f |= flags::PENDING;
                        let path = listing.path.join(&e.name);
                        if job_tx.send(Job { dir: id, path }).is_ok() {
                            self.pending += 1;
                        }
                    } else {
                        f |= flags::UNREADABLE;
                    }
                }
                NodeKind::Link | NodeKind::Other => {
                    self.links += u64::from(info.kind == NodeKind::Link);
                    add_l += info.logical;
                    add_a += info.allocated;
                }
            }
            t.nodes.push(Node {
                name: e.name.into_boxed_os_str(),
                parent: dir,
                first_child: NO_NODE,
                child_count: 0,
                kind: info.kind,
                flags: f,
                logical: info.logical,
                allocated: info.allocated,
                files: 0,
                dirs: 0,
                modified: info.modified,
            });
        }

        {
            let n = &mut t.nodes[dir as usize];
            n.flags &= !flags::PENDING;
            if count > 0 {
                n.first_child = first;
                n.child_count = count;
            }
            if listing.read_error.is_some() || listing.truncated {
                n.flags |= flags::PARTIAL;
            }
        }
        if let Some(err) = &listing.read_error {
            self.record(dir, err, IssueStage::ReadDirectory);
        }
        for (idx, err) in &listing.entry_errors {
            self.record(first + *idx as NodeId, err, IssueStage::ReadMetadata);
        }

        // Add this listing's direct contents to the directory and all
        // ancestors. Cost is O(depth) per directory, not per file.
        let mut cur = dir;
        while cur != NO_NODE {
            let n = &mut t.nodes[cur as usize];
            n.logical += add_l;
            n.allocated += add_a;
            n.files += add_f;
            n.dirs += add_d;
            cur = n.parent;
        }
    }

    fn snapshot(&self, t: &Tree, listed: &AtomicU64) -> Progress {
        let root = t.root();
        Progress {
            files: u64::from(root.files),
            dirs: u64::from(root.dirs),
            links: self.links,
            logical_bytes: root.logical,
            allocated_bytes: root.allocated,
            issues: self.issue_count,
            dirs_pending: self.pending,
            entries_listed: listed.load(Ordering::Relaxed),
            elapsed: self.started.elapsed(),
        }
    }
}
