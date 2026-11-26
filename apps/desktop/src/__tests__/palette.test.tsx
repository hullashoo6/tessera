// Checks the storage map's label contrast: at least 4.5:1 normally (WCAG AA)
// and at least 7:1 in high contrast (WCAG AAA), for every branch colour.
import { afterEach, describe, expect, it } from 'vitest'
import { frameFill, labelInk, moreFill, setHighContrast, tileFill, type Theme } from '../palette'

function rgb(css: string): [number, number, number, number] {
  const hsl = /^hsl\((\d+) (\d+)% (\d+)%\)$/.exec(css)
  if (hsl) {
    const [h, s, l] = [Number(hsl[1]), Number(hsl[2]) / 100, Number(hsl[3]) / 100]
    const k = (n: number) => (n + h / 30) % 12
    const a = s * Math.min(l, 1 - l)
    const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1))
    return [f(0) * 255, f(8) * 255, f(4) * 255, 1]
  }
  const rgba = /^rgba\((\d+), (\d+), (\d+), ([\d.]+)\)$/.exec(css)
  if (rgba) return [Number(rgba[1]), Number(rgba[2]), Number(rgba[3]), Number(rgba[4])]
  const hex = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(css)
  const byte = (s: string | undefined) => parseInt(s ?? '0', 16)
  if (hex) return [byte(hex[1]), byte(hex[2]), byte(hex[3]), 1]
  throw new Error(`unparsed colour ${css}`)
}

function luminance(r: number, g: number, b: number): number {
  const c = (v: number) => {
    const s = v / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b)
}

/** Contrast of `ink` (possibly translucent) drawn over `bg`. */
function contrast(ink: string, bg: string): number {
  const [ir, ig, ib, a] = rgb(ink)
  const [br, bgr, bb] = rgb(bg)
  const mixed = luminance(ir * a + br * (1 - a), ig * a + bgr * (1 - a), ib * a + bb * (1 - a))
  const under = luminance(br, bgr, bb)
  return (Math.max(mixed, under) + 0.05) / (Math.min(mixed, under) + 0.05)
}

function worst(theme: Theme): number {
  let min = contrast(labelInk(theme), moreFill(theme))
  for (let branch = 0; branch < 10; branch++) {
    for (let depth = 0; depth < 6; depth++) {
      for (const kind of ['file', 'dir']) {
        const t = { kind, depth, branch }
        min = Math.min(min, contrast(labelInk(theme), tileFill(t, theme)), contrast(labelInk(theme), frameFill(t, theme)))
      }
    }
  }
  return min
}

afterEach(() => setHighContrast(false))

describe('Storage map label contrast', () => {
  it('is at least 4.5:1 normally', () => {
    expect(worst('light')).toBeGreaterThanOrEqual(4.5)
    expect(worst('dark')).toBeGreaterThanOrEqual(4.5)
  })

  it('is at least 7:1 in high contrast', () => {
    setHighContrast(true)
    expect(worst('light')).toBeGreaterThanOrEqual(7)
    expect(worst('dark')).toBeGreaterThanOrEqual(7)
  })
})
