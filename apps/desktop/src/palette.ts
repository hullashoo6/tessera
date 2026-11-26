// Treemap colours: one muted hue per top-level branch, lighter with depth.
// Saturation stays low so labels remain legible and the map reads as a calm
// mosaic rather than a rainbow.

export type Theme = 'light' | 'dark'

const HUES = [213, 168, 38, 264, 128, 348, 192, 20, 296, 84]

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
  if (theme === 'dark') {
    const l = Math.min(50, 34 + t.depth * 5) - (dir ? 4 : 0)
    return `hsl(${h} 22% ${l}%)`
  }
  const l = Math.min(88, 74 + t.depth * 4) - (dir ? 6 : 0)
  return `hsl(${h} 32% ${l}%)`
}

/** Background of an opened folder (its label strip and frame). */
export function frameFill(t: Paintable, theme: Theme): string {
  const h = hue(t.branch)
  if (theme === 'dark') return `hsl(${h} 18% ${Math.min(40, 24 + t.depth * 5)}%)`
  return `hsl(${h} 24% ${Math.min(76, 58 + t.depth * 5)}%)`
}

export function moreFill(theme: Theme): string {
  return theme === 'dark' ? 'hsl(215 8% 25%)' : 'hsl(215 10% 85%)'
}

export function grout(theme: Theme): string {
  return theme === 'dark' ? '#16191d' : '#f6f7f8'
}

export function labelInk(theme: Theme): string {
  return theme === 'dark' ? 'rgba(236, 239, 243, 0.94)' : 'rgba(22, 27, 33, 0.88)'
}

export function selectionInk(theme: Theme): string {
  return theme === 'dark' ? '#ffffff' : '#11161c'
}

export function hoverInk(theme: Theme): string {
  return theme === 'dark' ? '#86a9d1' : '#3e6a9e'
}

export function swatch(index: number, kind: string, theme: Theme): string {
  return tileFill({ kind, depth: 0, branch: index }, theme)
}
