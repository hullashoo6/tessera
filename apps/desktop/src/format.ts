// Size units follow the operating system's own convention so numbers match
// what people see in Explorer (1 KB = 1024 bytes) or Finder (1 KB = 1000).
import type { Volume } from './api'

let base = 1024

/** "Windows (C:)" or "Macintosh HD (/)"; the mount alone when unlabelled. */
export function driveName(v: Volume): string {
  const mount = v.mount.replace(/[\\/]+$/, '') || v.mount
  return v.label ? `${v.label} (${mount})` : mount
}

export function setPlatform(os: string): void {
  base = os === 'macos' ? 1000 : 1024
}

export function platformNames(os: string): { fileManager: string; showIn: string } {
  return os === 'macos'
    ? { fileManager: 'Finder', showIn: 'Show in Finder' }
    : { fileManager: 'File Explorer', showIn: 'Show in File Explorer' }
}

/** Last path component, for showing a scanned root by its folder name. */
export function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, '')
  const cut = Math.max(trimmed.lastIndexOf('\\'), trimmed.lastIndexOf('/'))
  return cut >= 0 && cut < trimmed.length - 1 ? trimmed.slice(cut + 1) : path
}

/** Bytes in `n` units, using the platform's convention (1024 or 1000). */
export function unitBytes(n: number, unit: 'MB' | 'GB'): number {
  return n * base ** (unit === 'MB' ? 2 : 3)
}

const UNITS = ['bytes', 'KB', 'MB', 'GB', 'TB', 'PB']

export function formatBytes(n: number): string {
  if (n < base) return n === 1 ? '1 byte' : `${n} bytes`
  let v = n
  let u = 0
  while (v >= base && u < UNITS.length - 1) {
    v /= base
    u++
  }
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : 2
  return `${v.toFixed(digits)} ${UNITS[u]}`
}

const counter = new Intl.NumberFormat()

export function formatCount(n: number): string {
  return counter.format(n)
}

export function plural(n: number, one: string, many: string): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms} ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`
  const m = Math.floor(ms / 60_000)
  const s = Math.round((ms % 60_000) / 1000)
  return `${m} min ${s} s`
}

/** "about 3 minutes left", from the speed so far; null until it is reliable. */
export function timeLeft(done: number, total: number, elapsedMs: number): string | null {
  if (elapsedMs < 3000 || done <= 0 || total <= done || done / total < 0.01) return null
  const ms = ((total - done) / done) * elapsedMs
  if (ms < 60_000) return 'less than a minute left'
  const min = Math.round(ms / 60_000)
  if (min < 60) return `about ${plural(min, 'minute', 'minutes')} left`
  const h = Math.floor(min / 60)
  const m = min % 60
  return `about ${plural(h, 'hour', 'hours')}${m ? ` ${plural(m, 'minute', 'minutes')}` : ''} left`
}

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' })

export function formatDate(unixSeconds: number | null): string {
  if (unixSeconds === null) return 'Unknown'
  return dateFormat.format(new Date(unixSeconds * 1000))
}

export function percent(part: number, whole: number): string {
  if (whole <= 0) return '0%'
  const p = (part / whole) * 100
  if (p > 0 && p < 0.1) return '<0.1%'
  return `${p >= 10 ? p.toFixed(0) : p.toFixed(1)}%`
}
