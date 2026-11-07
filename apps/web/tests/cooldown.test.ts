// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, h, shallowRef } from 'vue'
import { DENIED_COOLDOWN_SECS, useDeniedCooldown } from '../src/composables/useCooldown'
import type { SenderState } from '../src/core/senderSession'

beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] }))
afterEach(() => vi.useRealTimers())

describe('"Ask again" after a denial (4.4: 15 s)', () => {
  it('counts down from 15, never starts higher even when the error is newer than the last tick, and only a denial waits', () => {
    const state = shallowRef<SenderState>({ phase: 'idle', errorCode: null, channelOpen: false })
    let value = -1
    const C = defineComponent({
      setup() {
        const c = useDeniedCooldown(state)
        return () => h('span', (value = c.value))
      },
    })
    const w = mount(C)
    expect(value).toBe(0)
    state.value = { phase: 'error', errorCode: 'join_denied', channelOpen: false, errorAt: Date.now() + 300 } // newer than the last tick
    return w.vm.$nextTick().then(() => {
      expect(DENIED_COOLDOWN_SECS).toBe(15)
      expect(value).toBe(15)
      vi.advanceTimersByTime(5200)
      return w.vm.$nextTick().then(() => {
        expect(value).toBeLessThanOrEqual(11)
        expect(value).toBeGreaterThanOrEqual(10)
        state.value = { phase: 'error', errorCode: 'room_locked', channelOpen: false, errorAt: Date.now() }
        return w.vm.$nextTick().then(() => expect(value).toBe(0))
      })
    })
  })
})
