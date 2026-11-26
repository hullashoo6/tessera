# Testing

## Rules

* Automated tests never read or modify personal files. Every test creates its
  own temporary directory (`tessera-test-*` under the system temp folder) and
  works only inside it.
* Simulated behaviour and real operating-system behaviour are labelled
  separately in STATUS.md. A passing CI build is not product acceptance.

## Running

```powershell
cargo test --workspace            # all tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Scanner tests (`crates/scan/tests/scan.rs`)

All run against the real filesystem of the machine executing them.

| Test | What it proves |
|---|---|
| `empty_directory` | Empty roots produce zero totals and no issues |
| `exact_sizes_and_aggregation` | Byte-exact totals at every level; sort orders; path lookup |
| `allocated_size_is_reported_for_dense_files` | Allocation ≥ size; Windows estimate is cluster-aligned and labelled |
| `unicode_and_unusual_names_round_trip` | CJK, RTL, emoji, combining marks, spaces and symbols are stored exactly and resolve back to real paths |
| `long_paths_beyond_260_characters` | Paths over 300 characters scan without errors |
| `links_and_junctions_are_not_followed` | Junctions (Windows) and directory symlinks are reported as links; targets are not counted |
| `hard_links_are_counted_once_when_checked` | Hard-linked bytes counted once; `Off` mode counts per path |
| `small_hard_links_below_windows_threshold_are_reported_as_unchecked` (Windows) | Summary reports exactly which files were checked |
| `sparse_files_report_small_allocation` | 256 MiB sparse file reports tiny allocation (measured, not estimated) |
| `permission_denied_directories_are_reported_not_fatal` | Unreadable folder is flagged, recorded as an issue, and the rest is still counted |
| `generated_fixture_totals_match_exactly` | 5,000-file tree: file/folder counts and bytes match the generator exactly, with 1 and 4 threads |
| `cancellation_before_start_returns_immediately` | Pre-cancelled scan returns at once with a consistent partial tree |
| `cancellation_mid_scan_leaves_consistent_partial_tree` | Cancel during a 20,000-file scan returns within 1 s, tree consistent |
| `files_changing_during_scan_do_not_break_it` | A thread creates/deletes files during three scans; no crash, invariants hold |
| `live_tree_is_consistent_while_scanning` | A reader checks every invariant repeatedly while the scan writes |
| `invalid_roots_are_rejected` | Missing paths and files-as-roots return clear errors |
| `volumes_are_listed_with_os_reported_capacity` | Drive list and OS capacity figures are sane |

Unit test: `platform::windows::tests::verbatim_forms` (long-path prefixes).

### Platform caveats

* Windows directory **symbolic links** need Developer Mode or elevation; when
  unavailable the symlink half of the link test is skipped with a note (the
  junction half always runs). macOS CI covers symlinks.
* The permission test uses `icacls` on Windows and `chmod 000` on macOS; it
  skips (with a note) when running as root.

`scan_target_disappearing_mid_scan_finishes_cleanly`: the scanned folder is
deleted while the scan runs; the scan ends normally with a consistent tree.

## Other automated checks

| Where | What |
|---|---|
| `crates/insights/tests/rules.rs` | Every Smart Scan rule fires only with its evidence; Windows and Mac locations are both checked on every system; large old files skip app data, hidden folders and app libraries |
| `apps/desktop/src-tauri` unit tests | Launch arguments (drive quoting, trailing backslash, relative paths); diagnostics log never contains paths; background-mode thresholds and settings; update version comparison |
| `apps/desktop/src/__tests__` (Vitest) | Screen behaviour with test data: Cleanup, Smart Scan, Duplicates, Restore, Apps, licence, History, error safety net, Welcome, Background setting, Report; storage-map label contrast for every colour |
| `tessera-cli soak --max-peak-growth-mib 8` | Memory does not accumulate over 10 scans (CI and `tools/check.ps1`) |
| `tessera-cli ui-bench --check` | Every request the screens make answers within its time budget |
| `tools/release.ps1` | Installed build: right-click entries, window appears and stays responsive, local log, closing behaviour |

Run all of them on the laptop with `powershell -File tools\check.ps1`.

## Manual checks per installer (cannot be automated)

Use disposable files only (a folder under `.fixtures\` with a
`.tessera-fixture` marker), never personal files.

Both systems:
1. Install; open Tessera; the welcome screen appears on a fresh account.
2. Right-click a disposable folder: File Explorer > Show more options > Scan
   with Tessera (Windows), or Finder > Open With > Tessera (Mac). Repeat with
   Tessera already open.
3. Drag a disposable folder onto the window; then drag a single file (it
   should say to drop a folder).
4. Smart Scan, Duplicates, Largest files, Search on the disposable folder.
5. Cleanup (Pro, test licence key): move two disposable files to the Recycle
   Bin / Trash, Put back one from Restore, check contents are identical.
6. Storage map > Report > Print or save as PDF: save a PDF and open it.
7. Export CSV through the save dialog and open the file.
8. Settings > Background: turn on keep-running; close the window; the icon
   is in the notification area / menu bar with free space; Open, Scan main
   drive and Quit work. The first close shows a one-time notification. Turn on
   "Start Tessera there when I sign in", sign out and in: Tessera is in the
   notification area / menu bar with no window. Turn both off again.
9. Settings > Diagnostics > Show log file: the log opens; it contains no file
   or folder names.
10. Uninstall; on Windows the right-click entries are gone.

Mac only: the one-time Privacy & Security "Open Anyway" step from the DMG
window; Dock-icon drop of a folder; iCloud Drive free-up; app uninstall with
leftovers.

## Not yet covered (tracked in STATUS.md)

External-drive disconnection mid-scan (the folder-disappears test covers the
scanner side), mount-boundary behaviour on macOS, APFS clones, cloud
placeholder files end to end (needs OneDrive or iCloud syncing on a test
machine), directories with millions of entries in a single folder, and
everything on real Mac hardware.
