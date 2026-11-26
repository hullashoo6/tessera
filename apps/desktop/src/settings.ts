import type { HardLinks, Metric } from './api'

export type ThemeSetting = 'system' | 'light' | 'dark'

export interface Settings {
  theme: ThemeSetting
  metric: Metric
  hardLinks: HardLinks
}

export const DEFAULTS: Settings = { theme: 'system', metric: 'allocated', hardLinks: 'auto' }

const KEY = 'tessera.settings.v1'

// Preferences only; no file names or paths are ever stored.
export function loadSettings(): Settings {
  try {
    const raw = window.localStorage.getItem(KEY)
    if (!raw) return DEFAULTS
    const v = JSON.parse(raw) as Partial<Settings>
    return {
      theme: v.theme === 'light' || v.theme === 'dark' ? v.theme : 'system',
      metric: v.metric === 'logical' ? 'logical' : 'allocated',
      hardLinks: v.hardLinks === 'thorough' || v.hardLinks === 'off' ? v.hardLinks : 'auto',
    }
  } catch {
    return DEFAULTS
  }
}

export function saveSettings(s: Settings): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(s))
  } catch {
    // Storage unavailable: settings last for this session only.
  }
}
