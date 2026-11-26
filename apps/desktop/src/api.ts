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
