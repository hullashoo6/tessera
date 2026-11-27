//! Data exchanged with the interface. Field names are camelCase in JSON.

use serde::{Deserialize, Serialize};
use tessera_scan::{
    HardLinkCoverage, IssueKind, Metric, NodeId, NodeKind, Progress, ROOT, ScanSummary, Tree,
    VolumeInfo, VolumeKind, VolumeSpace, flags,
};
use tessera_treemap::{Tile, TileKind};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub os: &'static str,
    pub version: &'static str,
    /// Installed from the Microsoft Store, which handles updates.
    pub store_managed: bool,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScanTarget {
    Volume {
        mount: String,
    },
    Home,
    Folder,
    /// The location of the current scan, again (for example after Undo).
    Again,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum HardLinkSetting {
    Auto,
    Thorough,
    Off,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSettings {
    pub hard_links: HardLinkSetting,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum MetricSetting {
    Allocated,
    Logical,
}

impl From<MetricSetting> for Metric {
    fn from(m: MetricSetting) -> Metric {
        match m {
            MetricSetting::Allocated => Metric::Allocated,
            MetricSetting::Logical => Metric::Logical,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceDto {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub used: u64,
}

impl From<VolumeSpace> for SpaceDto {
    fn from(s: VolumeSpace) -> Self {
        SpaceDto {
            total: s.total,
            available: s.available,
            free: s.free,
            used: s.used(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDto {
    pub mount: String,
    pub label: Option<String>,
    pub file_system: Option<String>,
    pub kind: &'static str,
    pub space: Option<SpaceDto>,
    /// The drive the operating system runs from (offered first to new users).
    pub system: bool,
}

/// `C:\` (or wherever Windows is installed) on Windows; `/` on macOS.
fn is_system_volume(mount: &str) -> bool {
    #[cfg(windows)]
    {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        mount
            .trim_end_matches('\\')
            .eq_ignore_ascii_case(drive.trim_end_matches('\\'))
    }
    #[cfg(not(windows))]
    {
        mount == "/"
    }
}

impl From<VolumeInfo> for VolumeDto {
    fn from(v: VolumeInfo) -> Self {
        let mount = v.mount_point.to_string_lossy().into_owned();
        VolumeDto {
            system: is_system_volume(&mount),
            mount,
            label: v.label,
            file_system: v.file_system,
            kind: match v.kind {
                VolumeKind::Fixed => "fixed",
                VolumeKind::Removable => "removable",
                VolumeKind::Network => "network",
                VolumeKind::Optical => "optical",
                VolumeKind::RamDisk => "ramdisk",
                VolumeKind::Unknown => "unknown",
            },
            space: v.space.map(SpaceDto::from),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    pub scan_id: u64,
    pub files: u64,
    pub dirs: u64,
    pub logical: u64,
    pub allocated: u64,
    pub issues: u64,
    pub pending: u64,
    pub elapsed_ms: u64,
}

impl ProgressDto {
    pub fn new(scan_id: u64, p: &Progress) -> Self {
        ProgressDto {
            scan_id,
            files: p.files,
            dirs: p.dirs,
            logical: p.logical_bytes,
            allocated: p.allocated_bytes,
            issues: p.issues,
            pending: p.dirs_pending,
            elapsed_ms: p.elapsed.as_millis() as u64,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDto {
    pub cancelled: bool,
    pub elapsed_ms: u64,
    pub issues: u64,
    pub permission_denied: u64,
    /// "all", "partial" or "none".
    pub hard_links: &'static str,
    pub hard_link_min_size: Option<u64>,
    pub internal_errors: u32,
}

impl From<&ScanSummary> for SummaryDto {
    fn from(s: &ScanSummary) -> Self {
        let (hard_links, min) = match s.hard_links {
            HardLinkCoverage::AllFiles => ("all", None),
            HardLinkCoverage::FilesAtLeast(n) => ("partial", Some(n)),
            HardLinkCoverage::NotChecked => ("none", None),
        };
        SummaryDto {
            cancelled: s.cancelled,
            elapsed_ms: s.elapsed.as_millis() as u64,
            issues: s.progress.issues,
            permission_denied: s
                .issues
                .iter()
                .filter(|i| i.kind == IssueKind::PermissionDenied)
                .count() as u64,
            hard_links,
            hard_link_min_size: min,
            internal_errors: s.internal_errors,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    pub scan_id: u64,
    pub root: String,
    /// "scanning", "complete", "cancelled" or "failed".
    pub phase: &'static str,
    pub progress: ProgressDto,
    pub summary: Option<SummaryDto>,
    pub error: Option<String>,
    pub volume: Option<SpaceDto>,
}

fn kind_name(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Dir => "dir",
        NodeKind::File => "file",
        NodeKind::Link => "link",
        NodeKind::Other => "other",
    }
}

const FLAG_NAMES: &[(u16, &str)] = &[
    (flags::PENDING, "pending"),
    (flags::UNREADABLE, "unreadable"),
    (flags::PARTIAL, "partial"),
    (flags::COUNTED_ELSEWHERE, "countedElsewhere"),
    (flags::HARD_LINKED, "hardLinked"),
    (flags::CLOUD_ONLY, "cloudOnly"),
    (flags::SPARSE, "sparse"),
    (flags::COMPRESSED, "compressed"),
    (flags::OTHER_FILESYSTEM, "otherFilesystem"),
    (flags::ALLOC_ESTIMATED, "estimated"),
    (flags::HIDDEN_OR_SYSTEM, "hidden"),
    (flags::METADATA_UNAVAILABLE, "metadataUnavailable"),
    (flags::SUBTREE_INCOMPLETE, "incomplete"),
    (flags::SUBTREE_ESTIMATED, "containsEstimates"),
    (flags::SUBTREE_CLOUD, "containsCloudFiles"),
];

fn display_name(tree: &Tree, id: NodeId) -> String {
    if id == ROOT {
        tree.root_path().to_string_lossy().into_owned()
    } else {
        tree.display_name(id).into_owned()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDto {
    pub id: NodeId,
    pub name: String,
    pub kind: &'static str,
    pub logical: u64,
    pub allocated: u64,
    pub files: u32,
    pub dirs: u32,
    pub child_count: u32,
    pub modified: Option<i64>,
    pub flags: Vec<&'static str>,
}

impl NodeDto {
    pub fn new(tree: &Tree, id: NodeId) -> Self {
        let n = tree.node(id);
        NodeDto {
            id,
            name: display_name(tree, id),
            kind: kind_name(n.kind),
            logical: n.logical,
            allocated: n.allocated,
            files: n.files,
            dirs: n.dirs,
            child_count: n.child_count,
            modified: (n.modified != i64::MIN).then_some(n.modified),
            flags: FLAG_NAMES
                .iter()
                .filter(|(f, _)| n.has(*f))
                .map(|(_, s)| *s)
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub struct CrumbDto {
    pub id: NodeId,
    pub name: String,
}

impl CrumbDto {
    pub fn new(tree: &Tree, id: NodeId) -> Self {
        CrumbDto {
            id,
            name: display_name(tree, id),
        }
    }
}

#[derive(Serialize)]
pub struct DetailsDto {
    pub node: NodeDto,
    pub path: String,
    pub crumbs: Vec<CrumbDto>,
}

#[derive(Serialize)]
pub struct PageDto {
    pub total: u32,
    pub items: Vec<NodeDto>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SortSetting {
    Size,
    Name,
    Modified,
    Type,
}

/// Search criteria from the interface; validated by `SearchQuery::to_filter`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub match_path: bool,
    pub extensions: Vec<String>,
    pub files: bool,
    pub folders: bool,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub modified_before: Option<i64>,
    pub modified_after: Option<i64>,
    pub empty_folders: bool,
    pub under: Option<NodeId>,
}

impl SearchQuery {
    pub fn to_filter(&self) -> Result<tessera_scan::query::Filter, String> {
        if self.text.chars().count() > 200 || self.extensions.len() > 64 {
            return Err("That search is too long.".into());
        }
        let mut extensions = Vec::with_capacity(self.extensions.len());
        for e in &self.extensions {
            let e = e.trim_start_matches('.').to_ascii_lowercase();
            if e.is_empty() || e.len() > 16 || !e.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err("A file type in the search is not valid.".into());
            }
            extensions.push(e);
        }
        let text = self.text.trim().to_string();
        let text = (!text.is_empty()).then_some(text);
        Ok(tessera_scan::query::Filter {
            name: if self.match_path { None } else { text.clone() },
            path: if self.match_path { text } else { None },
            extensions,
            files: self.files,
            folders: self.folders,
            min_size: self.min_size,
            max_size: self.max_size,
            modified_before: self.modified_before,
            modified_after: self.modified_after,
            empty_folders: self.empty_folders,
            under: self.under,
        })
    }
}

#[derive(Serialize)]
pub struct SearchResultDto {
    pub total: u32,
    pub items: Vec<FileRowDto>,
    /// Folders that contain at least one listed match (for map highlighting).
    pub containing: Vec<NodeId>,
}

#[derive(Serialize)]
pub struct FileRowDto {
    pub node: NodeDto,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileDto {
    pub id: NodeId,
    /// "file", "dir", "link", "other" or "more" (aggregate of small items).
    pub kind: &'static str,
    pub depth: u16,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub size: u64,
    pub count: u32,
    pub branch: u32,
    pub opened: bool,
    pub pending: bool,
    pub name: String,
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl TileDto {
    pub fn new(tree: &Tree, t: &Tile) -> Self {
        let (kind, name, pending) = match t.kind {
            TileKind::Aggregate => ("more", String::new(), false),
            _ => {
                let n = tree.node(t.id);
                (
                    kind_name(n.kind),
                    tree.display_name(t.id).into_owned(),
                    n.has(flags::PENDING),
                )
            }
        };
        TileDto {
            id: t.id,
            kind,
            depth: t.depth,
            x: r2(t.rect.x),
            y: r2(t.rect.y),
            w: r2(t.rect.w),
            h: r2(t.rect.h),
            size: t.weight,
            count: t.count,
            branch: t.branch,
            opened: t.opened,
            pending,
            name,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItemDto {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub logical: u64,
    pub allocated: u64,
    pub warning: Option<String>,
}

#[derive(Serialize)]
pub struct RejectedDto {
    /// The chosen item (review only), so it can be taken off the list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<NodeId>,
    pub path: String,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDto {
    pub review_id: u64,
    pub items: Vec<ReviewItemDto>,
    pub rejected: Vec<RejectedDto>,
    pub merged: u32,
    pub total_logical: u64,
    pub total_allocated: u64,
}

impl ReviewDto {
    pub fn new(review_id: u64, plan: &tessera_cleanup::Plan) -> Self {
        ReviewDto {
            review_id,
            items: plan
                .items
                .iter()
                .map(|c| ReviewItemDto {
                    id: c.id,
                    name: c
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    path: c.path.to_string_lossy().into_owned(),
                    kind: kind_name(c.kind),
                    logical: c.logical,
                    allocated: c.allocated,
                    warning: c.warning.clone(),
                })
                .collect(),
            rejected: plan
                .rejected
                .iter()
                .map(|r| RejectedDto {
                    id: Some(r.id),
                    path: r.path.to_string_lossy().into_owned(),
                    reason: r.reason.clone(),
                })
                .collect(),
            merged: plan.merged as u32,
            total_logical: plan.total_logical,
            total_allocated: plan.total_allocated,
        }
    }
}

/// Why an item was left in place, for the interface to explain.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FailureDto {
    /// inUse, permission, tooLarge, changed, protected, pathTooLong,
    /// cloudOffline, locked, other
    pub kind: &'static str,
    /// The system's own wording (shown in small print).
    pub detail: String,
    /// Apps holding it open, when Windows can tell.
    pub apps: Vec<String>,
    /// For a folder: a few of the files inside it that are in use.
    pub in_use_files: Vec<String>,
}

impl From<&tessera_cleanup::Failure> for FailureDto {
    fn from(f: &tessera_cleanup::Failure) -> Self {
        use tessera_cleanup::FailKind as K;
        FailureDto {
            kind: match f.kind {
                K::InUse => "inUse",
                K::Permission => "permission",
                K::TooLargeForBin => "tooLarge",
                K::Changed => "changed",
                K::Protected => "protected",
                K::PathTooLong => "pathTooLong",
                K::CloudOffline => "cloudOffline",
                K::Locked => "locked",
                K::Other => "other",
            },
            detail: f.detail.clone(),
            apps: f.apps.clone(),
            in_use_files: f
                .in_use_files
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedDto {
    pub id: NodeId,
    pub path: String,
    pub logical: u64,
    pub allocated: u64,
    pub failure: FailureDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReportDto {
    /// Identifies this cleanup for Undo.
    pub run_id: u64,
    /// "trash" or "permanent".
    pub method: &'static str,
    pub moved: u32,
    pub deleted: u32,
    /// Already gone before Tessera got to them (nothing to do).
    pub gone: u32,
    pub failed: u32,
    pub removed_logical: u64,
    pub removed_allocated: u64,
    /// Stop was pressed; `not_started` items were left untouched.
    pub stopped: bool,
    pub not_started: u32,
    /// Items no longer in the list: moved, deleted or already gone.
    pub done_ids: Vec<NodeId>,
    pub failures: Vec<FailedDto>,
    /// Free space reported by the OS before and after.
    pub free_before: Option<u64>,
    pub free_after: Option<u64>,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CleanupProgressDto {
    pub done: u32,
    pub total: u32,
    pub bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoDto {
    pub restored: u32,
    pub failed: Vec<RejectedDto>,
}

/// "Move everything except the files in use" for one folder.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialDto {
    /// False when the folder was too big to check in time (nothing moved).
    pub checked: bool,
    pub removed: u32,
    pub removed_logical: u64,
    pub removed_allocated: u64,
    pub failed: u32,
    pub in_use_count: u32,
    pub in_use_files: Vec<String>,
    pub apps: Vec<String>,
}
