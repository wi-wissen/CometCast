// SPDX-License-Identifier: GPL-3.0-or-later
//
// Media links: the receiver classifies a received `link` by the extension of the URL path. There is no
// wire message for this; the sender sends a normal `link`. Nothing is played until the person at the
// receiver clicks "Play" (see ContentOverlay / MediaPlayer).

export type MediaKind = 'video' | 'audio' | 'hls'

const EXTENSIONS: Readonly<Record<string, MediaKind>> = {
  mp4: 'video',
  webm: 'video',
  ogv: 'video',
  m3u8: 'hls',
  mp3: 'audio',
  ogg: 'audio',
  wav: 'audio',
  m4a: 'audio',
}

/** `video`, `audio` or `hls` from the path extension (query and fragment ignored, case-insensitive); null otherwise. */
export function classifyMediaUrl(url: string): MediaKind | null {
  let u: URL
  try {
    u = new URL(url)
  } catch {
    return null
  }
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return null
  const last = u.pathname.split('/').pop() ?? ''
  const dot = last.lastIndexOf('.')
  if (dot < 0) return null
  return EXTENSIONS[last.slice(dot + 1).toLowerCase()] ?? null
}

/** Whether this browser can play HLS natively (Safari yes, Chromium no). hls.js is deliberately not bundled. */
export function canPlayHlsNatively(probe: Pick<HTMLMediaElement, 'canPlayType'> | null = typeof document === 'undefined' ? null : document.createElement('video')): boolean {
  if (!probe) return false
  return probe.canPlayType('application/vnd.apple.mpegurl') !== '' || probe.canPlayType('application/x-mpegURL') !== ''
}

export type MediaErrorCode = 'aborted' | 'network' | 'decode' | 'unsupported' | 'blocked'

/**
 * Maps `MediaError.code` (1..4) to a translated notice key; never contains URL details.
 * UNCERTAIN: a media element does not tell a CORS failure, an HTTP error (404) or a bad format apart: Chrome
 * for Testing 153 reports all of them as code 4 (SRC_NOT_SUPPORTED) on a `crossorigin=anonymous` element (observed in the
 * e2e for a missing file and for a server without CORS; other browsers not tested). So `unsupported` is a
 * combined notice and there is no separate CORS notice. `blocked` is not a MediaError code: the player produces
 * it when `play()` is rejected with NotAllowedError (autoplay policy).
 */
export function mediaErrorCode(code: number | undefined): MediaErrorCode {
  switch (code) {
    case 1:
      return 'aborted'
    case 2:
      return 'network'
    case 3:
      return 'decode'
    case 4:
      return 'unsupported'
    default:
      return 'network'
  }
}

/** Player state. `card`: nothing loaded, the element does not exist. */
export type PlayerState = { phase: 'card' } | { phase: 'loading' } | { phase: 'playing' } | { phase: 'error'; code: MediaErrorCode }
export type PlayerEvent = { type: 'play' } | { type: 'playing' } | { type: 'error'; code: MediaErrorCode } | { type: 'stop' }

export const initialPlayer: PlayerState = { phase: 'card' }

/** Pure transitions. Only a deliberate `play` leaves `card`; `stop` always returns to it (element released). */
export function playerReduce(s: PlayerState, e: PlayerEvent): PlayerState {
  switch (e.type) {
    case 'play':
      return s.phase === 'card' || s.phase === 'error' ? { phase: 'loading' } : s
    case 'playing':
      return s.phase === 'loading' ? { phase: 'playing' } : s
    case 'error':
      return s.phase === 'card' ? s : { phase: 'error', code: e.code }
    case 'stop':
      return initialPlayer
  }
}

/** Longest host shown on a button or in an aria-label; the full host stays in the card's own text. */
export const MAX_SHORT_HOST = 40

/** A long host for a button or label: first 24 and last 12 characters around an ellipsis. */
export function shortHost(host: string): string {
  return host.length <= MAX_SHORT_HOST ? host : `${host.slice(0, 24)}\u2026${host.slice(-12)}`
}

/** Loading timeout of the player (until the first frame or sound can play). UNCERTAIN: 20 s is a guess. */
export const MEDIA_LOAD_TIMEOUT_MS = 20_000

/**
 * Hosts that point into the receiver's own network or at a bare address: IP literals (v4 and v6), `localhost`,
 * single-label names and the usual private suffixes. Name-based only: a public name that resolves to a private
 * address (DNS rebinding style) is NOT detected, and neither is a redirect.
 * `host` is `URL.hostname` (v6 literals in brackets).
 */
export function hostIsLocalOrIp(hostname: string): boolean {
  const h = hostname.toLowerCase().replace(/\.$/, '')
  if (h === '') return false
  if (h.startsWith('[') || /^\d+\.\d+\.\d+\.\d+$/.test(h)) return true
  if (!h.includes('.')) return true
  return /\.(local|localhost|localdomain|lan|home|internal|intranet|corp|private)$/.test(h) || h.endsWith('.home.arpa')
}

/** Whether the link points at an IP literal or a private-looking name (see `hostIsLocalOrIp`). */
export function urlHostIsLocalOrIp(url: string): boolean {
  try {
    return hostIsLocalOrIp(new URL(url).hostname)
  } catch {
    return false
  }
}

/** An http: media URL on an https: page would be blocked as mixed content, so no Play is offered for it. */
export function isMixedContent(pageProtocol: string, url: string): boolean {
  try {
    return pageProtocol === 'https:' && new URL(url).protocol === 'http:'
  } catch {
    return false
  }
}
