# Status

Legend: **Not implemented** · **Implemented but unverified** · **Automatically
tested** · **Real platform tested** · **Blocked**

"Real platform tested" here means exercised on the owner's Windows 11 laptop
against real NTFS data. No macOS hardware has been used yet.

Last updated: 2026-11-26.

## Phase 1 — Foundation

| Item | Status | Evidence / notes |
|---|---|---|
| Dev environment (Rust 1.100.0 pinned, VS 2022 Build Tools, Node 24, Git, gh) | Done | Installed and used for every build below |
| Repository structure, workspace | Done | Private repo `hullashoo6/tessera` |
| CI (Windows + macOS runners) | Passing | GitHub Actions on `windows-latest` and `macos-latest` (macOS 26, Apple Silicon): fmt, clippy, all tests, fixture benchmark. Tests may not skip in CI |
| Filesystem data model | Automatically tested | Invariants checked in every test, including during live scans |
| Test fixtures (exact trees, junctions, sparse, ACL denial, generator) | Automatically tested | `crates/testkit` |
| Security requirements | Draft | SECURITY.md |

## Phase 2 — Scanning engine

| Feature | Status | Evidence / notes |
|---|---|---|
| Parallel bounded traversal | Real platform tested (Windows) | 18 tests; `C:\Windows` 242,858 files |
| Live, consistent incremental results | Automatically tested | `live_tree_is_consistent_while_scanning` |
| Progress, elapsed time, counts | Real platform tested (Windows) | CLI progress line |
| Cancellation | Automatically tested | ≤ 13 ms measured; partial tree consistent |
| Permission-denied handling | Real platform tested (Windows) | Test + 50 protected folders in `C:\Windows` |
| Unicode, long paths (>300 chars) | Automatically tested (Windows) | |
| Files changing during scan | Automatically tested (Windows) | Churn thread across 3 scans |
| Links / junctions not followed | Automatically tested (Windows junctions + symlinks in CI; macOS symlinks in CI) | Locally, Windows dir symlinks need Developer Mode |
| Hard-link de-duplication | Real platform tested (Windows) | `C:\Windows`: 45.16 → 34.52 GiB (Auto), 33.99 GiB (Thorough) |
| Sparse files (measured allocation) | Automatically tested (Windows) | 256 MiB sparse → tiny allocation |
| Compressed / WOF files (measured allocation) | Implemented but unverified | Code path uses GetCompressedFileSizeW; needs a test with a compressed file |
| Allocated-size estimate for ordinary NTFS files | Automatically tested | Labelled "estimated"; MFT-resident small files over-estimated |
| Cloud placeholders (OneDrive / iCloud) not downloaded | Implemented but unverified | Never opened by design; needs a test machine with OneDrive Files On-Demand / iCloud |
| Drive enumeration + OS capacity | Real platform tested (Windows) | `tessera-cli volumes` |
| macOS adapter (lstat, st_blocks, dataless, getmntinfo, mount boundaries) | Automatically tested (CI VM only) | 16 scanner tests pass on a macOS 26 arm64 runner, incl. permissions, symlinks, sparse, hard links. Not yet run on real Mac hardware; dataless (iCloud) and mount boundaries untested |
| External drive disconnect mid-scan | Not tested | Errors are recorded per folder by design |
| Single folder with millions of entries | Not tested | Whole listing held in memory per folder |
| Deterministic owner for hard-linked bytes | Not implemented | First path seen gets the bytes; may vary between scans |
| NTFS MFT fast path | Not implemented (deferred by design) | Only after the portable scanner is proven |
| Benchmarks 10k / 100k / 1M | Measured (Windows) | BENCHMARKS.md; 1M: 5.6 s warm, 183 MiB peak |
| Memory efficiency at 1M+ entries | Below target | 183 MiB vs ~100 MiB estimate; chunked storage planned |

## Phases 3–8

| Item | Status |
|---|---|
| Desktop app shell (Tauri 2 + React + strict TS), command allow-list, strict CSP | Real platform tested (Windows, dev build) |
| Overview: drives with OS capacity, home folder, folder picker | Real platform tested (Windows, dev build) |
| Interactive treemap (squarified, nested, aggregation, tile cap, canvas, high-DPI, keyboard, screen-reader announcements) | Layout automatically tested (8 tests); UI checked by screenshots on Windows |
| Details panel, synced contents list, breadcrumbs, context menu | Real platform tested (Windows, dev build) |
| Largest files view | Real platform tested (Windows, dev build) |
| Open refuses programs/scripts; reveal; copy path | Rules automatically tested (3 tests); reveal/open not yet clicked through on an installed build |
| Light/dark/system theme, size-on-disk vs file size | Real platform tested (Windows, dev build) |
| Search and filters (name, path, type groups, size, date, empty folders, scope, 4 sort orders) with map highlighting | Engine automatically tested (4 tests); UI checked on Windows with a 100k-file scan |
| Sunburst, tree, table views | Not implemented |
| UI with 1M-file scans | Not tested (scanner tested at 1M; UI at 100k) |
| Categorisation, developer storage view | Not implemented |
| Cleanup (review → Trash) | Not implemented |
| Storage history, reports | Not implemented |
| Licensing engine (Dodo public activate/validate/deactivate endpoints, test mode only, OS credential store, 30-day offline grace, configurable Pro features) | Automatically tested against a simulated Dodo server (7 tests). **Mock only**: no real Dodo test-mode key has been used yet |
| Licensing in the app (Settings > Tessera Pro: activate/deactivate; cleanup gated to Pro; background re-check) | Implemented; interface type-checked. Real Dodo **test server** reached: unknown key correctly refused (opt-in test). Real Dodo **test-mode** cycle verified from the app on Windows (dev build), 26 Nov 2026: owner activated a real test key (Pro shown, credential stored); "Check now" re-validated with Dodo (last confirmed 6:46 pm to 6:47 pm, Pro kept); deactivation returned the app to Free and removed the credential. Not yet repeated on an installed build or on macOS |
| Live checkout | **Blocked by design** until owner approval (`LIVE_MODE_APPROVED = false`). Live product `pdt_0Nsmg4vQiRrOCZ4yTeDEk` bound to live mode only; test product `pdt_0Nsmh1XPpX4NRvbkO8N7C` bound to test mode. See docs/LICENSING.md |
| Website (Astro 7, static): home, help, changelog, draft privacy/terms, 404, sitemap, robots, JSON-LD, OG image | Built and checked by headless-browser screenshots (desktop and narrow widths). Not deployed. Lighthouse not yet measured |
| Website downloads | Driven by `website/src/data/releases.json` (empty: no release exists, so no download links are shown) |
| Website pricing | Shows USD 14.99 marked "Draft price"; no checkout link. `pricing.approved` must be set by owner decision |
| Windows installer (NSIS, per-user, no admin) `Tessera_0.1.0_x64-setup.exe`, 3.0 MB | Real platform tested on Windows 11, 26 Nov 2026: silent install (exit 0, uninstall entry "Tessera 0.1.0 by Beyond Enterprises", Start menu shortcut), installed app launched and scanned a folder (1,400 files in 140 ms), silent uninstall (exit 0; files, entry and shortcut removed; licence and WebView settings intentionally kept). **Unsigned**: SmartScreen behaviour not yet tested (local build has no download mark). Cleanup and licensing not yet repeated on the installed build |
| Release manifest (`tools/release-manifest.ps1`) | Implemented: file, version, size, SHA-256, signed/verified flags; nothing published |
| Code signing (Windows Authenticode, Apple Developer ID) | **Blocked**: paid certificates need owner approval |
| macOS installer (DMG) | Not built: needs a Mac or CI release job |

## Owner decisions outstanding

* Final product name (Tessera is a working name; trademark search needed).
* Pricing (USD 14.99 is provisional; not published anywhere).
* Mac hardware for real macOS testing (CI alone is not acceptance).
* Code-signing certificates for Windows and Apple Developer Program (paid).
