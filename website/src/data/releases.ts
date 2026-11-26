import manifest from './releases.json'
import type { PlatformId } from './site'

export interface Release {
  platform: PlatformId
  version: string
  installer: string
  file: string
  url: string
  bytes: number
  sha256: string
  minimumOs: string
  notesUrl: string
}

// Only well-formed entries are shown: an https URL, a size and a SHA-256.
export const releases: Release[] = (manifest.releases as Release[]).filter(
  (r) => /^[0-9a-f]{64}$/.test(r.sha256) && r.url.startsWith('https://') && r.bytes > 0,
)

export function releaseFor(id: PlatformId): Release | undefined {
  return releases.find((r) => r.platform === id)
}
