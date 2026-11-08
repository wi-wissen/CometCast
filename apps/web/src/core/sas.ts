// SPDX-License-Identifier: GPL-3.0-or-later
//
// Connection check code (a short authentication string). Both sides hash the two DTLS
// certificate fingerprints of the final offer/answer and show the result; if a relay swapped the
// fingerprints during the key exchange, the two codes differ. It only helps when people really
// compare the codes (not enforced). A short code can in principle be ground by an active
// attacker who has time to try many certificates, so it is a check, not a proof.

/**
 * The `a=fingerprint:sha-256 XX:..` of an SDP, lowercase, or null. Lines end at LF (one CR before it
 * is part of the line end), as for the DTLS stack's SDP parser; an SDP with any other CR, U+2028 or
 * U+2029, or whose sha-256 fingerprint lines (any case, session and media level) disagree or are
 * malformed, gives null. So a relay cannot hide a second fingerprint that only this check would
 * read. Same rule as crates/cc-transport/src/sas.rs; vectors: tests/fixtures/sas.json.
 */
export function sdpFingerprint(sdp: string): string | null {
  if (/[\u2028\u2029]/.test(sdp)) return null
  let found: string | null = null
  for (const raw of sdp.split('\n')) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    if (line.includes('\r')) return null
    if (!line.startsWith('a=fingerprint:')) continue
    const rest = line.slice('a=fingerprint:'.length)
    const m = /^([^ \t]*)(.*)$/s.exec(rest)!
    if (m[1].toLowerCase() !== 'sha-256') continue
    const value = m[2].replace(/^[ \t]+|[ \t]+$/g, '')
    if (!/^[0-9A-Fa-f]{2}(?::[0-9A-Fa-f]{2}){31}$/.test(value)) return null
    const fp = value.toLowerCase()
    if (found !== null && found !== fp) return null
    found = fp
  }
  return found
}

const DOMAIN = 'cometcast/connection-check/v1'

/** 6 digits ("123 456") from the two fingerprints, independent of their order (sorted), or null. */
export async function connectionCode(
  localSdp: string,
  remoteSdp: string,
  subtle: SubtleCrypto | null = globalThis.crypto?.subtle ?? null,
): Promise<string | null> {
  const a = sdpFingerprint(localSdp)
  const b = sdpFingerprint(remoteSdp)
  if (!a || !b || !subtle) return null
  const [x, y] = [a, b].sort()
  const data = new TextEncoder().encode(`${DOMAIN}\n${x}\n${y}`)
  const h = new Uint8Array(await subtle.digest('SHA-256', data))
  const n = ((h[0] << 24) | (h[1] << 16) | (h[2] << 8) | h[3]) >>> 0
  const d = String(n % 1_000_000).padStart(6, '0')
  return `${d.slice(0, 3)} ${d.slice(3)}`
}
