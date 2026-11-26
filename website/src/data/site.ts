// Single source of truth for claims made on the website. Keep in sync with
// STATUS.md: only features marked "available" may be presented as working.

export const product = {
  name: 'Tessera',
  tagline: 'Every byte, accounted for.',
  summary: 'See what fills your computer. Find the files that matter. Reclaim your space with confidence.',
  description:
    'Tessera is a disk space analyzer for Windows and Mac that maps every folder on your drive to scale, finds your largest files, and keeps every scan on your computer.',
}

/**
 * Set by the owner: USD 9.78 one-time, tax included, one device, no expiry.
 * While `onSale` is false there is no purchase link anywhere on the site.
 */
export const pricing = {
  onSale: false,
  currency: 'USD',
  pro: 9.78,
}

export type Status = 'available' | 'in development'

export interface Feature {
  name: string
  status: Status
}

export const freeFeatures: Feature[] = [
  { name: 'Unlimited scans of any drive or folder', status: 'available' },
  { name: 'Interactive storage map', status: 'available' },
  { name: 'Largest files, ranked', status: 'available' },
  { name: 'Open, reveal and copy the path of any item', status: 'available' },
  { name: 'Search and filters', status: 'available' },
  { name: 'Smart Scan suggestions and duplicate finder', status: 'available' },
]

export const proFeatures: Feature[] = [
  { name: 'Everything in Free', status: 'available' },
  { name: 'Review-first cleanup to the Recycle Bin or Trash', status: 'available' },
  { name: 'Clean up Smart Scan suggestions and duplicates in a few clicks', status: 'available' },
  { name: 'Uninstall apps, including leftover files on Mac and Store apps on Windows', status: 'available' },
  { name: 'Put back anything Tessera moved, from inside the app', status: 'available' },
  { name: 'Free up space from OneDrive and iCloud files, keeping them in the cloud', status: 'available' },
  { name: 'Storage history and growth between scans', status: 'in development' },
  { name: 'Exportable reports (CSV)', status: 'available' },
]

export type PlatformId = 'windows-x64' | 'mac-arm64' | 'mac-x64'

export interface Platform {
  id: PlatformId
  label: string
  requirement: string
  /** Shown until a verified release exists in releases.json. */
  state: string
}

export const platforms: Platform[] = [
  { id: 'windows-x64', label: 'Windows', requirement: 'Windows 11, 64-bit', state: 'In testing' },
  { id: 'mac-arm64', label: 'Mac with Apple silicon', requirement: 'macOS version to be confirmed', state: 'In testing' },
  { id: 'mac-x64', label: 'Mac with Intel', requirement: 'Not yet verified', state: 'Planned' },
]

export function formatPrice(): string {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: pricing.currency }).format(pricing.pro)
}

export const contact = {
  email: 'hullashoo6@gmail.com',
  company: 'Beyond Enterprises',
}

/** Newest released app version, read by the app's update check. */
export const latest = { version: '0.1.0' }
