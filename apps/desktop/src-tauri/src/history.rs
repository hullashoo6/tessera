//! Storage history: a small summary of each finished scan (total size, the
//! largest top-level items and the largest folders at any depth), kept on
//! this computer so the person can see how a drive or folder changes over
//! time and which folders grew. Viewing is a Pro feature; recording and
//! clearing are always available. Never leaves the computer.

use crate::licence::is_pro;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tessera_scan::{Metric, NodeId, NodeKind, ROOT, Tree, query::sorted_children};

const PER_ROOT: usize = 60;
const TOTAL: usize = 600;
const TOP: usize = 50;
/// Largest folders kept per snapshot, at any depth (about 15 KB).
const DEEP: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub root: String,
    /// Unix seconds when the scan finished.
    pub at: i64,
    pub allocated: u64,
    pub logical: u64,
    pub files: u64,
    pub folders: u64,
    /// Largest items directly inside the scanned location, by size on disk.
    pub top: Vec<(String, u64)>,
    /// The largest folders anywhere below the scanned location (path relative
    /// to it, size on disk), largest first. Empty in snapshots made before
    /// this was recorded.
    #[serde(default)]
    pub deep: Vec<(String, u64)>,
    /// Every folder missing from `deep` was smaller than this, so a folder
    /// absent from one snapshot is known to have been below it.
    #[serde(default)]
    pub deep_floor: u64,
}

/// The `DEEP` largest folders below the root, with paths relative to it.
fn largest_folders(tree: &Tree) -> (Vec<(String, u64)>, u64) {
    let mut dirs: Vec<(NodeId, u64)> = (1..tree.len() as NodeId)
        .filter(|&id| tree.node(id).kind == NodeKind::Dir)
        .map(|id| (id, tree.node(id).counted().1))
        .filter(|&(_, bytes)| bytes > 0)
        .collect();
    dirs.sort_by_key(|&(_, bytes)| std::cmp::Reverse(bytes));
    let floor = if dirs.len() > DEEP { dirs[DEEP].1 } else { 0 };
    dirs.truncate(DEEP);
    let root = tree.root_path().to_path_buf();
    let list = dirs
        .into_iter()
        .filter_map(|(id, bytes)| {
            let full = tree.path_of(id);
            let rel = full
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .into_owned();
            (!rel.is_empty()).then_some((rel, bytes))
        })
        .collect();
    (list, floor)
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_local_data_dir()
        .ok()
        .map(|d| d.join("storage-history.json"))
}

pub fn load(path: &Path) -> Vec<Snapshot> {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(path: &Path, items: &[Snapshot]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(items).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(tmp, path)
}

/// Summary of a finished scan.
pub fn snapshot(tree: &Tree, at: i64) -> Snapshot {
    let root = tree.root();
    let top = sorted_children(tree, ROOT, Metric::Allocated)
        .into_iter()
        .take(TOP)
        .map(|id| {
            (
                tree.display_name(id).into_owned(),
                tree.node(id).counted().1,
            )
        })
        .collect();
    let (deep, deep_floor) = largest_folders(tree);
    Snapshot {
        root: tree.root_path().to_string_lossy().into_owned(),
        at,
        allocated: root.allocated,
        logical: root.logical,
        files: u64::from(root.files),
        folders: u64::from(root.dirs),
        top,
        deep,
        deep_floor,
    }
}

/// Newest first, at most `PER_ROOT` per location and `TOTAL` overall.
pub fn prune(mut items: Vec<Snapshot>) -> Vec<Snapshot> {
    items.sort_by_key(|s| std::cmp::Reverse(s.at));
    let mut per: std::collections::HashMap<String, usize> = Default::default();
    items.retain(|s| {
        let n = per.entry(s.root.to_lowercase()).or_default();
        *n += 1;
        *n <= PER_ROOT
    });
    items.truncate(TOTAL);
    items
}

/// Records a finished scan. Failures are ignored: history is a convenience.
pub fn record(app: &AppHandle, tree: &Tree, at: i64) {
    if let Some(path) = file(app) {
        let mut items = load(&path);
        items.push(snapshot(tree, at));
        let _ = save(&path, &prune(items));
    }
}

#[tauri::command(async)]
pub fn storage_history(app: AppHandle) -> Result<Vec<Snapshot>, String> {
    if !is_pro() {
        return Err("Storage history is part of Tessera Pro.".into());
    }
    Ok(file(&app).map(|p| prune(load(&p))).unwrap_or_default())
}

#[tauri::command(async)]
pub fn clear_storage_history(app: AppHandle) -> Result<(), String> {
    let path = file(&app).ok_or("Tessera couldn't find its history file.")?;
    save(&path, &[]).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(root: &str, at: i64) -> Snapshot {
        Snapshot {
            root: root.into(),
            at,
            allocated: 1,
            logical: 1,
            files: 0,
            folders: 0,
            top: vec![],
            deep: vec![],
            deep_floor: 0,
        }
    }

    #[test]
    fn old_snapshots_without_deep_folders_still_load() {
        let old = r#"[{"root":"C:\\","at":1,"allocated":5,"logical":5,"files":1,"folders":1,"top":[["Users",5]]}]"#;
        let items: Vec<Snapshot> = serde_json::from_str(old).unwrap();
        assert!(items[0].deep.is_empty());
        assert_eq!(items[0].deep_floor, 0);
    }

    #[test]
    fn keeps_newest_per_location_and_overall() {
        let mut items: Vec<Snapshot> = (0..70).map(|i| snap("C:\\", i)).collect();
        items.push(snap("D:\\", 5));
        let kept = prune(items);
        assert_eq!(kept.iter().filter(|s| s.root == "C:\\").count(), PER_ROOT);
        assert_eq!(kept.first().unwrap().at, 69, "newest first");
        assert!(kept.iter().any(|s| s.root == "D:\\"));
    }

    #[test]
    fn snapshot_of_a_real_scan_lists_top_folders_by_size() {
        let dir = std::env::temp_dir().join(format!("tessera-history-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("big")).unwrap();
        std::fs::create_dir_all(dir.join("small")).unwrap();
        std::fs::write(dir.join("big/a.bin"), vec![1u8; 300_000]).unwrap();
        std::fs::write(dir.join("small/b.bin"), vec![1u8; 10_000]).unwrap();
        let (tree, _) = tessera_scan::scan(
            &dir,
            &Default::default(),
            &tessera_scan::CancelToken::new(),
            &mut |_| {},
        )
        .unwrap();
        let s = snapshot(&tree, 42);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(s.at, 42);
        assert_eq!(s.files, 2);
        assert_eq!(s.top.first().map(|t| t.0.as_str()), Some("big"));
        assert!(s.allocated >= 310_000);
        // Deep list: folders only, relative paths, largest first, all kept (floor 0).
        let names: Vec<&str> = s.deep.iter().map(|d| d.0.as_str()).collect();
        assert_eq!(names, vec!["big", "small"]);
        assert_eq!(s.deep_floor, 0);
    }
}
