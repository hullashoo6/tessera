// Screen tests: each screen rendered in a simulated browser with the backend
// replaced by test data. They check what the person sees and what buttons do.
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
  smartScan: vi.fn(),
  findDuplicates: vi.fn(),
  cancelDuplicates: vi.fn(),
  onDupesProgress: vi.fn(() => Promise.resolve(() => undefined)),
  recentlyMoved: vi.fn(),
  putBack: vi.fn(),
  clearMovedHistory: vi.fn(),
  listApps: vi.fn(),
  uninstallApp: vi.fn(),
  moveLeftovers: vi.fn(),
  activateLicense: vi.fn(),
  deactivateLicense: vi.fn(),
  refreshLicense: vi.fn(),
  storageHistory: vi.fn(),
  clearStorageHistory: vi.fn(),
  freeUpCloud: vi.fn(),
  logError: vi.fn(() => Promise.resolve()),
}))
vi.mock('../api', () => ({ api, errorText: (e: unknown) => String(e) }))

import { Apps } from '../components/Apps'
import { Cleanup } from '../components/Cleanup'
import { Duplicates } from '../components/Duplicates'
import { ErrorBoundary } from '../components/ErrorBoundary'
import { History } from '../components/History'
import { ProLicence } from '../components/ProLicence'
import { Restore } from '../components/Restore'
import { Suggestions } from '../components/Suggestions'
import { Welcome } from '../components/Welcome'

const noop = () => undefined
beforeEach(() => vi.clearAllMocks())
afterEach(cleanup)

const sug = (id: number, safety: string, group = 'developer') => ({
  id,
  group,
  label: `item ${id}`,
  reason: 'because',
  safety,
  safetyText: safety,
  bytes: 5_000_000,
  name: `n${id}`,
  path: `C:\\p\\${id}`,
})

describe('Cleanup', () => {
  it('is locked without Pro and offers the licence screen', () => {
    const open = vi.fn()
    render(<Cleanup scanId={1} marked={[3]} metric="allocated" os="windows" pro={false} onOpenSettings={open} onUnmark={noop} onFinished={noop} onError={noop} />)
    expect(screen.getByText('Cleanup is part of Tessera Pro')).toBeTruthy()
    fireEvent.click(screen.getByText('Enter licence key'))
    expect(open).toHaveBeenCalled()
  })
})

describe('Smart Scan', () => {
  it('bulk-adds only items that are recreated automatically, never app-managed ones', async () => {
    api.smartScan.mockResolvedValue([sug(1, 'regenerable'), sug(2, 'review'), sug(3, 'useAppTool'), sug(4, 'redownloadable')])
    const many = vi.fn()
    render(<Suggestions scanId={1} scanning={false} version={0} marked={new Set()} onToggleMark={noop} onMarkMany={many} onOpenCleanup={noop} onError={noop} onDone={noop} />)
    fireEvent.click(await screen.findByText(/Add 2 items recreated automatically/))
    expect(many).toHaveBeenCalledWith([1, 4])
    expect(screen.getByText('Use the app')).toBeTruthy()
    expect(screen.getAllByText('Add to cleanup')).toHaveLength(3)
  })

  it('shows long groups 100 at a time', async () => {
    api.smartScan.mockResolvedValue(Array.from({ length: 250 }, (_, i) => sug(i + 1, 'regenerable')))
    render(<Suggestions scanId={1} scanning={false} version={0} marked={new Set()} onToggleMark={noop} onMarkMany={noop} onOpenCleanup={noop} onError={noop} onDone={noop} />)
    await screen.findByText(/Show 100 more of 250/)
    expect(screen.getAllByText('Add to cleanup')).toHaveLength(100)
  })
})

describe('Duplicates', () => {
  const copy = (id: number, modified: number) => ({ node: { id, name: `c${id}`, modified }, path: `C:\\d\\${id}` })
  const report = { groups: [{ size: 3e6, reclaimable: 6e6, hash: 'abc', copies: [copy(1, 10), copy(2, 20), copy(3, 30)] }], filesCompared: 3, bytesRead: 9e6, skipped: 0, cancelled: false }

  it('keeps the oldest copy and adds the others', async () => {
    api.findDuplicates.mockResolvedValue(report)
    const many = vi.fn()
    render(<Duplicates scanId={1} scanning={false} marked={new Set()} onToggleMark={noop} onMarkMany={many} onOpenCleanup={noop} onError={noop} />)
    fireEvent.click(screen.getByText('Find duplicates'))
    fireEvent.click(await screen.findByText('Keep the oldest'))
    expect(many).toHaveBeenCalledWith([2, 3])
  })

  it('warns when every copy is chosen', async () => {
    api.findDuplicates.mockResolvedValue(report)
    render(<Duplicates scanId={1} scanning={false} marked={new Set([1, 2, 3])} onToggleMark={noop} onMarkMany={noop} onOpenCleanup={noop} onError={noop} />)
    fireEvent.click(screen.getByText('Find duplicates'))
    expect(await screen.findByText(/At least one copy would be lost/)).toBeTruthy()
  })
})

describe('Restore', () => {
  it('offers Put back only for items still in the bin', async () => {
    api.recentlyMoved.mockResolvedValue([
      { index: 0, path: 'C:\\a', name: 'a', bytes: 1, movedAt: 1, state: 'inBin' },
      { index: 1, path: 'C:\\b', name: 'b', bytes: 1, movedAt: 1, state: 'restored' },
      { index: 2, path: 'C:\\c', name: 'c', bytes: 1, movedAt: 1, state: 'gone' },
    ])
    api.putBack.mockResolvedValue([])
    render(<Restore os="windows" onDone={noop} onError={noop} />)
    fireEvent.click(await screen.findByText('Put back'))
    expect(api.putBack).toHaveBeenCalledWith(0)
    expect(screen.getByText('Restored')).toBeTruthy()
    expect(screen.getByText('No longer in the Recycle Bin')).toBeTruthy()
  })
})

describe('Apps', () => {
  it('asks for confirmation before uninstalling, and Store apps are removed', async () => {
    api.listApps.mockResolvedValue([
      { id: 'APPX|game_1', name: 'Game', publisher: 'P', version: '1', installDate: null, measuredBytes: null, reportedBytes: null, location: null, perUser: true, canUninstall: true },
    ])
    api.uninstallApp.mockResolvedValue({ kind: 'removed', bundleId: null, leftovers: [] })
    render(<Apps os="windows" scanId={null} onDone={noop} onError={noop} />)
    fireEvent.click(await screen.findByText('Uninstall…'))
    expect(screen.getByText('Remove Game from this computer?')).toBeTruthy()
    expect(api.uninstallApp).not.toHaveBeenCalled()
    fireEvent.click(screen.getByText('Remove app'))
    await waitFor(() => expect(api.uninstallApp).toHaveBeenCalledWith('APPX|game_1'))
  })

  it('on a Mac, offers to move leftovers after the app goes to the Trash', async () => {
    api.listApps.mockResolvedValue([
      { id: 'APP|/Applications/X.app', name: 'X', publisher: 'com.x', version: '1', installDate: null, measuredBytes: null, reportedBytes: 1e6, location: null, perUser: false, canUninstall: true },
    ])
    api.uninstallApp.mockResolvedValue({ kind: 'trashed', bundleId: 'com.x', leftovers: [['/Users/u/Library/Caches/com.x', 2e6]] })
    render(<Apps os="macos" scanId={null} onDone={noop} onError={noop} />)
    fireEvent.click(await screen.findByText('Uninstall…'))
    fireEvent.click(screen.getByText('Move to Trash'))
    expect(await screen.findByText('Move leftovers to Trash')).toBeTruthy()
  })
})

describe('Tessera Pro licence', () => {
  it('sends the key as typed (the backend trims it) and says test keys only in the test version', async () => {
    api.activateLicense.mockResolvedValue({ pro: true, summary: 'Pro', customerEmail: null, maskedKey: '•••• abcd', testMode: true, lastValidated: 1 })
    const change = vi.fn()
    render(<ProLicence entitlement={{ pro: false, summary: 'Free', customerEmail: null, maskedKey: null, testMode: true, lastValidated: null }} onChange={change} />)
    expect(screen.getByText(/only test licence keys work/)).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Licence key'), { target: { value: '  KEY-123  ' } })
    fireEvent.click(screen.getByText('Activate'))
    await waitFor(() => expect(change).toHaveBeenCalled())
    expect(api.activateLicense).toHaveBeenCalledWith('  KEY-123  ')
  })
})

describe('History', () => {
  it('is locked without Pro', () => {
    render(<History pro={false} onOpenSettings={noop} onError={noop} />)
    expect(screen.getByText('Storage history is part of Tessera Pro')).toBeTruthy()
    expect(api.storageHistory).not.toHaveBeenCalled()
  })

  it('shows growth since about a month ago', async () => {
    const day = 86_400
    api.storageHistory.mockResolvedValue([
      { root: 'C:\\', at: 40 * day, allocated: 120e9, logical: 0, files: 0, folders: 0, top: [['Users', 80e9], ['Games', 20e9]] },
      { root: 'C:\\', at: 10 * day, allocated: 100e9, logical: 0, files: 0, folders: 0, top: [['Users', 70e9]] },
    ])
    render(<History pro onOpenSettings={noop} onError={noop} />)
    expect(await screen.findByText('Games')).toBeTruthy()
    expect(screen.getByText('new since then')).toBeTruthy()
    // The chart is labelled: highest and lowest size, first and latest date.
    const chart = screen.getByRole('img')
    expect(chart.getAttribute('aria-label')).toMatch(/over 2 scans, from .+ to .+/)
    expect(chart.querySelectorAll('.axis text')).toHaveLength(4)
  })
})

describe('Error safety net', () => {
  it('shows Reload instead of a blank window', () => {
    const Boom = () => {
      throw new Error('boom')
    }
    vi.spyOn(console, 'error').mockImplementation(noop)
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    )
    expect(screen.getByText('Reload')).toBeTruthy()
    expect(api.logError).toHaveBeenCalledWith(expect.stringContaining('Error: boom'))
  })
})

describe('Welcome', () => {
  const vol = (mount: string, system: boolean) => ({
    mount,
    label: system ? 'Main' : null,
    fileSystem: 'NTFS',
    kind: 'fixed' as const,
    space: { total: 200e9, available: 50e9, free: 50e9, used: 150e9 },
    system,
  })

  it('offers the main drive first and scans it in one click', () => {
    const scan = vi.fn()
    render(<Welcome os="windows" volumes={[vol('D:\\', false), vol('C:\\', true)]} onScan={scan} onSkip={noop} />)
    expect(screen.getByText(/moves items to the Recycle Bin/)).toBeTruthy()
    fireEvent.click(screen.getByText('Scan main drive'))
    expect(scan).toHaveBeenCalledWith({ kind: 'volume', mount: 'C:\\' })
  })

  it('falls back to the home folder, says Trash on a Mac, and can be skipped', () => {
    const scan = vi.fn()
    const skip = vi.fn()
    render(<Welcome os="macos" volumes={[]} onScan={scan} onSkip={skip} />)
    expect(screen.queryByText('Scan main drive')).toBeNull()
    expect(screen.getByText(/moves items to the Trash/)).toBeTruthy()
    fireEvent.click(screen.getByText('Scan home folder'))
    expect(scan).toHaveBeenCalledWith({ kind: 'home' })
    fireEvent.click(screen.getByText('Choose another drive or folder'))
    expect(skip).toHaveBeenCalled()
  })
})
