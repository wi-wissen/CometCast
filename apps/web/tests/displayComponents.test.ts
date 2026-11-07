// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createAppI18n } from '../src/i18n'
import RequestDialog from '../src/components/display/RequestDialog.vue'
import SettingsDialog from '../src/components/display/SettingsDialog.vue'
import SetupWizard from '../src/components/display/SetupWizard.vue'
import IdleScreen from '../src/components/display/IdleScreen.vue'
import { REQUEST_GUARD_MS } from '../src/core/joinGuard'
import { resetRelayState } from '../src/composables/useRelay'
import type { JoinRequest } from '../src/core/receiverSession'

const global = { plugins: [createAppI18n('en')] }
const req = (id: number, name = `Person ${id}`): JoinRequest => ({ id, name, check: 40 + id, receivedAt: Date.now() })

beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] }))
afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
  localStorage.clear()
})

describe('join request dialog (4.4)', () => {
  const mk = (props: Record<string, unknown> = {}) => mount(RequestDialog, { attachTo: document.body, global, props: { request: req(1, 'Lena'), more: 0, ...props } })

  it('is an alertdialog with the name in the label, the check number, and Yes focused', async () => {
    const w = mk()
    await nextTick()
    const d = w.get('[role=alertdialog]')
    expect(d.attributes('aria-label')).toBe('Request: “Lena”')
    expect(w.get('[data-testid=request-name]').text()).toBe('“Lena”')
    expect(w.get('[data-testid=request-sub]').text()).toBe('wants to connect')
    expect(w.get('[data-testid=request-check]').text()).toBe('41')
    expect(document.activeElement).toBe(w.get('[data-testid=request-yes]').element)
    // Order: Yes, No, then No and block
    const labels = w.findAll('button').map((b) => b.text())
    expect(labels).toEqual(['Yes', 'No', 'No and block'])
  })

  it('ignores presses for 1 s after it appeared, then one press answers', async () => {
    const w = mk()
    await w.get('[data-testid=request-yes]').trigger('click')
    await w.get('[data-testid=request-block]').trigger('click')
    expect(w.emitted('allow')).toBeUndefined()
    expect(w.emitted('block')).toBeUndefined()
    vi.advanceTimersByTime(REQUEST_GUARD_MS + 150)
    await nextTick()
    await w.get('[data-testid=request-yes]').trigger('click')
    expect(w.emitted('allow')).toHaveLength(1)
    await w.get('[data-testid=request-no]').trigger('click')
    expect(w.emitted('deny')).toHaveLength(1)
    await w.get('[data-testid=request-block]').trigger('click')
    expect(w.emitted('block')).toHaveLength(1)
  })

  it('starts the guard again when it switches to the next request, and refocuses Yes', async () => {
    const w = mk()
    vi.advanceTimersByTime(REQUEST_GUARD_MS + 150)
    await nextTick()
    ;(w.get('[data-testid=request-no]').element as HTMLElement).focus()
    await w.setProps({ request: req(2, 'Ben') })
    await nextTick()
    expect(w.get('[data-testid=request-name]').text()).toBe('“Ben”')
    expect(document.activeElement).toBe(w.get('[data-testid=request-yes]').element)
    await w.get('[data-testid=request-yes]').trigger('click')
    expect(w.emitted('allow')).toBeUndefined()
    vi.advanceTimersByTime(REQUEST_GUARD_MS + 150)
    await nextTick()
    await w.get('[data-testid=request-yes]').trigger('click')
    expect(w.emitted('allow')).toHaveLength(1)
  })

  it('shows the queue, "+2 waiting"', () => {
    expect(mk({ more: 2 }).get('[data-testid=request-queue]').text()).toBe('+2 waiting')
    expect(mk({ more: 0 }).find('[data-testid=request-queue]').exists()).toBe(false)
  })

  it('names: plain text, cleaned, cut to 18 characters + …, full name only in the accessible label; empty = No name', () => {
    const long = 'Anna-Maria Schmid-Wolfgang <b>x</b>'
    const w = mk({ request: req(1, long) })
    expect(w.get('[data-testid=request-name]').text()).toBe('“Anna-Maria Schmid-…”')
    expect(w.get('[data-testid=request-name]').html()).not.toContain('<b>')
    expect(w.get('[role=alertdialog]').attributes('aria-label')).toContain('<b>x</b>')
    const messy = mk({ request: req(1, '  A‮​  B  \n C ') })
    expect(messy.get('[data-testid=request-name]').text()).toBe('“A B C”')
    expect(mk({ request: req(1, ' ​ ') }).get('[data-testid=request-name]').text()).toBe('No name')
  })
})

describe('settings dialog (4.3)', () => {
  const mk = (mode: 'open' | 'confirm' | 'locked' = 'confirm', appearance: 'dark' | 'light' | 'oled' | 'auto' = 'dark') =>
    mount(SettingsDialog, { attachTo: document.body, global, props: { mode, appearance } })

  it('has the join mode and the appearance, radio groups, and describes the current mode', async () => {
    const w = mk('confirm')
    await nextTick()
    expect(w.findAll('[role=radiogroup]')).toHaveLength(2)
    expect(w.findAll('[role=radio]').map((b) => b.text())).toEqual(['Open', 'Approval on', 'Locked', 'Dark', 'Light', 'OLED black', 'Automatic'])
    expect(w.get('[data-testid=settings-mode-confirm]').attributes('aria-checked')).toBe('true')
    expect(w.get('[data-testid=settings-mode-desc]').text()).toBe('You let each person in one by one.')
    expect(w.get('[data-testid=settings-look-dark]').attributes('aria-checked')).toBe('true')
    expect(w.get('[data-testid=settings-close]').text()).toBe('Close')
  })
  it('starts with the focus on the tile of the current join mode', async () => {
    const w = mk('locked')
    await nextTick()
    expect(document.activeElement).toBe(w.get('[data-testid=settings-mode-locked]').element)
  })
  it('changes apply at once (events), there is no Save button', async () => {
    const w = mk()
    await w.get('[data-testid=settings-mode-locked]').trigger('click')
    await w.get('[data-testid=settings-look-oled]').trigger('click')
    await w.get('[data-testid=settings-close]').trigger('click')
    expect(w.emitted('mode')).toEqual([['locked']])
    expect(w.emitted('appearance')).toEqual([['oled']])
    expect(w.emitted('close')).toHaveLength(1)
    expect(w.text()).not.toMatch(/save/i)
  })
  it('German (Sie) texts', async () => {
    const w = mount(SettingsDialog, { global: { plugins: [createAppI18n('de')] }, props: { mode: 'confirm', appearance: 'auto' } })
    expect(w.get('[data-testid=settings-mode-desc]').text()).toBe('Sie lassen jede Person einzeln zu.')
    expect(w.text()).toContain('Mit Freigabe')
    expect(w.text()).toContain('OLED-Schwarz')
  })
})

describe('setup wizard (4.1)', () => {
  beforeEach(() => resetRelayState())
  const mk = () => mount(SetupWizard, { attachTo: document.body, global, props: { theme: 'dark' as const } })

  it('runs through all five screens with the focus where the table says and stores what was chosen', async () => {
    const w = mk()
    await nextTick()
    expect(w.find('[data-testid=wz-steps]').exists()).toBe(false) // hidden on the welcome screen
    expect(document.activeElement).toBe(w.get('[data-testid=wz-start]').element)
    await w.get('[data-testid=wz-start]').trigger('click')
    await nextTick()
    // Step 1: five chips, the first preselected; focus on Next
    expect(w.get('[data-testid=wz-steps]').text()).toBe('Step 1 of 4')
    const chips = w.findAll('.cd-chip')
    expect(chips.map((c) => c.text())).toEqual(['Meeting room Elbe', 'Living room', 'Clubhouse', 'Workshop', 'Room 204'])
    expect(chips[0].classes()).toContain('on')
    expect(document.activeElement).toBe(w.get('[data-testid=wz-next]').element)
    await chips[2].trigger('click')
    expect((w.get('[data-testid=wz-room]').element as HTMLInputElement).value).toBe('Clubhouse')
    await w.get('[data-testid=wz-room]').setValue('Room 12')
    await w.get('[data-testid=wz-next]').trigger('click')
    await nextTick()
    // Step 2: Open preselected and focused (D1)
    expect(w.get('[data-testid=wz-mode-open]').attributes('aria-checked')).toBe('true')
    expect(document.activeElement).toBe(w.get('[data-testid=wz-mode-open]').element)
    await w.get('[data-testid=wz-mode-confirm]').trigger('click')
    await w.get('[data-testid=wz-next]').trigger('click')
    await nextTick()
    // Step 3: the address, focus on "Looks right"
    expect(w.get('[data-testid=wz-domain]').text()).not.toBe('')
    expect(w.text()).toContain('Default server. Nothing to set up.')
    expect(document.activeElement).toBe(w.get('[data-testid=wz-ok]').element)
    await w.get('[data-testid=wz-ok]').trigger('click')
    await nextTick()
    // Step 4: done
    expect(w.text()).toContain('Done.')
    expect(w.text()).toContain('“Settings” at the bottom right')
    expect(document.activeElement).toBe(w.get('[data-testid=wz-finish]').element)
    await w.get('[data-testid=wz-finish]').trigger('click')
    const [setup] = w.emitted('finish')![0] as [{ room: string; mode: string; server: string }]
    expect(setup).toMatchObject({ room: 'Room 12', mode: 'confirm' })
    expect(setup.server).toMatch(/^wss?:\/\/.*\/ws$/)
  })

  it('Back goes one step back; on the welcome screen it does nothing', async () => {
    const w = mk()
    await w.get('[data-testid=wz-start]').trigger('click')
    await nextTick()
    await w.get('[data-testid=wz-back]').trigger('click')
    expect(w.find('[data-testid=wz-start]').exists()).toBe(true)
    ;(w.vm as unknown as { back(): void }).back()
    await nextTick()
    expect(w.find('[data-testid=wz-start]').exists()).toBe(true)
  })

  it('"Other address" with one known address opens a text field and checks it', async () => {
    const w = mk()
    for (let i = 0; i < 3; i++) {
      await w.get('[data-testid=wz-start], [data-testid=wz-next]').trigger('click')
      await nextTick()
    }
    await w.get('[data-testid=wz-change]').trigger('click')
    const f = w.get('[data-testid=wz-addr]')
    await f.setValue('ws://public.example.org')
    await f.trigger('keydown', { key: 'Enter' })
    expect(w.get('[data-testid=wz-addr-error]').text()).toContain('wss://')
    await f.setValue('cast.example.org')
    await f.trigger('keydown', { key: 'Enter' })
    expect(w.get('[data-testid=wz-domain]').text()).toBe('cast.example.org')
    expect(w.text()).toContain('Found via the company network')
  })
})

describe('idle screen (4.2)', () => {
  const props = { theme: 'dark' as const, room: 'Meeting room Elbe', mode: 'open' as const, code: 'K7QM2W', dom: 'cometcast.de', offline: false, dimmed: false, scheme: 'https' as const }
  const mk = (p: Record<string, unknown> = {}) => mount(IdleScreen, { global, props: { ...props, ...p } })

  it('shows room, mode, the two steps, the QR code with its URL, the app line and Settings', () => {
    const w = mk()
    expect(w.get('[data-testid=room-name]').text()).toBe('Meeting room Elbe')
    expect(w.get('[data-testid=mode-pill]').text()).toBe('Open')
    expect(w.text()).toContain('Open in your browser')
    expect(w.get('[data-testid=address]').text()).toBe('cometcast.de')
    expect(w.text()).toContain('Enter this code')
    expect(w.get('[data-testid=room-code]').text()).toBe('K7Q M2W')
    expect(w.get('svg[role=img]').attributes('aria-label')).toBe('QR: https://cometcast.de/K7QM2W')
    expect(w.get('[data-testid=qr-caption]').text()).toBe('Or scan')
    expect(w.text()).toContain('No download needed. Prefer an app: App Store · Google Play · cometcast.de/app')
    expect(w.get('[data-testid=settings-open]').text()).toBe('Settings')
  })
  it('Approval on only changes the pill', () => {
    const w = mk({ mode: 'confirm' })
    expect(w.get('[data-testid=mode-pill]').text()).toBe('Approval on')
    expect(w.find('[data-testid=locked-note]').exists()).toBe(false)
  })
  it('Locked: code at 35 %, the note, QR at 25 % and its caption', () => {
    const w = mk({ mode: 'locked' })
    expect(w.get('[data-testid=room-code]').classes()).toContain('dim')
    expect(w.get('[data-testid=locked-note]').text()).toBe('Locked right now.')
    expect(w.get('[data-testid=qr]').classes()).toContain('dim')
    expect(w.get('[data-testid=qr-caption]').text()).toBe('Locked')
  })
  it('offline: the box replaces steps 1 and 2, the QR code is dimmed', () => {
    const w = mk({ offline: true })
    expect(w.get('[data-testid=offline]').text()).toContain('No connection to cometcast.de')
    expect(w.find('[data-testid=room-code]').exists()).toBe(false)
    expect(w.get('[data-testid=qr]').classes()).toContain('dim')
  })
  it('dimming is an overlay that is switched by the prop', async () => {
    const w = mk()
    expect(w.get('[data-testid=dim]').classes()).not.toContain('on')
    await w.setProps({ dimmed: true })
    expect(w.get('[data-testid=dim]').classes()).toContain('on')
  })
  it('the QR code of a plain local server uses http', () => {
    expect(mk({ scheme: 'http', dom: '127.0.0.1:8443' }).get('svg[role=img]').attributes('aria-label')).toBe('QR: http://127.0.0.1:8443/K7QM2W')
  })
})
