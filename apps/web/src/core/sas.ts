// SPDX-License-Identifier: GPL-3.0-or-later
//
// Connection check code (a short authentication string). Both sides hash the two DTLS
// certificate fingerprints of the final offer/answer and show the result; if a relay swapped the
// fingerprints during the key exchange, the two codes differ. It only helps when people really
// compare the codes (not enforced). A short code can in principle be ground by an active
// attacker who has time to try many certificates, so it is a check, not a proof.

/** First `a=fingerprint:sha-256 XX:..` of an SDP, normalised to lowercase hex, or null. */
export function sdpFingerprint(sdp: string): string | null {
  const m = /^a=fingerprint:sha-256[ \t]+([0-9A-Fa-f]{2}(?::[0-9A-Fa-f]{2}){31})[ \t]*\r?$/m.exec(sdp)
  return m ? m[1].toLowerCase() : null
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
