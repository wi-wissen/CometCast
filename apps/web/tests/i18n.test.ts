// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { createAppI18n, detectLocale, initialLocale, STORAGE_KEY } from '../src/i18n'
import { isValidRoomCode, normalizeRoomCode } from '../src/core/roomCode'
import App from '../src/App.vue'

describe('detectLocale', () => {
  it('maps de* to de', () => {
    for (const l of ['de', 'de-DE', 'de-AT', 'DE-ch', 'de_DE']) expect(detectLocale(l)).toBe('de')
  })
  it('maps everything else to en', () => {
    for (const l of ['en-US', 'fr', 'dk', 'deu', '', undefined]) expect(detectLocale(l)).toBe('en')
  })
})

describe('initialLocale', () => {
  beforeEach(() => localStorage.clear())
  it('prefers the stored choice', () => {
    localStorage.setItem(STORAGE_KEY, 'de')
    expect(initialLocale()).toBe('de')
  })
  it('falls back to navigator.language (jsdom: en-US)', () => {
    expect(initialLocale()).toBe('en')
  })
})

describe('room code', () => {
  it('normalizes and validates', () => {
    expect(normalizeRoomCode('ab0o1i-cdefgh')).toBe('ABCDEF')
    expect(isValidRoomCode('ABCDEF')).toBe(true)
    expect(isValidRoomCode('ABCDE')).toBe(false)
  })
})

describe('App', () => {
  afterEach(() => history.replaceState(null, '', '/'))
  it('shows the presenter start page at / and picks the language by the browser language', () => {
    const en = mount(App, { global: { plugins: [createAppI18n('en')] } })
    expect(en.text()).toContain('Connect to a screen')
    expect(en.find('[data-testid=start-page]').exists()).toBe(true)
    const de = mount(App, { global: { plugins: [createAppI18n('de')] } })
    expect(de.text()).toContain('Mit einem Bildschirm verbinden')
    expect(document.documentElement.lang).toBe('de')
  })
  it('opens the start page with the code when the path is a code (QR code)', () => {
    history.replaceState(null, '', '/K7QM2W')
    const w = mount(App, { global: { plugins: [createAppI18n('en')] } })
    expect(w.get('[data-testid=start-code-chip]').text()).toBe('K7Q M2W')
  })
  it('shows the display at /display (setup wizard on first run)', () => {
    history.replaceState(null, '', '/display')
    localStorage.clear()
    const w = mount(App, { global: { plugins: [createAppI18n('en')] } })
    expect(w.find('[data-testid=display]').exists()).toBe(true)
    expect(w.find('[data-testid=setup]').exists()).toBe(true)
    expect(w.get('[data-testid=display]').attributes('data-theme')).toBe('dark')
  })
})
