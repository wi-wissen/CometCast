// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it } from 'vitest'
import { collectRows, moveFocus, nextCell } from '../src/core/dpad'

describe('focus moves in rows (7.1)', () => {
  // Settings: [Close] / [Open, Approval on, Locked] / [Dark, Light, OLED black, Automatic]
  const rows = [1, 3, 4]
  it('left/right move inside a row and do not wrap', () => {
    expect(nextCell(rows, { row: 2, col: 1 }, 'right')).toEqual({ row: 2, col: 2 })
    expect(nextCell(rows, { row: 2, col: 3 }, 'right')).toBeNull()
    expect(nextCell(rows, { row: 2, col: 0 }, 'left')).toBeNull()
  })
  it('up/down keep the column or go to the last item of the next row; no wrap', () => {
    expect(nextCell(rows, { row: 2, col: 3 }, 'up')).toEqual({ row: 1, col: 2 })
    expect(nextCell(rows, { row: 1, col: 2 }, 'up')).toEqual({ row: 0, col: 0 })
    expect(nextCell(rows, { row: 0, col: 0 }, 'up')).toBeNull()
    expect(nextCell(rows, { row: 1, col: 1 }, 'down')).toEqual({ row: 2, col: 1 })
    expect(nextCell(rows, { row: 2, col: 0 }, 'down')).toBeNull()
  })
})

describe('moveFocus on the DOM', () => {
  afterEach(() => (document.body.innerHTML = ''))
  it('walks the rows given by data-row and ignores disabled items', () => {
    document.body.innerHTML = `<div id="r">
      <button id="a" data-row="0"></button>
      <button id="b" data-row="1"></button><button id="c" data-row="1" disabled></button><button id="d" data-row="1"></button>
    </div>`
    const root = document.getElementById('r')!
    expect(collectRows(root).map((r) => r.map((e) => e.id))).toEqual([['a'], ['b', 'd']])
    document.getElementById('a')!.focus()
    moveFocus(root, 'down')
    expect(document.activeElement?.id).toBe('b')
    moveFocus(root, 'right')
    expect(document.activeElement?.id).toBe('d')
    moveFocus(root, 'right')
    expect(document.activeElement?.id).toBe('d')
    moveFocus(root, 'up')
    expect(document.activeElement?.id).toBe('a')
  })
})
