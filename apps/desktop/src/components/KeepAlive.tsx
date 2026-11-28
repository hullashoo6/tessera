import { useState, type ReactNode } from 'react'

/**
 * Keeps a page in place, hidden, once it has been opened, so work started on
 * it (a cleanup, a duplicate search, an uninstall) carries on while the
 * person looks at other pages, and its results are still there when they
 * come back (owner, 28 Nov 2026: "this background feature should be for all
 * options"). The page itself stays out of sight and out of the keyboard and
 * screen-reader order while hidden.
 */
export function KeepAlive({ active, children }: { active: boolean; children: ReactNode }) {
  const [opened, setOpened] = useState(active)
  // Adjusting state while rendering (React's documented pattern): the page
  // mounts the first time it is shown and is never unmounted afterwards.
  if (active && !opened) setOpened(true)
  if (!opened && !active) return null
  return (
    <div className="keep-alive" hidden={!active}>
      {children}
    </div>
  )
}
