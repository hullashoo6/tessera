//! Windows adapter.
//!
//! `std::fs::read_dir` on Windows is backed by FindFirstFileExW/FindNextFileW,
//! and `DirEntry::metadata()` is built from the returned find data, so the
//! common path costs no extra system call per entry and never opens files.
//!
//! * Allocated size: NTFS does not report allocation in find data. For
//!   ordinary files we round the logical size up to the cluster size and flag
//!   the value as an estimate (very small files stored inside the MFT are
//!   over-estimated). For sparse, compressed and non-link reparse-point files
//!   (e.g. WOF/CompactOS) we ask GetCompressedFileSizeW, which reports the
//!   real on-disk size without reading data.
//! * Cloud placeholders (OneDrive and other Cloud Files providers) carry the
//!   OFFLINE / RECALL_ON_OPEN / RECALL_ON_DATA_ACCESS attributes. We never
//!   open them and report zero bytes on disk.
//! * Symbolic links, junctions and volume mount points are "name surrogate"
//!   reparse points, which std reports as symlinks. They are never followed.
//! * Hard links: link counts require opening each file, so they are only
//!   collected in `HardLinkMode::Thorough`.

use super::{EntryInfo, VolumeContext, VolumeInfo, VolumeKind, VolumeSpace, unix_seconds};
use crate::model::{NodeKind, flags};
use std::fs::{DirEntry, Metadata};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, GetCompressedFileSizeW, GetDiskFreeSpaceExW,
    GetDiskFreeSpaceW, GetDriveTypeW, GetFileInformationByHandle, GetLogicalDriveStringsW,
    GetVolumeInformationW, GetVolumePathNameW,
};

const ATTR_HIDDEN: u32 = 0x2;
const ATTR_SYSTEM: u32 = 0x4;
const ATTR_SPARSE: u32 = 0x200;
const ATTR_REPARSE_POINT: u32 = 0x400;
const ATTR_COMPRESSED: u32 = 0x800;
const ATTR_OFFLINE: u32 = 0x1000;
const ATTR_RECALL_ON_OPEN: u32 = 0x4_0000;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
const CLOUD_MASK: u32 = ATTR_OFFLINE | ATTR_RECALL_ON_OPEN | ATTR_RECALL_ON_DATA_ACCESS;

const FILE_READ_ATTRIBUTES: u32 = 0x80;
const SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
const OPEN_EXISTING: u32 = 3;
const FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

const DEFAULT_CLUSTER: u64 = 4096;

pub(super) fn volume_context(
    root: &Path,
    _root_meta: &Metadata,
    cross_filesystems: bool,
    identity_min_size: Option<u64>,
) -> VolumeContext {
    VolumeContext {
        cluster_size: cluster_size(root),
        root_device: None,
        cross_filesystems,
        identity_min_size,
    }
}

pub(super) fn describe(entry: &DirEntry, dir: &Path, ctx: &VolumeContext) -> io::Result<EntryInfo> {
    let md = entry.metadata()?;
    let attrs = md.file_attributes();
    let ft = md.file_type();
    let kind = if ft.is_symlink() {
        NodeKind::Link
    } else if ft.is_dir() {
        NodeKind::Dir
    } else if ft.is_file() {
        NodeKind::File
    } else {
        NodeKind::Other
    };

    let mut info = EntryInfo {
        kind,
        logical: 0,
        allocated: 0,
        flags: if attrs & (ATTR_HIDDEN | ATTR_SYSTEM) != 0 {
            flags::HIDDEN_OR_SYSTEM
        } else {
            0
        },
        modified: unix_seconds(&md),
        identity: None,
        nlink: 0,
        other_fs: false,
    };
    if kind != NodeKind::File {
        return Ok(info);
    }

    let len = md.len();
    info.logical = len;

    if attrs & CLOUD_MASK != 0 {
        info.flags |= flags::CLOUD_ONLY | flags::ALLOC_ESTIMATED;
        return Ok(info);
    }
    if attrs & ATTR_SPARSE != 0 {
        info.flags |= flags::SPARSE;
    }
    if attrs & ATTR_COMPRESSED != 0 {
        info.flags |= flags::COMPRESSED;
    }

    let special = attrs & (ATTR_SPARSE | ATTR_COMPRESSED | ATTR_REPARSE_POINT) != 0;
    let want_identity = len > 0 && ctx.identity_min_size.is_some_and(|min| len >= min);
    let wide = if special || want_identity {
        Some(wide_verbatim(&dir.join(entry.file_name())))
    } else {
        None
    };

    let measured = match (&wide, special) {
        (Some(w), true) => compressed_size(w),
        _ => None,
    };
    match measured {
        Some(bytes) => info.allocated = bytes,
        None => {
            let c = ctx.cluster_size.unwrap_or(DEFAULT_CLUSTER);
            info.allocated = len.div_ceil(c).saturating_mul(c);
            info.flags |= flags::ALLOC_ESTIMATED;
        }
    }

    if want_identity && let Some((volume, index, links)) = wide.as_deref().and_then(file_identity) {
        info.identity = Some((volume, index));
        info.nlink = links;
    }
    Ok(info)
}

/// NUL-terminated UTF-16 path in `\\?\` form so long paths and names with
/// trailing dots/spaces are addressed exactly.
pub(crate) fn wide_verbatim(path: &Path) -> Vec<u16> {
    let s: Vec<u16> = path.as_os_str().encode_wide().collect();
    let starts = |p: &str| {
        let p: Vec<u16> = p.encode_utf16().collect();
        s.len() >= p.len() && s[..p.len()] == p[..]
    };
    let mut out: Vec<u16> = Vec::with_capacity(s.len() + 9);
    if starts(r"\\?\") || starts(r"\\.\") {
        out.extend_from_slice(&s);
    } else if starts(r"\\") {
        out.extend(r"\\?\UNC\".encode_utf16());
        out.extend_from_slice(&s[2..]);
    } else if s.len() >= 3 && s[1] == u16::from(b':') && s[2] == u16::from(b'\\') {
        out.extend(r"\\?\".encode_utf16());
        out.extend_from_slice(&s);
    } else {
        out.extend_from_slice(&s);
    }
    out.push(0);
    out
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn compressed_size(wide: &[u16]) -> Option<u64> {
    let mut high: u32 = 0;
    // SAFETY: `wide` is NUL-terminated; `high` is a valid out pointer.
    let low = unsafe { GetCompressedFileSizeW(wide.as_ptr(), &mut high) };
    if low == u32::MAX {
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(0) {
            return None;
        }
    }
    Some((u64::from(high) << 32) | u64::from(low))
}

/// (volume serial, file index, link count). Opens the file for attribute
/// access only, without following reparse points; never reads contents.
fn file_identity(wide: &[u16]) -> Option<(u64, u64, u64)> {
    // SAFETY: valid NUL-terminated path; null security attributes/template;
    // handle closed before returning.
    unsafe {
        let h = CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            SHARE_ALL,
            std::ptr::null(),
            OPEN_EXISTING,
            FLAG_BACKUP_SEMANTICS | FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        );
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        let ok = GetFileInformationByHandle(h, &mut info);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        Some((
            u64::from(info.dwVolumeSerialNumber),
            (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            u64::from(info.nNumberOfLinks),
        ))
    }
}

fn volume_root(path: &Path) -> Option<Vec<u16>> {
    let w = wide(path);
    let mut buf = vec![0u16; 1024];
    // SAFETY: buffer length passed matches allocation.
    let ok = unsafe { GetVolumePathNameW(w.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    buf.truncate(end);
    buf.push(0);
    Some(buf)
}

fn cluster_size(path: &Path) -> Option<u64> {
    let root = volume_root(path)?;
    let (mut spc, mut bps, mut free, mut total) = (0u32, 0u32, 0u32, 0u32);
    // SAFETY: valid NUL-terminated root, valid out pointers.
    let ok = unsafe { GetDiskFreeSpaceW(root.as_ptr(), &mut spc, &mut bps, &mut free, &mut total) };
    if ok == 0 {
        return None;
    }
    Some(u64::from(spc) * u64::from(bps)).filter(|&c| c > 0)
}

pub(super) fn volume_space(path: &Path) -> io::Result<VolumeSpace> {
    let w = wide(path);
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: valid NUL-terminated path, valid out pointers.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(VolumeSpace {
        total,
        available: avail,
        free,
    })
}

fn utf16_until_nul(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub(super) fn list_volumes() -> Vec<VolumeInfo> {
    let mut buf = vec![0u16; 512];
    // SAFETY: buffer length matches allocation.
    let n = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) } as usize;
    if n == 0 || n > buf.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for root in buf[..n].split(|&c| c == 0).filter(|s| !s.is_empty()) {
        let mut root_w: Vec<u16> = root.to_vec();
        root_w.push(0);
        // SAFETY: NUL-terminated.
        let kind = match unsafe { GetDriveTypeW(root_w.as_ptr()) } {
            1 => continue, // DRIVE_NO_ROOT_DIR
            2 => VolumeKind::Removable,
            3 => VolumeKind::Fixed,
            4 => VolumeKind::Network,
            5 => VolumeKind::Optical,
            6 => VolumeKind::RamDisk,
            _ => VolumeKind::Unknown,
        };
        let mut name = vec![0u16; 261];
        let mut fs = vec![0u16; 261];
        let (mut serial, mut maxlen, mut fsflags) = (0u32, 0u32, 0u32);
        // SAFETY: buffer sizes passed match allocations.
        let ok = unsafe {
            GetVolumeInformationW(
                root_w.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                &mut serial,
                &mut maxlen,
                &mut fsflags,
                fs.as_mut_ptr(),
                fs.len() as u32,
            )
        };
        let (label, file_system) = if ok != 0 {
            let l = utf16_until_nul(&name);
            (
                Some(l).filter(|s| !s.is_empty()),
                Some(utf16_until_nul(&fs)).filter(|s| !s.is_empty()),
            )
        } else {
            (None, None)
        };
        let mount_point = PathBuf::from(String::from_utf16_lossy(root));
        let space = volume_space(&mount_point).ok();
        out.push(VolumeInfo {
            mount_point,
            label,
            file_system,
            kind,
            space,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::wide_verbatim;
    use std::path::Path;

    fn s(w: Vec<u16>) -> String {
        String::from_utf16(&w[..w.len() - 1]).unwrap()
    }

    #[test]
    fn verbatim_forms() {
        assert_eq!(s(wide_verbatim(Path::new(r"C:\a\b"))), r"\\?\C:\a\b");
        assert_eq!(
            s(wide_verbatim(Path::new(r"\\srv\share\x"))),
            r"\\?\UNC\srv\share\x"
        );
        assert_eq!(
            s(wide_verbatim(Path::new(r"\\?\C:\already"))),
            r"\\?\C:\already"
        );
        assert_eq!(*wide_verbatim(Path::new(r"C:\a")).last().unwrap(), 0);
    }
}
