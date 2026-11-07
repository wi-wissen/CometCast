// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { ScreenWakeLock, type VisibilityEnv, type WakeLockSentinelLike } from '../src/core/wakeLock'

function env() {
  const log: string[] = []
  const sentinels: Array<WakeLockSentinelLike & { released: boolean }> = []
  const listeners = new Set<() => void>()
  const doc: VisibilityEnv = {
    visibilityState: 'visible',
    addEventListener: (_t, cb) => listeners.add(cb),
    removeEventListener: (_t, cb) => listeners.delete(cb),
  }
  const nav = {
    wakeLock: {
      request: async (t: 'screen') => {
        log.push(`request:${t}`)
        const s = {
          released: false,
          release: async () => {
            s.released = true
            log.push('release')
          },
        }
        sentinels.push(s)
        return s
      },
    },
  }
  const flush = () => new Promise((r) => setTimeout(r, 0))
  const setVisible = (v: boolean) => {
    doc.visibilityState = v ? 'visible' : 'hidden'
    listeners.forEach((l) => l())
  }
  return { log, sentinels, nav, doc, flush, setVisible, listeners }
}

describe('screen wake lock (D5): only while presenting', () => {
  it('is not requested on idle, acquired when presenting starts and released when it stops', async () => {
    const e = env()
    const l = new ScreenWakeLock(e.nav, e.doc)
    await e.flush()
    expect(e.log).toEqual([])
    l.setWanted(true)
    await e.flush()
    expect(e.log).toEqual(['request:screen'])
    expect(l.held).toBe(true)
    l.setWanted(true) // same state: no second request
    await e.flush()
    expect(e.log).toEqual(['request:screen'])
    l.setWanted(false)
    await e.flush()
    expect(e.log).toEqual(['request:screen', 'release'])
    expect(l.held).toBe(false)
  })
  it('is released when the page goes to the background and asked for again when it is back', async () => {
    const e = env()
    const l = new ScreenWakeLock(e.nav, e.doc)
    l.setWanted(true)
    await e.flush()
    e.setVisible(false)
    await e.flush()
    expect(l.held).toBe(false)
    e.setVisible(true)
    await e.flush()
    expect(e.log).toEqual(['request:screen', 'release', 'request:screen'])
    expect(l.held).toBe(true)
  })
  it('does not ask again after the presentation ended while the page was hidden', async () => {
    const e = env()
    const l = new ScreenWakeLock(e.nav, e.doc)
    l.setWanted(true)
    await e.flush()
    e.setVisible(false)
    l.setWanted(false)
    e.setVisible(true)
    await e.flush()
    expect(e.log.filter((x) => x === 'request:screen').length).toBe(1)
    expect(l.held).toBe(false)
  })
  it('survives a missing API and a refusing one', async () => {
    const e = env()
    const none = new ScreenWakeLock({}, e.doc)
    none.setWanted(true)
    await e.flush()
    expect(none.held).toBe(false)
    const refusing = new ScreenWakeLock({ wakeLock: { request: async () => Promise.reject(new DOMException('no', 'NotAllowedError')) } }, e.doc)
    refusing.setWanted(true)
    await e.flush()
    expect(refusing.held).toBe(false)
  })
  it('releases a lock that arrives after the presentation already ended', async () => {
    const e = env()
    const l = new ScreenWakeLock(e.nav, e.doc)
    l.setWanted(true)
    l.setWanted(false)
    await e.flush()
    await e.flush()
    expect(l.held).toBe(false)
    expect(e.sentinels.every((s) => s.released)).toBe(true)
  })
  it('dispose removes the listener', () => {
    const e = env()
    const l = new ScreenWakeLock(e.nav, e.doc)
    expect(e.listeners.size).toBe(1)
    l.dispose()
    expect(e.listeners.size).toBe(0)
  })
})
