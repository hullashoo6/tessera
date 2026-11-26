# Changelog

Everything listed works the same on Windows and Mac unless a line says how
the two differ in wording only (Recycle Bin / Trash, File Explorer / Finder).
How far each item has been verified is recorded in STATUS.md.

## Unreleased

### Added
- First-run welcome screen: what Tessera does, the privacy promise, and a
  one-click first scan of the main drive or the home folder. Settings >
  Welcome screen shows it again.
- Start a scan from outside Tessera: right-click a folder or drive in File
  Explorer ("Scan with Tessera", under "Show more options" on Windows 11) or
  in Finder ("Open With > Tessera"), drop a folder onto Tessera's window or
  Dock icon, or launch Tessera with a folder. If Tessera is already open, the
  open window scans it.
- Smart Scan finds more: Firefox caches, Android emulator images and virtual
  devices, iOS simulators and Xcode archives, Unity and Unreal Engine caches,
  a previous Windows installation and Windows Update downloads (with the
  steps to remove them using Windows' own clean-up), macOS installers,
  installers left in Downloads for over a month, and two new groups:
  "Backups and recordings" (iPhone and iPad backups, screen recordings) and
  "Large old files" (256 MB or more, unchanged for a year).
- Background mode (Settings > Background, off unless turned on): Tessera can
  stay in the notification area or menu bar after its window is closed,
  showing free space, and can warn when the main drive is almost full.
- Diagnostics: a small local log of crashes and errors, with no file or
  folder names, that can be attached to a support email (Settings >
  Diagnostics). Nothing is sent automatically.
- History chart shows its scale and dates.
- High contrast: Windows contrast themes and the Mac's "Increase contrast"
  setting give stronger text and lines, outlines for states otherwise shown
  only by colour, and a high-contrast storage map (labels at least 7:1).

### Changed
- Small grey text and the dark storage map's colours adjusted so all text
  meets the 4.5:1 contrast guideline.
- Smart Scan suggestions that only knew Windows locations (browser caches,
  pip and Yarn caches, crash reports) now find the Mac equivalents too.

### Fixed
- If something unexpected stops a scan, Tessera now says so instead of
  showing "Scanning" forever.

## 0.1.0 (release candidate, 27 Nov 2026; not published)

### Free
- Live parallel scan of drives and folders; storage map (treemap), details
  panel, contents list, breadcrumbs, keyboard navigation, context menu.
- Size on disk or file size; hard links counted once; online-only cloud files
  never opened or downloaded.
- Largest files; Search (name, type groups, size, date, empty folders, shown
  on the map).
- Smart Scan: evidence-based suggestions with a reason and a safety level
  (developer build folders, package caches, Xcode data, AI models, browser
  caches, crash dumps, old temporary files, Docker data, downloaded cloud
  files).
- Duplicates: size, partial hash, then full BLAKE3; keep oldest or newest;
  warning when every copy is chosen.
- Apps: installed apps with sizes (Windows classic and Store apps; Mac apps).
- Restore: everything Tessera moved in the last 30 days, with Put back
  (never overwrites).
- Update check (can be turned off); nothing is downloaded automatically.

### Pro (USD 9.78 one-time, one device)
- Cleanup: review first, then move to the Recycle Bin or Trash only, with
  protection rules and a re-check of each item before it moves.
- Free up space from OneDrive and iCloud files while keeping them in the
  cloud.
- Uninstall apps (the app's own uninstaller or Store removal on Windows; move
  to the Trash plus leftovers found by bundle id on Mac).
- CSV reports; storage history with growth between scans.

### Engine and safety
- Every heavy task runs off the window thread; hung network drives time out;
  scans run at low priority; memory stays flat over long sessions (1 million
  files: ~75 MiB held).
- No analytics or telemetry; scans never leave the computer.
