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
| Desktop app (Tauri 2 + React), dashboard, treemap, views, search | Not implemented |
| Categorisation, developer storage view | Not implemented |
| Cleanup (review → Trash) | Not implemented |
| Storage history, reports | Not implemented |
| Free/Pro entitlements, Dodo Payments (test mode) | Not implemented |
| Website (Astro) | Not implemented |
| Installers, signing, release manifest | Not implemented; signing **blocked** on owner decisions (paid certificates) |

## Owner decisions outstanding

* Final product name (Tessera is a working name; trademark search needed).
* Pricing (USD 14.99 is provisional; not published anywhere).
* Mac hardware for real macOS testing (CI alone is not acceptance).
* Code-signing certificates for Windows and Apple Developer Program (paid).
