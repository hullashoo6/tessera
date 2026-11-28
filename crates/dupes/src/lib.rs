//! Duplicate file detection.
//!
//! 1. Group files by exact size (free: from the scan).
//! 2. For same-size files, hash three small samples: the start, the middle
//!    and the end. Files that differ almost always differ there.
//! 3. For files whose samples match, hash the whole file (BLAKE3).
//!
//! Only files with identical full hashes are reported as duplicates.
//!
//! Several files are read at once (28 Nov 2026: one file at a time was too
//! slow on large drives), and progress reports exact totals for each stage,
//! so the screen can show how far along it is and what was found so far.
//!
//! Reads file contents locally; nothing leaves the computer. Online-only
//! (cloud placeholder) files are never opened, so nothing is downloaded.
//! Extra hard links are not duplicates (they share storage) and are skipped.
//! Files that changed since the scan are skipped. Nothing is removed here.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tessera_scan::{NodeId, NodeKind, Tree, flags};

/// Bytes read at each of the three sample places.
const SAMPLE: u64 = 16 * 1024;

/// Full hashes of the files of one set of possible copies.
type Hashed = Mutex<Vec<(NodeId, [u8; 32])>>;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Files smaller than this are ignored.
    pub min_size: u64,
    /// Files sampled at once (small reads; 0 = automatic).
    pub sample_threads: usize,
    /// Files read in full at once (long reads; 0 = automatic: two, which
    /// speeds up solid-state drives without making hard disks seek much).
    pub full_threads: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            min_size: 1 << 20,
            sample_threads: 0,
            full_threads: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Group {
    /// Size of each copy in bytes.
    pub size: u64,
    /// Hex BLAKE3 hash of the content.
    pub hash: String,
    /// The copies, oldest modification first.
    pub ids: Vec<NodeId>,
}

impl Group {
    /// Space that removing all but one copy could free.
    pub fn reclaimable(&self) -> u64 {
        self.size * (self.ids.len() as u64 - 1)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub groups: Vec<Group>,
    pub files_compared: u64,
    pub bytes_read: u64,
    /// Files that could not be read or changed since the scan.
    pub skipped: u64,
    pub cancelled: bool,
}

/// How far a search has got.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    /// 1: checking samples of files that share a size; 2: comparing the
    /// possible copies in full.
    pub stage: u8,
    /// Files finished in this stage, out of `files_total`.
    pub files_done: u64,
    pub files_total: u64,
    /// Bytes read in this stage, out of `bytes_total` (stage 2 only; the
    /// total is exact, as every possible copy is read in full).
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// Bytes read in all so far.
    pub bytes_read: u64,
    /// Duplicate groups confirmed so far, and the space they could free.
    pub groups: u64,
    pub reclaimable: u64,
}

fn eligible(tree: &Tree, id: NodeId, min: u64) -> bool {
    let n = tree.node(id);
    n.kind == NodeKind::File
        && n.logical >= min
        && n.flags
            & (flags::REMOVED
                | flags::COUNTED_ELSEWHERE
                | flags::CLOUD_ONLY
                | flags::METADATA_UNAVAILABLE)
            == 0
}

fn open_unchanged(tree: &Tree, id: NodeId) -> io::Result<File> {
    let path = tree.path_of(id);
    let md = std::fs::symlink_metadata(&path)?;
    if !md.is_file() || md.len() != tree.node(id).logical {
        return Err(io::Error::other("changed since the scan"));
    }
    File::open(path)
}

/// Hash of the start, middle and end of a file (the whole file when small).
fn sample_hash(tree: &Tree, id: NodeId, read: &AtomicU64) -> io::Result<[u8; 32]> {
    let size = tree.node(id).logical;
    let mut f = open_unchanged(tree, id)?;
    let mut h = blake3::Hasher::new();
    if size <= 3 * SAMPLE {
        let mut buf = Vec::with_capacity(size as usize);
        f.read_to_end(&mut buf)?;
        read.fetch_add(buf.len() as u64, Ordering::Relaxed);
        h.update(&buf);
        return Ok(*h.finalize().as_bytes());
    }
    let mut buf = vec![0u8; SAMPLE as usize];
    for at in [0, size / 2 - SAMPLE / 2, size - SAMPLE] {
        f.seek(SeekFrom::Start(at))?;
        f.read_exact(&mut buf)?;
        read.fetch_add(SAMPLE, Ordering::Relaxed);
        h.update(&buf);
    }
    Ok(*h.finalize().as_bytes())
}

fn full_hash(
    tree: &Tree,
    id: NodeId,
    cancel: &AtomicBool,
    read: &[&AtomicU64],
) -> io::Result<Option<[u8; 32]>> {
    let mut f = open_unchanged(tree, id)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        for r in read {
            r.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    Ok(Some(*h.finalize().as_bytes()))
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn threads(wanted: usize, auto: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    if wanted > 0 { wanted } else { auto.min(cores) }.max(1)
}

/// Runs `work(i)` for every i in 0..n on `workers` threads, calling `tick`
/// on this thread about every 100 ms until all are done.
fn in_parallel(n: usize, workers: usize, work: &(dyn Fn(usize) + Sync), tick: &mut dyn FnMut()) {
    let next = AtomicUsize::new(0);
    let running = AtomicUsize::new(workers);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                tessera_scan::background_priority();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    work(i);
                }
                running.fetch_sub(1, Ordering::Release);
            });
        }
        while running.load(Ordering::Acquire) > 0 {
            std::thread::sleep(Duration::from_millis(100));
            tick();
        }
    });
}

/// Finds groups of identical files. `progress` is called on the calling
/// thread about every 100 ms, and once at the end.
pub fn find(
    tree: &Tree,
    opts: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&Progress),
) -> Report {
    let mut report = Report::default();
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for id in 1..tree.len() as NodeId {
        if eligible(tree, id, opts.min_size.max(1)) {
            by_size.entry(tree.node(id).logical).or_default().push(id);
        }
    }
    // Largest first, so the most useful results arrive early.
    let mut sizes: Vec<(u64, Vec<NodeId>)> =
        by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();
    sizes.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    let sampled: Vec<NodeId> = sizes.iter().flat_map(|(_, v)| v.iter().copied()).collect();

    let read = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);
    let done = AtomicU64::new(0);
    let mut p = Progress {
        stage: 1,
        files_total: sampled.len() as u64,
        ..Progress::default()
    };
    progress(&p);

    // Stage 1: samples.
    let samples: Vec<Mutex<Option<[u8; 32]>>> = sampled.iter().map(|_| Mutex::new(None)).collect();
    let sample_one = |i: usize| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        match sample_hash(tree, sampled[i], &read) {
            Ok(h) => *samples[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(h),
            Err(_) => {
                skipped.fetch_add(1, Ordering::Relaxed);
            }
        }
        done.fetch_add(1, Ordering::Relaxed);
    };
    in_parallel(
        sampled.len(),
        threads(opts.sample_threads, 4),
        &sample_one,
        &mut || {
            p.files_done = done.load(Ordering::Relaxed);
            p.bytes_read = read.load(Ordering::Relaxed);
            progress(&p);
        },
    );
    report.files_compared = done.load(Ordering::Relaxed);
    if cancel.load(Ordering::Relaxed) {
        report.cancelled = true;
        return finish(tree, report, &read, &skipped);
    }

    // Possible copies: same size and same samples, largest first.
    let mut sets: Vec<(u64, Vec<NodeId>)> = Vec::new();
    let mut i = 0;
    for (size, ids) in &sizes {
        let mut by_sample: HashMap<[u8; 32], Vec<NodeId>> = HashMap::new();
        for &id in ids {
            if let Some(h) = *samples[i].lock().unwrap_or_else(|e| e.into_inner()) {
                by_sample.entry(h).or_default().push(id);
            }
            i += 1;
        }
        let mut groups: Vec<Vec<NodeId>> =
            by_sample.into_values().filter(|v| v.len() > 1).collect();
        groups.sort();
        sets.extend(groups.into_iter().map(|v| (*size, v)));
    }

    // Stage 2: full contents. Each set settles when its last file is read.
    let work: Vec<(usize, NodeId)> = sets
        .iter()
        .enumerate()
        .flat_map(|(s, (_, ids))| ids.iter().map(move |&id| (s, id)))
        .collect();
    let left: Vec<AtomicUsize> = sets
        .iter()
        .map(|(_, v)| AtomicUsize::new(v.len()))
        .collect();
    let hashed: Vec<Hashed> = sets.iter().map(|_| Mutex::new(Vec::new())).collect();
    let found: Mutex<Vec<Group>> = Mutex::new(Vec::new());
    let (groups_n, reclaim) = (AtomicU64::new(0), AtomicU64::new(0));
    let stage_bytes = AtomicU64::new(0);
    done.store(0, Ordering::Relaxed);
    p = Progress {
        stage: 2,
        files_total: work.len() as u64,
        bytes_total: work.iter().map(|&(s, _)| sets[s].0).sum(),
        bytes_read: read.load(Ordering::Relaxed),
        ..Progress::default()
    };
    progress(&p);
    let compare_one = |i: usize| {
        let (s, id) = work[i];
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        match full_hash(tree, id, cancel, &[&read, &stage_bytes]) {
            Ok(Some(h)) => hashed[s]
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((id, h)),
            Ok(None) => return,
            Err(_) => {
                skipped.fetch_add(1, Ordering::Relaxed);
            }
        }
        done.fetch_add(1, Ordering::Relaxed);
        if left[s].fetch_sub(1, Ordering::AcqRel) == 1 {
            let mut by_full: HashMap<[u8; 32], Vec<NodeId>> = HashMap::new();
            for &(id, h) in hashed[s].lock().unwrap_or_else(|e| e.into_inner()).iter() {
                by_full.entry(h).or_default().push(id);
            }
            for (h, ids) in by_full.into_iter().filter(|(_, v)| v.len() > 1) {
                let g = Group {
                    size: sets[s].0,
                    hash: hex(&h),
                    ids,
                };
                groups_n.fetch_add(1, Ordering::Relaxed);
                reclaim.fetch_add(g.reclaimable(), Ordering::Relaxed);
                found.lock().unwrap_or_else(|e| e.into_inner()).push(g);
            }
        }
    };
    in_parallel(
        work.len(),
        threads(opts.full_threads, 2),
        &compare_one,
        &mut || {
            p.files_done = done.load(Ordering::Relaxed);
            p.bytes_done = stage_bytes.load(Ordering::Relaxed);
            p.bytes_read = read.load(Ordering::Relaxed);
            p.groups = groups_n.load(Ordering::Relaxed);
            p.reclaimable = reclaim.load(Ordering::Relaxed);
            progress(&p);
        },
    );
    report.cancelled = cancel.load(Ordering::Relaxed);
    report.groups = found.into_inner().unwrap_or_else(|e| e.into_inner());
    p.files_done = done.load(Ordering::Relaxed);
    p.bytes_done = stage_bytes.load(Ordering::Relaxed);
    p.bytes_read = read.load(Ordering::Relaxed);
    p.groups = groups_n.load(Ordering::Relaxed);
    p.reclaimable = reclaim.load(Ordering::Relaxed);
    progress(&p);
    finish(tree, report, &read, &skipped)
}

fn finish(tree: &Tree, mut report: Report, read: &AtomicU64, skipped: &AtomicU64) -> Report {
    report.bytes_read = read.load(Ordering::Relaxed);
    report.skipped = skipped.load(Ordering::Relaxed);
    for g in &mut report.groups {
        g.ids.sort_by_key(|&id| (tree.node(id).modified, id));
    }
    report
        .groups
        .sort_by_key(|g| std::cmp::Reverse((g.reclaimable(), g.size)));
    report
}
