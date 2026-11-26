//! Tessera scanning engine.
//!
//! A read-only, parallel filesystem scanner that builds an in-memory size
//! tree. This crate never modifies, opens for writing, or deletes anything.
//! Cleanup lives in a separate crate with its own safety checks.

#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

pub mod model;
pub mod platform;
pub mod query;
pub mod scanner;

pub use model::{NO_NODE, Node, NodeId, NodeKind, ROOT, Tree, flags};
pub use platform::{VolumeInfo, VolumeKind, VolumeSpace, list_volumes, volume_space};
pub use query::Metric;
pub use scanner::{
    CancelToken, HardLinkCoverage, HardLinkMode, IssueKind, IssueStage, Progress, ScanError,
    ScanIssue, ScanOptions, ScanSummary, prepare_root, scan, scan_shared,
};
