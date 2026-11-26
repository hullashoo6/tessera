// Which folders changed between two storage-history snapshots, at any depth.
// Each snapshot keeps its largest folders (`deep`) and the size below which
// folders were not kept (`deepFloor`), so a folder missing from one snapshot
// is known to have been smaller than that floor: its change is then reported
// as "at least" the difference, never guessed.
import type { Snapshot } from './api'

export interface FolderChange {
  /** Path relative to the scanned location. */
  path: string
  then: number | null
  now: number | null
  /** Bytes, positive when the folder grew. */
  delta: number
  /** The true change is at least `delta` (the folder was below the other snapshot's floor). */
  atLeast: boolean
}

const MIN_CHANGE = 1 << 20

function isAncestor(a: string, b: string): boolean {
  return b.length > a.length && b.startsWith(a) && (b[a.length] === '\\' || b[a.length] === '/')
}

/** True when both snapshots carry the deep folder list. */
export function hasDeep(a: Snapshot, b: Snapshot): boolean {
  return (a.deep?.length ?? 0) > 0 && (b.deep?.length ?? 0) > 0
}

/**
 * The folders whose size changed most between `base` and `latest`, the most
 * specific first: a folder is left out when one of its own subfolders in the
 * list accounts for most (75% or more) of its change.
 */
export function deepChanges(latest: Snapshot, base: Snapshot, limit = 15): FolderChange[] {
  const now = new Map(latest.deep ?? [])
  const then = new Map(base.deep ?? [])
  const all: FolderChange[] = []
  for (const path of new Set([...now.keys(), ...then.keys()])) {
    const n = now.get(path)
    const t = then.get(path)
    if (n !== undefined && t !== undefined) {
      all.push({ path, now: n, then: t, delta: n - t, atLeast: false })
    } else if (n !== undefined) {
      // Not among the largest before: it was smaller than base's floor (or did not exist).
      all.push({ path, now: n, then: null, delta: n - base.deepFloor, atLeast: base.deepFloor > 0 })
    } else if (t !== undefined) {
      all.push({ path, now: null, then: t, delta: latest.deepFloor - t, atLeast: latest.deepFloor > 0 })
    }
  }
  const sorted = all.filter((c) => Math.abs(c.delta) >= MIN_CHANGE).sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta))
  const specific = sorted.filter(
    (c) =>
      !sorted.some(
        (d) => isAncestor(c.path, d.path) && Math.sign(d.delta) === Math.sign(c.delta) && Math.abs(d.delta) >= 0.75 * Math.abs(c.delta),
      ),
  )
  return specific.slice(0, limit)
}
