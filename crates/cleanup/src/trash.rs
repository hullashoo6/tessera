//! Moving items to the Recycle Bin / Trash. Never deletes permanently.

use std::io;
use std::path::Path;

/// Moves `path` to the Recycle Bin (Windows) or Trash (macOS).
///
/// On Windows, items the Recycle Bin cannot hold (for example files larger
/// than its capacity, or on drives without one) are refused: the operation
/// is aborted for that item instead of Windows deleting it permanently.
///
/// Returns where the item now sits inside the Recycle Bin when the system
/// reports it (Windows), so it can be put back later; `None` otherwise.
pub fn move_to_trash(path: &Path) -> io::Result<Option<std::path::PathBuf>> {
    imp::move_to_trash(path)
}

/// Moves several items at once, with one result per item in the same order,
/// exactly as `move_to_trash` would report it. On Windows the items share
/// one shell operation, which is far faster than one operation each; any
/// item the shared operation did not settle is moved on its own.
pub fn move_many_to_trash(paths: &[&Path]) -> Vec<io::Result<Option<std::path::PathBuf>>> {
    #[cfg(windows)]
    {
        imp::move_many_to_trash(paths)
    }
    #[cfg(not(windows))]
    {
        paths.iter().map(|p| imp::move_to_trash(p)).collect()
    }
}

/// The largest item, in bytes of full length, that the Recycle Bin of
/// `path`'s drive keeps. Windows deletes anything bigger permanently, and
/// with no confirmation shown it does so without asking (27 Nov 2026: a
/// 38 GB file went that way on a drive whose limit was 14 GB). `Some(0)`:
/// the drive has no Recycle Bin, or it is set to delete at once. `None`: no
/// such limit (the macOS Trash) or it could not be read.
pub fn recycle_limit(path: &Path) -> Option<u64> {
    imp::recycle_limit(path)
}

/// True when the Recycle Bin / Trash can keep an item of `bytes` (its full
/// length) at `path`.
pub fn fits_in_trash(path: &Path, bytes: u64) -> bool {
    match recycle_limit(path) {
        Some(0) => false,
        Some(limit) => bytes < limit,
        None => true,
    }
}

/// The error for an item the Recycle Bin cannot keep (shown as "Too large for
/// the Recycle Bin", with a separately warned Delete permanently).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn too_big() -> io::Error {
    io::Error::new(
        io::ErrorKind::StorageFull,
        "the Recycle Bin cannot hold it (too large, or the drive has no Recycle Bin), so it was left in place",
    )
}

/// Deletes items permanently, skipping the Recycle Bin / Trash. Only used
/// when the person explicitly chose "Delete permanently". One result per
/// item, in order; an item that is already gone counts as deleted.
pub fn delete_many_permanently(paths: &[&Path]) -> Vec<io::Result<()>> {
    imp::delete_many_permanently(paths)
}

/// Puts an item Tessera moved back where it was. Never overwrites: fails if
/// something already exists at `original`. Windows only for now.
pub fn put_back(recycled: &Path, original: &Path) -> io::Result<()> {
    imp::put_back(recycled, original)
}

#[cfg(windows)]
mod imp {
    use std::cell::Cell;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows::Win32::Foundation::{E_ABORT, RPC_E_CHANGED_MODE};
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::UI::Shell::{
        FOF_ALLOWUNDO, FOF_NO_UI, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation,
        IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
        SHCreateItemFromParsingName, SIGDN_FILESYSPATH, TSF_DELETE_RECYCLE_IF_POSSIBLE,
    };
    use windows::core::{HRESULT, PCWSTR, Ref, Result as WinResult, implement};

    /// Progress sink that vetoes any delete Windows would not recycle.
    #[implement(IFileOperationProgressSink)]
    struct RecycleGuard {
        refused: Cell<bool>,
        failed: Cell<Option<HRESULT>>,
        landed: std::cell::RefCell<Option<std::path::PathBuf>>,
    }

    impl IFileOperationProgressSink_Impl for RecycleGuard_Impl {
        fn StartOperations(&self) -> WinResult<()> {
            Ok(())
        }
        fn FinishOperations(&self, _hr: HRESULT) -> WinResult<()> {
            Ok(())
        }
        fn PreRenameItem(&self, _f: u32, _i: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostRenameItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _n: &PCWSTR,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn PreMoveItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
        ) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostMoveItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn PreCopyItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
        ) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostCopyItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn PreDeleteItem(&self, flags: u32, _i: Ref<IShellItem>) -> WinResult<()> {
            if flags & (TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32) == 0 {
                // Windows would delete this permanently. Refuse.
                self.refused.set(true);
                return Err(E_ABORT.into());
            }
            Ok(())
        }
        fn PostDeleteItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            hr: HRESULT,
            created: Ref<IShellItem>,
        ) -> WinResult<()> {
            if hr.is_err() {
                self.failed.set(Some(hr));
            } else if let Some(item) = created.as_ref() {
                // Where the item now lives inside the Recycle Bin.
                // SAFETY: the returned string is freed with CoTaskMemFree.
                unsafe {
                    if let Ok(p) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                        if let Ok(s) = p.to_string() {
                            *self.landed.borrow_mut() = Some(std::path::PathBuf::from(s));
                        }
                        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
                    }
                }
            }
            Ok(())
        }
        fn PreNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostNewItem(
            &self,
            _f: u32,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
            _t: &PCWSTR,
            _a: u32,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn UpdateProgress(&self, _total: u32, _done: u32) -> WinResult<()> {
            Ok(())
        }
        fn ResetTimer(&self) -> WinResult<()> {
            Ok(())
        }
        fn PauseTimer(&self) -> WinResult<()> {
            Ok(())
        }
        fn ResumeTimer(&self) -> WinResult<()> {
            Ok(())
        }
    }

    fn to_io(e: windows::core::Error) -> io::Error {
        hr_to_io(e.code())
    }

    /// Keeps the Windows error number when there is one, so the cause (file
    /// in use, access denied, ...) can be explained to the person.
    pub(super) fn hr_to_io(hr: HRESULT) -> io::Error {
        let v = hr.0 as u32;
        if v & 0xFFFF_0000 == 0x8007_0000 {
            return io::Error::from_raw_os_error((v & 0xFFFF) as i32);
        }
        // The shell's own copy-engine errors (sherrors.h, COPYENGINE_E_*),
        // which carry no system message: map them to the matching causes.
        match v {
            0x8027_0027 | 0x8027_0028 => io::Error::from_raw_os_error(32), // sharing violation
            0x8027_0021 | 0x8027_0022 | 0x8027_003F | 0x8027_0002 => {
                io::Error::from_raw_os_error(5) // access denied / needs administrator
            }
            0x8027_0023 => io::Error::new(io::ErrorKind::NotFound, "it is no longer there"),
            0x8027_001D | 0x8027_001E | 0x8027_0038 | 0x8027_003B | 0x8027_003C => {
                io::Error::from_raw_os_error(206) // path or name too long
            }
            0x8027_0036 | 0x8027_0037 | 0x8027_003A => io::Error::new(
                io::ErrorKind::StorageFull,
                "the Recycle Bin cannot hold it (too large, or the drive has no Recycle Bin)",
            ),
            0x8027_0042 | 0x8027_0045 | 0x8027_0046 => io::Error::from_raw_os_error(362), // cloud item unavailable
            0x8027_003D => io::Error::from_raw_os_error(145), // folder not empty
            0x8027_0025 => io::Error::other("the network location cannot be reached"),
            0x8027_0000 | 0x8027_0001 => io::Error::other("the operation was cancelled"),
            _ => {
                let text = windows::core::Error::from(hr).message();
                io::Error::other(if text.is_empty() {
                    format!("Windows error 0x{v:08X}")
                } else {
                    format!("{text} (0x{v:08X})")
                })
            }
        }
    }

    /// Comparable form of a path: no `\\?\` prefix, lower case.
    fn key(p: &str) -> String {
        p.strip_prefix(r"\\?\").unwrap_or(p).to_lowercase()
    }

    /// The key for one of our paths, as Windows will report it: the shell
    /// gives full names, so a path with short 8.3 parts (`C:\Users\RUNNER~1`)
    /// is expanded first; otherwise its result would not be matched and the
    /// item would be wrongly reported as already gone.
    fn path_key(p: &Path) -> String {
        key(&long_form(p))
    }

    /// `p` with any short 8.3 parts expanded. Works for an item that no longer
    /// exists too, through its folder.
    fn long_form(p: &Path) -> String {
        use windows::Win32::Storage::FileSystem::GetLongPathNameW;
        let expand = |p: &Path| -> Option<String> {
            let w = wide(p);
            let mut buf = vec![0u16; 1024];
            // SAFETY: `w` is NUL-terminated; `buf` is a valid buffer of its length.
            let n = unsafe { GetLongPathNameW(PCWSTR(w.as_ptr()), Some(&mut buf)) } as usize;
            (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]))
        };
        expand(p)
            .or_else(|| {
                let parent = expand(p.parent()?)?;
                let name = p.file_name()?.to_string_lossy();
                Some(format!("{}\\{name}", parent.trim_end_matches('\\')))
            })
            .unwrap_or_else(|| p.to_string_lossy().into_owned())
    }

    fn item_path(item: &IShellItem) -> Option<String> {
        // SAFETY: the returned string is freed with CoTaskMemFree.
        unsafe {
            let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let s = p.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
            s
        }
    }

    /// Progress sink for a batch: records each item's result, and vetoes
    /// (which stops the rest of the batch) any delete Windows would not recycle.
    #[implement(IFileOperationProgressSink)]
    struct BatchGuard {
        /// Permanent deletion was asked for: nothing to veto.
        permanent: bool,
        refused: Cell<bool>,
        results: std::cell::RefCell<
            std::collections::HashMap<String, Result<Option<std::path::PathBuf>, HRESULT>>,
        >,
    }

    impl IFileOperationProgressSink_Impl for BatchGuard_Impl {
        fn StartOperations(&self) -> WinResult<()> {
            Ok(())
        }
        fn FinishOperations(&self, _hr: HRESULT) -> WinResult<()> {
            Ok(())
        }
        fn PreRenameItem(&self, _f: u32, _i: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostRenameItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _n: &PCWSTR,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn PreMoveItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
        ) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostMoveItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn PreCopyItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
        ) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostCopyItem(
            &self,
            _f: u32,
            _i: Ref<IShellItem>,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn PreDeleteItem(&self, flags: u32, _i: Ref<IShellItem>) -> WinResult<()> {
            if !self.permanent && flags & (TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32) == 0 {
                self.refused.set(true);
                return Err(E_ABORT.into());
            }
            Ok(())
        }
        fn PostDeleteItem(
            &self,
            _f: u32,
            item: Ref<IShellItem>,
            hr: HRESULT,
            created: Ref<IShellItem>,
        ) -> WinResult<()> {
            if let Some(src) = item.as_ref().and_then(item_path) {
                let landed = created
                    .as_ref()
                    .and_then(item_path)
                    .map(std::path::PathBuf::from);
                let r = if hr.is_err() { Err(hr) } else { Ok(landed) };
                self.results.borrow_mut().insert(key(&src), r);
            }
            Ok(())
        }
        fn PreNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
            Err(E_ABORT.into())
        }
        fn PostNewItem(
            &self,
            _f: u32,
            _d: Ref<IShellItem>,
            _n: &PCWSTR,
            _t: &PCWSTR,
            _a: u32,
            _hr: HRESULT,
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            Ok(())
        }
        fn UpdateProgress(&self, _total: u32, _done: u32) -> WinResult<()> {
            Ok(())
        }
        fn ResetTimer(&self) -> WinResult<()> {
            Ok(())
        }
        fn PauseTimer(&self) -> WinResult<()> {
            Ok(())
        }
        fn ResumeTimer(&self) -> WinResult<()> {
            Ok(())
        }
    }

    /// Moves several items to the Recycle Bin in one shell operation (much
    /// faster than one operation each). Any item the batch did not settle,
    /// including everything after a refusal, is moved on its own instead, so
    /// the result per item is exactly what `move_to_trash` would give.
    pub(super) fn move_many_to_trash(
        paths: &[&Path],
    ) -> Vec<io::Result<Option<std::path::PathBuf>>> {
        // Files the Recycle Bin cannot keep never reach Windows, which would
        // delete them permanently without asking.
        let too_big: Vec<bool> = paths.iter().map(|p| file_too_big(p)).collect();
        // Keys are worked out before the move, while every item still exists.
        let keys: Vec<String> = paths.iter().map(|p| path_key(p)).collect();
        let fit: Vec<&Path> = paths
            .iter()
            .zip(&too_big)
            .filter(|(_, big)| !**big)
            .map(|(p, _)| *p)
            .collect();
        let (batch, refused) = if fit.is_empty() {
            Default::default()
        } else {
            run_batch(&fit, false)
        };
        paths
            .iter()
            .zip(too_big)
            .zip(keys)
            .map(|((p, big), k)| {
                if big {
                    return Err(super::too_big());
                }
                let gone = std::fs::symlink_metadata(p).is_err();
                let r = match batch.get(&k) {
                    // Settled by the batch, and really gone from its place.
                    Some(Ok(landed)) if gone => Ok(landed.clone()),
                    // A real failure of this item (not the rest of a batch
                    // being cancelled after a refusal).
                    Some(Err(hr)) if !refused && *hr != E_ABORT => Err(hr_to_io(*hr)),
                    // Moved by the batch, but not reported under this name:
                    // find it in the Recycle Bin.
                    _ if gone => Ok(None),
                    // Not settled (refused, cancelled with the rest, or not
                    // reported): do it on its own.
                    _ => return move_to_trash(p),
                };
                confirm_landed(p, r)
            })
            .collect()
    }

    /// Deletes items permanently (no Recycle Bin), in one shell operation.
    pub(super) fn delete_many_permanently(paths: &[&Path]) -> Vec<io::Result<()>> {
        let keys: Vec<String> = paths.iter().map(|p| path_key(p)).collect();
        let (batch, _) = run_batch(paths, true);
        paths
            .iter()
            .zip(keys)
            .map(|(p, k)| {
                let gone = || std::fs::symlink_metadata(p).is_err();
                match batch.get(&k) {
                    Some(Ok(_)) if gone() => Ok(()),
                    Some(Err(hr)) if *hr != E_ABORT => Err(hr_to_io(*hr)),
                    _ if gone() => Ok(()),
                    _ => {
                        // Not settled: try once more on its own.
                        let (one, _) = run_batch(&[p], true);
                        match one.get(&k) {
                            Some(Err(hr)) => Err(hr_to_io(*hr)),
                            _ if gone() => Ok(()),
                            _ => Err(io::Error::other("it is still in place")),
                        }
                    }
                }
            })
            .collect()
    }

    type Settled = std::collections::HashMap<String, Result<Option<std::path::PathBuf>, HRESULT>>;

    /// One shell operation over `paths`: to the Recycle Bin, or permanently.
    fn run_batch(paths: &[&Path], permanent: bool) -> (Settled, bool) {
        (|| -> Option<(Settled, bool)> {
            // SAFETY: COM is initialised for this thread and released below;
            // all pointers passed are valid for the duration of the calls.
            unsafe {
                let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let owns_com = init.is_ok();
                if init.is_err() && init != RPC_E_CHANGED_MODE {
                    return None;
                }
                let out = (|| -> WinResult<_> {
                    let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
                    op.SetOperationFlags(if permanent {
                        FOF_NO_UI
                    } else {
                        FOF_NO_UI | FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE
                    })?;
                    let guard = windows_core::ComObject::new(BatchGuard {
                        permanent,
                        refused: Cell::new(false),
                        results: Default::default(),
                    });
                    let sink: IFileOperationProgressSink = guard.to_interface();
                    let cookie = op.Advise(&sink)?;
                    for p in paths {
                        let w = wide(p);
                        if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                            PCWSTR(w.as_ptr()),
                            None,
                        ) {
                            let _ = op.DeleteItem(&item, None);
                        }
                    }
                    let _ = op.PerformOperations();
                    let _ = op.Unadvise(cookie);
                    Ok((guard.results.take(), guard.refused.get()))
                })()
                .ok();
                if owns_com {
                    CoUninitialize();
                }
                out
            }
        })()
        .unwrap_or_default()
    }

    /// A file the Recycle Bin of its drive cannot keep (folders are checked
    /// by `run` with the sizes from the scan).
    fn file_too_big(p: &Path) -> bool {
        match std::fs::symlink_metadata(p) {
            Ok(m) if m.is_file() => !super::fits_in_trash(p, m.len()),
            Ok(_) => matches!(super::recycle_limit(p), Some(0)),
            Err(_) => false,
        }
    }

    /// After a move that reported no place in the Recycle Bin: find where it
    /// is there, or `None` when it is not there at all (Windows deleted it).
    fn confirm_landed(
        p: &Path,
        r: io::Result<Option<std::path::PathBuf>>,
    ) -> io::Result<Option<std::path::PathBuf>> {
        match r {
            Ok(None) if std::fs::symlink_metadata(p).is_err() => Ok(find_in_bin(p)),
            other => other,
        }
    }

    /// The newest item in the Recycle Bin of `original`'s drive that came
    /// from `original` (read from the bin's $I index files).
    pub(super) fn find_in_bin(original: &Path) -> Option<std::path::PathBuf> {
        use windows::Win32::Storage::FileSystem::GetVolumePathNameW;
        let w = wide(original);
        let mut root = [0u16; 512];
        // SAFETY: `w` is NUL-terminated and `root` is a valid buffer.
        unsafe { GetVolumePathNameW(PCWSTR(w.as_ptr()), &mut root) }.ok()?;
        let n = root.iter().position(|&c| c == 0)?;
        let bin =
            std::path::PathBuf::from(String::from_utf16_lossy(&root[..n])).join("$Recycle.Bin");
        // The bin records full names; compare like with like.
        let want = path_key(original);
        let mut best: Option<(i64, std::path::PathBuf)> = None;
        for user in std::fs::read_dir(bin).ok()?.flatten() {
            let Ok(entries) = std::fs::read_dir(user.path()) else {
                continue;
            };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Some(rest) = name.strip_prefix("$I") else {
                    continue;
                };
                let Ok(b) = std::fs::read(e.path()) else {
                    continue;
                };
                if b.len() < 28 {
                    continue;
                }
                let at = i64::from_le_bytes(b[16..24].try_into().ok()?);
                // Version 2 (Windows 10 and later): length, then the path.
                // Version 1: a fixed 260-character path.
                let raw = if b[0] == 2 {
                    let len = u32::from_le_bytes(b[24..28].try_into().ok()?) as usize;
                    b.get(28..28 + len * 2).unwrap_or(&[])
                } else {
                    b.get(24..24 + 520).unwrap_or(&[])
                };
                let units: Vec<u16> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                let from = String::from_utf16_lossy(&units);
                if key(from.trim_end_matches('\0')) != want {
                    continue;
                }
                let item = user.path().join(format!("$R{rest}"));
                if std::fs::symlink_metadata(&item).is_ok()
                    && best.as_ref().is_none_or(|(t, _)| at > *t)
                {
                    best = Some((at, item));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// See `super::recycle_limit`.
    pub(super) fn recycle_limit(path: &Path) -> Option<u64> {
        use windows::Win32::Storage::FileSystem::{
            GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeNameForVolumeMountPointW,
            GetVolumePathNameW,
        };
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        const DRIVE_FIXED: u32 = 3;
        let w = wide(path);
        let mut root = [0u16; 512];
        // SAFETY: `w` is NUL-terminated; `root` is a valid, zeroed buffer
        // that stays alive for every call below that reads it.
        unsafe { GetVolumePathNameW(PCWSTR(w.as_ptr()), &mut root) }.ok()?;
        let root_p = PCWSTR(root.as_ptr());
        // Only fixed drives keep a Recycle Bin: USB sticks, memory cards,
        // network and optical drives delete at once.
        if unsafe { GetDriveTypeW(root_p) } != DRIVE_FIXED {
            return Some(0);
        }
        let policy = |name: &str| -> Option<u32> {
            [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
                .into_iter()
                .find_map(|hive| {
                    RegKey::predef(hive)
                        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer")
                        .ok()?
                        .get_value::<u32, _>(name)
                        .ok()
                })
        };
        if policy("NoRecycleFiles") == Some(1) {
            return Some(0);
        }
        let mut total = 0u64;
        unsafe { GetDiskFreeSpaceExW(root_p, None, Some(&mut total as *mut u64), None) }.ok()?;
        if let Some(percent) = policy("RecycleBinSize") {
            return Some(total / 100 * u64::from(percent.min(100)));
        }
        let mut volume = [0u16; 64];
        if unsafe { GetVolumeNameForVolumeMountPointW(root_p, &mut volume) }.is_ok() {
            let v = String::from_utf16_lossy(&volume);
            if let (Some(a), Some(b)) = (v.find('{'), v.find('}'))
                && let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!(
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{}",
                    &v[a..=b]
                ))
            {
                if k.get_value::<u32, _>("NukeOnDelete").ok() == Some(1) {
                    return Some(0);
                }
                if let Ok(mb) = k.get_value::<u32, _>("MaxCapacity") {
                    return Some(u64::from(mb) << 20);
                }
            }
        }
        // Windows' default size: 10% of the first 40 GB plus 5% of the rest.
        let first = total.min(40 << 30);
        Some(first / 10 + (total - first) / 20)
    }

    /// Moves `path` to the Recycle Bin, but never something it cannot keep
    /// (Windows would delete that permanently), and reports where it went.
    pub(super) fn move_to_trash(path: &Path) -> io::Result<Option<std::path::PathBuf>> {
        if file_too_big(path) {
            return Err(super::too_big());
        }
        confirm_landed(path, move_one(path))
    }

    fn move_one(path: &Path) -> io::Result<Option<std::path::PathBuf>> {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: COM is initialised for this thread and released below;
        // all pointers passed are valid for the duration of the calls.
        unsafe {
            let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let owns_com = init.is_ok();
            if init.is_err() && init != RPC_E_CHANGED_MODE {
                return Err(io::Error::other("the Windows shell could not be started"));
            }
            let result = (|| -> io::Result<Option<std::path::PathBuf>> {
                let op: IFileOperation =
                    CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(to_io)?;
                op.SetOperationFlags(
                    FOF_NO_UI | FOF_ALLOWUNDO | FOFX_EARLYFAILURE | FOFX_RECYCLEONDELETE,
                )
                .map_err(to_io)?;
                let item: IShellItem =
                    SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).map_err(to_io)?;
                let guard = windows_core::ComObject::new(RecycleGuard {
                    refused: Cell::new(false),
                    failed: Cell::new(None),
                    landed: std::cell::RefCell::new(None),
                });
                let sink: IFileOperationProgressSink = guard.to_interface();
                op.DeleteItem(&item, &sink).map_err(to_io)?;
                let performed = op.PerformOperations();
                if guard.refused.get() {
                    // StorageFull marks "the Recycle Bin can't take it" for the interface.
                    return Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        "the Recycle Bin cannot hold it (too large, or the drive has no Recycle Bin), so it was left in place",
                    ));
                }
                if let Some(hr) = guard.failed.get() {
                    return Err(hr_to_io(hr));
                }
                performed.map_err(to_io)?;
                if op.GetAnyOperationsAborted().map_err(to_io)?.as_bool() {
                    return Err(io::Error::other("the move was cancelled"));
                }
                Ok(guard.landed.borrow_mut().take())
            })();
            if owns_com {
                CoUninitialize();
            }
            result
        }
    }

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub(super) fn put_back(recycled: &Path, original: &Path) -> io::Result<()> {
        use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
        if std::fs::symlink_metadata(recycled).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "it is no longer in the Recycle Bin",
            ));
        }
        if std::fs::symlink_metadata(original).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "something with that name is already there",
            ));
        }
        if let Some(parent) = original.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // No MOVEFILE_REPLACE_EXISTING: the move fails rather than overwrite.
        // The Recycle Bin is on the same drive, so this is a rename.
        let (from, to) = (wide(recycled), wide(original));
        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVE_FILE_FLAGS(0),
            )
        }
        .map_err(|e| io::Error::other(e.message()))?;
        // Remove the Recycle Bin's index entry ($I...) for this item so it
        // no longer appears there. It holds only the old name and date.
        if let (Some(dir), Some(name)) = (
            recycled.parent(),
            recycled.file_name().and_then(|n| n.to_str()),
        ) && let Some(rest) = name.strip_prefix("$R")
        {
            let _ = std::fs::remove_file(dir.join(format!("$I{rest}")));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    /// Foundation's error codes, as kinds the interface can explain.
    fn ns_err(e: &objc2_foundation::NSError) -> io::Error {
        let text = e.localizedDescription().to_string();
        let kind = match e.code() {
            4 => io::ErrorKind::NotFound,                 // NSFileNoSuchFileError
            257 | 513 => io::ErrorKind::PermissionDenied, // no permission to read / write
            642 => io::ErrorKind::ReadOnlyFilesystem,     // read-only volume
            3328 => io::ErrorKind::StorageFull,           // feature unsupported: no Trash here
            _ => io::ErrorKind::Other,
        };
        io::Error::new(kind, text)
    }

    /// The Trash has no size limit, and trashItemAtURL fails (never deletes)
    /// on a volume without one.
    pub(super) fn recycle_limit(_path: &Path) -> Option<u64> {
        None
    }

    pub(super) fn move_to_trash(path: &Path) -> io::Result<Option<PathBuf>> {
        // NSFileManager trashItemAtURL: moves to the Trash or fails; it never
        // deletes permanently, and it reports where the item landed.
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let mut landed = None;
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut landed))
            .map_err(|e| ns_err(&e))?;
        Ok(landed
            .and_then(|u| u.path())
            .map(|s| PathBuf::from(s.to_string())))
    }

    pub(super) fn delete_many_permanently(paths: &[&Path]) -> Vec<io::Result<()>> {
        paths
            .iter()
            .map(|p| {
                if std::fs::symlink_metadata(p).is_err() {
                    return Ok(());
                }
                let url = NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()));
                NSFileManager::defaultManager()
                    .removeItemAtURL_error(&url)
                    .map_err(|e| ns_err(&e))
            })
            .collect()
    }

    pub(super) fn put_back(recycled: &Path, original: &Path) -> io::Result<()> {
        if std::fs::symlink_metadata(recycled).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "it is no longer in the Trash",
            ));
        }
        if std::fs::symlink_metadata(original).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "something with that name is already there",
            ));
        }
        if let Some(parent) = original.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let from = CString::new(recycled.as_os_str().as_bytes())?;
        let to = CString::new(original.as_os_str().as_bytes())?;
        // RENAME_EXCL: fail rather than replace anything at the destination.
        // SAFETY: both are valid NUL-terminated C strings.
        if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } != 0 {
            let e = io::Error::last_os_error();
            return Err(if e.raw_os_error() == Some(libc::EEXIST) {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "something with that name is already there",
                )
            } else {
                e
            });
        }
        Ok(())
    }
}
#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use std::io;
    use std::path::Path;

    pub(super) fn put_back(_recycled: &Path, _original: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "not supported on this system",
        ))
    }

    pub(super) fn recycle_limit(_path: &Path) -> Option<u64> {
        None
    }

    pub(super) fn move_to_trash(_path: &Path) -> io::Result<Option<std::path::PathBuf>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "moving to the Trash is not supported on this system",
        ))
    }

    pub(super) fn delete_many_permanently(paths: &[&Path]) -> Vec<io::Result<()>> {
        paths
            .iter()
            .map(|_| {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "not supported on this system",
                ))
            })
            .collect()
    }
}
