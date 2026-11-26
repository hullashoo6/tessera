// Only the commands listed here can be called from the interface, and only
// if a capability file grants them (see capabilities/default.json).
const COMMANDS: &[&str] = &[
    "app_info",
    "list_volumes",
    "start_scan",
    "cancel_scan",
    "scan_status",
    "node_details",
    "children",
    "treemap",
    "largest_files",
    "search",
    "smart_scan",
    "find_duplicates",
    "cancel_duplicates",
    "recently_moved",
    "put_back_item",
    "clear_moved_history",
    "list_apps",
    "uninstall_app",
    "move_leftovers",
    "export_report",
    "free_up_cloud",
    "check_update",
    "storage_history",
    "clear_storage_history",
    "open_download_page",
    "review_cleanup",
    "confirm_cleanup",
    "license_status",
    "refresh_license",
    "activate_license",
    "deactivate_license",
    "reveal_item",
    "open_item",
    "log_error",
    "show_log_file",
    "background_settings",
    "set_background_settings",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
