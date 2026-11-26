import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'

interface Props<T> {
  items: T[]
  /** Total rows when more can be loaded than are present. */
  total?: number
  rowHeight: number
  label: string
  selectedIndex: number
  onSelectIndex: (index: number) => void
  onActivate?: (index: number) => void
  onEndReached?: () => void
  renderRow: (item: T, index: number) => ReactNode
  className?: string
  rowClassName: string
}

/** Fixed-height virtual list: only visible rows are in the DOM. */
export function VirtualList<T>(props: Props<T>) {
  const { items, rowHeight, label, selectedIndex, onSelectIndex, onActivate, onEndReached, renderRow } = props
  const ref = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [height, setHeight] = useState(0)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver(() => setHeight(el.clientHeight))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // Keep the keyboard selection in view.
  useEffect(() => {
    const el = ref.current
    if (!el || selectedIndex < 0) return
    const top = selectedIndex * rowHeight
    if (top < el.scrollTop) el.scrollTop = top
    else if (top + rowHeight > el.scrollTop + el.clientHeight) el.scrollTop = top + rowHeight - el.clientHeight
  }, [selectedIndex, rowHeight])

  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - 6)
  const last = Math.min(items.length, Math.ceil((scrollTop + height) / rowHeight) + 6)

  useEffect(() => {
    if (onEndReached && last >= items.length - 20 && items.length < (props.total ?? items.length)) onEndReached()
  }, [last, items.length, props.total, onEndReached])

  const indices = Array.from({ length: Math.max(0, last - first) }, (_, k) => first + k)
  const rows: ReactNode[] = indices.map((i) => {
    const item = items[i]
    if (item === undefined) return null
    return (
      <div
        key={i}
        className={props.rowClassName}
        role="option"
        aria-selected={i === selectedIndex}
        aria-posinset={i + 1}
        aria-setsize={props.total ?? items.length}
        ref={(el) => {
          if (el) {
            el.style.top = `${i * rowHeight}px`
            el.style.height = `${rowHeight}px`
          }
        }}
        onClick={() => onSelectIndex(i)}
        onDoubleClick={() => onActivate?.(i)}
      >
        {renderRow(item, i)}
      </div>
    )
  })

  return (
    <div
      ref={ref}
      className={`vlist ${props.className ?? ''}`}
      role="listbox"
      aria-label={label}
      tabIndex={0}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      onKeyDown={(e) => {
        if (e.key === 'ArrowDown') {
          e.preventDefault()
          onSelectIndex(Math.min(items.length - 1, selectedIndex + 1))
        } else if (e.key === 'ArrowUp') {
          e.preventDefault()
          onSelectIndex(Math.max(0, selectedIndex - 1))
        } else if (e.key === 'Enter' && selectedIndex >= 0) {
          e.preventDefault()
          onActivate?.(selectedIndex)
        }
      }}
    >
      <div
        ref={(el) => {
          if (el) el.style.height = `${items.length * rowHeight}px`
        }}
      >
        {rows}
      </div>
    </div>
  )
}
