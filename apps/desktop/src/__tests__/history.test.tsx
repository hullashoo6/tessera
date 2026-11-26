import { describe, expect, it } from 'vitest'
import type { Snapshot } from '../api'
import { deepChanges, hasDeep } from '../historyChanges'

const GB = 1024 ** 3
const snap = (at: number, deep: [string, number][], deepFloor = 0): Snapshot => ({
  root: 'C:\\',
  at,
  allocated: 0,
  logical: 0,
  files: 0,
  folders: 0,
  top: [],
  deep,
  deepFloor,
})

describe('Folder changes between snapshots', () => {
  it('names the most specific folder that grew, not every folder above it', () => {
    const base = snap(1, [
      ['Users', 50 * GB],
      ['Users\\a', 40 * GB],
      ['Users\\a\\Videos', 10 * GB],
      ['Games', 20 * GB],
    ])
    const latest = snap(2, [
      ['Users', 70 * GB],
      ['Users\\a', 60 * GB],
      ['Users\\a\\Videos', 29 * GB],
      ['Games', 15 * GB],
    ])
    const changes = deepChanges(latest, base)
    expect(changes.map((c) => c.path)).toEqual(['Users\\a\\Videos', 'Games'])
    expect(changes[0]!.delta).toBe(19 * GB)
    expect(changes[1]!.delta).toBe(-5 * GB)
  })

  it('keeps a parent when its growth is spread over many subfolders', () => {
    const base = snap(1, [['Projects', 10 * GB], ['Projects\\x', 2 * GB]])
    const latest = snap(2, [['Projects', 20 * GB], ['Projects\\x', 4 * GB]])
    expect(deepChanges(latest, base).map((c) => c.path)).toEqual(['Projects', 'Projects\\x'])
  })

  it('reports "at least" for folders that were below the other snapshot’s floor', () => {
    const base = snap(1, [['Users', 50 * GB]], 3 * GB)
    const latest = snap(2, [['Users', 58 * GB], ['Downloads', 8 * GB]], 3 * GB)
    const dl = deepChanges(latest, base).find((c) => c.path === 'Downloads')!
    expect(dl.then).toBeNull()
    expect(dl.atLeast).toBe(true)
    expect(dl.delta).toBe(5 * GB) // 8 GB now, under 3 GB before
  })

  it('a folder missing from a complete snapshot is new', () => {
    const base = snap(1, [['Users', 5 * GB]], 0)
    const latest = snap(2, [['Users', 5 * GB], ['New', 2 * GB]], 0)
    const added = deepChanges(latest, base)
    expect(added).toHaveLength(1)
    expect(added[0]).toMatchObject({ path: 'New', atLeast: false, delta: 2 * GB })
  })

  it('needs the deep list in both snapshots', () => {
    expect(hasDeep(snap(1, []), snap(2, [['a', 1]]))).toBe(false)
    expect(hasDeep(snap(1, [['a', 1]]), snap(2, [['a', 1]]))).toBe(true)
  })
})
