//! Installed apps and their own uninstallers.
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
    pub install_location: Option<PathBuf>,
    /// True when installed for this user only.
    pub per_user: bool,
    pub can_uninstall: bool,
}

pub fn supported() -> bool {
    cfg!(windows)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The app's own uninstaller was opened; it finishes the job.
    OpenedUninstaller,
    /// A Microsoft Store app was removed by Windows.
    Removed,
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
                    install_location: text(&k, "InstallLocation")
                        .map(|p| PathBuf::from(p.trim_matches('"'))),
                    per_user: tag == "HKCU",
                    can_uninstall: msi || text(&k, "UninstallString").is_some(),
                });
            }
        }
        out.extend(store_apps());
        out.sort_by_key(|a| std::cmp::Reverse(a.reported_bytes.unwrap_or(0)));
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
                install_location: p.InstalledPath().ok().map(|s| PathBuf::from(s.to_string())),
                per_user: true,
                can_uninstall: true,
            });
        }
        out
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

#[cfg(not(windows))]
mod imp {
    use super::{InstalledApp, Outcome};

    pub fn installed_apps() -> Vec<InstalledApp> {
        Vec::new()
    }

    pub fn launch_uninstaller(_id: &str) -> Result<Outcome, String> {
        Err("Uninstalling apps from Tessera is not available on this system yet.".into())
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
}
