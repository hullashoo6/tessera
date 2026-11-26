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
        io::Error::other(e.message())
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
                    return Err(io::Error::other(
                        "the Recycle Bin cannot hold it (too large, or the drive has no Recycle Bin), so it was left in place",
                    ));
                }
                if let Some(hr) = guard.failed.get() {
                    return Err(io::Error::other(windows::core::Error::from(hr).message()));
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

    pub(super) fn move_to_trash(path: &Path) -> io::Result<Option<PathBuf>> {
        // NSFileManager trashItemAtURL: moves to the Trash or fails; it never
        // deletes permanently, and it reports where the item landed.
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let mut landed = None;
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut landed))
            .map_err(|e| io::Error::other(e.localizedDescription().to_string()))?;
        Ok(landed
            .and_then(|u| u.path())
            .map(|s| PathBuf::from(s.to_string())))
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
}
