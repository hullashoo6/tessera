// @vitest-environment jsdom
// The cleanup list and its results: selection, the two kinds of removal,
// plain explanations of what was left in place, Try again and Undo. The
// backend is replaced by test data.
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { REVIEW_EXPIRED, explain, filterItems, groupFailures, sortItems } from '../cleanupText'
import type { CleanupReport, FailedItem, ReviewItem } from '../api'

const api = vi.hoisted(() => ({
  reviewCleanup: vi.fn(),
  confirmCleanup: vi.fn(),
  stopCleanup: vi.fn(() => Promise.resolve()),
  undoCleanup: vi.fn(),
  cleanupExceptInUse: vi.fn(),
  openRecycleBin: vi.fn(() => Promise.resolve()),
  revealItem: vi.fn(() => Promise.resolve()),
  onCleanupProgress: vi.fn((_cb: (p: { done: number; total: number; bytes: number }) => void) =>
    Promise.resolve(() => undefined),
  ),
}))
vi.mock('../api', () => ({ api, errorText: (e: unknown) => String(e) }))

import { Cleanup } from '../components/Cleanup'

beforeAll(() => {
  // The virtual list measures itself; jsdom has no layout.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver
})
beforeEach(() => {
  // Reset queued test answers too, so one test's data never leaks into the next.
  vi.resetAllMocks()
  // Defaults once a test's own answers run out (the list is checked again
  // after every cleanup).
  api.reviewCleanup.mockResolvedValue(review([]))
  api.confirmCleanup.mockResolvedValue(report())
  api.undoCleanup.mockResolvedValue({ restored: 0, failed: [] })
  api.stopCleanup.mockResolvedValue(undefined)
  api.openRecycleBin.mockResolvedValue(undefined)
  api.revealItem.mockResolvedValue(undefined)
  api.onCleanupProgress.mockResolvedValue(() => undefined)
})
afterEach(cleanup)

const G = 1024 ** 3
const DOT = '\u00b7'
const ELLIPSIS = '\u2026'

const item = (id: number, bytes: number, kind: 'file' | 'dir' = 'file'): ReviewItem => ({
  id,
  name: `item${id}.bin`,
  path: `C:\\Users\\a\\Downloads\\item${id}.bin`,
  kind: kind as ReviewItem['kind'],
  logical: bytes,
  allocated: bytes,
  warning: null,
})
const review = (items: ReviewItem[], reviewId = 7) => ({
  reviewId,
  items,
  rejected: [],
  merged: 0,
  totalLogical: items.reduce((s, i) => s + i.logical, 0),
  totalAllocated: items.reduce((s, i) => s + i.allocated, 0),
})
const failed = (
  id: number,
  kind: FailedItem['failure']['kind'],
  apps: string[] = [],
  inUseFiles: string[] = [],
): FailedItem => ({
  id,
  path: `C:\\Users\\a\\Downloads\\item${id}.bin`,
  logical: G,
  allocated: G,
  failure: { kind, detail: 'system words', apps, inUseFiles },
})
const report = (over: Partial<CleanupReport> = {}): CleanupReport => ({
  runId: 11,
  method: 'trash',
  moved: 2,
  deleted: 0,
  gone: 0,
  failed: 0,
  removedLogical: 3 * G,
  removedAllocated: 3 * G,
  stopped: false,
  notStarted: 0,
  doneIds: [1, 2],
  failures: [],
  freeBefore: 10 * G,
  freeAfter: 10 * G,
  ...over,
})

function renderCleanup(props: Partial<Parameters<typeof Cleanup>[0]> = {}) {
  const fns = {
    onUnmark: vi.fn(),
    onFinished: vi.fn(),
    onChanged: vi.fn(),
    onRescan: vi.fn(),
    onDone: vi.fn(),
    onError: vi.fn(),
    onOpenSettings: vi.fn(),
  }
  const view = render(<Cleanup scanId={1} marked={[1, 2, 3]} metric="allocated" os="windows" pro {...fns} {...props} />)
  return { ...fns, ...view }
}

const moveDialog = (bin = 'Recycle Bin') =>
  fireEvent.click(within(screen.getByRole('alertdialog')).getByText(`Move to ${bin}`))

describe('cleanup list', () => {
  it('starts with everything selected; unticking changes what the button moves', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G), item(3, 3 * G)]))
    renderCleanup()
    expect(await screen.findByText(`All 3 selected ${DOT} 6.00 GB`)).toBeTruthy()
    expect(screen.getByText('Move 3 items (6.00 GB) to the Recycle Bin')).toBeTruthy()
    fireEvent.click(screen.getByLabelText('Include item3.bin'))
    expect(screen.getByText(`2 of 3 selected ${DOT} 3.00 GB`)).toBeTruthy()
    expect(screen.getByText('Move 2 items (3.00 GB) to the Recycle Bin')).toBeTruthy()
    // Select all again, then none.
    fireEvent.click(screen.getByLabelText('Select all'))
    expect(screen.getByText(`All 3 selected ${DOT} 6.00 GB`)).toBeTruthy()
    fireEvent.click(screen.getByLabelText('Select none'))
    expect(screen.getByText(`0 of 3 selected ${DOT} 0 bytes`)).toBeTruthy()
    expect((screen.getByText('Move to the Recycle Bin') as HTMLButtonElement).disabled).toBe(true)
  })

  it('takes the selected items off the list without touching files', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G), item(3, 3 * G)]))
    const { onUnmark } = renderCleanup()
    await screen.findByText(`All 3 selected ${DOT} 6.00 GB`)
    fireEvent.click(screen.getByLabelText('Include item2.bin'))
    fireEvent.click(screen.getByText('Take 2 off the list'))
    expect(onUnmark).toHaveBeenCalledWith([3, 1])
    fireEvent.click(screen.getByLabelText('Take item2.bin off the list'))
    expect(onUnmark).toHaveBeenLastCalledWith([2])
    expect(api.confirmCleanup).not.toHaveBeenCalled()
  })

  it('filters by name or folder, and the buttons act on what is shown', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G), item(13, 3 * G)]))
    renderCleanup({ marked: [1, 2, 13] })
    await screen.findByText(`All 3 selected ${DOT} 6.00 GB`)
    fireEvent.change(screen.getByLabelText('Filter the cleanup list'), { target: { value: 'item1' } })
    expect(screen.getByText(/Showing 2 of 3/)).toBeTruthy()
    expect(screen.getByText('Move 2 items (4.00 GB) to the Recycle Bin')).toBeTruthy()
  })

  it('does not check the files again when items are only taken off the list', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G), item(3, 3 * G)]))
    const { rerender, onUnmark, onFinished, onChanged, onRescan, onDone, onError, onOpenSettings } = renderCleanup()
    const fns = { onUnmark, onFinished, onChanged, onRescan, onDone, onError, onOpenSettings }
    await screen.findByText(`All 3 selected ${DOT} 6.00 GB`)
    rerender(<Cleanup scanId={1} marked={[1, 3]} metric="allocated" os="windows" pro {...fns} />)
    expect(await screen.findByText(`All 2 selected ${DOT} 4.00 GB`)).toBeTruthy()
    expect(api.reviewCleanup).toHaveBeenCalledTimes(1)
  })
})

describe('removing', () => {
  it('moves to the Recycle Bin after one confirmation, then offers Undo and the Recycle Bin', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G)]))
    api.confirmCleanup.mockResolvedValue(report())
    api.undoCleanup.mockResolvedValue({ restored: 2, failed: [] })
    const { onFinished, onRescan } = renderCleanup({ marked: [1, 2] })
    fireEvent.click(await screen.findByText('Move 2 items (3.00 GB) to the Recycle Bin'))
    expect(within(screen.getByRole('alertdialog')).getByText(/You can undo this straight afterwards/)).toBeTruthy()
    moveDialog()
    expect(await screen.findByText('Moved 2 items to the Recycle Bin')).toBeTruthy()
    expect(api.confirmCleanup).toHaveBeenCalledWith(7, [2, 1], false)
    expect(onFinished).toHaveBeenCalled()
    expect(screen.getByText(/gets this space back when the Recycle Bin is emptied/)).toBeTruthy()
    fireEvent.click(screen.getByText('Open Recycle Bin'))
    expect(api.openRecycleBin).toHaveBeenCalled()
    fireEvent.click(screen.getByText('Undo: put them all back'))
    expect(await screen.findByText(/Put back 2 items/)).toBeTruthy()
    expect(api.undoCleanup).toHaveBeenCalledWith(11)
    fireEvent.click(screen.getByText('Scan again'))
    expect(onRescan).toHaveBeenCalled()
  })

  it('never calls something Windows deleted "moved": says so, and offers Undo only for what is in the bin', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G)]))
    api.confirmCleanup.mockResolvedValue(report({ moved: 1, deleted: 1, doneIds: [1, 2] }))
    renderCleanup({ marked: [1, 2] })
    fireEvent.click(await screen.findByText('Move 2 items (3.00 GB) to the Recycle Bin'))
    moveDialog()
    expect(await screen.findByText('Moved 1 item to the Recycle Bin')).toBeTruthy()
    expect(screen.getByRole('alert').textContent).toMatch(/Windows deleted 1 item permanently.*cannot be put back/)
    expect(screen.getByText('Undo: put it back')).toBeTruthy()
  })

  it('if the list changed since it was checked, checks again and finishes by itself', async () => {
    api.reviewCleanup.mockResolvedValueOnce(review([item(1, G), item(2, 2 * G)], 7))
    api.reviewCleanup.mockResolvedValueOnce(review([item(1, G), item(2, 2 * G)], 8))
    api.confirmCleanup.mockRejectedValueOnce(REVIEW_EXPIRED)
    api.confirmCleanup.mockResolvedValueOnce(report())
    const { onError } = renderCleanup({ marked: [1, 2] })
    fireEvent.click(await screen.findByText('Move 2 items (3.00 GB) to the Recycle Bin'))
    moveDialog()
    expect(await screen.findByText('Moved 2 items to the Recycle Bin')).toBeTruthy()
    expect(api.confirmCleanup).toHaveBeenNthCalledWith(1, 7, [2, 1], false)
    expect(api.confirmCleanup).toHaveBeenNthCalledWith(2, 8, [2, 1], false)
    expect(onError).not.toHaveBeenCalled()
  })

  it('deletes permanently only after the person ticks that it cannot be undone', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G)]))
    api.confirmCleanup.mockResolvedValue(
      report({ method: 'permanent', moved: 0, deleted: 1, doneIds: [1], removedLogical: G, removedAllocated: G, freeAfter: 11 * G }),
    )
    renderCleanup({ marked: [1] })
    fireEvent.click(await screen.findByText(`Delete permanently${ELLIPSIS}`))
    const dialog = screen.getByRole('alertdialog')
    expect(within(dialog).getByText(/nobody can put them back/)).toBeTruthy()
    const go = within(dialog).getByText('Delete permanently') as HTMLButtonElement
    expect(go.disabled).toBe(true)
    fireEvent.click(within(dialog).getByLabelText(/I understand this cannot be undone/))
    expect(go.disabled).toBe(false)
    fireEvent.click(go)
    expect(await screen.findByText('Deleted 1 item')).toBeTruthy()
    expect(api.confirmCleanup).toHaveBeenCalledWith(7, [1], true)
    expect(screen.getByText(/Free space went up by 1.00 GB/)).toBeTruthy()
    expect(screen.queryByText(/Undo/)).toBeNull()
  })

  it('shows progress with a Stop button while it runs', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G), item(2, 2 * G)]))
    type P = { done: number; total: number; bytes: number; current?: string | null }
    let progress: ((p: P) => void) | null = null
    api.onCleanupProgress.mockImplementation((cb: (p: P) => void) => {
      progress = cb
      return Promise.resolve(() => undefined)
    })
    let finish: (r: CleanupReport) => void = () => undefined
    api.confirmCleanup.mockReturnValue(new Promise((r) => (finish = r)))
    renderCleanup({ marked: [1, 2] })
    fireEvent.click(await screen.findByText('Move 2 items (3.00 GB) to the Recycle Bin'))
    moveDialog()
    await waitFor(() => expect(progress).not.toBeNull())
    progress!({ done: 1, total: 2, bytes: G, current: 'big-folder' })
    expect(await screen.findByText(`1 of 2 items ${DOT} 1.00 GB moved`)).toBeTruthy()
    // The item being worked on is named, so a long one is visibly not stuck.
    expect(screen.getByText('big-folder')).toBeTruthy()
    fireEvent.click(screen.getByText('Stop'))
    expect(api.stopCleanup).toHaveBeenCalled()
    expect(screen.getByText('Stopping…')).toBeTruthy()
    finish(report({ moved: 1, stopped: true, notStarted: 1, doneIds: [1] }))
    expect(await screen.findByText(/1 item was not started and is still in your cleanup list/)).toBeTruthy()
  })
})

describe('items left in place', () => {
  it('names the app holding files open, and Try again checks and moves just those', async () => {
    api.reviewCleanup.mockResolvedValueOnce(review([item(1, G), item(2, 2 * G), item(3, 3 * G, 'dir')]))
    // After a cleanup the list is checked again straight away.
    api.reviewCleanup.mockResolvedValueOnce(review([item(2, 2 * G), item(3, 3 * G, 'dir')], 8))
    api.confirmCleanup.mockResolvedValueOnce(
      report({
        moved: 1,
        failed: 2,
        doneIds: [1],
        failures: [failed(2, 'inUse', ['Microsoft Word']), failed(3, 'inUse', ['Microsoft Word'], ['C:\\x\\cache\\lock.db'])],
      }),
    )
    const { rerender, onUnmark, onFinished, onChanged, onRescan, onDone, onError, onOpenSettings } = renderCleanup()
    const fns = { onUnmark, onFinished, onChanged, onRescan, onDone, onError, onOpenSettings }
    fireEvent.click(await screen.findByText('Move 3 items (6.00 GB) to the Recycle Bin'))
    moveDialog()
    expect(await screen.findByText('In use by Microsoft Word')).toBeTruthy()
    expect(screen.getByText(/Close it, then choose Try again/)).toBeTruthy()
    expect(screen.getByText('Open inside: lock.db')).toBeTruthy()
    // Only the folder offers to move the rest.
    expect(screen.getAllByText('Move the rest')).toHaveLength(1)

    // The app took item 1 off the list (it moved); Try again re-checks the rest.
    rerender(<Cleanup scanId={1} marked={[2, 3]} metric="allocated" os="windows" pro {...fns} />)
    api.confirmCleanup.mockResolvedValueOnce(report({ moved: 2, doneIds: [2, 3] }))
    fireEvent.click(screen.getByText('Try again'))
    await waitFor(() => expect(api.confirmCleanup).toHaveBeenLastCalledWith(8, [2, 3], false))
  })

  it('moves the rest of a folder whose files are open, and says what is still open', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(3, 3 * G, 'dir')]))
    api.confirmCleanup.mockResolvedValue(
      report({ moved: 0, failed: 1, doneIds: [], failures: [failed(3, 'inUse', ['Google Chrome'], ['C:\\c\\lock'])] }),
    )
    api.cleanupExceptInUse.mockResolvedValue({
      checked: true,
      removed: 40,
      removedLogical: 2 * G,
      removedAllocated: 2 * G,
      failed: 0,
      inUseCount: 1,
      inUseFiles: ['C:\\c\\lock'],
      apps: ['Google Chrome'],
    })
    const { onChanged } = renderCleanup({ marked: [3] })
    fireEvent.click(await screen.findByText('Move 1 item (3.00 GB) to the Recycle Bin'))
    moveDialog()
    fireEvent.click(await screen.findByText('Move the rest'))
    expect(
      await screen.findByText(/Moved 40 items \(2.00 GB\) from inside it. 1 file is still open in Google Chrome/),
    ).toBeTruthy()
    expect(api.cleanupExceptInUse).toHaveBeenCalledWith(1, 3)
    expect(onChanged).toHaveBeenCalled()
  })

  it('offers permanent deletion for items too large for the Recycle Bin, after the warning', async () => {
    api.reviewCleanup.mockResolvedValueOnce(review([item(1, G)]))
    api.reviewCleanup.mockResolvedValueOnce(review([item(1, G)], 9))
    api.confirmCleanup.mockResolvedValueOnce(report({ moved: 0, failed: 1, doneIds: [], failures: [failed(1, 'tooLarge')] }))
    renderCleanup({ marked: [1] })
    fireEvent.click(await screen.findByText('Move 1 item (1.00 GB) to the Recycle Bin'))
    moveDialog()
    expect(await screen.findByText('Too large for the Recycle Bin')).toBeTruthy()
    api.confirmCleanup.mockResolvedValueOnce(report({ method: 'permanent', moved: 0, deleted: 1, doneIds: [1] }))
    fireEvent.click(screen.getByText(`Delete it permanently${ELLIPSIS}`))
    const dialog = screen.getByRole('alertdialog')
    fireEvent.click(within(dialog).getByLabelText(/I understand/))
    fireEvent.click(within(dialog).getByText('Delete permanently'))
    await waitFor(() => expect(api.confirmCleanup).toHaveBeenLastCalledWith(9, [1], true))
  })

  it('uses Mac words on a Mac', async () => {
    api.reviewCleanup.mockResolvedValue(review([item(1, G)]))
    api.confirmCleanup.mockResolvedValue(report({ moved: 0, failed: 1, doneIds: [], failures: [failed(1, 'permission')] }))
    renderCleanup({ marked: [1], os: 'macos' })
    fireEvent.click(await screen.findByText('Move 1 item (1.00 GB) to the Trash'))
    moveDialog('Trash')
    expect(await screen.findByText('macOS did not allow it')).toBeTruthy()
    expect(screen.getByText(/Full Disk Access/)).toBeTruthy()
    expect(screen.getByText('Show in Finder')).toBeTruthy()
  })
})

describe('explanations', () => {
  it('groups by cause, people-fixable causes first, and files in use by app', () => {
    const groups = groupFailures(
      [
        failed(1, 'other'),
        failed(2, 'inUse', ['Word']),
        failed(3, 'tooLarge'),
        failed(4, 'inUse', ['Excel']),
        failed(5, 'inUse', ['Word']),
      ],
      'windows',
    )
    expect(groups.map((g) => g.title)).toEqual([
      'In use by Word',
      'In use by Excel',
      'Too large for the Recycle Bin',
      'Could not be removed',
    ])
    expect(groups.find((g) => g.title === 'In use by Word')!.items.map((i) => i.id)).toEqual([2, 5])
  })

  it('says something useful even when Windows cannot name the app', () => {
    expect(explain('inUse', [], 'windows').title).toBe('In use by another app')
    expect(explain('inUse', ['Word', 'Excel', 'Teams'], 'windows').title).toBe('In use by Word, Excel and Teams')
    expect(explain('tooLarge', [], 'macos').title).toBe('Too large for the Trash')
  })

  it('sorts and filters the list', () => {
    const list = [item(1, 5), item(2, 9), item(3, 1)]
    expect(sortItems(list, 'size', 'allocated').map((i) => i.id)).toEqual([2, 1, 3])
    expect(sortItems(list, 'name', 'allocated').map((i) => i.id)).toEqual([1, 2, 3])
    expect(filterItems(list, ' ITEM2 ').map((i) => i.id)).toEqual([2])
  })
})
