// Squarified treemap (Bruls, Huizing & van Wijk, 2000), the same algorithm
// the app uses, here only for drawing the decorative hero mosaic at build time.

export interface Rect {
  x: number
  y: number
  w: number
  h: number
}

export function squarify(weights: number[], bounds: Rect): Rect[] {
  const total = weights.reduce((a, b) => a + b, 0)
  const out: Rect[] = weights.map(() => ({ x: 0, y: 0, w: 0, h: 0 }))
  if (!weights.length || total <= 0) return out
  const scale = (bounds.w * bounds.h) / total
  const areas = weights.map((w) => w * scale)
  const free = { ...bounds }
  const worst = (max: number, min: number, sum: number, side: number) =>
    Math.max((side * side * max) / (sum * sum), (sum * sum) / (side * side * min))
  let i = 0
  while (i < areas.length) {
    const side = Math.min(free.w, free.h)
    let j = i + 1
    let sum = areas[i]!
    let current = worst(sum, sum, sum, side)
    while (j < areas.length) {
      const s = sum + areas[j]!
      const w = worst(areas[i]!, areas[j]!, s, side)
      if (w > current) break
      current = w
      sum = s
      j++
    }
    const last = j === areas.length
    if (free.w >= free.h) {
      const t = last ? free.w : sum / free.h
      let y = free.y
      for (let k = i; k < j; k++) {
        const h = k === j - 1 ? free.y + free.h - y : areas[k]! / t
        out[k] = { x: free.x, y, w: t, h }
        y += h
      }
      free.x += t
      free.w -= t
    } else {
      const t = last ? free.h : sum / free.w
      let x = free.x
      for (let k = i; k < j; k++) {
        const w = k === j - 1 ? free.x + free.w - x : areas[k]! / t
        out[k] = { x, y: free.y, w, h: t }
        x += w
      }
      free.y += t
      free.h -= t
    }
    i = j
  }
  return out
}

/** Deterministic pseudo-random numbers, so every build draws the same mosaic. */
export function rng(seed: number): () => number {
  let s = seed >>> 0
  return () => {
    s = (s + 0x6d2b79f5) >>> 0
    let t = s
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}
