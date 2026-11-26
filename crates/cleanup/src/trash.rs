//! Moving items to the Recycle Bin / Trash. Never deletes permanently.

use std::io;
use std::path::Path;

/// Moves `path` to the Recycle Bin (Windows) or Trash (macOS).
///
/// On Windows, items the Recycle Bin cannot hold (for example files larger
/// than its capacity, or on drives without one) are refused: the operation
/// is aborted for that item instead of Windows deleting it permanently.
pub fn move_to_trash(path: &Path) -> io::Result<()> {
    imp::move_to_trash(path)
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
        SHCreateItemFromParsingName, TSF_DELETE_RECYCLE_IF_POSSIBLE,
    };
    use windows::core::{HRESULT, PCWSTR, Ref, Result as WinResult, implement};

    /// Progress sink that vetoes any delete Windows would not recycle.
    #[implement(IFileOperationProgressSink)]
    struct RecycleGuard {
        refused: Cell<bool>,
        failed: Cell<Option<HRESULT>>,
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
            _c: Ref<IShellItem>,
        ) -> WinResult<()> {
            if hr.is_err() {
                self.failed.set(Some(hr));
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

    pub(super) fn move_to_trash(path: &Path) -> io::Result<()> {
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
            let result = (|| -> io::Result<()> {
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
                Ok(())
            })();
            if owns_com {
                CoUninitialize();
            }
            result
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::io;
    use std::path::Path;
    use trash::macos::{DeleteMethod, TrashContextExtMacos};

    pub(super) fn move_to_trash(path: &Path) -> io::Result<()> {
        // NSFileManager trashItemAtURL: moves to the Trash or fails; it
        // never deletes permanently and does not script the Finder.
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(path)
            .map_err(|e| io::Error::other(e.to_string()))
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use std::io;
    use std::path::Path;

    pub(super) fn move_to_trash(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "moving to the Trash is not supported on this system",
        ))
    }
}
