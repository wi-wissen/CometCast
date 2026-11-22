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
 * U+2029, or whose fingerprint lines (session and media level) are not all sha-256 (any case), disagree
 * or are malformed, gives null; so does any other line that holds the word "fingerprint" (any case),
 * such as `a= fingerprint:..`, which a lenient SDP parser reads as a fingerprint. So a relay cannot
 * show one fingerprint to this check and another one to DTLS. Same rule as crates/cc-transport/src/sas.rs; vectors: tests/fixtures/sas.json.
 */
export function sdpFingerprint(sdp: string): string | null {
  if (/[\u2028\u2029]/.test(sdp)) return null
  let found: string | null = null
  for (const raw of sdp.split('\n')) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    if (line.includes('\r')) return null
    if (!line.startsWith('a=fingerprint:')) {
      // `a= fingerprint:..` and the like: a fingerprint for a lenient SDP parser, so never skipped.
      if (/fingerprint/i.test(line)) return null
      continue
    }
    const rest = line.slice('a=fingerprint:'.length)
    const m = /^([^ \t]*)(.*)$/s.exec(rest)!
    // Another algorithm is refused, not skipped: a DTLS stack may take the first fingerprint line.
    if (m[1].toLowerCase() !== 'sha-256') return null
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
