// SPDX-License-Identifier: GPL-3.0-or-later
//
// Pixel size of a received image, read from its header without decoding it. The byte limit of a
// transfer (MAX_IMAGE_BYTES) says nothing about the decoded size: a few MiB of PNG can describe
// 30000 x 30000 pixels (3.6 GB once decoded), which freezes or crashes a display, above all a TV.
// So the display reads width and height first and refuses anything above MAX_IMAGE_PIXELS.

/**
 * Largest image the display decodes, in pixels. UNCERTAIN: 50 MP (an 8K frame is 33 MP) is a
 * placeholder; the memory of TV browsers is not measured (docs/measurement-points.md).
 */
export const MAX_IMAGE_PIXELS = 50_000_000
/** Largest width or height; browsers refuse or clip larger bitmaps anyway. */
export const MAX_IMAGE_SIDE = 16_384

export interface ImageSize {
  width: number
  height: number
}

const u16be = (b: Uint8Array, o: number) => (b[o] << 8) | b[o + 1]
const u16le = (b: Uint8Array, o: number) => b[o] | (b[o + 1] << 8)
const u24le = (b: Uint8Array, o: number) => b[o] | (b[o + 1] << 8) | (b[o + 2] << 16)
const u32be = (b: Uint8Array, o: number) => ((b[o] << 24) >>> 0) + ((b[o + 1] << 16) | (b[o + 2] << 8) | b[o + 3])
const ascii = (b: Uint8Array, o: number, s: string) => [...s].every((c, i) => b[o + i] === c.charCodeAt(0))

function png(b: Uint8Array): ImageSize | null {
  // Signature, then the IHDR chunk: length (4), "IHDR", width (4), height (4).
  if (b.length < 24 || !ascii(b, 1, 'PNG') || !ascii(b, 12, 'IHDR')) return null
  return { width: u32be(b, 16), height: u32be(b, 20) }
}

function gif(b: Uint8Array): ImageSize | null {
  // "GIF87a"/"GIF89a", then the logical screen size. Frames may not be larger than the screen
  // in a well-formed file; browsers clip larger ones to it.
  if (b.length < 10 || !ascii(b, 0, 'GIF8')) return null
  return { width: u16le(b, 6), height: u16le(b, 8) }
}

function jpeg(b: Uint8Array): ImageSize | null {
  if (b.length < 4 || b[0] !== 0xff || b[1] !== 0xd8) return null
  let o = 2
  while (o + 4 <= b.length) {
    if (b[o] !== 0xff) return null
    const marker = b[o + 1]
    if (marker === 0xff) {
      o++ // fill byte
      continue
    }
    // Markers without a length.
    if (marker === 0xd8 || marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) {
      o += 2
      continue
    }
    if (marker === 0xd9 || marker === 0xda) return null // end of image or start of scan before a frame header
    const len = u16be(b, o + 2)
    if (len < 2) return null
    // SOF0..SOF15 except DHT (C4), JPG (C8) and DAC (CC): precision (1), height (2), width (2).
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      if (o + 9 > b.length) return null
      return { width: u16be(b, o + 7), height: u16be(b, o + 5) }
    }
    o += 2 + len
  }
  return null
}

function webp(b: Uint8Array): ImageSize | null {
  if (b.length < 30 || !ascii(b, 0, 'RIFF') || !ascii(b, 8, 'WEBP')) return null
  if (ascii(b, 12, 'VP8X')) return { width: u24le(b, 24) + 1, height: u24le(b, 27) + 1 }
  if (ascii(b, 12, 'VP8L')) {
    if (b[20] !== 0x2f) return null
    const bits = (b[21] | (b[22] << 8) | (b[23] << 16) | (b[24] << 24)) >>> 0
    return { width: (bits & 0x3fff) + 1, height: ((bits >>> 14) & 0x3fff) + 1 }
  }
  if (ascii(b, 12, 'VP8 ')) {
    // Key frame start code 9d 01 2a, then 14-bit width and height (2 bits of scale each).
    if (b[23] !== 0x9d || b[24] !== 0x01 || b[25] !== 0x2a) return null
    return { width: u16le(b, 26) & 0x3fff, height: u16le(b, 28) & 0x3fff }
  }
  return null
}

/** Width and height from the header for the given MIME type, or null when the header cannot be read. */
export function imageSize(bytes: Uint8Array, mime: string): ImageSize | null {
  switch (mime) {
    case 'image/png':
      return png(bytes)
    case 'image/gif':
      return gif(bytes)
    case 'image/jpeg':
      return jpeg(bytes)
    case 'image/webp':
      return webp(bytes)
    default:
      return null
  }
}

/** 'ok', 'too_many_pixels' or 'unreadable' (no size in the header, or a zero side). */
export function checkImageSize(bytes: Uint8Array, mime: string): 'ok' | 'too_many_pixels' | 'unreadable' {
  const s = imageSize(bytes, mime)
  if (!s || s.width < 1 || s.height < 1) return 'unreadable'
  if (s.width > MAX_IMAGE_SIDE || s.height > MAX_IMAGE_SIDE || s.width * s.height > MAX_IMAGE_PIXELS) return 'too_many_pixels'
  return 'ok'
}
