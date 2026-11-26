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

## Not yet covered (tracked in STATUS.md)

External-drive disconnection mid-scan, mount-boundary behaviour on macOS,
APFS clones, cloud placeholder files (needs a real OneDrive/iCloud account on
a test machine), and directories with millions of entries in a single folder.
