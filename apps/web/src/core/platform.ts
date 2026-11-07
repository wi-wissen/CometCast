// SPDX-License-Identifier: GPL-3.0-or-later
//
// "Detected: Android phone, Chrome" (5.2). Reads the user agent string and a few feature hints.
// It is only used for wording and for choosing the store link; it never limits what works.

export type DeviceKey = 'android' | 'iphone' | 'ipad' | 'phone' | 'chromebook' | 'laptop'
export type OsName = 'Windows' | 'macOS' | 'Linux' | 'ChromeOS' | 'Android' | 'iOS' | 'iPadOS'

export interface Platform {
  device: DeviceKey
  os: OsName
  /** Browser name for the "Detected" line. */
  browser: string
  /** A phone (single column page, no screen sharing in the browser). */
  isPhone: boolean
  /** Android or iOS family: the store link is for the system store. */
  store: 'google-play' | 'app-store' | null
  /** "Laptop" class layouts: computers, Chromebooks and tablets with a desktop browser. */
  isLaptop: boolean
}

export interface PlatformInput {
  ua: string
  maxTouchPoints?: number
  platform?: string
}

export function detectBrowser(ua: string): string {
  if (/\bEdg(e|A|iOS)?\//.test(ua)) return 'Edge'
  if (/\bOPR\/|\bOpera\b/.test(ua)) return 'Opera'
  if (/\bSamsungBrowser\//.test(ua)) return 'Samsung Internet'
  if (/\bFirefox\/|\bFxiOS\//.test(ua)) return 'Firefox'
  if (/\bCriOS\/|\bChrome\/|\bChromium\//.test(ua)) return 'Chrome'
  if (/\bSafari\//.test(ua) && /\bVersion\//.test(ua)) return 'Safari'
  return 'Browser'
}

export function detectPlatform(inp: PlatformInput): Platform {
  const ua = inp.ua
  const touch = inp.maxTouchPoints ?? 0
  const isAndroid = /\bAndroid\b/.test(ua)
  const mobile = /\bMobile\b/.test(ua)
  const isIphone = /\biPhone\b|\biPod\b/.test(ua)
  // iPadOS 13+ reports as a Mac; the touch screen gives it away.
  const isIpad = /\biPad\b/.test(ua) || (/\bMacintosh\b/.test(ua) && touch > 1)
  const isCros = /\bCrOS\b/.test(ua)
  const browser = detectBrowser(ua)
  const base = { browser }
  if (isIphone) return { ...base, device: 'iphone', os: 'iOS', isPhone: true, store: 'app-store', isLaptop: false }
  if (isIpad) return { ...base, device: 'ipad', os: 'iPadOS', isPhone: false, store: 'app-store', isLaptop: true }
  if (isAndroid && mobile) return { ...base, device: 'android', os: 'Android', isPhone: true, store: 'google-play', isLaptop: false }
  if (isAndroid) return { ...base, device: 'phone', os: 'Android', isPhone: false, store: 'google-play', isLaptop: true }
  if (isCros) return { ...base, device: 'chromebook', os: 'ChromeOS', isPhone: false, store: null, isLaptop: true }
  const os: OsName = /\bWindows\b/.test(ua) ? 'Windows' : /\bMac OS X\b|\bMacintosh\b/.test(ua) ? 'macOS' : 'Linux'
  return { ...base, device: 'laptop', os, isPhone: false, store: null, isLaptop: true }
}

export function currentPlatform(): Platform {
  if (typeof navigator === 'undefined') return detectPlatform({ ua: '' })
  return detectPlatform({ ua: navigator.userAgent, maxTouchPoints: navigator.maxTouchPoints, platform: navigator.platform })
}
