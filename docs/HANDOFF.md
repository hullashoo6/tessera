# Tessera — handoff for the next Claude session

First written 27 Nov 2026 00:45 IST; updated 27 Nov ~06:00 IST at the end of an unattended overnight session. Read this first, then `STATUS.md`, then `git log --oneline -30`.

## Start here: what to do next

1. Tell the owner what is waiting on them (the list in STATUS.md "Owner decisions outstanding"); the most urgent are the **GitHub Actions minutes** (no CI and no Mac build until resolved), the **Partner Center identity values** for the Store package, and an **OK to redeploy the website** (guides, privacy and FAQ edits are committed, not live).
2. Until GitHub Actions works again, run `powershell -File tools\check.ps1` before every push, and push in batches.
3. Next product work, in order: localisation once the owner picks languages (docs/LOCALISATION.md); then the "still open" items in section 7. When the Store identity arrives: `tools\make-msix.ps1`, then docs/STORE.md.
4. Manual checks only a person can do are listed in TESTING.md ("Manual checks per installer"); the owner can do several in a few minutes (right-click a folder in File Explorer, drag a folder onto the window, save a report as PDF, the tray icon).

## 0. Every session (the owner only uses incognito chats, so nothing is remembered between sessions)

- Start: read this file, `STATUS.md` and `git log --oneline -30` before doing anything.
- Finish (and whenever the owner says they are stopping, or before a long unattended run ends): update this file (what changed, new IDs or decisions, new lessons, next steps) and `STATUS.md`, then commit and push. This file is the project's only memory.
- The short start prompt the owner pastes is in `docs/START_PROMPT.md`.

## Session log (newest first)

- **27 Nov 2026, 00:50-06:00 IST (owner asleep from 00:52; unattended, all permissions granted, owner rules unchanged: nothing paid, published or sold, no personal files).** Later in the night also: printable report / save as PDF (`components/Report.tsx`, `print_report`); storage-map tips; Microsoft Store readiness (`update::store_managed()`, MSIX script built and validated with placeholder identity); fix for the stale "This scan is no longer available" banner (`SCAN_REPLACED` in App.tsx must match `active()` in lib.rs); website guides (`website/src/pages/guides/`, not deployed); `tools/release.ps1` (one-command Windows release candidate with automated smoke test of the installed copy) and `tools/check.ps1`; high contrast and dark mode checked by emulation (`tools/cdp-shot.mjs`); start at sign-in (`tauri-plugin-autostart`, `--background`, window now starts hidden in tauri.conf.json and is shown in `setup`); History compares the 200 largest folders at any depth (`deep`, `deepFloor` in snapshots; `src/historyChanges.ts`); scans started from outside the window use the remembered hard-link setting (`%APPDATA%\app.tessera.desktop\scan-settings.txt`). The owner's settings were left as found: background mode off, no sign-in entry, hard links Standard; the storage-map tips are not dismissed and the welcome screen was already dismissed (Settings > Welcome screen shows both again). Done earlier in the night: STATUS.md brought up to date; first-run welcome screen (Settings > Welcome screen shows it again; `system` flag on drives so the main drive is offered first); **scan from outside the window** (`src-tauri/src/launch.rs`: Windows right-click "Scan with Tessera" via NSIS hooks `src-tauri/windows/hooks.nsh`, verified with a local installer install/uninstall; macOS `Info.plist` folder document type + `RunEvent::Opened`; window drop; second launch hands its folder over; `"C:\"` quote repair); high contrast (CSS media queries + `palette.ts` high-contrast palette, contrast test `palette.test.tsx`; grey text darkened for 4.5:1); crash resilience (`diagnostics.rs` local log with path scrubbing, Settings > Diagnostics, scan panics caught); History chart axis labels; CI memory gate (`soak --max-peak-growth-mib`) and screen-request budgets (`tessera-cli ui-bench --check`); CHANGELOG rewritten; Smart Scan parity and new rules incl. "Backups and recordings" and "Large old files" groups (`crates/insights`); background mode (`src-tauri/src/background.rs`: tray icon, low-space notification, `tauri-plugin-notification`, `tray-icon` feature; settings in `%APPDATA%\app.tessera.desktop\background.json`, left **off**); small screens (min window 800 x 480, fit-to-screen at start, narrow CSS); **1M-file UI check** (all requests 0.3-225 ms, window never over 34 ms to respond; BENCHMARKS.md). The laptop now has a **local build** installed (unsigned, from this session) instead of the release-workflow RC. The website was **not** redeployed this session (publishing needs the owner's OK); website edits (privacy policy mentions the local log, FAQ and feature list mention right-click scanning) are committed only.

- **GitHub Actions stopped running (27 Nov ~02:00 IST).** Every run since commit `0e695cb` fails in 2 s with "The job was not started because recent account payments have failed or your spending limit needs to be increased". The private repo's free monthly Actions minutes are used up (macOS minutes count 10x; many pushes tonight). This also blocks `release.yml` (the Mac DMG can only be built there). Not changed without the owner: options are (a) wait for the monthly reset, (b) make the repo public (free minutes, but publishes the source code), (c) add a payment method / spending limit (paid). Meanwhile: `tools/check.ps1` runs every CI check locally plus a macOS compile check of the engine crates; docs-only pushes no longer start CI. Last commit fully verified on GitHub's macOS runner: `bb7d2f2` (background mode); later desktop changes are cross-platform.

## 1. Your role and the owner

You are the founding technical team for **Tessera**, a commercial disk-space analyser for **Windows and Mac**, sold by **Beyond Enterprises** (owner: non-technical founder in Udaipur, India, timezone IST / UTC+5:30). Contact: **hullashoo6@gmail.com**.

How the owner wants you to work:
- Plain English, short answers, exact click-by-click steps when they must act. They are not a developer.
- Keep working without stopping; commit and push regularly; if blocked or waiting on approval, work around it and build something else.
- They pre-approve tool permission prompts ("always allow").
- **Windows and Mac must have identical features.** Never ship a feature on one only, and never word anything as "Windows only".
- **No lag, ever.** Responsiveness is a top priority (see section 6).
- **No paid services or purchases without explicit approval.** They declined Apple's USD 99/year notarisation.
- Keep **KeyStow** installed on their laptop (an app of theirs; do not uninstall it).
- The name **Tessera** is approved.
- Hard rules: never enable live sales or publish anything new without approval; never put secrets (API keys) in the app, website or GitHub; never scan or screenshot their personal files; **no usernames or personal paths on the website**; label status honestly (Not implemented / Implemented but unverified / Automatically tested / Real platform tested / Blocked).

## 2. Accounts and places (no secrets here)

| What | Where |
|---|---|
| Code | Private GitHub repo **github.com/hullashoo6/tessera** (account hullashoo6; commits as 357693785+hullashoo6@users.noreply.github.com). Old duplicate hullash007/tessera is obsolete. |
| Laptop project folder | `C:\Users\mengh\projects\tessera` |
| Website | **https://tessera-96w.pages.dev** (Cloudflare Pages project `tessera`, owner's free Cloudflare account; Wrangler is logged in on the laptop). Deploy steps in `docs/WEBSITE.md`. |
| Payments | **Dodo Payments**, owner's existing Beyond Enterprises account. Merchant of record. |
| Dodo LIVE | product `pdt_0Nsmg4vQiRrOCZ4yTeDEk`, licence entitlement `ent_0Nsmfr7CY18AXemTSmNxC`, USD **9.78** one-time, tax included, licence for **1 device, no expiry**. Live checkout was "still being verified" by Dodo; **live sales must stay disabled** until the owner approves. |
| Dodo TEST | product `pdt_0Nsmh1XPpX4NRvbkO8N7C`, entitlement `ent_0NsmgeTrxozGfeC4KvhR9`, API base https://test.dodopayments.com. A real test purchase was made; its key (masked `•••• f4ef`, registered to hullasho7@gmail.com) is activated on the laptop. |
| Discount | The owner has a 100%-off Dodo discount code for real test purchases. Ask them for it; it is deliberately not stored in the repo. |
| Microsoft Store | Owner will create a **free** Partner Center account (planned 27 Nov). Needed for free signing (section 7). |
| OneDrive | A personal OneDrive is signed in on the laptop, but it was **not syncing** (new files never uploaded), so the cloud "free up space" test could not finish. |

Laptop: Windows 11 Home (build 26300), Intel i3-1115G4 (2 cores / 4 threads), 11.8 GB RAM, ~44 GB free of 236 GB on C:. Tools: Git, gh CLI, Node 24 / npm 11, Rust pinned to **1.100.0** (`rust-toolchain.toml`), VS 2022 Build Tools, winget.

## 3. What the product does today

Tauri 2 desktop app (Rust backend, React 19 + TypeScript 7 + Vite 8 interface). Read-only scanner; all scans stay on the computer; no analytics/telemetry.

Free:
- Live parallel scan of drives/folders; nested **storage map** (treemap), details panel, contents list, breadcrumbs, keyboard navigation, context menu; Size on disk vs File size; hard-link handling; online-only (cloud placeholder) files never opened.
- **Largest files**, **Search** (name, type groups, size, date, empty folders, show on map).
- **Smart Scan**: evidence-based suggestions (node_modules with package.json, Rust target with Cargo.toml, Python venvs, web build caches, package caches npm/pip/yarn/gradle/maven/nuget/cargo, Xcode DerivedData, AI models, browser caches, crash dumps, temp files older than a week, Docker data marked "use Docker's own clean-up", and a **Cloud** group of downloaded OneDrive/iCloud files). Each item: reason + safety level.
- **Duplicates**: size → partial hash → full BLAKE3; keep oldest/newest; warning if every copy chosen.
- **Apps** list: Windows classic apps (registry) and Microsoft Store apps (PackageManager); Mac apps in /Applications and ~/Applications. Sizes measured where possible.
- **Restore**: list of everything Tessera moved (30 days / 500 items), **Put back** (never overwrites). Never Pro-gated.
- Settings: theme, size metric, hard links, Tessera Pro licence, **update check** (on by default, can be turned off; Store copies leave updates to the Store), **Background** (keep running in the notification area / menu bar, start at sign-in, low-space warning; all off by default), **Diagnostics** (local log, no paths), Welcome screen again.
- Added 27 Nov: first-run **welcome screen** and storage-map **tips**; **scan from outside the window** (right-click "Scan with Tessera" on Windows, Open With / Dock on Mac, drop a folder on the window, second launch); Smart Scan **Backups and recordings** and **Large old files** groups and many new rules with Mac parity; **high contrast**; small screens (min 800 x 480).

Pro (USD 9.78 one-time):
- **Cleanup**: review-first, moves to Recycle Bin/Trash only (never permanent delete), protection rules for system folders, re-validation before each move.
- **Free up space** for cloud files (OneDrive/Cloud Files on Windows via the unpinned attribute; iCloud Drive eviction on Mac).
- **Uninstall** from Tessera: Windows opens the app's own uninstaller or removes Store apps; Mac moves the .app to the Trash (recorded for Put back) and offers leftovers in ~/Library found by bundle id only.
- **CSV reports** (largest files, current folder; formula-safe).
- **Storage history**: summary of each finished scan (since 27 Nov including the 200 largest folders at any depth); History page charts growth and names the most specific folders that changed vs about a month ago / previous scan.
- **Printable report**: Storage map > Report, one-page summary of the open folder; print or save as PDF through the system dialog.

Robustness built in: every heavy command runs off the window thread (`#[tauri::command(async)]`); per-drive 2 s time limits (hung network drives shown "Not responding"); scan/hash workers at low priority (raised to normal on Cancel); names stored in one buffer per scan (memory flat over long sessions); long lists paged 100 at a time; single instance; React error boundary with Reload.

Measured (owner laptop): 1M files: peak 144 MiB (was 183), ~75 MiB held and steady, cancel ≤3 ms, scan ~6 s median. 100k: 8.5 MiB held. Lighthouse on live site: mobile 98/100/100/100, desktop 100/100/100/100.

## 4. Repository map

- `crates/scan` scanner, model (arena tree, shared name buffer), queries; `crates/treemap` layout; `crates/testkit` fixtures; `crates/cli` (`volumes`, `scan`, `gen`, `bench`, `soak`, `demo`).
- `crates/cleanup` protection, plan/revalidate/execute, `trash.rs` (Windows IFileOperation with recycle-only guard; macOS NSFileManager trashItemAtURL with resulting URL), `put_back` (MoveFileExW no-replace / renamex_np RENAME_EXCL), `history.rs` (recently moved), `cloud.rs` (placeholder detection with per-thread placeholder exposure, free-up).
- `crates/licensing` Dodo public licence endpoints (activate/validate/deactivate), TEST/LIVE configs, `LIVE_MODE_APPROVED = false`, keyring storage, `https_get_json` for the update check.
- `crates/insights` Smart Scan rules; `crates/dupes` duplicate finder; `crates/apps` installed apps & uninstall (Windows registry + Store, macOS bundles + leftovers).
- `apps/desktop` Tauri app: `src-tauri/src/lib.rs` (commands, start-up), `launch.rs` (scans from outside the window), `background.rs` (tray, notifications, autostart), `diagnostics.rs` (local log), `licence.rs`, `update.rs`, `history.rs`, `dto.rs`; `src-tauri/windows/hooks.nsh` (installer: right-click entries, autostart clean-up), `src-tauri/Info.plist` (macOS: opens folders); `src/` React screens (Welcome, Overview, Smart Scan=Suggestions, Duplicates, Storage map, Report, Largest files, Search, Cleanup, Restore, History, Apps, Settings incl. BackgroundSetting), `src/historyChanges.ts`, `src/diagnostics.ts`, `src/palette.ts`; tests in `src/__tests__/` (screens, palette contrast, history changes).
- `website/` Astro static site (`src/data/site.ts` pricing/contact/latest version, `faq.ts`, `releases.json` empty = no downloads, pages incl. privacy, terms, latest.json).
- `tools/`: `check.ps1` (every CI check locally + macOS compile check), `release.ps1` (Windows release candidate: build, install, smoke test, manifest), `cdp-shot.mjs` (screenshot the dev page with emulated high contrast / dark mode), `capture-window.ps1`, `ui-invoke.ps1`, `ui-type.ps1`, `ui-window.ps1` (UI automation by process), `release-manifest.ps1`, `notices.ps1` (regenerates THIRD_PARTY_NOTICES.md), `make-msix.ps1`, `make-dmg-background.ps1`. `crates/cli` also has `ui-bench` (screen request time budgets) and `soak --max-peak-growth-mib`.
- Docs: `STATUS.md` (honest status table), `BENCHMARKS.md`, `SECURITY.md`, `ARCHITECTURE.md`, `TESTING.md`, `RELEASE.md`, `CHANGELOG.md`, `THIRD_PARTY_NOTICES.md`, `docs/LICENSING.md`, `docs/STORE.md`, `docs/WEBSITE.md`.
- CI: `.github/workflows/ci.yml` (Windows + macOS: fmt, clippy, tests incl. real Trash round trip on macOS, screen tests, website build). `release.yml` (manual): Windows NSIS + macOS **universal** DMG as private artifacts.
- `dist-local/` (gitignored): latest installers downloaded from the release workflow.

## 5. Verification status (honest summary)

Real-platform tested on Windows 11: scanning, map, search, Smart Scan, duplicates, cleanup to Recycle Bin, Put back, cloud placeholder detection, Store-app removal (Simple Spider Solitaire), uninstall launch (temporary test entry; KeyStow's real uninstaller opened but not completed by design), licence activate/validate/deactivate with a real Dodo test key, installed build end to end (release-workflow artifact `Tessera_0.1.0_x64-setup.exe`, 3.3 MB, unsigned).

Tested on GitHub's macOS runner (real macOS, not real hardware): Trash move + Put back, Mac app listing/trash/leftovers. **Never run on a real Mac.** DMG `Tessera_0.1.0_universal.dmg` (10 MB) is ad-hoc signed, not notarised; its window background tells users the one-time Privacy & Security "Open Anyway" step and why (no USD 99 notarisation) — the owner wants this explanation **only** in the installer window, nowhere else.

Not yet verified: cloud free-up end to end (OneDrive not syncing); CSV export through the Save dialog (dialog not automatable; needs a 30-second manual check); MSIX package (needs Store identity); everything on real Mac hardware; NVDA/VoiceOver screen-reader runs.

## 6. Lessons learned on this laptop (avoid repeating)

- **PowerShell 5.1 encoding:** `Get-Content`/`Set-Content` default to ANSI and garbled UTF-8 once. Use `[IO.File]::ReadAllText(p,[Text.Encoding]::UTF8)` / `WriteAllText(p,s,(New-Object Text.UTF8Encoding($false)))`, or the Desktop Commander `edit_block` tool (safest for code).
- In double-quoted PowerShell strings, `$x` and `${a.name}` interpolate (this blanked JS template literals once). Use single-quoted here-strings `@' ... '@` or `edit_block` for code containing `$`.
- PowerShell single-pair arrays must be `@(,@('a','b'))`; `@(@('a','b'))` flattens and once replaced every `.` with `f` in a file (restored from git). Always check replacements actually matched (print MISSING) and compile.
- `write_file` needs `mode: rewrite` for existing files. rustfmt rewraps lines, so text replacements against formatted code can silently miss.
- Tauri sync commands run on the main thread: every command touching the tree, file system, keyring or OS must be async.
- Windows hides cloud placeholders from apps by default; `cloud.rs` exposes them per thread only during checks.
- Windows 11 folder picker, Save dialog and Inno Setup dialogs are **not automatable** — automating the picker once scanned the owner's Downloads. Test by launching the app with a folder argument (`tessera-desktop.exe "C:\path"`, or `TESSERA_DEV_SCAN` in dev).
- Find windows **by process id, never by title** (a title search once read text from the owner's KeyStow and Notepad windows).
- The debug `target` folder grew to 30.7 GB and filled the disk; dev profile now uses minimal debug info. Check free space before big builds; `cargo clean` is safe.
- `npm install` with `NODE_ENV=production` pruned devDependencies; use `NODE_ENV=development` and `npm install --include=dev`.
- `Select-String` is case-insensitive by default (use `-CaseSensitive` for "FAILED").
- PowerShell `Invoke-WebRequest` can't follow 308 redirects; Cloudflare Pages adds trailing slashes.
- A debug build run without `npx tauri dev` shows a blank window (needs the Vite dev server).
- Disposable fixtures go in `.fixtures\<name>` with a `.tessera-fixture` marker and are deleted afterwards. Website screenshots come from a made-up `C:\Demo` folder so no personal paths appear.
- **GitHub Actions minutes are limited** (private repo; macOS minutes count 10x). Each push starts CI; pushing every few minutes used up the month on 27 Nov. Batch pushes, and docs-only pushes no longer trigger CI. `tools/check.ps1` runs the same checks locally.
- Windows PowerShell 5.1 with `$ErrorActionPreference = 'Stop'` stops the script when a native program's stderr is redirected (`*>`, `2>&1`): run such commands through `cmd /c "... > log 2>&1"`.
- `SetWindowPos` called from (non-DPI-aware) PowerShell takes logical coordinates: on this 125% screen, 800 x 480 gives a true 800 x 480-point window.
- Desktop Commander's `start_process` sometimes returns only the first lines of output; call `read_process_output` for the rest. Long jobs (> 4 min) must run with `Start-Process` in the background and be polled, or the tool call times out.
- The dev app and the installed app share all state (same identifier `app.tessera.desktop`): WebView settings such as "welcomed", the background-mode file in `%APPDATA%\app.tessera.desktop`, and the log in `%LOCALAPPDATA%\app.tessera.desktop\logs`. Leave settings as you found them after testing.
- macOS cross-checks from Windows: `cargo clippy --target aarch64-apple-darwin` works for scan, insights, cli and treemap; crates with C code (ring, blake3, objc2-exception-helper) need Apple's toolchain, so the desktop app itself can only be checked on GitHub's macOS runner.
- The Restore list contains the owner's own entries (they cleaned Hugging Face models, crash dumps and a temp file on 26 Nov ~22:04 and emptied the bin). Do not clear it.

## 7. Roadmap to a great product

### Phase 0 — Launch blockers (needs the owner)
1. **Microsoft Store (free signing, no SmartScreen warning):** owner creates a free Partner Center account (Company "Beyond Enterprises" or Individual), reserves "Tessera", sends Identity Name, Publisher (`CN=...`), PublisherDisplayName. Then run `tools/make-msix.ps1`, validate with the Windows App Certification Kit, test install, submit (owner approves). Non-game apps may keep Dodo payments (100% revenue). See `docs/STORE.md`.
2. **OneDrive:** owner checks the OneDrive icon (paused or error), then rerun the ignored test `free_up_space_with_a_real_cloud_file` with a disposable synced file; record result.
3. **Real Mac testing** (borrow a Mac, ideally Apple silicon and Intel): install the DMG, follow the Open Anyway step, run the full checklist (scan, map, Smart Scan, duplicates, cleanup to Trash, Put back, Apps uninstall + leftovers, iCloud free-up, licence, update check, history), measure performance.
4. **Live payments:** when Dodo confirms live checkout and the owner approves: set `LIVE_MODE_APPROVED = true`, add the live checkout link to the website (`pricing.onSale = true`), make a real live purchase with the owner's 100%-off code, activate/deactivate it, test a refund, then rebuild installers.
5. **Downloads:** host installers (Cloudflare Pages allows files up to 25 MB, or GitHub Releases on a public repo, or Cloudflare R2), fill `website/src/data/releases.json` only with verified files, publish SHA-256s, bump `latest.json` per release.
6. **Legal:** lawyer review of privacy and terms (add governing law), trademark search/registration for "Tessera" in class 9, a dedicated support email (e.g. support@ a custom domain) and optionally a custom domain on Cloudflare (paid; ask).

### Phase 1 — v1.0 polish (no approval needed)
- DONE 27 Nov: first-run welcome screen, storage-map tips (the "short tour"); "Scan with Tessera" (right-click on Windows, Open With/Dock on Mac, drop on window, second launch); CHANGELOG; release automation (`tools/check.ps1`, `tools/release.ps1`); History axis labels; keyboard audit (no mouse-only controls found); high contrast; small screens / high scaling layout; crash resilience (local log, scan faults, folder vanishing mid-scan); 1M-file screen performance measured; CI memory and time-budget gates.
- Still open: version 1.0.0 bump (do it when the owner wants to release); CSV export and Print-to-PDF manual checks; Store app sizes on more machines; NVDA (Windows) and VoiceOver (Mac) runs; look at the app with a Windows contrast theme switched on and at real 200% scaling; JavaScript frame-time measurement during scans.

### Phase 2 — Features that beat DissectMac and others
- DONE 27 Nov: Smart Scan rules (Windows Update downloads and previous Windows installation via Windows' own clean-up, iPhone/iPad backups, Android images and AVDs, Xcode simulators/archives/device support, Unity/Unreal, old installers in Downloads, screen recordings, Firefox, macOS installers; Mac parity for browser caches, pip, Yarn, crash reports); "Large old files" (by modified date: access dates are unreliable); background mode with tray/menu-bar free space and low-space warning; drag a folder onto the window; printable report / save as PDF (Pro).
- Still open: Delivery Optimization, WhatsApp/Telegram media and language packs (paths not verified; add only with evidence); compare two scans in depth (History compares the top-level folders; deeper needs more data per snapshot); scheduled scans; OneDrive free-up for whole folders; Dropbox/Google Drive; **localisation (plan in docs/LOCALISATION.md; owner to choose languages and reviewers)**; "Start at sign-in" for background mode (would need the autostart plugin).

### Phase 3 — Launch and growth
- Landing page copy and comparison pages (vs WinDirStat, WizTree, TreeSize, DaisyDisk, DissectMac), honest benchmarks page.
- DRAFTED 27 Nov (committed, not deployed): guides at `website/src/pages/guides/` (Windows, Mac, developer caches, cloud files). Deploy after the owner's OK.
- Demo video (60-90 s), screenshots per platform, Product Hunt launch, Reddit (r/DataHoarder, r/windows, r/mac, r/webdev), Hacker News Show HN, YouTube reviewers, Microsoft Store listing optimisation.
- Launch discount codes in Dodo; referral or bundle ideas later.
- Support: FAQ, email templates for refunds/licence moves, a public changelog.

### Phase 4 — Business operations
- Pricing review after first sales (current USD 9.78 vs DissectMac USD 19.99), regional pricing in Dodo.
- Licence device policy (1 device now; consider 2-3 for a family pack).
- Apple Developer Program (USD 99/year) when revenue justifies it: notarisation removes the Gatekeeper step and enables Mac App Store.
- Paid Windows signing only if direct downloads matter beyond the Store.

## 8. Commands cheat sheet

```powershell
cd C:\Users\mengh\projects\tessera
powershell -File tools\check.ps1                                   # everything CI checks (use before pushing)
powershell -File tools\release.ps1                                 # Windows release candidate, installed and smoke-tested
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo fmt --all; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
cd apps\desktop; $env:NODE_ENV='development'; npx tsc -b; npx oxlint src; npm test
$env:TESSERA_DEV_SCAN='C:\path\to\fixture'; npx tauri dev          # dev app scanning a folder
npx tauri build --bundles nsis                                     # local Windows installer (~30 min cold)
gh workflow run release.yml -R hullashoo6/tessera                  # both installers in the cloud
gh run download <run-id> -R hullashoo6/tessera -D dist-local\ci
cargo run --release -p tessera-cli -- gen .fixtures\perf-100k --files 100000
cargo run --release -p tessera-cli -- soak .fixtures\perf-100k --runs 20
powershell -File tools\notices.ps1                                 # regenerate licence credits
cd website; $env:PUBLIC_SITE_URL='https://tessera-96w.pages.dev'; npx astro build; npx wrangler pages deploy dist --project-name tessera --branch main
```

Ignored real-system tests: `cargo test -p tessera-cleanup -- --ignored put_back` (real Recycle Bin/Trash), `-- --ignored free_up_space` (needs `TESSERA_CLOUD_TEST_FILE`), `cargo test -p tessera-licensing -- --ignored` (real Dodo test server, unknown key).
