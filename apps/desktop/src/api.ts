// Typed wrappers around the Rust commands. The interface never sends file
// paths: scans start from a detected drive, the home folder or the native
// folder picker, and everything else is addressed by node id.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

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
  rejected: { path: string; reason: string }[]
  merged: number
  totalLogical: number
  totalAllocated: number
}

export interface CleanupReport {
  moved: number
  failed: number
  movedLogical: number
  movedAllocated: number
  outcomes: { id: number; path: string; logical: number; allocated: number; ok: boolean; reason: string | null }[]
  freeBefore: number | null
  freeAfter: number | null
}

export interface Suggestion {
  id: number
  group: 'junk' | 'developer' | 'downloads'
  label: string
  reason: string
  safety: 'regenerable' | 'redownloadable' | 'review' | 'useAppTool'
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
  state: 'inBin' | 'restored' | 'gone' | 'finder'
}

export interface Entitlement {
  pro: boolean
  summary: string
  customerEmail: string | null
  maskedKey: string | null
  testMode: boolean
  lastValidated: number | null
}

export type ScanTarget = { kind: 'volume'; mount: string } | { kind: 'home' } | { kind: 'folder' }

export const api = {
  appInfo: () => invoke<{ os: string; version: string }>('app_info'),
  listVolumes: () => invoke<Volume[]>('list_volumes'),
  startScan: (target: ScanTarget, hardLinks: HardLinks) =>
    invoke<number | null>('start_scan', { target, settings: { hardLinks } }),
  cancelScan: () => invoke<void>('cancel_scan'),
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
  findDuplicates: (scanId: number) => invoke<DupReport>('find_duplicates', { scanId }),
  cancelDuplicates: () => invoke<void>('cancel_duplicates'),
  recentlyMoved: () => invoke<MovedItem[]>('recently_moved'),
  putBack: (index: number) => invoke<MovedItem[]>('put_back_item', { index }),
  clearMovedHistory: () => invoke<void>('clear_moved_history'),
  onDupesProgress: (cb: (bytes: number) => void): Promise<UnlistenFn> =>
    listen<number>('dupes-progress', (e) => cb(e.payload)),
  reviewCleanup: (scanId: number, ids: number[]) => invoke<Review>('review_cleanup', { scanId, ids }),
  confirmCleanup: (reviewId: number) => invoke<CleanupReport>('confirm_cleanup', { reviewId }),
  licenseStatus: () => invoke<Entitlement>('license_status'),
  refreshLicense: (force = false) => invoke<Entitlement>('refresh_license', { force }),
  activateLicense: (key: string) => invoke<Entitlement>('activate_license', { key }),
  deactivateLicense: () => invoke<Entitlement>('deactivate_license'),
  revealItem: (scanId: number, id: number) => invoke<void>('reveal_item', { scanId, id }),
  openItem: (scanId: number, id: number) => invoke<void>('open_item', { scanId, id }),
  onProgress: (cb: (p: Progress) => void): Promise<UnlistenFn> =>
    listen<Progress>('scan-progress', (e) => cb(e.payload)),
  onFinished: (cb: (scanId: number) => void): Promise<UnlistenFn> =>
    listen<number>('scan-finished', (e) => cb(e.payload)),
}

/** Error text from a rejected command, suitable for showing to the user. */
export function errorText(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return 'Something went wrong.'
}
