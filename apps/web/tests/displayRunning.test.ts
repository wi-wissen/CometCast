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
const hoisted = vi.hoisted(() => ({ onEvent: undefined as undefined | ((e: unknown) => void), opts: undefined as undefined | Record<string, unknown> }))
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
    hoisted.opts = opts
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
    hoisted.onEvent?.({ kind: 'openDisabled' })
    await nextTick()
    expect(w.findAll('[data-testid=toast]')).toHaveLength(1)
    expect(w.get('[data-testid=toast]').text()).toBe('This server only allows Approval on')
    vi.advanceTimersByTime(3199)
    await nextTick()
    expect(w.find('[data-testid=toast]').exists()).toBe(true)
    vi.advanceTimersByTime(2)
    await nextTick()
    expect(w.find('[data-testid=toast]').exists()).toBe(false)
  })

  it('there is no toast for locked-while-requests-wait any more (a request closes Settings instead)', async () => {
    const w = mk()
    hoisted.onEvent?.({ kind: 'lockedRejected', n: 3 })
    await nextTick()
    expect(w.find('[data-testid=toast]').exists()).toBe(false)
  })

  it('sends the room name of the setup to the server (it reaches the presenter pages)', () => {
    mk()
    expect(hoisted.opts).toMatchObject({ roomName: 'Workshop', joinMode: 'confirm' })
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

  it('the connected toast of open mode adds the neutral device sentence only for a name another device used', async () => {
    const w = mk()
    hoisted.onEvent?.({ kind: 'joined', name: 'Ana', otherDevice: true })
    await nextTick()
    expect(w.get('[data-testid=toast]').text()).toBe('“Ana” connected This name was used here before by a different device.')
    hoisted.onEvent?.({ kind: 'joined', name: 'Ana' })
    await nextTick()
    expect(w.get('[data-testid=toast]').text()).toBe('“Ana” connected')
  })

  it('a lost connection: the code and QR code go at once, the box comes only when the retry at 3 s has failed', async () => {
    const w = mk()
    await flushPromises()
    await set({ phase: 'error', errorCode: 'connection_lost', code: null })
    // "connecting" look: empty code line and empty QR box, no box yet (no flicker on short drops)
    expect(w.find('[data-testid=offline]').exists()).toBe(false)
    expect(w.get('[data-testid=room-code]').text()).toBe('')
    expect(w.get('[data-testid=qr]').attributes('data-empty')).toBe('true')
    // 1 s and 2 s: the retries run (and fail) without a box
    api.start.mockClear()
    for (const failAt of [1000, 2000]) {
      await vi.advanceTimersByTimeAsync(failAt === 1000 ? 1000 : 1000)
      expect(api.start).toHaveBeenCalledTimes(failAt / 1000)
      await set({ phase: 'connecting', errorCode: null })
      await set({ phase: 'error', errorCode: 'connection_lost' })
      expect(w.find('[data-testid=offline]').exists()).toBe(false)
    }
    await vi.advanceTimersByTimeAsync(1000)
    expect(api.start).toHaveBeenCalledTimes(3) // the attempt at 3 s
    expect(w.find('[data-testid=offline]').exists()).toBe(false) // it is still running
    await set({ phase: 'connecting', errorCode: null })
    await set({ phase: 'error', errorCode: 'connection_lost' }) // ... and fails: now the box
    expect(w.find('[data-testid=offline]').exists()).toBe(true)
    expect(w.get('[data-testid=offline-body]').text()).toContain('This screen is offline right now')
    expect(w.get('[data-testid=qr]').attributes('data-empty')).toBe('true')
    // the box stays while the next attempt runs, which comes 4 s later (at 7 s)
    await vi.advanceTimersByTimeAsync(3900)
    expect(api.start).toHaveBeenCalledTimes(3)
    await vi.advanceTimersByTimeAsync(200)
    expect(api.start).toHaveBeenCalledTimes(4)
    await set({ phase: 'connecting', errorCode: null })
    expect(w.find('[data-testid=offline]').exists()).toBe(true) // no flicker while a later attempt runs
  })

  describe('reconnect schedule with a server that comes back (decision 13.30)', () => {
    // A server that answers or not at the moment an attempt starts.
    let up = false
    const attempts: number[] = []
    beforeEach(() => {
      up = false
      attempts.length = 0
      api.start.mockImplementation(async () => {
        attempts.push(Date.now() - t0)
        await set({ phase: 'connecting', errorCode: null })
        if (up) await set({ phase: 'waiting_sender', code: 'AB2CD3', errorCode: null })
        else await set({ phase: 'error', errorCode: 'connection_lost', code: null })
      })
    })
    afterEach(() => api.start.mockImplementation(async () => {}))
    let t0 = 0
    /** Mounts with a down server and returns the offline box state sampled every 250 ms for `ms`. */
    async function run(w: ReturnType<typeof mk>, ms: number, backAt: number) {
      const seen: boolean[] = []
      for (let t = 0; t < ms; t += 250) {
        if (t >= backAt) up = true
        await vi.advanceTimersByTimeAsync(250)
        seen.push(w.find('[data-testid=offline]').exists())
      }
      return seen
    }
    const start = async () => {
      t0 = Date.now()
      const w = mk()
      await flushPromises() // onMounted: the first attempt fails at once
      return w
    }

    it('tries again at 1 s, 2 s, 3 s, then every 4 s', async () => {
      const w = await start()
      await run(w, 12_000, Infinity)
      expect(attempts).toEqual([0, 1000, 2000, 3000, 7000, 11_000])
    })
    it('server back at 2.5 s: no box ever (a screenshot series every 250 ms), the code is back at the attempt at 3 s', async () => {
      const w = await start()
      const seen = await run(w, 5000, 2500)
      expect(seen.some(Boolean)).toBe(false)
      expect(w.get('[data-testid=room-code]').text()).toBe('AB2 CD3')
    })
    it('server back at 3.5 s: the box from 3 s until the attempt at 7 s', async () => {
      const w = await start()
      const seen = await run(w, 8000, 3500) // sample i covers up to (i + 1) * 250 ms
      const at = (ms: number) => seen[ms / 250 - 1]
      expect(at(2750)).toBe(false)
      expect(at(3250)).toBe(true)
      expect(at(6750)).toBe(true)
      expect(at(7250)).toBe(false)
      expect(attempts).toEqual([0, 1000, 2000, 3000, 7000])
      expect(w.get('[data-testid=room-code]').text()).toBe('AB2 CD3')
    })
  })

  it('a drop that is over before 3 s never shows the box', async () => {
    const w = mk()
    await flushPromises()
    await set({ phase: 'error', errorCode: 'connection_lost', code: null })
    vi.advanceTimersByTime(2000)
    await set({ phase: 'connecting', errorCode: null })
    await set({ phase: 'waiting_sender', code: 'AB2CD3' })
    vi.advanceTimersByTime(5000)
    await nextTick()
    expect(w.find('[data-testid=offline]').exists()).toBe(false)
    expect(w.get('[data-testid=room-code]').text()).toBe('AB2 CD3')
  })

  it('no code for 8 s after the start also ends in the box', async () => {
    const w = mk()
    await set({ phase: 'connecting', code: null })
    vi.advanceTimersByTime(7000)
    await nextTick()
    expect(w.find('[data-testid=offline]').exists()).toBe(false)
    expect(w.get('[data-testid=room-code]').text()).toBe('')
    vi.advanceTimersByTime(1100)
    await nextTick()
    expect(w.find('[data-testid=offline]').exists()).toBe(true)
  })

  it('the box body follows the cause; a busy server is tried again after 30 s, an outdated page never', async () => {
    const w = mk()
    await flushPromises()
    await set({ phase: 'error', errorCode: 'rate_limited', code: null })
    vi.advanceTimersByTime(3100)
    await nextTick()
    expect(w.get('[data-testid=offline-body]').text()).toBe('The server is busy right now. This screen tries again by itself.')
    api.start.mockClear()
    vi.advanceTimersByTime(20_000)
    expect(api.start).not.toHaveBeenCalled()
    vi.advanceTimersByTime(7000)
    expect(api.start).toHaveBeenCalledTimes(1)
    await set({ phase: 'connecting', errorCode: null })
    await set({ phase: 'error', errorCode: 'proto_mismatch', code: null })
    await nextTick()
    expect(w.get('[data-testid=offline-body]').text()).toBe('This page is out of date. Reload it to get a code.')
    api.start.mockClear()
    vi.advanceTimersByTime(120_000)
    expect(api.start).not.toHaveBeenCalled()
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
    // the annotation toolbar is hidden at rest too; nothing else is drawn over the picture
    expect(w.get('[data-testid=toolbar-root]').classes()).toContain('hid')
    expect(w.get('[data-testid=toolbar-root]').attributes('inert')).toBeDefined()
    expect(w.find('[data-testid=content-close]').exists()).toBe(false)
    expect(w.find('[data-testid=frozen-badge]').exists()).toBe(false)
    await set({ content: null, phase: 'waiting_sender', peerName: null })
    await flushPromises()
    expect(wakeLog).toEqual(['acquire', 'release'])
    expect(w.get('.cd-canvas').attributes('style') ?? '').not.toContain('display: none')
  })

  it('a link card or a join request puts name tag and End under the scrim and makes them inert (decision 13.68)', async () => {
    const w = mk()
    await set({ ...presentingState(), content: { kind: 'link', url: 'https://example.org/x', host: 'example.org' } })
    await flushPromises()
    expect(w.get('[data-testid=byline]').attributes('inert')).toBeDefined()
    expect(w.get('[data-testid=byline]').classes()).toContain('under')
    expect(w.get('[data-testid=end]').element.closest('.cd-pv')!.hasAttribute('inert')).toBe(true)
    await set({ content: { kind: 'image', url: 'blob:x', name: 'a.png' } })
    expect(w.get('[data-testid=byline]').attributes('inert')).toBeUndefined()
    await set({ requests: [{ id: 1, name: 'Ana', check: 'AB' }] as never })
    expect(w.get('[data-testid=byline]').attributes('inert')).toBeDefined()
    expect(w.get('[data-testid=byline]').classes()).not.toContain('under')
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
    // the toolbar comes with them and goes with them
    expect(w.get('[data-testid=toolbar-root]').classes()).not.toContain('hid')
    expect(w.get('[data-testid=toolbar-root]').attributes('inert')).toBeUndefined()
    vi.advanceTimersByTime(5001)
    await nextTick()
    expect(w.get('[data-testid=byline]').classes()).toContain('hid')
    expect(w.get('[data-testid=toolbar-root]').classes()).toContain('hid')
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

  it('the name tag has the security code as its second line, without a device type', async () => {
    const w = mk()
    await set({ ...presentingState(), sas: '171 852' })
    await flushPromises()
    expect(w.get('[data-testid=byline-name]').text()).toBe('“Lena” is showing')
    expect(w.get('[data-testid=sas-line]').text()).toBe('Security code 171 852')
    expect(w.get('[data-testid=sas-code]').classes()).toContain('mono')
  })

  it('the toast sits below the toolbar while the controls are shown', async () => {
    const w = mk()
    await set(presentingState())
    await flushPromises()
    hoisted.onEvent?.({ kind: 'left', name: 'Ana' })
    await nextTick()
    expect(w.get('[data-testid=toast]').classes()).not.toContain('low')
    window.dispatchEvent(new MouseEvent('pointermove'))
    await nextTick()
    expect(w.get('[data-testid=toast]').classes()).toContain('low')
    vi.advanceTimersByTime(5001) // the controls (and the toolbar) fade out; the toast is gone by then, a new one sits at the top
    hoisted.onEvent?.({ kind: 'left', name: 'Ben' })
    await nextTick()
    expect(w.get('[data-testid=toast]').classes()).not.toContain('low')
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

  it('a join request closes Settings (one dialog at a time), focuses Yes, and gives the focus back to the Settings button', async () => {
    const w = mk()
    await flushPromises()
    await w.get('[data-testid=settings-open]').trigger('click')
    await w.get('[data-testid=settings-mode-locked]').trigger('click') // changes already made stay
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(true)
    await set({ requests: [{ id: 5, name: 'Lena', check: 33, receivedAt: Date.now() }] })
    await flushPromises()
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(false)
    expect(w.find('[data-testid=request-modal]').exists()).toBe(true)
    expect(document.activeElement).toBe(w.get('[data-testid=request-yes]').element)
    expect(api.setJoinMode).toHaveBeenCalledWith('locked')
    vi.advanceTimersByTime(REQUEST_GUARD_MS + 200)
    await nextTick()
    await w.get('[data-testid=request-no]').trigger('click')
    await set({ requests: [] })
    await flushPromises()
    expect(w.find('[data-testid=request-modal]').exists()).toBe(false)
    expect(w.find('[data-testid=settings-modal]').exists()).toBe(false) // it does not come back by itself
    expect(document.activeElement).toBe(w.get('[data-testid=settings-open]').element)
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
