// SPDX-License-Identifier: GPL-3.0-or-later
//
// Some sentences carry a value that is styled on its own (a mono security code, a link). The translated sentence
// is built once with a marker in place of the value and cut there, so that every language puts the value at its own
// place and the page can draw it as an element.

const MARK = '\u0001'

export interface Slot {
  before: string
  after: string
}

/** `translate(mark)` returns the sentence with `mark` where the value belongs. `suffix` text that must follow the marker directly is dropped from `after` (see below). */
export function splitSlot(translate: (mark: string) => string, suffixLength = 0): Slot {
  const s = translate(MARK)
  const at = s.indexOf(MARK)
  if (at < 0) return { before: s, after: '' }
  return { before: s.slice(0, at), after: s.slice(at + MARK.length + suffixLength) }
}
