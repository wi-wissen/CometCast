// SPDX-License-Identifier: GPL-3.0-or-later
//
// Local annotation model for the receiver. Strokes live only in this browser tab; nothing is sent
// anywhere. Points are normalised to the stage (0..1) so that resizing and fullscreen keep them in place.

export type PenKind = 'pen' | 'marker'
export type Point = readonly [number, number]

export interface Stroke {
  kind: PenKind
  color: string
  /** Line width in CSS pixels. */
  width: number
  points: Point[]
}

const MAX_HISTORY = 100

function distToSegment(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax
  const dy = by - ay
  const len2 = dx * dx + dy * dy
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2))
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy))
}

export class AnnotationModel {
  strokes: Stroke[] = []
  /** Called after every change (the view redraws). */
  onChange: (() => void) | null = null
  private history: Stroke[][] = []
  private current: Stroke | null = null
  private erasePushed = false

  get canUndo(): boolean {
    return this.history.length > 0
  }

  get isEmpty(): boolean {
    return this.strokes.length === 0
  }

  private push(): void {
    this.history.push(this.strokes)
    if (this.history.length > MAX_HISTORY) this.history.shift()
  }

  private changed(): void {
    this.onChange?.()
  }

  beginStroke(kind: PenKind, color: string, width: number, p: Point): void {
    this.push()
    this.current = { kind, color, width, points: [p] }
    this.strokes = [...this.strokes, this.current]
    this.changed()
  }

  extend(p: Point): void {
    if (!this.current) return
    this.current.points.push(p)
    this.changed()
  }

  endStroke(): void {
    this.current = null
  }

  beginErase(): void {
    this.erasePushed = false
  }

  /** Removes every stroke that passes within `radiusPx` of p. `w`/`h` is the stage size in px. */
  eraseAt(p: Point, radiusPx: number, w: number, h: number): boolean {
    const px = p[0] * w
    const py = p[1] * h
    const keep = this.strokes.filter((s) => {
      const r = radiusPx + s.width / 2
      const pts = s.points
      if (pts.length === 1) return Math.hypot(pts[0]![0] * w - px, pts[0]![1] * h - py) > r
      for (let i = 1; i < pts.length; i++) {
        const a = pts[i - 1]!
        const b = pts[i]!
        if (distToSegment(px, py, a[0] * w, a[1] * h, b[0] * w, b[1] * h) <= r) return false
      }
      return true
    })
    if (keep.length === this.strokes.length) return false
    if (!this.erasePushed) {
      this.push()
      this.erasePushed = true
    }
    this.strokes = keep
    this.changed()
    return true
  }

  undo(): boolean {
    const prev = this.history.pop()
    if (!prev) return false
    this.current = null
    this.strokes = prev
    this.changed()
    return true
  }

  /** Drops all strokes and the undo history (used when the content underneath changes). */
  reset(): void {
    this.strokes = []
    this.history = []
    this.current = null
    this.changed()
  }

  clear(): void {
    if (this.strokes.length === 0) return
    this.push()
    this.strokes = []
    this.changed()
  }
}

/** Draws all strokes. `w`/`h` is the canvas size in CSS pixels; the context is already scaled by the pixel ratio. */
export function drawStrokes(ctx: CanvasRenderingContext2D, strokes: readonly Stroke[], w: number, h: number): void {
  ctx.clearRect(0, 0, w, h)
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'
  for (const s of strokes) {
    ctx.globalAlpha = s.kind === 'marker' ? 0.35 : 1
    ctx.strokeStyle = s.color
    ctx.fillStyle = s.color
    ctx.lineWidth = s.width
    const first = s.points[0]
    if (!first) continue
    if (s.points.length === 1) {
      ctx.beginPath()
      ctx.arc(first[0] * w, first[1] * h, s.width / 2, 0, Math.PI * 2)
      ctx.fill()
      continue
    }
    ctx.beginPath()
    ctx.moveTo(first[0] * w, first[1] * h)
    for (let i = 1; i < s.points.length; i++) ctx.lineTo(s.points[i]![0] * w, s.points[i]![1] * h)
    ctx.stroke()
  }
  ctx.globalAlpha = 1
}
