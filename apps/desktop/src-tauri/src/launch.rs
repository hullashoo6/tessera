//! Scans started from outside the window:
//!
//! * a folder given on the command line: the "Scan with Tessera" entry in
//!   File Explorer's right-click menu (added by the installer), or a shortcut;
//! * the same while Tessera is already open (the second launch hands its
//!   folder to the open window, which scans it);
//! * a folder or drive dropped onto the window;
//! * on macOS, a folder opened with Tessera from Finder's right-click
//!   "Open With" menu, or dropped on Tessera's Dock icon.
//!
//! In every case the person chose the folder themselves, as with the folder
//! picker, and the interface never sends a path. Only an existing folder or
//! drive is accepted; anything else is ignored or politely refused.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tessera_scan::prepare_root;

/// Turns a command-line argument into a folder to scan, if it is one.
///
/// Windows passes a drive as `"C:\"`, and the standard argument rules read
/// the backslash before the closing quote as an escaped quote, giving `C:"`.
/// That is repaired here. Relative paths (from a second launch in a terminal)
/// are resolved against that launch's working folder.
pub fn folder_from_arg(arg: &OsString, cwd: Option<&Path>) -> Option<PathBuf> {
    let text = arg.to_string_lossy();
    let mut path = if cfg!(windows) && text.ends_with('"') {
        PathBuf::from(format!("{}\\", text.trim_end_matches('"')))
    } else {
        PathBuf::from(arg)
    };
    if path.is_relative()
        && let Some(cwd) = cwd
    {
        path = cwd.join(path);
    }
    // Rebuilding from components drops a trailing separator ("D:\Photos\"
    // becomes "D:\Photos") but keeps a drive's root ("C:\").
    let path: PathBuf = path.components().collect();
    path.is_dir().then_some(path)
}

/// The first argument after the program name, as a folder.
pub fn folder_from_args<I: IntoIterator<Item = OsString>>(
    args: I,
    cwd: Option<&Path>,
) -> Option<PathBuf> {
    args.into_iter()
        .nth(1)
        .and_then(|a| folder_from_arg(&a, cwd))
}

/// Starts a scan of `folder` with the person's scan settings and tells the
/// interface, which switches to the storage map. Returns false if it could
/// not start.
pub fn scan_folder(app: &AppHandle, folder: &Path) -> bool {
    let Ok(root) = prepare_root(folder) else {
        refuse(app, "That folder cannot be scanned.");
        return false;
    };
    let state = app.state::<super::AppState>();
    let opts = super::remembered_scan_options(app);
    match super::begin_scan(app.clone(), &state, root, opts) {
        Ok(id) => {
            let _ = app.emit("scan-started", id);
            true
        }
        Err(e) => {
            refuse(app, &e);
            false
        }
    }
}

/// Shows a short message in the window (for example, a file was dropped).
pub fn refuse(app: &AppHandle, message: &str) {
    let _ = app.emit("scan-refused", message.to_string());
}

/// Brings the main window forward.
pub fn focus(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// A drop onto the window: the first folder or drive is scanned.
pub fn dropped(app: &AppHandle, paths: &[PathBuf]) {
    match paths.iter().find(|p| p.is_dir()) {
        Some(folder) => {
            scan_folder(app, folder);
        }
        None => refuse(app, "Drop a folder or a drive to scan it."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_existing_folders_only() {
        let dir = std::env::temp_dir();
        let exe = OsString::from("tessera");
        let found = folder_from_args([exe.clone(), dir.clone().into_os_string()], None);
        assert_eq!(found.as_deref(), Some(dir.as_path()));
        assert_eq!(folder_from_args([exe.clone()], None), None);
        let missing = dir.join("tessera-no-such-folder-7f3a");
        assert_eq!(
            folder_from_args([exe, missing.into_os_string()], None),
            None
        );
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let file = std::env::temp_dir().join("tessera-launch-test-file.txt");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(folder_from_arg(&file.clone().into_os_string(), None), None);
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn relative_paths_use_the_launch_folder() {
        let base = std::env::temp_dir();
        let name = "tessera-launch-test-rel";
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let found = folder_from_arg(&OsString::from(name), Some(&base));
        std::fs::remove_dir(&dir).unwrap();
        assert_eq!(found, Some(dir));
    }

    #[cfg(windows)]
    #[test]
    fn a_trailing_backslash_is_dropped() {
        // `"D:\Photos\"` arrives as `D:\Photos"`.
        let dir = std::env::temp_dir().join("tessera-launch-test-trail");
        std::fs::create_dir_all(&dir).unwrap();
        let mangled = OsString::from(format!("{}\\\"", dir.display()));
        let found = folder_from_arg(&mangled, None);
        std::fs::remove_dir(&dir).unwrap();
        assert_eq!(found, Some(dir));
    }

    #[cfg(windows)]
    #[test]
    fn repairs_the_drive_quote() {
        // What Windows' argument rules make of `"C:\"`.
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let mangled = OsString::from(format!("{drive}\""));
        assert_eq!(
            folder_from_arg(&mangled, None),
            Some(PathBuf::from(format!("{drive}\\")))
        );
    }
}
