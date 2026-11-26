//! Installed apps and their own uninstallers.
//!
//! macOS: third-party apps in /Applications and ~/Applications; uninstalling
//! moves the app to the Trash (it can be put back) and finds its leftovers
//! in ~/Library by bundle id.
//!
//! Windows: read from the same registry entries Windows Settings uses
//! (per-user and machine-wide, 64- and 32-bit views). Hidden system
//! components and updates are left out. Uninstalling opens the app's own
//! uninstaller; Tessera never deletes app files itself. Other systems: not
//! supported yet.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    /// Stable id: hive, view and registry key name.
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    /// As recorded by the installer, `YYYYMMDD`.
    pub install_date: Option<String>,
    /// Size the app reports about itself (often missing or out of date).
    pub reported_bytes: Option<u64>,
    /// Size measured by Tessera from the app's own folder, when known.
    pub measured_bytes: Option<u64>,
    pub install_location: Option<PathBuf>,
    /// True when installed for this user only.
    pub per_user: bool,
    pub can_uninstall: bool,
}

pub fn supported() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

/// Splits an uninstall command into program and arguments.
pub fn split_command(cmd: &str) -> Option<(String, String)> {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some((rest[..end].to_string(), rest[end + 1..].trim().to_string()));
    }
    let i = c.to_ascii_lowercase().find(".exe")? + 4;
    Some((c[..i].to_string(), c[i..].trim().to_string()))
}

/// What uninstalling did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The app's own uninstaller was opened; it finishes the job.
    OpenedUninstaller,
    /// A Microsoft Store app was removed by Windows.
    Removed,
    /// A Mac app bundle was moved to the Trash (it can be put back).
    MovedToTrash {
        original: PathBuf,
        recycled: Option<PathBuf>,
        /// Size of the app bundle on disk.
        bytes: u64,
        /// The app's own support files in ~/Library, found by bundle id.
        leftovers: Vec<(PathBuf, u64)>,
        bundle_id: Option<String>,
    },
}

/// A bundle id is reverse-DNS: letters, digits, dots and dashes only.
pub fn valid_bundle_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// Leftover support files of a Mac app in a Library folder, by bundle id.
pub fn leftover_paths(library: &std::path::Path, bundle_id: &str) -> Vec<PathBuf> {
    if !valid_bundle_id(bundle_id) {
        return Vec::new();
    }
    [
        format!("Application Support/{bundle_id}"),
        format!("Caches/{bundle_id}"),
        format!("Containers/{bundle_id}"),
        format!("Preferences/{bundle_id}.plist"),
        format!("Saved Application State/{bundle_id}.savedState"),
        format!("HTTPStorages/{bundle_id}"),
        format!("WebKit/{bundle_id}"),
        format!("Logs/{bundle_id}"),
    ]
    .iter()
    .map(|rel| library.join(rel))
    .filter(|p| {
        p.symlink_metadata()
            .is_ok_and(|m| !m.file_type().is_symlink())
    })
    .collect()
}

pub use imp::{installed_apps, launch_uninstaller};

#[cfg(windows)]
mod imp {
    use super::{InstalledApp, Outcome, split_command};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };

    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

    fn sources() -> [(&'static str, winreg::HKEY, u32); 3] {
        [
            ("HKCU", HKEY_CURRENT_USER, KEY_WOW64_64KEY),
            ("HKLM64", HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            ("HKLM32", HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        ]
    }

    fn open(hive: winreg::HKEY, view: u32, key: &str) -> Option<RegKey> {
        RegKey::predef(hive)
            .open_subkey_with_flags(format!(r"{UNINSTALL}\{key}"), KEY_READ | view)
            .ok()
    }

    fn text(k: &RegKey, name: &str) -> Option<String> {
        k.get_value::<String, _>(name)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn num(k: &RegKey, name: &str) -> Option<u32> {
        k.get_value::<u32, _>(name).ok()
    }

    pub fn installed_apps() -> Vec<InstalledApp> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for (tag, hive, view) in sources() {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, KEY_READ | view)
            else {
                continue;
            };
            for key in root.enum_keys().flatten() {
                let Some(k) = open(hive, view, &key) else {
                    continue;
                };
                let Some(name) = text(&k, "DisplayName") else {
                    continue;
                };
                let hidden = num(&k, "SystemComponent") == Some(1)
                    || text(&k, "ParentKeyName").is_some()
                    || text(&k, "ReleaseType")
                        .is_some_and(|t| t.contains("Update") || t.contains("Hotfix"));
                if hidden {
                    continue;
                }
                let version = text(&k, "DisplayVersion");
                if !seen.insert((name.to_lowercase(), version.clone())) {
                    continue;
                }
                let msi = num(&k, "WindowsInstaller") == Some(1);
                out.push(InstalledApp {
                    id: format!("{tag}|{key}"),
                    name,
                    publisher: text(&k, "Publisher"),
                    version,
                    install_date: text(&k, "InstallDate"),
                    reported_bytes: num(&k, "EstimatedSize").map(|kb| kb as u64 * 1024),
                    measured_bytes: None,
                    install_location: text(&k, "InstallLocation")
                        .map(|p| PathBuf::from(p.trim_matches('"'))),
                    per_user: tag == "HKCU",
                    can_uninstall: msi || text(&k, "UninstallString").is_some(),
                });
            }
        }
        out.extend(store_apps());
        out.sort_by_key(|a| std::cmp::Reverse(a.measured_bytes.or(a.reported_bytes).unwrap_or(0)));
        out
    }

    fn com() {
        // WinRT needs COM on this thread; an existing setup is fine.
        // SAFETY: plain initialisation call with no pointers.
        let _ = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
        };
    }

    /// Microsoft Store apps installed for this user (as in Windows Settings).
    fn store_apps() -> Vec<InstalledApp> {
        use windows::ApplicationModel::PackageSignatureKind;
        use windows::Management::Deployment::PackageManager;
        com();
        let Ok(pm) = PackageManager::new() else {
            return Vec::new();
        };
        let Ok(pkgs) = pm.FindPackagesByUserSecurityId(&windows_core::HSTRING::new()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in pkgs {
            let hidden = p.IsFramework().unwrap_or(true)
                || p.IsResourcePackage().unwrap_or(true)
                || p.IsStub().unwrap_or(false)
                || p.SignatureKind()
                    .map(|k| k == PackageSignatureKind::System)
                    .unwrap_or(true);
            if hidden {
                continue;
            }
            let name = p.DisplayName().map(|s| s.to_string()).unwrap_or_default();
            if name.is_empty() || name.starts_with("ms-resource:") {
                continue;
            }
            let Ok(id) = p.Id() else { continue };
            let Ok(full) = id.FullName() else { continue };
            let version = id
                .Version()
                .ok()
                .map(|v| format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision));
            let install_date = p
                .InstalledDate()
                .ok()
                .map(|d| yyyymmdd(d.UniversalTime / 10_000_000 - 11_644_473_600));
            let location = p.InstalledPath().ok().map(|s| PathBuf::from(s.to_string()));
            // Windows keeps no size for Store apps that apps can read, so the
            // package's own folder is measured (shown as measured, not reported).
            let measured = location.as_deref().map(folder_size).filter(|&n| n > 0);
            out.push(InstalledApp {
                id: format!("APPX|{full}"),
                name,
                publisher: p
                    .PublisherDisplayName()
                    .ok()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty()),
                version,
                install_date,
                reported_bytes: None,
                measured_bytes: measured,
                install_location: location,
                per_user: true,
                can_uninstall: true,
            });
        }
        out
    }

    fn folder_size(p: &std::path::Path) -> u64 {
        let mut total = 0;
        let mut stack = vec![p.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in rd.flatten() {
                match e.file_type() {
                    Ok(t) if t.is_dir() => stack.push(e.path()),
                    Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                    _ => {}
                }
            }
        }
        total
    }

    fn yyyymmdd(t: i64) -> String {
        let z = t.div_euclid(86_400) + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        format!("{y:04}{m:02}{d:02}")
    }

    fn remove_store_app(full: &str) -> Result<Outcome, String> {
        use windows::Management::Deployment::PackageManager;
        com();
        let pm = PackageManager::new().map_err(|e| e.message())?;
        let op = pm
            .RemovePackageAsync(&windows_core::HSTRING::from(full))
            .map_err(|e| e.message())?;
        let result = op
            .join()
            .map_err(|e| format!("Windows could not remove it: {}", e.message()))?;
        let text = result
            .ErrorText()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !text.is_empty() {
            return Err(format!("Windows could not remove it: {text}"));
        }
        Ok(Outcome::Removed)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Opens the app's own uninstaller. Windows may ask for administrator
    /// permission for machine-wide apps.
    pub fn launch_uninstaller(id: &str) -> Result<Outcome, String> {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;

        let (tag, key) = id.split_once('|').ok_or("Unknown app.")?;
        if tag == "APPX" {
            return remove_store_app(key);
        }
        let (_, hive, view) = sources()
            .into_iter()
            .find(|(t, _, _)| *t == tag)
            .ok_or("Unknown app.")?;
        let k = open(hive, view, key).ok_or("This app is no longer installed.")?;
        let (program, args) = if num(&k, "WindowsInstaller") == Some(1) && key.starts_with('{') {
            // What Windows Settings does for Windows Installer apps.
            ("msiexec.exe".to_string(), format!("/x {key}"))
        } else {
            let cmd = text(&k, "UninstallString")
                .ok_or("This app has no uninstaller. Use Windows Settings > Apps.")?;
            let (p, a) = split_command(&cmd).ok_or(
                "This app's uninstaller could not be understood. Use Windows Settings > Apps.",
            )?;
            if !std::path::Path::new(&p).is_file() {
                return Err(
                    "This app's uninstaller is missing. Use Windows Settings > Apps.".into(),
                );
            }
            (p, a)
        };
        let (verb, prog, params) = (wide("open"), wide(&program), wide(&args));
        // SAFETY: all strings are NUL-terminated and outlive the call.
        let r = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(prog.as_ptr()),
                PCWSTR(params.as_ptr()),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if r.0 as usize > 32 {
            Ok(Outcome::OpenedUninstaller)
        } else {
            Err("The uninstaller could not be started.".into())
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{InstalledApp, Outcome, leftover_paths};
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    fn home() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }

    fn roots() -> Vec<PathBuf> {
        let mut r = vec![PathBuf::from("/Applications")];
        if let Some(h) = home() {
            r.push(h.join("Applications"));
        }
        r
    }

    /// (name, version, bundle id) from the app's Info.plist.
    pub fn read_bundle(app: &Path) -> Option<(String, Option<String>, Option<String>)> {
        let v = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
        let d = v.as_dictionary()?;
        let s = |k: &str| {
            d.get(k)
                .and_then(|x| x.as_string())
                .map(str::to_string)
                .filter(|x| !x.is_empty())
        };
        let name = s("CFBundleDisplayName")
            .or_else(|| s("CFBundleName"))
            .or_else(|| app.file_stem().map(|n| n.to_string_lossy().into_owned()))?;
        Some((
            name,
            s("CFBundleShortVersionString"),
            s("CFBundleIdentifier"),
        ))
    }

    pub fn size_of(p: &Path) -> u64 {
        let mut total = 0;
        let mut stack = vec![p.to_path_buf()];
        while let Some(d) = stack.pop() {
            match std::fs::symlink_metadata(&d) {
                Ok(m) if m.is_dir() => {
                    if let Ok(rd) = std::fs::read_dir(&d) {
                        stack.extend(rd.flatten().map(|e| e.path()));
                    }
                }
                Ok(m) => total += m.blocks() * 512,
                Err(_) => {}
            }
        }
        total
    }

    /// Third-party apps directly inside `roots` (Apple's own apps excluded).
    pub fn apps_in(roots: &[PathBuf]) -> Vec<InstalledApp> {
        let mut out = Vec::new();
        for root in roots {
            let Ok(rd) = std::fs::read_dir(root) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                let is_app = p.extension().and_then(|x| x.to_str()) == Some("app")
                    && p.symlink_metadata().is_ok_and(|m| m.is_dir());
                if !is_app {
                    continue;
                }
                let Some((name, version, bundle)) = read_bundle(&p) else {
                    continue;
                };
                if bundle
                    .as_deref()
                    .is_some_and(|b| b.starts_with("com.apple."))
                {
                    continue;
                }
                out.push(InstalledApp {
                    id: format!("APP|{}", p.display()),
                    name,
                    publisher: bundle,
                    version,
                    install_date: None,
                    reported_bytes: None,
                    measured_bytes: Some(size_of(&p)),
                    install_location: Some(p.clone()),
                    per_user: root != Path::new("/Applications"),
                    can_uninstall: true,
                });
            }
        }
        out.sort_by_key(|a| std::cmp::Reverse(a.measured_bytes.or(a.reported_bytes).unwrap_or(0)));
        out
    }

    pub fn installed_apps() -> Vec<InstalledApp> {
        apps_in(&roots())
    }

    /// Moves the app to the Trash. `roots` limits which folders it may be in.
    pub fn trash_app(
        id: &str,
        roots: &[PathBuf],
        library: Option<&Path>,
    ) -> Result<Outcome, String> {
        let p = PathBuf::from(id.strip_prefix("APP|").ok_or("Unknown app.")?);
        let allowed = p.extension().and_then(|x| x.to_str()) == Some("app")
            && p.parent().is_some_and(|d| roots.iter().any(|r| r == d))
            && p.symlink_metadata().is_ok_and(|m| m.is_dir());
        if !allowed {
            return Err("This app is no longer installed.".into());
        }
        let bundle_id = read_bundle(&p).and_then(|(_, _, b)| b);
        let leftovers = match (&bundle_id, library) {
            (Some(b), Some(lib)) => leftover_paths(lib, b)
                .into_iter()
                .map(|l| {
                    let n = size_of(&l);
                    (l, n)
                })
                .collect(),
            _ => Vec::new(),
        };
        let bytes = size_of(&p);
        let recycled = tessera_cleanup::move_to_trash(&p)
            .map_err(|e| format!("It couldn't be moved to the Trash: {e}"))?;
        Ok(Outcome::MovedToTrash {
            original: p,
            recycled,
            bytes,
            leftovers,
            bundle_id,
        })
    }

    pub fn launch_uninstaller(id: &str) -> Result<Outcome, String> {
        let lib = home().map(|h| h.join("Library"));
        trash_app(id, &roots(), lib.as_deref())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::{InstalledApp, Outcome};

    pub fn installed_apps() -> Vec<InstalledApp> {
        Vec::new()
    }

    pub fn launch_uninstaller(_id: &str) -> Result<Outcome, String> {
        Err("Uninstalling apps from Tessera is not available on this system.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_and_unquoted_commands() {
        assert_eq!(
            split_command(r#""C:\Program Files\App\unins000.exe" /SILENT"#),
            Some((
                r"C:\Program Files\App\unins000.exe".into(),
                "/SILENT".into()
            ))
        );
        assert_eq!(
            split_command(r"C:\Apps\Tool\uninstall.exe --remove"),
            Some((r"C:\Apps\Tool\uninstall.exe".into(), "--remove".into()))
        );
        assert_eq!(split_command("no program here"), None);
    }

    #[cfg(windows)]
    #[test]
    fn lists_installed_apps_read_only() {
        let apps = installed_apps();
        assert!(!apps.is_empty(), "a Windows system has installed apps");
        assert!(
            apps.iter()
                .all(|a| !a.name.is_empty() && a.id.contains('|'))
        );
    }

    #[test]
    fn bundle_ids_are_validated_before_building_paths() {
        assert!(valid_bundle_id("com.example.App-1"));
        for bad in ["", "../etc", "com/evil", "a..b", "com.example App"] {
            assert!(!valid_bundle_id(bad), "{bad}");
        }
        assert!(leftover_paths(std::path::Path::new("/tmp"), "../x").is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_app_listed_trashed_with_leftovers_and_put_back() {
        let t = tessera_testkit::tempfile::Builder::new()
            .prefix("tessera-apps-")
            .tempdir()
            .unwrap();
        let apps = t.path().join("Applications");
        let app = apps.join("Tessera Trial.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::write(app.join("Contents/MacOS/trial"), vec![7u8; 64 * 1024]).unwrap();
        std::fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleName</key><string>Tessera Trial</string><key>CFBundleIdentifier</key><string>app.tessera.trial</string><key>CFBundleShortVersionString</key><string>1.2</string></dict></plist>"#,
        )
        .unwrap();
        let lib = t.path().join("Library");
        std::fs::create_dir_all(lib.join("Caches/app.tessera.trial")).unwrap();
        std::fs::write(lib.join("Caches/app.tessera.trial/cache.db"), b"x").unwrap();
        std::fs::create_dir_all(lib.join("Caches/someone.else")).unwrap();

        let listed = imp::apps_in(std::slice::from_ref(&apps));
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Tessera Trial");
        assert_eq!(listed[0].version.as_deref(), Some("1.2"));
        assert!(listed[0].measured_bytes.unwrap() >= 64 * 1024);

        let out = imp::trash_app(&listed[0].id, std::slice::from_ref(&apps), Some(&lib)).unwrap();
        let Outcome::MovedToTrash {
            original,
            recycled,
            leftovers,
            bundle_id,
            ..
        } = out
        else {
            panic!("expected a Trash move")
        };
        assert!(!original.exists(), "the app is in the Trash");
        assert_eq!(bundle_id.as_deref(), Some("app.tessera.trial"));
        assert_eq!(
            leftovers.len(),
            1,
            "only the app's own cache folder: {leftovers:?}"
        );
        let recycled = recycled.expect("macOS reports where it landed");
        tessera_cleanup::put_back(&recycled, &original).unwrap();
        assert!(app.join("Contents/Info.plist").exists(), "put back intact");

        // An app outside the allowed folders is refused.
        assert!(
            imp::trash_app(
                &format!("APP|{}", app.display()),
                &[t.path().join("Other")],
                None
            )
            .is_err()
        );
    }
}
