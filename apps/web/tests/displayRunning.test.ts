// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick, shallowRef } from 'vue'
import { createAppI18n } from '../src/i18n'
import type { ReceiverState } from '../src/core/receiverSession'

// The receiver (WebSocket, WebRTC) is replaced by a state that the test controls.
const base: ReceiverState = {
  phase: 'waiting_sender',
  code: 'K7QM2W',
  joinMode: 'confirm',
  requests: [],
  peerName: null,
  blocks: [],
  notice: null,
  errorCode: null,
  stream: null,
  content: null,
  transfer: null,
  contentError: null,
}
const hoisted = vi.hoisted(() => ({ onEvent: undefined as undefined | ((e: unknown) => void) }))
const state = shallowRef<ReceiverState>({ ...base })
const api = {
  start: vi.fn(async () => {}),
  setJoinMode: vi.fn(),
  decide: vi.fn(),
  endRoom: vi.fn(),
  clearContent: vi.fn(),
  nextPage: vi.fn(),
  prevPage: vi.fn(),
  renderPage: vi.fn(),
}
vi.mock('../src/composables/useReceiver', () => ({
  useReceiver: (opts: { onEvent?: (e: unknown) => void }) => {
    hoisted.onEvent = opts.onEvent
    return { state, ...api }
  },
}))

import DisplayRunning from '../src/components/display/DisplayRunning.vue'
import { IDLE_DIM_MS } from '../src/core/idleDim'
import { REQUEST_GUARD_MS } from '../src/core/joinGuard'

const setup = { room: 'Workshop', mode: 'confirm' as const, server: null }
const mounted: Array<{ unmount(): void }> = []
const mk = () => {
  const w = mount(DisplayRunning, {
    attachTo: document.body,
    global: { plugins: [createAppI18n('en')] },
    props: { setup, theme: 'dark' as const, appearance: 'dark' as const },
  })
  mounted.push(w)
  return w
}
const set = async (p: Partial<ReceiverState>) => {
  state.value = { ...state.value, ...p }
  await nextTick()
}
const key = (k: string) => window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }))

let wakeLog: string[]
beforeEach(() => {
  HTMLCanvasElement.prototype.getContext = (() => null) as never
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] })
  state.value = { ...base }
  Object.values(api).forEach((f) => f.mockClear())
  wakeLog = []
  Object.defineProperty(navigator, 'wakeLock', {
    configurable: true,
    value: {
      request: async () => {
        wakeLog.push('acquire')
        return { release: async () => void wakeLog.push('release'), addEventListener() {} }
      },
    },
  })
})
afterEach(() => {
  mounted.splice(0).forEach((w) => w.unmount())
  vi.useRealTimers()
  document.body.innerHTML = ''
  delete (navigator as unknown as Record<string, unknown>).wakeLock
})

describe('display: idle', () => {
  it('starts the connection, shows the code and the Settings button holds the focus', async () => {
    const w = mk()
    await flushPromises()
    expect(api.start).toHaveBeenCalled()
    expect(w.get('[data-testid=room-code]').text()).toBe('K7Q M2W')
    expect(document.activeElement).toBe(w.get('[data-testid=settings-open]').element)
    // no wake lock on idle (D5)
    expect(wakeLog).toEqual([])
  })

  it('Settings opens by click and by M, closes with Escape, and gives the focus back; mode changes reach the receiver', async () => {
    const w = mk()
    await flushPromises()
    await w.get('[data-testid=settings-open]').trigger('click')
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(true)
    await w.get('[data-testid=settings-mode-locked]').trigger('click')
    expect(api.setJoinMode).toHaveBeenCalledWith('locked')
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(true) // stays open after a change
    key('Escape')
    await flushPromises()
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(false)
    expect(document.activeElement).toBe(w.get('[data-testid=settings-open]').element)
    key('m')
    await nextTick()
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(true)
  })

  it('a join request appears over idle; Back and Menu do nothing; Yes answers after the guard', async () => {
    const w = mk()
    await set({ requests: [{ id: 7, name: 'Lena', check: 47, receivedAt: Date.now() }] })
    await flushPromises()
    expect(w.get('[data-testid=request-name]').text()).toBe('“Lena”')
    key('Escape')
    key('m')
    await nextTick()
    expect(w.find('[data-testid=request-modal]').exists()).toBe(true)
    await w.get('[data-testid=request-yes]').trigger('click')
    expect(api.decide).not.toHaveBeenCalled()
    vi.advanceTimersByTime(REQUEST_GUARD_MS + 200)
    await nextTick()
    await w.get('[data-testid=request-yes]').trigger('click')
    expect(api.decide).toHaveBeenCalledWith(7, true)
    await w.get('[data-testid=request-block]').trigger('click')
    expect(api.decide).toHaveBeenLastCalledWith(7, false, true)
  })

  it('toasts: one line, replaced by the next, gone after 3.2 s', async () => {
    const w = mk()
    hoisted.onEvent?.({ kind: 'denied', name: 'Ana' })
    await nextTick()
    expect(w.get('[data-testid=toast]').text()).toBe('“Ana” denied')
    expect(w.get('[data-testid=toast]').attributes('role')).toBe('status')
    hoisted.onEvent?.({ kind: 'lockedRejected', n: 3 })
    await nextTick()
    expect(w.findAll('[data-testid=toast]')).toHaveLength(1)
    expect(w.get('[data-testid=toast]').text()).toBe('Locked · 3 open requests denied')
    vi.advanceTimersByTime(3199)
    await nextTick()
    expect(w.find('[data-testid=toast]').exists()).toBe(true)
    vi.advanceTimersByTime(2)
    await nextTick()
    expect(w.find('[data-testid=toast]').exists()).toBe(false)
  })

  it('a long name in a toast is cut to 18 characters, an empty one is "No name"', async () => {
    const w = mk()
    hoisted.onEvent?.({ kind: 'expired', name: 'Anna-Maria Schmid-Wolfgang' })
    await nextTick()
    expect(w.get('[data-testid=toast]').text()).toBe('Request from “Anna-Maria Schmid-…” expired')
    hoisted.onEvent?.({ kind: 'joined', name: '' })
    await nextTick()
    expect(w.get('[data-testid=toast]').text()).toBe('No name connected')
  })

  it('offline: the box is shown, and the connection is tried again by itself', async () => {
    const w = mk()
    await set({ phase: 'error', errorCode: 'connection_lost', code: null })
    expect(w.find('[data-testid=offline]').exists()).toBe(true)
    api.start.mockClear()
    vi.advanceTimersByTime(4100)
    expect(api.start).toHaveBeenCalledTimes(1)
  })

  it('dims after 7 minutes; the first key press only restores; activity restarts the count', async () => {
    const w = mk()
    vi.advanceTimersByTime(IDLE_DIM_MS - 1000)
    await nextTick()
    expect(w.get('[data-testid=dim]').classes()).not.toContain('on')
    vi.advanceTimersByTime(1000)
    await nextTick()
    expect(w.get('[data-testid=dim]').classes()).toContain('on')
    key('m') // would open Settings
    await nextTick()
    expect(w.get('[data-testid=dim]').classes()).not.toContain('on')
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(false)
    key('m')
    await nextTick()
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(true)
  })
})

describe('display: presenting', () => {
  const presentingState = (): Partial<ReceiverState> => ({
    phase: 'receiving',
    peerName: 'Lena',
    content: { kind: 'image', url: 'blob:x', name: 'a.png' },
  })

  it('shows only the picture at rest; keeps the screen awake only now; releases it when the presentation stops', async () => {
    const w = mk()
    await flushPromises()
    expect(wakeLog).toEqual([])
    await set(presentingState())
    await flushPromises()
    expect(wakeLog).toEqual(['acquire'])
    expect(w.get('[data-testid=stage]').isVisible()).toBe(true)
    expect(w.get('.cd-canvas').attributes('style')).toContain('display: none')
    expect(w.get('[data-testid=byline]').classes()).toContain('hid')
    await set({ content: null, phase: 'waiting_sender', peerName: null })
    await flushPromises()
    expect(wakeLog).toEqual(['acquire', 'release'])
    expect(w.get('.cd-canvas').attributes('style') ?? '').not.toContain('display: none')
  })

  it('input reveals the name tag and End for 5 s; the first key only reveals; End ends the session', async () => {
    const w = mk()
    await set(presentingState())
    await flushPromises()
    window.dispatchEvent(new MouseEvent('pointermove'))
    await nextTick()
    expect(w.get('[data-testid=byline]').classes()).not.toContain('hid')
    expect(w.get('[data-testid=byline]').text()).toContain('“Lena” is showing')
    expect(w.get('[data-testid=end]').text()).toBe('End')
    vi.advanceTimersByTime(5001)
    await nextTick()
    expect(w.get('[data-testid=byline]').classes()).toContain('hid')
    // a key while hidden only reveals and moves the focus to End; it activates nothing
    key('Enter')
    await flushPromises()
    expect(api.endRoom).not.toHaveBeenCalled()
    expect(w.get('[data-testid=byline]').classes()).not.toContain('hid')
    expect(document.activeElement).toBe(w.get('[data-testid=end]').element)
    // Back hides, a click reveals and End ends
    key('Escape')
    await nextTick()
    expect(w.get('[data-testid=byline]').classes()).toContain('hid')
    window.dispatchEvent(new Event('pointerdown'))
    await nextTick()
    await w.get('[data-testid=end]').trigger('click')
    expect(api.endRoom).toHaveBeenCalledTimes(1)
  })

  it('a join request appears over the running presentation (Settings does not)', async () => {
    const w = mk()
    await set(presentingState())
    await flushPromises()
    await set({ requests: [{ id: 1, name: 'Ben', check: 12, receivedAt: Date.now() }] })
    expect(w.find('[data-testid=request-modal]').exists()).toBe(true)
    expect(w.get('[data-testid=stage]').isVisible()).toBe(true)
    key('m')
    await nextTick()
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(false)
  })

  it('Settings closes when a presentation starts', async () => {
    const w = mk()
    await flushPromises()
    await w.get('[data-testid=settings-open]').trigger('click')
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(true)
    await set(presentingState())
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(false)
  })
})

describe('display: content that cannot be shown', () => {
  it('is told as a toast, because the idle screen has no other place for it', async () => {
    const w = mk()
    await set({ contentError: 'document_invalid' })
    expect(w.get('[data-testid=toast]').text()).toBe('The PDF is damaged or not supported and cannot be shown.')
  })
})
