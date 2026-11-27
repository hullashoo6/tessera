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
        let (batch, refused) = run_batch(paths, false);
        paths
            .iter()
            .map(|p| {
                let k = key(&p.to_string_lossy());
                match batch.get(&k) {
                    // Settled by the batch, and really gone from its place.
                    Some(Ok(landed)) if std::fs::symlink_metadata(p).is_err() => Ok(landed.clone()),
                    // A real failure of this item (not the rest of a batch
                    // being cancelled after a refusal).
                    Some(Err(hr)) if !refused && *hr != E_ABORT => Err(hr_to_io(*hr)),
                    // Not settled (refused, cancelled with the rest, or not
                    // reported): do it on its own.
                    _ => move_to_trash(p),
                }
            })
            .collect()
    }

    /// Deletes items permanently (no Recycle Bin), in one shell operation.
    pub(super) fn delete_many_permanently(paths: &[&Path]) -> Vec<io::Result<()>> {
        let (batch, _) = run_batch(paths, true);
        paths
            .iter()
            .map(|p| {
                let k = key(&p.to_string_lossy());
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

    pub(super) fn move_to_trash(path: &Path) -> io::Result<Option<std::path::PathBuf>> {
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
