// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick, shallowRef } from 'vue'
import { createAppI18n } from '../src/i18n'
import type { SenderState } from '../src/core/senderSession'
import en from '../src/locales/en.json'

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
// The store and download addresses are empty placeholders (src/config/stores.ts); tests set them here.
const urls = vi.hoisted(() => ({ googlePlay: '', appStore: '', desktop: '' }))
vi.mock('../src/config/stores', () => ({
  STORE_URLS: urls,
  appUrlFor: (store: 'google-play' | 'app-store' | null) => (store === 'google-play' ? urls.googlePlay : store === 'app-store' ? urls.appStore : urls.desktop),
  anyAppLink: () => Boolean(urls.googlePlay || urls.appStore || urls.desktop),
}))

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
  Object.assign(urls, { googlePlay: '', appStore: '', desktop: '' })
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
  it('the field shows every typed character in uppercase (at most 9); spaces and dashes are removed on submit only', async () => {
    const w = mk()
    const field = w.get('[data-testid=start-code]')
    await field.setValue('k7q-m 2w')
    expect(w.emitted('connect')).toBeUndefined() // validated on submit only
    expect((field.element as HTMLInputElement).value).toBe('K7Q-M 2W') // nothing dropped, nothing regrouped
    await w.get('form').trigger('submit')
    expect(w.emitted('connect')).toEqual([['K7QM2W']])
    await field.setValue('abcdefghijkl')
    expect((field.element as HTMLInputElement).value).toBe('ABCDEFGHI')
    expect(field.attributes('maxlength')).toBe('9')
  })
  it('an O, 0, I or 1 stays visible in the field; it is the error page (not the field) that reacts', async () => {
    const w = mk()
    const field = w.get('[data-testid=start-code]')
    await field.setValue('k7o m2w')
    expect((field.element as HTMLInputElement).value).toBe('K7O M2W')
    await w.get('form').trigger('submit')
    expect(w.emitted('connect')).toEqual([['K7OM2W']]) // the caller checks it and shows the error page
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
  it('without a store or download link the page ends with the browser button, its small print and the detected line', () => {
    const w = mk()
    expect(w.get('[data-testid=start-browser]').text()).toBe('Continue in the browser')
    expect(w.text()).toContain('No install. Show your screen, a window, files and links.')
    expect(w.find('[data-testid=start-app]').exists()).toBe(false)
    expect(w.find('[data-testid=start-or]').exists()).toBe(false)
    expect(w.text()).not.toContain('Download the app')
    expect(w.text()).not.toContain('The app finds screens')
    expect(w.get('[data-testid=detected]').text()).toBe('Detected: Laptop, Chrome')
    // the laptop card: browser button, small print, detected line, "be the display"
    expect(w.find('.cb').element.children.length).toBe(3)
    for (const ua_ of [UA.android, UA.iphone]) {
      ua(ua_)
      const p = mk()
      expect(p.find('[data-testid=start-app]').exists()).toBe(false)
      expect(p.find('[data-testid=start-or]').exists()).toBe(false)
      expect(p.text()).not.toContain('With the app you can also')
    }
  })
  it('laptop with a download link: desktop download with the system name; clicking it starts it and says so as small print', async () => {
    urls.desktop = 'https://example.org/download'
    const assign = vi.fn()
    vi.stubGlobal('location', { ...window.location, assign })
    const w = mk()
    expect(w.get('[data-testid=start-or]').text()).toBe('or')
    expect(w.get('[data-testid=start-app]').text()).toBe('Download the app for Linux')
    expect(w.text()).toContain('The app finds screens on the same network without a code.')
    expect(w.find('[data-testid=download-note]').exists()).toBe(false)
    await w.get('[data-testid=start-app]').trigger('click')
    expect(assign).toHaveBeenCalledWith('https://example.org/download')
    const note = w.get('[data-testid=download-note]')
    expect(note.text()).toContain('Download started.')
    expect(note.classes()).toEqual(expect.arrayContaining(['cp-small', 'cp-dl'])) // small print, no green box
    vi.unstubAllGlobals()
  })
  it('"be the display": {dom}/display is a link to /display in the same tab, the rest of the sentence is text', () => {
    const w = mk()
    const p = w.get('[data-testid=be-display]')
    expect(p.text()).toBe('Should this device be the screen itself? Open cometcast.de/display.')
    const a = w.get('[data-testid=be-display-link]')
    expect(a.element.tagName).toBe('A')
    expect(a.text()).toBe('cometcast.de/display')
    expect(a.attributes('href')).toBe('/display')
    expect(a.attributes('target')).toBeUndefined()
    expect(a.classes()).toContain('cp-link')
    const de = mount(StartPage, { global: i18n('de'), props: { qrCode: null, dom: 'cast.example.org', busy: false } })
    expect(de.get('[data-testid=be-display]').text()).toBe('Soll dieses Gerät selbst der Bildschirm sein? cast.example.org/display öffnen.')
    expect(de.get('[data-testid=be-display-link]').text()).toBe('cast.example.org/display')
  })
  it('Android phone: Google Play wording; iPhone: App Store wording; no laptop-only line (with their store links)', () => {
    urls.googlePlay = 'https://play.example/app'
    urls.appStore = 'https://apps.example/app'
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
    ['noScreen', 'Screen sharing does not work in this browser', ['Show photo or file', 'Show a link']],
    ['occupied', 'Someone else is connected', ['Try again']],
    ['busy', 'The server is busy', ['Try again']],
    ['outdated', 'This page is out of date', ['Reload page']],
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
  it('noScreen offers "Get the app" only when the app can be offered', () => {
    const w = mk({ kind: 'noScreen', appOffer: true })
    expect(w.findAll('.cp-acts button').map((b) => b.text())).toEqual(['Show photo or file', 'Show a link', 'Get the app'])
  })
  it('noScreen without an app to get does not promise one (the text of the spec: nowNoApp); with an app it keeps the old text', () => {
    expect(mk({ kind: 'noScreen' }).get('[data-testid=error-now]').text()).toBe('Show a photo, a file or a link instead.')
    expect(mk({ kind: 'noScreen', appOffer: true }).get('[data-testid=error-now]').text()).toBe('Show a photo, a file or a link instead. Or get the app, then the screen works too.')
  })
  it('displayOff has two why texts: the plain one and the one for a relay that could not help', () => {
    expect(mk({ kind: 'displayOff' }).get('[data-testid=error-why]').text()).toBe('The display device is off, asleep, or Comet Cast is closed there. Your request did not arrive.')
    expect(mk({ kind: 'displayOff', whyRelay: true }).get('[data-testid=error-why]').text()).toBe('The two devices could not reach each other, and the server could not help this time.')
  })
  it('the new pages say why and what now, in the words of the spec', () => {
    const t = (kind: string) => {
      const w = mk({ kind })
      return [w.get('[data-testid=error-why]').text(), w.get('[data-testid=error-now]').text()]
    }
    expect(t('occupied')).toEqual(['This screen takes one person at a time.', 'Ask that person to disconnect, or ask the host to press End. Then try again.'])
    expect(t('busy')).toEqual(['Too many attempts from this network, or too many people at once.', 'Wait a minute, then try again.'])
    expect(t('outdated')).toEqual(['The server was updated in the meantime.', 'Reload the page.'])
    expect(t('timeout')[0]).toBe('Nobody answered your request in time.')
    expect(t('code')[0]).toBe('Usually a typo. A code has 6 characters and never contains O, 0, I or 1.')
  })
  it('"Reload page" is its own event; "Try again" on the others is "retry"', async () => {
    const w = mk({ kind: 'outdated' })
    await w.get('[data-testid=error-reload]').trigger('click')
    expect(w.emitted('reload')).toHaveLength(1)
    expect(w.emitted('retry')).toBeUndefined()
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
  it('a failure without a page of its own: "Something went wrong" with the existing text of the code as Why and "Try again"', () => {
    const w = mk({ kind: 'other', code: 'forbidden' })
    expect(w.get('[data-testid=error-title]').text()).toBe('Something went wrong')
    expect(w.get('[data-testid=error-why]').text()).toBe(en.errors.forbidden)
    expect(w.get('[data-testid=error-now]').text()).toBe('Try again. If it happens again, tell the host.')
    expect(w.get('[data-testid=error-retry]').text()).toBe('Try again')
    expect(w.get('.cp-ico').classes()).toContain('mut')
    expect(mk({ kind: 'other', code: 'no_such_code' }).get('[data-testid=error-why]').text()).toBe(en.errors.unknown)
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
    await set({ phase: 'sharing', channelOpen: true, sas: '123 456', roomName: 'Meeting room Elbe' })
    expect(w.get('[data-testid=what-page]').text()).toContain('What would you like to show?')
    expect(w.get('[data-testid=connected-to]').text()).toBe('Connected to “Meeting room Elbe”')
    // the security code is not on this page (only on "Presenting")
    expect(w.find('[data-testid=sas-line]').exists()).toBe(false)
    expect(w.text()).not.toContain('K7Q')
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
    expect(w.get('[data-testid=presenting-where]').text()).toBe('On “Meeting room Elbe”.')
    expect(w.get('[data-testid=sas-line]').text()).toBe('Security code 123 456. The screen shows the same code under your name.')
    expect(w.get('[data-testid=sas-code]').text()).toBe('123 456')
    expect(w.get('[data-testid=presenting-page]').text()).not.toContain('K7Q')
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
    [{ phase: 'error', errorCode: 'room_full' }, 'Someone else is connected'],
    [{ phase: 'error', errorCode: 'rate_limited' }, 'The server is busy'],
    [{ phase: 'error', errorCode: 'proto_mismatch' }, 'This page is out of date'],
  ] as const)('the sender session %j shows "%s"', async (patch, title) => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set(patch as Partial<SenderState>)
    expect(w.get('[data-testid=error-title]').text()).toBe(title)
  })

  it('the room name from the protocol is on the waiting page; without one the row is only the spinner, and the code is never shown instead', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'waiting_approval', check: 47, timeoutSecs: 60, roomName: 'Meeting room Elbe' })
    expect(w.get('[data-testid=waiting-room]').text()).toBe('Meeting room Elbe')
    await set({ phase: 'waiting_approval', check: 47, timeoutSecs: 60, roomName: undefined })
    expect(w.find('[data-testid=waiting-room]').exists()).toBe(false)
    expect(w.find('.cp-spin').exists()).toBe(true)
    expect(w.get('[data-testid=waiting-page]').text()).not.toContain('K7Q')
  })

  it('an old server (no room name): "Connected", no "On ..." line, never the code', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    expect(w.get('[data-testid=connected-to]').text()).toBe('Connected')
    await w.get('[data-testid=what-link]').trigger('click')
    await w.get('[data-testid=link-url]').setValue('https://example.org/x')
    await w.get('form').trigger('submit')
    expect(w.find('[data-testid=presenting-where]').exists()).toBe(false)
    expect(w.get('[data-testid=presenting-page]').text()).not.toContain('K7Q')
    expect(w.find('[data-testid=sas-line]').exists()).toBe(false) // none known yet
  })

  it('a code with an O is the error page and never asks the server; "k7q-m2w" connects', async () => {
    const w = mk()
    await w.get('[data-testid=start-code]').setValue('K7O M2W')
    expect((w.get('[data-testid=start-code]').element as HTMLInputElement).value).toBe('K7O M2W')
    await w.get('form').trigger('submit')
    expect(api.start).not.toHaveBeenCalled()
    expect(w.get('[data-testid=error-title]').text()).toBe('This code does not exist right now')
    expect(w.get('[data-testid=error-why]').text()).toBe('Usually a typo. A code has 6 characters and never contains O, 0, I or 1.')
    await w.get('[data-testid=error-retype]').trigger('click')
    await w.get('[data-testid=start-code]').setValue('k7q-m2w')
    await w.get('form').trigger('submit')
    expect(api.start).toHaveBeenCalledWith('K7QM2W', { capture: false })
  })

  it('"Reload page" on the out-of-date page reloads the page', async () => {
    const reload = vi.fn()
    vi.stubGlobal('location', { ...window.location, reload })
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'error', errorCode: 'bad_hello' })
    await w.get('[data-testid=error-reload]').trigger('click')
    expect(reload).toHaveBeenCalled()
    vi.unstubAllGlobals()
  })

  it('a shown PDF: Previous and Next side by side above Stop; "Page n of m" only when the count is known', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    await w.get('[data-testid=what-file]').trigger('click')
    const file = new File([new Uint8Array([37, 80, 68, 70])], 'a.pdf', { type: 'application/pdf' })
    const input = w.get('[data-testid=file-input]')
    Object.defineProperty(input.element, 'files', { configurable: true, value: [file] })
    await input.trigger('change')
    await flushPromises()
    expect(api.sendDocument).toHaveBeenCalled()
    const row = w.get('.cp-pages')
    expect(row.findAll('button').map((b) => b.text())).toEqual(['Previous page', 'Next page'])
    expect(w.find('[data-testid=presenting-pageof]').exists()).toBe(false)
    // the row sits above Stop
    const html = w.get('[data-testid=presenting-page]').html()
    expect(html.indexOf('cp-pages')).toBeLessThan(html.indexOf('presenting-stop'))
    await w.get('[data-testid=send-next-page]').trigger('click')
    await w.get('[data-testid=send-prev-page]').trigger('click')
    expect(api.sendCmd.mock.calls).toEqual([['next_page'], ['prev_page']])
  })

  it('the file page in a browser: heading, body and one primary button "Choose file"', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    await w.get('[data-testid=what-file]').trigger('click')
    const page = w.get('[data-testid=file-page]')
    expect(page.get('h1').text()).toBe('Show a photo or file')
    expect(page.text()).toContain('A PDF or an image from this device. It appears on the screen as soon as you pick it.')
    expect(page.get('[data-testid=file-choose]').text()).toBe('Choose file')
    expect(page.get('[data-testid=file-choose]').classes()).toContain('cp-pbtn')
    expect(page.text()).not.toContain('Recent on this device')
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
    // No store listing exists (the shipped constants are empty): the row does not promise an app (spec 5.5)
    expect(w.get('[data-testid=what-screen]').text()).toContain('Not possible in a phone browser')
    await w.get('[data-testid=what-screen]').trigger('click')
    expect(w.get('[data-testid=error-title]').text()).toBe('Screen sharing does not work in this browser')
    expect(api.shareScreen).not.toHaveBeenCalled()
  })

  it('the Screen row does not promise an app when none can be offered, and does when one can', async () => {
    ua(UA.android)
    urls.googlePlay = 'https://play.example/app'
    try {
      const w = mk('K7QM2W')
      await w.get('form').trigger('submit')
      await set({ phase: 'sharing', channelOpen: true })
      expect(w.get('[data-testid=what-screen]').text()).toContain('On a phone only with the app')
      await w.get('[data-testid=what-screen]').trigger('click')
      expect(w.get('[data-testid=error-now]').text()).toContain('Or get the app')
      expect(w.find('[data-testid=error-getApp]').exists()).toBe(true)
    } finally {
      urls.googlePlay = ''
    }
  })

  it('capture_denied has no page: back to "What to show"', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    expect(w.find('[data-testid=what-page]').exists()).toBe(true)
    await set({ phase: 'error', errorCode: 'capture_denied', channelOpen: false })
    expect(w.find('[data-testid=error-page]').exists()).toBe(false)
  })

  it('unlisted failures show "Something went wrong" with the text of the code; Try again connects again', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'error', errorCode: 'wrong_role' })
    expect(w.get('[data-testid=error-title]').text()).toBe('Something went wrong')
    expect(w.get('[data-testid=error-why]').text()).toBe(en.errors.wrong_role)
    api.start.mockClear()
    await w.get('[data-testid=error-retry]').trigger('click')
    expect(api.start).toHaveBeenCalledWith('K7QM2W', { capture: false })
  })

  it('peer_failed: "busy" when the STUN/TURN list was refused, "displayOff" with its own why when it failed, plain otherwise', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'error', errorCode: 'peer_failed', iceState: { kind: 'refused' } })
    expect(w.get('[data-testid=error-title]').text()).toBe('The server is busy')
    await set({ phase: 'error', errorCode: 'peer_failed', iceState: { kind: 'unreachable', timedOut: true } })
    expect(w.get('[data-testid=error-title]').text()).toBe('The screen does not answer')
    expect(w.get('[data-testid=error-why]').text()).toContain('the server could not help this time')
    await set({ phase: 'error', errorCode: 'peer_failed', iceState: { kind: 'none' } })
    expect(w.get('[data-testid=error-why]').text()).toContain('The display device is off')
    await set({ phase: 'error', errorCode: 'peer_failed', iceState: undefined })
    expect(w.get('[data-testid=error-why]').text()).toContain('The display device is off')
  })

  it('a PDF shows "Page n of m" from the display and disables Previous on page 1', async () => {
    const w = mk('K7QM2W')
    await w.get('form').trigger('submit')
    await set({ phase: 'sharing', channelOpen: true })
    await w.get('[data-testid=what-file]').trigger('click')
    const input = w.get('input[type=file]')
    Object.defineProperty(input.element, 'files', { value: [new File([new Uint8Array(8)], 'a.pdf', { type: 'application/pdf' })], configurable: true })
    await input.trigger('change')
    await flushPromises()
    expect(w.find('[data-testid=presenting-page]').exists()).toBe(true)
    expect(w.find('[data-testid=presenting-pageof]').exists()).toBe(false) // unknown until the display reports it
    await set({ pdfPage: { page: 1, pages: 7 } })
    expect(w.get('[data-testid=presenting-pageof]').text()).toBe('Page 1 of 7')
    expect(w.get('[data-testid=send-prev-page]').attributes('disabled')).toBeDefined()
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

describe('PresentingPage: page count', () => {
  it('shows "Page n of m" above the buttons when it is known', async () => {
    const { default: PresentingPage } = await import('../src/components/presenter/PresentingPage.vue')
    const w = mount(PresentingPage, { global: i18n(), props: { kind: 'file', keepBar: false, pdf: true, page: 3, total: 12 } })
    expect(w.get('[data-testid=presenting-pageof]').text()).toBe('Page 3 of 12')
    expect(w.find('[data-testid=presenting-where]').exists()).toBe(false) // no room name: no "On ..." line
    const de = mount(PresentingPage, { global: i18n('de'), props: { kind: 'file', keepBar: false, pdf: true, page: 3, total: 12, room: 'Raum 204' } })
    expect(de.get('[data-testid=presenting-pageof]').text()).toBe('Seite 3 von 12')
    expect(de.get('[data-testid=presenting-where]').text()).toBe('Auf „Raum 204“.')
  })
})
