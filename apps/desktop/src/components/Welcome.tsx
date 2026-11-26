import type { ScanTarget, Volume } from '../api'
import { driveName, formatBytes } from '../format'

interface Props {
  os: string
  volumes: Volume[] | null
  /** Starts a scan; the welcome screen is then not shown again. */
  onScan: (target: ScanTarget) => void
  /** Goes to the full list of drives and folders. */
  onSkip: () => void
}

/** First-run screen: what Tessera does, the privacy promise, one-click first scan. */
export function Welcome({ os, volumes, onScan, onSkip }: Props) {
  const bin = os === 'macos' ? 'Trash' : 'Recycle Bin'
  const main = volumes?.find((v) => v.system) ?? null
  return (
    <div className="overview welcome">
      <h2>Welcome to Tessera</h2>
      <p className="lede">Tessera shows what is filling your storage, so you can decide what to keep.</p>

      <ul className="welcome-points">
        <li>
          <div className="target-name">A map of your storage</div>
          <div className="target-meta">
            Every folder is drawn to scale, so the biggest space users stand out at a glance. Smart Scan points out
            caches and leftovers that are usually safe to remove, and says why.
          </div>
        </li>
        <li>
          <div className="target-name">Private by design</div>
          <div className="target-meta">
            Scans happen on this computer. File names, folders and scan results are never sent anywhere.
          </div>
        </li>
        <li>
          <div className="target-name">Nothing removed without you</div>
          <div className="target-meta">
            Tessera only looks until you choose something to clean up. Cleanup moves items to the {bin}, and the
            Restore page can put them back.
          </div>
        </li>
      </ul>

      <div className="section-title">Start with your first scan</div>
      <div className="target-list">
        {main && (
          <div className="target">
            <div>
              <div className="target-name">{driveName(main)}</div>
              <div className="target-meta">
                Your main drive
                {main.space ? `: ${formatBytes(main.space.used)} used of ${formatBytes(main.space.total)}` : ''}. You
                can stop the scan at any time.
              </div>
            </div>
            <button className="btn btn-primary" onClick={() => onScan({ kind: 'volume', mount: main.mount })}>
              Scan main drive
            </button>
          </div>
        )}
        <div className="target">
          <div>
            <div className="target-name">Your home folder</div>
            <div className="target-meta">Documents, downloads, pictures and app data for your account</div>
          </div>
          <button className={main ? 'btn' : 'btn btn-primary'} onClick={() => onScan({ kind: 'home' })}>
            Scan home folder
          </button>
        </div>
      </div>
      <div className="welcome-actions">
        <button className="btn btn-quiet" onClick={onSkip}>
          Choose another drive or folder
        </button>
      </div>
    </div>
  )
}
