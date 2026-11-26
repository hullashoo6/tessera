//! Tessera Pro licence commands. Network calls go only to Dodo's public
//! licence endpoints for this build's mode (test mode until approved) and
//! run off the main thread. A licence problem never affects scanning.

use std::time::{SystemTime, UNIX_EPOCH};
use tessera_licensing::{Entitlement, HttpsTransport, KeyringStore, Licensing};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn licensing() -> Licensing<HttpsTransport, KeyringStore> {
    Licensing {
        config: tessera_licensing::active(),
        transport: HttpsTransport::default(),
        store: KeyringStore::new("app.tessera.desktop"),
        device_label: format!(
            "Tessera on {}",
            if cfg!(target_os = "macos") {
                "macOS"
            } else {
                "Windows"
            }
        ),
    }
}

/// Whether Pro features may be used right now (no network access).
pub fn is_pro() -> bool {
    // Development builds only: TESSERA_DEV_PRO=1 unlocks Pro for local
    // testing. Compiled out of release builds.
    #[cfg(debug_assertions)]
    if std::env::var_os("TESSERA_DEV_PRO").is_some() {
        return true;
    }
    licensing().status(now()).pro
}

#[tauri::command(async)]
pub fn license_status() -> Entitlement {
    licensing().status(now())
}

#[tauri::command]
pub async fn refresh_license(force: Option<bool>) -> Entitlement {
    let force = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || licensing().refresh(now(), force))
        .await
        .unwrap_or_else(|_| licensing().status(now()))
}

#[tauri::command]
pub async fn activate_license(key: String) -> Result<Entitlement, String> {
    if key.len() > 200 {
        return Err("That doesn't look like a licence key.".into());
    }
    tauri::async_runtime::spawn_blocking(move || licensing().activate(&key, now()))
        .await
        .map_err(|_| "Activation stopped unexpectedly. Please try again.".to_string())?
}

#[tauri::command]
pub async fn deactivate_license() -> Result<Entitlement, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let l = licensing();
        l.deactivate()?;
        Ok(l.status(now()))
    })
    .await
    .map_err(|_| "Deactivation stopped unexpectedly. Please try again.".to_string())?
}
