// SPDX-License-Identifier: GPL-3.0-or-later
import { beforeEach, describe, expect, it } from 'vitest'
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
  it('switches language and updates <html lang>', async () => {
    const w = mount(App, { global: { plugins: [createAppI18n('en')] } })
    expect(w.text()).toContain('Send screen')
    await w.find('button[lang="de"]').trigger('click')
    expect(w.text()).toContain('Bildschirm senden')
    expect(document.documentElement.lang).toBe('de')
  })
})
