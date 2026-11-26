import { useEffect, useState } from 'react'
import { api, errorText, type BackgroundSettings } from '../api'

interface Props {
  os: string
}

/** Settings > Background: keep running after the window closes; low-space warning. */
export function BackgroundSetting({ os }: Props) {
  const [s, setS] = useState<BackgroundSettings | null>(null)
  const [error, setError] = useState<string | null>(null)
  const place = os === 'macos' ? 'the menu bar' : 'the notification area of the taskbar'

  useEffect(() => {
    api
      .backgroundSettings()
      .then(setS)
      .catch((e) => setError(errorText(e)))
  }, [])

  const change = (keepRunning: boolean, lowSpaceAlerts: boolean) => {
    setError(null)
    api
      .setBackgroundSettings(keepRunning, lowSpaceAlerts)
      .then(setS)
      .catch((e) => setError(errorText(e)))
  }

  return (
    <div className="setting">
      <h3>Background</h3>
      <p>
        Tessera can stay in {place} after you close its window, showing how much space is free on your main drive, and
        warn you when that drive is almost full. Nothing is scanned in the background.
      </p>
      {s && (
        <div className="checks">
          <label>
            <input type="checkbox" checked={s.keepRunning} onChange={(e) => change(e.target.checked, s.lowSpaceAlerts)} />
            Keep Tessera in {place} when its window is closed
          </label>
          <label>
            <input type="checkbox" checked={s.lowSpaceAlerts} onChange={(e) => change(s.keepRunning, e.target.checked)} />
            Warn me when my main drive is almost full (while Tessera is running)
          </label>
        </div>
      )}
      {error && <p className="target-meta">{error}</p>}
    </div>
  )
}
