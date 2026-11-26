/** Tessera mark: a small treemap of five tiles. */
export function Mark({ size = 22 }: { size?: number }) {
  return (
    <svg className="mark" width={size} height={size} viewBox="0 0 22 22" aria-hidden="true">
      <rect className="mark-1" x="1" y="1" width="11" height="20" rx="1.5" />
      <rect className="mark-2" x="13" y="1" width="8" height="9" rx="1.5" />
      <rect className="mark-3" x="13" y="11" width="8" height="5" rx="1.5" />
      <rect className="mark-4" x="13" y="17" width="3.5" height="4" rx="1" />
      <rect className="mark-5" x="17.5" y="17" width="3.5" height="4" rx="1" />
    </svg>
  )
}
