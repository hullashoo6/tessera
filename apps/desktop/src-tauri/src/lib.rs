//! Tessera desktop backend.
//!
//! The interface talks to this module through a small set of commands. It
//! never sends file paths: scans start from a detected drive, the home
//! folder, or the native folder picker, and every other request refers to
//! items by id within the current scan. Nothing here modifies files.

mod dto;
mod licence;
mod safety;

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

#[tauri::command]
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
#[tauri::command]
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
    let found = tessera_insights::analyse(&tree, &tessera_insights::Places::for_this_user(), now);
    Ok(found
        .into_iter()
        .take(2000)
        .map(|s| SuggestionDto {
            id: s.id,
            group: match s.group {
                Group::Junk => "junk",
                Group::Developer => "developer",
                Group::Downloads => "downloads",
            },
            label: s.label,
            safety: match s.safety {
                Safety::Regenerable => "regenerable",
                Safety::Redownloadable => "redownloadable",
                Safety::Review => "review",
                Safety::UseAppTool => "useAppTool",
            },
            safety_text: tessera_insights::safety_text(s.safety),
            reason: s.reason,
            bytes: s.bytes,
            name: tree.node(s.id).display_name().into_owned(),
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
#[tauri::command]
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

#[tauri::command]
fn clear_moved_history(app: AppHandle) -> CmdResult<()> {
    let file = history_file(&app).ok_or("Tessera couldn't find its list of moved items.")?;
    tessera_cleanup::history::save(&file, &[]).map_err(|e| e.to_string())
}

struct PendingReview {
    id: u64,
    scan_id: u64,
    plan: tessera_cleanup::Plan,
}

#[tauri::command]
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
        .setup(|app| {
            // A folder given on the command line (`tessera-desktop.exe "D:\Photos"`,
            // a shortcut, or a future "Scan with Tessera" menu entry) is scanned
            // at launch. Only an existing folder is accepted; anything else is
            // ignored. The person launching the app chose it, as with the picker.
            let folder = std::env::args_os()
                .nth(1)
                .map(PathBuf::from)
                .filter(|p| p.is_dir());
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
            review_cleanup,
            confirm_cleanup,
            license_status,
            refresh_license,
            activate_license,
            deactivate_license,
            reveal_item,
            open_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tessera");
}
