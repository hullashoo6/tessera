import { useEffect, useMemo, useRef, useState } from 'react'
import {
  api,
  errorText,
  type CleanupProgress,
  type CleanupReport,
  type FailedItem,
  type Metric,
  type PartialResult,
  type Review,
} from '../api'
import { baseName, formatBytes, formatCount, plural } from '../format'
import { filterItems, groupFailures, parentPath, sortItems, warningLabel, type SortKey } from '../cleanupText'
import { VirtualList } from './VirtualList'

interface Props {
  scanId: number
  marked: number[]
  metric: Metric
  os: string
  pro: boolean
  onOpenSettings: () => void
  /** Takes items off the cleanup list (never touches the files). */
  onUnmark: (ids: number[]) => void
  onFinished: (report: CleanupReport) => void
  /** Files were removed outside a full cleanup (the rest of a folder). */
  onChanged: () => void
  onRescan: () => void
  onDone: (message: string) => void
  onError: (message: string) => void
}

interface Confirming {
  permanent: boolean
  ids: number[]
  logical: number
  allocated: number
}

interface Running {
  permanent: boolean
  progress: CleanupProgress | null
  stopping: boolean
}

const ROW = 52
const SHOW_PER_GROUP = 100

export function Cleanup(props: Props) {
  const { scanId, marked, metric, os, pro, onError } = props
  const mac = os === 'macos'
  const bin = mac ? 'Trash' : 'Recycle Bin'
  const fm = mac ? 'Finder' : 'File Explorer'
  // The files are checked once for a set of items (`ids`). Taking items off
  // the list needs no new check; adding items, or a finished cleanup, does.
  const [reviewState, setReviewState] = useState<{ scanId: number; ids: Set<number>; review: Review } | null>(null)
  const [reviewFailedFor, setReviewFailedFor] = useState<string | null>(null)
  const [unchecked, setUnchecked] = useState<Set<number>>(() => new Set())
  const [query, setQuery] = useState('')
  const [sort, setSort] = useState<SortKey>('size')
  const [focus, setFocus] = useState(0)
  const [showRejected, setShowRejected] = useState(false)
  const [confirm, setConfirm] = useState<Confirming | null>(null)
  const [understood, setUnderstood] = useState(false)
  const [running, setRunning] = useState<Running | null>(null)
  const [report, setReport] = useState<CleanupReport | null>(null)
  const [partials, setPartials] = useState<Record<number, PartialResult | 'busy'>>({})
  const [undo, setUndo] = useState<null | 'busy' | { restored: number; failed: number }>(null)
  // A run waiting for the list to be checked again (Try again, or Delete
  // permanently from the results page).
  const queued = useRef<{ permanent: boolean; ids: number[] } | null>(null)
  const confirmRef = useRef<HTMLButtonElement>(null)
  const markedSet = useMemo(() => new Set(marked), [marked])
  const size = (logical: number, allocated: number) => formatBytes(metric === 'allocated' ? allocated : logical)

  const review = reviewState && reviewState.scanId === scanId ? reviewState.review : null
  const needsReview = !reviewState || reviewState.scanId !== scanId || marked.some((id) => !reviewState.ids.has(id))
  const key = marked.join(',')
  const reviewing = pro && marked.length > 0 && !running && needsReview && reviewFailedFor !== `${scanId}:${key}`

  const items = useMemo(() => (review ? review.items.filter((it) => markedSet.has(it.id)) : []), [review, markedSet])
  const rejected = useMemo(() => (review ? review.rejected.filter((r) => markedSet.has(r.id)) : []), [review, markedSet])
  const shown = useMemo(() => sortItems(filterItems(items, query), sort, metric), [items, query, sort, metric])
  const selected = useMemo(() => shown.filter((it) => !unchecked.has(it.id)), [shown, unchecked])
  const selLogical = selected.reduce((s, it) => s + it.logical, 0)
  const selAllocated = selected.reduce((s, it) => s + it.allocated, 0)
  const allOn = shown.length > 0 && selected.length === shown.length
  const someOn = selected.length > 0 && !allOn
  const ready = review !== null && !needsReview && !reviewing

  useEffect(() => {
    if (confirm) confirmRef.current?.focus()
  }, [confirm])

  const toggle = (id: number) =>
    setUnchecked((u) => {
      const next = new Set(u)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  const setAll = (on: boolean) =>
    setUnchecked((u) => {
      const next = new Set(u)
      for (const it of shown) {
        if (on) next.delete(it.id)
        else next.add(it.id)
      }
      return next
    })

  const run = (permanent: boolean, ids: number[], rv: Review | null = review) => {
    if (!rv || ids.length === 0) return
    setConfirm(null)
    setReport(null)
    setPartials({})
    setUndo(null)
    setRunning({ permanent, progress: null, stopping: false })
    let unlisten: (() => void) | null = null
    api
      .onCleanupProgress((p) => setRunning((r) => (r ? { ...r, progress: p } : r)))
      .then((un) => {
        unlisten = un
        return api.confirmCleanup(rv.reviewId, ids, permanent)
      })
      .then((r) => {
        // The review was used up; what is left on the list is checked again.
        setReviewState(null)
        setReport(r)
        props.onFinished(r)
      })
      .catch((e) => onError(errorText(e)))
      .finally(() => {
        unlisten?.()
        setRunning(null)
      })
  }

  // A run queued from the results page, once the list has been checked again.
  const startQueued = (q: { permanent: boolean; ids: number[] }, rv: Review) => {
    const available = new Set(rv.items.map((it) => it.id))
    const ids = q.ids.filter((id) => available.has(id))
    if (ids.length > 0) run(q.permanent, ids, rv)
    else props.onDone('Nothing left to try again: those items changed or are gone.')
  }
  const queue = (permanent: boolean, ids: number[]) => {
    queued.current = { permanent, ids }
    setConfirm(null)
    setReport(null)
    // The review was used up by the last cleanup, so the list is being
    // checked again; the run starts as soon as that finishes.
    if (ready && review) {
      queued.current = null
      startQueued({ permanent, ids }, review)
    }
  }

  useEffect(() => {
    if (!reviewing) return
    let live = true
    const ids = [...marked]
    api
      .reviewCleanup(scanId, ids)
      .then((r) => {
        if (!live) return
        setReviewState({ scanId, ids: new Set(ids), review: r })
        const q = queued.current
        queued.current = null
        if (q) startQueued(q, r)
      })
      .catch((e) => {
        if (!live) return
        setReviewFailedFor(`${scanId}:${ids.join(',')}`)
        onError(errorText(e))
      })
    return () => {
      live = false
    }
    // `key` captures the list contents; startQueued only reads its arguments.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reviewing, scanId, key, onError])

  const askToRemove = (permanent: boolean, list: { id: number; logical: number; allocated: number }[]) => {
    setUnderstood(false)
    setConfirm({
      permanent,
      ids: list.map((x) => x.id),
      logical: list.reduce((s, x) => s + x.logical, 0),
      allocated: list.reduce((s, x) => s + x.allocated, 0),
    })
  }

  const startConfirmed = () => {
    if (!confirm) return
    if (report || !ready) queue(confirm.permanent, confirm.ids)
    else run(confirm.permanent, confirm.ids)
  }

  const reveal = (f: FailedItem) => api.revealItem(scanId, f.id).catch((e) => onError(errorText(e)))

  const moveRest = (f: FailedItem) => {
    setPartials((p) => ({ ...p, [f.id]: 'busy' }))
    api
      .cleanupExceptInUse(scanId, f.id)
      .then((r) => {
        setPartials((p) => ({ ...p, [f.id]: r }))
        if (r.removed > 0) props.onChanged()
      })
      .catch((e) => {
        setPartials((p) => {
          const next = { ...p }
          delete next[f.id]
          return next
        })
        onError(errorText(e))
      })
  }

  const doUndo = (r: CleanupReport) => {
    setUndo('busy')
    api
      .undoCleanup(r.runId)
      .then((u) => {
        setUndo({ restored: u.restored, failed: u.failed.length })
        props.onDone(`Put back ${plural(u.restored, 'item', 'items')}`)
      })
      .catch((e) => {
        setUndo(null)
        onError(errorText(e))
      })
  }

  const confirmDialog = confirm && (
    <div className="modal-backdrop" role="presentation" onKeyDown={(e) => e.key === 'Escape' && setConfirm(null)}>
      <div className="modal" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-body">
        {confirm.permanent ? (
          <>
            <h3 id="confirm-title">
              Delete {plural(confirm.ids.length, 'item', 'items')} ({size(confirm.logical, confirm.allocated)}) permanently?
            </h3>
            <p id="confirm-body">
              They will not go to the {bin}, and nobody can put them back: not Tessera, not {mac ? 'macOS' : 'Windows'}. The
              space is freed straight away. Each item is checked again just before it goes, and anything that changed is left
              where it is.
            </p>
            <label className="confirm-check">
              <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />I understand
              this cannot be undone
            </label>
            <div className="actions">
              <button ref={confirmRef} className="btn" onClick={() => setConfirm(null)}>
                Cancel
              </button>
              <button className="btn btn-danger" disabled={!understood} onClick={startConfirmed}>
                Delete permanently
              </button>
            </div>
          </>
        ) : (
          <>
            <h3 id="confirm-title">
              Move {plural(confirm.ids.length, 'item', 'items')} ({size(confirm.logical, confirm.allocated)}) to the {bin}?
            </h3>
            <p id="confirm-body">
              You can undo this straight afterwards, or put items back later from Tessera’s Restore page or the {bin}. Each
              item is checked again just before it moves, and anything that changed is left where it is.
            </p>
            <div className="actions">
              <button className="btn" onClick={() => setConfirm(null)}>
                Cancel
              </button>
              <button ref={confirmRef} className="btn btn-primary" onClick={startConfirmed}>
                Move to {bin}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  )

  if (!pro) {
    return (
      <div className="empty">
        <div>
          <h2>Cleanup is part of Tessera Pro</h2>
          <p>
            {marked.length > 0 ? `You have chosen ${plural(marked.length, 'item', 'items')}. ` : ''}
            Activate a licence key to review items and move them to the {bin}. Scanning, the map and search stay free.
          </p>
          <button className="btn btn-primary" onClick={props.onOpenSettings}>
            Enter licence key
          </button>
        </div>
      </div>
    )
  }

  if (running) {
    const p = running.progress
    const done = p?.done ?? 0
    const total = p?.total ?? 0
    return (
      <div className="overview cleanup">
        <h2>{running.permanent ? 'Deleting permanently…' : `Moving to the ${bin}…`}</h2>
        <div
          className="meter"
          role="progressbar"
          aria-label="Cleanup progress"
          aria-valuemin={0}
          aria-valuemax={total}
          aria-valuenow={done}
        >
          <div
            className="meter-fill"
            ref={(el) => {
              if (el) el.style.width = `${total > 0 ? Math.round((done / total) * 100) : 0}%`
            }}
          />
        </div>
        <p className="lede num">
          {p
            ? `${formatCount(done)} of ${plural(total, 'item', 'items')} · ${formatBytes(p.bytes)} ${running.permanent ? 'deleted' : 'moved'}`
            : 'Starting…'}
        </p>
        <div className="actions">
          <button
            className="btn"
            disabled={running.stopping}
            onClick={() => {
              setRunning((r) => (r ? { ...r, stopping: true } : r))
              void api.stopCleanup()
            }}
          >
            {running.stopping ? 'Stopping after the current few items…' : 'Stop'}
          </button>
        </div>
      </div>
    )
  }

  if (report) {
    const removed = report.moved + report.deleted
    const permanent = report.method === 'permanent'
    const gained =
      report.freeBefore !== null && report.freeAfter !== null ? report.freeAfter - report.freeBefore : null
    const groups = groupFailures(report.failures, os)
    const failedIds = report.failures.map((f) => f.id)
    return (
      <div className="overview cleanup">
        <h2>
          {removed === 0
            ? report.gone > 0
              ? 'Nothing left to remove'
              : 'Nothing was removed'
            : permanent
              ? `Deleted ${plural(removed, 'item', 'items')}`
              : `Moved ${plural(removed, 'item', 'items')} to the ${bin}`}
        </h2>
        <p className="lede">
          {removed > 0 &&
            (permanent
              ? `${size(report.removedLogical, report.removedAllocated)} deleted permanently.${gained !== null && gained > 0 ? ` Free space went up by ${formatBytes(gained)}.` : ''} `
              : `${size(report.removedLogical, report.removedAllocated)} is now in the ${bin}. Your drive gets this space back when the ${bin} is emptied. `)}
          {report.gone > 0 && `${plural(report.gone, 'item was', 'items were')} already gone, so there was nothing to do. `}
          {report.stopped &&
            `Stopped: ${plural(report.notStarted, 'item was', 'items were')} not started and ${report.notStarted === 1 ? 'is' : 'are'} still in your cleanup list.`}
        </p>
        {removed > 0 && !permanent && (
          <div className="actions result-actions">
            <button className="btn" onClick={() => api.openRecycleBin().catch((e) => onError(errorText(e)))}>
              Open {bin}
            </button>
            {undo === null && report.moved > 0 && (
              <button className="btn" onClick={() => doUndo(report)}>
                Undo: put {report.moved === 1 ? 'it' : 'them all'} back
              </button>
            )}
            {undo === 'busy' && (
              <button className="btn" disabled>
                Putting back…
              </button>
            )}
          </div>
        )}
        {undo !== null && undo !== 'busy' && (
          <p className="notice" role="status">
            Put back {plural(undo.restored, 'item', 'items')}
            {undo.failed > 0 && ` (${plural(undo.failed, 'item', 'items')} could not be put back; see the Restore page)`}. Scan
            again to see {undo.restored === 1 ? 'it' : 'them'} on the map.{' '}
            <button className="btn btn-quiet inline-btn" onClick={props.onRescan}>
              Scan again
            </button>
          </p>
        )}

        {groups.length > 0 && (
          <>
            <div className="section-title">
              {plural(report.failed, 'item was', 'items were')} left in place
            </div>
            {groups.map((g) => (
              <section key={g.key} className="fail-group" aria-label={g.title}>
                <h3>
                  <span>{g.title}</span> <span className="num muted">· {plural(g.items.length, 'item', 'items')}</span>
                </h3>
                <p>{g.text}</p>
                <ul className="review-list">
                  {g.items.slice(0, SHOW_PER_GROUP).map((f) => {
                    const part = partials[f.id]
                    const isDir = f.failure.inUseFiles.length > 0
                    return (
                      <li key={f.id}>
                        <div className="review-main">
                          <span className="review-name">{baseName(f.path)}</span>
                          <span className="selectable review-path">{parentPath(f.path)}</span>
                          {f.failure.inUseFiles.length > 0 && (
                            <span className="review-path">
                              Open inside: {f.failure.inUseFiles.map(baseName).join(', ')}
                            </span>
                          )}
                          {(g.kind === 'other' || g.kind === 'permission') && f.failure.detail && (
                            <span className="review-path">{f.failure.detail}</span>
                          )}
                          {part && part !== 'busy' && (
                            <span className="review-reason" role="status">
                              {!part.checked
                                ? 'This folder is too big to check for open files quickly, so nothing was moved.'
                                : `Moved ${plural(part.removed, 'item', 'items')} (${size(part.removedLogical, part.removedAllocated)}) from inside it. ${plural(part.inUseCount, 'file is', 'files are')} still open${part.apps.length > 0 ? ` in ${part.apps.join(', ')}` : ''}.`}
                            </span>
                          )}
                        </div>
                        <span className="num">{size(f.logical, f.allocated)}</span>
                        <span className="row-actions">
                          {g.kind === 'inUse' && isDir && !part && (
                            <button className="btn btn-quiet" onClick={() => moveRest(f)}>
                              Move the rest
                            </button>
                          )}
                          {part === 'busy' && (
                            <button className="btn btn-quiet" disabled>
                              Moving…
                            </button>
                          )}
                          <button className="btn btn-quiet" onClick={() => reveal(f)}>
                            Show in {fm}
                          </button>
                        </span>
                      </li>
                    )
                  })}
                </ul>
                {g.items.length > SHOW_PER_GROUP && (
                  <p className="muted">and {plural(g.items.length - SHOW_PER_GROUP, 'more item', 'more items')}</p>
                )}
                {g.kind === 'tooLarge' && (
                  <div className="actions">
                    <button className="btn btn-danger-quiet" onClick={() => askToRemove(true, g.items)}>
                      Delete {g.items.length === 1 ? 'it' : 'these'} permanently…
                    </button>
                  </div>
                )}
              </section>
            ))}
          </>
        )}
        <div className="actions">
          {failedIds.length > 0 && (
            <button className="btn btn-primary" onClick={() => queue(permanent, failedIds)}>
              Try again
            </button>
          )}
          {failedIds.length > 0 && (
            <button
              className="btn"
              onClick={() => {
                props.onUnmark(failedIds)
                setReport(null)
              }}
            >
              Take {failedIds.length === 1 ? 'it' : 'them'} off the list
            </button>
          )}
          <button className={failedIds.length > 0 ? 'btn btn-quiet' : 'btn btn-primary'} onClick={() => setReport(null)}>
            {marked.length > 0 ? 'Back to the cleanup list' : 'Done'}
          </button>
        </div>
        {confirmDialog}
      </div>
    )
  }

  if (marked.length === 0) {
    return (
      <div className="empty">
        <div>
          <h2>Your cleanup list is empty</h2>
          <p>
            Choose Add to cleanup on anything you no longer need: in the storage map (right-click), Largest files, Search,
            Smart Scan or Duplicates. You can also select something and press {mac ? '⌘ Delete' : 'Delete'}. Nothing is
            removed until you review the list here.
          </p>
        </div>
      </div>
    )
  }

  const focused = shown[Math.min(focus, shown.length - 1)]
  return (
    <div className="overview cleanup cleanup-page">
      <h2>Cleanup list</h2>
      <p className="lede">
        {review
          ? `${plural(items.length, 'item', 'items')}, ${size(
              items.reduce((s, it) => s + it.logical, 0),
              items.reduce((s, it) => s + it.allocated, 0),
            )} in total. Nothing is removed until you choose.`
          : 'Checking the items on your list…'}
        {review && review.merged > 0 && ` ${plural(review.merged, 'item is', 'items are')} already included through a chosen folder.`}
      </p>

      {items.length > 0 && (
        <div className="clean-toolbar">
          <label className="clean-all">
            <input
              type="checkbox"
              checked={allOn}
              ref={(el) => {
                if (el) el.indeterminate = someOn
              }}
              onChange={() => setAll(!allOn)}
              aria-label={allOn ? 'Select none' : 'Select all'}
            />
            <span className="num">
              {selected.length === shown.length ? `All ${formatCount(shown.length)} selected` : `${formatCount(selected.length)} of ${formatCount(shown.length)} selected`}
              {' · '}
              {size(selLogical, selAllocated)}
            </span>
          </label>
          <span className="spacer" />
          <input
            className="search-input clean-filter"
            type="search"
            placeholder="Filter by name or folder"
            aria-label="Filter the cleanup list"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setFocus(0)
            }}
          />
          <select aria-label="Sort" value={sort} onChange={(e) => setSort(e.target.value as SortKey)}>
            <option value="size">Largest first</option>
            <option value="name">Name</option>
            <option value="folder">Folder</option>
          </select>
          <button
            className="btn"
            disabled={selected.length === 0}
            onClick={() => props.onUnmark(selected.map((it) => it.id))}
            title="Takes them off this list. The files stay where they are."
          >
            Take {selected.length === shown.length && !query ? 'all' : formatCount(selected.length)} off the list
          </button>
        </div>
      )}
      {query && items.length > 0 && (
        <p className="muted clean-hint">
          Showing {formatCount(shown.length)} of {formatCount(items.length)}. Buttons act on the selected items shown.
        </p>
      )}

      {shown.length > 0 && (
        <div
          className="clean-list"
          onKeyDown={(e) => {
            if (!focused) return
            if (e.key === ' ') {
              e.preventDefault()
              toggle(focused.id)
            } else if (e.key === 'Delete' || e.key === 'Backspace') {
              e.preventDefault()
              props.onUnmark([focused.id])
            }
          }}
        >
          <VirtualList
            items={shown}
            rowHeight={ROW}
            label="Cleanup list. Space selects or clears an item, Delete takes it off the list."
            selectedIndex={Math.min(focus, shown.length - 1)}
            onSelectIndex={setFocus}
            onActivate={(i) => toggle(shown[i].id)}
            rowClassName="clean-row"
            renderRow={(it) => (
              <>
                <input
                  type="checkbox"
                  checked={!unchecked.has(it.id)}
                  onChange={() => toggle(it.id)}
                  onClick={(e) => e.stopPropagation()}
                  aria-label={`Include ${it.name}`}
                  tabIndex={-1}
                />
                <div className="review-main">
                  <span className="review-name">
                    {it.name}
                    {it.kind === 'dir' && <span className="kind-tag">folder</span>}
                    {it.warning && (
                      <span className="clean-warn" title={it.warning}>
                        {warningLabel(it.warning)}
                      </span>
                    )}
                  </span>
                  <span className="review-path">{parentPath(it.path)}</span>
                </div>
                <span className="num">{size(it.logical, it.allocated)}</span>
                <button
                  className="btn btn-quiet btn-x"
                  aria-label={`Take ${it.name} off the list`}
                  title="Take off the list (the file stays)"
                  tabIndex={-1}
                  onClick={(e) => {
                    e.stopPropagation()
                    props.onUnmark([it.id])
                  }}
                >
                  ×
                </button>
              </>
            )}
          />
        </div>
      )}
      {query && shown.length === 0 && items.length > 0 && <p className="muted">Nothing on the list matches “{query}”.</p>}

      {rejected.length > 0 && (
        <div className="rejected">
          <button className="btn btn-quiet" aria-expanded={showRejected} onClick={() => setShowRejected((s) => !s)}>
            {showRejected ? '▾' : '▸'} {plural(rejected.length, 'item', 'items')} Tessera won’t remove
          </button>
          {showRejected && (
            <>
              <ul className="review-list muted-list">
                {rejected.slice(0, SHOW_PER_GROUP).map((r) => (
                  <li key={r.id}>
                    <div className="review-main">
                      <span className="selectable review-path">{r.path}</span>
                      <span className="review-reason">{r.reason}</span>
                    </div>
                  </li>
                ))}
              </ul>
              <button className="btn" onClick={() => props.onUnmark(rejected.map((r) => r.id))}>
                Take {rejected.length === 1 ? 'it' : 'them'} off the list
              </button>
            </>
          )}
        </div>
      )}

      {items.length > 0 && (
        <div className="clean-footer">
          <p className="muted">
            Moving to the {bin} can be undone. The space comes back when the {bin} is emptied.
          </p>
          <div className="actions">
            <button
              className="btn btn-danger-quiet"
              disabled={!ready || selected.length === 0}
              onClick={() => askToRemove(true, selected)}
            >
              Delete permanently…
            </button>
            <button
              className="btn btn-primary"
              disabled={!ready || selected.length === 0}
              onClick={() => askToRemove(false, selected)}
            >
              {selected.length === 0
                ? `Move to the ${bin}`
                : `Move ${plural(selected.length, 'item', 'items')} (${size(selLogical, selAllocated)}) to the ${bin}`}
            </button>
          </div>
        </div>
      )}
      {confirmDialog}
    </div>
  )
}
