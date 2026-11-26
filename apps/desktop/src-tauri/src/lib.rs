//! Tessera desktop backend.
//!
//! The interface talks to this module through a small set of commands. It
//! never sends file paths: scans start from a detected drive, the home
//! folder, the native folder picker, or a folder the person hands to Tessera
//! from outside the window (`launch.rs`), and every other request refers to
//! items by id within the current scan. Nothing here modifies files.

mod background;
mod diagnostics;
mod dto;
mod history;
mod launch;
mod licence;
mod safety;
mod update;

use licence::{activate_license, deactivate_license, is_pro, license_status, refresh_license};

use dto::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tessera_scan::query::{
    SortKey, largest_files as top_files, search as run_search, sorted_children,
};
use tessera_scan::{
    CancelToken, HardLinkMode, NodeId, NodeKind, Progress, ScanOptions, ScanSummary, Tree,
    list_volumes as volumes, prepare_root, scan_shared, volume_space,
};
use tessera_treemap::{Item, Params, Rect, layout};

enum Outcome {
    Running(Progress),
    Finished(ScanSummary),
    Failed(String),
}

struct ActiveScan {
    id: u64,
    root: PathBuf,
    tree: Arc<RwLock<Tree>>,
    cancel: CancelToken,
    outcome: Mutex<Outcome>,
}

#[derive(Default)]
struct AppState {
    scan: Mutex<Option<Arc<ActiveScan>>>,
    review: Mutex<Option<PendingReview>>,
    dupes_cancel: Mutex<Option<Arc<std::sync::atomic::AtomicBool>>>,
    next_id: AtomicU64,
}

type CmdResult<T> = Result<T, String>;

fn active(state: &AppState, scan_id: u64) -> CmdResult<Arc<ActiveScan>> {
    match &*state.scan.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(s) if s.id == scan_id => Ok(s.clone()),
        _ => Err("This scan is no longer available. Start a new scan.".into()),
    }
}

fn read(scan: &ActiveScan) -> std::sync::RwLockReadGuard<'_, Tree> {
    scan.tree.read().unwrap_or_else(|e| e.into_inner())
}

fn check_id(tree: &Tree, id: NodeId) -> CmdResult<()> {
    if (id as usize) < tree.len() {
        Ok(())
    } else {
        Err("That item is not part of this scan.".into())
    }
}

#[tauri::command]
fn app_info() -> AppInfoDto {
    AppInfoDto {
        os: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command(async)]
fn list_volumes() -> Vec<VolumeDto> {
    volumes().into_iter().map(VolumeDto::from).collect()
}

#[tauri::command]
async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    target: ScanTarget,
    settings: ScanSettings,
) -> CmdResult<Option<u64>> {
    let chosen: PathBuf = match target {
        ScanTarget::Volume { mount } => volumes()
            .into_iter()
            .find(|v| v.mount_point.to_string_lossy() == mount)
            .map(|v| v.mount_point)
            .ok_or("That drive is no longer available.")?,
        ScanTarget::Home => app
            .path()
            .home_dir()
            .map_err(|_| "Your home folder could not be found.")?,
        ScanTarget::Folder => {
            let handle = app.clone();
            let picked = tauri::async_runtime::spawn_blocking(move || {
                handle
                    .dialog()
                    .file()
                    .set_title("Choose a folder to scan")
                    .blocking_pick_folder()
            })
            .await
            .map_err(|_| "The folder picker closed unexpectedly.")?;
            match picked {
                None => return Ok(None),
                Some(p) => p
                    .into_path()
                    .map_err(|_| "That location cannot be scanned.")?,
            }
        }
    };
    let root = prepare_root(&chosen).map_err(|e| format!("Cannot scan this location: {e}."))?;
    let opts = ScanOptions {
        hard_links: match settings.hard_links {
            HardLinkSetting::Auto => HardLinkMode::Auto,
            HardLinkSetting::Thorough => HardLinkMode::Thorough,
            HardLinkSetting::Off => HardLinkMode::Off,
        },
        ..ScanOptions::default()
    };
    begin_scan(app, &state, root, opts).map(Some)
}

/// Replaces any running scan with a new one on a background thread.
fn begin_scan(
    app: AppHandle,
    state: &AppState,
    root: PathBuf,
    opts: ScanOptions,
) -> CmdResult<u64> {
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let scan = Arc::new(ActiveScan {
        id,
        root: root.clone(),
        tree: Arc::new(RwLock::new(Tree::new(root))),
        cancel: CancelToken::new(),
        outcome: Mutex::new(Outcome::Running(Progress::default())),
    });
    {
        let mut slot = state.scan.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = slot.take() {
            previous.cancel.cancel();
        }
        *slot = Some(scan.clone());
    }
    std::thread::Builder::new()
        .name("tessera-scan".into())
        .spawn(move || run_scan(app, scan, opts))
        .map_err(|_| "The scan could not be started.")?;
    Ok(id)
}

fn run_scan(app: AppHandle, scan: Arc<ActiveScan>, opts: ScanOptions) {
    let id = scan.id;
    // An unexpected fault inside the scanner ends this scan with a message
    // (the crash itself goes to the local log) instead of leaving the window
    // showing "Scanning" forever. Locks recover from poisoning everywhere.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scan_shared(&scan.tree, &opts, &scan.cancel, &mut |p| {
            *scan.outcome.lock().unwrap_or_else(|e| e.into_inner()) = Outcome::Running(*p);
            let _ = app.emit("scan-progress", ProgressDto::new(id, p));
        })
    }));
    if let Ok(Ok(summary)) = &result
        && !summary.cancelled
    {
        history::record(&app, &read(&scan), unix_now());
    }
    let outcome = match result {
        Ok(Ok(summary)) => Outcome::Finished(summary),
        Ok(Err(e)) => {
            let message = format!("The scan stopped: {e}.");
            diagnostics::record("scan failed", &message);
            Outcome::Failed(message)
        }
        Err(_) => Outcome::Failed(
            "Something went wrong during the scan, and it stopped. Your files were not changed. \
             Please try again; if it keeps happening, Settings > Diagnostics shows a log you can send us."
                .into(),
        ),
    };
    *scan.outcome.lock().unwrap_or_else(|e| e.into_inner()) = outcome;
    let _ = app.emit("scan-finished", id);
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    if let Some(s) = &*state.scan.lock().unwrap_or_else(|e| e.into_inner()) {
        s.cancel.cancel();
    }
}

#[tauri::command(async)]
fn scan_status(state: State<'_, AppState>) -> Option<StatusDto> {
    let scan = state
        .scan
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()?;
    let outcome = scan.outcome.lock().unwrap_or_else(|e| e.into_inner());
    let (phase, progress, summary, error) = match &*outcome {
        Outcome::Running(p) => ("scanning", ProgressDto::new(scan.id, p), None, None),
        Outcome::Finished(s) => {
            // Totals come from the live tree, so they reflect any cleanup
            // done since the scan finished.
            let mut p = ProgressDto::new(scan.id, &s.progress);
            let tree = read(&scan);
            let root = tree.root();
            (p.files, p.dirs, p.logical, p.allocated) = (
                u64::from(root.files),
                u64::from(root.dirs),
                root.logical,
                root.allocated,
            );
            drop(tree);
            (
                if s.cancelled { "cancelled" } else { "complete" },
                p,
                Some(SummaryDto::from(s)),
                None,
            )
        }
        Outcome::Failed(e) => (
            "failed",
            ProgressDto::new(scan.id, &Progress::default()),
            None,
            Some(e.clone()),
        ),
    };
    Some(StatusDto {
        scan_id: scan.id,
        root: scan.root.to_string_lossy().into_owned(),
        phase,
        progress,
        summary,
        error,
        volume: volume_space(&scan.root).ok().map(SpaceDto::from),
    })
}

#[tauri::command(async)]
fn node_details(state: State<'_, AppState>, scan_id: u64, id: NodeId) -> CmdResult<DetailsDto> {
    let scan = active(&state, scan_id)?;
    let tree = read(&scan);
    check_id(&tree, id)?;
    let mut crumbs: Vec<CrumbDto> = tree
        .ancestors(id)
        .map(|a| CrumbDto::new(&tree, a))
        .collect();
    crumbs.reverse();
    crumbs.push(CrumbDto::new(&tree, id));
    Ok(DetailsDto {
        node: NodeDto::new(&tree, id),
        path: tree.path_of(id).to_string_lossy().into_owned(),
        crumbs,
    })
}

#[tauri::command(async)]
fn children(
    state: State<'_, AppState>,
    scan_id: u64,
    id: NodeId,
    metric: MetricSetting,
    offset: u32,
    limit: u32,
) -> CmdResult<PageDto> {
    let scan = active(&state, scan_id)?;
    let tree = read(&scan);
    check_id(&tree, id)?;
    let all = sorted_children(&tree, id, metric.into());
    let items = all
        .iter()
        .skip(offset as usize)
        .take(limit.min(500) as usize)
        .map(|&c| NodeDto::new(&tree, c))
        .collect();
    Ok(PageDto {
        total: all.len() as u32,
        items,
    })
}

#[tauri::command(async)]
fn treemap(
    state: State<'_, AppState>,
    scan_id: u64,
    id: NodeId,
    width: f64,
    height: f64,
    metric: MetricSetting,
) -> CmdResult<Vec<TileDto>> {
    if !(1.0..=20_000.0).contains(&width) || !(1.0..=20_000.0).contains(&height) {
        return Err("Invalid treemap size.".into());
    }
    let scan = active(&state, scan_id)?;
    let tree = read(&scan);
    check_id(&tree, id)?;
    let metric: tessera_scan::Metric = metric.into();
    // Children are supplied in the same order the side list uses.
    let tiles = layout(
        id,
        Rect::new(0.0, 0.0, width, height),
        &Params::default(),
        |parent| {
            sorted_children(&tree, parent, metric)
                .into_iter()
                .map(|c| Item {
                    id: c,
                    weight: metric.of(&tree, c),
                    container: tree.node(c).kind == NodeKind::Dir && tree.node(c).child_count > 0,
                })
                .collect()
        },
    );
    Ok(tiles.iter().map(|t| TileDto::new(&tree, t)).collect())
}

#[tauri::command(async)]
fn largest_files(
    state: State<'_, AppState>,
    scan_id: u64,
    under: Option<NodeId>,
    metric: MetricSetting,
    limit: u32,
) -> CmdResult<Vec<FileRowDto>> {
    let scan = active(&state, scan_id)?;
    let tree = read(&scan);
    if let Some(u) = under {
        check_id(&tree, u)?;
    }
    Ok(
        top_files(&tree, under, limit.min(1000) as usize, metric.into())
            .into_iter()
            .map(|id| FileRowDto {
                node: NodeDto::new(&tree, id),
                path: tree.path_of(id).to_string_lossy().into_owned(),
            })
            .collect(),
    )
}

#[tauri::command(async)]
fn search(
    state: State<'_, AppState>,
    scan_id: u64,
    query: SearchQuery,
    metric: MetricSetting,
    sort: SortSetting,
    limit: u32,
) -> CmdResult<SearchResultDto> {
    let filter = query.to_filter()?;
    let scan = active(&state, scan_id)?;
    let tree = read(&scan);
    if let Some(u) = filter.under {
        check_id(&tree, u)?;
    }
    let sort = match sort {
        SortSetting::Size => SortKey::Size,
        SortSetting::Name => SortKey::Name,
        SortSetting::Modified => SortKey::Modified,
        SortSetting::Type => SortKey::Type,
    };
    let hits = run_search(
        &tree,
        &filter,
        metric.into(),
        sort,
        limit.min(2000) as usize,
    );
    let mut containing = std::collections::BTreeSet::new();
    for &id in &hits.ids {
        containing.extend(tree.ancestors(id));
    }
    Ok(SearchResultDto {
        total: hits.total as u32,
        items: hits
            .ids
            .iter()
            .map(|&id| FileRowDto {
                node: NodeDto::new(&tree, id),
                path: tree.path_of(id).to_string_lossy().into_owned(),
            })
            .collect(),
        containing: containing.into_iter().collect(),
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SuggestionDto {
    id: NodeId,
    group: &'static str,
    label: &'static str,
    reason: String,
    safety: &'static str,
    safety_text: &'static str,
    bytes: u64,
    name: String,
    path: String,
}

/// Smart Scan suggestions for a finished scan. Seeing them is free;
/// acting on them goes through the Pro-gated cleanup.
#[tauri::command(async)]
fn smart_scan(state: State<'_, AppState>, scan_id: u64) -> CmdResult<Vec<SuggestionDto>> {
    use tessera_insights::{Group, Safety};
    let scan = active(&state, scan_id)?;
    if !matches!(
        *scan.outcome.lock().unwrap_or_else(|e| e.into_inner()),
        Outcome::Finished(_)
    ) {
        return Err("Suggestions appear when the scan finishes.".into());
    }
    let tree = read(&scan);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let found = tessera_insights::analyse(
        &tree,
        &tessera_insights::Places {
            cloud_roots: tessera_cleanup::cloud::sync_roots(),
            ..tessera_insights::Places::for_this_user()
        },
        now,
    );
    Ok(found
        .into_iter()
        .take(2000)
        .map(|s| SuggestionDto {
            id: s.id,
            group: match s.group {
                Group::Junk => "junk",
                Group::Developer => "developer",
                Group::Downloads => "downloads",
                Group::Cloud => "cloud",
                Group::Backups => "backups",
                Group::Old => "old",
            },
            label: s.label,
            safety: match s.safety {
                Safety::Regenerable => "regenerable",
                Safety::Redownloadable => "redownloadable",
                Safety::Review => "review",
                Safety::UseAppTool => "useAppTool",
                Safety::StaysInCloud => "staysInCloud",
            },
            safety_text: tessera_insights::safety_text(s.safety),
            reason: s.reason,
            bytes: s.bytes,
            name: tree.display_name(s.id).into_owned(),
            path: tree.path_of(s.id).to_string_lossy().into_owned(),
        })
        .collect())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DupGroupDto {
    size: u64,
    reclaimable: u64,
    hash: String,
    copies: Vec<FileRowDto>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DupReportDto {
    groups: Vec<DupGroupDto>,
    files_compared: u64,
    bytes_read: u64,
    skipped: u64,
    cancelled: bool,
}

/// Compares file contents to find identical files. Reads files locally;
/// online-only files are never opened. Results only; nothing is removed.
#[tauri::command]
async fn find_duplicates(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
) -> CmdResult<DupReportDto> {
    let scan = active(&state, scan_id)?;
    if !matches!(
        *scan.outcome.lock().unwrap_or_else(|e| e.into_inner()),
        Outcome::Finished(_)
    ) {
        return Err("Duplicates can be found when the scan finishes.".into());
    }
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    *state.dupes_cancel.lock().unwrap_or_else(|e| e.into_inner()) = Some(cancel.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let tree = read(&scan);
        let mut last = std::time::Instant::now();
        let report = tessera_dupes::find(
            &tree,
            &tessera_dupes::Options::default(),
            &cancel,
            &mut |bytes| {
                if last.elapsed() >= std::time::Duration::from_millis(250) {
                    last = std::time::Instant::now();
                    let _ = app.emit("dupes-progress", bytes);
                }
            },
        );
        DupReportDto {
            groups: report
                .groups
                .iter()
                .take(1000)
                .map(|g| DupGroupDto {
                    size: g.size,
                    reclaimable: g.reclaimable(),
                    hash: g.hash[..16].to_string(),
                    copies: g
                        .ids
                        .iter()
                        .map(|&id| FileRowDto {
                            node: NodeDto::new(&tree, id),
                            path: tree.path_of(id).to_string_lossy().into_owned(),
                        })
                        .collect(),
                })
                .collect(),
            files_compared: report.files_compared,
            bytes_read: report.bytes_read,
            skipped: report.skipped,
            cancelled: report.cancelled,
        }
    })
    .await
    .map_err(|_| "The duplicate search stopped unexpectedly.".to_string())
}

#[tauri::command]
fn cancel_duplicates(state: State<'_, AppState>) {
    if let Some(c) = &*state.dupes_cancel.lock().unwrap_or_else(|e| e.into_inner()) {
        c.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn history_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_local_data_dir()
        .ok()
        .map(|d| d.join("recently-moved.json"))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MovedDto {
    index: usize,
    path: String,
    name: String,
    bytes: u64,
    moved_at: i64,
    /// "inBin", "restored", "gone" (no longer in the bin) or "finder" (macOS).
    state: &'static str,
}

fn moved_list(app: &AppHandle) -> Vec<MovedDto> {
    let Some(file) = history_file(app) else {
        return Vec::new();
    };
    let items = tessera_cleanup::history::prune(tessera_cleanup::history::load(&file), unix_now());
    items
        .iter()
        .enumerate()
        .map(|(index, m)| MovedDto {
            index,
            path: m.original.to_string_lossy().into_owned(),
            name: m
                .original
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            bytes: m.bytes,
            moved_at: m.moved_at,
            state: if m.restored {
                "restored"
            } else {
                match &m.recycled {
                    Some(r) if std::fs::symlink_metadata(r).is_ok() => "inBin",
                    Some(_) => "gone",
                    None => "finder",
                }
            },
        })
        .collect()
}

/// Items Tessera moved in the last 30 days. Available without Pro: a person
/// must always be able to get their files back.
#[tauri::command(async)]
fn recently_moved(app: AppHandle) -> Vec<MovedDto> {
    moved_list(&app)
}

#[tauri::command]
async fn put_back_item(app: AppHandle, index: usize) -> CmdResult<Vec<MovedDto>> {
    let file = history_file(&app).ok_or("Tessera couldn't find its list of moved items.")?;
    let mut items =
        tessera_cleanup::history::prune(tessera_cleanup::history::load(&file), unix_now());
    let item = items
        .get(index)
        .cloned()
        .ok_or("That item is no longer in the list.")?;
    let recycled = item
        .recycled
        .clone()
        .ok_or("Open the Trash in the Finder and choose Put Back.")?;
    let original = item.original.clone();
    tauri::async_runtime::spawn_blocking(move || tessera_cleanup::put_back(&recycled, &original))
        .await
        .map_err(|_| "Putting it back stopped unexpectedly.".to_string())?
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => "Something with that name is already in its old place, so nothing was changed. Move or rename it, then try again.".to_string(),
            std::io::ErrorKind::NotFound => "It is no longer in the Recycle Bin (it may have been emptied or restored already).".to_string(),
            _ => format!("It couldn't be put back: {e}"),
        })?;
    if std::fs::symlink_metadata(&item.original).is_err() {
        return Err(
            "Windows reported success, but the item is not back in place. Check the Recycle Bin."
                .into(),
        );
    }
    items[index].restored = true;
    let _ = tessera_cleanup::history::save(&file, &items);
    Ok(moved_list(&app))
}

#[tauri::command(async)]
fn clear_moved_history(app: AppHandle) -> CmdResult<()> {
    let file = history_file(&app).ok_or("Tessera couldn't find its list of moved items.")?;
    tessera_cleanup::history::save(&file, &[]).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDto {
    id: String,
    name: String,
    publisher: Option<String>,
    version: Option<String>,
    install_date: Option<String>,
    /// Bytes measured in the current scan, when its folder was scanned.
    measured_bytes: Option<u64>,
    reported_bytes: Option<u64>,
    location: Option<String>,
    per_user: bool,
    can_uninstall: bool,
}

/// Installed apps (Windows). Sizes are measured from the current scan when
/// the app's folder is part of it; otherwise the app's own figure is shown.
#[tauri::command]
async fn list_apps(state: State<'_, AppState>, scan_id: Option<u64>) -> CmdResult<Vec<AppDto>> {
    let scan = scan_id.and_then(|id| active(&state, id).ok());
    tauri::async_runtime::spawn_blocking(move || {
        let tree = scan.as_ref().map(|s| read(s));
        tessera_apps::installed_apps()
            .into_iter()
            .map(|a| {
                let measured = match (&tree, &a.install_location) {
                    (Some(t), Some(loc)) => tessera_scan::query::find_by_path(t, loc)
                        .filter(|&id| t.node(id).kind == tessera_scan::NodeKind::Dir)
                        .map(|id| t.node(id).counted().1),
                    _ => None,
                };
                AppDto {
                    id: a.id,
                    name: a.name,
                    publisher: a.publisher,
                    version: a.version,
                    install_date: a.install_date,
                    measured_bytes: measured.or(a.measured_bytes),
                    reported_bytes: a.reported_bytes,
                    location: a.install_location.map(|p| p.to_string_lossy().into_owned()),
                    per_user: a.per_user,
                    can_uninstall: a.can_uninstall,
                }
            })
            .collect()
    })
    .await
    .map_err(|_| "Listing apps stopped unexpectedly.".to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UninstallDto {
    /// "opened" (its own uninstaller is open), "removed" (Store app) or
    /// "trashed" (Mac app moved to the Trash; can be put back).
    kind: &'static str,
    bundle_id: Option<String>,
    leftovers: Vec<(String, u64)>,
}

fn record_moved(app: &AppHandle, items: Vec<tessera_cleanup::history::Moved>) {
    if let Some(file) = history_file(app) {
        let _ = tessera_cleanup::history::append(&file, items, unix_now());
    }
}

/// Uninstalls after the person confirmed in Tessera: opens the app's own
/// uninstaller (Windows), removes a Store app, or moves a Mac app to the Trash.
#[tauri::command]
async fn uninstall_app(app: AppHandle, id: String) -> CmdResult<UninstallDto> {
    if !is_pro() {
        return Err("Uninstalling from Tessera is part of Tessera Pro. You can always uninstall the usual way for your system.".into());
    }
    if id.len() > 1024 {
        return Err("Unknown app.".into());
    }
    let outcome =
        tauri::async_runtime::spawn_blocking(move || tessera_apps::launch_uninstaller(&id))
            .await
            .map_err(|_| "Uninstalling stopped unexpectedly.".to_string())??;
    Ok(match outcome {
        tessera_apps::Outcome::OpenedUninstaller => UninstallDto {
            kind: "opened",
            bundle_id: None,
            leftovers: Vec::new(),
        },
        tessera_apps::Outcome::Removed => UninstallDto {
            kind: "removed",
            bundle_id: None,
            leftovers: Vec::new(),
        },
        tessera_apps::Outcome::MovedToTrash {
            original,
            recycled,
            bytes,
            leftovers,
            bundle_id,
        } => {
            record_moved(
                &app,
                vec![tessera_cleanup::history::Moved {
                    original,
                    recycled,
                    bytes,
                    moved_at: unix_now(),
                    restored: false,
                }],
            );
            UninstallDto {
                kind: "trashed",
                bundle_id,
                leftovers: leftovers
                    .into_iter()
                    .map(|(p, n)| (p.to_string_lossy().into_owned(), n))
                    .collect(),
            }
        }
    })
}

/// Moves a Mac app's leftover support files (found by bundle id in
/// ~/Library) to the Trash. Paths are recomputed here, never taken from the
/// interface. Each can be put back from Restore.
#[tauri::command]
async fn move_leftovers(app: AppHandle, bundle_id: String) -> CmdResult<(usize, u64)> {
    if !is_pro() {
        return Err("This is part of Tessera Pro.".into());
    }
    if !cfg!(target_os = "macos") || !tessera_apps::valid_bundle_id(&bundle_id) {
        return Err("Leftover files are only found for Mac apps.".into());
    }
    let library = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library"))
        .ok_or("Your home folder could not be found.")?;
    let moved = tauri::async_runtime::spawn_blocking(move || {
        let mut moved = Vec::new();
        for p in tessera_apps::leftover_paths(&library, &bundle_id) {
            if let Ok(recycled) = tessera_cleanup::move_to_trash(&p) {
                moved.push(tessera_cleanup::history::Moved {
                    original: p,
                    recycled,
                    bytes: 0,
                    moved_at: unix_now(),
                    restored: false,
                });
            }
        }
        moved
    })
    .await
    .map_err(|_| "Moving leftovers stopped unexpectedly.".to_string())?;
    let n = moved.len();
    record_moved(&app, moved);
    Ok((n, 0))
}
/// One CSV cell: quoted, and protected against spreadsheet formulas.
fn csv_cell(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Unix seconds to "YYYY-MM-DD HH:MM" (UTC).
fn utc_text(t: i64) -> String {
    if t == i64::MIN {
        return String::new();
    }
    let (days, secs) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        secs / 3600,
        secs % 3600 / 60
    )
}

/// The report's contents: one row per item, sizes in bytes.
fn report_csv(tree: &tessera_scan::Tree, ids: &[NodeId]) -> String {
    let mut out = String::from(
        "\u{feff}Path,Name,Type,Size on disk (bytes),File size (bytes),Modified (UTC)\r\n",
    );
    for &id in ids {
        let n = tree.node(id);
        let (logical, allocated) = n.counted();
        let kind = match n.kind {
            tessera_scan::NodeKind::Dir => "Folder",
            tessera_scan::NodeKind::File => "File",
            _ => "Other",
        };
        out.push_str(&format!(
            "{},{},{},{},{},{}\r\n",
            csv_cell(&tree.path_of(id).to_string_lossy()),
            csv_cell(&tree.display_name(id)),
            kind,
            allocated,
            logical,
            utc_text(n.modified)
        ));
    }
    out
}

/// Saves a CSV report (largest files, or a folder's contents) where the
/// person chooses. Pro. Returns the saved path, or None if cancelled.
#[tauri::command]
async fn export_report(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
    kind: String,
    under: Option<NodeId>,
    metric: MetricSetting,
) -> CmdResult<Option<String>> {
    if !is_pro() {
        return Err(
            "Reports are part of Tessera Pro. Enter your licence key in Settings to use them."
                .into(),
        );
    }
    let scan = active(&state, scan_id)?;
    let ids: Vec<NodeId> = {
        let tree = read(&scan);
        if let Some(u) = under {
            check_id(&tree, u)?;
        }
        match kind.as_str() {
            "largest" => top_files(&tree, under, 1000, metric.into()),
            "folder" => sorted_children(&tree, under.unwrap_or(0), metric.into()),
            _ => return Err("Unknown report.".into()),
        }
    };
    let name = if kind == "largest" {
        "Tessera largest files.csv"
    } else {
        "Tessera folder report.csv"
    };
    let Some(dest) = app
        .dialog()
        .file()
        .set_file_name(name)
        .add_filter("CSV", &["csv"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = dest
        .into_path()
        .map_err(|_| "That location can't be used.".to_string())?;
    let out = report_csv(&read(&scan), &ids);
    std::fs::write(&path, out).map_err(|e| format!("The report couldn't be saved: {e}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn csv_cells_are_quoted_and_formula_safe() {
        assert_eq!(csv_cell(r#"a "b""#), r#""a ""b""""#);
        assert_eq!(csv_cell("=HYPERLINK(1)"), "\"'=HYPERLINK(1)\"");
        assert_eq!(csv_cell("plain.txt"), "\"plain.txt\"");
    }

    #[test]
    fn report_rows_for_a_real_scan_are_formula_safe() {
        let dir = std::env::temp_dir().join(format!("tessera-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("=HYPERLINK(evil).txt"), b"x").unwrap();
        std::fs::write(dir.join("big.bin"), vec![1u8; 4096]).unwrap();
        let opts = tessera_scan::ScanOptions::default();
        let (tree, _) =
            tessera_scan::scan(&dir, &opts, &tessera_scan::CancelToken::new(), &mut |_| {})
                .unwrap();
        let ids = top_files(&tree, None, 10, tessera_scan::Metric::Logical);
        let csv = report_csv(&tree, &ids);
        std::fs::remove_dir_all(&dir).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3, "header plus two files: {csv}");
        assert!(lines[0].starts_with('\u{feff}'));
        assert!(
            lines[1].contains("\"big.bin\",File,") && lines[1].contains(",4096,"),
            "{csv}"
        );
        assert!(
            csv.contains("\"'=HYPERLINK(evil).txt\""),
            "formula neutralised: {csv}"
        );
    }

    #[test]
    fn dates_format_in_utc() {
        assert_eq!(utc_text(0), "1970-01-01 00:00");
        assert_eq!(utc_text(1_764_160_200), "2025-11-26 12:30");
        assert_eq!(utc_text(i64::MIN), "");
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FreeUpDto {
    freed: usize,
    freed_bytes: u64,
    skipped: Vec<(String, String)>,
}

/// Frees the local copy of cloud files (Pro). Each file is re-checked: only
/// real cloud files are changed; nothing is deleted from the cloud.
#[tauri::command]
async fn free_up_cloud(
    state: State<'_, AppState>,
    scan_id: u64,
    ids: Vec<NodeId>,
) -> CmdResult<FreeUpDto> {
    if !is_pro() {
        return Err("Freeing space from cloud files is part of Tessera Pro.".into());
    }
    if ids.len() > 10_000 {
        return Err("Too many files at once.".into());
    }
    let scan = active(&state, scan_id)?;
    let items: Vec<(String, PathBuf, u64)> = {
        let tree = read(&scan);
        ids.iter()
            .filter(|&&id| check_id(&tree, id).is_ok())
            .map(|&id| {
                (
                    tree.display_name(id).into_owned(),
                    tree.path_of(id),
                    tree.node(id).counted().1,
                )
            })
            .collect()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = FreeUpDto {
            freed: 0,
            freed_bytes: 0,
            skipped: Vec::new(),
        };
        for (name, path, bytes) in items {
            match tessera_cleanup::cloud::free_up(&path) {
                Ok(()) => {
                    out.freed += 1;
                    out.freed_bytes += bytes;
                }
                Err(e) => out.skipped.push((name, e.to_string())),
            }
        }
        out
    })
    .await
    .map_err(|_| "Freeing space stopped unexpectedly.".to_string())
}

struct PendingReview {
    id: u64,
    scan_id: u64,
    plan: tessera_cleanup::Plan,
}

#[tauri::command(async)]
fn review_cleanup(
    state: State<'_, AppState>,
    scan_id: u64,
    ids: Vec<NodeId>,
) -> CmdResult<ReviewDto> {
    if !is_pro() {
        return Err(
            "Cleanup is part of Tessera Pro. Enter your licence key in Settings to use it.".into(),
        );
    }
    if ids.len() > 100_000 {
        return Err("Too many items selected at once.".into());
    }
    let scan = active(&state, scan_id)?;
    if !matches!(
        *scan.outcome.lock().unwrap_or_else(|e| e.into_inner()),
        Outcome::Finished(_)
    ) {
        return Err("Wait for the scan to finish before cleaning up.".into());
    }
    let plan = {
        let tree = read(&scan);
        tessera_cleanup::build_plan(&tree, &ids, &tessera_cleanup::Protection::for_this_system())
    };
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let dto = ReviewDto::new(id, &plan);
    *state.review.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(PendingReview { id, scan_id, plan });
    Ok(dto)
}

#[tauri::command]
async fn confirm_cleanup(
    app: AppHandle,
    state: State<'_, AppState>,
    review_id: u64,
) -> CmdResult<CleanupReportDto> {
    if !is_pro() {
        return Err(
            "Cleanup is part of Tessera Pro. Enter your licence key in Settings to use it.".into(),
        );
    }
    // Taking the plan out of the state means a review can run only once.
    let pending = state
        .review
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let pending = match pending {
        Some(p) if p.id == review_id => p,
        _ => return Err("This review has expired. Review the items again.".into()),
    };
    let scan = active(&state, pending.scan_id)?;
    let root = scan.root.clone();
    let plan = pending.plan;
    let (report, before, after) = tauri::async_runtime::spawn_blocking(move || {
        let before = volume_space(&root).ok().map(|s| s.available);
        let protection = tessera_cleanup::Protection::for_this_system();
        let report = tessera_cleanup::execute(&plan, &protection, &mut |p| {
            tessera_cleanup::move_to_trash(p)
        });
        let after = volume_space(&root).ok().map(|s| s.available);
        (report, before, after)
    })
    .await
    .map_err(|_| "The cleanup stopped unexpectedly. Check the Recycle Bin before trying again.")?;
    // Remember what was moved so it can be put back from the Restore page.
    if let Some(file) = history_file(&app) {
        let now = unix_now();
        let _ = tessera_cleanup::history::append(
            &file,
            report
                .outcomes
                .iter()
                .filter(|o| o.result.is_ok())
                .map(|o| tessera_cleanup::history::Moved {
                    original: o.path.clone(),
                    recycled: o.recycled.clone(),
                    bytes: o.allocated,
                    moved_at: now,
                    restored: false,
                }),
            now,
        );
    }

    let moved: Vec<NodeId> = report
        .outcomes
        .iter()
        .filter(|o| o.result.is_ok())
        .map(|o| o.id)
        .collect();
    scan.tree
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .detach_many(&moved);
    Ok(CleanupReportDto {
        moved: report.moved as u32,
        failed: report.failed as u32,
        moved_logical: report.moved_logical,
        moved_allocated: report.moved_allocated,
        outcomes: report
            .outcomes
            .iter()
            .map(|o| OutcomeDto {
                id: o.id,
                path: o.path.to_string_lossy().into_owned(),
                logical: o.logical,
                allocated: o.allocated,
                ok: o.result.is_ok(),
                reason: o.result.as_ref().err().cloned(),
            })
            .collect(),
        free_before: before,
        free_after: after,
    })
}

fn existing_path(state: &AppState, scan_id: u64, id: NodeId) -> CmdResult<(PathBuf, NodeKind)> {
    let scan = active(state, scan_id)?;
    let tree = read(&scan);
    check_id(&tree, id)?;
    let path = tree.path_of(id);
    let kind = tree.node(id).kind;
    drop(tree);
    if std::fs::symlink_metadata(&path).is_err() {
        return Err("This item no longer exists at its scanned location.".into());
    }
    Ok((path, kind))
}

#[tauri::command(async)]
fn reveal_item(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
    id: NodeId,
) -> CmdResult<()> {
    let (path, _) = existing_path(&state, scan_id, id)?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|_| "The item could not be shown.".to_string())
}

#[tauri::command(async)]
fn open_item(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
    id: NodeId,
) -> CmdResult<()> {
    let (path, kind) = existing_path(&state, scan_id, id)?;
    safety::check_openable(&path, kind)?;
    let p = path
        .to_str()
        .ok_or("This item's name cannot be opened by other apps.")?;
    app.opener()
        .open_path(p, None::<&str>)
        .map_err(|_| "No app is set up to open this item.".to_string())
}

/// The window opens at 1280 x 820 points; on a small or highly scaled screen
/// (for example 1920 x 1080 at 200%, which leaves 960 x 540 points) that would
/// spill off the edges, so it is shrunk to fit the screen's free area.
fn fit_window_to_screen(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(m)) = w.current_monitor() else {
        return;
    };
    let scale = m.scale_factor();
    let area = m.work_area().size;
    let (free_w, free_h) = (area.width as f64 / scale, area.height as f64 / scale);
    let (want_w, want_h) = (1280.0, 820.0);
    if free_w < want_w || free_h < want_h {
        let size = tauri::LogicalSize::new(
            want_w.min(free_w * 0.96).max(800.0),
            want_h.min(free_h * 0.96).max(480.0),
        );
        let _ = w.set_size(size);
        let _ = w.center();
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        // A second launch brings the open window forward instead of starting
        // another copy that would scan alongside it. If it was given a folder
        // ("Scan with Tessera" while Tessera is open), the open window scans it.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            launch::focus(app);
            let cwd = PathBuf::from(cwd);
            if let Some(folder) =
                launch::folder_from_args(args.into_iter().map(std::ffi::OsString::from), Some(&cwd))
            {
                launch::scan_folder(app, &folder);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .manage(background::Background::default())
        .setup(|app| {
            diagnostics::init(app.handle());
            background::init(app.handle());
            fit_window_to_screen(app.handle());
            // A folder given on the command line (`tessera-desktop.exe "D:\Photos"`,
            // a shortcut, or the "Scan with Tessera" menu entry) is scanned at
            // launch. The interface picks the scan up when it loads.
            let folder = launch::folder_from_args(std::env::args_os(), None);
            // Development builds only: TESSERA_DEV_SCAN=<folder> does the same.
            #[cfg(debug_assertions)]
            let folder = folder.or_else(|| std::env::var_os("TESSERA_DEV_SCAN").map(PathBuf::from));
            if let Some(p) = folder
                && let Ok(root) = prepare_root(&p)
            {
                let state = app.state::<AppState>();
                let _ = begin_scan(app.handle().clone(), &state, root, ScanOptions::default());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // A folder or drive dropped onto the window is scanned.
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                launch::dropped(window.app_handle(), paths);
            }
            // Background mode: closing the window keeps Tessera in the
            // notification area or menu bar.
            tauri::WindowEvent::CloseRequested { api, .. }
                if background::keep_running_on_close(window.app_handle()) =>
            {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_volumes,
            start_scan,
            cancel_scan,
            scan_status,
            node_details,
            children,
            treemap,
            largest_files,
            search,
            smart_scan,
            find_duplicates,
            cancel_duplicates,
            recently_moved,
            put_back_item,
            clear_moved_history,
            list_apps,
            uninstall_app,
            move_leftovers,
            export_report,
            free_up_cloud,
            update::check_update,
            history::storage_history,
            history::clear_storage_history,
            update::open_download_page,
            review_cleanup,
            confirm_cleanup,
            license_status,
            refresh_license,
            activate_license,
            deactivate_license,
            reveal_item,
            open_item,
            diagnostics::log_error,
            diagnostics::show_log_file,
            background::background_settings,
            background::set_background_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error while starting Tessera");
    app.run(|_app, _event| {
        // macOS: a folder opened with Tessera from Finder ("Open With", or
        // dropped on the Dock icon). Info.plist declares that Tessera can
        // open folders and drives, as a viewer only.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = &_event
            && let Some(folder) = urls
                .iter()
                .filter_map(|u| u.to_file_path().ok())
                .find(|p| p.is_dir())
        {
            launch::focus(_app);
            launch::scan_folder(_app, &folder);
        }
        // macOS: clicking the Dock icon while the window is hidden (background
        // mode) brings it back.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = &_event
        {
            launch::focus(_app);
        }
    });
}
