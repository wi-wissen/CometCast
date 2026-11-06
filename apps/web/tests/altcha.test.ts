// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
// The fixture lives next to the Rust test that solves it too (crates/cc-transport/testdata).
import knownFixture from '../../../crates/cc-transport/testdata/altcha-known.json'
import {
  AltchaError,
  buildPassword,
  checkChallengeLimits,
  MAX_COST,
  MAX_KEY_LENGTH,
  bytesToHex,
  encodePayload,
  hexToBytes,
  solveChallenge,
  type AltchaChallenge,
} from '../src/core/altcha'

// Made by the Rust `altcha` crate (the server's implementation); the same file is solved by the
// Rust client in crates/cc-transport/src/altcha.rs (test known_challenge_fixture).
const fixture = knownFixture as unknown as { challenge: AltchaChallenge; counter: number; derivedKey: string }
const subtle = globalThis.crypto.subtle

describe('ALTCHA solver', () => {
  it('finds the counter and derived key of a challenge made by the Rust crate', async () => {
    const s = await solveChallenge(fixture.challenge, { subtle })
    expect(s.counter).toBe(fixture.counter)
    expect(s.derivedKey).toBe(fixture.derivedKey)
    expect(s.time).toBeGreaterThanOrEqual(0)
  })

  it('agrees with a one-shot PBKDF2 computation written separately from the solver', async () => {
    const p = fixture.challenge.parameters
    const hex = (h: string) => new Uint8Array(h.match(/../g)!.map((x) => parseInt(x, 16)))
    const nonce = hex(p.nonce)
    const pw = new Uint8Array([...nonce, 0, 0, 0x03, 0x09]) // counter 777 = 0x0309, big-endian
    const key = await subtle.importKey('raw', pw, 'PBKDF2', false, ['deriveBits'])
    const bits = await subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: hex(p.salt), iterations: p.cost }, key, p.keyLength * 8)
    const derived = [...new Uint8Array(bits)].map((b) => b.toString(16).padStart(2, '0')).join('')
    expect(derived).toBe(fixture.derivedKey)
    expect(derived.startsWith(p.keyPrefix)).toBe(true)
  })

  it('builds the password as nonce || uint32 big-endian', () => {
    const pw = buildPassword(new Uint8Array([1, 2]), 0x01020304)
    expect([...pw]).toEqual([1, 2, 1, 2, 3, 4])
    expect(bytesToHex(hexToBytes('00ff10'))).toBe('00ff10')
    expect(() => hexToBytes('abc')).toThrow(AltchaError)
    expect(() => hexToBytes('zz')).toThrow(AltchaError)
  })

  it('accepts an upper-case or odd-length key prefix', async () => {
    const upper = structuredClone(fixture.challenge)
    upper.parameters.keyPrefix = fixture.challenge.parameters.keyPrefix.toUpperCase()
    expect((await solveChallenge(upper, { subtle })).counter).toBe(fixture.counter)
    // "fdf" is odd length: the Rust crate compares it as text against the hex of the key.
    const odd = structuredClone(fixture.challenge)
    odd.parameters.keyPrefix = 'fdf'
    expect((await solveChallenge(odd, { subtle })).counter).toBeLessThanOrEqual(fixture.counter)
  })

  it('refuses algorithms it does not implement and malformed challenges', async () => {
    const bad = structuredClone(fixture.challenge)
    bad.parameters.algorithm = 'ARGON2ID'
    await expect(solveChallenge(bad, { subtle })).rejects.toMatchObject({ reason: 'unsupported_algorithm' })
    const noNonce = { parameters: { algorithm: 'PBKDF2/SHA-256' } } as unknown as AltchaChallenge
    await expect(solveChallenge(noNonce, { subtle })).rejects.toMatchObject({ reason: 'bad_challenge' })
    const badHex = structuredClone(fixture.challenge)
    badHex.parameters.keyPrefix = 'xyz'
    await expect(solveChallenge(badHex, { subtle })).rejects.toMatchObject({ reason: 'bad_challenge' })
  })

  it('can be aborted and has a timeout', async () => {
    const ctl = new AbortController()
    ctl.abort()
    await expect(solveChallenge(fixture.challenge, { subtle, signal: ctl.signal })).rejects.toMatchObject({ reason: 'aborted' })
    // An unreachable prefix (31 hex digits, the last one 1: practically never matches).
    const hard = structuredClone(fixture.challenge)
    hard.parameters.keyPrefix = '0'.repeat(30) + '1'
    await expect(solveChallenge(hard, { subtle, timeoutMs: 50 })).rejects.toMatchObject({ reason: 'timeout' })
  })

  it('encodes the payload as base64 of {challenge, solution} with the challenge unchanged', async () => {
    const s = await solveChallenge(fixture.challenge, { subtle })
    const text = encodePayload(fixture.challenge, s)
    const back = JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(text), (c) => c.charCodeAt(0))))
    expect(back.challenge).toEqual(fixture.challenge)
    expect(back.solution).toEqual({ counter: 777, derivedKey: fixture.derivedKey, time: s.time })
    // standard base64 (with padding), as the server's decoder expects
    expect(text).toMatch(/^[A-Za-z0-9+/]+=*$/)
  })
})

describe('ALTCHA limits', () => {
  const withParams = (patch: Record<string, unknown>): AltchaChallenge => {
    const c = structuredClone(fixture.challenge)
    Object.assign(c.parameters, patch)
    return c
  }
  const failing = {
    importKey: () => Promise.reject(new Error('must not be called')),
    deriveBits: () => Promise.reject(new Error('must not be called')),
  } as unknown as SubtleCrypto

  it('refuses an oversized cost before any key derivation', async () => {
    await expect(solveChallenge(withParams({ cost: 1_000_000_000 }), { subtle: failing })).rejects.toMatchObject({ reason: 'too_hard' })
    await expect(solveChallenge(withParams({ cost: MAX_COST + 1 }), { subtle: failing })).rejects.toMatchObject({ reason: 'too_hard' })
    expect(() => checkChallengeLimits(withParams({ cost: MAX_COST }))).not.toThrow()
  })

  it('refuses an oversized keyLength and a prefix longer than the key', async () => {
    await expect(solveChallenge(withParams({ keyLength: 1_000_000 }), { subtle: failing })).rejects.toMatchObject({ reason: 'too_hard' })
    await expect(solveChallenge(withParams({ keyLength: MAX_KEY_LENGTH + 1 }), { subtle: failing })).rejects.toMatchObject({ reason: 'too_hard' })
    await expect(solveChallenge(withParams({ keyLength: 1, keyPrefix: '000000' }), { subtle: failing })).rejects.toMatchObject({ reason: 'bad_challenge' })
  })

  it('refuses an expired challenge and malformed expiresAt', async () => {
    const past = Math.floor(Date.now() / 1000) - 1
    await expect(solveChallenge(withParams({ expiresAt: past }), { subtle: failing })).rejects.toMatchObject({ reason: 'expired' })
    expect(() => checkChallengeLimits(withParams({ expiresAt: 'soon' }))).toThrow(AltchaError)
    expect(() => checkChallengeLimits(withParams({ expiresAt: Math.floor(Date.now() / 1000) + 60 }))).not.toThrow()
  })
})
