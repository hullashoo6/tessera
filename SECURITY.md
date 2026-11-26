# Security and privacy

Status: **draft threat model**, updated as each component is built.

## Principles

* Disk analysis runs entirely on the user's computer. File names, paths, scan
  results and file contents are never uploaded.
* No mandatory telemetry, advertising or tracking SDKs. No hidden background
  scanning.
* Least privilege: no administrator rights for ordinary scanning.
* Logs exclude personal paths by default (the scanner records issues by node
  id, not by path).

## Threat model

### Scanner (implemented)

| Threat | Mitigation |
|---|---|
| Following a link/junction into unexpected locations or loops | Links, junctions and mount points are never followed; other filesystems are not entered; directory (dev, inode) de-duplication on Unix |
| Downloading cloud-only files | Metadata comes from directory enumeration only; placeholder files are never opened |
| Modifying user data | Crate is read-only: no write, rename or delete APIs are used. Windows hard-link mode opens files with attribute-read access only |
| Crashes on hostile names / huge directories | Names stored losslessly as OS strings; long paths handled; errors recorded, never fatal; bounded threads and backpressure |
| Leaking paths via logs or diagnostics | Issues reference node ids; no logging of paths by default |

### Desktop IPC (planned)

* Tauri capabilities restricted to the app's own commands; no generic fs/shell
  plugins exposed to the webview.
* Commands take node ids from the backend's own scan where possible; any path
  input is validated in Rust.
* Strict Content Security Policy; no remote content in privileged windows.

### Cleanup (planned)

* Native Trash/Recycle Bin only; permanent deletion disabled in v1.
* Time-of-check/time-of-use: immediately before each move, re-resolve the path
  without following links, compare file identity with the scanned identity,
  re-check protected-location rules, and refuse on any mismatch.
* Protected locations: OS directories, boot/recovery, application bundles in
  use, user-profile roots, the volume root.

### Licensing, payments, downloads, updates (planned)

* No merchant secrets or signing private keys in the app or website.
* Licence failures never block scanning or access to the user's own files.
* Release artifacts carry SHA-256 checksums from a verified manifest; automatic
  updates only after signature verification and rollback are implemented.

## Reporting a vulnerability

Contact address to be provided by the owner before release.
