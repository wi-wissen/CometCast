// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'
import deSie from '../src/locales/de-sie.json'
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
    for (const [name, file] of [['en', en], ['de', de], ['de-sie', deSie]] as const) {
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
  it('"du" is the loaded set; "Sie" is kept in de-sie.json but not loaded', () => {
    expect(get(de, 'start.nameLabel')).toBe('Dein Name')
    expect(get(deSie, 'start.nameLabel')).toBe('Ihr Name')
  })
  it('both sets have identical keys and identical placeholders for every key', () => {
    expect(keys(deSie).sort()).toEqual(keys(de).sort())
    for (const k of keys(de)) {
      expect(placeholders(get(deSie, k)), k).toEqual(placeholders(get(de, k)))
    }
  })
  it('the loaded de set has no formal forms (Sie, Ihr..., Ihnen) except where the spec itself has them', () => {
    const formal = /\b(Sie|Ihr|Ihre|Ihren|Ihrem|Ihrer|Ihres|Ihnen)\b/
    for (const k of keys(de)) if (formal.test(get(de, k))) expect(k).toBe('receive.media.hint') // "Sie" = the media file
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
    const key = c[0].replace(/`/g, '').replace(/ \(valid: [^)]*\)$/, '')
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
  it('every row is in en.json, de.json (du) and de-sie.json (Sie), word for word', () => {
    for (const r of rows) {
      expect(get(en, r.key), `en:${r.key}`).toBe(r.en)
      expect(get(de, r.key), `de:${r.key}`).toBe(r.du)
      expect(get(deSie, r.key), `de-sie:${r.key}`).toBe(r.sie)
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
      if (r.sie !== undefined) expect(get(deSie, r.key), `de-sie:${r.key}`).toBe(r.sie)
      expect(get(de, r.key), `de:${r.key}`).toBe(r.du)
    }
  })
  it('every key where du and Sie differ is in the spec', () => {
    const known = new Set([...rows.map((r) => r.key), ...more.map((r) => r.key), 'relay.confirm.body', 'relay.confirm.risk', 'receive.link.hint', 'send.status.idle', 'sendContent.status.idle'])
    for (const k of keys(de)) if (get(de, k) !== get(deSie, k)) expect(known.has(k), k).toBe(true)
  })
})
