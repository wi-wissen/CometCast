// SPDX-License-Identifier: GPL-3.0-or-later
// The pure parts of the fix round (docs/design/fix-list-web.md).
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, vi } from 'vitest'
import { nextTick } from 'vue'
import { createAppI18n } from '../src/i18n'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'
import deSie from '../src/locales/de-sie.json'
import { ringHidden } from '../src/core/focusRing'
import { codeFieldText, codeFromField, CODE_FIELD_MAX, isValidRoomCode } from '../src/core/roomCode'
import { splitSlot } from '../src/core/textSlot'
import { anyAppLink, appUrlFor } from '../src/config/stores'
import {
  connectionView,
  offlineCause,
  FAILED_ATTEMPTS_FOR_BOX,
  OFFLINE_AFTER_SILENCE_MS,
  retryDelayMs,
} from '../src/core/connectionView'
import { ControlsReveal } from '../src/core/controlsReveal'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
const read = (rel: string) => readFileSync(fileURLToPath(new URL(rel, import.meta.url)), 'utf8')
const presenterCss = read('../src/styles/presenter.css')
const displayCss = read('../src/styles/display.css')
const toolbarSrc = read('../src/components/ReceiverToolbar.vue')
import ReceiverToolbar from '../src/components/ReceiverToolbar.vue'
import AnnotationLayer from '../src/components/AnnotationLayer.vue'
import { AnnotationModel } from '../src/core/annotations'
import { initialToolbar, toggleTool } from '../src/core/toolbarState'
import RelayConfirm from '../src/components/RelayConfirm.vue'
import RequestDialog from '../src/components/display/RequestDialog.vue'
import { ensureRelayTrusted, initRelay, resetRelayState } from '../src/composables/useRelay'

const get = (o: unknown, k: string): string => k.split('.').reduce<any>((x, p) => x?.[p], o)
const render = (l: any, key: string, vars: Record<string, string> = {}) =>
  get(l, key).replace(/\{(\w+)\}/g, (_: string, n: string) => vars[n] ?? `{${n}}`)

describe('the device type is out of the strings (decision 13.3)', () => {
  it('"wants to connect" and "“Lena” is showing" come from the locale values alone, in EN, DE du and DE Sie', () => {
    const q = (name: string) => `“${name}”`
    expect(render(en, 'request.subtitle')).toBe('wants to connect')
    expect(render(de, 'request.subtitle')).toBe('möchte sich verbinden')
    expect(render(deSie, 'request.subtitle')).toBe('möchte sich verbinden')
    expect(render(en, 'present.byline', { name: q('Lena') })).toBe('“Lena” is showing')
    expect(render(de, 'present.byline', { name: '„Lena“' })).toBe('„Lena“ zeigt')
    expect(render(deSie, 'present.byline', { name: '„Lena“' })).toBe('„Lena“ zeigt')
    for (const l of [en, de, deSie]) {
      expect(get(l, 'request.subtitle')).not.toContain('{device}')
      expect(get(l, 'present.byline')).not.toContain('{device}')
      expect(get(l, 'request.subtitle')).not.toContain('·')
      expect(get(l, 'present.byline')).not.toContain('·')
    }
  })
  it('the request dialog shows the subtitle as the locale gives it', () => {
    for (const [loc, text] of [['en', 'wants to connect'], ['de', 'möchte sich verbinden']] as const) {
      const w = mount(RequestDialog, { global: { plugins: [createAppI18n(loc)] }, props: { request: { id: 1, name: 'Lena', check: 41, receivedAt: 0 }, more: 0 } })
      expect(w.get('[data-testid=request-sub]').text()).toBe(text)
      w.unmount()
    }
  })
  it('the security code line is "Security code {sas}" / "Sicherheitscode {sas}"', () => {
    expect(render(en, 'present.sas', { sas: '171 852' })).toBe('Security code 171 852')
    expect(render(de, 'present.sas', { sas: '171 852' })).toBe('Sicherheitscode 171 852')
    expect(get(en, 'sas')).toBeUndefined() // the old "connection check code" strings are gone
  })
})

describe('code field helpers (spec 5.2)', () => {
  it('shows every typed character in uppercase, at most 9, and never drops one', () => {
    expect(CODE_FIELD_MAX).toBe(9)
    expect(codeFieldText('k7o-m2w')).toBe('K7O-M2W')
    expect(codeFieldText('abcdefghijk')).toBe('ABCDEFGHI')
    expect(codeFieldText('0O1I l')).toBe('0O1I L')
  })
  it('on submit spaces and dashes go; anything not 6 characters of the alphabet is invalid', () => {
    expect(codeFromField('k7q-m2w')).toBe('K7QM2W')
    expect(isValidRoomCode(codeFromField('k7q-m2w'))).toBe(true)
    expect(isValidRoomCode(codeFromField('K7O M2W'))).toBe(false) // O
    expect(isValidRoomCode(codeFromField('K7Q M2W4'))).toBe(false) // length
    expect(isValidRoomCode(codeFromField('K7Q 0 2W'))).toBe(false) // 0
    expect(isValidRoomCode(codeFromField('  k7q m2w  '))).toBe(true)
  })
})

describe('text slots', () => {
  it('cuts a translated sentence where the value belongs, in every language', () => {
    expect(splitSlot((m) => `Open ${m}/display.`, '/display'.length)).toEqual({ before: 'Open ', after: '.' })
    expect(splitSlot((m) => `${m}/display öffnen.`, '/display'.length)).toEqual({ before: '', after: ' öffnen.' })
    expect(splitSlot((m) => `Security code ${m}`)).toEqual({ before: 'Security code ', after: '' })
    expect(splitSlot(() => 'no slot')).toEqual({ before: 'no slot', after: '' })
  })
})

describe('app offers only when they exist (spec 5.2, 4.2)', () => {
  const none = { googlePlay: '', appStore: '', desktop: '' }
  it('an empty address is no offer; the placeholders are empty today', () => {
    expect(appUrlFor('google-play', none)).toBe('')
    expect(appUrlFor('app-store', none)).toBe('')
    expect(appUrlFor(null, none)).toBe('')
    expect(anyAppLink(none)).toBe(false)
    expect(anyAppLink()).toBe(false) // the shipped constants
  })
  it('each platform has its own address', () => {
    const u = { googlePlay: 'https://p', appStore: '', desktop: 'https://d' }
    expect(appUrlFor('google-play', u)).toBe('https://p')
    expect(appUrlFor('app-store', u)).toBe('')
    expect(appUrlFor(null, u)).toBe('https://d')
    expect(anyAppLink(u)).toBe(true)
  })
})

describe('what the idle screen shows about the connection (spec 4, 4.2)', () => {
  const base = { code: null as string | null, phase: 'connecting', failed: false, failedAttempts: 0, cause: 'network' as const, withoutCodeMs: 0 }
  it('a code means online', () => {
    expect(connectionView({ ...base, code: 'K7QM2W', phase: 'waiting_sender' })).toBe('online')
  })
  it('a lost connection: connecting until the retry at 3 s has failed, then the box (the code is gone at once even if the old one is still known)', () => {
    const lost = { ...base, phase: 'error', failed: true }
    expect(connectionView({ ...lost, code: 'K7QM2W', failedAttempts: 1 })).toBe('connecting')
    expect(connectionView({ ...lost, failedAttempts: FAILED_ATTEMPTS_FOR_BOX - 1 })).toBe('connecting')
    expect(connectionView({ ...lost, failedAttempts: FAILED_ATTEMPTS_FOR_BOX })).toBe('offline')
    // a retry that is running does not make the box flicker away
    expect(connectionView({ ...base, failed: true, failedAttempts: FAILED_ATTEMPTS_FOR_BOX })).toBe('offline')
    expect(connectionView({ ...base, failed: true, failedAttempts: 2 })).toBe('connecting')
  })
  it('busy refusals and an outdated page show the box at once (no 3 s wait)', () => {
    expect(connectionView({ ...base, phase: 'error', failed: true, failedAttempts: 1, cause: 'busy' })).toBe('offline')
    expect(connectionView({ ...base, phase: 'connecting', failed: true, failedAttempts: 1, cause: 'busy' })).toBe('offline')
    expect(connectionView({ ...base, phase: 'error', failed: true, failedAttempts: 1, cause: 'outdated' })).toBe('offline')
  })
  it('no code and no failure: 8 s of connecting, then the box', () => {
    expect(connectionView({ ...base, withoutCodeMs: OFFLINE_AFTER_SILENCE_MS - 1 })).toBe('connecting')
    expect(connectionView({ ...base, withoutCodeMs: OFFLINE_AFTER_SILENCE_MS })).toBe('offline')
  })
  it('the cause picks the body and the retry: network 1 s, 2 s, 3 s after the loss then every 4 s; busy 30 s; outdated never', () => {
    for (const c of ['rate_limited', 'banned', 'server_full', 'forbidden']) {
      expect(offlineCause(c)).toBe('busy')
      expect(retryDelayMs(c)).toBe(30_000)
    }
    for (const c of ['proto_mismatch', 'bad_hello']) {
      expect(offlineCause(c)).toBe('outdated')
      expect(retryDelayMs(c)).toBeNull()
    }
    for (const c of ['connection_lost', 'peer_failed', null, 'altcha_failed']) {
      expect(offlineCause(c)).toBe('network')
      // the n-th retry is due at 1 s, 2 s, 3 s, 7 s, 11 s ... after the loss
      expect([1, 2, 3, 4, 5, 6].map((n) => retryDelayMs(c, n, 0))).toEqual([1000, 2000, 3000, 7000, 11_000, 15_000])
    }
    // measured from the loss: a failure that took 300 ms leaves 700 ms to the first retry; a late failure retries at once
    expect(retryDelayMs('connection_lost', 1, 300)).toBe(700)
    expect(retryDelayMs('connection_lost', 2, 5000)).toBe(0)
  })
})

describe('focus ring rule (spec 7.2)', () => {
  it('shown on load, hidden by the first mouse or touch input, shown again by the next key', () => {
    let hidden = false
    hidden = ringHidden(hidden, 'pointer')
    expect(hidden).toBe(true)
    hidden = ringHidden(hidden, 'pointer')
    expect(hidden).toBe(true)
    hidden = ringHidden(hidden, 'key')
    expect(hidden).toBe(false)
  })
})

describe('controls reveal: strokes count as input', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())
  it('a stroke (poke) shows the controls and restarts the 5 s', () => {
    const seen: boolean[] = []
    const c = new ControlsReveal((v) => seen.push(v))
    expect(c.poke()).toBe(true)
    vi.advanceTimersByTime(4000)
    c.poke() // drawing goes on
    vi.advanceTimersByTime(4000)
    expect(c.visible).toBe(true)
    vi.advanceTimersByTime(1001)
    expect(c.visible).toBe(false)
    expect(seen).toEqual([true, false])
  })
  it('the annotation layer reports a stroke as input', async () => {
    vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
    const model = new AnnotationModel()
    const w = mount(AnnotationLayer, { props: { model, toolbar: toggleTool(initialToolbar(), 'pen'), label: 'x' } })
    const canvas = w.get('canvas').element as HTMLCanvasElement
    canvas.setPointerCapture = () => undefined
    canvas.getContext = (() => null) as never
    const ev = (type: string) => Object.assign(new Event(type, { bubbles: true, cancelable: true }), { pointerId: 1, pointerType: 'mouse', button: 0, clientX: 5, clientY: 5 })
    canvas.dispatchEvent(ev('pointerdown'))
    canvas.dispatchEvent(ev('pointermove'))
    expect(w.emitted('input')?.length).toBeGreaterThanOrEqual(2)
    vi.unstubAllGlobals()
  })
})

describe('the toolbar (spec 3.5): no grip, no collapse, hidden at rest', () => {
  const props = { state: initialToolbar(), canUndo: false, hasStrokes: false, canFreeze: false, fullscreen: false, fullscreenAvailable: true }
  it('has the tools and nothing to move or collapse', () => {
    const w = mount(ReceiverToolbar, { global: { plugins: [createAppI18n('en')] }, props: { ...props, shown: true } })
    for (const id of ['pen', 'marker', 'eraser', 'color-red', 'size-0', 'undo', 'clear', 'laser', 'freeze', 'black', 'fullscreen']) {
      expect(w.find(`[data-testid=tb-${id}]`).exists(), id).toBe(true)
    }
    expect(w.find('[data-testid=toolbar-grip]').exists()).toBe(false)
    expect(w.find('[data-testid=tb-collapse]').exists()).toBe(false)
    expect(w.find('[data-testid=toolbar-dot]').exists()).toBe(false)
    expect(w.get('[data-testid=toolbar-root]').classes()).not.toContain('hid')
  })
  it('hidden: faded, not focusable (inert), until the controls are shown', async () => {
    const w = mount(ReceiverToolbar, { global: { plugins: [createAppI18n('en')] }, props: { ...props, shown: false } })
    const root = w.get('[data-testid=toolbar-root]')
    expect(root.classes()).toContain('hid')
    expect(root.attributes('inert')).toBeDefined()
    expect(root.attributes('aria-hidden')).toBe('true')
    await w.setProps({ shown: true })
    expect(root.classes()).not.toContain('hid')
    expect(root.attributes('inert')).toBeUndefined()
  })
  it('tokens only: top 12 centred, 46 high, 2 px --line border, panel fill 92 %, selected tool 2 px --acc, icons --fg', () => {
    const css = toolbarSrc.slice(toolbarSrc.indexOf('<style'))
    expect(css).toMatch(/top:\s*12px/)
    expect(css).toMatch(/height:\s*46px/)
    expect(css).toMatch(/border:\s*2px solid var\(--line\)/)
    expect(css).toContain('color-mix(in srgb, var(--panel) 92%, transparent)')
    expect(css).toMatch(/aria-pressed='true'\]\s*\{\s*border-color:\s*var\(--acc\)/)
    expect(css).toMatch(/color:\s*var\(--fg\)/)
    expect(css).toMatch(/transition:\s*opacity 0\.4s/)
    // none of the old hard-coded colours of the dark toolbar
    for (const old of ['#1a1d24', 'rgba(26, 29, 36', '#e9b04a', '#2a1f00', '#eef0f4']) expect(css).not.toContain(old)
    expect(css).not.toContain('rgba(26')
  })
})

describe('presenter colours (fix 14)', () => {
  const hex = (css: string, name: string, scope: 'light' | 'dark') => {
    const dark = css.indexOf('@media (prefers-color-scheme: dark)')
    const part = scope === 'light' ? css.slice(0, dark) : css.slice(dark)
    const m = new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`).exec(part)
    if (!m) throw new Error(`no --${name} in ${scope}`)
    return m[1]
  }
  const lum = (h: string) => {
    const c = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
  }
  const ratio = (a: string, b: string) => {
    const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p)
    return (x + 0.05) / (y + 0.05)
  }
  it('the disabled button has the values of the spec and is readable (>= 4.5:1), light and dark', () => {
    expect(hex(presenterCss, 'p-disabled', 'light')).toBe('#dde1e8')
    expect(hex(presenterCss, 'p-ondisabled', 'light')).toBe('#4a5160')
    expect(hex(presenterCss, 'p-disabled', 'dark')).toBe('#2a2f38')
    expect(hex(presenterCss, 'p-ondisabled', 'dark')).toBe('#9ea6b3')
    for (const s of ['light', 'dark'] as const) {
      expect(ratio(hex(presenterCss, 'p-disabled', s), hex(presenterCss, 'p-ondisabled', s)), s).toBeGreaterThanOrEqual(4.5)
    }
  })
  it('the dark icon discs are hex tints', () => {
    expect(hex(presenterCss, 'p-error-bg', 'dark')).toBe('#342a2c')
    expect(hex(presenterCss, 'p-ok-bg', 'dark')).toBe('#21312f')
    expect(hex(presenterCss, 'p-warn-bg', 'dark')).toBe('#332f26')
  })
})

describe('display CSS rules of the fix round', () => {
  it('OLED black: selected tiles keep a black fill, the check box has a 1 px --line border', () => {
    expect(displayCss).toMatch(/\.cd\[data-theme='oled'\] \.cd-mt\.on[\s\S]*?background:\s*#000000/)
    expect(displayCss).toMatch(/\.cd\[data-theme='oled'\] \.cd-chk\s*\{\s*border:\s*1px solid var\(--line\)/)
  })
  it('"No and block" is 34 high and centred, not stretched', () => {
    const ghost = /\.cd-ghost\s*\{([^}]*)\}/.exec(displayCss)![1]
    expect(ghost).toMatch(/height:\s*34px/)
    expect(ghost).toMatch(/align-self:\s*center/)
    expect(/\.cd-acts\s*\{([^}]*)\}/.exec(displayCss)![1]).toMatch(/align-items:\s*center/)
  })
  it('the focus ring follows the pointer rule', () => {
    expect(displayCss).toContain('.cd:not(.cd-pointer) :is(')
  })
})

describe('relay confirmation (fix 9)', () => {
  const cfg = (async () => new Response('{"relays":["wss://relay.cometcast.de/ws"]}')) as unknown as typeof fetch
  async function ask(variant: 'display' | 'presenter') {
    resetRelayState()
    history.replaceState(null, '', `/?server=${encodeURIComponent('wss://crafted.example.org')}`)
    await initRelay({ fetchImpl: cfg })
    const w = mount(RelayConfirm, { attachTo: document.body, global: { plugins: [createAppI18n('en')] }, props: { variant } })
    const r = ensureRelayTrusted()
    await nextTick()
    await nextTick()
    return { w, r }
  }
  afterEach(() => {
    history.replaceState(null, '', '/')
    resetRelayState()
    document.body.innerHTML = ''
  })
  it('on /display it is the dialog card on the scaled canvas over plain --bg with only the logo; Cancel is secondary and has the focus, Use is primary; row [Cancel, Use]', async () => {
    const { w, r } = await ask('display')
    expect(w.get('[data-testid=relay-confirm-backdrop]').classes()).toEqual(expect.arrayContaining(['cd', 'cd-relay']))
    expect(w.find('[data-testid=relay-confirm-backdrop] .cd-canvas').exists()).toBe(true) // 960 x 540 reference px, scaled
    expect(w.findAll('.cd-modal')).toHaveLength(0) // no scrim: there is nothing behind it
    expect(w.findAll('img')).toHaveLength(1) // the logo, nothing else of the idle screen
    expect(w.find('[data-testid=qr]').exists()).toBe(false)
    expect(w.get('[data-testid=relay-confirm]').classes()).toContain('cd-wz')
    const cancel = w.get('[data-testid=relay-confirm-cancel]')
    const use = w.get('[data-testid=relay-confirm-use]')
    expect(cancel.classes()).not.toContain('p')
    expect(use.classes()).toContain('p')
    expect(document.activeElement).toBe(cancel.element)
    expect(w.findAll('[data-row="0"]').map((b) => b.attributes('data-testid'))).toEqual(['relay-confirm-cancel', 'relay-confirm-use'])
    expect(w.get('#relay-confirm-title').text()).toBe('Use this relay?')
    expect(w.get('[data-testid=relay-confirm-host]').text()).toBe('crafted.example.org')
    // arrows move the focus; Escape (Back) = Cancel
    await w.get('[data-testid=relay-confirm-backdrop]').trigger('keydown', { key: 'ArrowRight' })
    expect(document.activeElement).toBe(use.element)
    await w.get('[data-testid=relay-confirm-backdrop]').trigger('keydown', { key: 'Escape' })
    expect(await r).toBe(false)
    w.unmount()
  })
  it('on the presenter pages it is its own page in the error-page layout: icon disc, heading, body, host line, small print, [Cancel, Use]', async () => {
    const { w, r } = await ask('presenter')
    expect(w.get('[data-testid=relay-confirm-backdrop]').classes()).toContain('cp')
    expect(w.get('[data-testid=relay-confirm]').classes()).toEqual(expect.arrayContaining(['cp-page']))
    expect(w.find('.cp-over').exists()).toBe(false) // not a card over another page
    expect(w.get('.cp-ico').classes()).toContain('warn')
    expect(w.get('h1').classes()).toContain('cp-h')
    expect(w.get('[data-testid=relay-confirm-host]').classes()).toContain('mono')
    expect(w.get('.cp-small').text()).toContain('operator of this relay')
    expect(w.get('[data-testid=relay-confirm-cancel]').classes()).toContain('cp-sbtn')
    expect(w.get('[data-testid=relay-confirm-use]').classes()).toContain('cp-pbtn')
    expect(document.activeElement).toBe(w.get('[data-testid=relay-confirm-cancel]').element)
    // Cancel is first in the focus order (and on top on a phone, left on a laptop)
    expect(w.findAll('button').map((b) => b.attributes('data-testid'))).toEqual(['relay-confirm-cancel', 'relay-confirm-use'])
    await w.get('[data-testid=relay-confirm-use]').trigger('click')
    expect(await r).toBe(true)
    w.unmount()
  })
})
