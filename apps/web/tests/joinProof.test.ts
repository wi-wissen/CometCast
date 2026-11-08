// SPDX-License-Identifier: GPL-3.0-or-later
//
// Presenter device proof (docs/design/presenter-identity.md) against the shared vectors
// tests/fixtures/join-proof.json (made by tools/gen-join-proof-vectors.py with an independent implementation; the Rust
// and Swift tests read the same file). WebCrypto is Node's `crypto.webcrypto`, which supports Ed25519.
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  deviceIdOf,
  fromBase64Url,
  isWellFormedDevice,
  joinSignedBytes,
  loadOrCreateKeyPair,
  makeJoinProof,
  toBase64Url,
  verifyJoinProof,
  type KeyStore,
} from '../src/core/deviceIdentity'
import vectors from '../../../tests/fixtures/join-proof.json'

const fromHex = (h: string) => Uint8Array.from(h.match(/../g) ?? [], (b) => parseInt(b, 16))
const toHex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('')

beforeEach(() => vi.stubGlobal('crypto', webcrypto))
afterEach(() => vi.unstubAllGlobals())

class MemoryStore implements KeyStore {
  pair: CryptoKeyPair | undefined
  async get() {
    return this.pair
  }
  async put(p: CryptoKeyPair) {
    this.pair = p
  }
}

/** A private key from its 32-byte seed: PKCS#8 wrapper of an Ed25519 key. */
async function keyFromSeed(seed: Uint8Array): Promise<CryptoKey> {
  const prefix = fromHex('302e020100300506032b657004220420')
  const der = new Uint8Array(prefix.length + 32)
  der.set(prefix)
  der.set(seed, prefix.length)
  return webcrypto.subtle.importKey('pkcs8', der, { name: 'Ed25519' }, false, ['sign'])
}

describe('shared vectors', () => {
  it('window and domain are the ones of the spec', () => {
    expect(vectors.window_ms).toBe(120000)
    expect(vectors.domain).toBe('cometcast-join-v1')
  })

  it('device ids, signed bytes and signatures match the independent implementation', async () => {
    for (const k of vectors.keys) {
      expect(await deviceIdOf(fromHex(k.public_key_hex))).toBe(k.id)
      expect(toBase64Url(fromHex(k.public_key_hex))).toBe(k.pk)
    }
    for (const v of vectors.valid) {
      expect(toHex(joinSignedBytes(v.signed_for_code, v.device.ts)), v.name).toBe(v.signed_bytes_hex)
      const seed = fromHex(vectors.keys.find((k) => k.name === v.key)!.seed_hex)
      const sig = new Uint8Array(await webcrypto.subtle.sign({ name: 'Ed25519' }, await keyFromSeed(seed), joinSignedBytes(v.signed_for_code, v.device.ts) as BufferSource))
      expect(toBase64Url(sig), v.name).toBe(v.device.sig)
    }
  })

  it('valid proofs verify to their device id', async () => {
    for (const v of vectors.valid) expect(await verifyJoinProof(v.device, v.code, v.now_ms), v.name).toBe(v.device_id)
  })

  it('refused proofs are refused', async () => {
    for (const r of vectors.refused) expect(await verifyJoinProof(r.device, r.code, r.now_ms), r.name).toBeNull()
  })
})

describe('encoding', () => {
  it('base64url round-trips and refuses non-canonical spellings', () => {
    const bytes = Uint8Array.from({ length: 32 }, (_, i) => i * 7)
    expect(fromBase64Url(toBase64Url(bytes), 32)).toEqual(bytes)
    expect(toBase64Url(bytes)).toHaveLength(43)
    expect(fromBase64Url(toBase64Url(bytes).slice(0, 42), 32)).toBeNull()
    expect(fromBase64Url(toBase64Url(bytes).slice(0, 42) + '=', 32)).toBeNull()
    expect(fromBase64Url(toBase64Url(bytes).slice(0, 42) + '+', 32)).toBeNull()
  })

  it('the shape check wants exactly pk, ts and sig', () => {
    const d = vectors.valid[0].device
    expect(isWellFormedDevice(d)).toBe(true)
    expect(isWellFormedDevice({ ...d, name: 'x' })).toBe(false)
    expect(isWellFormedDevice({ ...d, ts: '5' })).toBe(false)
    expect(isWellFormedDevice({ ...d, ts: -1 })).toBe(false)
    expect(isWellFormedDevice(null)).toBe(false)
    expect(isWellFormedDevice([])).toBe(false)
  })
})

describe('the presenter key', () => {
  it('is made once, non-extractable, kept in the store and signs a proof that verifies', async () => {
    const store = new MemoryStore()
    const proof = await makeJoinProof('k7qm2w', 1_700_000_000_000, store)
    expect(proof).not.toBeNull()
    expect(store.pair?.privateKey.extractable).toBe(false)
    expect(proof!.ts).toBe(1_700_000_000_000)
    const id = await verifyJoinProof(proof, 'K7QM2W', 1_700_000_000_000)
    expect(id).toMatch(/^[0-9a-f]{32}$/)
    // The same key the next time: the same device id.
    const again = await makeJoinProof('P4X9TR', 1_700_000_050_000, store)
    expect(await verifyJoinProof(again, 'P4X9TR', 1_700_000_050_000)).toBe(id)
    // The proof is for this room only.
    expect(await verifyJoinProof(proof, 'P4X9TR', 1_700_000_000_000)).toBeNull()
    expect(await verifyJoinProof(proof, 'K7QM2W', 1_700_000_000_000 + 121_000)).toBeNull()
  })

  it('sends no proof where WebCrypto has no Ed25519', async () => {
    const subtle = {
      generateKey: () => Promise.reject(new DOMException('not supported', 'NotSupportedError')),
    }
    vi.stubGlobal('crypto', { subtle })
    expect(await makeJoinProof('K7QM2W', 1, new MemoryStore())).toBeNull()
    vi.stubGlobal('crypto', undefined)
    expect(await makeJoinProof('K7QM2W', 1, new MemoryStore())).toBeNull()
  })

  it('sends no proof when the store cannot be read or written', async () => {
    const broken: KeyStore = {
      get: () => Promise.reject(new Error('blocked')),
      put: () => Promise.reject(new Error('blocked')),
    }
    expect(await loadOrCreateKeyPair(broken)).toBeNull()
    const readonly: KeyStore = { get: async () => undefined, put: () => Promise.reject(new Error('quota')) }
    expect(await makeJoinProof('K7QM2W', 1, readonly)).toBeNull()
  })
})
