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
    if (c[2] === '' || c[2] === '|') continue // `error.other.why` has no text of its own: it shows the existing `errors.<code>` text
    const sie = c[3] === 'same as du' ? c[2] : c[3]
    if (key.startsWith('setup.name.s1')) {
      const [e, d] = [c[1].split(' · '), c[2].split(' · ')]
      e.forEach((v: string, i: number) => rows.push({ key: `setup.name.s${i + 1}`, en: v, du: d[i], sie: d[i] }))
      continue
    }
    // `settings.join` and `settings.join.*Text` cannot both be a string and a group in nested JSON.
    // The EN column of two rows carries a note of the spec ("(not used, ...)", "(apps only)"), not copy.
    rows.push({ key: key === 'settings.join' ? 'settings.join.label' : key, en: c[1].replace(/ \((not used, decision 13\.2|apps only)\)$/, ''), du: c[2], sie })
  }
  it('has read all rows', () => expect(rows.length).toBe(210))
  it('every row is in en.json, de.json (Sie) and de-du.json (du), word for word', () => {
    for (const r of rows) {
      expect(get(en, r.key), `en:${r.key}`).toBe(r.en)
      expect(get(de, r.key), `de:${r.key}`).toBe(r.sie)
      expect(get(deDu, r.key), `de-du:${r.key}`).toBe(r.du)
    }
  })
  // 11.7 (items the mockup did not draw, EN / du / Sie) and 11.8 (the du values of Sie texts that address the reader).
  const more: Array<{ key: string; en?: string; du: string; sie?: string }> = []
  const tail = spec.slice(spec.indexOf('### 11.7 Copy of items'), spec.indexOf('## 12. Open points'))
  for (const line of tail.split('\n')) {
    if (!line.startsWith('| `')) continue
    const c = line.trim().replace(/^\||\|$/g, '').split(' | ').map((x: string) => x.trim())
    const keys = [...c[0].matchAll(/`([^`]+)`/g)].map((m) => m[1])
    if (keys.length !== 1 || c[0].includes(' · ') || c[1] === 'see 11.7') continue // grouped rows are checked by their own tests
    if (c.length === 2) more.push({ key: keys[0], du: c[1] })
    else more.push({ key: keys[0], en: c[1], du: c[2], sie: c[3] === 'same as du' ? c[2] : c[3] })
  }
  it('11.7 and 11.8: every single-key row is in the files, word for word', () => {
    expect(more.length).toBeGreaterThan(50)
    for (const r of more) {
      if (get(en, r.key) === undefined) continue // native-only keys (app.beDisplay is there; others are Flutter's)
      if (r.en !== undefined) expect(get(en, r.key), `en:${r.key}`).toBe(r.en)
      if (r.sie !== undefined) expect(get(de, r.key), `de:${r.key}`).toBe(r.sie)
      // The du file lists only the texts that differ from Sie; a key it lacks shows the Sie text.
      expect(get(deDu, r.key) ?? get(de, r.key), `de-du:${r.key}`).toBe(r.du)
    }
  })
  it('the du set has every key whose Sie text addresses the reader (spec 11.8)', () => {
    const addresses = /\b(Sie|Ihr|Ihre|Ihren|Ihrem|Ihrer|Ihres|Ihnen)\b/
    const du = new Set(keys(deDu))
    for (const k of keys(de)) if (addresses.test(get(de, k)) && k !== 'receive.media.hint') expect(du.has(k), k).toBe(true)
  })
  it('de-du.json has no key that is not in the spec', () => {
    const known = new Set([...rows.map((r) => r.key), ...more.map((r) => r.key), 'relay.confirm.body', 'relay.confirm.risk', 'receive.link.hint', 'send.status.idle', 'sendContent.status.idle'])
    for (const k of keys(deDu)) expect(known.has(k), k).toBe(true)
  })
})
