//! Locations Tessera will never move to the Trash.
//!
//! Two kinds of rule:
//! * **Protected trees**: nothing at or inside them may be removed
//!   (the OS folder, program folders, boot and recovery files).
//! * **Protected containers**: the folder itself may not be removed, but
//!   items inside it may (the Users folder, your home folder, Documents).
//!
//! In both cases an *ancestor* of a protected location is also refused, so
//! selecting `C:\` or `C:\Users` can never take system folders with it.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Protection {
    trees: Vec<PathBuf>,
    containers: Vec<PathBuf>,
}

/// Lower-cased, separator-normalised form used for comparisons. Windows and
/// macOS file systems are case-insensitive by default.
fn key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s).to_string();
    let s = s.trim_end_matches('/').to_lowercase();
    if s.is_empty() { "/".into() } else { s }
}

fn is_within(path: &str, base: &str) -> bool {
    path == base
        || (path.starts_with(base) && path.as_bytes().get(base.len()) == Some(&b'/'))
        || base == "/"
}

impl Protection {
    pub fn new(trees: Vec<PathBuf>, containers: Vec<PathBuf>) -> Self {
        Protection { trees, containers }
    }

    /// Rules for the computer this runs on.
    pub fn for_this_system() -> Self {
        let mut trees = Vec::new();
        let mut containers = Vec::new();
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        if cfg!(windows) {
            let drive = env("SystemDrive")
                .map(|d| PathBuf::from(format!("{}\\", d.to_string_lossy())))
                .unwrap_or_else(|| "C:\\".into());
            for k in [
                "SystemRoot",
                "windir",
                "ProgramFiles",
                "ProgramFiles(x86)",
                "ProgramW6432",
                "ProgramData",
                "CommonProgramFiles",
            ] {
                trees.extend(env(k));
            }
            for name in [
                "System Volume Information",
                "$Recycle.Bin",
                "Recovery",
                "Boot",
                "EFI",
                "$WinREAgent",
                "$SysReset",
                "Config.Msi",
                "pagefile.sys",
                "hiberfil.sys",
                "swapfile.sys",
                "bootmgr",
                "BOOTNXT",
                "DumpStack.log.tmp",
            ] {
                trees.push(drive.join(name));
            }
            containers.push(drive.join("Users"));
            if let Some(home) = env("USERPROFILE") {
                for sub in [
                    "",
                    "AppData",
                    "AppData\\Local",
                    "AppData\\Roaming",
                    "AppData\\LocalLow",
                    "Desktop",
                    "Documents",
                    "Downloads",
                    "Pictures",
                    "Music",
                    "Videos",
                    "OneDrive",
                ] {
                    containers.push(if sub.is_empty() {
                        home.clone()
                    } else {
                        home.join(sub)
                    });
                }
            }
        } else {
            for p in [
                "/System", "/Library", "/usr", "/bin", "/sbin", "/private", "/etc", "/var", "/dev",
                "/cores", "/opt",
            ] {
                trees.push(p.into());
            }
            containers.extend(["/Applications", "/Users", "/Volumes"].map(PathBuf::from));
            if let Some(home) = env("HOME") {
                trees.push(home.join("Library/Keychains"));
                trees.push(home.join("Library/Mobile Documents"));
                for sub in [
                    "",
                    "Library",
                    "Desktop",
                    "Documents",
                    "Downloads",
                    "Pictures",
                    "Movies",
                    "Music",
                ] {
                    containers.push(if sub.is_empty() {
                        home.clone()
                    } else {
                        home.join(sub)
                    });
                }
            }
        }
        // The running app itself.
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
        {
            trees.push(dir);
        }
        Protection { trees, containers }
    }

    /// `Err(reason)` if `path` must not be removed.
    pub fn check(&self, path: &Path) -> Result<(), String> {
        if path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err("The path is not in a form Tessera can verify.".into());
        }
        if path.parent().is_none() || path.components().count() <= 1 {
            return Err("A whole drive cannot be removed.".into());
        }
        let p = key(path);
        if p.len() <= 3 && (p.ends_with(':') || p.ends_with(":/") || p == "/") {
            return Err("A whole drive cannot be removed.".into());
        }
        for t in &self.trees {
            let t = key(t);
            if is_within(&p, &t) {
                return Err("This is part of the operating system or installed programs.".into());
            }
            if is_within(&t, &p) {
                return Err("This folder contains operating-system or program files.".into());
            }
        }
        for c in &self.containers {
            if is_within(&key(c), &p) {
                return Err(
                    "This is a main system or personal folder; remove items inside it instead."
                        .into(),
                );
            }
        }
        Ok(())
    }
}
