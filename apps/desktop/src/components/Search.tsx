import { useEffect, useState } from 'react'
import { api, errorText, type FileRow, type Metric, type SearchQuery, type SortKey } from '../api'
import { formatBytes, formatDate, platformNames, plural, unitBytes } from '../format'
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
  onHighlight: (matches: number[], containing: number[], label: string) => void
  marked: Set<number>
  onToggleMark: (id: number) => void
  onError: (message: string) => void
}

// File-type groups are plain extension lists: deterministic and explainable.
const TYPES: [string, string, string[]][] = [
  ['any', 'Any type', []],
  ['video', 'Videos', ['mp4', 'mov', 'mkv', 'avi', 'wmv', 'm4v', 'webm', 'mts', 'm2ts']],
  ['image', 'Images', ['jpg', 'jpeg', 'png', 'gif', 'heic', 'webp', 'tif', 'tiff', 'bmp', 'cr2', 'cr3', 'nef', 'arw', 'dng', 'psd']],
  ['audio', 'Audio', ['mp3', 'wav', 'flac', 'aac', 'm4a', 'ogg', 'aiff', 'wma']],
  ['doc', 'Documents', ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'rtf', 'odt', 'pages', 'numbers', 'key', 'md']],
  ['archive', 'Archives', ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'tgz']],
  ['installer', 'Installers and disk images', ['exe', 'msi', 'dmg', 'pkg', 'iso', 'img', 'vhd', 'vhdx', 'vmdk', 'qcow2']],
]

type Kind = 'files' | 'folders' | 'both' | 'empty'
type Age = 'any' | 'recent' | 'year' | 'three'
const DAY = 86_400

const sizes = (): [string, string, number | null][] => [
  ['any', 'Any size', null],
  ['10m', '10 MB or more', unitBytes(10, 'MB')],
  ['100m', '100 MB or more', unitBytes(100, 'MB')],
  ['1g', '1 GB or more', unitBytes(1, 'GB')],
  ['10g', '10 GB or more', unitBytes(10, 'GB')],
]

export function Search(props: Props) {
  const { scanId, viewRoot, viewRootName, metric, os, version, onCommand, onHighlight, marked, onToggleMark, onError } = props
  const [text, setText] = useState('')
  const [debounced, setDebounced] = useState('')
  const [matchPath, setMatchPath] = useState(false)
  const [type, setType] = useState('any')
  const [kind, setKind] = useState<Kind>('files')
  const [size, setSize] = useState('any')
  const [age, setAge] = useState<Age>('any')
  const [inFolder, setInFolder] = useState(false)
  const [sort, setSort] = useState<SortKey>('size')
  const [rows, setRows] = useState<FileRow[]>([])
  const [total, setTotal] = useState(0)
  const [containing, setContaining] = useState<number[]>([])
  const [selected, setSelected] = useState(-1)
  const names = platformNames(os)

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(text), 220)
    return () => window.clearTimeout(t)
  }, [text])

  useEffect(() => {
    const now = Math.floor(Date.now() / 1000)
    const query: SearchQuery = {
      text: debounced,
      matchPath,
      extensions: TYPES.find((t) => t[0] === type)?.[2] ?? [],
      files: kind === 'files' || kind === 'both',
      folders: kind === 'folders' || kind === 'both',
      minSize: sizes().find((s) => s[0] === size)?.[2] ?? null,
      maxSize: null,
      modifiedBefore: age === 'year' ? now - 365 * DAY : age === 'three' ? now - 3 * 365 * DAY : null,
      modifiedAfter: age === 'recent' ? now - 30 * DAY : null,
      emptyFolders: kind === 'empty',
      under: inFolder && viewRoot !== 0 ? viewRoot : null,
    }
    let live = true
    api
      .search(scanId, query, metric, sort, 2000)
      .then((r) => {
        if (!live) return
        setRows(r.items)
        setTotal(r.total)
        setContaining(r.containing)
        setSelected(-1)
      })
      .catch((e) => live && onError(errorText(e)))
    return () => {
      live = false
    }
  }, [scanId, debounced, matchPath, type, kind, size, age, inFolder, viewRoot, metric, sort, version, onError])

  return (
    <div className="table-view search-view">
      <div className="toolbar search-bar">
        <input
          className="search-input"
          type="search"
          placeholder={matchPath ? 'Search in paths' : 'Search by name'}
          aria-label={matchPath ? 'Search in paths' : 'Search by name'}
          value={text}
          onChange={(e) => setText(e.target.value)}
          autoFocus
        />
        <select aria-label="File type" value={type} onChange={(e) => setType(e.target.value)} disabled={kind !== 'files'}>
          {TYPES.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
        </select>
        <select aria-label="Size" value={size} onChange={(e) => setSize(e.target.value)} disabled={kind === 'empty'}>
          {sizes().map(([v, l]) => <option key={v} value={v}>{l}</option>)}
        </select>
        <select aria-label="Last modified" value={age} onChange={(e) => setAge(e.target.value as Age)}>
          <option value="any">Any date</option>
          <option value="recent">Changed in the last 30 days</option>
          <option value="year">Not changed for a year</option>
          <option value="three">Not changed for 3 years</option>
        </select>
        <select aria-label="Show" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>
          <option value="files">Files</option>
          <option value="folders">Folders</option>
          <option value="both">Files and folders</option>
          <option value="empty">Empty folders</option>
        </select>
        <select aria-label="Sort" value={sort} onChange={(e) => setSort(e.target.value as SortKey)}>
          <option value="size">Largest first</option>
          <option value="name">Name, A to Z</option>
          <option value="modified">Oldest first</option>
          <option value="type">File type</option>
        </select>
      </div>
      <div className="search-options">
        <label><input type="checkbox" checked={matchPath} onChange={(e) => setMatchPath(e.target.checked)} /> Match full path</label>
        <label>
          <input type="checkbox" checked={inFolder} disabled={viewRoot === 0} onChange={(e) => setInFolder(e.target.checked)} /> Only in{' '}
          {viewRoot === 0 ? 'the current folder' : viewRootName}
        </label>
        <span className="spacer" />
        <span className="num">
          {total > rows.length ? `${plural(total, 'match', 'matches')}, showing the first ${rows.length.toLocaleString()}` : plural(total, 'match', 'matches')}
        </span>
        <button
          className="btn"
          disabled={rows.length === 0}
          onClick={() => {
            const what = debounced ? `for “${debounced}”` : 'for these filters'
            const count =
              total > rows.length
                ? `the largest ${rows.length.toLocaleString()} of ${plural(total, 'match', 'matches')}`
                : plural(total, 'match', 'matches')
            onHighlight(rows.map((r) => r.node.id), containing, `${count} ${what}`)
          }}
        >
          Show on map
        </button>
      </div>
      <div className="map-pane">
        <div className="table-head" role="presentation">
          <span>{metric === 'allocated' ? 'On disk' : 'Size'}</span>
          <span>Name</span>
          <span>Location</span>
          <span>Modified</span>
          <span />
        </div>
        {rows.length === 0 ? (
          <div className="empty">
            <p>Nothing matches. Try a shorter name or fewer filters.</p>
          </div>
        ) : (
          <VirtualList
            items={rows}
            rowHeight={40}
            label="Search results"
            rowClassName="file-row"
            selectedIndex={selected}
            onSelectIndex={setSelected}
            onActivate={(i) => {
              const r = rows[i]
              if (r) onCommand(r.node.kind === 'dir' ? 'zoom' : 'reveal', r.node.id)
            }}
            renderRow={(r) => (
              <>
                <span className="num">{formatBytes(metric === 'allocated' ? r.node.allocated : r.node.logical)}</span>
                <span className="row-name" title={r.node.name}>{r.node.name}{r.node.kind === 'dir' ? ' (folder)' : ''}</span>
                <span className="path" title={r.path}><bdi>{r.path}</bdi></span>
                <span className="target-meta">{formatDate(r.node.modified)}</span>
                <span className="row-actions">
                  {r.node.kind === 'dir' && (
                    <button className="btn btn-quiet" onClick={() => onCommand('zoom', r.node.id)}>Map</button>
                  )}
                  <button className="btn btn-quiet" title={names.showIn} aria-label={`${names.showIn}: ${r.node.name}`} onClick={() => onCommand('reveal', r.node.id)}>Show</button>
                  <button className="btn btn-quiet" aria-pressed={marked.has(r.node.id)} onClick={() => onToggleMark(r.node.id)}>{marked.has(r.node.id) ? 'Added' : 'Add to cleanup'}</button>
                </span>
              </>
            )}
          />
        )}
      </div>
    </div>
  )
}
