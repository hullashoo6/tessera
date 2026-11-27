# Status

Legend: **Not implemented** · **Implemented but unverified** · **Automatically
tested** · **Real platform tested** · **Blocked**

"Real platform tested" here means exercised on the owner's Windows 11 laptop
against real NTFS data. No macOS hardware has been used yet.

Last updated: 2026-11-27.

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
| Benchmarks 10k / 100k / 1M | Measured (Windows) | BENCHMARKS.md; 1M after the memory work (26 Nov): 6.0 s median, 144 MiB peak, cancel 3 ms |
| Memory efficiency at 1M+ entries | Held memory meets target; peak above it | ~75 MiB held after a 1M scan, steady over rescans (target < 100 MiB). Peak 144 MiB during the scan, 216 MiB while rescanning with the old result still shown |

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
| UI with 1M-file scans | **Real platform tested (Windows, dev build), 27 Nov**: every request the screens make answered in 0.3-225 ms on 1,073,748 items (BENCHMARKS.md); the window never took over 34 ms to respond during the scan or while switching through every screen (1,110 checks). macOS not measured; JavaScript frame times not measured |
| Small screens and high display scaling (minimum window 800 x 480; window fits the screen at start; narrow layout) | Checked on Windows in the dev app at 800 x 480 points: map, details, tables and status bar fit without overlap. 200% scaling itself not tried (would change the owner's display settings) |
| Smart Scan (tessera-insights): evidence-based junk, developer and large-download suggestions; app screen with reasons, safety labels, add-to-cleanup | Rules automatically tested (positive and negative evidence); screen checked on Windows with a disposable project folder. Seeing suggestions is free; cleanup stays Pro |
| Smart Scan additions, 27 Nov: Mac locations for every rule that had only Windows ones (browser caches, pip, Yarn, crash reports); Firefox; Android emulator images and virtual devices; iOS simulators, Xcode archives and device support; Unity and Unreal caches (project evidence required); previous Windows installation and Windows Update downloads (pointed to Windows' own clean-up); macOS installer apps; installers in Downloads older than a month; new "Backups and recordings" group (iPhone/iPad backups, screen recordings) and "Large old files" group (256 MB+, unchanged for a year, own folders only, app libraries such as the Photos library skipped); none of the personal groups is ever bulk-added | Automatically tested: both systems' locations checked on every system (the rules read only the scanned tree), with evidence present and absent. Not yet seen in the app against real data on either system |
| Background mode (Settings > Background, off by default): stay in the notification area (Windows) or menu bar (Mac) after closing, free space on the main drive, Open / Scan main drive / Quit; optional start at sign-in (autostart plugin, starts hidden; only with keep-running); optional low-space warning at most once a day | Windows dev build, 27 Nov: setting saved; with it on, closing hid the window and Tessera kept running; launching again showed the same window (one process); with it off, closing quit. Start at sign-in: turning it on wrote the Run entry with `--background`; a start with that flag kept Tessera's window hidden (only its helper windows exist) and opening Tessera again showed the same window; the owner's settings were put back to off and the entry removed afterwards. Uninstall removes the entry (not during updates); not yet tried with a real sign-in. Threshold and settings parsing unit-tested. **Not yet seen:** the tray icon and its menu (can't be automated without reading other apps' tray icons), the notifications (drive not low; dev builds show notifications under PowerShell's name), anything on a Mac |
| Cleanup (review → Recycle Bin/Trash, protection rules, re-check before each move; Pro). **Rebuilt 27 Nov evening** (docs/CLEANUP.md): cleanup list for thousands of items (Select all, filter, sort, take off the list, keyboard); one confirmation; **Delete permanently** as a separately warned choice (owner-approved); batched moves (~4x faster) with progress and Stop; results grouped by plain cause with the fix (which app holds a file open, via Windows' Restart Manager; too large for the bin; permission; changed; cloud app not running; locked; path too long), Try again, "Move the rest" for folders with open files, Show in File Explorer/Finder, **Undo** for the whole cleanup, Open Recycle Bin/Trash; already-gone items count as done; Delete key (Cmd+Delete on Mac) adds to cleanup; Search adds all results | **Real platform tested on Windows (dev build), 27 Nov, disposable fixture**: 29 files moved while 2 items held open by another program were explained as "In use by Windows PowerShell 5.1"; "Move the rest" moved 41 items from the folder and left only the open file; after the program closed, the 2 moved and Undo put both back (on disk); Stop after the first 25 of 300 left 275 untouched and listed; Delete permanently on a filtered 11 removed them with nothing in the Recycle Bin and free space up 44 KB; 264 items moved in 4.7 s; Restore lists the deleted items. Engine: 16 Rust tests incl. a file held open by a child process and a real Recycle Bin batch; 14 screen tests. Before: installed build moved disposable files and duplicates, Put back identical (26-27 Nov). **macOS: compiles (engine check); Trash moves per item; "which app" is Windows-only (macOS lets open files move); permission/locked/Full Disk Access wording untested on a real Mac** |
| Duplicates (tessera-dupes): size, partial hash, full BLAKE3; screen with keep-oldest/newest and every-copy warning | Engine automatically tested (incl. same-size same-edges different-middle files, hard links, changed files, cancel); screen checked on Windows with a disposable folder |
| Restore: "Recently moved" list with Put back (never overwrites; not Pro-gated) | **Real platform tested on Windows**, 26 Nov 2026: real Recycle Bin round trip test (incl. refusal when the original name is taken) and end to end in the app (cleanup, then Put back: identical SHA-256, nothing left in the bin). macOS: implemented (Trash location from NSFileManager, put back with RENAME_EXCL); real Trash round trip runs in CI on the macOS runner |
| Apps (Windows): installed apps from the Settings registry entries, measured or app-reported size, Uninstall opens the app's own uninstaller (Pro) | Listing tested read-only on the owner laptop; uninstall launch verified end to end with a temporary harmless test entry (removed afterwards). Real apps, 26 Nov 2026: Microsoft Store app (Simple Spider Solitaire) listed and removed through Tessera (confirmed gone via Windows' package list); KeyStow's real uninstaller was opened by Tessera, but its dialog can't be driven by automation and it was not completed. Store app and Mac app sizes are measured from their folders (since 27 Nov; to be checked on more machines). macOS: implemented (apps in /Applications and ~/Applications, moved to the Trash and recorded for Put back, leftovers in ~/Library by bundle id only); automated test on the macOS CI runner; not run on a real Mac |
| Reports: CSV export of Largest files and of the current folder (Pro), formula-safe cells | Report contents automatically tested on a real scan; save-dialog cancel verified in the app. Saving through the dialog not automated (Windows dialog not scriptable) - needs one manual check |
| Printable report / save as PDF (Pro): Storage map > Report shows a one-page summary of the open folder (totals, largest items, largest files, Smart Scan totals inside that folder); "Print or save as PDF" opens the system print dialog | Automatically tested (Pro lock; contents; only suggestions inside the folder counted). Windows dev build, 27 Nov: report shown; print preview showed only the report (2 pages, app controls hidden) with Microsoft Print to PDF offered; cancelled. Nothing was printed or saved. **macOS: print dialog untested** |
| Cloud "free up space" engine (Windows Cloud Files/OneDrive; macOS iCloud Drive) | Windows, 26 Nov 2026, real OneDrive: placeholder detection verified (Windows hides placeholders from apps by default; Tessera exposes them per thread only during the check) and the Explorer-equivalent 'free up space' mark applied correctly. **Not yet verified end to end**: OneDrive on the laptop was not processing changes (a new test file never uploaded), so the local copy was not removed within 60 s. The owner's file was restored exactly. macOS: compiles; untested (no iCloud on CI) |
| Storage history (summary of each finished scan kept locally; History page with growth chart and biggest folder changes; Pro). Since 27 Nov each snapshot also keeps the 200 largest folders at any depth and the size below which folders were not kept, so History names the most specific folders that changed and says "at least" when a folder was below the other scan's floor | Automatically tested: pruning per location, a real-scan snapshot including the deep list, old snapshots still load (Rust tests); Pro lock, growth comparison and chart labels (screen tests); the folder-change logic (5 tests: most specific folder, spread-out growth, "at least", new folders, old snapshots fall back to top-level). Not yet clicked through on an installed build, so the website still lists it as "in development" |
| Update check (Settings > Updates, on by default, can be turned off; reads the website's latest.json; banner opens the download page; nothing downloaded automatically) | Real platform tested (Windows installed build) for the "no update" case only: no false banner. The "update available" banner has not been seen on an installed build |
| First-run welcome screen (what Tessera does, privacy promise, one-click scan of the main drive or home folder; Settings > Welcome screen shows it again) | Automatically tested (2 screen tests: main drive offered first, Recycle Bin/Trash wording, home-folder fallback, skip). Checked on Windows in the dev app, 27 Nov: shown on first launch, "Choose another drive or folder" goes to the usual Overview |
| Scan from outside the window: right-click "Scan with Tessera" (Windows: folders, inside a folder, drives; per-user, removed on uninstall), "Open With > Tessera" and Dock-icon drop (macOS), folder dropped on the window, second launch hands its folder to the open window | **Real platform tested on Windows**, 27 Nov, local installer build: install added all three menu entries; the menu's command run exactly as File Explorer runs it opened Tessera on a disposable folder; with Tessera open, a second launch made the open window scan the new folder (one process); trailing-backslash/drive quoting repaired (5 unit tests); uninstall removed all three entries. Not yet clicked in File Explorer itself (Explorer's menu can't be automated); drag-and-drop onto the window not tested (can't be automated). **macOS: compiles and passes CI; untested on a real Mac.** Microsoft Store (MSIX) builds would need a different mechanism for the menu entry (not done) |
| High contrast (Windows contrast themes, macOS Increase contrast) | Storage map label contrast automatically tested for every colour (at least 4.5:1 normally, 7:1 in high contrast). Checked on Windows, 27 Nov, by emulating the settings in the dev app's web view (`tools/cdp-shot.mjs`, the computer's settings untouched): "increase contrast" gives darker text, stronger lines and the high-contrast map; a forced-colours (contrast theme) emulation outlines the current page and pressed buttons and borders every button; list colour squares and size bars kept. Dark mode checked the same way. A real Windows contrast theme and the Mac setting not tried |
| Licensing engine (Dodo public activate/validate/deactivate endpoints, test mode only, OS credential store, 30-day offline grace, configurable Pro features) | Automatically tested against a simulated Dodo server (7 tests). A real Dodo test-mode key has since been used from the app (next row) |
| Licensing in the app (Settings > Tessera Pro: activate/deactivate; cleanup gated to Pro; background re-check) | Implemented; interface type-checked. Real Dodo **test server** reached: unknown key correctly refused (opt-in test). Real Dodo **test-mode** cycle verified from the app on Windows (dev build), 26 Nov 2026: owner activated a real test key (Pro shown, credential stored); "Check now" re-validated with Dodo (last confirmed 6:46 pm to 6:47 pm, Pro kept); deactivation returned the app to Free and removed the credential. Not yet repeated on an installed build or on macOS |
| Live checkout | **Blocked by design** until owner approval (`LIVE_MODE_APPROVED = false`). Live product `pdt_0Nsmg4vQiRrOCZ4yTeDEk` bound to live mode only; test product `pdt_0Nsmh1XPpX4NRvbkO8N7C` bound to test mode. See docs/LICENSING.md |
| Website (Astro 7, static): home, help, changelog, draft privacy/terms, 404, sitemap, robots, JSON-LD, OG image | Live at https://tessera-96w.pages.dev since 26 Nov 2026; Lighthouse 98-100 (see Website below) |
| Website downloads | Driven by `website/src/data/releases.json` (empty: no release exists, so no download links are shown) |
| Website pricing | USD 9.78 one-time (set by the owner), shown as not yet on sale; no checkout link anywhere while `pricing.onSale` is false |
| Windows installer (NSIS, per-user, no admin) `Tessera_0.1.0_x64-setup.exe`, 3.0 MB | Real platform tested on Windows 11, 26 Nov 2026: silent install (exit 0, uninstall entry "Tessera 0.1.0 by Beyond Enterprises", Start menu shortcut), installed app launched and scanned a folder (1,400 files in 140 ms), silent uninstall (exit 0; files, entry and shortcut removed; licence and WebView settings intentionally kept). **Unsigned**: SmartScreen behaviour not yet tested (local build has no download mark). Installed build, second pass (same day): launched with a disposable folder as argument; licence persisted across reinstall, "Check now" re-validated with Dodo test mode; cleanup moved a 4 MB disposable file to the Recycle Bin (confirmed there), other files untouched |
| Release manifest (`tools/release-manifest.ps1`) | Implemented: file, version, size, SHA-256, signed/verified flags; nothing published |
| Code signing (Windows Authenticode, Apple Developer ID) | **Blocked**: paid certificates need owner approval. The owner declined Apple's USD 99/year for now; Windows is to be signed free through the Microsoft Store (next rows) |
| macOS installer `Tessera_0.1.0_universal.dmg` (Apple silicon + Intel), 9.1 MB | Built by the manual release workflow (private artifact, sha256 7ec7b9098ed781ce94beb8f3cc72673cd698a12c7900688ba97c82db776082f5). Installer window shows the one-time "Open Anyway" steps. Ad-hoc signed, **not notarised**; **never run on a real Mac** (Intel or Apple silicon) |
| Free Windows signing via Microsoft Store (MSIX) | docs/STORE.md, docs/STORE-LISTING.md, tools/make-msix.ps1 (ships the licence credits, takes the version from the app, real identity as defaults). 27 Nov evening: Partner Center product reserved as MSIX; package built with the **real identity** and a provisional display name, accepted by makeappx; not yet installed (needs Developer Mode) or submitted (needs the exact reserved name and the owner's OK). "Start at sign-in" is refused and shown as unavailable in Store copies (packaged apps' registry writes are private). Store copies skip the website update check (Store rules) and Settings says so. Right-click "Scan with Tessera" is not available in a Store package (needs a different mechanism). Needs the owner's free Partner Center account |

## Owner decisions outstanding

Decided: the name Tessera (approved); price USD 9.78 one-time, tax included, one device, no expiry.

Decided 27 Nov evening: website republish approved; **English only, no other languages** (owner, 19:49 IST; docs/LOCALISATION.md); Delete permanently allowed as a warned second choice; Microsoft Store route **MSIX** (free signing and Store updates; EXE/MSI would need a paid certificate) and the Store name "Tessera - Disk Space Analyzer and Cleaner" (plain "Tessera" is taken in the Store; the app itself stays "Tessera"); Developer Mode on the laptop for Store test installs; **live sales on and Store submission approved** (20:46 IST).

Still outstanding:
* GitHub Actions minutes are used up for this month (all CI runs since 27 Nov ~02:00 IST fail before starting; the Mac installer can only be built there). Advised to keep the repository private (making it public can be undone, but copies made meanwhile can't) and wait for the monthly reset; CI now runs macOS only weekly, on request, or with [mac] in a commit message.
* Why OneDrive on the laptop is not syncing (blocks the cloud free-up test).
* Mac hardware for real macOS testing (CI alone is not acceptance).
* Live sales are on (27 Nov): the owner should make one real purchase with their 100%-off code and activate the key in the installed app, so the whole buying path is proven.
* Microsoft Store submission: the helper uploads `target\store-submission` in Partner Center (choose "Publish manually").
* Trademark search for "Tessera" and a lawyer review of the privacy policy and terms.
* Apple Developer Program (USD 99/year), declined for now.

## Responsiveness ("not responding" prevention), 26 Nov 2026

| Scenario | Prevention | Verified |
|---|---|---|
| Heavy work on the window thread (map, search, Smart Scan, review, drive list, open/reveal, Restore list, licence check) | All 14 such commands run off the main thread | Compiles; app checked on a 100k-item scan, window responding |
| Hung network / optical drive | Each drive described on its own thread, 2 s limit, shown as Not responding; free-space query 3 s limit | Drive list checked on the owner laptop; a hung drive not simulated |
| Scans starving the interface | Scan and duplicate workers at below-normal priority (Windows) / utility QoS (macOS); raised again on Cancel; pre-cancelled scan starts no threads | Full test suite 4/4 in parallel under CPU contention |
| Memory creep over long sessions | Names in one buffer per scan; spare capacity released | Soak: flat after 10 rescans (see BENCHMARKS.md) |
| Very long lists | Smart Scan groups and Duplicates show 100 at a time | Type-checked |
| Huge single folder in the map | Tiles capped and small items merged (existing); layout now off the window thread | Existing tests |
| Leaked listeners / observers | Audited: every listener and observer is removed on unmount | Code audit |
| Second copy of the app | Single-instance: a second launch focuses the open window | Verified: second launch left one process |
| Unexpected screen error (blank window) | Error boundary with Reload; scans kept in the backend | Type-checked; not triggered deliberately |

## Website

Republished 27 Nov 2026 ~19:40 IST with the owner's OK: guides, FAQ (deletion "never without asking", Undo, "What if a file cannot be removed?"), feature list, privacy policy effective 27 Nov (local diagnostics log; permanently deleted items on the local list), and two older home-page claims corrected (it said Tessera "never changes or deletes your files" and "makes no network connections at all"; both stopped being true with Pro cleanup and the licence/update checks). Live at https://tessera-96w.pages.dev (Cloudflare Pages, first deployed 26 Nov 2026): contact (Beyond Enterprises, hullashoo6@gmail.com), privacy policy and terms with a 14-day refund policy published (not lawyer-reviewed), FAQ updated. Since 1.0.0 (27 Nov ~21:06 IST): on sale with the live Dodo checkout, Windows download published (see Release 1.0.0 below). See docs/WEBSITE.md.

## Screen tests

35 automated screen tests (Vitest + Testing Library, simulated browser, backend replaced by test data; 42 interface tests in all with the 5 history-logic and 2 contrast tests). Cleanup (15, `cleanup.test.tsx`): everything selected at first, Select all/none, unticking, filter, take off the list without touching files, no re-check when only taking items off, one-step move with Undo/Open Recycle Bin/Scan again, recovers by itself when the list changed since it was checked, Delete permanently only after ticking the warning, progress and Stop, files in use named by app with Try again, Move the rest, Delete permanently for items too big for the bin, Mac wording, cause grouping and order. Also: Cleanup Pro lock; Smart Scan bulk-add safety and paging; Duplicates keep-oldest and every-copy warning; Restore states; Apps confirmation, Store removal, Mac leftovers; licence entry and test-mode notice; History Pro lock, growth comparison and chart labels; error safety net and its log entry; Welcome (main drive first, Mac wording, skip); Background setting (off by default, Mac wording, start at sign-in unavailable in the Store version); Report (Pro lock, contents). Plus 2 storage-map contrast tests. Run in CI on Windows and macOS, and by `tools/check.ps1`.

Website refresh, 27 Nov 2026: screenshots of Smart Scan, Duplicates, Restore and History (from a disposable C:\Demo folder; no personal paths), FAQ rewritten, latest.json for the update check, stretched-screenshot bug fixed. Lighthouse on the live site: mobile 98/100/100/100, desktop 100/100/100/100 (performance/accessibility/best practices/SEO).

## Release 1.0.0, 27 Nov 2026 ~21:05 IST (owner: "submit, and also make sales live")

| Item | State |
|---|---|
| Live sales | **On.** App builds from 1.0.0 use live licensing (`LIVE_MODE_APPROVED = true`). Before switching, the live checkout link opened a Dodo session for Tessera at USD 9.78 and the live licence server answered. Website Buy button and the app's Buy Tessera Pro open `https://checkout.dodopayments.com/buy/pdt_0Nsmg4vQiRrOCZ4yTeDEk?quantity=1`. **Not yet done: one real live purchase (owner's 100%-off code) activated in the installed app.** |
| Windows installer `Tessera_1.0.0_x64-setup.exe` | 3.6 MB (3,765,434 bytes), sha256 cae6eeaedef26cd3981ae52b79b798fd823f614edc3045b19f88a1cfeca90e1e. tools/release.ps1 with quick checks (all passed), production interface, installed on the owner laptop and smoke-tested (window responsive, slowest 74 ms; right-click entries; no crash). Live mode confirmed in the installed app (no test notice, Buy Tessera Pro shown; the old test activation no longer counts). **Unsigned** (SmartScreen note on the download section) |
| Website | Republished ~21:06 IST: on sale (Buy Tessera Pro), Windows download live at `/downloads/Tessera_1.0.0_x64-setup.exe` (download from the live site has the same SHA-256), Mac shown as in testing, latest.json 1.0.0, changelog page with 1.0.0 |
| Microsoft Store | `target\msix\Tessera_1.0.0.0_x64.msix` built from the same program with the real identity and reserved name; everything to submit is in `target\store-submission` (package, HOW-TO-SUBMIT.md, listing text, 5 screenshots). **Submission must be done in Partner Center by the owner's helper** (needs their sign-in) |

## Local release candidate, 27 Nov 2026 ~20:30 IST (tools/release.ps1 with its checks)

| Installer | Size | SHA-256 | Verified |
|---|---|---|---|
| Tessera_0.1.0_x64-setup.exe (local build, **production interface**) | 3.6 MB | 4daf989de5b92dd3cead15254a8f06b3d50966d05e9db850ad9f560d73243338 | Quick checks passed (lint, 35 screen tests, production-bundle check, Rust format/clippy/tests incl. the command and shared-message tests, macOS compile check, website build). Interface 331 KB, production React (was 575 KB development React). Smoke test on the owner laptop: silent install, right-click entries, window responsive (slowest 45 ms over 10 s), log start, no crash, closing quit. **Now installed.** Store package `Tessera_0.1.0.0_x64.msix` built from the same program, test-installed (Developer Mode), cleanup of 6 disposable files to the Recycle Bin and Undo worked, Settings showed the Store texts; then removed. Unsigned. No macOS build. **Earlier local builds tonight (b687a0ab..., 77a55028..., d762cfbd..., 825feb1d...) bundled React's development version** (NODE_ENV leak; see HANDOFF lessons) |

## Release candidate, 27 Nov 2026 (built by the release workflow from commit 3802247)

| Installer | Size | SHA-256 | Verified |
|---|---|---|---|
| Tessera_0.1.0_x64-setup.exe | 3.3 MB | 34df36201b4c9ba3bc13acaae4782e7e199f8911275a7553e102c40b214a50d3 | Installed on Windows 11 (owner laptop): silent install over the previous build, licence notices shipped, folder argument scan, Smart Scan, Duplicates (exact copies), cleanup of 2 copies to the Recycle Bin then Put back with identical contents and nothing left in the bin, Pro licence active, no false update banner, single instance, window responding. Unsigned |
| Tessera_0.1.0_universal.dmg | 10 MB | 4f5cdd636c02f2814d0b0ddcaa745fbc08f87b1dcafb5a368ce59e6217e3e5d4 | Built on GitHub's macOS runner; ad-hoc signed, not notarised; not run on a real Mac |
