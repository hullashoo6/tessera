import { useCallback, useEffect, useRef, useState } from 'react'
import { api, errorText, type Crumb, type Entitlement, type ScanTarget, type Status, type Tile, type Volume } from './api'
import { Apps } from './components/Apps'
import { Cleanup } from './components/Cleanup'
import { ContextMenu, type MenuAction } from './components/ContextMenu'
import { Details, type ItemCommand } from './components/Details'
import { Duplicates } from './components/Duplicates'
import { History } from './components/History'
import { LargestFiles } from './components/LargestFiles'
import { Mark } from './components/Mark'
import { Overview } from './components/Overview'
import { SettingsView } from './components/SettingsView'
import { Report } from './components/Report'
import { Restore } from './components/Restore'
import { Search } from './components/Search'
import { Suggestions } from './components/Suggestions'
import { Treemap, type Highlight } from './components/Treemap'
import { Welcome } from './components/Welcome'
import { baseName, formatBytes, formatDuration, platformNames, plural, setPlatform } from './format'
import { setHighContrast, type Theme } from './palette'
import { loadSettings, saveSettings, type Settings } from './settings'

type View =
  | 'overview'
  | 'suggestions'
  | 'duplicates'
  | 'map'
  | 'largest'
  | 'search'
  | 'cleanup'
  | 'restore'
  | 'history'
  | 'apps'
  | 'settings'
  | 'report'

const isDriveRoot = (p: string) => /^[A-Za-z]:\\?$/.test(p) || p === '/'
const CONTRAST_QUERY = '(prefers-contrast: more), (forced-colors: active)'
/** Must match `active()` in src-tauri/src/lib.rs. */
const SCAN_REPLACED = 'This scan is no longer available. Start a new scan.'

export function App() {
  const [settings, setSettings] = useState<Settings>(loadSettings)
  const [systemDark, setSystemDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches)
  const theme: Theme = settings.theme === 'system' ? (systemDark ? 'dark' : 'light') : settings.theme
  // "Increase contrast" on macOS, contrast themes on Windows.
  const [contrast, setContrast] = useState(() => window.matchMedia(CONTRAST_QUERY).matches)
  setHighContrast(contrast) // before children draw; idempotent

  const [info, setInfo] = useState({ os: 'windows', version: '', storeManaged: false })
  const [volumes, setVolumes] = useState<Volume[] | null>(null)
  const [status, setStatus] = useState<Status | null>(null)
  const [view, setView] = useState<View>('overview')
  const [viewRoot, setViewRoot] = useState(0)
  const [crumbs, setCrumbs] = useState<Crumb[]>([])
  const [selected, setSelected] = useState<number | null>(null)
  const [version, setVersion] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [toast, setToast] = useState<string | null>(null)
  const [update, setUpdate] = useState<string | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number; tile: Tile } | null>(null)
  const [highlight, setHighlight] = useState<Highlight | null>(null)
  const [marked, setMarked] = useState<Set<number>>(() => new Set())
  const [entitlement, setEntitlement] = useState<Entitlement | null>(null)
  const markMany = (ids: number[]) =>
    setMarked((m) => {
      const next = new Set(m)
      for (const id of ids) next.add(id)
      return next
    })
  const toggleMark = (id: number) =>
    setMarked((m) => {
      const next = new Set(m)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  const lastRefresh = useRef(0)

  const scanId = status?.scanId ?? null
  const names = platformNames(info.os)
  // The backend says a scan is "no longer available" only when a newer scan
  // has replaced it (for example one started from File Explorer or Finder).
  // A request still in flight for the old scan is not an error to show: the
  // window just catches up with the new scan.
  const onError = useCallback((m: string) => {
    if (m === SCAN_REPLACED) {
      api
        .scanStatus()
        .then(setStatus)
        .catch(() => undefined)
      return
    }
    setError(m)
  }, [])
  // The first-run welcome screen is shown until the first scan or a skip.
  const markWelcomed = useCallback(
    () =>
      setSettings((cur) => {
        if (cur.welcomed) return cur
        const next = { ...cur, welcomed: true }
        saveSettings(next)
        return next
      }),
    [],
  )

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    const on = (e: MediaQueryListEvent) => setSystemDark(e.matches)
    mq.addEventListener('change', on)
    const hc = window.matchMedia(CONTRAST_QUERY)
    const onContrast = (e: MediaQueryListEvent) => setContrast(e.matches)
    hc.addEventListener('change', onContrast)
    return () => {
      mq.removeEventListener('change', on)
      hc.removeEventListener('change', onContrast)
    }
  }, [])

  const refreshDrives = useCallback(() => {
    api.listVolumes().then(setVolumes).catch((e) => onError(errorText(e)))
  }, [onError])

  useEffect(() => {
    api
      .appInfo()
      .then((i) => {
        setPlatform(i.os)
        setInfo(i)
      })
      .catch((e) => onError(errorText(e)))
    refreshDrives()
    api.licenseStatus().then(setEntitlement).catch(() => undefined)
    // Optional update check (Settings > Updates); failures are silent.
    if (loadSettings().checkUpdates) {
      api
        .checkUpdate()
        .then((u) => u.available && u.latest && setUpdate(u.latest))
        .catch(() => undefined)
    }
    // Re-confirms the licence in the background when due; never blocks.
    api.refreshLicense().then(setEntitlement).catch(() => undefined)
    api
      .scanStatus()
      .then((s) => {
        setStatus(s)
        if (s) {
          // A scan already exists (a folder given at launch, or the window was reloaded).
          setView('map')
          markWelcomed()
        }
      })
      .catch((e) => onError(errorText(e)))
  }, [refreshDrives, onError, markWelcomed])

  // Live progress, and periodic map refreshes while a scan runs.
  useEffect(() => {
    const unProgress = api.onProgress((p) => {
      setStatus((s) => (s && s.scanId === p.scanId ? { ...s, progress: p } : s))
      const now = performance.now()
      if (now - lastRefresh.current > 800) {
        lastRefresh.current = now
        setVersion((v) => v + 1)
      }
    })
    const unFinished = api.onFinished(() => {
      api.scanStatus().then(setStatus).catch((e) => onError(errorText(e)))
      refreshDrives()
      setVersion((v) => v + 1)
    })
    return () => {
      void unProgress.then((f) => f())
      void unFinished.then((f) => f())
    }
  }, [onError, refreshDrives])

  useEffect(() => {
    if (scanId === null) return
    let live = true
    api
      .nodeDetails(scanId, viewRoot)
      .then((d) => live && setCrumbs(d.crumbs))
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [scanId, viewRoot, onError])

  useEffect(() => {
    if (!toast) return
    const t = window.setTimeout(() => setToast(null), 1800)
    return () => window.clearTimeout(t)
  }, [toast])

  const updateSettings = (s: Settings) => {
    if (s.hardLinks !== settings.hardLinks) void api.setScanSettings(s.hardLinks).catch(() => undefined)
    setSettings(s)
    saveSettings(s)
  }

  // Clears everything that belonged to the previous scan and opens the map.
  const showNewScan = useCallback(() => {
    setViewRoot(0)
    setSelected(null)
    setCrumbs([])
    setHighlight(null)
    setMarked(new Set())
    setView('map')
    return api.scanStatus().then(setStatus)
  }, [])

  const startScan = (target: ScanTarget) => {
    setError(null)
    api
      .startScan(target, settings.hardLinks)
      .then((id) => {
        if (id === null) return // folder picker dismissed
        return showNewScan()
      })
      .catch((e) => onError(errorText(e)))
  }

  // Scans started from outside the window (right-click menu, a second launch,
  // a folder dropped on the window, or Finder), and the drop hint.
  const [dropHint, setDropHint] = useState(false)
  useEffect(() => {
    const unStarted = api.onScanStarted(() => {
      setError(null)
      markWelcomed()
      showNewScan().catch((e) => onError(errorText(e)))
    })
    const unRefused = api.onScanRefused((m) => onError(m))
    const unHover = api.onDragHover(setDropHint)
    return () => {
      for (const u of [unStarted, unRefused, unHover]) void u.then((f) => f()).catch(() => undefined)
    }
  }, [markWelcomed, showNewScan, onError])

  const zoom = (id: number) => {
    setViewRoot(id)
    setSelected(null)
  }

  const up = () => {
    const parent = crumbs[crumbs.length - 2]
    if (parent) {
      setSelected(viewRoot)
      setViewRoot(parent.id)
    }
  }

  const command = (cmd: ItemCommand, id: number) => {
    if (scanId === null) return
    if (cmd === 'zoom') {
      setView('map')
      zoom(id)
    } else if (cmd === 'reveal') {
      api.revealItem(scanId, id).catch((e) => onError(errorText(e)))
    } else if (cmd === 'open') {
      api.openItem(scanId, id).catch((e) => onError(errorText(e)))
    } else {
      api
        .nodeDetails(scanId, id)
        .then((d) => navigator.clipboard.writeText(d.path))
        .then(() => setToast('Path copied'))
        .catch(() => onError('The path could not be copied.'))
    }
  }

  const menuActions = (t: Tile): MenuAction[] => {
    if (t.kind === 'more') {
      // An aggregate's id is the folder that holds the small items.
      return [
        { label: 'Open folder in map', run: () => zoom(t.id), disabled: t.id === viewRoot },
        { label: names.showIn, run: () => command('reveal', t.id) },
      ]
    }
    const actions: MenuAction[] = []
    if (t.kind === 'dir') actions.push({ label: 'Open in map', run: () => zoom(t.id) })
    actions.push(
      { label: names.showIn, run: () => command('reveal', t.id) },
      { label: 'Open', run: () => command('open', t.id), disabled: t.kind === 'link' || t.kind === 'other' },
      { label: 'Copy path', run: () => command('copy', t.id) },
    )
    if (t.kind === 'file' || t.kind === 'dir') {
      actions.push({ label: marked.has(t.id) ? 'Remove from cleanup' : 'Add to cleanup', run: () => toggleMark(t.id), separatorBefore: true })
    }
    if (crumbs.length > 1) actions.push({ label: 'Up one level', run: up, separatorBefore: true })
    return actions
  }

  const p = status?.progress
  const scanning = status?.phase === 'scanning'
  const lastCrumb = crumbs[crumbs.length - 1]?.name ?? status?.root ?? ''
  const viewName = crumbs.length <= 1 ? baseName(lastCrumb) : lastCrumb

  const nav: [View, string, boolean][] = [
    ['overview', 'Overview', false],
    ['suggestions', 'Smart Scan', scanId === null],
    ['duplicates', 'Duplicates', scanId === null],
    ['map', 'Storage map', scanId === null],
    ['largest', 'Largest files', scanId === null],
    ['search', 'Search', scanId === null],
    ['cleanup', 'Cleanup', scanId === null],
    ['restore', 'Restore', false],
    ['history', 'History', false],
    ['apps', 'Apps', false],
    ['settings', 'Settings', false],
  ]

  return (
    <div className="app">
      <nav className="sidebar" aria-label="Main">
        <div className="brand">
          <Mark />
          <span className="brand-name">Tessera</span>
        </div>
        {nav.map(([v, label, disabled]) => (
          <button
            key={v}
            className="nav-item"
            aria-current={view === v ? 'page' : undefined}
            disabled={disabled}
            title={disabled ? 'Scan a drive or folder first' : undefined}
            onClick={() => setView(v)}
          >
            {label}
            {v === 'cleanup' && marked.size > 0 && <span className="nav-count" aria-label={` chosen`}>{marked.size}</span>}
          </button>
        ))}
        <div className="sidebar-foot">Scans stay on this computer.</div>
      </nav>

      <main className="main">
        <div>
          {error && (
            <div className="banner" role="alert">
              <span className="spacer">{error}</span>
              <button className="btn" onClick={() => setError(null)}>
                Dismiss
              </button>
            </div>
          )}
          {view === 'map' && scanId !== null && (
            <div className="toolbar">
              <h1>Storage map</h1>
              <div className="segmented" role="group" aria-label="Size to show">
                <button
                  aria-pressed={settings.metric === 'allocated'}
                  onClick={() => updateSettings({ ...settings, metric: 'allocated' })}
                >
                  Size on disk
                </button>
                <button
                  aria-pressed={settings.metric === 'logical'}
                  onClick={() => updateSettings({ ...settings, metric: 'logical' })}
                >
                  File size
                </button>
              </div>
              <span className="spacer" />
              <button
                className="btn"
                disabled={scanning || scanId === null}
                onClick={() =>
                  scanId !== null &&
                  void api
                    .exportReport(scanId, 'folder', viewRoot, settings.metric)
                    .then((path) => path && setToast(`Report saved to ${path}`))
                    .catch((e) => onError(errorText(e)))
                }
              >
                Export CSV
              </button>
              <button className="btn" onClick={() => setView('report')} title="A printable summary of this folder; save it as a PDF from the print dialog">
                Report
              </button>
              <button className="btn" onClick={() => setView('overview')}>
                New scan
              </button>
            </div>
          )}
        </div>

        {update && (
          <div className="info-strip" role="status">
            <span className="spacer">Tessera {update} is available.</span>
            <button className="btn" onClick={() => void api.openDownloadPage().catch((e) => onError(errorText(e)))}>
              Open download page
            </button>
            <button className="btn" onClick={() => setUpdate(null)}>
              Not now
            </button>
          </div>
        )}
        {view !== 'overview' && view !== 'settings' && settings.metric === 'logical' && p && p.logical > p.allocated * 1.25 + 1024 ** 3 && (
          <div className="info-strip" role="note">
            <span className="spacer">
              File size counts each file's full length. These files add up to {formatBytes(p.logical)} but occupy only{' '}
              {formatBytes(p.allocated)} on disk, because some are sparse, compressed or online-only, so this total can be
              larger than your drive.
            </span>
            <button className="btn" onClick={() => updateSettings({ ...settings, metric: 'allocated' })}>
              Show size on disk
            </button>
          </div>
        )}
        <div className={view === 'map' || view === 'largest' || view === 'search' ? 'content fill' : 'content'}>
          {view === 'overview' && !settings.welcomed && (
            <Welcome
              os={info.os}
              volumes={volumes}
              onScan={(target) => {
                markWelcomed()
                startScan(target)
              }}
              onSkip={markWelcomed}
            />
          )}
          {view === 'overview' && settings.welcomed && (
            <Overview
              volumes={volumes}
              status={status}
              onScan={startScan}
              onOpenMap={() => setView('map')}
              onRefreshDrives={refreshDrives}
            />
          )}
          {view === 'map' && scanId !== null && (
            <div className="explore">
              <div className={highlight || !settings.mapTipsSeen ? 'map-pane with-strip' : 'map-pane'}>
                {!highlight && !settings.mapTipsSeen && (
                  <div className="highlight-strip" role="note">
                    <span className="spacer">
                      Click a tile to see what it is, double-click a folder to open it, and right-click
                      {info.os === 'macos' ? ' (or Control-click)' : ''} for actions.{' '}
                      {info.os === 'macos' ? 'The Delete key' : 'Backspace'} goes back up. Smart Scan suggests what you
                      could remove.
                    </span>
                    <button className="btn" onClick={() => updateSettings({ ...settings, mapTipsSeen: true })}>
                      Got it
                    </button>
                  </div>
                )}
                {highlight && (
                  <div className="highlight-strip" role="status">
                    <span className="spacer">Highlighting {highlight.label}</span>
                    <button className="btn" onClick={() => setHighlight(null)}>Clear</button>
                  </div>
                )}
                <div className="crumbs" aria-label="Location">
                  {crumbs.map((c, i) => (
                    <span key={c.id} className="crumb-item">
                      {i > 0 && <span className="crumb-sep">›</span>}
                      <button
                        className="crumb"
                        aria-current={i === crumbs.length - 1 ? 'location' : undefined}
                        onClick={() => i < crumbs.length - 1 && zoom(c.id)}
                        title={c.name}
                      >
                        {i === 0 ? baseName(c.name) : c.name}
                      </button>
                    </span>
                  ))}
                </div>
                <Treemap
                  scanId={scanId}
                  rootId={viewRoot}
                  rootName={viewName}
                  metric={settings.metric}
                  theme={theme}
                  contrast={contrast}
                  os={info.os}
                  version={version}
                  selected={selected}
                  highlight={highlight}
                  onSelect={setSelected}
                  onZoom={zoom}
                  onUp={up}
                  onMenu={(tile, x, y) => setMenu({ tile, x, y })}
                  onError={onError}
                />
              </div>
              <Details
                scanId={scanId}
                viewRoot={viewRoot}
                selected={selected}
                metric={settings.metric}
                theme={theme}
                os={info.os}
                version={version}
                onSelect={setSelected}
                onCommand={command}
                marked={marked}
                onToggleMark={toggleMark}
                onError={onError}
              />
            </div>
          )}
          {view === 'largest' && scanId !== null && (
            <LargestFiles
              scanId={scanId}
              viewRoot={viewRoot}
              viewRootName={viewName}
              metric={settings.metric}
              os={info.os}
              version={version}
              onCommand={command}
              marked={marked}
              onToggleMark={toggleMark}
              onError={onError}
              onDone={setToast}
            />
          )}
          {view === 'search' && scanId !== null && (
            <Search
              scanId={scanId}
              viewRoot={viewRoot}
              viewRootName={viewName}
              metric={settings.metric}
              os={info.os}
              version={version}
              onCommand={command}
              onHighlight={(m, c, label) => {
                setHighlight({ matches: new Set(m), containing: new Set(c), label })
                setView('map')
              }}
              marked={marked}
              onToggleMark={toggleMark}
              onError={onError}
            />
          )}
          {view === 'suggestions' && scanId !== null && (
            <Suggestions
              scanId={scanId}
              scanning={scanning}
              version={version}
              marked={marked}
              onToggleMark={toggleMark}
              onMarkMany={markMany}
              onOpenCleanup={() => setView('cleanup')}
              onError={onError}
              onDone={setToast}
            />
          )}
          {view === 'report' && scanId !== null && (
            <Report
              scanId={scanId}
              viewRoot={viewRoot}
              metric={settings.metric}
              scanFinished={status?.phase === 'complete'}
              pro={entitlement?.pro ?? false}
              appVersion={info.version}
              onOpenSettings={() => setView('settings')}
              onBack={() => setView('map')}
              onError={onError}
            />
          )}
          {view === 'restore' && <Restore os={info.os} onDone={setToast} onError={onError} />}
          {view === 'history' && (
            <History pro={entitlement?.pro ?? false} onOpenSettings={() => setView('settings')} onError={onError} />
          )}
          {view === 'apps' && <Apps os={info.os} scanId={scanId} onDone={setToast} onError={onError} />}
          {view === 'duplicates' && scanId !== null && (
            <Duplicates
              scanId={scanId}
              scanning={scanning}
              marked={marked}
              onToggleMark={toggleMark}
              onMarkMany={markMany}
              onOpenCleanup={() => setView('cleanup')}
              onError={onError}
            />
          )}
          {view === 'cleanup' && scanId !== null && (
            <Cleanup
              scanId={scanId}
              marked={[...marked]}
              metric={settings.metric}
              os={info.os}
              pro={entitlement?.pro ?? false}
              onOpenSettings={() => setView('settings')}
              onUnmark={toggleMark}
              onFinished={(r) => {
                setMarked((m) => {
                  const next = new Set(m)
                  for (const o of r.outcomes) if (o.ok) next.delete(o.id)
                  return next
                })
                setSelected(null)
                setVersion((v) => v + 1)
                refreshDrives()
                api.scanStatus().then(setStatus).catch((e) => onError(errorText(e)))
              }}
              onError={onError}
            />
          )}
          {view === 'settings' && (
            <SettingsView
              settings={settings}
              os={info.os}
              version={info.version}
              storeManaged={info.storeManaged}
              onChange={updateSettings}
              onShowWelcome={() => {
                updateSettings({ ...settings, welcomed: false, mapTipsSeen: false })
                setView('overview')
              }}
              entitlement={entitlement}
              onEntitlement={setEntitlement}
            />
          )}
        </div>

        <footer className="statusbar" aria-live="polite">
          {!status && <span>No scan yet. Choose a drive or folder on the Overview page.</span>}
          {status && p && (
            <>
              {scanning && <span className="pulse" aria-hidden="true" />}
              <span className="strong">
                {scanning && 'Scanning'}
                {status.phase === 'complete' && `Scanned in ${formatDuration(status.summary?.elapsedMs ?? p.elapsedMs)}`}
                {status.phase === 'cancelled' && 'Scan stopped; results are partial'}
                {status.phase === 'failed' && (status.error ?? 'Scan failed')}
              </span>
              <span className="num">
                {plural(p.files, 'file', 'files')} in {plural(p.dirs, 'folder', 'folders')}
              </span>
              <span className="num">
                {settings.metric === 'allocated' ? `${formatBytes(p.allocated)} on disk` : `${formatBytes(p.logical)} of files`}
              </span>
              {scanning && <span className="num">{formatDuration(p.elapsedMs)}</span>}
              {p.issues > 0 && <span className="num">{plural(p.issues, 'item', 'items')} could not be read</span>}
              {status.summary?.hardLinks === 'partial' && !scanning && (
                <span title="Smaller hard-linked files may be counted more than once. Change this in Settings.">
                  Hard links checked for files of {formatBytes(status.summary.hardLinkMinSize ?? 0)} and larger
                </span>
              )}
              {status.summary?.hardLinks === 'none' && !scanning && <span>Hard links not checked</span>}
              <span className="spacer" />
              {status.volume && isDriveRoot(status.root) && status.phase === 'complete' && (
                <span className="num" title={`Used space as reported by ${info.os === 'macos' ? 'macOS' : 'Windows'}`}>
                  Drive: {formatBytes(status.volume.used)} used,{' '}
                  {formatBytes(Math.max(0, status.volume.used - p.allocated))} not attributed to scanned files
                </span>
              )}
              {scanning && (
                <button className="btn" onClick={() => void api.cancelScan()}>
                  Stop scan
                </button>
              )}
            </>
          )}
        </footer>
      </main>

      {dropHint && (
        <div className="drop-hint" aria-hidden="true">
          <div className="drop-hint-card">Drop a folder or drive to scan it</div>
        </div>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} actions={menuActions(menu.tile)} onClose={() => setMenu(null)} />}
      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}
    </div>
  )
}
