//! Why an item was left in place, in categories the interface can explain
//! in plain words with a suggested fix.

use crate::inuse;
use std::io;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    /// Open in another app (`Failure::apps` names it when Windows can tell).
    InUse,
    /// The system did not allow it: another owner, administrator-only, or a
    /// privacy-protected place.
    Permission,
    /// The Recycle Bin / Trash cannot hold it (too large, or the drive has
    /// none). Tessera refuses rather than delete it permanently unasked.
    TooLargeForBin,
    /// It changed (or something on its path did) after it was chosen.
    Changed,
    /// Tessera's protection rules keep it.
    Protected,
    /// Its path is too long for the Recycle Bin.
    PathTooLong,
    /// A cloud file whose sync app (for example OneDrive) is not running.
    CloudOffline,
    /// Locked in Finder (Get Info > Locked) or on a read-only drive.
    Locked,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailKind,
    /// The system's own wording, kept for support; not the main message.
    pub detail: String,
    /// Apps holding the item (or files inside it) open, when known.
    pub apps: Vec<String>,
    /// For a folder: files inside it found in use (a few, for showing).
    pub in_use_files: Vec<std::path::PathBuf>,
}

impl Failure {
    pub fn new(kind: FailKind, detail: impl Into<String>) -> Self {
        Failure {
            kind,
            detail: detail.into(),
            apps: Vec::new(),
            in_use_files: Vec::new(),
        }
    }
}

#[cfg(windows)]
mod code {
    pub const ACCESS_DENIED: i32 = 5;
    pub const SHARING_VIOLATION: i32 = 32;
    pub const LOCK_VIOLATION: i32 = 33;
    pub const WRITE_PROTECT: i32 = 19;
    pub const FILENAME_TOO_LONG: i32 = 206;
    pub const BUFFER_OVERFLOW: i32 = 111;
    pub const CLOUD_NOT_RUNNING: i32 = 362;
    pub const DIR_NOT_EMPTY: i32 = 145;
}

/// Turns the error from a move or delete of `path` into an explanation. For
/// errors that can mean "something is open" it looks for what holds the item
/// (inside a folder, for at most a second).
pub fn classify(err: &io::Error, path: &Path) -> Failure {
    let detail = err.to_string();
    if err.kind() == io::ErrorKind::StorageFull {
        return Failure::new(FailKind::TooLargeForBin, detail);
    }
    #[cfg(windows)]
    {
        match err.raw_os_error() {
            Some(code::SHARING_VIOLATION | code::LOCK_VIOLATION) => {
                return in_use(path, detail);
            }
            // A folder cannot be moved while a file in it is open, and Windows
            // often says "access denied" for that. Look before calling it a
            // permission problem.
            Some(code::ACCESS_DENIED | code::DIR_NOT_EMPTY) => {
                let f = in_use(path, detail.clone());
                if !f.apps.is_empty() || !f.in_use_files.is_empty() {
                    return f;
                }
                return Failure::new(FailKind::Permission, detail);
            }
            Some(code::WRITE_PROTECT) => return Failure::new(FailKind::Locked, detail),
            Some(code::FILENAME_TOO_LONG | code::BUFFER_OVERFLOW) => {
                return Failure::new(FailKind::PathTooLong, detail);
            }
            Some(code::CLOUD_NOT_RUNNING) => return Failure::new(FailKind::CloudOffline, detail),
            _ => {}
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        const UF_IMMUTABLE: u32 = 0x2;
        const SF_IMMUTABLE: u32 = 0x2_0000;
        if let Ok(md) = std::fs::symlink_metadata(path)
            && md.st_flags() & (UF_IMMUTABLE | SF_IMMUTABLE) != 0
        {
            return Failure::new(FailKind::Locked, detail);
        }
    }
    if err.kind() == io::ErrorKind::PermissionDenied {
        return Failure::new(FailKind::Permission, detail);
    }
    if err.kind() == io::ErrorKind::ReadOnlyFilesystem {
        return Failure::new(FailKind::Locked, detail);
    }
    Failure::new(FailKind::Other, detail)
}

/// An in-use failure, with the apps and (for folders) the files involved.
/// Windows only: macOS lets open files be moved.
#[cfg_attr(not(windows), allow(dead_code))]
fn in_use(path: &Path, detail: String) -> Failure {
    let mut f = Failure::new(FailKind::InUse, detail);
    let dir = path.is_dir();
    let files = if dir {
        inuse::locked_files_in(path, 20, 50_000, Duration::from_secs(1)).files
    } else if inuse::is_locked(path) {
        vec![path.to_path_buf()]
    } else {
        Vec::new()
    };
    let asked: Vec<std::path::PathBuf> = if files.is_empty() {
        vec![path.to_path_buf()]
    } else {
        files.clone()
    };
    f.apps = inuse::apps_using(&asked).into_iter().take(5).collect();
    // Only a folder has files "inside" it worth listing.
    if dir {
        f.in_use_files = files.into_iter().take(5).collect();
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_refusal_is_recognised() {
        let e = io::Error::new(io::ErrorKind::StorageFull, "too large");
        assert_eq!(classify(&e, Path::new("x")).kind, FailKind::TooLargeForBin);
    }

    #[test]
    fn permission_and_other_errors() {
        let tmp = std::env::temp_dir().join("tessera-classify-missing");
        let e = io::Error::new(io::ErrorKind::PermissionDenied, "no");
        assert_eq!(classify(&e, &tmp).kind, FailKind::Permission);
        let e = io::Error::other("strange");
        assert_eq!(classify(&e, &tmp).kind, FailKind::Other);
    }

    #[cfg(windows)]
    #[test]
    fn windows_codes_map_to_plain_causes() {
        let tmp = std::env::temp_dir().join("tessera-classify-missing");
        let k = |c| classify(&io::Error::from_raw_os_error(c), &tmp).kind;
        assert_eq!(k(code::SHARING_VIOLATION), FailKind::InUse);
        assert_eq!(
            k(code::ACCESS_DENIED),
            FailKind::Permission,
            "nothing holds a missing path open"
        );
        assert_eq!(k(code::FILENAME_TOO_LONG), FailKind::PathTooLong);
        assert_eq!(k(code::CLOUD_NOT_RUNNING), FailKind::CloudOffline);
    }
}
