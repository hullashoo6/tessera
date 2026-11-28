import { useEffect, useState } from 'react'
import { api, errorText, type DupProgress, type DupReport } from '../api'
import { formatBytes, formatCount, formatDate, plural, timeLeft } from '../format'

interface Props {
  scanId: number
  scanning: boolean
  marked: Set<number>
  onToggleMark: (id: number) => void
  onMarkMany: (ids: number[]) => void
  onOpenCleanup: () => void
  /** Told when a search starts and ends (sidebar "still working"). */
  onBusy?: (busy: boolean) => void
  onError: (message: string) => void
}

export function Duplicates({ scanId, scanning, marked, onToggleMark, onMarkMany, onOpenCleanup, onBusy, onError }: Props) {
  // Results belong to the scan they were found in.
  const [result, setResult] = useState<{ scanId: number; report: DupReport } | null>(null)
  const [runningFor, setRunningFor] = useState<number | null>(null)
  const [progress, setProgress] = useState<DupProgress | null>(null)
  // When the current stage started (for the time left).
  const [stageStart, setStageStart] = useState<{ stage: number; at: number }>({ stage: 0, at: 0 })
  const [now, setNow] = useState(() => Date.now())
  // Shown 100 groups at a time so the window stays smooth.
  const [shownGroups, setShownGroups] = useState(100)
  const report = result?.scanId === scanId ? result.report : null
  const running = runningFor === scanId

  useEffect(() => {
    onBusy?.(runningFor !== null)
  }, [runningFor, onBusy])

  useEffect(() => {
    const un = api.onDupesProgress((p) => {
      setStageStart((s) => (s.stage === p.stage ? s : { stage: p.stage, at: Date.now() }))
      setProgress(p)
      setNow(Date.now())
    })
    return () => {
      void un.then((f) => f())
    }
  }, [])

  const start = () => {
    const forScan = scanId
    setRunningFor(forScan)
    setProgress(null)
    setStageStart({ stage: 0, at: Date.now() })
    setShownGroups(100)
    api
      .findDuplicates(forScan)
      .then((r) => setResult({ scanId: forScan, report: r }))
      .catch((e) => onError(errorText(e)))
      .finally(() => setRunningFor((s) => (s === forScan ? null : s)))
  }

  const total = report?.groups.reduce((n, g) => n + g.reclaimable, 0) ?? 0
  const chosen = report?.groups.flatMap((g) => g.copies).filter((c) => marked.has(c.node.id)).length ?? 0
  const p = running ? progress : null
  const comparing = p?.stage === 2
  const [done, of] = comparing ? [p.bytesDone, p.bytesTotal] : [p?.filesDone ?? 0, p?.filesTotal ?? 0]
  const pct = of > 0 ? Math.min(100, Math.floor((done / of) * 100)) : 0
  const left = p ? timeLeft(done, of, now - stageStart.at) : null

  return (
    <div className="overview suggestions">
      <h2>{report ? (report.groups.length ? `${formatBytes(total)} in duplicate copies` : 'No duplicates found') : 'Find duplicate files'}</h2>
      <p className="lede">
        Tessera compares the contents of files of 1 MB and larger, on this computer only, and lists only exact copies.
        Online-only files are skipped so nothing is downloaded. You choose which copies to remove.
      </p>
      {running && (
        <div className="dup-progress">
          <div
            className="meter"
            role="progressbar"
            aria-label={comparing ? 'Comparing possible copies' : 'Checking files'}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct}
          >
            <div
              className="meter-fill"
              ref={(el) => {
                if (el) el.style.width = `${pct}%`
              }}
            />
          </div>
          <p className="target-meta num">
            {!p
              ? 'Starting…'
              : comparing
                ? `Comparing possible copies: ${formatBytes(p.bytesDone)} of ${formatBytes(p.bytesTotal)}`
                : `Checking files that share a size: ${formatCount(p.filesDone)} of ${formatCount(p.filesTotal)}`}
            {left ? ` · ${left}` : ''}
          </p>
          {p && p.groups > 0 && (
            <p className="target-meta num">
              Found so far: {formatBytes(p.reclaimable)} in {plural(p.groups, 'group', 'groups')} of copies.
            </p>
          )}
          <p className="target-meta">You can use other pages meanwhile. The search carries on.</p>
        </div>
      )}
      <div className="actions">
        {running ? (
          <button className="btn" onClick={() => void api.cancelDuplicates()}>
            Stop
          </button>
        ) : (
          <button className="btn btn-primary" disabled={scanning} onClick={start}>
            {report ? 'Search again' : 'Find duplicates'}
          </button>
        )}
      </div>
      {report && (
        <p className="target-meta num">
          {report.cancelled ? 'Stopped early; results are partial. ' : ''}
          Compared {plural(report.filesCompared, 'file', 'files')}, read {formatBytes(report.bytesRead)}
          {report.skipped ? `, skipped ${plural(report.skipped, 'file', 'files')} that changed or could not be read` : ''}.
        </p>
      )}
      {chosen > 0 && (
        <div className="current-scan">
          <span className="spacer">{plural(chosen, 'copy', 'copies')} added to cleanup.</span>
          <button className="btn btn-primary" onClick={onOpenCleanup}>
            Review in Cleanup
          </button>
        </div>
      )}
      {report?.groups.slice(0, shownGroups).map((g) => {
        const allChosen = g.copies.every((c) => marked.has(c.node.id))
        const others = (keep: number) => g.copies.filter((_, i) => i !== keep).map((c) => c.node.id)
        return (
          <section key={g.hash} className="suggestion-group" aria-label={`${g.copies.length} copies`}>
            <div className="group-head">
              <div>
                <div className="section-title num">
                  {plural(g.copies.length, 'copy', 'copies')} of {formatBytes(g.size)}, {formatBytes(g.reclaimable)} could be freed
                </div>
                {allChosen && <span className="review-reason">Every copy is chosen. At least one copy would be lost.</span>}
              </div>
              <div className="actions">
                <button className="btn" onClick={() => onMarkMany(others(0))}>
                  Keep the oldest
                </button>
                <button className="btn" onClick={() => onMarkMany(others(g.copies.length - 1))}>
                  Keep the newest
                </button>
              </div>
            </div>
            <ul className="review-list">
              {g.copies.map((c) => (
                <li key={c.node.id}>
                  <div className="review-main">
                    <span className="review-name">{c.node.name}</span>
                    <span className="selectable review-path">{c.path}</span>
                    <span className="target-meta">Modified {formatDate(c.node.modified)}</span>
                  </div>
                  <span className="num">{formatBytes(g.size)}</span>
                  <button className="btn btn-quiet" aria-label={`Add to cleanup: ${c.path}`} aria-pressed={marked.has(c.node.id)} onClick={() => onToggleMark(c.node.id)}>
                    {marked.has(c.node.id) ? 'Added' : 'Add to cleanup'}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )
      })}
      {report && report.groups.length > shownGroups && (
        <button className="btn" onClick={() => setShownGroups((n) => n + 100)}>
          Show {Math.min(100, report.groups.length - shownGroups)} more of {report.groups.length} groups
        </button>
      )}
    </div>
  )
}
