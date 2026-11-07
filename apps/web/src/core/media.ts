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

/** Maps `MediaError.code` (1..4) to a translated notice key; never contains URL details. */
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
