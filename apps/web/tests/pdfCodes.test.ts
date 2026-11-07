// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { codeOf } from '../src/core/pdfCodes'

describe('codeOf', () => {
  it('passes the codes of the wasm side', () => {
    for (const c of ['invalid', 'encrypted', 'too_large', 'page_out_of_range', 'bad_size', 'timeout', 'memory'])
      expect(codeOf(new Error(c))).toBe(c)
  })
  it('turns traps and unknown messages into a fatal code', () => {
    // `unreachable` also looks like a code ([a-z_]): it must not pass as one.
    expect(codeOf(new Error('unreachable'))).toBe('memory')
    expect(codeOf(new Error('memory access out of bounds'))).toBe('memory')
    expect(codeOf(new Error('some_new_thing'))).toBe('failed')
    expect(codeOf('oops')).toBe('failed')
    expect(codeOf(undefined)).toBe('failed')
  })
})
