// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'

function keys(obj: Record<string, unknown>, prefix = ''): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === 'object' ? keys(v as Record<string, unknown>, `${prefix}${k}.`) : [`${prefix}${k}`],
  )
}

describe('locale files', () => {
  it('en.json and de.json have identical key sets', () => {
    expect(keys(de).sort()).toEqual(keys(en).sort())
  })
  it('has no empty strings', () => {
    for (const [name, file] of [['en', en], ['de', de]] as const) {
      for (const k of keys(file)) {
        const v = k.split('.').reduce<any>((o, p) => o[p], file)
        expect(v, `${name}:${k}`).not.toBe('')
      }
    }
  })
})
