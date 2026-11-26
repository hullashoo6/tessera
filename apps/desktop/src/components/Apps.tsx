import { useCallback, useEffect, useState } from 'react'
import { api, errorText, type AppItem } from '../api'
import { formatBytes } from '../format'

interface Props {
  os: string
  scanId: number | null
  onDone: (message: string) => void
  onError: (message: string) => void
}

const date = (d: string | null) => (d && /^\d{8}$/.test(d) ? `${d.slice(6, 8)}/${d.slice(4, 6)}/${d.slice(0, 4)}` : null)
const size = (a: AppItem) => a.measuredBytes ?? a.reportedBytes ?? 0

/** Installed apps with their own uninstallers (Windows). */
export function Apps({ os, scanId, onDone, onError }: Props) {
  const [apps, setApps] = useState<AppItem[] | null>(null)
  const [filter, setFilter] = useState('')
  const [confirm, setConfirm] = useState<string | null>(null)

  const load = useCallback(() => {
    api
      .listApps(scanId)
      .then((a) => setApps([...a].sort((x, y) => size(y) - size(x))))
      .catch((e) => onError(errorText(e)))
  }, [scanId, onError])
  useEffect(load, [load])

  if (os !== 'windows') {
    return (
      <div className="empty">
        <p>Uninstalling apps from Tessera is coming to macOS later. For now, drag an app from Applications to the Trash.</p>
      </div>
    )
  }

  const shown = (apps ?? []).filter((a) => `${a.name} ${a.publisher ?? ''}`.toLowerCase().includes(filter.trim().toLowerCase()))

  const uninstall = (a: AppItem) => {
    setConfirm(null)
    api
      .uninstallApp(a.id)
      .then((r) => {
        if (r === 'removed') {
          onDone(`${a.name} was removed.`)
          load()
        } else onDone(`${a.name}'s own uninstaller is open. When it finishes, choose Refresh.`)
      })
      .catch((e) => onError(errorText(e)))
  }

  return (
    <div className="overview suggestions">
      <h2>Apps</h2>
      <p className="lede">
        Apps installed on this computer, largest first. Uninstall opens the app’s own uninstaller, the same one Windows
        Settings uses. Tessera does not delete app files itself.
      </p>
      <div className="actions">
        <input
          className="search-input"
          aria-label="Filter apps"
          placeholder="Filter by name or publisher"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <button className="btn" onClick={load}>
          Refresh
        </button>
      </div>
      {apps === null ? (
        <p className="target-meta">Reading the list of installed apps…</p>
      ) : (
        <ul className="review-list">
          {shown.map((a) => (
            <li key={a.id}>
              <div className="review-main">
                <span className="review-name">{a.name}</span>
                <span className="target-meta">
                  {[a.publisher, a.version && `version ${a.version}`, date(a.installDate) && `installed ${date(a.installDate)}`]
                    .filter(Boolean)
                    .join(' · ')}
                  {a.perUser ? ' · for this user only' : ''}
                </span>
                {confirm === a.id && (
                  <span className="actions">
                    <span className="review-reason">{a.id.startsWith('APPX|') ? `Remove ${a.name} from this computer?` : `Open ${a.name}’s uninstaller? Windows may ask for permission.`}</span>
                    <button className="btn btn-primary" onClick={() => uninstall(a)}>
                      {a.id.startsWith('APPX|') ? 'Remove app' : 'Open uninstaller'}
                    </button>
                    <button className="btn" onClick={() => setConfirm(null)}>
                      Cancel
                    </button>
                  </span>
                )}
              </div>
              <span className="num" title={a.measuredBytes !== null ? 'Measured in this scan' : 'Reported by the app'}>
                {size(a) ? formatBytes(size(a)) : 'Size not reported'}
                <span className="target-meta">
                  {a.measuredBytes !== null ? ' measured' : a.reportedBytes !== null ? ' reported by the app' : ''}
                </span>
              </span>
              {a.canUninstall ? (
                <button className="btn btn-quiet" onClick={() => setConfirm(a.id)}>
                  Uninstall…
                </button>
              ) : (
                <span className="target-meta">Use Windows Settings</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
