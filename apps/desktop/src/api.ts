// Typed wrappers around the Rust commands. The interface never sends file
// paths: scans start from a detected drive, the home folder or the native
// folder picker, and everything else is addressed by node id.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'

export type Metric = 'allocated' | 'logical'
export type HardLinks = 'auto' | 'thorough' | 'off'
export type NodeKind = 'dir' | 'file' | 'link' | 'other'

export interface Space {
  total: number
  available: number
  free: number
  used: number
}

export interface Volume {
  mount: string
  label: string | null
  fileSystem: string | null
  kind: 'fixed' | 'removable' | 'network' | 'optical' | 'ramdisk' | 'unknown'
  space: Space | null
  /** The drive the operating system runs from. */
  system: boolean
}

export interface Progress {
  scanId: number
  files: number
  dirs: number
  logical: number
  allocated: number
  issues: number
  pending: number
  elapsedMs: number
}

export interface Summary {
  cancelled: boolean
  elapsedMs: number
  issues: number
  permissionDenied: number
  hardLinks: 'all' | 'partial' | 'none'
  hardLinkMinSize: number | null
  internalErrors: number
}

export type Phase = 'scanning' | 'complete' | 'cancelled' | 'failed'

export interface Status {
  scanId: number
  root: string
  phase: Phase
  progress: Progress
  summary: Summary | null
  error: string | null
  volume: Space | null
}

export interface NodeInfo {
  id: number
  name: string
  kind: NodeKind
  logical: number
  allocated: number
  files: number
  dirs: number
  childCount: number
  modified: number | null
  flags: string[]
}

export interface Crumb {
  id: number
  name: string
}

export interface Details {
  node: NodeInfo
  path: string
  crumbs: Crumb[]
}

export interface Page {
  total: number
  items: NodeInfo[]
}

export interface Tile {
  id: number
  kind: NodeKind | 'more'
  depth: number
  x: number
  y: number
  w: number
  h: number
  size: number
  count: number
  branch: number
  opened: boolean
  pending: boolean
  name: string
}

export interface FileRow {
  node: NodeInfo
  path: string
}

export type SortKey = 'size' | 'name' | 'modified' | 'type'

export interface SearchQuery {
  text: string
  matchPath: boolean
  extensions: string[]
  files: boolean
  folders: boolean
  minSize: number | null
  maxSize: number | null
  modifiedBefore: number | null
  modifiedAfter: number | null
  emptyFolders: boolean
  under: number | null
}

export interface SearchResult {
  total: number
  items: FileRow[]
  containing: number[]
}

export interface ReviewItem {
  id: number
  name: string
  path: string
  kind: NodeKind
  logical: number
  allocated: number
  warning: string | null
}

export interface Review {
  reviewId: number
  items: ReviewItem[]
  rejected: { id: number; path: string; reason: string }[]
  merged: number
  totalLogical: number
  totalAllocated: number
}

export type FailKind =
  | 'inUse'
  | 'permission'
  | 'tooLarge'
  | 'changed'
  | 'protected'
  | 'pathTooLong'
  | 'cloudOffline'
  | 'locked'
  | 'other'

export interface Failure {
  kind: FailKind
  /** The system's own wording, for small print. */
  detail: string
  /** Apps holding it open, when Windows can tell. */
  apps: string[]
  /** For a folder: a few files inside it that are in use. */
  inUseFiles: string[]
}

export interface FailedItem {
  id: number
  path: string
  logical: number
  allocated: number
  failure: Failure
}

export interface CleanupReport {
  runId: number
  method: 'trash' | 'permanent'
  moved: number
  deleted: number
  /** Already gone before Tessera got to them. */
  gone: number
  failed: number
  removedLogical: number
  removedAllocated: number
  stopped: boolean
  notStarted: number
  /** No longer in the cleanup list: moved, deleted or already gone. */
  doneIds: number[]
  failures: FailedItem[]
  freeBefore: number | null
  freeAfter: number | null
}

export interface CleanupProgress {
  done: number
  total: number
  bytes: number
}

export interface PartialResult {
  checked: boolean
  removed: number
  removedLogical: number
  removedAllocated: number
  failed: number
  inUseCount: number
  inUseFiles: string[]
  apps: string[]
}

/** Settings > Background (kept by the backend, which needs them before the window opens). */
export interface BackgroundSettings {
  keepRunning: boolean
  lowSpaceAlerts: boolean
  /** Starts hidden at sign-in (only together with keepRunning). */
  startAtLogin: boolean
}

export interface Suggestion {
  id: number
  group: 'junk' | 'developer' | 'downloads' | 'backups' | 'old' | 'cloud'
  label: string
  reason: string
  safety: 'regenerable' | 'redownloadable' | 'review' | 'useAppTool' | 'staysInCloud'
  safetyText: string
  bytes: number
  name: string
  path: string
}

export interface DupGroup {
  size: number
  reclaimable: number
  hash: string
  copies: FileRow[]
}

export interface DupReport {
  groups: DupGroup[]
  filesCompared: number
  bytesRead: number
  skipped: number
  cancelled: boolean
}

export interface MovedItem {
  index: number
  path: string
  name: string
  bytes: number
  movedAt: number
  state: 'inBin' | 'restored' | 'gone' | 'finder' | 'deleted'
}

export interface AppItem {
  id: string
  name: string
  publisher: string | null
  version: string | null
  installDate: string | null
  measuredBytes: number | null
  reportedBytes: number | null
  location: string | null
  perUser: boolean
  canUninstall: boolean
}

export interface Snapshot {
  root: string
  at: number
  allocated: number
  logical: number
  files: number
  folders: number
  top: [string, number][]
  /** Largest folders at any depth (relative path, bytes); empty in older snapshots. */
  deep: [string, number][]
  /** Folders not in `deep` were smaller than this. */
  deepFloor: number
}

export interface Entitlement {
  pro: boolean
  summary: string
  customerEmail: string | null
  maskedKey: string | null
  testMode: boolean
  lastValidated: number | null
}

export type ScanTarget = { kind: 'volume'; mount: string } | { kind: 'home' } | { kind: 'folder' } | { kind: 'again' }

export const api = {
  /** `storeManaged`: installed from the Microsoft Store, which handles updates. */
  appInfo: () => invoke<{ os: string; version: string; storeManaged: boolean }>('app_info'),
  listVolumes: () => invoke<Volume[]>('list_volumes'),
  startScan: (target: ScanTarget, hardLinks: HardLinks) =>
    invoke<number | null>('start_scan', { target, settings: { hardLinks } }),
  cancelScan: () => invoke<void>('cancel_scan'),
  /** Remembers the hard-link setting for scans started from outside the window. */
  setScanSettings: (hardLinks: HardLinks) => invoke<void>('set_scan_settings', { settings: { hardLinks } }),
  scanStatus: () => invoke<Status | null>('scan_status'),
  nodeDetails: (scanId: number, id: number) => invoke<Details>('node_details', { scanId, id }),
  children: (scanId: number, id: number, metric: Metric, offset: number, limit: number) =>
    invoke<Page>('children', { scanId, id, metric, offset, limit }),
  treemap: (scanId: number, id: number, width: number, height: number, metric: Metric) =>
    invoke<Tile[]>('treemap', { scanId, id, width, height, metric }),
  largestFiles: (scanId: number, under: number | null, metric: Metric, limit: number) =>
    invoke<FileRow[]>('largest_files', { scanId, under, metric, limit }),
  search: (scanId: number, query: SearchQuery, metric: Metric, sort: SortKey, limit: number) =>
    invoke<SearchResult>('search', { scanId, query, metric, sort, limit }),
  smartScan: (scanId: number) => invoke<Suggestion[]>('smart_scan', { scanId }),
  storageHistory: () => invoke<Snapshot[]>('storage_history'),
  clearStorageHistory: () => invoke<void>('clear_storage_history'),
  checkUpdate: () => invoke<{ available: boolean; current: string; latest: string | null }>('check_update'),
  openDownloadPage: () => invoke<void>('open_download_page'),
  freeUpCloud: (scanId: number, ids: number[]) =>
    invoke<{ freed: number; freedBytes: number; skipped: [string, string][] }>('free_up_cloud', { scanId, ids }),
  findDuplicates: (scanId: number) => invoke<DupReport>('find_duplicates', { scanId }),
  cancelDuplicates: () => invoke<void>('cancel_duplicates'),
  recentlyMoved: () => invoke<MovedItem[]>('recently_moved'),
  listApps: (scanId: number | null) => invoke<AppItem[]>('list_apps', { scanId }),
  uninstallApp: (id: string) =>
    invoke<{ kind: 'opened' | 'removed' | 'trashed'; bundleId: string | null; leftovers: [string, number][] }>('uninstall_app', { id }),
  moveLeftovers: (bundleId: string) => invoke<[number, number]>('move_leftovers', { bundleId }),
  exportReport: (scanId: number, kind: 'largest' | 'folder', under: number | null, metric: Metric) =>
    invoke<string | null>('export_report', { scanId, kind, under, metric }),
  /** Opens the system print dialog for the report on screen (Pro). */
  printReport: () => invoke<void>('print_report'),
  putBack: (index: number) => invoke<MovedItem[]>('put_back_item', { index }),
  clearMovedHistory: () => invoke<void>('clear_moved_history'),
  onDupesProgress: (cb: (bytes: number) => void): Promise<UnlistenFn> =>
    listen<number>('dupes-progress', (e) => cb(e.payload)),
  reviewCleanup: (scanId: number, ids: number[]) => invoke<Review>('review_cleanup', { scanId, ids }),
  /** Removes the chosen reviewed items: to the Recycle Bin / Trash, or permanently once confirmed. */
  confirmCleanup: (reviewId: number, ids: number[] | null, permanent: boolean) =>
    invoke<CleanupReport>('confirm_cleanup', { reviewId, ids, permanent }),
  stopCleanup: () => invoke<void>('stop_cleanup'),
  undoCleanup: (runId: number) =>
    invoke<{ restored: number; failed: { path: string; reason: string }[] }>('undo_cleanup', { runId }),
  cleanupExceptInUse: (scanId: number, id: number) => invoke<PartialResult>('cleanup_except_in_use', { scanId, id }),
  openRecycleBin: () => invoke<void>('open_recycle_bin'),
  revealInScan: (scanId: number, path: string) => invoke<void>('reveal_in_scan', { scanId, path }),
  onCleanupProgress: (cb: (p: CleanupProgress) => void): Promise<UnlistenFn> =>
    listen<CleanupProgress>('cleanup-progress', (e) => cb(e.payload)),
  licenseStatus: () => invoke<Entitlement>('license_status'),
  refreshLicense: (force = false) => invoke<Entitlement>('refresh_license', { force }),
  activateLicense: (key: string) => invoke<Entitlement>('activate_license', { key }),
  deactivateLicense: () => invoke<Entitlement>('deactivate_license'),
  /** Opens Dodo Payments' checkout for Tessera Pro in the browser (address fixed by the backend). */
  openPurchasePage: () => invoke<void>('open_purchase_page'),
  revealItem: (scanId: number, id: number) => invoke<void>('reveal_item', { scanId, id }),
  openItem: (scanId: number, id: number) => invoke<void>('open_item', { scanId, id }),
  /** Adds a screen error to the local log (file and folder names are removed by the backend). */
  logError: (message: string) => invoke<void>('log_error', { message }),
  showLogFile: () => invoke<void>('show_log_file'),
  backgroundSettings: () => invoke<BackgroundSettings>('background_settings'),
  setBackgroundSettings: (keepRunning: boolean, lowSpaceAlerts: boolean, startAtLogin: boolean) =>
    invoke<BackgroundSettings>('set_background_settings', { keepRunning, lowSpaceAlerts, startAtLogin }),
  onProgress: (cb: (p: Progress) => void): Promise<UnlistenFn> =>
    listen<Progress>('scan-progress', (e) => cb(e.payload)),
  onFinished: (cb: (scanId: number) => void): Promise<UnlistenFn> =>
    listen<number>('scan-finished', (e) => cb(e.payload)),
  /** A scan started from outside the window: right-click menu, second launch, drop, Finder. */
  onScanStarted: (cb: (scanId: number) => void): Promise<UnlistenFn> =>
    listen<number>('scan-started', (e) => cb(e.payload)),
  /** Something was dropped or opened that cannot be scanned. */
  onScanRefused: (cb: (message: string) => void): Promise<UnlistenFn> =>
    listen<string>('scan-refused', (e) => cb(e.payload)),
  /** Dragging over the window, for the "drop to scan" hint. The drop itself is handled by the backend. */
  onDragHover: (cb: (over: boolean) => void): Promise<UnlistenFn> =>
    getCurrentWebview().onDragDropEvent((e) => cb(e.payload.type === 'enter' || e.payload.type === 'over')),
}

/** Error text from a rejected command, suitable for showing to the user. */
export function errorText(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return 'Something went wrong.'
}
