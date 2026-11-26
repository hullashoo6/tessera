import { useEffect, useState } from 'react'
import { api, errorText, type Suggestion } from '../api'
import { formatBytes, plural } from '../format'

interface Props {
  scanId: number
  scanning: boolean
  version: number
  marked: Set<number>
  onToggleMark: (id: number) => void
  onMarkMany: (ids: number[]) => void
  onOpenCleanup: () => void
  onError: (message: string) => void
}

const GROUPS: { key: Suggestion['group']; title: string; blurb: string }[] = [
  { key: 'junk', title: 'Junk', blurb: 'Temporary files, caches and crash reports that apps recreate when needed.' },
  { key: 'developer', title: 'Developer files', blurb: 'Installed packages, build output and package caches that tools can rebuild or download again.' },
  { key: 'downloads', title: 'Large downloads', blurb: 'Big downloaded data you may no longer use. Look before removing.' },
]

const safe = (s: Suggestion) => s.safety === 'regenerable' || s.safety === 'redownloadable'

export function Suggestions({ scanId, scanning, version, marked, onToggleMark, onMarkMany, onOpenCleanup, onError }: Props) {
  const [items, setItems] = useState<Suggestion[] | null>(null)

  useEffect(() => {
    if (scanning) return
    let live = true
    api
      .smartScan(scanId)
      .then((s) => live && setItems(s))
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [scanId, scanning, version, onError])

  if (scanning) {
    return (
      <div className="empty">
        <p>Suggestions appear when the scan finishes.</p>
      </div>
    )
  }
  if (!items) {
    return (
      <div className="empty">
        <p>Looking for space you can recover…</p>
      </div>
    )
  }

  const total = items.filter((s) => s.safety !== 'useAppTool').reduce((n, s) => n + s.bytes, 0)
  const chosen = items.filter((s) => marked.has(s.id))

  return (
    <div className="overview suggestions">
      <h2>{items.length ? `Up to ${formatBytes(total)} you could recover` : 'Nothing to suggest here'}</h2>
      <p className="lede">
        {items.length
          ? 'Each suggestion says why it was found and how safe it is. Nothing is removed until you review it in Cleanup.'
          : 'Tessera found no caches, build output or temporary files worth removing in this scan.'}
      </p>
      {chosen.length > 0 && (
        <div className="current-scan">
          <span className="spacer">{plural(chosen.length, 'suggestion', 'suggestions')} added to cleanup.</span>
          <button className="btn btn-primary" onClick={onOpenCleanup}>Review in Cleanup</button>
        </div>
      )}
      {GROUPS.map((g) => {
        const list = items.filter((s) => s.group === g.key)
        if (!list.length) return null
        const addable = list.filter((s) => safe(s) && !marked.has(s.id))
        const sum = list.reduce((n, s) => n + s.bytes, 0)
        return (
          <section key={g.key} className="suggestion-group" aria-label={g.title}>
            <div className="group-head">
              <div>
                <div className="section-title">
                  {g.title} <span className="num">{formatBytes(sum)}</span>
                </div>
                <div className="target-meta">{g.blurb}</div>
              </div>
              {addable.length > 0 && (
                <button className="btn" onClick={() => onMarkMany(addable.map((s) => s.id))}>
                  Add {plural(addable.length, 'item', 'items')} recreated automatically
                </button>
              )}
            </div>
            <ul className="review-list">
              {list.map((s) => (
                <li key={s.id}>
                  <div className="review-main">
                    <span className="review-name">{s.label}</span>
                    <span className="selectable review-path">{s.path}</span>
                    <span className="target-meta">{s.reason}</span>
                    <span className={`safety safety-${s.safety}`}>{s.safetyText}</span>
                  </div>
                  <span className="num">{formatBytes(s.bytes)}</span>
                  {s.safety === 'useAppTool' ? (
                    <span className="target-meta">Use the app</span>
                  ) : (
                    <button className="btn btn-quiet" aria-pressed={marked.has(s.id)} onClick={() => onToggleMark(s.id)}>
                      {marked.has(s.id) ? 'Added' : 'Add to cleanup'}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        )
      })}
    </div>
  )
}
