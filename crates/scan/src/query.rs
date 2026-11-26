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
    let mut v: Vec<NodeId> = tree.children(id).collect();
    v.sort_by(|&a, &b| {
        metric
            .of(tree, b)
            .cmp(&metric.of(tree, a))
            .then_with(|| tree.node(a).name.cmp(&tree.node(b).name))
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
        if n.kind != NodeKind::File || n.has(flags::COUNTED_ELSEWHERE) {
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
        cur = tree.children(cur).find(|&c| &*tree.node(c).name == name)?;
    }
    Some(cur)
}
