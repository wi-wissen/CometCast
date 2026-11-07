// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  addressSourceKey,
  chipSelected,
  cleanRoomName,
  initialWizard,
  loadSetup,
  nextAddressStep,
  ROOM_NAME_MAX,
  saveDefaultMode,
  saveSetup,
  SETUP_KEY,
  wizardReduce,
  type WizardState,
} from '../src/core/displaySetup'

afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

const run = (s: WizardState, ...acts: Parameters<typeof wizardReduce>[1][]) => acts.reduce(wizardReduce, s)

describe('setup wizard state machine', () => {
  it('starts on the welcome screen with the first name and Open preselected (D1)', () => {
    expect(initialWizard('Meeting room Elbe')).toEqual({ step: 0, room: 'Meeting room Elbe', custom: '', mode: 'open' })
  })
  it('goes through the four steps and stops at the last; Back on the welcome screen does nothing', () => {
    let s = initialWizard('A')
    expect(wizardReduce(s, { type: 'back' }).step).toBe(0)
    for (let i = 1; i <= 4; i++) {
      s = wizardReduce(s, { type: 'next' })
      expect(s.step).toBe(i)
    }
    expect(wizardReduce(s, { type: 'next' }).step).toBe(4)
    expect(run(s, { type: 'back' }, { type: 'back' }).step).toBe(2)
  })
  it('keeps the room and the mode while going back and forth', () => {
    const s = run(
      initialWizard('A'),
      { type: 'next' },
      { type: 'room', value: 'Workshop' },
      { type: 'next' },
      { type: 'mode', value: 'confirm' },
      { type: 'back' },
      { type: 'back' },
    )
    expect(s).toEqual({ step: 0, room: 'Workshop', custom: '', mode: 'confirm' })
  })
  it('the custom field starts empty; typing deselects the chips, choosing a chip clears the field (spec 4.1)', () => {
    const chip = 'Meeting room Elbe'
    let s = initialWizard(chip)
    expect(s.custom).toBe('')
    expect(chipSelected(s, chip)).toBe(true)
    s = wizardReduce(s, { type: 'custom', value: 'Raum 204 ' })
    expect(s).toMatchObject({ custom: 'Raum 204 ', room: 'Raum 204' }) // what is typed stays as typed (a trailing space can be followed by more text)
    expect(chipSelected(s, chip)).toBe(false)
    expect(chipSelected(s, 'Raum 204')).toBe(false) // even a chip with the same text is not "selected" while the field has text
    s = wizardReduce(s, { type: 'room', value: 'Workshop' })
    expect(s).toMatchObject({ custom: '', room: 'Workshop' })
    expect(chipSelected(s, 'Workshop')).toBe(true)
    // the field cuts at 32 characters
    expect(Array.from(wizardReduce(s, { type: 'custom', value: 'y'.repeat(50) }).custom).length).toBe(ROOM_NAME_MAX)
    // emptying the field leaves no chip selected and no custom name (Finish falls back to the first suggestion)
    expect(wizardReduce(wizardReduce(s, { type: 'custom', value: 'x' }), { type: 'custom', value: '' })).toMatchObject({ custom: '', room: '' })
  })
  it('the address step: the source line and "Other address" (web)', () => {
    const k = (typed: boolean, source: 'user' | 'query' | 'config' | 'build' | 'origin', inChoices = true) => addressSourceKey({ typed, source, inChoices })
    expect(k(false, 'build')).toBe('setup.address.sourceDefault') // the default relay built into the page
    expect(k(false, 'origin')).toBe('setup.address.sourcePage') // the page's own server
    expect(k(false, 'config')).toBe('setup.address.sourcePage') // config.json
    expect(k(false, 'query')).toBe('setup.address.sourcePage')
    expect(k(false, 'user', true)).toBe('setup.address.sourcePage') // picked from the relays of config.json
    expect(k(false, 'user', false)).toBe('setup.address.sourceTyped') // chosen earlier in this tab, not offered by the page
    expect(k(true, 'user', false)).toBe('setup.address.sourceTyped')
    expect(k(true, 'build')).toBe('setup.address.sourceTyped')
    const ws = ['wss://a/ws', 'wss://b/ws', 'wss://c/ws']
    // through the relays of config.json, then the field
    expect(nextAddressStep(ws, 'wss://a/ws', false)).toEqual({ kind: 'choice', ws: 'wss://b/ws' })
    expect(nextAddressStep(ws, 'wss://b/ws', false)).toEqual({ kind: 'choice', ws: 'wss://c/ws' })
    expect(nextAddressStep(ws, 'wss://c/ws', false)).toEqual({ kind: 'field' })
    // one relay, or a typed address: the field at once
    expect(nextAddressStep(['wss://a/ws'], 'wss://a/ws', false)).toEqual({ kind: 'field' })
    expect(nextAddressStep(ws, 'wss://a/ws', true)).toEqual({ kind: 'field' })
    expect(nextAddressStep(ws, 'wss://typed/ws', false)).toEqual({ kind: 'field' })
  })
  it('cuts the room name at 32 characters and strips control characters', () => {
    expect(Array.from(cleanRoomName('x'.repeat(50))).length).toBe(ROOM_NAME_MAX)
    expect(cleanRoomName('  Room\u0007  204 ')).toBe('Room 204')
  })
})

describe('what Finish stores', () => {
  it('stores room name, default mode and server, and reads them back', () => {
    expect(loadSetup()).toBeNull()
    saveSetup({ room: 'Clubhouse', mode: 'confirm', server: 'wss://cast.example.org/ws' })
    expect(loadSetup()).toEqual({ room: 'Clubhouse', mode: 'confirm', server: 'wss://cast.example.org/ws' })
  })
  it('Settings changes only the default mode; Locked is never a default', () => {
    saveSetup({ room: 'R', mode: 'open', server: null })
    saveDefaultMode('confirm')
    expect(loadSetup()?.mode).toBe('confirm')
    localStorage.setItem(SETUP_KEY, JSON.stringify({ room: 'R', mode: 'locked', server: null }))
    expect(loadSetup()?.mode).toBe('open')
  })
  it('a broken or empty entry means the wizard shows again; a throwing storage is survived', () => {
    localStorage.setItem(SETUP_KEY, '{nope')
    expect(loadSetup()).toBeNull()
    localStorage.setItem(SETUP_KEY, JSON.stringify({ room: '  ', mode: 'open' }))
    expect(loadSetup()).toBeNull()
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(loadSetup()).toBeNull()
    expect(() => saveSetup({ room: 'R', mode: 'open', server: null })).not.toThrow()
  })
})
