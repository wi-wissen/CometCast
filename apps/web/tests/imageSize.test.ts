// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { checkImageSize, imageSize, MAX_IMAGE_PIXELS } from '../src/core/imageSize'

const be32 = (n: number) => [(n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255]
const le16 = (n: number) => [n & 255, (n >>> 8) & 255]
const le24 = (n: number) => [n & 255, (n >>> 8) & 255, (n >>> 16) & 255]
const str = (s: string) => [...s].map((c) => c.charCodeAt(0))

const png = (w: number, h: number) => new Uint8Array([0x89, ...str('PNG'), 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, ...str('IHDR'), ...be32(w), ...be32(h), 8, 6, 0, 0, 0])
const gif = (w: number, h: number) => new Uint8Array([...str('GIF89a'), ...le16(w), ...le16(h), 0, 0, 0])
const jpeg = (w: number, h: number) =>
  new Uint8Array([
    0xff, 0xd8,
    0xff, 0xe0, 0, 16, ...str('JFIF'), 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, // APP0
    0xff, 0xdb, 0, 3, 0, // a short DQT-like segment to skip
    0xff, 0xc2, 0, 11, 8, (h >> 8) & 255, h & 255, (w >> 8) & 255, w & 255, 1, 1, 0x11, 0, // SOF2
    0xff, 0xda,
  ])
const riff = (chunk: number[]) => new Uint8Array([...str('RIFF'), 0, 0, 0, 0, ...str('WEBP'), ...chunk, ...new Array(16).fill(0)])
const webpX = (w: number, h: number) => riff([...str('VP8X'), 10, 0, 0, 0, 0, 0, 0, 0, ...le24(w - 1), ...le24(h - 1)])
const webpL = (w: number, h: number) => {
  const bits = ((w - 1) & 0x3fff) | (((h - 1) & 0x3fff) << 14)
  return riff([...str('VP8L'), 5, 0, 0, 0, 0x2f, bits & 255, (bits >>> 8) & 255, (bits >>> 16) & 255, (bits >>> 24) & 255])
}
const webpLossy = (w: number, h: number) => riff([...str('VP8 '), 10, 0, 0, 0, 0, 0, 0, 0x9d, 0x01, 0x2a, ...le16(w), ...le16(h)])

describe('image size from the header', () => {
  it('reads PNG, GIF, JPEG and the three WebP kinds', () => {
    expect(imageSize(png(640, 480), 'image/png')).toEqual({ width: 640, height: 480 })
    expect(imageSize(gif(320, 200), 'image/gif')).toEqual({ width: 320, height: 200 })
    expect(imageSize(jpeg(1920, 1080), 'image/jpeg')).toEqual({ width: 1920, height: 1080 })
    expect(imageSize(webpX(4000, 3000), 'image/webp')).toEqual({ width: 4000, height: 3000 })
    expect(imageSize(webpL(800, 600), 'image/webp')).toEqual({ width: 800, height: 600 })
    expect(imageSize(webpLossy(1024, 768), 'image/webp')).toEqual({ width: 1024, height: 768 })
  })
  it('refuses a decompression bomb and an over-long side', () => {
    expect(checkImageSize(png(30000, 30000), 'image/png')).toBe('too_many_pixels')
    expect(checkImageSize(png(20000, 10), 'image/png')).toBe('too_many_pixels')
    expect(checkImageSize(webpX(16000, 16000), 'image/webp')).toBe('too_many_pixels')
    expect(checkImageSize(png(7680, 4320), 'image/png')).toBe('ok')
    expect(7680 * 4320).toBeLessThan(MAX_IMAGE_PIXELS)
  })
  it('refuses a header it cannot read, a zero side and a type mix-up', () => {
    expect(checkImageSize(new Uint8Array([1, 2, 3]), 'image/png')).toBe('unreadable')
    expect(checkImageSize(png(0, 10), 'image/png')).toBe('unreadable')
    expect(checkImageSize(png(10, 10), 'image/jpeg')).toBe('unreadable')
    expect(checkImageSize(new Uint8Array([0xff, 0xd8, 0xff, 0xda]), 'image/jpeg')).toBe('unreadable')
  })
})
