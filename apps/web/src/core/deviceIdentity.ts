// SPDX-License-Identifier: GPL-3.0-or-later
//
// Presenter device identity (docs/design/presenter-identity.md).
//
// A presenter installation has an Ed25519 key pair. With `join_room` it sends a proof: the public key, a time
// stamp and the signature over `cometcast-join-v1` 0x00 <room code, upper case> 0x00 <ts, decimal>. The name is
// not signed. The display checks the proof (this file does it with WebCrypto too) and derives the device id: the
// lower-case hex of the first 16 bytes of SHA-256(public key).
//
// Web presenter: the key pair is made with WebCrypto `Ed25519`, NON-EXTRACTABLE, and kept as a CryptoKeyPair in
// IndexedDB. Where WebCrypto has no Ed25519, or IndexedDB is missing or throws (private windows, blocked site
// data), no proof is sent and the join works as before. UNCERTAIN: support (Chrome 137+, Firefox 129+, Safari 17+)
// is to be checked in the e2e browsers (see docs/measurement-points.md, A25).
//
// Vectors shared with Rust and Swift: tests/fixtures/join-proof.json.

/** The `device` object of `join_room` / `join_request` / `peer_joined`. */
export interface JoinDevice {
  pk: string
  ts: number
  sig: string
}

export const JOIN_DOMAIN = 'cometcast-join-v1'
/** How far `ts` may be from the display's clock (either direction), in milliseconds. */
export const JOIN_WINDOW_MS = 120_000
const PK_CHARS = 43
const SIG_CHARS = 86
const DB_NAME = 'cometcast-device'
const STORE = 'keys'
const KEY_NAME = 'presenter'

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'

export function toBase64Url(bytes: Uint8Array): string {
  let out = ''
  for (let i = 0; i < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0)
    out += B64[(n >> 18) & 63] + B64[(n >> 12) & 63]
    if (i + 1 < bytes.length) out += B64[(n >> 6) & 63]
    if (i + 2 < bytes.length) out += B64[n & 63]
  }
  return out
}

/** Decodes canonical unpadded base64url of exactly `bytes` bytes; null for anything else (also stray bits). */
export function fromBase64Url(text: string, bytes: number): Uint8Array | null {
  const chars = Math.ceil((bytes * 4) / 3)
  if (typeof text !== 'string' || text.length !== chars) return null
  const out = new Uint8Array(bytes)
  let acc = 0
  let bits = 0
  let o = 0
  for (const ch of text) {
    const v = B64.indexOf(ch)
    if (v < 0) return null
    acc = (acc << 6) | v
    bits += 6
    if (bits >= 8) {
      bits -= 8
      if (o < bytes) out[o++] = (acc >> bits) & 255
      acc &= (1 << bits) - 1
    }
  }
  // The leftover bits of the last character must be zero (canonical spelling).
  return acc === 0 && o === bytes ? out : null
}

/** The shape the server checks: 43 / 86 canonical base64url characters, `ts` a non-negative integer, nothing else. */
export function isWellFormedDevice(d: unknown): d is JoinDevice {
  if (d === null || typeof d !== 'object' || Array.isArray(d)) return false
  const o = d as Record<string, unknown>
  const keys = Object.keys(o)
  if (keys.length !== 3 || !('pk' in o) || !('ts' in o) || !('sig' in o)) return false
  return (
    typeof o.pk === 'string' &&
    typeof o.sig === 'string' &&
    typeof o.ts === 'number' &&
    Number.isSafeInteger(o.ts) &&
    o.ts >= 0 &&
    fromBase64Url(o.pk, 32) !== null &&
    fromBase64Url(o.sig, 64) !== null &&
    o.pk.length === PK_CHARS &&
    o.sig.length === SIG_CHARS
  )
}

/** The bytes that are signed: domain, 0x00, code in upper case, 0x00, ts in decimal. */
export function joinSignedBytes(code: string, ts: number): Uint8Array {
  return new TextEncoder().encode(`${JOIN_DOMAIN}\0${code.toUpperCase()}\0${ts}`)
}

function subtle(): SubtleCrypto | null {
  return globalThis.crypto?.subtle ?? null
}

/** The device id of a public key: lower-case hex of the first 16 bytes of SHA-256. */
export async function deviceIdOf(pk: Uint8Array): Promise<string> {
  const s = subtle()
  if (!s) throw new Error('no WebCrypto')
  const digest = new Uint8Array(await s.digest('SHA-256', pk as BufferSource)).slice(0, 16)
  return Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('')
}

// ---- presenter: the key pair ------------------------------------------------------------------------------------

/** Where the key pair is kept. The default is IndexedDB; tests give their own. */
export interface KeyStore {
  get(): Promise<CryptoKeyPair | undefined>
  put(pair: CryptoKeyPair): Promise<void>
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1)
    req.onupgradeneeded = () => req.result.createObjectStore(STORE)
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
    req.onblocked = () => reject(new Error('blocked'))
  })
}

/** The key pair in IndexedDB (a CryptoKeyPair is stored by structured clone; a non-extractable key stays non-extractable). */
export const indexedDbKeyStore: KeyStore = {
  async get() {
    const db = await openDb()
    try {
      return await new Promise<CryptoKeyPair | undefined>((resolve, reject) => {
        const req = db.transaction(STORE, 'readonly').objectStore(STORE).get(KEY_NAME)
        req.onsuccess = () => resolve(req.result as CryptoKeyPair | undefined)
        req.onerror = () => reject(req.error)
      })
    } finally {
      db.close()
    }
  },
  async put(pair) {
    const db = await openDb()
    try {
      await new Promise<void>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readwrite')
        tx.objectStore(STORE).put(pair, KEY_NAME)
        tx.oncomplete = () => resolve()
        tx.onerror = () => reject(tx.error)
        tx.onabort = () => reject(tx.error)
      })
    } finally {
      db.close()
    }
  },
}

let cached: Promise<CryptoKeyPair | null> | null = null

/**
 * The presenter key pair: read from the store, or made on first use (`generateKey` with `extractable: false`) and stored.
 * `null` when WebCrypto has no Ed25519, the store cannot be read or written, or the key cannot be kept (a key that
 * would be new on every visit is no identity). Never throws.
 */
export function loadOrCreateKeyPair(store: KeyStore = indexedDbKeyStore): Promise<CryptoKeyPair | null> {
  const run = async (): Promise<CryptoKeyPair | null> => {
    try {
      const s = subtle()
      if (!s) return null
      const existing = await store.get()
      if (existing?.privateKey && existing.publicKey) return existing
      const pair = (await s.generateKey({ name: 'Ed25519' }, false, ['sign'])) as CryptoKeyPair
      await store.put(pair)
      return pair
    } catch {
      return null
    }
  }
  if (store !== indexedDbKeyStore) return run()
  cached ??= run()
  return cached
}

/** Forgets the cached default key pair (tests). */
export function resetKeyCache(): void {
  cached = null
}

/** The proof of this installation for joining `code` at `now`; `null` when no key is available. Never throws. */
export async function makeJoinProof(code: string, now: number = Date.now(), store?: KeyStore): Promise<JoinDevice | null> {
  try {
    const pair = await loadOrCreateKeyPair(store)
    const s = subtle()
    if (!pair || !s) return null
    const pk = new Uint8Array(await s.exportKey('raw', pair.publicKey))
    const sig = new Uint8Array(await s.sign({ name: 'Ed25519' }, pair.privateKey, joinSignedBytes(code, now) as BufferSource))
    if (pk.length !== 32 || sig.length !== 64) return null
    return { pk: toBase64Url(pk), ts: now, sig: toBase64Url(sig) }
  } catch {
    return null
  }
}

// ---- display: verify ---------------------------------------------------------------------------------------------

/** Order of the Ed25519 base point group. */
const L = (1n << 252n) + 27742317777372353535851937790883648493n

/** The encodings of the points of small order (eight points of the curve, and their non-canonical spellings). */
const SMALL_ORDER_HEX = [
  '0100000000000000000000000000000000000000000000000000000000000000',
  'ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  '0000000000000000000000000000000000000000000000000000000000000080',
  '0000000000000000000000000000000000000000000000000000000000000000',
  'c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a',
  'c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac03fa',
  '26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05',
  '26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc85',
  'edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  'eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  'edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
  'ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
  'eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
]

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')
}

/**
 * Checks a proof for room `code` against the clock `now`: shape, `|ts - now| <= 120 s`, and the signature. Returns the
 * device id, or `null` for every kind of failure (the request is then shown without any device note).
 *
 * WebCrypto's `verify` is not guaranteed to be strict like Rust's `verify_strict`, so the two strict rules are checked
 * here too: a public key of small order is refused, and so is a signature whose S is not reduced (S >= L).
 */
export async function verifyJoinProof(device: unknown, code: string, now: number = Date.now()): Promise<string | null> {
  try {
    const s = subtle()
    if (!s || !isWellFormedDevice(device)) return null
    if (Math.abs(device.ts - now) > JOIN_WINDOW_MS) return null
    const pk = fromBase64Url(device.pk, 32)
    const sig = fromBase64Url(device.sig, 64)
    if (!pk || !sig) return null
    if (SMALL_ORDER_HEX.includes(hex(pk))) return null
    let sInt = 0n
    for (let i = 63; i >= 32; i--) sInt = (sInt << 8n) | BigInt(sig[i])
    if (sInt >= L) return null
    const key = await s.importKey('raw', pk as BufferSource, { name: 'Ed25519' }, false, ['verify'])
    const ok = await s.verify({ name: 'Ed25519' }, key, sig as BufferSource, joinSignedBytes(code, device.ts) as BufferSource)
    return ok ? await deviceIdOf(pk) : null
  } catch {
    return null
  }
}
