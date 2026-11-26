//! Update check: fetches the website's public `latest.json` and compares it
//! with this version. Never downloads or installs anything; the person opens
//! the download page if they want the new version.

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

const LATEST_URL: &str = "https://tessera-96w.pages.dev/latest.json";
const DOWNLOAD_PAGE: &str = "https://tessera-96w.pages.dev/#download";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDto {
    available: bool,
    current: String,
    latest: Option<String>,
}

/// Numeric "1.2.3" comparison; anything unparsable is never "newer".
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> {
        v.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| p.parse().ok())
            .collect()
    };
    match (parse(latest), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateDto, String> {
    let current = app.package_info().version.to_string();
    let doc =
        tauri::async_runtime::spawn_blocking(|| tessera_licensing::https_get_json(LATEST_URL))
            .await
            .map_err(|_| "The update check stopped unexpectedly.".to_string())??;
    let latest = doc
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(UpdateDto {
        available: latest.as_deref().is_some_and(|l| is_newer(l, &current)),
        current,
        latest,
    })
}

/// Opens Tessera's download page in the browser (the only address allowed).
#[tauri::command]
pub fn open_download_page(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(DOWNLOAD_PAGE, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }
}
