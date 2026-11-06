// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { AnnotationModel } from '../src/core/annotations'
import {
  MARKER_WIDTHS,
  PEN_WIDTHS,
  initialToolbar,
  moveTo,
  penStyle,
  setColor,
  setOpen,
  setSize,
  toggleBlack,
  toggleFreeze,
  toggleTool,
} from '../src/core/toolbarState'

describe('toolbar state', () => {
  it('starts open with no tool', () => {
    const s = initialToolbar()
    expect(s.open).toBe(true)
    expect(s.tool).toBeNull()
    expect(penStyle(s)).toBeNull()
  })
  it('tools are exclusive and toggle off when selected twice', () => {
    let s = toggleTool(initialToolbar(), 'pen')
    expect(s.tool).toBe('pen')
    s = toggleTool(s, 'eraser')
    expect(s.tool).toBe('eraser')
    s = toggleTool(s, 'eraser')
    expect(s.tool).toBeNull()
  })
  it('choosing a colour while not drawing selects the pen; while on marker it stays', () => {
    expect(setColor(initialToolbar(), 'blue').tool).toBe('pen')
    expect(setColor(toggleTool(initialToolbar(), 'eraser'), 'blue').tool).toBe('pen')
    const m = setColor(toggleTool(initialToolbar(), 'marker'), 'green')
    expect(m.tool).toBe('marker')
    expect(m.color).toBe('green')
  })
  it('pen and marker widths follow the size and the tool', () => {
    let s = toggleTool(initialToolbar(), 'pen')
    for (const i of [0, 1, 2] as const) {
      s = setSize(s, i)
      expect(penStyle(s)).toMatchObject({ kind: 'pen', width: PEN_WIDTHS[i] })
    }
    s = setSize(toggleTool(initialToolbar(), 'marker'), 2)
    expect(penStyle(s)).toMatchObject({ kind: 'marker', width: MARKER_WIDTHS[2] })
  })
  it('freeze, black screen and open are independent toggles', () => {
    let s = toggleTool(initialToolbar(), 'laser')
    s = toggleFreeze(toggleBlack(setOpen(s, false)))
    expect(s).toMatchObject({ tool: 'laser', frozen: true, black: true, open: false })
    s = toggleFreeze(s)
    expect(s.frozen).toBe(false)
  })
  it('clamps the position to 0..100', () => {
    expect(moveTo(initialToolbar(), -5, 140)).toMatchObject({ x: 0, y: 100 })
    expect(moveTo(initialToolbar(), NaN, 20)).toMatchObject({ x: 0, y: 20 })
  })
})

describe('AnnotationModel', () => {
  it('undoes strokes, erase and clear', () => {
    const m = new AnnotationModel()
    m.beginStroke('pen', '#f00', 4, [0.1, 0.1])
    m.extend([0.5, 0.1])
    m.endStroke()
    m.beginStroke('marker', '#ff0', 12, [0.1, 0.5])
    m.extend([0.9, 0.5])
    m.endStroke()
    expect(m.strokes.length).toBe(2)
    expect(m.canUndo).toBe(true)

    m.beginErase()
    expect(m.eraseAt([0.3, 0.1], 10, 1000, 1000)).toBe(true)
    expect(m.strokes.length).toBe(1)
    expect(m.eraseAt([0.3, 0.9], 10, 1000, 1000)).toBe(false)
    m.undo() // un-erase (one step for the whole gesture)
    expect(m.strokes.length).toBe(2)

    m.clear()
    expect(m.isEmpty).toBe(true)
    m.undo()
    expect(m.strokes.length).toBe(2)
    m.undo()
    m.undo()
    expect(m.isEmpty).toBe(true)
    expect(m.undo()).toBe(false)
  })
  it('notifies listeners and does not clear an empty canvas into history', () => {
    const m = new AnnotationModel()
    let n = 0
    m.onChange = () => n++
    m.clear()
    expect(n).toBe(0)
    expect(m.canUndo).toBe(false)
    m.beginStroke('pen', '#000', 2, [0, 0])
    expect(n).toBe(1)
  })
})
