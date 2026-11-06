// SPDX-License-Identifier: GPL-3.0-or-later
//
// Pure state of the receiver toolbar (no DOM). Tools: pen, marker, eraser and laser are exclusive;
// freeze and black screen are independent toggles.
import type { PenKind } from './annotations'

export type Tool = 'pen' | 'marker' | 'eraser' | 'laser'
export type SizeIndex = 0 | 1 | 2

export const COLORS = [
  { id: 'red', hex: '#e5484d' },
  { id: 'yellow', hex: '#f5c451' },
  { id: 'green', hex: '#30a46c' },
  { id: 'blue', hex: '#3e8bff' },
  { id: 'white', hex: '#ffffff' },
] as const
export type ColorId = (typeof COLORS)[number]['id']

/** Line widths in CSS px for the three sizes. */
export const PEN_WIDTHS = [2, 4, 8] as const
export const MARKER_WIDTHS = [12, 20, 32] as const
export const ERASER_RADIUS = 16

export interface ToolbarState {
  open: boolean
  tool: Tool | null
  color: ColorId
  size: SizeIndex
  frozen: boolean
  black: boolean
  /** Position of the toolbar's top-left corner, in percent of the stage. */
  x: number
  y: number
}

export function initialToolbar(): ToolbarState {
  return { open: true, tool: null, color: 'red', size: 1, frozen: false, black: false, x: 50, y: 3 }
}

/** Selecting the active tool again deselects it (back to plain viewing). */
export function toggleTool(s: ToolbarState, tool: Tool): ToolbarState {
  return { ...s, tool: s.tool === tool ? null : tool }
}

/** Picking a colour while no drawing tool is active switches to the pen. */
export function setColor(s: ToolbarState, color: ColorId): ToolbarState {
  const drawing = s.tool === 'pen' || s.tool === 'marker'
  return { ...s, color, tool: drawing ? s.tool : 'pen' }
}

export function setSize(s: ToolbarState, size: SizeIndex): ToolbarState {
  return { ...s, size }
}

export function toggleFreeze(s: ToolbarState): ToolbarState {
  return { ...s, frozen: !s.frozen }
}

export function toggleBlack(s: ToolbarState): ToolbarState {
  return { ...s, black: !s.black }
}

export function setOpen(s: ToolbarState, open: boolean): ToolbarState {
  return { ...s, open }
}

export function moveTo(s: ToolbarState, x: number, y: number): ToolbarState {
  const c = (v: number) => Math.min(100, Math.max(0, Number.isFinite(v) ? v : 0))
  return { ...s, x: c(x), y: c(y) }
}

export function penStyle(s: ToolbarState): { kind: PenKind; color: string; width: number } | null {
  if (s.tool !== 'pen' && s.tool !== 'marker') return null
  const hex = COLORS.find((c) => c.id === s.color)?.hex ?? COLORS[0].hex
  const width = (s.tool === 'pen' ? PEN_WIDTHS : MARKER_WIDTHS)[s.size]
  return { kind: s.tool, color: hex, width }
}
