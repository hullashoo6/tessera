//! Finding out what is holding a file open, so a cleanup that fails because
//! a file is in use can say which app to close (as File Explorer's "File in
//! use" dialog does).
//!
//! Windows: a file opened by another program without delete sharing cannot
//! be moved, and neither can a folder containing one. `is_locked` tests a
//! single file by opening it the way a move would; `locked_files_in` looks
//! inside a folder within a time limit; `apps_using` asks the Restart
//! Manager which applications hold given files. macOS lets open files be
//! moved, so these find nothing there.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Files found locked inside a folder, and whether the search was complete.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LockedInside {
    pub files: Vec<PathBuf>,
    /// False when the folder was too big to check within the time limit.
    pub complete: bool,
}

/// True when another program holds `file` open so that it cannot be moved.
pub fn is_locked(file: &Path) -> bool {
    imp::is_locked(file)
}

/// Locked files below `dir` (at most `max_found`), searching for at most
/// `budget` and `max_entries` entries.
pub fn locked_files_in(
    dir: &Path,
    max_found: usize,
    max_entries: usize,
    budget: Duration,
) -> LockedInside {
    let start = Instant::now();
    let mut out = LockedInside {
        files: Vec::new(),
        complete: true,
    };
    let mut stack = vec![dir.to_path_buf()];
    let mut seen = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            seen += 1;
            if seen > max_entries || start.elapsed() > budget {
                out.complete = false;
                return out;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(e.path());
            } else if is_locked(&e.path()) {
                out.files.push(e.path());
                if out.files.len() >= max_found {
                    out.complete = false;
                    return out;
                }
            }
        }
    }
    out
}

/// Names of the applications holding any of `files` open (Windows Restart
/// Manager), without duplicates. Empty when unknown.
pub fn apps_using(files: &[PathBuf]) -> Vec<String> {
    imp::apps_using(files)
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Path, PathBuf};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };
    use windows::core::{PCWSTR, PWSTR};

    const DELETE: u32 = 0x0001_0000;
    const SHARE_ALL: u32 = 0x7;
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;

    pub(super) fn is_locked(file: &Path) -> bool {
        // Opening for delete, while sharing everything ourselves, fails with
        // a sharing violation exactly when someone else's open blocks moves.
        match std::fs::OpenOptions::new()
            .access_mode(DELETE)
            .share_mode(SHARE_ALL)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(file)
        {
            Ok(_) => false,
            Err(e) => matches!(
                e.raw_os_error(),
                Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
            ),
        }
    }

    pub(super) fn apps_using(files: &[PathBuf]) -> Vec<String> {
        if files.is_empty() {
            return Vec::new();
        }
        let wide: Vec<Vec<u16>> = files
            .iter()
            .take(64)
            .map(|p| {
                p.as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
            })
            .collect();
        let names: Vec<PCWSTR> = wide.iter().map(|w| PCWSTR(w.as_ptr())).collect();
        let mut session = 0u32;
        let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
        // SAFETY: every buffer passed lives until after RmEndSession.
        unsafe {
            if RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) != ERROR_SUCCESS {
                return Vec::new();
            }
            let mut apps = Vec::new();
            if RmRegisterResources(session, Some(&names), None, None) == ERROR_SUCCESS {
                let mut needed = 0u32;
                let mut count = 0u32;
                let mut reasons = 0u32;
                let mut list: Vec<RM_PROCESS_INFO> = Vec::new();
                let mut rc = RmGetList(session, &mut needed, &mut count, None, &mut reasons);
                if rc == ERROR_MORE_DATA {
                    list.resize(needed as usize + 4, RM_PROCESS_INFO::default());
                    count = list.len() as u32;
                    rc = RmGetList(
                        session,
                        &mut needed,
                        &mut count,
                        Some(list.as_mut_ptr()),
                        &mut reasons,
                    );
                }
                if rc == ERROR_SUCCESS {
                    for info in list.iter().take(count as usize) {
                        let end = info
                            .strAppName
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(info.strAppName.len());
                        let name = String::from_utf16_lossy(&info.strAppName[..end]);
                        if !name.is_empty() && !apps.contains(&name) {
                            apps.push(name);
                        }
                    }
                }
            }
            let _ = RmEndSession(session);
            apps
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::{Path, PathBuf};

    pub(super) fn is_locked(_file: &Path) -> bool {
        false
    }

    pub(super) fn apps_using(_files: &[PathBuf]) -> Vec<String> {
        Vec::new()
    }
}
