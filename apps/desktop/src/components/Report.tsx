import { useEffect, useState } from 'react'
import { api, errorText, type Details, type FileRow, type Metric, type NodeInfo, type Suggestion } from '../api'
import { baseName, formatBytes, formatDate, percent, plural } from '../format'

interface Props {
  scanId: number
  /** The folder the report is about (the one open in the storage map). */
  viewRoot: number
  metric: Metric
  scanFinished: boolean
  pro: boolean
  appVersion: string
  onOpenSettings: () => void
  onBack: () => void
  onError: (message: string) => void
}

interface Data {
  details: Details
  folders: NodeInfo[]
  files: FileRow[]
  suggestions: Suggestion[] | null
  /** Unix seconds when the report was put together. */
  madeAt: number
}

const GROUP_NAMES: Record<Suggestion['group'], string> = {
  junk: 'Junk',
  developer: 'Developer files',
  downloads: 'Large downloads',
  backups: 'Backups and recordings',
  old: 'Large old files',
  cloud: 'Cloud files on this computer',
}

/**
 * A one-page summary of the folder open in the storage map, laid out for
 * paper. "Print or save as PDF" opens the system's print dialog, where both
 * Windows ("Microsoft Print to PDF") and macOS ("Save as PDF") can make a PDF.
 * Pro, like the CSV reports.
 */
export function Report({ scanId, viewRoot, metric, scanFinished, pro, appVersion, onOpenSettings, onBack, onError }: Props) {
  const [data, setData] = useState<Data | null>(null)
  const size = (n: NodeInfo) => (metric === 'allocated' ? n.allocated : n.logical)

  useEffect(() => {
    if (!pro) return
    let live = true
    Promise.all([
      api.nodeDetails(scanId, viewRoot),
      api.children(scanId, viewRoot, metric, 0, 15),
      api.largestFiles(scanId, viewRoot === 0 ? null : viewRoot, metric, 25),
      scanFinished ? api.smartScan(scanId).catch(() => null) : Promise.resolve(null),
    ])
      .then(([details, page, files, suggestions]) => {
        if (live) setData({ details, folders: page.items, files, suggestions, madeAt: Math.floor(Date.now() / 1000) })
      })
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [pro, scanId, viewRoot, metric, scanFinished, onError])

  if (!pro) {
    return (
      <div className="empty">
        <div>
          <h2>Printable reports are part of Tessera Pro</h2>
          <p>Print a one-page summary of any folder, or save it as a PDF to share or keep.</p>
          <button className="btn btn-primary" onClick={onOpenSettings}>
            Enter licence key
          </button>
        </div>
      </div>
    )
  }
  if (!data) return <div className="empty"><p>Preparing the report…</p></div>

  const total = size(data.details.node)
  // Only suggestions inside the folder this report is about.
  const base = data.details.path.replace(/[\\/]+$/, '')
  const inside = (p: string) => viewRoot === 0 || p === base || p.startsWith(`${base}\\`) || p.startsWith(`${base}/`)
  const groups = new Map<Suggestion['group'], number>()
  for (const s of data.suggestions ?? []) {
    if (s.safety !== 'useAppTool' && inside(s.path)) groups.set(s.group, (groups.get(s.group) ?? 0) + s.bytes)
  }
  const sizeLabel = metric === 'allocated' ? 'Size on disk' : 'File size'

  return (
    <div className="report-page">
      <div className="report-actions">
        <button className="btn" onClick={onBack}>
          Back to the map
        </button>
        <span className="spacer" />
        <button
          className="btn btn-primary"
          onClick={() => void api.printReport().catch((e) => onError(errorText(e)))}
        >
          Print or save as PDF
        </button>
      </div>
      <article className="report" aria-label="Storage report">
        <header>
          <h1>Storage report: {baseName(data.details.path)}</h1>
          <p className="report-meta">
            {data.details.path}
            <br />
            Made {formatDate(data.madeAt)} with Tessera {appVersion}
          </p>
        </header>
        <section className="report-totals">
          <div>
            <div className="report-figure">{formatBytes(total)}</div>
            <div className="report-meta">{sizeLabel}</div>
          </div>
          <div>
            <div className="report-figure">{plural(data.details.node.files, 'file', 'files')}</div>
            <div className="report-meta">in {plural(data.details.node.dirs, 'folder', 'folders')}</div>
          </div>
        </section>

        <h2>Largest items here</h2>
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th className="num">{sizeLabel}</th>
              <th className="num">Share</th>
            </tr>
          </thead>
          <tbody>
            {data.folders.map((n) => (
              <tr key={n.id}>
                <td>
                  {n.name}
                  {n.kind === 'dir' ? ' (folder)' : ''}
                </td>
                <td className="num">{formatBytes(size(n))}</td>
                <td className="num">{percent(size(n), total)}</td>
              </tr>
            ))}
          </tbody>
        </table>

        <h2>Largest files</h2>
        <table>
          <thead>
            <tr>
              <th>File</th>
              <th>Folder</th>
              <th className="num">{sizeLabel}</th>
              <th>Modified</th>
            </tr>
          </thead>
          <tbody>
            {data.files.map((f) => (
              <tr key={f.node.id}>
                <td>{f.node.name}</td>
                <td className="report-path">{f.path.slice(0, Math.max(0, f.path.length - f.node.name.length - 1))}</td>
                <td className="num">{formatBytes(size(f.node))}</td>
                <td>{formatDate(f.node.modified)}</td>
              </tr>
            ))}
          </tbody>
        </table>

        {data.suggestions && (
          <>
            <h2>Smart Scan</h2>
            {groups.size === 0 ? (
              <p>No suggestions for this scan.</p>
            ) : (
              <table>
                <tbody>
                  {[...groups].map(([g, bytes]) => (
                    <tr key={g}>
                      <td>{GROUP_NAMES[g]}</td>
                      <td className="num">{formatBytes(bytes)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
            <p className="report-meta">Suggestions are for review. Nothing was changed on this computer.</p>
          </>
        )}
        <footer className="report-meta">Tessera reads file sizes on this computer only. This report was not sent anywhere.</footer>
      </article>
    </div>
  )
}
