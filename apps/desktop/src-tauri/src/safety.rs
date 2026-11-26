//! Rules for handing items to other applications.
//!
//! "Open" asks the operating system to open an item with its default app.
//! For programs, scripts, installers and shortcuts that would *run* code, so
//! those are refused; "Show in Explorer/Finder" remains available.

use std::path::Path;
use tessera_scan::NodeKind;

/// Extensions whose default action executes code (compared case-insensitively).
const RUNNABLE: &[&str] = &[
    // Windows
    "exe",
    "com",
    "bat",
    "cmd",
    "ps1",
    "psm1",
    "psd1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "msi",
    "msp",
    "mst",
    "scr",
    "pif",
    "lnk",
    "url",
    "reg",
    "hta",
    "cpl",
    "msc",
    "jar",
    "appref-ms",
    "inf",
    "gadget",
    "application",
    "appx",
    "msix",
    "appxbundle",
    "msixbundle",
    "settingcontent-ms",
    "library-ms",
    // macOS and Unix
    "app",
    "command",
    "tool",
    "sh",
    "bash",
    "zsh",
    "csh",
    "ksh",
    "fish",
    "pkg",
    "mpkg",
    "workflow",
    "scpt",
    "applescript",
    "terminal",
    "action",
    "prefpane",
    "osax",
    "webloc",
    "inetloc",
    "fileloc",
    // Cross-platform interpreters
    "py",
    "pyw",
    "rb",
    "pl",
    "php",
];

pub fn check_openable(path: &Path, kind: NodeKind) -> Result<(), String> {
    match kind {
        NodeKind::Link | NodeKind::Other => {
            return Err(
                "Links and special files are not opened. Use Show in folder instead.".into(),
            );
        }
        NodeKind::Dir | NodeKind::File => {}
    }
    let runnable = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| RUNNABLE.iter().any(|r| r.eq_ignore_ascii_case(e)))
        .unwrap_or(false);
    if runnable {
        return Err(
            "For safety, Tessera does not open programs, scripts or shortcuts. \
                    Use Show in folder instead."
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_and_scripts_are_refused() {
        for p in [
            "setup.EXE",
            "run.bat",
            "x.ps1",
            "Thing.app",
            "do.command",
            "a.lnk",
            "s.sh",
            "t.py",
        ] {
            assert!(check_openable(Path::new(p), NodeKind::File).is_err(), "{p}");
        }
        // A macOS application bundle is a folder: still refused.
        assert!(check_openable(Path::new("/Applications/Safari.app"), NodeKind::Dir).is_err());
    }

    #[test]
    fn documents_and_folders_are_allowed() {
        for p in [
            "report.pdf",
            "photo.JPG",
            "movie.mp4",
            "notes.txt",
            "archive.zip",
            "noextension",
        ] {
            assert!(check_openable(Path::new(p), NodeKind::File).is_ok(), "{p}");
        }
        assert!(check_openable(Path::new("C:\\Users\\me\\Documents"), NodeKind::Dir).is_ok());
    }

    #[test]
    fn links_are_refused() {
        assert!(check_openable(Path::new("shortcut"), NodeKind::Link).is_err());
    }
}
