# Benchmarks

All figures are **measured**, on the hardware stated, with the exact command
shown. They are not marketing claims and are not comparable with other tools
unless measured on the same machine, dataset, permissions and size metric.

## Test environment (owner's laptop)

| | |
|---|---|
| CPU | Intel Core i3-1115G4, 2 cores / 4 threads |
| RAM | 11.8 GB |
| OS | Windows 11 Home, build 26300, x64 |
| Disk | internal SSD, NTFS, 4 KiB clusters (236.5 GiB volume) |
| Antivirus | Microsoft Defender real-time protection (default; left on) |
| Build | `cargo build --release` (Rust 1.100.0, thin LTO) |
| Date | 2026-11-26 |

"Warm cache" means a warm-up scan ran first so Windows' file-metadata cache
was populated. Cold-cache figures require a reboot and are listed separately
when measured.

## Synthetic fixtures

Generated with `tessera-cli gen <dir> --files N` (deterministic seed; 90% of
files ≤ 600 B, 9.5% ≤ 16 KiB, 0.5% ≤ 1 MiB; ~40 files per folder).
Measured with `tessera-cli bench <dir> --runs 5` (3 runs for Thorough).
Raw JSON: `bench-results/`.

| Dataset | Files | Folders | Hard-link mode | Median scan | Min–max | First results | Cancel latency (max) | Peak memory |
|---|---|---|---|---|---|---|---|---|
| 10k | 10,001 | 755 | Auto (≥64 KiB) | 0.082 s | 0.045–0.286 s | 3.8 ms | 13 ms | 7.1 MiB |
| 100k | 100,001 | 7,234 | Auto (≥64 KiB) | 0.243 s | 0.234–0.339 s | 2.1 ms | 4.9 ms | 17.5 MiB |
| 100k | 100,001 | 7,234 | Thorough (all files) | 3.40 s | 3.40–3.54 s | 8.6 ms | 6.9 ms | 16.3 MiB |
| 1M | pending | | | | | | | |

Note: the two 100k "Auto" rows above were recorded before the 64 KiB
threshold existed, when Auto meant "not checked" on Windows. Fixture files
above 64 KiB are 0.5% of the total, so the difference is expected to be
small; the rows will be re-measured with the current build.

## Real data: `C:\Windows` (read-only)

242,858 files, 121,856 folders, 50 permission-denied folders. Single runs;
system activity (an installer was running earlier) causes notable variance.

| Mode | Elapsed | Size | On disk (est.) |
|---|---|---|---|
| Off (each path counted) | 12.4 s | 45.16 GiB | 44.98 GiB |
| **Auto (files ≥ 64 KiB checked) — default** | 11.6 s | 34.52 GiB | 34.31 GiB |
| Thorough (all files checked) | 17.9 s | 33.99 GiB | 33.67 GiB |
| First, cold-ish run (Off) | 34.3 s | — | — |

Findings:
* Not checking hard links overstates `C:\Windows` by ~11 GiB (33%), because
  `WinSxS` and `System32` share files. The 64 KiB threshold removes ~95% of
  that error with no measurable time cost.
* Thread count (Auto mode, warm): 2 → 11.5 s, 4 → 7.2 s, 8 → 7.2 s,
  16 → 7.3 s. The automatic default (CPU threads, clamped 2–8) is 4 here.
* Per-folder overhead dominates on real system folders (≈120k small folders);
  this is where a future NTFS MFT reader could help (deferred; see STATUS.md).

## Not yet measured

* Cold-cache runs (needs controlled reboots).
* macOS (no Mac hardware available yet; CI runs a smoke benchmark only, and
  CI runners are shared VMs, so their numbers are not product figures).
* UI responsiveness (no UI yet).
* Comparisons with other analysers (to be done on identical hardware and data,
  same metric, before any claim is made).
