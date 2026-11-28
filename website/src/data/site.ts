// Single source of truth for claims made on the website. Keep in sync with
// STATUS.md: only features marked "available" may be presented as working.

export const product = {
  name: 'Tessera',
  tagline: 'Every byte, accounted for.',
  summary: 'See what fills your computer. Find the files that matter. Reclaim your space with confidence.',
  description:
    'Tessera is a disk space analyser for Windows and Mac that maps every folder on your drive to scale, finds your largest files, and keeps every scan on your computer.',
}

/**
 * Set by the owner: USD 9.78 one-time, tax included, one device, no expiry.
 * While `onSale` is false there is no purchase link anywhere on the site.
 * On sale since 27 Nov 2026 (owner: "make sales live"); the link is Dodo's
 * live static payment link for the live Pro product.
 */
export const pricing = {
  onSale: true,
  currency: 'USD',
  pro: 9.78,
  checkoutUrl: 'https://checkout.dodopayments.com/buy/pdt_0Nsmg4vQiRrOCZ4yTeDEk?quantity=1&minimalAddress=true',
}

export type Status = 'available' | 'in development'

export interface Feature {
  name: string
  status: Status
}

export const freeFeatures: Feature[] = [
  { name: 'Unlimited scans of any drive or folder', status: 'available' },
  { name: 'The storage map, largest files and search', status: 'available' },
  { name: 'Smart Scan suggestions, each with its reason', status: 'available' },
  { name: 'An exact duplicate finder', status: 'available' },
]

export const proFeatures: Feature[] = [
  { name: 'Everything in Free', status: 'available' },
  { name: 'One-step cleanup to the Recycle Bin or Trash, with Undo', status: 'available' },
  { name: 'Plain answers when something can’t be removed', status: 'available' },
  { name: 'Uninstall apps, with their leftover files', status: 'available' },
  { name: 'Free up space from OneDrive and iCloud, keeping files in the cloud', status: 'available' },
  { name: 'Storage history and printable reports', status: 'available' },
]

export type PlatformId = 'windows-x64' | 'mac-universal'

export interface Platform {
  id: PlatformId
  label: string
  requirement: string
  /** Shown until a verified release exists in releases.json. */
  state: string
}

export const platforms: Platform[] = [
  { id: 'windows-x64', label: 'Windows', requirement: 'Windows 11, 64-bit', state: 'Available soon' },
  { id: 'mac-universal', label: 'Mac', requirement: 'macOS 12 or later, Apple silicon and Intel', state: 'Available soon' },
]

export function formatPrice(): string {
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: pricing.currency }).format(pricing.pro)
}

export const contact = {
  email: 'hullashoo6@gmail.com',
  company: 'Beyond Enterprises',
}

/** Newest released app version, read by the app's update check. */
export const latest = { version: '1.0.2' }
