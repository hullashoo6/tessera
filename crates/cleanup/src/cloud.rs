//! "Free up space" for cloud files: keep the file in the cloud, drop the
//! local copy. Never deletes: the file stays in the cloud and downloads
//! again when opened.
//!
//! Windows: marks the file unpinned (what Explorer's "Free up space" does);
//! the sync app (OneDrive and other Cloud Files providers) then removes the
//! local data. Only real cloud placeholders are touched.
//! macOS: asks the system to remove the downloaded copy of an iCloud Drive
//! file (`evictUbiquitousItemAtURL`).

use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudState {
    /// Not managed by a cloud provider.
    NotCloud,
    /// Downloaded; space can be freed.
    Downloaded,
    /// Marked "Always keep on this device"; respected, not changed.
    AlwaysKeep,
    /// Already online-only.
    OnlineOnly,
}

/// Folders kept in sync with a cloud service for this user.
pub fn sync_roots() -> Vec<PathBuf> {
    imp::sync_roots()
}

pub fn state(path: &Path) -> CloudState {
    imp::state(path)
}

/// Attributes and reparse tag as seen by this process (diagnostics).
#[cfg(windows)]
#[doc(hidden)]
pub fn find_data(path: &Path) -> Option<(u32, u32)> {
    imp::find_data(path)
}

/// Frees the local copy of one cloud file. Files marked "always keep" are
/// left alone; non-cloud files are refused.
pub fn free_up(path: &Path) -> io::Result<()> {
    match state(path) {
        CloudState::Downloaded => imp::free_up(path),
        CloudState::OnlineOnly => Ok(()),
        CloudState::AlwaysKeep => Err(io::Error::other("it is set to always stay on this device")),
        CloudState::NotCloud => Err(io::Error::other("it is not a cloud file")),
    }
}

#[cfg(windows)]
mod imp {
    use super::CloudState;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAGS_AND_ATTRIBUTES, FindClose, FindFirstFileW, GetFileAttributesW,
        INVALID_FILE_ATTRIBUTES, SetFileAttributesW, WIN32_FIND_DATAW,
    };
    use windows::core::PCWSTR;

    const REPARSE_POINT: u32 = 0x400;
    const OFFLINE: u32 = 0x1000;
    const PINNED: u32 = 0x0008_0000;
    const UNPINNED: u32 = 0x0010_0000;
    const RECALL_ON_OPEN: u32 = 0x0004_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlSetThreadPlaceholderCompatibilityMode(mode: i8) -> i8;
    }
    const PHCM_EXPOSE_PLACEHOLDERS: i8 = 2;

    /// By default Windows shows cloud placeholders to apps as ordinary files.
    /// While this guard lives, this thread sees them as they are.
    struct Expose(i8);
    impl Expose {
        fn new() -> Self {
            // SAFETY: documented ntdll call; affects only this thread.
            Expose(unsafe { RtlSetThreadPlaceholderCompatibilityMode(PHCM_EXPOSE_PLACEHOLDERS) })
        }
    }
    impl Drop for Expose {
        fn drop(&mut self) {
            if self.0 >= 0 {
                // SAFETY: restores the previous mode for this thread.
                unsafe { RtlSetThreadPlaceholderCompatibilityMode(self.0) };
            }
        }
    }

    fn under_sync_root(path: &Path) -> bool {
        static ROOTS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
        ROOTS
            .get_or_init(sync_roots)
            .iter()
            .any(|r| path.starts_with(r))
    }

    /// A cloud placeholder: a reparse point inside a cloud sync folder whose
    /// tag is a Cloud Files tag (0x9000?01A) or hidden by Windows (shown as
    /// 0 to apps, which Windows does for cloud placeholders). Links and
    /// junctions always show their own tags, so they are excluded.
    fn is_cloud_placeholder(path: &Path) -> bool {
        if !under_sync_root(path) {
            return false;
        }
        let w = wide(path);
        let mut data = WIN32_FIND_DATAW::default();
        // SAFETY: valid NUL-terminated path and out-pointer; handle closed.
        unsafe {
            let Ok(h) = FindFirstFileW(PCWSTR(w.as_ptr()), &mut data) else {
                return false;
            };
            let _ = FindClose(h);
        }
        let tag = data.dwReserved0;
        data.dwFileAttributes & REPARSE_POINT != 0 && (tag == 0 || tag & 0xFFFF_0FFF == 0x9000_001A)
    }

    pub fn find_data(path: &Path) -> Option<(u32, u32)> {
        let _expose = Expose::new();
        let w = wide(path);
        let mut data = WIN32_FIND_DATAW::default();
        // SAFETY: valid NUL-terminated path and out-pointer; handle closed.
        unsafe {
            let h = FindFirstFileW(PCWSTR(w.as_ptr()), &mut data).ok()?;
            let _ = FindClose(h);
        }
        Some((data.dwFileAttributes, data.dwReserved0))
    }

    fn attrs(path: &Path) -> Option<u32> {
        let w = wide(path);
        // SAFETY: valid NUL-terminated path.
        let a = unsafe { GetFileAttributesW(PCWSTR(w.as_ptr())) };
        (a != INVALID_FILE_ATTRIBUTES).then_some(a)
    }

    pub fn state(path: &Path) -> CloudState {
        let _expose = Expose::new();
        let Some(a) = attrs(path) else {
            return CloudState::NotCloud;
        };
        if !is_cloud_placeholder(path) {
            return CloudState::NotCloud;
        }
        if a & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0 {
            CloudState::OnlineOnly
        } else if a & PINNED != 0 {
            CloudState::AlwaysKeep
        } else {
            CloudState::Downloaded
        }
    }

    pub fn free_up(path: &Path) -> io::Result<()> {
        let _expose = Expose::new();
        let a = attrs(path).ok_or_else(io::Error::last_os_error)?;
        let w = wide(path);
        // SAFETY: valid NUL-terminated path.
        unsafe {
            SetFileAttributesW(
                PCWSTR(w.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES((a | UNPINNED) & !PINNED),
            )
        }
        .map_err(|e| io::Error::other(e.message()))
    }

    pub fn sync_roots() -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
            .iter()
            .filter_map(|k| std::env::var_os(k).map(PathBuf::from))
            .collect();
        if let Ok(accounts) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\OneDrive\Accounts")
        {
            for name in accounts.enum_keys().flatten() {
                if let Ok(folder) = accounts
                    .open_subkey(&name)
                    .and_then(|k| k.get_value::<String, _>("UserFolder"))
                {
                    roots.push(PathBuf::from(folder));
                }
            }
        }
        roots.retain(|p| p.is_dir());
        roots.sort();
        roots.dedup();
        roots
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::CloudState;
    use std::io;
    use std::path::{Path, PathBuf};

    fn icloud_drive() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Mobile Documents/com~apple~CloudDocs"))
    }

    pub fn sync_roots() -> Vec<PathBuf> {
        icloud_drive().into_iter().filter(|p| p.is_dir()).collect()
    }

    pub fn state(path: &Path) -> CloudState {
        use std::os::unix::fs::MetadataExt;
        let in_icloud = std::env::var_os("HOME")
            .map(|h| path.starts_with(PathBuf::from(h).join("Library/Mobile Documents")))
            .unwrap_or(false);
        match std::fs::symlink_metadata(path) {
            Ok(m) if in_icloud && m.is_file() => {
                if m.blocks() == 0 && m.len() > 0 {
                    CloudState::OnlineOnly
                } else {
                    CloudState::Downloaded
                }
            }
            _ => CloudState::NotCloud,
        }
    }

    pub fn free_up(path: &Path) -> io::Result<()> {
        use objc2_foundation::{NSFileManager, NSString, NSURL};
        let s = NSString::from_str(&path.to_string_lossy());
        let url = NSURL::fileURLWithPath(&s);
        NSFileManager::defaultManager()
            .evictUbiquitousItemAtURL_error(&url)
            .map_err(|e| io::Error::other(e.localizedDescription().to_string()))
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::CloudState;
    use std::io;
    use std::path::{Path, PathBuf};

    pub fn sync_roots() -> Vec<PathBuf> {
        Vec::new()
    }
    pub fn state(_path: &Path) -> CloudState {
        CloudState::NotCloud
    }
    pub fn free_up(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "not supported on this system",
        ))
    }
}
