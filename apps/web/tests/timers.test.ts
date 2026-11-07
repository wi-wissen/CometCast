// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { IDLE_DIM_MS, IdleDim, dimDelayMs } from '../src/core/idleDim'
import { CONTROLS_HIDE_MS, ControlsReveal, MOUSE_THROTTLE_MS } from '../src/core/controlsReveal'
import { isArmed, REQUEST_GUARD_MS } from '../src/core/joinGuard'

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('idle dimming (8.6)', () => {
  it('dims after 7 minutes without activity and restores on any activity', () => {
    expect(IDLE_DIM_MS).toBe(7 * 60 * 1000)
    expect(dimDelayMs()).toBe(IDLE_DIM_MS) // no build-time override in a normal build
    const calls: boolean[] = []
    const d = new IdleDim((v) => calls.push(v), IDLE_DIM_MS)
    d.setEnabled(true)
    vi.advanceTimersByTime(IDLE_DIM_MS - 1)
    expect(d.isDimmed).toBe(false)
    vi.advanceTimersByTime(1)
    expect(d.isDimmed).toBe(true)
    d.activity()
    expect(d.isDimmed).toBe(false)
    expect(calls).toEqual([true, false])
    vi.advanceTimersByTime(IDLE_DIM_MS)
    expect(d.isDimmed).toBe(true)
  })
  it('activity restarts the countdown; leaving idle restores and stops it', () => {
    const d = new IdleDim(() => {}, 1000)
    d.setEnabled(true)
    vi.advanceTimersByTime(900)
    d.activity()
    vi.advanceTimersByTime(900)
    expect(d.isDimmed).toBe(false)
    vi.advanceTimersByTime(200)
    expect(d.isDimmed).toBe(true)
    d.setEnabled(false)
    expect(d.isDimmed).toBe(false)
    vi.advanceTimersByTime(5000)
    expect(d.isDimmed).toBe(false)
  })
  it('does nothing while disabled (presenting)', () => {
    const d = new IdleDim(() => {}, 100)
    d.activity()
    vi.advanceTimersByTime(1000)
    expect(d.isDimmed).toBe(false)
  })
})

describe('controls while presenting (4.5, 8.1)', () => {
  it('input reveals for 5 s after the last input; the first input only reveals', () => {
    let shown = false
    const c = new ControlsReveal((v) => (shown = v), Date.now)
    expect(c.poke()).toBe(true) // was hidden: this input only reveals
    expect(shown).toBe(true)
    vi.advanceTimersByTime(CONTROLS_HIDE_MS - 1)
    expect(c.poke()).toBe(false) // already shown: normal input
    vi.advanceTimersByTime(CONTROLS_HIDE_MS - 1)
    expect(shown).toBe(true)
    vi.advanceTimersByTime(1)
    expect(shown).toBe(false)
  })
  it('mouse movement triggers at most once per 400 ms', () => {
    let n = 0
    const c = new ControlsReveal(() => n++, Date.now)
    expect(c.mouseMove()).toBe(true)
    c.hideNow()
    expect(c.mouseMove()).toBe(false) // throttled
    vi.advanceTimersByTime(MOUSE_THROTTLE_MS)
    expect(c.mouseMove()).toBe(true)
  })
})

describe('request guard (4.4)', () => {
  it('ignores input for 1 s', () => {
    expect(REQUEST_GUARD_MS).toBe(1000)
    expect(isArmed(0, 999)).toBe(false)
    expect(isArmed(0, 1000)).toBe(true)
  })
})
