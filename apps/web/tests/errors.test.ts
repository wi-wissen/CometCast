// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'
import { SERVER_ERROR_CODES } from '../src/core/signaling'

describe('error code translations', () => {
  it('has a translation for every server error code and local code', () => {
    for (const lang of [en, de]) {
      for (const c of [...SERVER_ERROR_CODES, 'connection_lost', 'peer_failed', 'capture_denied', 'capture_failed', 'unknown']) {
        expect((lang.errors as Record<string, string>)[c], c).toBeTruthy()
      }
    }
  })
})
