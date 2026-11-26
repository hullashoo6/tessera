//! Platform adapters. Everything OS-specific about reading entry metadata,
//! volume information and file identity lives below this module.

use crate::model::NodeKind;
use std::fs::DirEntry;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

/// What the scanner learns about one directory entry.
#[derive(Debug, Clone)]
pub(crate) struct EntryInfo {
    pub kind: NodeKind,
    pub logical: u64,
    pub allocated: u64,
    pub flags: u16,
    pub modified: i64,
    /// (device or volume id, file id) when known.
    pub identity: Option<(u64, u64)>,
    /// Hard-link count when known (0 = unknown).
    pub nlink: u64,
    /// Directory lies on a different filesystem than the scan root.
    pub other_fs: bool,
}

impl EntryInfo {
    pub(crate) fn unknown(kind: NodeKind) -> Self {
        EntryInfo {
            kind,
            logical: 0,
            allocated: 0,
            flags: crate::model::flags::METADATA_UNAVAILABLE,
            modified: i64::MIN,
            identity: None,
            nlink: 0,
            other_fs: false,
        }
    }
}

/// Per-scan facts about the root's volume, shared by all workers.
#[derive(Debug, Clone)]
pub(crate) struct VolumeContext {
    /// Allocation unit used for size estimates when the OS does not report
    /// allocated size directly.
    pub cluster_size: Option<u64>,
    /// Device id of the root (Unix); Windows needs none because mount
    /// points are reparse points and are never followed.
    #[cfg_attr(windows, allow(dead_code))]
    pub root_device: Option<u64>,
    #[cfg_attr(windows, allow(dead_code))]
    pub cross_filesystems: bool,
    /// Files at least this large have their hard-link identity collected;
    /// `None` disables hard-link checks. Adapters may lower this when
    /// identity is free (Unix reports `Some(1)`: all files).
    pub identity_min_size: Option<u64>,
}

impl VolumeContext {
    pub(crate) fn new(
        root: &Path,
        root_meta: &std::fs::Metadata,
        cross_filesystems: bool,
        identity_min_size: Option<u64>,
    ) -> Self {
        imp::volume_context(root, root_meta, cross_filesystems, identity_min_size)
    }
}

/// Describes an entry. Must not open or read file contents, so cloud
/// placeholders are never downloaded.
#[inline]
pub(crate) fn describe(entry: &DirEntry, dir: &Path, ctx: &VolumeContext) -> io::Result<EntryInfo> {
    imp::describe(entry, dir, ctx)
}

/// Best guess at the kind of an entry whose metadata could not be read.
pub(crate) fn guess_kind(entry: &DirEntry) -> NodeKind {
    match entry.file_type() {
        Ok(t) if t.is_symlink() => NodeKind::Link,
        Ok(t) if t.is_dir() => NodeKind::Dir,
        Ok(t) if t.is_file() => NodeKind::File,
        _ => NodeKind::Other,
    }
}

/// Modification time in whole seconds relative to the Unix epoch.
pub(crate) fn unix_seconds(md: &std::fs::Metadata) -> i64 {
    match md.modified() {
        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
        },
        Err(_) => i64::MIN,
    }
}

/// Whether hard-link identity can be gathered without extra system calls.
pub fn hard_links_cheap() -> bool {
    cfg!(unix)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeKind {
    Fixed,
    Removable,
    Network,
    Optical,
    RamDisk,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub mount_point: PathBuf,
    pub label: Option<String>,
    pub file_system: Option<String>,
    pub kind: VolumeKind,
    pub space: Option<VolumeSpace>,
}

/// Capacity as reported by the operating system (not by summing files).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeSpace {
    pub total: u64,
    /// Free bytes available to the current user (may exclude reserved space).
    pub available: u64,
    /// Total free bytes on the volume.
    pub free: u64,
}

impl VolumeSpace {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }
}

/// Lists local drives / mounted volumes.
pub fn list_volumes() -> Vec<VolumeInfo> {
    imp::list_volumes()
}

/// Capacity of the volume containing `path`. Gives up after 3 seconds so a
/// hung network drive cannot stall the caller.
pub fn volume_space(path: &Path) -> io::Result<VolumeSpace> {
    let p = path.to_path_buf();
    with_timeout(3000, move || imp::volume_space(&p)).unwrap_or_else(|| {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the drive did not respond",
        ))
    })
}

/// Runs `f` on its own thread and waits at most `ms` milliseconds.
pub(crate) fn with_timeout<T: Send + 'static>(
    ms: u64,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("tessera-volume".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .ok()?;
    rx.recv_timeout(std::time::Duration::from_millis(ms)).ok()
}
