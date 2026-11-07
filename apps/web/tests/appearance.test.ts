// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest'
import { APPEARANCE_KEY, DEFAULT_APPEARANCE, loadAppearance, resolveAppearance, saveAppearance } from '../src/core/appearance'

afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('appearance', () => {
  it('Dark is the default and a bad value falls back to it', () => {
    expect(DEFAULT_APPEARANCE).toBe('dark')
    expect(loadAppearance()).toBe('dark')
    localStorage.setItem(APPEARANCE_KEY, 'neon')
    expect(loadAppearance()).toBe('dark')
  })
  it('is kept on the device', () => {
    for (const a of ['light', 'oled', 'auto', 'dark'] as const) {
      saveAppearance(a)
      expect(loadAppearance()).toBe(a)
    }
  })
  it('Automatic follows the system and never picks OLED black', () => {
    expect(resolveAppearance('auto', true)).toBe('dark')
    expect(resolveAppearance('auto', false)).toBe('light')
    expect(resolveAppearance('oled', false)).toBe('oled')
    expect(resolveAppearance('light', true)).toBe('light')
  })
  it('survives a localStorage that throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(loadAppearance()).toBe('dark')
    expect(() => saveAppearance('oled')).not.toThrow()
  })
})
