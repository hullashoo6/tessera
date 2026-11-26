//! Duplicate file detection.
//!
//! 1. Group files by exact size (free: from the scan).
//! 2. For same-size files, hash the first and last 64 KiB.
//! 3. For files whose partial hashes match, hash the whole file (BLAKE3).
//!
//! Only files with identical full hashes are reported as duplicates.
//!
//! Reads file contents locally; nothing leaves the computer. Online-only
//! (cloud placeholder) files are never opened, so nothing is downloaded.
//! Extra hard links are not duplicates (they share storage) and are skipped.
//! Files that changed since the scan are skipped. Nothing is removed here.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use tessera_scan::{NodeId, NodeKind, Tree, flags};

const EDGE: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Files smaller than this are ignored.
    pub min_size: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { min_size: 1 << 20 }
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

fn partial_hash(tree: &Tree, id: NodeId, read: &mut u64) -> io::Result<[u8; 32]> {
    let size = tree.node(id).logical;
    let mut f = open_unchanged(tree, id)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; EDGE as usize];
    let n = f.read(&mut buf)?;
    h.update(&buf[..n]);
    *read += n as u64;
    if size > 2 * EDGE {
        f.seek(SeekFrom::End(-(EDGE as i64)))?;
        let n = f.read(&mut buf)?;
        h.update(&buf[..n]);
        *read += n as u64;
    }
    Ok(*h.finalize().as_bytes())
}

fn full_hash(
    tree: &Tree,
    id: NodeId,
    cancel: &AtomicBool,
    read: &mut u64,
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
        *read += n as u64;
    }
    Ok(Some(*h.finalize().as_bytes()))
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Finds groups of identical files. `progress` receives bytes read so far.
pub fn find(
    tree: &Tree,
    opts: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Report {
    tessera_scan::background_priority();
    let mut report = Report::default();
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for id in 1..tree.len() as NodeId {
        if eligible(tree, id, opts.min_size.max(1)) {
            by_size.entry(tree.node(id).logical).or_default().push(id);
        }
    }
    let mut sizes: Vec<(u64, Vec<NodeId>)> =
        by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();
    // Largest first, so the most useful results arrive early.
    sizes.sort_by_key(|(s, _)| std::cmp::Reverse(*s));

    for (size, ids) in sizes {
        let mut by_partial: HashMap<[u8; 32], Vec<NodeId>> = HashMap::new();
        for id in ids {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                return finish(tree, report);
            }
            match partial_hash(tree, id, &mut report.bytes_read) {
                Ok(h) => by_partial.entry(h).or_default().push(id),
                Err(_) => report.skipped += 1,
            }
            report.files_compared += 1;
            progress(report.bytes_read);
        }
        for (_, candidates) in by_partial.into_iter().filter(|(_, v)| v.len() > 1) {
            let mut by_full: HashMap<[u8; 32], Vec<NodeId>> = HashMap::new();
            for id in candidates {
                match full_hash(tree, id, cancel, &mut report.bytes_read) {
                    Ok(Some(h)) => by_full.entry(h).or_default().push(id),
                    Ok(None) => {
                        report.cancelled = true;
                        return finish(tree, report);
                    }
                    Err(_) => report.skipped += 1,
                }
                progress(report.bytes_read);
            }
            for (h, ids) in by_full.into_iter().filter(|(_, v)| v.len() > 1) {
                report.groups.push(Group {
                    size,
                    hash: hex(&h),
                    ids,
                });
            }
        }
    }
    finish(tree, report)
}

fn finish(tree: &Tree, mut report: Report) -> Report {
    for g in &mut report.groups {
        g.ids.sort_by_key(|&id| (tree.node(id).modified, id));
    }
    report
        .groups
        .sort_by_key(|g| std::cmp::Reverse(g.reclaimable()));
    report
}
