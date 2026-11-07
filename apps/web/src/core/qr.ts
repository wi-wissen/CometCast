// SPDX-License-Identifier: GPL-3.0-or-later
//
// QR code for the idle screen (4.2): encodes `https://{dom}/{CODE}` (code without spaces), error
// correction level M. The encoder is the bundled npm package qrcode-generator (MIT, pinned in
// package.json); nothing is loaded from the network.
import qrcode from 'qrcode-generator'

export interface QrSvg {
  /** Side length in modules (without quiet margin). */
  size: number
  /** SVG path data: one 1x1 square per dark module. */
  path: string
}

export function qrSvg(text: string): QrSvg {
  const qr = qrcode(0, 'M')
  qr.addData(text, 'Byte')
  qr.make()
  const n = qr.getModuleCount()
  let d = ''
  for (let y = 0; y < n; y++) {
    let x = 0
    while (x < n) {
      if (!qr.isDark(y, x)) {
        x++
        continue
      }
      let run = 1
      while (x + run < n && qr.isDark(y, x + run)) run++
      d += `M${x} ${y}h${run}v1h-${run}z`
      x += run
    }
  }
  return { size: n, path: d }
}

/** The URL inside the QR code: https://{dom}/{CODE}; `dom` may carry a port. */
export function joinUrl(dom: string, code: string, scheme: 'https' | 'http' = 'https'): string {
  return `${scheme}://${dom}/${code.replace(/\s+/g, '')}`
}
