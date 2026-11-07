// SPDX-License-Identifier: GPL-3.0-or-later
//
// Focus moves in rows (7.1): Left/Right move within a row, Up/Down move to the same column (or the
// last item) of the next row. Focus does not wrap. Rows are given by `data-row` on the focusable
// elements (a number); the order inside a row is the document order.

export type Dir = 'left' | 'right' | 'up' | 'down'

export interface Cell {
  row: number
  col: number
}

/** Pure: where the focus goes. `rows[r]` = number of items in row r (rows sorted ascending). Null = stay. */
export function nextCell(rows: number[], cur: Cell, dir: Dir): Cell | null {
  if (rows.length === 0) return null
  if (dir === 'left') return cur.col > 0 ? { row: cur.row, col: cur.col - 1 } : null
  if (dir === 'right') return cur.col < (rows[cur.row] ?? 0) - 1 ? { row: cur.row, col: cur.col + 1 } : null
  const r = dir === 'up' ? cur.row - 1 : cur.row + 1
  if (r < 0 || r >= rows.length) return null
  return { row: r, col: Math.min(cur.col, Math.max(0, rows[r] - 1)) }
}

/** The focusable rows inside `root` (elements carrying data-row), in document order, not disabled or hidden. */
export function collectRows(root: HTMLElement): HTMLElement[][] {
  const byRow = new Map<number, HTMLElement[]>()
  for (const el of Array.from(root.querySelectorAll<HTMLElement>('[data-row]'))) {
    if ((el as HTMLButtonElement).disabled || el.getAttribute('aria-hidden') === 'true') continue
    const r = Number(el.dataset.row)
    if (!Number.isFinite(r)) continue
    byRow.set(r, [...(byRow.get(r) ?? []), el])
  }
  return [...byRow.entries()].sort((a, b) => a[0] - b[0]).map(([, els]) => els)
}

/** Moves the focus inside `root` for an arrow key. Returns true when the key was handled. */
export function moveFocus(root: HTMLElement, dir: Dir, active: Element | null = document.activeElement): boolean {
  const rows = collectRows(root)
  if (rows.length === 0) return false
  let cur: Cell | null = null
  rows.forEach((els, r) => {
    const c = els.findIndex((e) => e === active)
    if (c >= 0) cur = { row: r, col: c }
  })
  if (!cur) {
    rows[0][0].focus()
    return true
  }
  const next = nextCell(rows.map((r) => r.length), cur, dir)
  if (next) rows[next.row][next.col].focus()
  return true
}

export function arrowDir(key: string): Dir | null {
  switch (key) {
    case 'ArrowLeft':
      return 'left'
    case 'ArrowRight':
      return 'right'
    case 'ArrowUp':
      return 'up'
    case 'ArrowDown':
      return 'down'
    default:
      return null
  }
}
