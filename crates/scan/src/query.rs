//! Read-only queries over a scanned tree.

use crate::model::{NodeId, NodeKind, Tree, flags};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Which size a view is measuring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Metric {
    /// Bytes of content ("size").
    Logical,
    /// Bytes reserved on disk ("size on disk"); may be an estimate.
    #[default]
    Allocated,
}

impl Metric {
    #[inline]
    pub fn of(self, tree: &Tree, id: NodeId) -> u64 {
        let (l, a) = tree.node(id).counted();
        match self {
            Metric::Logical => l,
            Metric::Allocated => a,
        }
    }
}

/// Children of `id`, largest first; ties broken by name for stable output.
pub fn sorted_children(tree: &Tree, id: NodeId, metric: Metric) -> Vec<NodeId> {
    let mut v: Vec<NodeId> = tree
        .children(id)
        .filter(|&c| !tree.node(c).has(flags::REMOVED))
        .collect();
    v.sort_by(|&a, &b| {
        metric
            .of(tree, b)
            .cmp(&metric.of(tree, a))
            .then_with(|| tree.name(a).cmp(tree.name(b)))
    });
    v
}

/// The `limit` largest files beneath `under` (or the whole tree), largest
/// first. Extra hard links are skipped so a file is never listed twice.
pub fn largest_files(
    tree: &Tree,
    under: Option<NodeId>,
    limit: usize,
    metric: Metric,
) -> Vec<NodeId> {
    if limit == 0 {
        return Vec::new();
    }
    let range = match under {
        None => 0..tree.len() as NodeId,
        Some(id) => subtree_range(tree, id),
    };
    let mut heap: BinaryHeap<Reverse<(u64, NodeId)>> = BinaryHeap::with_capacity(limit + 1);
    for id in range {
        let n = tree.node(id);
        if n.kind != NodeKind::File || n.flags & (flags::COUNTED_ELSEWHERE | flags::REMOVED) != 0 {
            continue;
        }
        if under.is_some_and(|u| u != id && !is_descendant(tree, id, u)) {
            continue;
        }
        let size = metric.of(tree, id);
        if heap.len() < limit {
            heap.push(Reverse((size, id)));
        } else if heap.peek().is_some_and(|Reverse((s, _))| size > *s) {
            heap.pop();
            heap.push(Reverse((size, id)));
        }
    }
    let mut out: Vec<(u64, NodeId)> = heap.into_iter().map(|Reverse(x)| x).collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, id)| id).collect()
}

/// Whether `id` lies beneath `ancestor`.
pub fn is_descendant(tree: &Tree, id: NodeId, ancestor: NodeId) -> bool {
    id > ancestor && tree.ancestors(id).any(|a| a == ancestor)
}

/// Node ids at or after `id` that can belong to its subtree. Ids of a
/// subtree are not contiguous (directories are read in parallel), but they
/// are all greater than `id`.
fn subtree_range(tree: &Tree, id: NodeId) -> std::ops::Range<NodeId> {
    id..tree.len() as NodeId
}

/// Looks up a node by its path relative to the root, component by component.
pub fn find_by_path(tree: &Tree, path: &std::path::Path) -> Option<NodeId> {
    let rel = path.strip_prefix(tree.root_path()).ok()?;
    let mut cur = crate::model::ROOT;
    for comp in rel.components() {
        let name = comp.as_os_str();
        cur = tree.children(cur).find(|&c| tree.name(c) == name)?;
    }
    Some(cur)
}

/// Criteria for `search`. Empty criteria match everything of the chosen kinds.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// Case-insensitive substring of the item's name.
    pub name: Option<String>,
    /// Case-insensitive substring of the full path (slower; checked last).
    pub path: Option<String>,
    /// Lower-case extensions without the dot; files only.
    pub extensions: Vec<String>,
    pub files: bool,
    pub folders: bool,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    /// Modified strictly before / at or after these Unix times.
    pub modified_before: Option<i64>,
    pub modified_after: Option<i64>,
    /// Only folders that were read and contain nothing.
    pub empty_folders: bool,
    /// Restrict to the subtree below this node.
    pub under: Option<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortKey {
    /// Largest first.
    #[default]
    Size,
    /// A to Z, ignoring case.
    Name,
    /// Oldest first.
    Modified,
    /// By extension, then largest first.
    Type,
}

#[derive(Clone, Debug, Default)]
pub struct SearchHits {
    /// Matching ids in sorted order, at most `limit` of them.
    pub ids: Vec<NodeId>,
    /// Number of matches before truncation.
    pub total: usize,
}

fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then(|| ext.to_lowercase())
}

/// Linear scan over the tree applying `filter`. Extra hard links are
/// skipped so a file's bytes are never listed twice.
pub fn search(
    tree: &Tree,
    filter: &Filter,
    metric: Metric,
    sort: SortKey,
    limit: usize,
) -> SearchHits {
    let name_q = filter
        .name
        .as_ref()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty());
    let path_q = filter
        .path
        .as_ref()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty());
    let mut hits: Vec<NodeId> = Vec::new();
    for id in 1..tree.len() as NodeId {
        let n = tree.node(id);
        if n.flags & (flags::COUNTED_ELSEWHERE | flags::REMOVED) != 0 {
            continue;
        }
        let is_file = n.kind == NodeKind::File;
        let is_dir = n.kind == NodeKind::Dir;
        if filter.empty_folders {
            let unread =
                flags::PENDING | flags::UNREADABLE | flags::OTHER_FILESYSTEM | flags::PARTIAL;
            if !is_dir || n.child_count != 0 || n.flags & unread != 0 {
                continue;
            }
        } else if !((is_file && filter.files) || (is_dir && filter.folders)) {
            continue;
        }
        let size = metric.of(tree, id);
        if filter.min_size.is_some_and(|m| size < m) || filter.max_size.is_some_and(|m| size > m) {
            continue;
        }
        if filter
            .modified_before
            .is_some_and(|t| n.modified == i64::MIN || n.modified >= t)
            || filter
                .modified_after
                .is_some_and(|t| n.modified == i64::MIN || n.modified < t)
        {
            continue;
        }
        if name_q.is_some() || !filter.extensions.is_empty() {
            let name = tree.name(id).to_string_lossy();
            if !filter.extensions.is_empty() {
                let ok =
                    is_file && extension(&name).is_some_and(|e| filter.extensions.contains(&e));
                if !ok {
                    continue;
                }
            }
            if let Some(q) = &name_q
                && !name.to_lowercase().contains(q.as_str())
            {
                continue;
            }
        }
        if let Some(u) = filter.under
            && !is_descendant(tree, id, u)
        {
            continue;
        }
        if let Some(q) = &path_q
            && !tree
                .path_of(id)
                .to_string_lossy()
                .to_lowercase()
                .contains(q.as_str())
        {
            continue;
        }
        hits.push(id);
    }

    let total = hits.len();
    let lower = |id: NodeId| tree.name(id).to_string_lossy().to_lowercase();
    match sort {
        SortKey::Size => {
            hits.sort_by(|&a, &b| metric.of(tree, b).cmp(&metric.of(tree, a)).then(a.cmp(&b)))
        }
        SortKey::Name => hits.sort_by_cached_key(|&id| (lower(id), id)),
        SortKey::Modified => hits.sort_by_key(|&id| (tree.node(id).modified, id)),
        SortKey::Type => hits.sort_by_cached_key(|&id| {
            let ext = extension(&tree.name(id).to_string_lossy()).unwrap_or_default();
            (ext, std::cmp::Reverse(metric.of(tree, id)), id)
        }),
    }
    hits.truncate(limit);
    SearchHits { ids: hits, total }
}
