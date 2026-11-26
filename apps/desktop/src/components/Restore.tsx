import { useEffect, useState } from 'react'
import { api, errorText, type MovedItem } from '../api'
import { formatBytes, formatDate } from '../format'

interface Props {
  os: string
  onDone: (message: string) => void
  onError: (message: string) => void
}

const STATE_TEXT: Record<MovedItem['state'], string> = {
  inBin: '',
  restored: 'Restored',
  gone: 'No longer in the Recycle Bin',
  finder: 'Use Put Back in the Finder’s Trash',
}

/** Items Tessera moved, with Put back. Never requires Pro. */
export function Restore({ os, onDone, onError }: Props) {
  const [items, setItems] = useState<MovedItem[] | null>(null)
  const [busy, setBusy] = useState<number | null>(null)
  const bin = os === 'macos' ? 'Trash' : 'Recycle Bin'

  useEffect(() => {
    api
      .recentlyMoved()
      .then(setItems)
      .catch((e) => onError(errorText(e)))
  }, [onError])

  const putBack = (m: MovedItem) => {
    setBusy(m.index)
    api
      .putBack(m.index)
      .then((list) => {
        setItems(list)
        onDone(`${m.name} is back in its original place. Scan again to include it in the map.`)
      })
      .catch((e) => onError(errorText(e)))
      .finally(() => setBusy(null))
  }

  return (
    <div className="overview suggestions">
      <h2>Restore</h2>
      <p className="lede">
        Items Tessera moved to the {bin} in the last 30 days. Put back returns an item to where it was. It never replaces
        anything already there.
      </p>
      {items && items.length === 0 && <p className="target-meta">Nothing moved by Tessera yet.</p>}
      {items && items.length > 0 && (
        <>
          <ul className="review-list">
            {items.map((m) => (
              <li key={`${m.index}-${m.movedAt}-${m.path}`}>
                <div className="review-main">
                  <span className="review-name">{m.name}</span>
                  <span className="selectable review-path">{m.path}</span>
                  <span className="target-meta">Moved {formatDate(m.movedAt)}</span>
                </div>
                <span className="num">{formatBytes(m.bytes)}</span>
                {m.state === 'inBin' ? (
                  <button className="btn btn-quiet" disabled={busy !== null} onClick={() => putBack(m)}>
                    {busy === m.index ? 'Putting back…' : 'Put back'}
                  </button>
                ) : (
                  <span className="target-meta">{STATE_TEXT[m.state]}</span>
                )}
              </li>
            ))}
          </ul>
          <div className="actions">
            <button
              className="btn"
              onClick={() =>
                void api
                  .clearMovedHistory()
                  .then(() => setItems([]))
                  .catch((e) => onError(errorText(e)))
              }
            >
              Clear this list
            </button>
            <span className="target-meta">Clearing the list does not change any files.</span>
          </div>
        </>
      )}
    </div>
  )
}
