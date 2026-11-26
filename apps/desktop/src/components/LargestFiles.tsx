import { useEffect, useState } from 'react'
import { api, errorText, type FileRow, type Metric } from '../api'
import { formatBytes, formatDate, platformNames } from '../format'
import type { ItemCommand } from './Details'
import { VirtualList } from './VirtualList'

interface Props {
  scanId: number
  viewRoot: number
  viewRootName: string
  metric: Metric
  os: string
  version: number
  onCommand: (command: ItemCommand, id: number) => void
  marked: Set<number>
  onToggleMark: (id: number) => void
  onError: (message: string) => void
}

const LIMIT = 1000

function location(row: FileRow): string {
  const cut = Math.max(row.path.lastIndexOf('\\'), row.path.lastIndexOf('/'))
  return cut > 0 ? row.path.slice(0, cut) : row.path
}

export function LargestFiles({ scanId, viewRoot, viewRootName, metric, os, version, onCommand, marked, onToggleMark, onError }: Props) {
  const [scope, setScope] = useState<'all' | 'folder'>('all')
  const [rows, setRows] = useState<FileRow[] | null>(null)
  const [selected, setSelected] = useState(-1)
  const names = platformNames(os)
  const under = scope === 'folder' ? viewRoot : null

  useEffect(() => {
    let live = true
    api
      .largestFiles(scanId, under, metric, LIMIT)
      .then((r) => {
        if (!live) return
        setRows(r)
        setSelected(-1)
      })
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [scanId, under, metric, version, onError])

  return (
    <div className="table-view">
      <div className="toolbar">
        <h1>Largest files</h1>
        <div className="segmented" role="group" aria-label="Which files to list">
          <button aria-pressed={scope === 'all'} onClick={() => setScope('all')}>
            Whole scan
          </button>
          <button aria-pressed={scope === 'folder'} disabled={viewRoot === 0} onClick={() => setScope('folder')}>
            {viewRoot === 0 ? 'Current folder' : `In ${viewRootName}`}
          </button>
        </div>
        <span className="spacer" />
        <span className="target-meta num">{rows ? `Top ${rows.length.toLocaleString()} by ${metric === 'allocated' ? 'size on disk' : 'file size'}` : ''}</span>
      </div>
      <div className="map-pane">
        <div className="table-head" role="presentation">
          <span>{metric === 'allocated' ? 'On disk' : 'Size'}</span>
          <span>Name</span>
          <span>Location</span>
          <span>Modified</span>
          <span />
        </div>
        {rows?.length === 0 ? (
          <div className="empty">
            <p>No files found here.</p>
          </div>
        ) : (
          <VirtualList
            items={rows ?? []}
            rowHeight={40}
            label="Largest files"
            rowClassName="file-row"
            selectedIndex={selected}
            onSelectIndex={setSelected}
            onActivate={(i) => {
              const r = rows?.[i]
              if (r) onCommand('reveal', r.node.id)
            }}
            renderRow={(r) => (
              <>
                <span className="num">{formatBytes(metric === 'allocated' ? r.node.allocated : r.node.logical)}</span>
                <span className="row-name" title={r.node.name}>
                  {r.node.name}
                </span>
                <span className="path" title={location(r)}>
                  <bdi>{location(r)}</bdi>
                </span>
                <span className="target-meta">{formatDate(r.node.modified)}</span>
                <span className="row-actions">
                  <button
                    className="btn btn-quiet"
                    title={names.showIn}
                    aria-label={`${names.showIn}: ${r.node.name}`}
                    onClick={() => onCommand('reveal', r.node.id)}
                  >
                    Show
                  </button>
                  <button className="btn btn-quiet" aria-pressed={marked.has(r.node.id)} onClick={() => onToggleMark(r.node.id)}>
                    {marked.has(r.node.id) ? 'Added' : 'Add to cleanup'}
                  </button>
                  <button className="btn btn-quiet" onClick={() => onCommand('copy', r.node.id)}>
                    Copy path
                  </button>
                </span>
              </>
            )}
          />
        )}
      </div>
    </div>
  )
}
