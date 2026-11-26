import { useEffect, useState } from 'react'
import { api, errorText, type DupReport } from '../api'
import { formatBytes, formatDate, plural } from '../format'

interface Props {
  scanId: number
  scanning: boolean
  marked: Set<number>
  onToggleMark: (id: number) => void
  onMarkMany: (ids: number[]) => void
  onOpenCleanup: () => void
  onError: (message: string) => void
}

export function Duplicates({ scanId, scanning, marked, onToggleMark, onMarkMany, onOpenCleanup, onError }: Props) {
  const [report, setReport] = useState<DupReport | null>(null)
  const [running, setRunning] = useState(false)
  const [read, setRead] = useState(0)

  useEffect(() => {
    const un = api.onDupesProgress(setRead)
    return () => {
      void un.then((f) => f())
    }
  }, [])

  const start = () => {
    setRunning(true)
    setRead(0)
    setReport(null)
    api
      .findDuplicates(scanId)
      .then(setReport)
      .catch((e) => onError(errorText(e)))
      .finally(() => setRunning(false))
  }

  const total = report?.groups.reduce((n, g) => n + g.reclaimable, 0) ?? 0
  const chosen = report?.groups.flatMap((g) => g.copies).filter((c) => marked.has(c.node.id)).length ?? 0

  return (
    <div className="overview suggestions">
      <h2>{report ? (report.groups.length ? `${formatBytes(total)} in duplicate copies` : 'No duplicates found') : 'Find duplicate files'}</h2>
      <p className="lede">
        Tessera compares the contents of files of 1 MB and larger, on this computer only, and lists only exact copies.
        Online-only files are skipped so nothing is downloaded. You choose which copies to remove.
      </p>
      <div className="actions">
        {running ? (
          <>
            <span className="target-meta num">Comparing files… {formatBytes(read)} read so far</span>
            <button className="btn" onClick={() => void api.cancelDuplicates()}>
              Stop
            </button>
          </>
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
      {report?.groups.map((g) => {
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
                  <button className="btn btn-quiet" aria-pressed={marked.has(c.node.id)} onClick={() => onToggleMark(c.node.id)}>
                    {marked.has(c.node.id) ? 'Added' : 'Add to cleanup'}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )
      })}
    </div>
  )
}
