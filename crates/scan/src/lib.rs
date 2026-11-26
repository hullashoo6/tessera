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

/// Lowers the calling thread's priority so the interface stays responsive
/// while background work (scanning, hashing) runs. Work still uses all idle
/// CPU; it just yields to the foreground first.
pub fn background_priority() {
    #[cfg(windows)]
    // SAFETY: pseudo-handle for the current thread; no resources to release.
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
    #[cfg(target_os = "macos")]
    // SAFETY: documented call that only changes this thread's QoS class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
}

/// Restores normal priority for the calling thread (used when a scan is
/// cancelled so its workers stop promptly even on a busy computer).
pub fn normal_priority() {
    #[cfg(windows)]
    // SAFETY: pseudo-handle for the current thread; no resources to release.
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_NORMAL,
        };
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);
    }
}
