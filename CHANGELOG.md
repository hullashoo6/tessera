# Changelog

## Unreleased

### Added
- Read-only parallel scanning engine (`tessera-scan`) with live, always
  consistent totals, cancellation, per-node issue reporting, and Windows /
  macOS platform adapters.
- Hard-link de-duplication: all files on macOS; files ≥ 64 KiB on Windows by
  default (measured to remove ~95% of the overcount on `C:\Windows` at no
  measurable cost), every file in Thorough mode.
- Drive/volume listing with OS-reported capacity.
- `tessera-cli` for real-disk scans, deterministic fixture generation and
  benchmarks.
- Real-filesystem test suite and CI for Windows and macOS.
