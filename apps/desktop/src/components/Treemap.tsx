import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { api, errorText, type Metric, type Tile } from '../api'
import { formatBytes, percent, plural } from '../format'
import {
  frameFill,
  grout,
  hatchInk,
  highContrast,
  hoverInk,
  labelInk,
  moreFill,
  selectionInk,
  tileFill,
  type Theme,
} from '../palette'

interface Props {
  scanId: number
  rootId: number
  rootName: string
  metric: Metric
  theme: Theme
  /** The system asks for high contrast (redraws with the high-contrast palette). */
  contrast: boolean
  /** Changing this re-requests the layout (used while a scan is running). */
  version: number
  selected: number | null
  highlight: Highlight | null
  onSelect: (id: number) => void
  onZoom: (id: number) => void
  onUp: () => void
  onMenu: (tile: Tile, x: number, y: number) => void
  onError: (message: string) => void
}

interface Size {
  w: number
  h: number
}

function tileName(t: Tile): string {
  return t.kind === 'more' ? `${plural(t.count, 'smaller item', 'smaller items')}` : t.name
}

function kindLabel(t: Tile): string {
  if (t.kind === 'more') return 'Too small to show individually'
  if (t.kind === 'dir') return t.pending ? 'Folder, still being scanned' : 'Folder'
  if (t.kind === 'link') return 'Link (not followed)'
  return 'File'
}

/** Deepest tile containing the point. Children follow parents in the array. */
function hitTest(tiles: Tile[], x: number, y: number): Tile | null {
  for (let i = tiles.length - 1; i >= 0; i--) {
    const t = tiles[i]!
    if (x >= t.x && x < t.x + t.w && y >= t.y && y < t.y + t.h) return t
  }
  return null
}

/** Innermost folder containing the point (for double-click zooming). */
function folderAt(tiles: Tile[], x: number, y: number): Tile | null {
  for (let i = tiles.length - 1; i >= 0; i--) {
    const t = tiles[i]!
    if (t.kind === 'dir' && x >= t.x && x < t.x + t.w && y >= t.y && y < t.y + t.h) return t
  }
  return null
}

function fit(ctx: CanvasRenderingContext2D, text: string, max: number): string {
  if (max <= 0) return ''
  if (ctx.measureText(text).width <= max) return text
  let lo = 0
  let hi = text.length
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (ctx.measureText(text.slice(0, mid) + '…').width <= max) lo = mid
    else hi = mid - 1
  }
  return lo === 0 ? '' : text.slice(0, lo) + '…'
}

function prepare(canvas: HTMLCanvasElement, size: Size): CanvasRenderingContext2D | null {
  const dpr = window.devicePixelRatio || 1
  const pw = Math.max(1, Math.round(size.w * dpr))
  const ph = Math.max(1, Math.round(size.h * dpr))
  if (canvas.width !== pw) canvas.width = pw
  if (canvas.height !== ph) canvas.height = ph
  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  return ctx
}

function hatch(ctx: CanvasRenderingContext2D, theme: Theme): CanvasPattern | null {
  const c = document.createElement('canvas')
  c.width = 6
  c.height = 6
  const p = c.getContext('2d')
  if (!p) return null
  p.strokeStyle = hatchInk(theme)
  p.lineWidth = 1
  p.beginPath()
  p.moveTo(0, 6)
  p.lineTo(6, 0)
  p.stroke()
  return ctx.createPattern(c, 'repeat')
}

function drawBase(canvas: HTMLCanvasElement, tiles: Tile[], size: Size, theme: Theme): void {
  const ctx = prepare(canvas, size)
  if (!ctx) return
  const font = getComputedStyle(document.documentElement).fontFamily
  ctx.fillStyle = grout(theme)
  ctx.fillRect(0, 0, size.w, size.h)
  const pattern = hatch(ctx, theme)
  const ink = labelInk(theme)
  // Sizes are drawn a little lighter than names, except in high contrast.
  const faint = highContrast() ? 1 : 0.7

  for (const t of tiles) {
    // A one-pixel "grout" line separates every tile.
    const x = t.x + 0.5
    const y = t.y + 0.5
    const w = t.w - 1
    const h = t.h - 1
    if (w <= 0.5 || h <= 0.5) continue
    ctx.globalAlpha = t.pending ? 0.6 : 1
    if (t.kind === 'more') {
      ctx.fillStyle = moreFill(theme)
      ctx.fillRect(x, y, w, h)
      if (pattern && w > 5 && h > 5) {
        ctx.fillStyle = pattern
        ctx.fillRect(x, y, w, h)
      }
    } else {
      ctx.fillStyle = t.opened ? frameFill(t, theme) : tileFill(t, theme)
      ctx.fillRect(x, y, w, h)
    }
    ctx.globalAlpha = 1

    ctx.fillStyle = ink
    if (t.opened) {
      if (w > 40) {
        ctx.font = `600 11.5px ${font}`
        const sizeText = formatBytes(t.size)
        const sizeW = ctx.measureText(sizeText).width
        const room = w - 12
        if (room > sizeW + 50) {
          ctx.fillText(fit(ctx, t.name, room - sizeW - 10), x + 6, y + 12.5)
          ctx.globalAlpha = faint
          ctx.fillText(sizeText, x + w - 6 - sizeW, y + 12.5)
          ctx.globalAlpha = 1
        } else {
          ctx.fillText(fit(ctx, t.name, room), x + 6, y + 12.5)
        }
      }
    } else if (w > 48 && h > 22) {
      ctx.font = `12px ${font}`
      ctx.fillText(fit(ctx, tileName(t), w - 12), x + 6, y + 16)
      if (h > 38) {
        ctx.globalAlpha = faint
        ctx.fillText(fit(ctx, formatBytes(t.size), w - 12), x + 6, y + 31)
        ctx.globalAlpha = 1
      }
    }
  }
}

function outline(ctx: CanvasRenderingContext2D, t: Tile, color: string, thin: number): void {
  // Thicker outlines in high contrast so the selection stands out from the solid tile gaps.
  const width = highContrast() ? thin + 1 : thin
  ctx.strokeStyle = color
  ctx.lineWidth = width
  const inset = width / 2 + 0.5
  ctx.strokeRect(t.x + inset, t.y + inset, Math.max(0, t.w - 2 * inset), Math.max(0, t.h - 2 * inset))
}

export interface Highlight {
  matches: Set<number>
  containing: Set<number>
  label: string
}

const inside = (outer: Tile, t: Tile) =>
  t.x >= outer.x && t.y >= outer.y && t.x + t.w <= outer.x + outer.w + 0.01 && t.y + t.h <= outer.y + outer.h + 0.01

function drawHighlight(ctx: CanvasRenderingContext2D, tiles: Tile[], theme: Theme, h: Highlight): void {
  const matched = tiles.filter((t) => t.kind !== 'more' && h.matches.has(t.id))
  const checkInside = matched.length <= 200
  ctx.fillStyle = theme === 'dark' ? 'rgba(22, 25, 29, 0.66)' : 'rgba(246, 247, 248, 0.7)'
  for (const t of tiles) {
    const keep =
      (t.kind === 'more' ? h.containing.has(t.id) : h.matches.has(t.id) || h.containing.has(t.id)) ||
      (checkInside && matched.some((m) => m !== t && inside(m, t)))
    if (keep) continue
    // Opened folders: dim only their label strip; their contents are dimmed individually.
    ctx.fillRect(t.x, t.y, t.w, t.opened ? Math.min(18, t.h) : t.h)
  }
  for (const t of matched) outline(ctx, t, hoverInk(theme), 2)
}

function drawOverlay(
  canvas: HTMLCanvasElement,
  tiles: Tile[],
  size: Size,
  theme: Theme,
  hover: Tile | null,
  selected: number | null,
  highlight: Highlight | null,
): void {
  const ctx = prepare(canvas, size)
  if (!ctx) return
  ctx.clearRect(0, 0, size.w, size.h)
  if (highlight) drawHighlight(ctx, tiles, theme, highlight)
  if (selected !== null) {
    const t = tiles.find((x) => x.id === selected && x.kind !== 'more')
    if (t) outline(ctx, t, selectionInk(theme), 2)
  }
  if (hover && !(hover.id === selected && hover.kind !== 'more')) outline(ctx, hover, hoverInk(theme), 1.5)
}

type Direction = 'left' | 'right' | 'up' | 'down'

function neighbour(tiles: Tile[], from: Tile, dir: Direction): Tile | null {
  const cx = from.x + from.w / 2
  const cy = from.y + from.h / 2
  let best: Tile | null = null
  let bestScore = Infinity
  for (const t of tiles) {
    if (t === from || t.depth !== from.depth) continue
    const dx = t.x + t.w / 2 - cx
    const dy = t.y + t.h / 2 - cy
    const along = dir === 'left' ? -dx : dir === 'right' ? dx : dir === 'up' ? -dy : dy
    const across = dir === 'left' || dir === 'right' ? Math.abs(dy) : Math.abs(dx)
    if (along <= 1) continue
    const score = along + across * 2
    if (score < bestScore) {
      bestScore = score
      best = t
    }
  }
  return best
}

export function Treemap(props: Props) {
  const { scanId, rootId, rootName, metric, theme, contrast, version, selected, highlight, onSelect, onZoom, onUp, onMenu, onError } =
    props
  const wrapRef = useRef<HTMLDivElement>(null)
  const baseRef = useRef<HTMLCanvasElement>(null)
  const overlayRef = useRef<HTMLCanvasElement>(null)
  const [size, setSize] = useState<Size>({ w: 0, h: 0 })
  const [tiles, setTiles] = useState<Tile[]>([])
  const [hover, setHover] = useState<{ tile: Tile; x: number; y: number } | null>(null)

  useLayoutEffect(() => {
    const el = wrapRef.current
    if (!el) return
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect
      if (r) setSize({ w: Math.floor(r.width), h: Math.floor(r.height) })
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  useEffect(() => {
    if (size.w < 20 || size.h < 20) return
    let live = true
    const timer = window.setTimeout(() => {
      api
        .treemap(scanId, rootId, size.w, size.h, metric)
        .then((t) => live && setTiles(t))
        .catch((e) => live && onError(errorText(e)))
    }, 60)
    return () => {
      live = false
      window.clearTimeout(timer)
    }
  }, [scanId, rootId, metric, size.w, size.h, version, onError])

  useEffect(() => {
    if (baseRef.current) drawBase(baseRef.current, tiles, size, theme)
  }, [tiles, size, theme, contrast])

  useEffect(() => {
    if (overlayRef.current) drawOverlay(overlayRef.current, tiles, size, theme, hover?.tile ?? null, selected, highlight)
  }, [tiles, size, theme, contrast, hover, selected, highlight])

  const total = tiles.reduce((s, t) => (t.depth === 0 ? s + t.size : s), 0)
  const selectedTile = tiles.find((t) => t.id === selected && t.kind !== 'more') ?? null

  const local = (e: React.MouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    return { x: e.clientX - r.left, y: e.clientY - r.top }
  }

  const choose = (t: Tile) => onSelect(t.id) // aggregates select their folder

  const onKeyDown = (e: React.KeyboardEvent) => {
    const arrows: Record<string, Direction> = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' }
    if (e.key === 'Backspace' || (e.altKey && e.key === 'ArrowUp')) {
      e.preventDefault()
      onUp()
      return
    }
    const dir = arrows[e.key]
    if (dir) {
      e.preventDefault()
      const from = selectedTile
      const next = from ? neighbour(tiles, from, dir) : (tiles.find((t) => t.depth === 0) ?? null)
      if (next) choose(next)
      return
    }
    if (e.key === 'Enter' && selectedTile?.kind === 'dir') {
      e.preventDefault()
      onZoom(selectedTile.id)
      return
    }
    if ((e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && selectedTile) {
      e.preventDefault()
      const r = baseRef.current?.getBoundingClientRect()
      if (r) onMenu(selectedTile, r.left + selectedTile.x + selectedTile.w / 2, r.top + selectedTile.y + selectedTile.h / 2)
    }
  }

  const announcement = selectedTile
    ? `${tileName(selectedTile)}, ${formatBytes(selectedTile.size)}, ${percent(selectedTile.size, total)} of ${rootName}`
    : ''

  return (
    <div className="map-wrap" ref={wrapRef}>
      <canvas
        ref={baseRef}
        className="base"
        tabIndex={0}
        role="application"
        aria-roledescription="storage map"
        aria-label={`Storage map of ${rootName}. Use arrow keys to move between items, Enter to open a folder, Backspace to go up.`}
        onKeyDown={onKeyDown}
        onMouseMove={(e) => {
          const p = local(e)
          const t = hitTest(tiles, p.x, p.y)
          setHover(t ? { tile: t, x: p.x, y: p.y } : null)
        }}
        onMouseLeave={() => setHover(null)}
        onClick={(e) => {
          const p = local(e)
          const t = hitTest(tiles, p.x, p.y)
          if (t) choose(t)
        }}
        onDoubleClick={(e) => {
          const p = local(e)
          const f = folderAt(tiles, p.x, p.y)
          if (f) onZoom(f.id)
        }}
        onContextMenu={(e) => {
          e.preventDefault()
          const p = local(e)
          const t = hitTest(tiles, p.x, p.y)
          if (t) {
            choose(t)
            onMenu(t, e.clientX, e.clientY)
          }
        }}
      />
      <canvas ref={overlayRef} className="overlay" aria-hidden="true" />
      {hover && (
        <div
          className="tooltip"
          ref={(el) => {
            if (!el || !wrapRef.current) return
            const maxX = wrapRef.current.clientWidth - el.offsetWidth - 4
            const maxY = wrapRef.current.clientHeight - el.offsetHeight - 4
            el.style.left = `${Math.max(4, Math.min(hover.x + 14, maxX))}px`
            el.style.top = `${Math.max(4, Math.min(hover.y + 16, maxY))}px`
          }}
        >
          <strong>{tileName(hover.tile)}</strong>
          <span className="num">
            {formatBytes(hover.tile.size)}, {percent(hover.tile.size, total)} of this view
          </span>
          <div>{kindLabel(hover.tile)}</div>
        </div>
      )}
      <div className="sr-only" aria-live="polite">
        {announcement}
      </div>
      {tiles.length === 0 && size.w > 0 && (
        <div className="empty">
          <p>Nothing to show here yet.</p>
        </div>
      )}
    </div>
  )
}
