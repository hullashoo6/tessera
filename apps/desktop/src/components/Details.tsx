import { useCallback, useEffect, useState } from 'react'
import { api, errorText, type Details as DetailsData, type Metric, type NodeInfo } from '../api'
import { baseName, formatBytes, formatDate, percent, platformNames, plural } from '../format'
import { swatch, type Theme } from '../palette'
import { VirtualList } from './VirtualList'

export type ItemCommand = 'zoom' | 'open' | 'reveal' | 'copy'

interface Props {
  scanId: number
  viewRoot: number
  selected: number | null
  metric: Metric
  theme: Theme
  os: string
  version: number
  onSelect: (id: number) => void
  onCommand: (command: ItemCommand, id: number) => void
  onError: (message: string) => void
}

const NOTES: Record<string, string> = {
  pending: 'Still being scanned.',
  unreadable: 'Tessera was not allowed to read this folder, so its contents are not included.',
  partial: 'This folder could only be read in part.',
  incomplete: 'Some folders inside could not be read, so the real total may be higher.',
  countedElsewhere:
    'This is another name (hard link) for a file already counted elsewhere, so its size is not added again here.',
  hardLinked: 'This file has more than one name (hard link). Removing one name frees no space while another remains.',
  cloudOnly: 'Online-only file: its contents are stored in the cloud and use no space on this disk.',
  containsCloudFiles: 'Contains online-only files, which use no space on this disk.',
  sparse: 'Sparse file: it uses less disk space than its size suggests.',
  compressed: 'Compressed by the file system, so it uses less disk space than its size.',
  otherFilesystem: 'On a different drive, so it is not included.',
  metadataUnavailable: 'Details for this item could not be read.',
}

function kindText(n: NodeInfo, isRoot: boolean): string {
  if (isRoot) return 'Scanned location'
  if (n.kind === 'dir') return 'Folder'
  if (n.kind === 'link') return 'Link'
  if (n.kind === 'other') return 'Special file'
  return 'File'
}

const PAGE = 200

export function Details(props: Props) {
  const { scanId, viewRoot, selected, metric, theme, os, version, onSelect, onCommand, onError } = props
  const focusId = selected ?? viewRoot
  const [details, setDetails] = useState<DetailsData | null>(null)
  const [rootSize, setRootSize] = useState(0)
  const [rootName, setRootName] = useState('')
  const [items, setItems] = useState<NodeInfo[]>([])
  const [total, setTotal] = useState(0)
  const names = platformNames(os)

  useEffect(() => {
    let live = true
    api
      .nodeDetails(scanId, focusId)
      .then((d) => live && setDetails(d))
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [scanId, focusId, version, onError])

  useEffect(() => {
    let live = true
    Promise.all([api.nodeDetails(scanId, viewRoot), api.children(scanId, viewRoot, metric, 0, PAGE)])
      .then(([d, page]) => {
        if (!live) return
        setRootSize(metric === 'allocated' ? d.node.allocated : d.node.logical)
        setRootName(baseName(d.node.name))
        setItems(page.items)
        setTotal(page.total)
      })
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [scanId, viewRoot, metric, version, onError])

  const loadMore = useCallback(() => {
    api
      .children(scanId, viewRoot, metric, items.length, PAGE)
      .then((page) => setItems((prev) => (prev.length === items.length ? [...prev, ...page.items] : prev)))
      .catch((e) => onError(errorText(e)))
  }, [scanId, viewRoot, metric, items.length, onError])

  const n = details?.node
  const isRoot = n?.id === viewRoot && details?.crumbs.length === 1
  const size = n ? (metric === 'allocated' ? n.allocated : n.logical) : 0
  const other = n ? (metric === 'allocated' ? n.logical : n.allocated) : 0
  const estimated = !!n && (n.flags.includes('estimated') || n.flags.includes('containsEstimates'))
  const notes = n ? n.flags.filter((f) => NOTES[f]).map((f) => NOTES[f]!) : []
  if (n?.kind === 'link') notes.unshift('A link to another location. Links are not followed, so the target is not counted.')
  const selectedIndex = items.findIndex((i) => i.id === selected)
  const listMax = items.reduce((m, i) => Math.max(m, metric === 'allocated' ? i.allocated : i.logical), 0)

  return (
    <aside className="details" aria-label="Details">
      <div className="details-head">
        {n ? (
          <>
            <div className="details-kind">{kindText(n, isRoot)}</div>
            <div className="details-name selectable">{n.name}</div>
            <div className="figure num">{formatBytes(size)}</div>
            <div className="figure-note num">
              {metric === 'allocated'
                ? `On disk${estimated ? ' (estimated)' : ''}. File size ${formatBytes(other)}.`
                : `File size. On disk ${formatBytes(other)}${estimated ? ' (estimated)' : ''}.`}
            </div>
            <dl className="facts">
              {n.kind === 'dir' && (
                <>
                  <dt>Contains</dt>
                  <dd className="num">
                    {plural(n.files, 'file', 'files')} in {plural(n.dirs, 'folder', 'folders')}
                  </dd>
                </>
              )}
              {!isRoot && rootName && (
                <>
                  <dt>Share</dt>
                  <dd className="num">{percent(size, rootSize)} of {rootName}</dd>
                </>
              )}
              <dt>Modified</dt>
              <dd>{formatDate(n.modified)}</dd>
              <dt>Location</dt>
              <dd className="selectable">{details?.path}</dd>
            </dl>
            {notes.length > 0 && (
              <ul className="notes">
                {notes.map((t) => (
                  <li key={t}>{t}</li>
                ))}
              </ul>
            )}
            <div className="actions">
              {n.kind === 'dir' && n.id !== viewRoot && (
                <button className="btn" onClick={() => onCommand('zoom', n.id)}>
                  Open in map
                </button>
              )}
              <button className="btn" onClick={() => onCommand('reveal', n.id)}>
                {names.showIn}
              </button>
              {(n.kind === 'file' || n.kind === 'dir') && (
                <button className="btn btn-quiet" onClick={() => onCommand('open', n.id)}>
                  Open
                </button>
              )}
              <button className="btn btn-quiet" onClick={() => onCommand('copy', n.id)}>
                Copy path
              </button>
            </div>
          </>
        ) : (
          <div className="details-kind">Loading…</div>
        )}
      </div>
      <div className="map-pane">
        <div className="list-head">
          <span>Contents of {rootName || 'this folder'}</span>
          <span className="num">{plural(total, 'item', 'items')}</span>
        </div>
        <VirtualList
          items={items}
          total={total}
          rowHeight={34}
          label={`Contents of ${rootName}`}
          rowClassName="row"
          selectedIndex={selectedIndex}
          onSelectIndex={(i) => {
            const it = items[i]
            if (it) onSelect(it.id)
          }}
          onActivate={(i) => {
            const it = items[i]
            if (it?.kind === 'dir') onCommand('zoom', it.id)
          }}
          onEndReached={loadMore}
          renderRow={(it, i) => {
            const s = metric === 'allocated' ? it.allocated : it.logical
            return (
              <>
                <span
                  className="row-swatch"
                  ref={(el) => {
                    if (el) el.style.background = swatch(i, it.kind, theme)
                  }}
                />
                <span className="row-name" title={it.name}>
                  {it.name}
                </span>
                <span className="row-size num">{formatBytes(s)}</span>
                <span
                  className="row-bar"
                  ref={(el) => {
                    if (el) el.style.width = `calc((100% - 32px) * ${listMax > 0 ? s / listMax : 0})`
                  }}
                />
              </>
            )
          }}
        />
      </div>
    </aside>
  )
}
