//! Optional background mode (Settings > Background, off by default).
//!
//! * Keep running: closing the window leaves Tessera in the notification area
//!   (Windows) or the menu bar (Mac), showing the main drive's free space,
//!   with Open, Scan main drive and Quit.
//! * Low-space warning: while Tessera is running, a system notification when
//!   the main drive is almost full, at most once a day.
//! * Start at sign-in (needs keep running): Tessera starts hidden in the
//!   notification area or menu bar when the person signs in, through the
//!   official autostart plugin (a Run entry on Windows, a Launch Agent on
//!   macOS). The flag `--background` marks such a start.
//!
//! Nothing is scanned in the background: only the drive's free space is read,
//! which the system reports instantly. Settings are kept on this computer.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tessera_scan::volume_space;

const CHECK_EVERY: Duration = Duration::from_secs(10 * 60);
const ALERT_GAP_SECS: i64 = 24 * 3_600;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackgroundSettings {
    pub keep_running: bool,
    pub low_space_alerts: bool,
    /// Registered to start at sign-in. The system's own registration is the
    /// truth; this mirrors it for the interface.
    pub start_at_login: bool,
    /// The one-time "Tessera is still running" notice has been shown.
    pub told_about_tray: bool,
}

/// Command-line flag of a start at sign-in.
pub const BACKGROUND_FLAG: &str = "--background";

/// True when Tessera was started at sign-in and should stay out of the way
/// (only if keep-running is still on; otherwise the window opens as usual).
pub fn starts_hidden(app: &AppHandle) -> bool {
    std::env::args().any(|a| a == BACKGROUND_FLAG)
        && app
            .state::<Background>()
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keep_running
}

fn login_registered(app: &AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[derive(Default)]
pub struct Background {
    settings: Mutex<BackgroundSettings>,
    tray: Mutex<Option<(TrayIcon<Wry>, MenuItem<Wry>)>>,
    last_alert: Mutex<i64>,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("background.json"))
}

fn save(app: &AppHandle, s: &BackgroundSettings) {
    if let Some(path) = file(app) {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(bytes) = serde_json::to_vec(s) {
            let _ = std::fs::write(path, bytes);
        }
    }
}

/// The drive the system runs from.
pub fn system_root() -> PathBuf {
    if cfg!(windows) {
        let d = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        PathBuf::from(format!("{}\\", d.trim_end_matches('\\')))
    } else {
        PathBuf::from("/")
    }
}

/// Sizes the way the system shows them (1 GB = 1024³ bytes in Windows,
/// 1000³ in macOS), one decimal.
pub fn human(bytes: u64) -> String {
    let base: f64 = if cfg!(target_os = "macos") {
        1000.0
    } else {
        1024.0
    };
    let units = ["bytes", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= base && i < units.len() - 1 {
        v /= base;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{v:.1} {}", units[i])
    }
}

/// Almost full: less than 10% of the drive free, and less than 20 GB.
pub fn is_low(available: u64, total: u64) -> bool {
    total > 0 && available < (total / 10).min(20 << 30)
}

fn drive_label(root: &std::path::Path) -> String {
    if cfg!(windows) {
        root.to_string_lossy().trim_end_matches('\\').to_string()
    } else {
        "your Mac".into()
    }
}

pub fn init(app: &AppHandle) {
    let settings: BackgroundSettings = file(app)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let state = app.state::<Background>();
    *state.settings.lock().unwrap_or_else(|e| e.into_inner()) = settings;
    if settings.keep_running {
        show_tray(app);
    }
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("tessera-free-space".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            loop {
                check(&app);
                std::thread::sleep(CHECK_EVERY);
            }
        });
}

/// Reads the main drive's free space, updates the tray text and warns if low.
fn check(app: &AppHandle) {
    let root = system_root();
    let Ok(space) = volume_space(&root) else {
        return;
    };
    let state = app.state::<Background>();
    let text = format!("{} free on {}", human(space.available), drive_label(&root));
    if let Some((tray, item)) = &*state.tray.lock().unwrap_or_else(|e| e.into_inner()) {
        let _ = item.set_text(&text);
        let _ = tray.set_tooltip(Some(format!("Tessera: {text}")));
    }
    let alerts = state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .low_space_alerts;
    if alerts && is_low(space.available, space.total) {
        let now = super::unix_now();
        let mut last = state.last_alert.lock().unwrap_or_else(|e| e.into_inner());
        if now - *last >= ALERT_GAP_SECS {
            *last = now;
            let _ = app
                .notification()
                .builder()
                .title("Your main drive is almost full")
                .body(format!(
                    "Only {text}. Open Tessera to see what is using the space."
                ))
                .show();
        }
    }
}

fn show_tray(app: &AppHandle) {
    let state = app.state::<Background>();
    let mut slot = state.tray.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_some() {
        return;
    }
    let build = || -> tauri::Result<(TrayIcon<Wry>, MenuItem<Wry>)> {
        let open = MenuItem::with_id(app, "open", "Open Tessera", true, None::<&str>)?;
        let space = MenuItem::with_id(app, "space", "Checking free space…", false, None::<&str>)?;
        let scan = MenuItem::with_id(app, "scan", "Scan main drive", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit Tessera", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &open,
                &space,
                &PredefinedMenuItem::separator(app)?,
                &scan,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;
        let mut builder = TrayIconBuilder::with_id("tessera")
            .tooltip("Tessera")
            .menu(&menu)
            // Menu-bar convention on a Mac: a click opens the menu. Windows:
            // a click opens the window and the menu is on right-click.
            .show_menu_on_left_click(cfg!(target_os = "macos"))
            .on_menu_event(|app, event| match event.id().as_ref() {
                "open" => super::launch::focus(app),
                "scan" => {
                    super::launch::focus(app);
                    super::launch::scan_folder(app, &system_root());
                }
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                    && !cfg!(target_os = "macos")
                {
                    super::launch::focus(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        Ok((builder.build(app)?, space))
    };
    match build() {
        Ok(made) => *slot = Some(made),
        Err(e) => super::diagnostics::record("tray", &format!("could not show the icon: {e}")),
    }
    drop(slot);
    let app = app.clone();
    std::thread::spawn(move || check(&app));
}

fn hide_tray(app: &AppHandle) {
    let state = app.state::<Background>();
    if let Some((tray, _)) = state.tray.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = tray.set_visible(false);
        app.remove_tray_by_id("tessera");
    }
}

/// Called when the window's close button is pressed. True if Tessera should
/// keep running (the window is hidden instead of closed).
pub fn keep_running_on_close(app: &AppHandle) -> bool {
    let state = app.state::<Background>();
    let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    if !s.keep_running {
        return false;
    }
    if !s.told_about_tray {
        s.told_about_tray = true;
        save(app, &s);
        let place = if cfg!(target_os = "macos") {
            "the menu bar"
        } else {
            "the notification area of the taskbar"
        };
        let _ = app
            .notification()
            .builder()
            .title("Tessera is still running")
            .body(format!(
                "It is in {place}. To close it completely, choose Quit Tessera there, or turn this off in Settings."
            ))
            .show();
    }
    true
}

#[tauri::command(async)]
pub fn background_settings(app: AppHandle) -> BackgroundSettings {
    let registered = login_registered(&app);
    let state = app.state::<Background>();
    let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    s.start_at_login = registered;
    *s
}

#[tauri::command(async)]
pub fn set_background_settings(
    app: AppHandle,
    keep_running: bool,
    low_space_alerts: bool,
    start_at_login: bool,
) -> Result<BackgroundSettings, String> {
    use tauri_plugin_autostart::ManagerExt;
    // Starting at sign-in only makes sense when Tessera keeps running. A
    // Microsoft Store (MSIX) copy can't use the sign-in Run entry: Windows
    // keeps a packaged app's registry writes private, so it would silently
    // do nothing. Store copies need the package's own startup task instead
    // (not built yet), so the option is refused there.
    let want_login = start_at_login && keep_running && !super::update::store_managed();
    if want_login != login_registered(&app) {
        let result = if want_login {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(e) = result {
            super::diagnostics::record("autostart", &e.to_string());
            return Err("Tessera could not change whether it starts when you sign in.".into());
        }
    }
    let updated = {
        let state = app.state::<Background>();
        let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        s.keep_running = keep_running;
        s.low_space_alerts = low_space_alerts;
        s.start_at_login = login_registered(&app);
        save(&app, &s);
        *s
    };
    if keep_running {
        show_tray(&app);
    } else {
        hide_tray(&app);
    }
    if low_space_alerts {
        let a = app.clone();
        std::thread::spawn(move || check(&a));
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn almost_full_means_under_ten_percent_and_under_20_gb() {
        let gb = 1u64 << 30;
        assert!(is_low(5 * gb, 236 * gb));
        assert!(!is_low(47 * gb, 236 * gb));
        assert!(
            !is_low(25 * gb, 2_000 * gb),
            "big drives: 20 GB is still plenty"
        );
        assert!(is_low(19 * gb, 2_000 * gb));
        assert!(is_low(gb, 64 * gb));
        assert!(!is_low(7 * gb, 64 * gb), "64 GB drive: 10% is 6.4 GB");
        assert!(!is_low(0, 0), "unknown size is never 'full'");
    }

    #[test]
    fn sizes_read_like_the_system() {
        assert_eq!(human(512), "512 bytes");
        if cfg!(target_os = "macos") {
            assert_eq!(human(47_500_000_000), "47.5 GB");
        } else {
            assert_eq!(human(51_002_736_640), "47.5 GB");
        }
    }

    #[test]
    fn settings_default_off_and_tolerate_old_files() {
        let s: BackgroundSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, BackgroundSettings::default());
        assert!(!s.keep_running && !s.low_space_alerts);
        let s: BackgroundSettings = serde_json::from_str(r#"{"keepRunning":true}"#).unwrap();
        assert!(s.keep_running && !s.low_space_alerts);
    }
}
