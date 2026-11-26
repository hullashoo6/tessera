# Architecture

Working product name: **Tessera** (placeholder — trademark check required before launch).

## Repository layout

```
tessera/
├── Cargo.toml              Rust workspace (pinned toolchain in rust-toolchain.toml)
├── crates/
│   ├── scan/               Read-only scanning engine (this is the product's core)
│   │   ├── src/model.rs        Arena tree + size semantics + invariants
│   │   ├── src/scanner.rs      Parallel, cancellable traversal + aggregation
│   │   ├── src/query.rs        Sorted children, largest files, path lookup
│   │   └── src/platform/       OS adapters (windows.rs, unix.rs)
│   ├── testkit/            Disposable fixtures (exact-size trees, links, sparse, ACLs)
│   └── cli/                tessera-cli: real-disk scans, fixture generation, benchmarks
├── apps/desktop/           (planned) Tauri 2 + React + TypeScript app
├── website/                (planned) Astro static site
├── docs/                   Supporting documentation
└── .github/workflows/      CI (Windows + macOS runners)
```

Planned crates, added only when their phase starts: `categorize` (deterministic,
explainable rules), `cleanup` (review → trash, with revalidation), `history`
(scan summaries), `licensing` (entitlements).

Business logic lives in Rust crates. The desktop frontend renders and sends
narrow, validated commands; it never receives arbitrary filesystem access.

## Scanning engine

### Data model

The scan result is an arena (`Vec<Node>`) indexed by `NodeId` (u32).

* A directory's children are stored **contiguously**, because each listing is
  inserted in one step. `children(id)` is a range, not a list allocation.
* Parents always have smaller ids than children, so a reverse pass over ids is
  a bottom-up traversal (used by `Tree::finalize`).
* Names are stored losslessly as `OsStr` (Windows names may not be valid
  Unicode). Display uses lossy UTF-8; operations use `Tree::path_of`.
* Approximate memory: ~64 bytes per node plus the name. A 1,000,000-entry tree
  is expected to need roughly 100 MB (to be confirmed by benchmark).

### Size semantics

| Field | Files | Directories |
|---|---|---|
| `logical` | content length ("Size") | sum of counted descendants |
| `allocated` | bytes reserved on disk ("Size on disk") | sum of counted descendants |

* Nodes flagged `COUNTED_ELSEWHERE` (extra hard links, revisited directories)
  keep their own sizes but contribute zero to totals: nothing is counted twice.
* `ALLOC_ESTIMATED` marks allocation that was computed rather than reported.
  `SUBTREE_ESTIMATED`, `SUBTREE_INCOMPLETE` and `SUBTREE_CLOUD` summarise
  descendants so the UI can label totals honestly.
* Volume capacity (total/used/free) always comes from the OS, never from the
  sum of files. The difference is presented as "not attributed to files".
* Directory metadata blocks are not included (documented limitation).

### Traversal

```
             jobs (unbounded queue of directories)
  aggregator ─────────────────────────────────────▶ N worker threads
  (caller)   ◀───────────────────────────────────── (read_dir + metadata)
             listings (bounded channel → backpressure)
```

* Workers: fixed pool, default = CPU count clamped to 2..8. No per-directory
  threads.
* The aggregator is the only writer. For each listing it appends entries,
  queues subdirectories, and adds the listing's bytes to every ancestor
  (O(depth) per directory). Totals are therefore correct at every instant and
  the tree can be rendered live via `scan_shared` (`Arc<RwLock<Tree>>`); a
  test checks the invariants continuously during a scan.
* Cancellation is a shared atomic flag checked per job and every 512 entries;
  dropping the channels stops all workers. Partial trees remain consistent.
* Errors are recorded per node (by id, not path) and never abort the scan.

### Platform behaviour

| Concern | Windows | macOS |
|---|---|---|
| Enumeration | FindFirstFileExW via `read_dir`; metadata comes from find data (no extra syscall, no file open) | `read_dir` + `lstat` |
| Links / junctions / mount points | name-surrogate reparse points → `Link`, never followed | symlinks → `Link`; other filesystems not entered (`st_dev`) |
| Allocated size | cluster-rounded estimate; measured via GetCompressedFileSizeW for sparse, compressed, WOF/other reparse files | `st_blocks × 512` (reported) |
| Cloud placeholders | OFFLINE / RECALL_ON_* attributes → `CLOUD_ONLY`, 0 bytes on disk, never opened | `SF_DATALESS` → `CLOUD_ONLY` |
| Hard links | only in `Thorough` mode (opens each file for attributes only) | de-duplicated by (dev, inode) |
| Loops | impossible (no links followed) | also guarded by (dev, inode) of directories |
| Long paths | std uses `\\?\` automatically; our Win32 calls use verbatim paths | n/a |

Known limitations to surface in the UI: APFS clones and snapshots, purgeable
space, NTFS MFT-resident small files (allocation over-estimated), ReFS 128-bit
file ids (Thorough hard-link mode uses 64-bit ids), Windows hard links in the
default mode (notably `C:\Windows\WinSxS`).

An NTFS MFT fast path is explicitly deferred until the portable scanner is
proven (see STATUS.md).

## Desktop application (planned, Phase 3)

* Tauri 2, React, TypeScript (strict), Vite.
* The Rust side owns the scan, the tree and all queries. Frontend commands are
  narrow (`start_scan(root_id)`, `cancel_scan`, `children(node, sort, page)`,
  `treemap(node, width, height, metric)`, `reveal(node)`…) and take node ids,
  not paths, wherever possible.
* Treemap: squarified layout computed in Rust for the visible subtree, with
  tiny items aggregated into labelled "smaller items" rectangles; rendered on a
  single `<canvas>` with high-DPI scaling. Lists are virtualised.

## Cleanup (planned, Phase 4)

Separate crate. Discover → Select → Review → Confirm → Trash → Verify. Native
trash only (no permanent delete in v1). Immediately before each item:
re-resolve the path, compare file identity with the scan, re-check protected
locations, refuse links and anything that cannot be validated. If this cannot
be made safe, v1 ships read-only.
