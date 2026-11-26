// Treemap colours: one muted hue per top-level branch, lighter with depth.
// Saturation stays low so labels remain legible and the map reads as a calm
// mosaic rather than a rainbow.
//
// High contrast ("Increase contrast" on macOS, contrast themes on Windows):
// tiles move to the far ends of the lightness range so labels in solid black
// or white reach at least 7:1, gaps between tiles become solid lines, and
// the selection and hover outlines use strong, distinct colours.

export type Theme = 'light' | 'dark'

const HUES = [213, 168, 38, 264, 128, 348, 192, 20, 296, 84]

let contrast = false

/** Set from the system's contrast preference before the map is drawn. */
export function setHighContrast(on: boolean): void {
  contrast = on
}

export function highContrast(): boolean {
  return contrast
}

function hue(branch: number): number {
  return HUES[branch % HUES.length] ?? 213
}

interface Paintable {
  kind: string
  depth: number
  branch: number
}

export function tileFill(t: Paintable, theme: Theme): string {
  const h = hue(t.branch)
  const dir = t.kind === 'dir'
  if (contrast) return theme === 'dark' ? `hsl(${h} 40% ${dir ? 14 : 19}%)` : `hsl(${h} 45% ${dir ? 82 : 88}%)`
  if (theme === 'dark') {
    // Capped at 34% so the light labels keep 4.5:1 on every hue.
    const l = Math.min(34, 24 + t.depth * 3) - (dir ? 4 : 0)
    return `hsl(${h} 24% ${l}%)`
  }
  const l = Math.min(88, 74 + t.depth * 4) - (dir ? 6 : 0)
  return `hsl(${h} 32% ${l}%)`
}

/** Background of an opened folder (its label strip and frame). */
export function frameFill(t: Paintable, theme: Theme): string {
  const h = hue(t.branch)
  if (contrast) return theme === 'dark' ? `hsl(${h} 35% 24%)` : `hsl(${h} 40% 72%)`
  if (theme === 'dark') return `hsl(${h} 20% ${Math.min(28, 18 + t.depth * 3)}%)`
  return `hsl(${h} 24% ${Math.min(78, 63 + t.depth * 5)}%)`
}

export function moreFill(theme: Theme): string {
  if (contrast) return theme === 'dark' ? 'hsl(215 8% 12%)' : 'hsl(215 10% 94%)'
  return theme === 'dark' ? 'hsl(215 8% 25%)' : 'hsl(215 10% 85%)'
}

/** Stripes on the "smaller items" tile. */
export function hatchInk(theme: Theme): string {
  if (contrast) return theme === 'dark' ? 'rgba(255,255,255,0.35)' : 'rgba(0,0,0,0.35)'
  return theme === 'dark' ? 'rgba(255,255,255,0.10)' : 'rgba(0,0,0,0.09)'
}

export function grout(theme: Theme): string {
  if (contrast) return theme === 'dark' ? '#ffffff' : '#000000'
  return theme === 'dark' ? '#16191d' : '#f6f7f8'
}

export function labelInk(theme: Theme): string {
  if (contrast) return theme === 'dark' ? '#ffffff' : '#000000'
  return theme === 'dark' ? 'rgba(236, 239, 243, 0.94)' : 'rgba(22, 27, 33, 0.92)'
}

export function selectionInk(theme: Theme): string {
  if (contrast) return theme === 'dark' ? '#ffff00' : '#0037da'
  return theme === 'dark' ? '#ffffff' : '#11161c'
}

export function hoverInk(theme: Theme): string {
  if (contrast) return theme === 'dark' ? '#00ffff' : '#8a00a8'
  return theme === 'dark' ? '#86a9d1' : '#3e6a9e'
}

export function swatch(index: number, kind: string, theme: Theme): string {
  return tileFill({ kind, depth: 0, branch: index }, theme)
}
