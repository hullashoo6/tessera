import { useState } from 'react'
import { api, errorText, type Entitlement } from '../api'
import { formatDate } from '../format'

interface Props {
  entitlement: Entitlement | null
  onChange: (e: Entitlement) => void
}

/** Settings section for entering, checking and moving a Pro licence. */
export function ProLicence({ entitlement, onChange }: Props) {
  const [key, setKey] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | null>(null)

  const run = (p: Promise<Entitlement>, ok: string) => {
    setBusy(true)
    setMessage(null)
    p.then((e) => {
      onChange(e)
      setKey('')
      setMessage(ok)
    })
      .catch((e) => setMessage(errorText(e)))
      .finally(() => setBusy(false))
  }

  const pro = entitlement?.pro ?? false
  return (
    <div className="setting">
      <h3>Tessera Pro</h3>
      {entitlement?.testMode && (
        <p className="review-reason">Test version: only test licence keys work, and nothing can be bought yet.</p>
      )}
      {pro ? (
        <>
          <p>
            Pro is active on this computer{entitlement?.maskedKey ? `, using key ${entitlement.maskedKey}` : ''}
            {entitlement?.customerEmail ? ` (${entitlement.customerEmail})` : ''}. Your licence covers one computer; to
            move it, deactivate it here first.
          </p>
          <p className="target-meta">
            Last confirmed with Dodo Payments: {formatDate(entitlement?.lastValidated ?? null)}
          </p>
          <div className="actions">
            <button className="btn" disabled={busy} onClick={() => run(api.refreshLicense(true), 'Checked with Dodo Payments just now.')}>
              Check now
            </button>
            <button className="btn" disabled={busy} onClick={() => run(api.deactivateLicense(), 'This computer is no longer activated.')}>
              {busy ? 'Working…' : 'Deactivate on this computer'}
            </button>
          </div>
        </>
      ) : (
        <>
          <p>
            {entitlement && entitlement.summary !== 'Free'
              ? entitlement.summary
              : 'Pro adds cleanup to the Recycle Bin or Trash. Enter the licence key from your purchase email.'}
          </p>
          <form
            className="licence-form"
            onSubmit={(e) => {
              e.preventDefault()
              if (key.trim()) run(api.activateLicense(key), 'Pro is now active on this computer.')
            }}
          >
            <input
              className="search-input"
              aria-label="Licence key"
              placeholder="Licence key"
              value={key}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => setKey(e.target.value)}
            />
            <button className="btn btn-primary" type="submit" disabled={busy || !key.trim()}>
              {busy ? 'Checking…' : 'Activate'}
            </button>
          </form>
        </>
      )}
      {message && (
        <p className="licence-message" role="status">
          {message}
        </p>
      )}
      <p className="target-meta">
        Activating sends only your licence key and a device label to the payment provider, Dodo Payments. Nothing about
        your files is sent.
      </p>
    </div>
  )
}
