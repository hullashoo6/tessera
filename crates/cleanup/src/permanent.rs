//! Permanent deletion, used only when the person chose "Delete permanently".
//!
//! The same code on Windows and macOS: the standard library's removal,
//! which never follows links or junctions (a link is removed, never what it
//! points to). It is far faster than the Windows shell for large folders,
//! which first lists everything inside (28 Nov 2026: 17 folders showed
//! "Deleting permanently, 0 of 17" for minutes). A folder is emptied one
//! entry at a time, so Stop takes effect part way through a large folder.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shown when Stop was pressed before an item was started.
pub(crate) const NOT_STARTED: &str = "Left in place: you pressed Stop before it was started.";
/// Shown when Stop was pressed while a folder was being emptied.
pub(crate) const STOPPED_PART_WAY: &str =
    "Stopped part way: some of what this folder contained was deleted, the rest is still there.";

fn interrupted(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, text)
}

/// Deletes `path` permanently. Already gone counts as done. With `stop`,
/// a folder is left part-emptied when Stop is pressed (the error says so).
pub fn delete_permanently(path: &Path, stop: Option<&AtomicBool>) -> io::Result<()> {
    let stopped = || stop.is_some_and(|s| s.load(Ordering::Relaxed));
    let md = match fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if stopped() {
        return Err(interrupted(NOT_STARTED));
    }
    let ft = md.file_type();
    if !ft.is_dir() || ft.is_symlink() {
        return remove_entry(path, ft.is_dir());
    }
    let mut deleted_any = false;
    for entry in fs::read_dir(path)? {
        if stopped() {
            return Err(interrupted(if deleted_any {
                STOPPED_PART_WAY
            } else {
                NOT_STARTED
            }));
        }
        let entry = entry?;
        let p = entry.path();
        let ft = entry.file_type()?;
        let r = if ft.is_dir() && !ft.is_symlink() {
            fs::remove_dir_all(&p)
        } else {
            remove_entry(&p, ft.is_dir())
        };
        match r {
            Ok(()) => deleted_any = true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    match fs::remove_dir(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

/// Removes a file or a link (a link to a folder is removed as a folder
/// entry on Windows, never followed).
fn remove_entry(p: &Path, is_dir: bool) -> io::Result<()> {
    let first = if is_dir {
        fs::remove_dir(p)
    } else {
        fs::remove_file(p)
    };
    let r = match first {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        // A link whose kind was misjudged: the other call removes it.
        Err(e) => match if is_dir {
            fs::remove_file(p)
        } else {
            fs::remove_dir(p)
        } {
            Ok(()) => return Ok(()),
            Err(_) => e,
        },
    };
    // Windows refuses to delete a read-only file; the person chose to
    // delete it, so the read-only mark is cleared first (Windows only: on
    // macOS a read-only file in a writable folder deletes normally).
    #[cfg(windows)]
    if r.kind() == io::ErrorKind::PermissionDenied
        && let Ok(md) = fs::symlink_metadata(p)
        && md.permissions().readonly()
    {
        let mut perms = md.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        if fs::set_permissions(p, perms).is_ok() {
            return fs::remove_file(p);
        }
    }
    Err(r)
}
