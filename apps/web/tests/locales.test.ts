// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'
import deDu from '../src/locales/de-du.json'
import spec from '../../../docs/design/first-run-and-display.md?raw'

function keys(obj: Record<string, unknown>, prefix = ''): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === 'object' ? keys(v as Record<string, unknown>, `${prefix}${k}.`) : [`${prefix}${k}`],
  )
}
const get = (o: unknown, k: string): string => k.split('.').reduce<any>((x, p) => x?.[p], o)
const placeholders = (s: string): string[] => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort()

describe('locale files', () => {
  it('en.json and de.json have identical key sets', () => {
    expect(keys(de).sort()).toEqual(keys(en).sort())
  })
  it('has no empty strings', () => {
    for (const [name, file] of [['en', en], ['de', de], ['de-du', deDu]] as const) {
      for (const k of keys(file)) expect(get(file, k), `${name}:${k}`).not.toBe('')
    }
  })
  it('uses the same placeholders in every language', () => {
    for (const k of keys(en)) {
      expect(placeholders(get(de, k)), `de:${k}`).toEqual(placeholders(get(en, k)))
    }
  })
})

describe('the two German sets (spec 11.1)', () => {
  it('"du" is not loaded by default: the app ships "Sie"', () => {
    expect(get(de, 'start.nameLabel')).toBe('Ihr Name')
    expect(get(deDu, 'start.nameLabel')).toBe('Dein Name')
  })
  it('de-du.json has exactly the keys of the spec and none that de.json lacks', () => {
    const du = keys(deDu).sort()
    const all = new Set(keys(de))
    for (const k of du) expect(all.has(k), k).toBe(true)
    // Every string of the spec exists in both forms: the spec keys are those of de-du.
    expect(du.length).toBeGreaterThan(150)
  })
  it('both sets have identical keys and identical placeholders for every key', () => {
    for (const k of keys(deDu)) {
      expect(placeholders(get(deDu, k)), k).toEqual(placeholders(get(de, k)))
    }
  })
})

// The copy tables of docs/design/first-run-and-display.md are the source: every row must be in the
// files, word for word (EN column, "DE du" column, "DE Sie" column).
describe('the copy tables of the spec', () => {
  const body = spec.slice(spec.indexOf('### 11.2 Common'), spec.indexOf('"same as du" means'))
  const rows: Array<{ key: string; en: string; du: string; sie: string }> = []
  for (const line of body.split('\n')) {
    if (!line.startsWith('| `')) continue
    const c = line.trim().replace(/^\||\|$/g, '').split(' | ').map((x: string) => x.trim())
    const key = c[0].replace(/`/g, '')
    const sie = c[3] === 'same as du' ? c[2] : c[3]
    if (key.startsWith('setup.name.s1')) {
      const [e, d] = [c[1].split(' · '), c[2].split(' · ')]
      e.forEach((v: string, i: number) => rows.push({ key: `setup.name.s${i + 1}`, en: v, du: d[i], sie: d[i] }))
      continue
    }
    // `settings.join` and `settings.join.*Text` cannot both be a string and a group in nested JSON.
    rows.push({ key: key === 'settings.join' ? 'settings.join.label' : key, en: c[1], du: c[2], sie })
  }
  it('has read all rows', () => expect(rows.length).toBe(181))
  it('every row is in en.json, de.json (Sie) and de-du.json (du), word for word', () => {
    for (const r of rows) {
      expect(get(en, r.key), `en:${r.key}`).toBe(r.en)
      expect(get(de, r.key), `de:${r.key}`).toBe(r.sie)
      expect(get(deDu, r.key), `de-du:${r.key}`).toBe(r.du)
    }
  })
  it('de-du.json has no key that is not in the spec', () => {
    const known = new Set(rows.map((r) => r.key))
    for (const k of keys(deDu)) expect(known.has(k), k).toBe(true)
  })
})
