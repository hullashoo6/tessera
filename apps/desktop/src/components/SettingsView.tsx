import type { HardLinks, Metric } from '../api'
import type { Settings, ThemeSetting } from '../settings'

interface Props {
  settings: Settings
  os: string
  version: string
  onChange: (s: Settings) => void
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

export function SettingsView({ settings, os, version, onChange }: Props) {
  return (
    <div className="settings">
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
        <h3>Privacy</h3>
        <p>
          Scanning happens entirely on this computer. File names, folders and scan results are never sent anywhere.
        </p>
      </div>
      <p className="target-meta">Tessera {version}</p>
    </div>
  )
}
