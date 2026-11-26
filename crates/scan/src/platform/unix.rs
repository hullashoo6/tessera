//! macOS / Unix adapter.
//!
//! `DirEntry::metadata()` performs `lstat`, which never follows symbolic
//! links and never reads file contents, so iCloud "dataless" files are not
//! downloaded. `st_blocks * 512` is the filesystem-reported allocation.
//!
//! Known limitations (reported, not hidden):
//! * APFS clones share extents; each clone reports its full allocation, so
//!   totals may exceed the physical space used. Not detectable via lstat.
//! * APFS snapshots and purgeable space are not attributed to any file.
//! * Directory entries' own metadata blocks are not included in totals.

use super::{EntryInfo, VolumeContext, VolumeInfo, VolumeKind, VolumeSpace, unix_seconds};
use crate::model::NodeKind;
#[cfg(target_os = "macos")]
use crate::model::flags;
use std::ffi::CString;
use std::fs::{DirEntry, Metadata};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub(super) fn volume_context(
    _root: &Path,
    root_meta: &Metadata,
    cross_filesystems: bool,
    identity_min_size: Option<u64>,
) -> VolumeContext {
    VolumeContext {
        cluster_size: None,
        root_device: Some(root_meta.dev()),
        cross_filesystems,
        // lstat already returns (dev, ino, nlink): checking is free, so all
        // files are checked whenever checking is enabled.
        identity_min_size: identity_min_size.map(|_| 1),
    }
}

pub(super) fn describe(
    entry: &DirEntry,
    _dir: &Path,
    ctx: &VolumeContext,
) -> io::Result<EntryInfo> {
    let md = entry.metadata()?;
    let ft = md.file_type();
    let kind = if ft.is_dir() {
        NodeKind::Dir
    } else if ft.is_file() {
        NodeKind::File
    } else if ft.is_symlink() {
        NodeKind::Link
    } else {
        NodeKind::Other
    };
    let is_dir = kind == NodeKind::Dir;
    let mut info = EntryInfo {
        kind,
        logical: if is_dir { 0 } else { md.len() },
        allocated: if is_dir {
            0
        } else {
            md.blocks().saturating_mul(512)
        },
        flags: 0,
        modified: unix_seconds(&md),
        identity: None,
        nlink: md.nlink(),
        other_fs: false,
    };
    if is_dir {
        info.identity = Some((md.dev(), md.ino()));
        if let Some(root_dev) = ctx.root_device
            && md.dev() != root_dev
            && !ctx.cross_filesystems
        {
            info.other_fs = true;
        }
    } else if kind == NodeKind::File && ctx.identity_min_size.is_some() {
        info.identity = Some((md.dev(), md.ino()));
    }

    #[cfg(target_os = "macos")]
    {
        const SF_DATALESS: u32 = 0x4000_0000;
        if std::os::macos::fs::MetadataExt::st_flags(&md) & SF_DATALESS != 0 {
            info.flags |= flags::CLOUD_ONLY;
        }
    }
    Ok(info)
}

pub(super) fn volume_space(path: &Path) -> io::Result<VolumeSpace> {
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    // SAFETY: zeroed statvfs is a valid out-parameter; `c` is NUL-terminated.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let unit = if s.f_frsize > 0 {
        s.f_frsize as u64
    } else {
        s.f_bsize as u64
    };
    Ok(VolumeSpace {
        total: (s.f_blocks as u64).saturating_mul(unit),
        available: (s.f_bavail as u64).saturating_mul(unit),
        free: (s.f_bfree as u64).saturating_mul(unit),
    })
}

#[cfg(target_os = "macos")]
pub(super) fn list_volumes() -> Vec<VolumeInfo> {
    use std::ffi::CStr;
    const MNT_NOWAIT: libc::c_int = 2;
    const MNT_LOCAL: u32 = 0x0000_1000;
    let mut buf: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo returns a pointer to an internal buffer valid until
    // the next call on this thread; we copy what we need immediately.
    let n = unsafe { libc::getmntinfo(&mut buf, MNT_NOWAIT) };
    if n <= 0 || buf.is_null() {
        return fallback_volumes();
    }
    let mounts = unsafe { std::slice::from_raw_parts(buf, n as usize) };
    let mut out = Vec::new();
    for m in mounts {
        let on = unsafe { CStr::from_ptr(m.f_mntonname.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let fs = unsafe { CStr::from_ptr(m.f_fstypename.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let local = m.f_flags & MNT_LOCAL != 0;
        let user_visible = on == "/" || on.starts_with("/Volumes/");
        if !user_visible || fs == "devfs" || fs == "autofs" {
            continue;
        }
        let unit = m.f_bsize as u64;
        let space = Some(VolumeSpace {
            total: m.f_blocks.saturating_mul(unit),
            available: m.f_bavail.saturating_mul(unit),
            free: m.f_bfree.saturating_mul(unit),
        });
        out.push(VolumeInfo {
            label: Path::new(&on)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned()),
            mount_point: PathBuf::from(on),
            file_system: Some(fs),
            kind: if local {
                VolumeKind::Fixed
            } else {
                VolumeKind::Network
            },
            space,
        });
    }
    out
}

#[cfg(not(target_os = "macos"))]
pub(super) fn list_volumes() -> Vec<VolumeInfo> {
    fallback_volumes()
}

fn fallback_volumes() -> Vec<VolumeInfo> {
    let root = PathBuf::from("/");
    let space = volume_space(&root).ok();
    vec![VolumeInfo {
        mount_point: root,
        label: None,
        file_system: None,
        kind: VolumeKind::Unknown,
        space,
    }]
}
