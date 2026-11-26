import { useEffect, useRef } from 'react'

export interface MenuAction {
  label: string
  run: () => void
  disabled?: boolean
  separatorBefore?: boolean
}

interface Props {
  x: number
  y: number
  actions: MenuAction[]
  onClose: () => void
}

export function ContextMenu({ x, y, actions, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    // Keep the menu on screen.
    const left = Math.min(x, window.innerWidth - el.offsetWidth - 8)
    const top = Math.min(y, window.innerHeight - el.offsetHeight - 8)
    el.style.left = `${Math.max(8, left)}px`
    el.style.top = `${Math.max(8, top)}px`
    el.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()

    const close = (e: Event) => {
      if (!el.contains(e.target as Node)) onClose()
    }
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('mousedown', close)
    window.addEventListener('keydown', key)
    window.addEventListener('blur', onClose)
    return () => {
      window.removeEventListener('mousedown', close)
      window.removeEventListener('keydown', key)
      window.removeEventListener('blur', onClose)
    }
  }, [x, y, onClose])

  const move = (e: React.KeyboardEvent, step: number) => {
    const buttons = Array.from(ref.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement)
    const next = buttons[(i + step + buttons.length) % buttons.length]
    e.preventDefault()
    next?.focus()
  }

  return (
    <div
      ref={ref}
      className="menu"
      role="menu"
      onKeyDown={(e) => {
        if (e.key === 'ArrowDown') move(e, 1)
        if (e.key === 'ArrowUp') move(e, -1)
      }}
    >
      {actions.map((a) => (
        <div key={a.label}>
          {a.separatorBefore && <hr />}
          <button
            role="menuitem"
            disabled={a.disabled}
            onClick={() => {
              onClose()
              a.run()
            }}
          >
            {a.label}
          </button>
        </div>
      ))}
    </div>
  )
}
