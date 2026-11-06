// SPDX-License-Identifier: GPL-3.0-or-later
//
// ALTCHA proof-of-work ("PoW v2", as implemented by the Rust `altcha` crate 1.1 that the
// server uses; crates/cc-server/src/puzzle.rs and docs/server.md).
//
// Scheme, read from the crate source (altcha-1.1.0 src/pow.rs, helpers.rs, algorithms/pbkdf2.rs):
//   password   = nonce (hex-decoded bytes) || counter as 4 bytes big-endian
//   derivedKey = PBKDF2-HMAC-<hash>(password, salt (hex-decoded), cost iterations, keyLength bytes)
//   solved     = hex(derivedKey) starts with keyPrefix (hex, case-insensitive)
// The server's puzzles are "deterministic": keyPrefix is the first keyLength/2 bytes of the key of
// a secret counter, so the search from 0 upwards ends exactly at that counter.
// The answer for create_room is base64(JSON {challenge, solution:{counter, derivedKey, time}}).
//
// This file is pure (no DOM, no Worker) so it runs in a Web Worker, on the main thread and in tests.

export interface AltchaParameters {
  algorithm: string
  cost: number
  keyLength: number
  keyPrefix: string
  nonce: string
  salt: string
  [extra: string]: unknown
}

/** The JSON object GET /altcha/challenge returns. It is sent back unchanged inside the payload. */
export interface AltchaChallenge {
  parameters: AltchaParameters
  signature?: string
}

export interface AltchaSolution {
  counter: number
  derivedKey: string
  /** Milliseconds the search took (the server accepts and ignores it). */
  time: number
}

export class AltchaError extends Error {
  constructor(
    /** 'unsupported_algorithm' | 'bad_challenge' | 'too_hard' | 'expired' | 'timeout' | 'aborted' */
    readonly reason: string,
    message?: string,
  ) {
    super(message ?? reason)
    this.name = 'AltchaError'
  }
}

/** UNCERTAIN: 60 s is a placeholder (same as the native client); solving time on weak devices is not measured. */
export const DEFAULT_SOLVE_TIMEOUT_MS = 60_000

/**
 * Largest PBKDF2 cost (iterations per attempt) we solve. The server default is 1000
 * (`altcha.cost`, crates/cc-server/src/config.rs); the server has no maximum for the cost.
 * UNCERTAIN: 10 times the default is a placeholder (same as the native client). One attempt is
 * a single uninterruptible deriveBits call, so this also bounds how long the main-thread
 * fallback can block the page per attempt.
 */
export const MAX_COST = 10_000

/** Largest derived key length in bytes. UNCERTAIN: 64 is a placeholder (same as the native client). */
export const MAX_KEY_LENGTH = 64

/**
 * Refuses a puzzle that is too expensive, oddly sized or already expired, before any work is done.
 * `nowMs` is a test seam. `expiresAt` is in seconds, like the server's.
 */
export function checkChallengeLimits(challenge: AltchaChallenge, nowMs: number = Date.now()): void {
  const p = challenge?.parameters
  if (!p) throw new AltchaError('bad_challenge')
  if (!Number.isInteger(p.cost) || p.cost < 1 || !Number.isInteger(p.keyLength) || p.keyLength < 1)
    throw new AltchaError('bad_challenge')
  if (p.cost > MAX_COST) throw new AltchaError('too_hard', `cost ${p.cost} is above ${MAX_COST}`)
  if (p.keyLength > MAX_KEY_LENGTH) throw new AltchaError('too_hard', `keyLength ${p.keyLength} is above ${MAX_KEY_LENGTH}`)
  if (typeof p.keyPrefix === 'string' && p.keyPrefix.length > p.keyLength * 2) throw new AltchaError('bad_challenge')
  const exp = p.expiresAt
  if (exp !== undefined && exp !== null) {
    if (typeof exp !== 'number' || !Number.isFinite(exp)) throw new AltchaError('bad_challenge')
    if (exp * 1000 <= nowMs) throw new AltchaError('expired')
  }
}

const HASHES: Record<string, 'SHA-256' | 'SHA-384' | 'SHA-512'> = {
  'PBKDF2/SHA-256': 'SHA-256',
  'PBKDF2/SHA-384': 'SHA-384',
  'PBKDF2/SHA-512': 'SHA-512',
}

export function hexToBytes(hex: string): Uint8Array<ArrayBuffer> {
  if (hex.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(hex)) throw new AltchaError('bad_challenge', 'not a hex string')
  const out = new Uint8Array(hex.length / 2)
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16)
  return out
}

export function bytesToHex(bytes: Uint8Array): string {
  let s = ''
  for (const b of bytes) s += b.toString(16).padStart(2, '0')
  return s
}

/** nonce || counter (uint32, big-endian). */
export function buildPassword(nonce: Uint8Array, counter: number): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(nonce.length + 4)
  out.set(nonce)
  new DataView(out.buffer).setUint32(nonce.length, counter >>> 0, false)
  return out
}

export interface SolveOptions {
  timeoutMs?: number
  signal?: AbortSignal
  /** Test seam; defaults to crypto.subtle. */
  subtle?: SubtleCrypto
  /** Called now and then with the number of attempts made (for progress, optional). */
  onProgress?: (attempts: number) => void
}

/**
 * Searches the counter, starting at 0, whose derived key starts with the challenge's key prefix.
 * Each attempt is one WebCrypto PBKDF2 derivation of `cost` iterations.
 */
export async function solveChallenge(challenge: AltchaChallenge, opts: SolveOptions = {}): Promise<AltchaSolution> {
  const p = challenge?.parameters
  if (!p || typeof p.algorithm !== 'string' || typeof p.nonce !== 'string' || typeof p.salt !== 'string')
    throw new AltchaError('bad_challenge')
  checkChallengeLimits(challenge)
  const hash = HASHES[p.algorithm.toUpperCase()]
  if (!hash) throw new AltchaError('unsupported_algorithm', p.algorithm)
  if (typeof p.keyPrefix !== 'string' || !/^[0-9a-fA-F]*$/.test(p.keyPrefix)) throw new AltchaError('bad_challenge')
  const subtle = opts.subtle ?? globalThis.crypto?.subtle
  if (!subtle) throw new AltchaError('unsupported_algorithm', 'WebCrypto is not available')

  const nonce = hexToBytes(p.nonce)
  const salt = hexToBytes(p.salt)
  const prefix = p.keyPrefix.toLowerCase()
  const timeoutMs = opts.timeoutMs ?? DEFAULT_SOLVE_TIMEOUT_MS
  const start = performance.now()

  for (let counter = 0; counter <= 0xffffffff; counter++) {
    if (opts.signal?.aborted) throw new AltchaError('aborted')
    if (timeoutMs > 0 && performance.now() - start > timeoutMs) throw new AltchaError('timeout')
    const key = await subtle.importKey('raw', buildPassword(nonce, counter), 'PBKDF2', false, ['deriveBits'])
    const bits = await subtle.deriveBits({ name: 'PBKDF2', hash, salt, iterations: p.cost }, key, p.keyLength * 8)
    const derived = bytesToHex(new Uint8Array(bits))
    if (derived.startsWith(prefix)) {
      return { counter, derivedKey: derived, time: Math.round(performance.now() - start) }
    }
    if (opts.onProgress && counter % 256 === 255) opts.onProgress(counter + 1)
  }
  throw new AltchaError('timeout')
}

/** base64 of the JSON `{challenge, solution}`; the value of create_room.altcha. */
export function encodePayload(challenge: AltchaChallenge, solution: AltchaSolution): string {
  const json = JSON.stringify({ challenge, solution })
  // The JSON is ASCII (hex and numbers), but encode as UTF-8 anyway before btoa.
  const bytes = new TextEncoder().encode(json)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  return btoa(bin)
}

/** Minimal shape check of the server's answer. */
export function parseChallenge(v: unknown): AltchaChallenge {
  if (v === null || typeof v !== 'object') throw new AltchaError('bad_challenge')
  const o = v as { parameters?: unknown; signature?: unknown }
  if (o.parameters === null || typeof o.parameters !== 'object') throw new AltchaError('bad_challenge')
  return v as AltchaChallenge
}
