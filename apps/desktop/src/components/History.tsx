import { useEffect, useMemo, useState } from 'react'
import { api, errorText, type Snapshot } from '../api'
import { formatBytes, formatDate } from '../format'

interface Props {
  pro: boolean
  onOpenSettings: () => void
  onError: (message: string) => void
}

const DAY = 86_400

function signed(n: number) {
  if (n === 0) return 'no change'
  return `${n > 0 ? '+' : '−'}${formatBytes(Math.abs(n))}`
}

/** How a drive or folder changed between scans (Pro). */
export function History({ pro, onOpenSettings, onError }: Props) {
  const [all, setAll] = useState<Snapshot[] | null>(null)
  const [root, setRoot] = useState<string | null>(null)
  const [mode, setMode] = useState<'month' | 'previous'>('month')

  useEffect(() => {
    if (!pro) return
    api
      .storageHistory()
      .then((h) => {
        setAll(h)
        setRoot((r) => r ?? h[0]?.root ?? null)
      })
      .catch((e) => onError(errorText(e)))
  }, [pro, onError])

  const entries = useMemo(() => (all ?? []).filter((s) => s.root === root).sort((a, b) => a.at - b.at), [all, root])
  const roots = useMemo(() => [...new Set((all ?? []).map((s) => s.root))], [all])

  if (!pro) {
    return (
      <div className="empty">
        <div>
          <h2>Storage history is part of Tessera Pro</h2>
          <p>See how a drive grew since last month and which folders changed. Every finished scan is already being remembered on this computer.</p>
          <button className="btn btn-primary" onClick={onOpenSettings}>
            Enter licence key
          </button>
        </div>
      </div>
    )
  }
  if (all === null) return <div className="empty"><p>Loading history…</p></div>
  if (!entries.length) {
    return (
      <div className="empty">
        <p>No history yet. Each finished scan is remembered here, so scan a drive or folder and come back after the next scan.</p>
      </div>
    )
  }

  const latest = entries[entries.length - 1]!
  const older = entries.slice(0, -1)
  const monthAgo = [...older].reverse().find((s) => latest.at - s.at >= 25 * DAY)
  const base = mode === 'month' ? (monthAgo ?? older[0]) : older[older.length - 1]
  const changes = base
    ? [...new Set([...latest.top.map(([n]) => n), ...base.top.map(([n]) => n)])]
        .map((name) => {
          const now = latest.top.find(([n]) => n === name)?.[1]
          const then = base.top.find(([n]) => n === name)?.[1]
          return { name, now, then, delta: (now ?? 0) - (then ?? 0) }
        })
        .filter((c) => c.delta !== 0)
        .sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta))
        .slice(0, 20)
    : []

  // Small line chart of the total over time.
  const w = 640
  const h = 140
  const vals = entries.map((s) => s.allocated)
  const lo = Math.min(...vals)
  const hi = Math.max(...vals)
  const t0 = entries[0]!.at
  const t1 = latest.at
  const pts = entries.map((s) => {
    const x = t1 === t0 ? w / 2 : ((s.at - t0) / (t1 - t0)) * (w - 20) + 10
    const y = hi === lo ? h / 2 : h - 10 - ((s.allocated - lo) / (hi - lo)) * (h - 20)
    return `${x.toFixed(1)},${y.toFixed(1)}`
  })

  return (
    <div className="overview suggestions">
      <h2>Storage history</h2>
      <div className="actions">
        <select className="search-input" aria-label="Location" value={root ?? ''} onChange={(e) => setRoot(e.target.value)}>
          {roots.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
        <button
          className="btn"
          onClick={() =>
            void api
              .clearStorageHistory()
              .then(() => setAll([]))
              .catch((e) => onError(errorText(e)))
          }
        >
          Clear history
        </button>
      </div>
      <p className="lede num">
        {formatBytes(latest.allocated)} on {formatDate(latest.at)}
        {base ? `, ${signed(latest.allocated - base.allocated)} since ${formatDate(base.at)}.` : '. Scan again later to see changes.'}
      </p>
      {entries.length > 1 && (
        <svg className="history-chart" viewBox={`0 0 ${w} ${h}`} role="img" aria-label={`Size of ${root} over ${entries.length} scans`}>
          <polyline points={pts.join(' ')} fill="none" strokeWidth="2" />
          {pts.map((p, i) => {
            const [x, y] = p.split(',')
            return <circle key={entries[i]!.at} cx={x} cy={y} r="3" />
          })}
        </svg>
      )}
      {base && (
        <>
          <div className="group-head">
            <div className="section-title">Biggest changes</div>
            <div className="segmented" role="group" aria-label="Compare with">
              <button aria-pressed={mode === 'month'} onClick={() => setMode('month')}>
                About a month ago
              </button>
              <button aria-pressed={mode === 'previous'} onClick={() => setMode('previous')}>
                Previous scan
              </button>
            </div>
          </div>
          {changes.length === 0 ? (
            <p className="target-meta">No folder changed size.</p>
          ) : (
            <ul className="review-list">
              {changes.map((c) => (
                <li key={c.name}>
                  <div className="review-main">
                    <span className="review-name">{c.name}</span>
                    <span className="target-meta">
                      {c.then === undefined ? 'new since then' : c.now === undefined ? 'no longer among the largest' : `${formatBytes(c.then)} → ${formatBytes(c.now)}`}
                    </span>
                  </div>
                  <span className={`num ${c.delta > 0 ? 'grew' : 'shrank'}`}>{signed(c.delta)}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  )
}
