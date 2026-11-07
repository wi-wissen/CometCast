// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanName, DISPLAY_NAME_KEY, loadDisplayName, saveDisplayName, shortName } from '../src/core/joinPrefs'

afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('presenter name', () => {
  it('cleans and cuts names', () => {
    expect(cleanName('  Anna \n\t Schmid  ')).toBe('Anna Schmid')
    expect(cleanName('A‮B​C\u0007D')).toBe('ABCD')
    expect(Array.from(cleanName('x'.repeat(80))).length).toBe(40)
    expect(shortName('Anna-Maria Schmid-Wolfgang')).toBe('Anna-Maria Schmid-…')
    expect(shortName('Ana')).toBe('Ana')
  })
  it('is remembered on the device and survives a localStorage that throws', () => {
    saveDisplayName('Lena')
    expect(localStorage.getItem(DISPLAY_NAME_KEY)).toBe('Lena')
    expect(loadDisplayName()).toBe('Lena')
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(loadDisplayName()).toBe('')
    expect(() => saveDisplayName('x')).not.toThrow()
  })
})
