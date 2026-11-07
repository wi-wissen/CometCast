// SPDX-License-Identifier: GPL-3.0-or-later
//
// The page is chosen by its path (docs/design/first-run-and-display.md, 5.1 and 7.2):
//   {dom}/display     the display (receiver)
//   {dom}/{CODE}      the presenter's start page with the code filled in (QR code, camera chip)
//   {dom}/app and any other path or none: the presenter's start page
// The web server answers these paths with index.html (see crates/cc-server and docs/web-m2.md).
import { normalizeRoomCode, isValidRoomCode } from './roomCode'

export type Route = { name: 'display' } | { name: 'start'; code: string | null }

/** `base` is Vite's BASE_URL (for example "/" or "/app/"). */
export function parseRoute(pathname: string, base = '/'): Route {
  let p = pathname
  if (base !== '/' && p.startsWith(base)) p = p.slice(base.length)
  else if (base !== '/' && p === base.replace(/\/$/, '')) p = ''
  p = p.replace(/^\/+|\/+$/g, '')
  if (p.toLowerCase() === 'display') return { name: 'display' }
  if (p && !p.includes('/')) {
    let decoded = p
    try {
      decoded = decodeURIComponent(p)
    } catch {
      /* keep the raw segment */
    }
    const code = normalizeRoomCode(decoded)
    if (isValidRoomCode(code) && decoded.replace(/[\s-]/g, '').length === code.length) return { name: 'start', code }
  }
  return { name: 'start', code: null }
}
