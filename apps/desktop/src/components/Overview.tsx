import type { ScanTarget, Status, Volume } from '../api'
import { driveName, formatBytes, formatDuration, plural } from '../format'

interface Props {
  volumes: Volume[] | null
  status: Status | null
  onScan: (target: ScanTarget) => void
  onOpenMap: () => void
  onRefreshDrives: () => void
}

function driveKind(v: Volume): string {
  switch (v.kind) {
    case 'removable':
      return 'Removable drive'
    case 'network':
      return 'Network drive (may scan slowly)'
    case 'optical':
      return 'Disc'
    default:
      return 'Local drive'
  }
}

export function Overview({ volumes, status, onScan, onOpenMap, onRefreshDrives }: Props) {
  const scanning = status?.phase === 'scanning'
  return (
    <div className="overview">
      <h2>See what fills your storage</h2>
      <p className="lede">
        Choose a drive or folder. Scanning only reads file sizes, on this computer, and changes nothing. Files are
        removed only when you choose them in Cleanup.
      </p>

      {status && (
        <div className="current-scan">
          <div className="spacer">
            <div className="target-name">{status.root}</div>
            <div className="target-meta num">
              {status.phase === 'scanning' && `Scanning: ${plural(status.progress.files, 'file', 'files')} so far`}
              {status.phase === 'complete' &&
                `Scanned in ${formatDuration(status.summary?.elapsedMs ?? 0)}: ${plural(status.progress.files, 'file', 'files')}, ${formatBytes(status.progress.allocated)}`}
              {status.phase === 'cancelled' && 'Scan stopped before it finished. Results are partial.'}
              {status.phase === 'failed' && (status.error ?? 'The scan failed.')}
            </div>
          </div>
          {status.phase !== 'failed' && (
            <button className="btn btn-primary" onClick={onOpenMap}>
              View storage map
            </button>
          )}
        </div>
      )}

      <div className="section-title">Drives</div>
      <div className="target-list">
        {volumes === null && <div className="target target-meta">Looking for drives…</div>}
        {volumes?.length === 0 && (
          <div className="target">
            <span>No drives were found.</span>
            <button className="btn" onClick={onRefreshDrives}>
              Look again
            </button>
          </div>
        )}
        {volumes?.map((v) => {
          const s = v.space
          const usedShare = s && s.total > 0 ? s.used / s.total : 0
          return (
            <div className="target" key={v.mount}>
              <div>
                <div className="target-name">{driveName(v)}</div>
                <div className="target-meta">
                  {driveKind(v)}
                  {v.fileSystem ? `, ${v.fileSystem}` : ''}
                </div>
                {s ? (
                  <div className="capacity">
                    <div
                      className="capacity-bar"
                      role="meter"
                      aria-label={`${driveName(v)} used space`}
                      aria-valuemin={0}
                      aria-valuemax={s.total}
                      aria-valuenow={s.used}
                    >
                      <div
                        className="capacity-used"
                        ref={(el) => {
                          if (el) el.style.width = `${(usedShare * 100).toFixed(2)}%`
                        }}
                      />
                    </div>
                    <div className="capacity-text num">
                      <span>
                        {formatBytes(s.used)} used of {formatBytes(s.total)}
                      </span>
                      <span>{formatBytes(s.free)} free</span>
                    </div>
                  </div>
                ) : (
                  <div className="target-meta">Capacity unavailable</div>
                )}
              </div>
              <button className="btn" disabled={scanning} onClick={() => onScan({ kind: 'volume', mount: v.mount })}>
                Scan drive
              </button>
            </div>
          )
        })}
      </div>

      <div className="section-title">Folders</div>
      <div className="target-list">
        <div className="target">
          <div>
            <div className="target-name">Your home folder</div>
            <div className="target-meta">Documents, downloads, pictures and app data for your account</div>
          </div>
          <button className="btn" disabled={scanning} onClick={() => onScan({ kind: 'home' })}>
            Scan home folder
          </button>
        </div>
        <div className="target">
          <div>
            <div className="target-name">A specific folder</div>
            <div className="target-meta">Pick any folder you can open</div>
          </div>
          <button className="btn" disabled={scanning} onClick={() => onScan({ kind: 'folder' })}>
            Choose folder…
          </button>
        </div>
      </div>
    </div>
  )
}
