//! Tessera desktop backend.
//!
//! The interface talks to this module through a small set of commands. It
//! never sends file paths: scans start from a detected drive, the home
//! folder, or the native folder picker, and every other request refers to
//! items by id within the current scan. Nothing here modifies files.

mod dto;
mod safety;

use dto::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tessera_scan::query::{largest_files as top_files, sorted_children};
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

#[tauri::command]
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
    let result = scan_shared(&scan.tree, &opts, &scan.cancel, &mut |p| {
        *scan.outcome.lock().unwrap_or_else(|e| e.into_inner()) = Outcome::Running(*p);
        let _ = app.emit("scan-progress", ProgressDto::new(id, p));
    });
    let outcome = match result {
        Ok(summary) => Outcome::Finished(summary),
        Err(e) => Outcome::Failed(format!("The scan stopped: {e}.")),
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

#[tauri::command]
fn scan_status(state: State<'_, AppState>) -> Option<StatusDto> {
    let scan = state
        .scan
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()?;
    let outcome = scan.outcome.lock().unwrap_or_else(|e| e.into_inner());
    let (phase, progress, summary, error) = match &*outcome {
        Outcome::Running(p) => ("scanning", ProgressDto::new(scan.id, p), None, None),
        Outcome::Finished(s) => (
            if s.cancelled { "cancelled" } else { "complete" },
            ProgressDto::new(scan.id, &s.progress),
            Some(SummaryDto::from(s)),
            None,
        ),
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

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|_app| {
            // Development builds only: TESSERA_DEV_SCAN=<folder> starts a scan
            // at launch so the UI can be checked against a synthetic fixture.
            #[cfg(debug_assertions)]
            if let Ok(p) = std::env::var("TESSERA_DEV_SCAN")
                && let Ok(root) = prepare_root(std::path::Path::new(&p))
            {
                let state = _app.state::<AppState>();
                let _ = begin_scan(_app.handle().clone(), &state, root, ScanOptions::default());
            }
            Ok(())
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
            reveal_item,
            open_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tessera");
}
