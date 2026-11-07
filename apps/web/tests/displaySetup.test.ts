// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cleanRoomName,
  initialWizard,
  loadSetup,
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
    expect(initialWizard('Meeting room Elbe')).toEqual({ step: 0, room: 'Meeting room Elbe', mode: 'open' })
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
    expect(s).toEqual({ step: 0, room: 'Workshop', mode: 'confirm' })
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
