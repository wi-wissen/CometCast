// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { detectPlatform } from '../src/core/platform'
import { parseRoute } from '../src/core/router'
import { errorKindFor } from '../src/core/presenterErrors'
import { formatCode } from '../src/core/codeFormat'
import { joinUrl, qrSvg } from '../src/core/qr'
import { displayScale } from '../src/composables/useDisplayScale'

describe('platform detection ("Detected: ...")', () => {
  const ua = {
    android: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36',
    iphone: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1',
    ipad: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15',
    win: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0',
    mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15',
    linux: 'Mozilla/5.0 (X11; Linux x86_64; rv:127.0) Gecko/20100101 Firefox/127.0',
    cros: 'Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
  }
  it('Android phone: Google Play, phone layout', () => {
    expect(detectPlatform({ ua: ua.android })).toMatchObject({ device: 'android', os: 'Android', browser: 'Chrome', isPhone: true, store: 'google-play', isLaptop: false })
  })
  it('iPhone: App Store; iPad (reports as a Mac, has touch): App Store, laptop layout', () => {
    expect(detectPlatform({ ua: ua.iphone })).toMatchObject({ device: 'iphone', browser: 'Safari', isPhone: true, store: 'app-store' })
    expect(detectPlatform({ ua: ua.ipad, maxTouchPoints: 5 })).toMatchObject({ device: 'ipad', store: 'app-store', isLaptop: true })
  })
  it('computers get the desktop download and their system name', () => {
    expect(detectPlatform({ ua: ua.win })).toMatchObject({ device: 'laptop', os: 'Windows', browser: 'Edge', store: null, isLaptop: true })
    expect(detectPlatform({ ua: ua.mac, maxTouchPoints: 0 })).toMatchObject({ os: 'macOS', browser: 'Safari' })
    expect(detectPlatform({ ua: ua.linux })).toMatchObject({ os: 'Linux', browser: 'Firefox' })
    expect(detectPlatform({ ua: ua.cros })).toMatchObject({ device: 'chromebook', os: 'ChromeOS' })
  })
})

describe('routes (5.1, 7.2)', () => {
  it('/display is the display, everything else the start page, a code path carries the code', () => {
    expect(parseRoute('/display')).toEqual({ name: 'display' })
    expect(parseRoute('/display/')).toEqual({ name: 'display' })
    expect(parseRoute('/')).toEqual({ name: 'start', code: null })
    expect(parseRoute('/app')).toEqual({ name: 'start', code: null })
    expect(parseRoute('/K7QM2W')).toEqual({ name: 'start', code: 'K7QM2W' })
    expect(parseRoute('/k7qm-2w')).toEqual({ name: 'start', code: 'K7QM2W' })
    expect(parseRoute('/K7QM2')).toEqual({ name: 'start', code: null })
    expect(parseRoute('/K7QM2O')).toEqual({ name: 'start', code: null }) // O is not in the alphabet
    expect(parseRoute('/a/b')).toEqual({ name: 'start', code: null })
  })
  it('follows a base path', () => {
    expect(parseRoute('/app/display', '/app/')).toEqual({ name: 'display' })
    expect(parseRoute('/app/', '/app/')).toEqual({ name: 'start', code: null })
    expect(parseRoute('/app', '/app/')).toEqual({ name: 'start', code: null })
    expect(parseRoute('/app/K7QM2W', '/app/')).toEqual({ name: 'start', code: 'K7QM2W' })
  })
})

describe('error pages of the presenter (6)', () => {
  it('maps server answers to the pages', () => {
    expect(errorKindFor('room_not_found')).toBe('code')
    expect(errorKindFor('room_not_found', { wasWaiting: true })).toBe('expired')
    expect(errorKindFor('join_denied')).toBe('denied')
    expect(errorKindFor('room_locked')).toBe('locked')
    expect(errorKindFor('room_locked_waiting')).toBe('locked')
    expect(errorKindFor('approval_timeout')).toBe('timeout')
    expect(errorKindFor('peer_left')).toBe('ended')
    expect(errorKindFor('connection_lost')).toBe('noInternet')
    expect(errorKindFor('peer_failed')).toBe('displayOff')
    expect(errorKindFor('rate_limited')).toBe('generic')
  })
})

describe('code and QR', () => {
  it('shows the code in two groups', () => {
    expect(formatCode('K7QM2W')).toBe('K7Q M2W')
    expect(formatCode('K7QM2W4P')).toBe('K7QM 2W4P')
    expect(formatCode('AB')).toBe('AB')
  })
  it('the QR code encodes https://{dom}/{CODE} without spaces', () => {
    expect(joinUrl('cometcast.de', 'K7Q M2W')).toBe('https://cometcast.de/K7QM2W')
    expect(joinUrl('127.0.0.1:8443', 'K7QM2W', 'http')).toBe('http://127.0.0.1:8443/K7QM2W')
    const q = qrSvg('https://cometcast.de/K7QM2W')
    expect(q.size).toBeGreaterThanOrEqual(21)
    expect(q.path.startsWith('M')).toBe(true)
    // Deterministic
    expect(qrSvg('https://cometcast.de/K7QM2W')).toEqual(q)
    expect(qrSvg('https://cometcast.de/ABCDEF').path).not.toBe(q.path)
  })
})

describe('scaling (3.1)', () => {
  it('1 reference px = min(width / 960, height / 540)', () => {
    expect(displayScale(1920, 1080)).toBe(2)
    expect(displayScale(3840, 2160)).toBe(4)
    expect(displayScale(1000, 540)).toBe(1) // wider than 16:9: limited by the height
    expect(displayScale(960, 1000)).toBe(1) // a portrait or square window is limited by the width
    expect(displayScale(480, 1000)).toBe(0.5)
  })
})

describe('browser names', () => {
  it('a headless Chrome is still Chrome', () => {
    expect(detectPlatform({ ua: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/126.0.0.0 Safari/537.36' }).browser).toBe('Chrome')
  })
})
