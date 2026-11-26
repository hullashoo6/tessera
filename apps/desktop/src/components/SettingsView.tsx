import { api, type Entitlement, type HardLinks, type Metric } from '../api'
import { ProLicence } from './ProLicence'
import type { Settings, ThemeSetting } from '../settings'

interface Props {
  settings: Settings
  os: string
  version: string
  onChange: (s: Settings) => void
  onShowWelcome: () => void
  entitlement: Entitlement | null
  onEntitlement: (e: Entitlement) => void
}

function Choice<T extends string>(props: { label: string; value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="segmented" role="group" aria-label={props.label}>
      {props.options.map(([v, text]) => (
        <button key={v} aria-pressed={props.value === v} onClick={() => props.onChange(v)}>
          {text}
        </button>
      ))}
    </div>
  )
}

export function SettingsView({ settings, os, version, onChange, onShowWelcome, entitlement, onEntitlement }: Props) {
  return (
    <div className="settings">
      <ProLicence entitlement={entitlement} onChange={onEntitlement} />
      <div className="setting">
        <h3>Appearance</h3>
        <p>Follow your system setting, or always use light or dark.</p>
        <Choice<ThemeSetting>
          label="Appearance"
          value={settings.theme}
          options={[
            ['system', 'System'],
            ['light', 'Light'],
            ['dark', 'Dark'],
          ]}
          onChange={(theme) => onChange({ ...settings, theme })}
        />
      </div>
      <div className="setting">
        <h3>Size to show</h3>
        <p>
          Size on disk is the space files actually take up, which is what you get back when you remove them. File
          size is the amount of data they contain.
        </p>
        <Choice<Metric>
          label="Size to show"
          value={settings.metric}
          options={[
            ['allocated', 'Size on disk'],
            ['logical', 'File size'],
          ]}
          onChange={(metric) => onChange({ ...settings, metric })}
        />
      </div>
      <div className="setting">
        <h3>Hard links</h3>
        <p>
          Some files have several names (hard links), so their space should be counted once.
          {os === 'windows'
            ? ' Standard checks files of 64 KB and larger, which catches nearly all shared space without slowing scans. Thorough checks every file and is slower.'
            : ' On this system every file is checked at no extra cost.'}{' '}
          Applies to the next scan.
        </p>
        <Choice<HardLinks>
          label="Hard link checking"
          value={settings.hardLinks}
          options={[
            ['auto', 'Standard'],
            ['thorough', 'Thorough'],
            ['off', 'Off'],
          ]}
          onChange={(hardLinks) => onChange({ ...settings, hardLinks })}
        />
      </div>
      <div className="setting">
        <h3>Updates</h3>
        <p>
          When Tessera opens, it can check this app’s website for a newer version. Nothing about you or your files is
          sent, and nothing is downloaded unless you choose to.
        </p>
        <Choice<'on' | 'off'>
          label="Check for updates"
          value={settings.checkUpdates ? 'on' : 'off'}
          options={[
            ['on', 'Check'],
            ['off', 'Don’t check'],
          ]}
          onChange={(v) => onChange({ ...settings, checkUpdates: v === 'on' })}
        />
      </div>
      <div className="setting">
        <h3>Privacy</h3>
        <p>
          Scanning happens entirely on this computer. File names, folders and scan results are never sent anywhere.
          Tessera connects to the internet only to check a Pro licence and, if allowed above, to check for updates.
        </p>
      </div>
      <div className="setting">
        <h3>Diagnostics</h3>
        <p>
          Tessera keeps a small log of crashes and errors on this computer, without any file or folder names. It is
          never sent anywhere. If you contact support about a problem, you can attach it to your email.
        </p>
        <button className="btn" onClick={() => void api.showLogFile().catch(() => undefined)}>
          Show log file
        </button>
      </div>
      <div className="setting">
        <h3>Welcome screen</h3>
        <p>See the short introduction from the first time you opened Tessera.</p>
        <button className="btn" onClick={onShowWelcome}>
          Show welcome screen
        </button>
      </div>
      <p className="target-meta">Tessera {version}</p>
    </div>
  )
}
