//! In-memory representation of a scanned directory tree.
//!
//! The tree is an arena: every node lives in one `Vec`, identified by a
//! `NodeId` index. Children of a directory are stored contiguously because a
//! directory's whole listing is inserted at once. Parents always have smaller
//! ids than their children, so iterating ids in reverse visits children
//! before parents (used for bottom-up passes).
//!
//! Size semantics:
//! * For files, `logical`/`allocated` are the file's own sizes.
//! * For directories, they are the totals of everything counted beneath them.
//! * Nodes flagged `COUNTED_ELSEWHERE` (extra hard links, revisited
//!   directories) keep their own sizes for display but contribute nothing to
//!   their ancestors' totals, so bytes are never counted twice.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub type NodeId = u32;
pub const NO_NODE: NodeId = u32::MAX;
pub const ROOT: NodeId = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NodeKind {
    Dir = 0,
    File = 1,
    /// Symbolic link, Windows junction or volume mount point. Never followed.
    Link = 2,
    /// Device, socket, FIFO or anything that is not a regular file or folder.
    Other = 3,
}

/// Bit flags stored on each node.
pub mod flags {
    /// Directory discovered but not yet read (scan running, or cancelled).
    pub const PENDING: u16 = 1 << 0;
    /// Directory could not be opened (usually permission denied).
    pub const UNREADABLE: u16 = 1 << 1;
    /// Directory listing stopped early because of an error.
    pub const PARTIAL: u16 = 1 << 2;
    /// Extra hard link, or a directory already visited; excluded from totals.
    pub const COUNTED_ELSEWHERE: u16 = 1 << 3;
    /// File has more than one hard link; this occurrence carries the bytes.
    pub const HARD_LINKED: u16 = 1 << 4;
    /// Cloud placeholder: contents are not stored on this disk.
    pub const CLOUD_ONLY: u16 = 1 << 5;
    pub const SPARSE: u16 = 1 << 6;
    pub const COMPRESSED: u16 = 1 << 7;
    /// Directory is on a different filesystem; not traversed.
    pub const OTHER_FILESYSTEM: u16 = 1 << 8;
    /// Allocated size is an estimate, not a filesystem-reported value.
    pub const ALLOC_ESTIMATED: u16 = 1 << 9;
    /// Hidden or system attribute (Windows).
    pub const HIDDEN_OR_SYSTEM: u16 = 1 << 10;
    /// Metadata for this entry could not be read; sizes are unknown (zero).
    pub const METADATA_UNAVAILABLE: u16 = 1 << 11;
    /// Set by `Tree::finalize` on directories with incomplete descendants.
    pub const SUBTREE_INCOMPLETE: u16 = 1 << 12;
    /// Set by `Tree::finalize` on directories containing estimated sizes.
    pub const SUBTREE_ESTIMATED: u16 = 1 << 13;
    /// Set by `Tree::finalize` on directories containing cloud placeholders.
    pub const SUBTREE_CLOUD: u16 = 1 << 14;
    /// Moved to the Trash by Tessera after the scan; excluded everywhere.
    pub const REMOVED: u16 = 1 << 15;

    pub(crate) const INCOMPLETE_MASK: u16 =
        PENDING | UNREADABLE | PARTIAL | METADATA_UNAVAILABLE | SUBTREE_INCOMPLETE;
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: Box<OsStr>,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub child_count: u32,
    pub kind: NodeKind,
    pub flags: u16,
    pub logical: u64,
    pub allocated: u64,
    /// Number of file entries beneath this directory (1 for a file itself
    /// is *not* stored; files report 0).
    pub files: u32,
    /// Number of directories beneath this directory.
    pub dirs: u32,
    /// Last modification time, seconds since the Unix epoch, or `i64::MIN`.
    pub modified: i64,
}

impl Node {
    #[inline]
    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    #[inline]
    pub fn is_dir(&self) -> bool {
        self.kind == NodeKind::Dir
    }

    /// Bytes this node contributes to its parent's totals.
    #[inline]
    pub fn counted(&self) -> (u64, u64) {
        if self.has(flags::COUNTED_ELSEWHERE) {
            (0, 0)
        } else {
            (self.logical, self.allocated)
        }
    }

    /// The name as UTF-8 for display. Invalid sequences become U+FFFD; use
    /// `Tree::path_of` for an exact path.
    pub fn display_name(&self) -> std::borrow::Cow<'_, str> {
        self.name.to_string_lossy()
    }
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub(crate) nodes: Vec<Node>,
    root_path: PathBuf,
}

impl Tree {
    /// Creates a tree containing only the (pending) root directory.
    pub fn new(root_path: PathBuf) -> Self {
        let root = Node {
            name: root_path.as_os_str().into(),
            parent: NO_NODE,
            first_child: NO_NODE,
            child_count: 0,
            kind: NodeKind::Dir,
            flags: flags::PENDING,
            logical: 0,
            allocated: 0,
            files: 0,
            dirs: 0,
            modified: i64::MIN,
        };
        Tree {
            nodes: vec![root],
            root_path,
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    pub fn root(&self) -> &Node {
        &self.nodes[ROOT as usize]
    }

    #[inline]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Ids of the direct children of `id` (empty for files and pending dirs).
    pub fn children(&self, id: NodeId) -> std::ops::Range<NodeId> {
        let n = self.node(id);
        if n.first_child == NO_NODE {
            0..0
        } else {
            n.first_child..n.first_child + n.child_count
        }
    }

    /// Ancestors from `id`'s parent up to the root.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let mut cur = self.node(id).parent;
        std::iter::from_fn(move || {
            if cur == NO_NODE {
                None
            } else {
                let out = cur;
                cur = self.node(cur).parent;
                Some(out)
            }
        })
    }

    /// Exact (lossless) filesystem path of a node.
    pub fn path_of(&self, id: NodeId) -> PathBuf {
        let mut chain: Vec<NodeId> = Vec::new();
        let mut cur = id;
        while cur != ROOT && cur != NO_NODE {
            chain.push(cur);
            cur = self.node(cur).parent;
        }
        let mut path = self.root_path.clone();
        for &n in chain.iter().rev() {
            path.push(&*self.node(n).name);
        }
        path
    }

    /// Number of path components below the root (root = 0).
    pub fn depth(&self, id: NodeId) -> usize {
        self.ancestors(id).count()
    }

    /// Removes a node's contribution from all ancestors after it has been
    /// moved to the Trash, and marks it so views and queries skip it.
    pub fn detach(&mut self, id: NodeId) {
        self.detach_many(&[id]);
    }

    /// `detach` for several nodes, with one pass to mark their contents.
    pub fn detach_many(&mut self, ids: &[NodeId]) {
        let mut first = u32::MAX;
        for &id in ids {
            if self.detach_one(id) {
                first = first.min(id);
            }
        }
        // Parents precede children, so one forward pass marks every
        // descendant of a removed folder.
        for i in (first as usize).saturating_add(1)..self.nodes.len() {
            let parent = self.nodes[i].parent;
            if parent != NO_NODE && self.nodes[parent as usize].flags & flags::REMOVED != 0 {
                self.nodes[i].flags |= flags::REMOVED;
            }
        }
    }

    fn detach_one(&mut self, id: NodeId) -> bool {
        if id == ROOT || (id as usize) >= self.nodes.len() || self.node(id).has(flags::REMOVED) {
            return false;
        }
        let n = self.node(id);
        let (l, a) = n.counted();
        let (f, d) = match n.kind {
            NodeKind::File => (1, 0),
            NodeKind::Dir => (n.files, n.dirs + 1),
            _ => (0, 0),
        };
        let mut cur = n.parent;
        while cur != NO_NODE {
            let p = &mut self.nodes[cur as usize];
            p.logical = p.logical.saturating_sub(l);
            p.allocated = p.allocated.saturating_sub(a);
            p.files = p.files.saturating_sub(f);
            p.dirs = p.dirs.saturating_sub(d);
            cur = p.parent;
        }
        self.nodes[id as usize].flags |= flags::REMOVED | flags::COUNTED_ELSEWHERE;
        true
    }

    /// Bottom-up pass that sets the SUBTREE_* summary flags.
    pub fn finalize(&mut self) {
        for i in (1..self.nodes.len()).rev() {
            let (parent, f) = {
                let n = &self.nodes[i];
                (n.parent, n.flags)
            };
            let mut up = 0u16;
            if f & flags::INCOMPLETE_MASK != 0 {
                up |= flags::SUBTREE_INCOMPLETE;
            }
            if f & (flags::ALLOC_ESTIMATED | flags::SUBTREE_ESTIMATED) != 0 {
                up |= flags::SUBTREE_ESTIMATED;
            }
            if f & (flags::CLOUD_ONLY | flags::SUBTREE_CLOUD) != 0 {
                up |= flags::SUBTREE_CLOUD;
            }
            if up != 0 && parent != NO_NODE {
                self.nodes[parent as usize].flags |= up;
            }
        }
        let root = &mut self.nodes[ROOT as usize];
        if root.flags & (flags::PENDING | flags::UNREADABLE | flags::PARTIAL) != 0 {
            root.flags |= flags::SUBTREE_INCOMPLETE;
        }
    }

    /// Verifies the aggregation invariants. Intended for tests and debug
    /// builds; returns a description of the first violation found.
    pub fn check_invariants(&self) -> Result<(), String> {
        for (i, n) in self.nodes.iter().enumerate() {
            let id = i as NodeId;
            if n.has(flags::REMOVED) {
                continue;
            }
            if n.kind != NodeKind::Dir {
                if n.first_child != NO_NODE {
                    return Err(format!("non-directory node {id} has children"));
                }
                continue;
            }
            let (mut l, mut a, mut f, mut d) = (0u64, 0u64, 0u64, 0u64);
            for c in self.children(id) {
                let cn = self.node(c);
                if cn.parent != id {
                    return Err(format!("node {c} lists parent {} not {id}", cn.parent));
                }
                if c <= id {
                    return Err(format!("child {c} does not follow parent {id}"));
                }
                if cn.has(flags::REMOVED) {
                    continue;
                }
                let (cl, ca) = cn.counted();
                l += cl;
                a += ca;
                match cn.kind {
                    NodeKind::File => f += 1,
                    NodeKind::Dir => {
                        f += cn.files as u64;
                        d += 1 + cn.dirs as u64;
                    }
                    _ => {}
                }
            }
            if l != n.logical || a != n.allocated || f != n.files as u64 || d != n.dirs as u64 {
                return Err(format!(
                    "directory {id} totals (logical {}, allocated {}, files {}, dirs {}) != \
                     sum of children ({l}, {a}, {f}, {d})",
                    n.logical, n.allocated, n.files, n.dirs
                ));
            }
        }
        Ok(())
    }
}
