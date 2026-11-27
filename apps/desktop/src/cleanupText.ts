// Plain-language explanations for cleanup results, and the sorting and
// filtering of the cleanup list. Kept apart from the screen so they can be
// tested on their own.

import type { FailKind, FailedItem, Metric, ReviewItem } from './api'

export type SortKey = 'size' | 'name' | 'folder'

/**
 * Sent by the backend when the list checked on screen is no longer the one
 * it holds; the screen then checks again and retries once by itself. Must
 * match REVIEW_EXPIRED in src-tauri/src/lib.rs (a Rust test compares them).
 */
export const REVIEW_EXPIRED = 'The list changed since it was checked. Check it again, then try once more.'

export interface FailGroup {
  key: string
  kind: FailKind
  title: string
  text: string
  items: FailedItem[]
}

/** Order in which problems are listed: the ones people can fix first. */
const ORDER: FailKind[] = [
  'inUse',
  'tooLarge',
  'changed',
  'cloudOffline',
  'permission',
  'locked',
  'pathTooLong',
  'protected',
  'other',
]

function joinNames(names: string[]): string {
  if (names.length <= 1) return names[0] ?? ''
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`
}

/** Title and advice for one kind of problem. */
export function explain(kind: FailKind, apps: string[], os: string): { title: string; text: string } {
  const mac = os === 'macos'
  const bin = mac ? 'Trash' : 'Recycle Bin'
  const fm = mac ? 'Finder' : 'File Explorer'
  const system = mac ? 'macOS' : 'Windows'
  switch (kind) {
    case 'inUse':
      return apps.length > 0
        ? {
            title: `In use by ${joinNames(apps)}`,
            text: `Close ${apps.length === 1 ? 'it' : 'them'}, then choose Try again. For a folder you can also move everything in it except the files that are open.`,
          }
        : {
            title: 'In use by another app',
            text: 'An open app is using these. Close the app (or restart the computer), then choose Try again.',
          }
    case 'tooLarge':
      return {
        title: `Too large for the ${bin}`,
        text: `The ${bin} can't hold these: they are bigger than it allows, or this drive has no ${bin}. Tessera never deletes permanently without asking, so they were left in place. Delete them permanently if you are sure.`,
      }
    case 'changed':
      return {
        title: 'Changed after you chose them',
        text: 'Tessera leaves anything that changed since it was chosen, in case it is now something different. Choose Try again to check them again.',
      }
    case 'cloudOffline':
      return {
        title: 'Cloud app not running',
        text: 'These are cloud files and their app (for example OneDrive) is not running or is paused. Start it, then choose Try again.',
      }
    case 'permission':
      return {
        title: `${system} did not allow it`,
        text: mac
          ? `They may belong to another user or need your password. Remove them in ${fm}, which can ask for it. Items in privacy-protected places may need Full Disk Access for Tessera (System Settings > Privacy & Security).`
          : `They may belong to another user or need administrator rights. Remove them in ${fm}, which can ask for permission.`,
      }
    case 'locked':
      return mac
        ? { title: 'Locked', text: `Unlock them in ${fm} (File > Get Info, untick Locked), then choose Try again.` }
        : { title: 'Write-protected', text: 'The drive or file is write-protected. Remove the protection, then choose Try again.' }
    case 'pathTooLong':
      return {
        title: `Name too long for the ${bin}`,
        text: `Its folder path is longer than the ${bin} accepts. Shorten a folder name on the way, or remove it in ${fm}.`,
      }
    case 'protected':
      return {
        title: 'Protected by Tessera',
        text: 'Tessera keeps system and app locations safe and will not remove these.',
      }
    default:
      return { title: 'Could not be removed', text: `${system} gave the reason shown under each item.` }
  }
}

/** Groups failures by cause (and, for files in use, by the apps holding them). */
export function groupFailures(failures: FailedItem[], os: string): FailGroup[] {
  const groups = new Map<string, FailGroup>()
  for (const f of failures) {
    const apps = f.failure.kind === 'inUse' ? [...f.failure.apps].sort() : []
    const key = `${f.failure.kind}:${apps.join('|')}`
    let g = groups.get(key)
    if (!g) {
      g = { key, kind: f.failure.kind, ...explain(f.failure.kind, apps, os), items: [] }
      groups.set(key, g)
    }
    g.items.push(f)
  }
  return [...groups.values()].sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind))
}

/** Folder part of a path, for the second line of a row. */
export function parentPath(path: string): string {
  const i = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
  return i > 0 ? path.slice(0, i) : path
}

/** Short label for a review warning (the full text is shown on hover). */
export function warningLabel(warning: string): string {
  const w = warning.toLowerCase()
  if (w.includes('hard link')) return 'Hard links'
  if (w.includes('online-only') || w.includes('cloud')) return 'Cloud files'
  if (w.includes('could not be read')) return 'Partly unreadable'
  return 'Note'
}

export function filterItems(items: ReviewItem[], query: string): ReviewItem[] {
  const q = query.trim().toLowerCase()
  if (!q) return items
  return items.filter((it) => it.path.toLowerCase().includes(q))
}

export function sortItems(items: ReviewItem[], sort: SortKey, metric: Metric): ReviewItem[] {
  const bytes = (it: ReviewItem) => (metric === 'allocated' ? it.allocated : it.logical)
  const out = [...items]
  if (sort === 'size') out.sort((a, b) => bytes(b) - bytes(a) || a.path.localeCompare(b.path))
  else if (sort === 'name') out.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }))
  else out.sort((a, b) => a.path.localeCompare(b.path, undefined, { sensitivity: 'base' }))
  return out
}
