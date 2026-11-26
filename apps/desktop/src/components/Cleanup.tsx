import { useEffect, useRef, useState } from 'react'
import { api, errorText, type CleanupReport, type Metric, type Review } from '../api'
import { formatBytes, plural } from '../format'

interface Props {
  scanId: number
  marked: number[]
  metric: Metric
  os: string
  pro: boolean
  onOpenSettings: () => void
  onUnmark: (id: number) => void
  onFinished: (report: CleanupReport) => void
  onError: (message: string) => void
}

export function Cleanup({ scanId, marked, metric, os, pro, onOpenSettings, onUnmark, onFinished, onError }: Props) {
  const bin = os === 'macos' ? 'Trash' : 'Recycle Bin'
  const [review, setReview] = useState<Review | null>(null)
  const [confirming, setConfirming] = useState(false)
  const [running, setRunning] = useState(false)
  const [report, setReport] = useState<CleanupReport | null>(null)
  const confirmRef = useRef<HTMLButtonElement>(null)
  const key = marked.join(',')

  useEffect(() => {
    if (marked.length === 0 || !pro) return // these states are rendered directly
    let live = true
    api
      .reviewCleanup(scanId, marked)
      .then((r) => live && setReview(r))
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
    // `key` captures the selection contents.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scanId, key, pro, onError])

  useEffect(() => {
    if (confirming) confirmRef.current?.focus()
  }, [confirming])

  const size = (logical: number, allocated: number) => formatBytes(metric === 'allocated' ? allocated : logical)

  const run = () => {
    if (!review) return
    setConfirming(false)
    setRunning(true)
    api
      .confirmCleanup(review.reviewId)
      .then((r) => {
        setReport(r)
        onFinished(r)
      })
      .catch((e) => onError(errorText(e)))
      .finally(() => setRunning(false))
  }

  if (report) {
    const gained = report.freeBefore !== null && report.freeAfter !== null ? report.freeAfter - report.freeBefore : null
    const failed = report.outcomes.filter((o) => !o.ok)
    return (
      <div className="overview cleanup">
        <h2>{report.moved > 0 ? `Moved ${plural(report.moved, 'item', 'items')} to the ${bin}` : 'Nothing was moved'}</h2>
        <p className="lede">
          {report.moved > 0 && `${size(report.movedLogical, report.movedAllocated)} of files are now in the ${bin}. `}
          {gained !== null && gained > 0 && `Free space on this drive went up by ${formatBytes(gained)}. `}
          {report.moved > 0 && `The space is fully freed when the ${bin} is emptied. To get an item back, open the ${bin}, right-click it and choose ${os === 'macos' ? 'Put Back' : 'Restore'}.`}
        </p>
        {failed.length > 0 && (
          <>
            <div className="section-title">{plural(failed.length, 'item was', 'items were')} left in place</div>
            <ul className="review-list">
              {failed.map((o) => (
                <li key={o.path}>
                  <div className="review-main">
                    <span className="selectable review-path">{o.path}</span>
                    <span className="review-reason">{o.reason}</span>
                  </div>
                  <span className="num">{size(o.logical, o.allocated)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
        <div className="actions">
          <button className="btn btn-primary" onClick={() => setReport(null)}>Done</button>
        </div>
      </div>
    )
  }

  if (!pro) {
    return (
      <div className="empty">
        <div>
          <h2>Cleanup is part of Tessera Pro</h2>
          <p>
            {marked.length > 0 ? `You have chosen ${plural(marked.length, 'item', 'items')}. ` : ''}
            Activate a licence key to review items and move them to the {bin}. Scanning, the map and search stay free.
          </p>
          <button className="btn btn-primary" onClick={onOpenSettings}>Enter licence key</button>
        </div>
      </div>
    )
  }

  if (marked.length === 0) {
    return (
      <div className="empty">
        <div>
          <h2>Nothing chosen for cleanup</h2>
          <p>In the storage map, Search or Largest files, choose Add to cleanup on anything you no longer need. You will review the full list here before anything moves.</p>
        </div>
      </div>
    )
  }

  return (
    <div className="overview cleanup">
      <h2>Review before moving to the {bin}</h2>
      <p className="lede">
        {review
          ? `${plural(review.items.length, 'item', 'items')}, up to ${size(review.totalLogical, review.totalAllocated)} that could be freed.`
          : 'Checking the selected items…'}
        {review && review.merged > 0 && ` ${plural(review.merged, 'item is', 'items are')} already included through a selected folder.`}
      </p>
      {review && review.items.length > 0 && (
        <ul className="review-list" aria-label="Items to move">
          {review.items.map((it) => (
            <li key={it.id}>
              <div className="review-main">
                <span className="review-name">{it.name}{it.kind === 'dir' ? ' (folder)' : ''}</span>
                <span className="selectable review-path">{it.path}</span>
                {it.warning && <span className="review-reason">{it.warning}</span>}
              </div>
              <span className="num">{size(it.logical, it.allocated)}</span>
              <button className="btn btn-quiet" onClick={() => onUnmark(it.id)}>Remove</button>
            </li>
          ))}
        </ul>
      )}
      {review && review.rejected.length > 0 && (
        <>
          <div className="section-title">Not included</div>
          <ul className="review-list muted-list">
            {review.rejected.map((r) => (
              <li key={r.path}>
                <div className="review-main">
                  <span className="selectable review-path">{r.path}</span>
                  <span className="review-reason">{r.reason}</span>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
      <div className="actions">
        <button
          className="btn btn-primary"
          disabled={!review || review.items.length === 0 || running}
          onClick={() => setConfirming(true)}
        >
          {running ? 'Moving…' : `Move ${review ? plural(review.items.length, 'item', 'items') : 'items'} to the ${bin}`}
        </button>
      </div>

      {confirming && review && (
        <div className="modal-backdrop" role="presentation" onKeyDown={(e) => e.key === 'Escape' && setConfirming(false)}>
          <div className="modal" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-body">
            <h3 id="confirm-title">
              Move {plural(review.items.length, 'item', 'items')} ({size(review.totalLogical, review.totalAllocated)}) to the {bin}?
            </h3>
            <p id="confirm-body">
              You can put them back from Tessera’s Restore page, or from the {bin}, until it is emptied. Each item is checked
              again just before it moves, and anything that has changed is left where it is.
            </p>
            <div className="actions">
              <button className="btn" onClick={() => setConfirming(false)}>Cancel</button>
              <button ref={confirmRef} className="btn btn-primary" onClick={run}>Move to {bin}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
