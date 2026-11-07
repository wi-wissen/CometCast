// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick, shallowRef } from 'vue'
import { createAppI18n } from '../src/i18n'
import type { SenderState } from '../src/core/senderSession'

const state = shallowRef<SenderState>({ phase: 'idle', errorCode: null, channelOpen: false })
const api = {
  start: vi.fn(async () => {}),
  stop: vi.fn(),
  sendImage: vi.fn(async () => {}),
  sendDocument: vi.fn(async () => {}),
  sendLink: vi.fn(() => true),
  sendCmd: vi.fn(),
  shareScreen: vi.fn(async () => 'ok' as 'ok' | 'cancelled' | 'failed'),
  stopScreen: vi.fn(async () => {}),
}
vi.mock('../src/composables/useSender', () => ({ useSender: () => ({ state, ...api }) }))

import PresenterView from '../src/views/PresenterView.vue'
import StartPage from '../src/components/presenter/StartPage.vue'
import ErrorPage from '../src/components/presenter/ErrorPage.vue'

const UA = {
  android: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36',
  iphone: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1',
  linux: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
}
function ua(v: string) {
  Object.defineProperty(navigator, 'userAgent', { configurable: true, get: () => v })
}
const i18n = (l: 'en' | 'de' = 'en') => ({ plugins: [createAppI18n(l)] })
const set = async (p: Partial<SenderState>) => {
  state.value = { ...state.value, ...p }
  await nextTick()
}

beforeEach(() => {
  state.value = { phase: 'idle', errorCode: null, channelOpen: false }
  Object.values(api).forEach((f) => f.mockClear())
  api.shareScreen.mockResolvedValue('ok')
  localStorage.clear()
  ua(UA.linux)
  // A laptop browser that can share its screen.
  Object.defineProperty(navigator, 'mediaDevices', { configurable: true, value: { getDisplayMedia: () => Promise.resolve({}) } })
  Object.defineProperty(window, 'isSecureContext', { configurable: true, value: true })
})
afterEach(() => {
  document.body.innerHTML = ''
  vi.useRealTimers()
})

describe('start page (5.2)', () => {
  const mk = (props: Record<string, unknown> = {}) => mount(StartPage, { global: i18n(), props: { qrCode: null, dom: 'cometcast.de', busy: false, ...props } })

  it('typed code: label, placeholder, empty submit gives the inline error and no connection', async () => {
    const w = mk()
    expect(w.get('label[for=start-code]').text()).toBe('Code from the screen')
    expect(w.get('[data-testid=start-code]').attributes('placeholder')).toBe('XXX XXX')
    await w.get('form').trigger('submit')
    expect(w.get('[data-testid=start-code-error]').text()).toBe('Please enter the code shown large on the screen.')
    expect(w.get('[data-testid=start-code]').classes()).toContain('bad')
    expect(w.emitted('connect')).toBeUndefined()
  })
  it('normalises the code (uppercase, spaces and dashes removed) and connects on submit only', async () => {
    const w = mk()
    await w.get('[data-testid=start-code]').setValue('k7q-m 2w')
    expect(w.emitted('connect')).toBeUndefined() // validated on submit only
    await w.get('form').trigger('submit')
    expect(w.emitted('connect')).toEqual([['K7QM2W']])
  })
  it('code from the QR code: chip with tick and "from the QR code"', () => {
    const w = mk({ qrCode: 'K7QM2W' })
    expect(w.find('[data-testid=start-code]').exists()).toBe(false)
    expect(w.get('[data-testid=start-code-chip]').text()).toBe('K7Q M2W')
    expect(w.get('[data-testid=code-chip]').text()).toContain('from the QR code')
  })
  it('the name is remembered on the device (max 40 characters)', async () => {
    const w = mk()
    expect(w.get('[data-testid=start-name]').attributes('maxlength')).toBe('40')
    await w.get('[data-testid=start-name]').setValue('Lena')
    expect(localStorage.getItem('cometcast.displayName')).toBe('Lena')
    expect(mk().get('[data-testid=start-name]').element).toHaveProperty('value', 'Lena')
  })
  it('laptop: browser text, desktop download with the system name, detected line, "be the display" line, download note', async () => {
    const w = mk()
    expect(w.get('[data-testid=start-browser]').text()).toBe('Continue in the browser')
    expect(w.text()).toContain('No install. Show your screen, a window, files and links.')
    expect(w.get('[data-testid=start-app]').text()).toBe('Download the app for Linux')
    expect(w.get('[data-testid=detected]').text()).toBe('Detected: Laptop, Chrome')
    expect(w.text()).toContain('Should this device be the screen itself? Open cometcast.de/display.')
    expect(w.find('[data-testid=download-note]').exists()).toBe(false)
    await w.get('[data-testid=start-app]').trigger('click')
    expect(w.get('[data-testid=download-note]').text()).toContain('Download started.')
  })
  it('Android phone: Google Play wording; iPhone: App Store wording; no laptop-only line', () => {
    ua(UA.android)
    const a = mk()
    expect(a.get('[data-testid=start-app]').text()).toBe('Get the app on Google Play')
    expect(a.text()).toContain('No install. Show photos, files and links.')
    expect(a.text()).toContain('With the app you can also share your whole phone screen.')
    expect(a.get('[data-testid=detected]').text()).toBe('Detected: Android phone, Chrome')
    expect(a.text()).not.toContain('Should this device be the screen itself')
    ua(UA.iphone)
    const i = mk()
    expect(i.get('[data-testid=start-app]').text()).toBe('Get the app on the App Store')
    expect(i.get('[data-testid=detected]').text()).toBe('Detected: iPhone, Safari')
  })
  it('German uses Sie', () => {
    const w = mount(StartPage, { global: i18n('de'), props: { qrCode: null, dom: 'cometcast.de', busy: false } })
    expect(w.get('label[for=start-name]').text()).toBe('Ihr Name')
    expect(w.text()).toContain('Im Browser weiter')
  })
})

describe('error pages (6)', () => {
  const mk = (props: Record<string, unknown>) => mount(ErrorPage, { global: i18n(), props: { cooldown: 0, ...props } as never })

  it.each([
    ['code', 'This code does not exist right now', ['Retype code']],
    ['expired', 'This code is no longer valid', ['Retype code']],
    ['noInternet', 'No internet on this device', ['Try again']],
    ['noScreen', 'Screen sharing does not work in this browser', ['Show photo or file', 'Show a link', 'Get the app']],
    ['displayOff', 'The screen does not answer', ['Try again']],
    ['denied', 'Not allowed', ['Ask again']],
    ['locked', 'Locked right now', ['Try again']],
    ['timeout', 'No answer', ['Ask again']],
    ['ended', 'Presentation ended', ['Retype code']],
  ])('%s: heading, why and what now, buttons', (kind, title, buttons) => {
    const w = mk({ kind })
    expect(w.get('[data-testid=error-title]').text()).toBe(title)
    expect(w.get('[data-testid=error-why]').text()).not.toBe('')
    expect(w.get('[data-testid=error-now]').text()).not.toBe('')
    expect(w.findAll('.cp-acts button').map((b) => b.text())).toEqual(buttons)
    expect(w.findAll('.cp-acts button')[0].classes()).toContain('cp-pbtn') // primary first
  })
  it('locked: two "why" variants', () => {
    expect(mk({ kind: 'locked' }).get('[data-testid=error-why]').text()).toBe('The code is right, but this screen is not taking anyone new right now.')
    expect(mk({ kind: 'locked', lockedWhile: true }).get('[data-testid=error-why]').text()).toBe('The host locked the room while you were waiting. Your request was dropped.')
  })
  it('denied: "Ask again" is disabled for 15 s and its label counts down', async () => {
    const w = mk({ kind: 'denied', cooldown: 14 })
    const b = w.get('[data-testid=error-ask]')
    expect(b.text()).toBe('Ask again in 14 s')
    expect(b.attributes('disabled')).toBeDefined()
    await w.setProps({ cooldown: 0 })
    expect(w.get('[data-testid=error-ask]').text()).toBe('Ask again')
    expect(w.get('[data-testid=error-ask]').attributes('disabled')).toBeUndefined()
  })
  it('a failure without a page of its own keeps the layout with the existing wording', () => {
    const w = mk({ kind: 'generic', code: 'rate_limited' })
    expect(w.get('[data-testid=error-title]').text()).toContain('Too many requests')
    expect(w.find('[data-testid=error-why]').exists()).toBe(false)
  })
})

describe('presenter flow', () => {
  const mk = (qrCode: string | null = null) => mount(PresenterView, { attachTo: document.body, global: i18n(), props: { qrCode } })

  it('a malformed code never connects: "This code does not exist right now"', async () => {
    const w = mk()
    await w.get('[data-testid=start-code]').setValue('ABC')
    await w.get('form').trigger('submit')
    expect(api.start).not.toHaveBeenCalled()
    expect(w.get('[data-testid=error-title]').text()).toBe('This code does not exist right now')
  })

  it('open mode: connect, then "What to show"; file and link go to presenting; Stop returns', async () => {
    const w = mk('K7QM2W')
    await w.get('[data-testid=start-browser]').trigger('click')
    await w.get('form').trigger('submit')
    expect(api.start).toHaveBeenCalledWith('K7QM2W', { capture: false })
    await set({ phase: 'joining' })
    expect(w.find('[data-testid=what-page]').exists()).toBe(false)
    await set({ phase: 'sharing', channelOpen: true, sas: '123 456' })
    expect(w.get('[data-testid=what-page]').text()).toContain('What would you like to show?')
    expect(w.get('[data-testid=connected-to]').text()).toBe('Connected to “K7Q M2W”')
    expect(w.get('[data-testid=what-screen]').text()).toContain('Whole screen or one window, live')
    // link
    await w.get('[data-testid=what-link]').trigger('click')
    await w.get('[data-testid=link-url]').setValue('javascript:alert(1)')
    await w.get('form').trigger('submit')
    expect(w.find('[data-testid=link-error]').exists()).toBe(true)
    expect(api.sendLink).not.toHaveBeenCalled()
    await w.get('[data-testid=link-url]').setValue('https://example.org/x')
    await w.get('form').trigger('submit')
    expect(api.sendLink).toHaveBeenCalledWith('https://example.org/x')
    expect(w.get('[data-testid=presenting-title]').text()).toBe('The screen shows your link')
    expect(w.get('[data-testid=presenting-page]').text()).toContain('On “K7Q M2W”.')
    await w.get('[data-testid=presenting-stop]').trigger('click')
    await flushPromises()
    expect(api.sendCmd).toHaveBeenCalledWith('stop')
    expect(w.find('[data-testid=what-page]').exists()).toBe(true)
  })

  it('screen: the system dialog opens directly; cancelling returns without an error; sharing and stopping', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    api.shareScreen.mockResolvedValueOnce('cancelled')
    await w.get('[data-testid=what-screen]').trigger('click')
    await flushPromises()
    expect(w.find('[data-testid=error-page]').exists()).toBe(false)
    expect(w.find('[data-testid=what-page]').exists()).toBe(true)
    await w.get('[data-testid=what-screen]').trigger('click')
    await flushPromises()
    expect(w.get('[data-testid=presenting-title]').text()).toBe('You are sharing your screen')
    await set({ screen: true })
    // the browser's own stop button ends the screen: back to "What to show"
    await set({ screen: undefined })
    expect(w.find('[data-testid=what-page]').exists()).toBe(true)
    await w.get('[data-testid=what-screen]').trigger('click')
    await flushPromises()
    await w.get('[data-testid=presenting-stop]').trigger('click')
    await flushPromises()
    expect(api.stopScreen).toHaveBeenCalled()
    expect(w.find('[data-testid=what-page]').exists()).toBe(true)
  })

  it('approval: waiting page with the check number; withdrawing goes back to the start page', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'waiting_approval', check: 47, timeoutSecs: 60 })
    expect(w.get('[data-testid=waiting-check]').text()).toBe('47')
    expect(w.get('[data-testid=waiting-page]').text()).toContain('Waiting for approval')
    expect(w.get('[data-testid=waiting-page]').text()).toContain('Also shown on the screen.')
    expect(w.get('[data-testid=waiting-left]').text()).toBe('The request expires in 60 s.')
    await w.get('[data-testid=withdraw]').trigger('click')
    expect(api.stop).toHaveBeenCalled()
    expect(w.find('[data-testid=start-page]').exists()).toBe(true)
  })

  it.each([
    [{ phase: 'error', errorCode: 'room_not_found' }, 'This code does not exist right now'],
    [{ phase: 'error', errorCode: 'join_denied' }, 'Not allowed'],
    [{ phase: 'error', errorCode: 'room_locked' }, 'Locked right now'],
    [{ phase: 'error', errorCode: 'approval_timeout' }, 'No answer'],
    [{ phase: 'error', errorCode: 'connection_lost' }, 'No internet on this device'],
    [{ phase: 'peer_left', errorCode: null }, 'Presentation ended'],
  ] as const)('the sender session %j shows "%s"', async (patch, title) => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set(patch as Partial<SenderState>)
    expect(w.get('[data-testid=error-title]').text()).toBe(title)
  })

  it('locked while waiting: its own "why"; waiting then room_not_found is "no longer valid"', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'waiting_approval', check: 11 })
    await set({ phase: 'error', errorCode: 'room_locked_waiting' })
    expect(w.get('[data-testid=error-why]').text()).toContain('locked the room while you were waiting')
    const w2 = mk('K7QM2W')
    await set({ phase: 'idle', errorCode: null })
    await w2.get('form').trigger('submit')
    await set({ phase: 'waiting_approval', check: 11 })
    await set({ phase: 'error', errorCode: 'room_not_found' })
    expect(w2.get('[data-testid=error-title]').text()).toBe('This code is no longer valid')
  })

  it('"Disconnect" ends the session and shows the goodbye page; "Connect again" returns to the start', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    await w.get('[data-testid=disconnect]').trigger('click')
    expect(api.stop).toHaveBeenCalled()
    expect(w.get('[data-testid=bye-page]').text()).toContain('Disconnected')
    await w.get('[data-testid=bye-again]').trigger('click')
    expect(w.find('[data-testid=start-page]').exists()).toBe(true)
  })

  it('a phone browser cannot share the screen: error page "Screen sharing does not work in this browser", the app hint is in the subtitle', async () => {
    ua(UA.android)
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    expect(w.get('[data-testid=what-screen]').text()).toContain('On a phone only with the app')
    await w.get('[data-testid=what-screen]').trigger('click')
    expect(w.get('[data-testid=error-title]').text()).toBe('Screen sharing does not work in this browser')
    expect(api.shareScreen).not.toHaveBeenCalled()
  })

  it('keeps 76 px free under the button while a laptop browser shows its sharing bar', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    await w.get('[data-testid=what-screen]').trigger('click')
    await flushPromises()
    expect(w.get('[data-testid=presenting-page]').classes()).toContain('keepbar')
  })
})
