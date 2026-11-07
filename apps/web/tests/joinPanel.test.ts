// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, nextTick, ref } from 'vue'
import { createAppI18n } from '../src/i18n'
import HoldButton from '../src/components/HoldButton.vue'
import JoinPanel from '../src/components/JoinPanel.vue'
import { cleanName, JOIN_MODE_KEY, loadJoinMode, saveJoinMode, shortName } from '../src/core/joinPrefs'
import type { JoinRequest } from '../src/core/receiverSession'

const i18n = createAppI18n('en')
const req = (id: number, name = `Person ${id}`): JoinRequest => ({ id, name, check: 10 + id, receivedAt: Date.now() })

function panel(props: Partial<InstanceType<typeof JoinPanel>['$props']> = {}) {
  return mount(JoinPanel, {
    attachTo: document.body,
    global: { plugins: [i18n] },
    props: { mode: 'confirm', requests: [], peerName: null, notice: null, compact: false, ...props },
  })
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
})

describe('HoldButton', () => {
  const mk = () => mount(HoldButton, { attachTo: document.body, global: { plugins: [i18n] }, props: { label: 'Allow', holdMs: 1000 } })

  it('acts only after the button was held', async () => {
    const w = mk()
    const b = w.get('button')
    await b.trigger('pointerdown')
    vi.advanceTimersByTime(500)
    await b.trigger('pointerup')
    expect(w.emitted('confirm')).toBeUndefined()
    await nextTick()
    expect(w.find('[data-testid=hold-hint]').exists()).toBe(true) // a short press only shows a hint
    await b.trigger('pointerdown')
    vi.advanceTimersByTime(1100)
    expect(w.emitted('confirm')).toHaveLength(1)
    await b.trigger('pointerup')
    expect(w.emitted('confirm')).toHaveLength(1)
  })

  it('works with Enter held on a keyboard or remote, not with a tap or a click', async () => {
    const w = mk()
    const b = w.get('button')
    await b.trigger('click')
    expect(w.emitted('confirm')).toBeUndefined()
    await b.trigger('keydown', { key: 'Enter' })
    vi.advanceTimersByTime(400)
    await b.trigger('keyup', { key: 'Enter' })
    expect(w.emitted('confirm')).toBeUndefined()
    await b.trigger('keydown', { key: 'Enter' })
    await b.trigger('keydown', { key: 'Enter', repeat: true }) // key repeat does not restart the hold
    vi.advanceTimersByTime(1100)
    expect(w.emitted('confirm')).toHaveLength(1)
  })

  it('leaving the button or losing the focus cancels the hold', async () => {
    const w = mk()
    const b = w.get('button')
    await b.trigger('pointerdown')
    vi.advanceTimersByTime(600)
    await b.trigger('pointerleave')
    vi.advanceTimersByTime(1000)
    expect(w.emitted('confirm')).toBeUndefined()
  })
})

describe('JoinPanel', () => {
  it('shows at most three requests and counts the rest; names stay plain text', () => {
    const w = panel({ requests: [1, 2, 3, 4, 5].map((i) => req(i, i === 1 ? '<img src=x onerror=alert(1)>' : undefined)) })
    expect(w.findAll('[data-testid=join-request]')).toHaveLength(3)
    expect(w.get('[data-testid=join-more]').text()).toBe('+2 more waiting')
    expect(w.find('img').exists()).toBe(false)
    expect(w.get('[data-testid=request-name]').text()).toContain('<img src=x one') // cut at 18 characters
    expect(w.get('[data-testid=request-name]').text()).not.toContain('alert')
    expect(w.get('[data-testid=request-name]').attributes('title')).toContain('alert(1)')
  })

  it('shows the check number on every request', () => {
    const w = panel({ requests: [req(1), req(2)] })
    expect(w.findAll('[data-testid=request-check]').map((e) => e.text())).toEqual(['11', '12'])
  })

  it('a new request never takes the focus', async () => {
    const w = panel({ requests: [req(1)] })
    const radio = w.get('[data-testid=mode-confirm]').element as HTMLElement
    radio.focus()
    expect(document.activeElement).toBe(radio)
    await w.setProps({ requests: [req(1), req(2)] })
    expect(document.activeElement).toBe(radio)
    expect(document.activeElement?.hasAttribute('data-allow')).toBe(false)
    expect(w.get('[data-testid=join-announce]').text()).toContain('Person 2')
  })

  it('after a decision the focus goes to the next request\'s Deny, never to Allow', async () => {
    // A parent that behaves like the receive view: the request leaves the list at once.
    const Host = defineComponent({
      components: { JoinPanel },
      setup() {
        const requests = ref([req(1), req(2)])
        const drop = (id: number) => (requests.value = requests.value.filter((r) => r.id !== id))
        return { requests, drop }
      },
      template: '<JoinPanel mode="confirm" :requests="requests" :peer-name="null" :notice="null" :compact="false" @deny="drop" @allow="drop" />',
    })
    const w = mount(Host, { attachTo: document.body, global: { plugins: [i18n] } })
    await w.findAll('[data-testid=deny]')[0].trigger('click')
    await nextTick()
    const active = document.activeElement as HTMLElement
    expect(active.hasAttribute('data-deny')).toBe(true)
    expect(active.hasAttribute('data-allow')).toBe(false)
    expect(w.findAll('[data-testid=join-request]')).toHaveLength(1)
    // Last one decided: the focus returns to the mode switch.
    await w.get('[data-testid=deny]').trigger('click')
    await nextTick()
    expect((document.activeElement as HTMLElement).getAttribute('role')).toBe('radio')
  })

  it('has Deny all from two requests on', () => {
    expect(panel({ requests: [req(1)] }).find('[data-testid=deny-all]').exists()).toBe(false)
    const w = panel({ requests: [req(1), req(2)] })
    expect(w.find('[data-testid=deny-all]').exists()).toBe(true)
  })

  it('switches the mode with one press and locks with the quick action', async () => {
    const w = panel({ mode: 'open' })
    await w.get('[data-testid=mode-confirm]').trigger('click')
    await w.get('[data-testid=lock-now]').trigger('click')
    expect(w.emitted('mode')).toEqual([['confirm'], ['locked']])
    await w.setProps({ mode: 'locked' })
    expect(w.find('[data-testid=lock-now]').exists()).toBe(false)
    expect(w.get('[data-testid=mode-locked]').attributes('aria-checked')).toBe('true')
  })

  it('arrow keys move the focus between the modes without selecting', async () => {
    const w = panel({ mode: 'open' })
    const open = w.get('[data-testid=mode-open]')
    ;(open.element as HTMLElement).focus()
    await open.trigger('keydown', { key: 'ArrowRight' })
    expect(document.activeElement).toBe(w.get('[data-testid=mode-confirm]').element)
    expect(w.emitted('mode')).toBeUndefined()
  })

  it('lists the connected sender with a Remove button', async () => {
    const w = panel({ peerName: 'Ana' })
    expect(w.get('[data-testid=peer-name]').text()).toContain('Ana')
    await w.get('[data-testid=remove-peer]').trigger('click')
    expect(w.emitted('remove')).toHaveLength(1)
  })

  it('shrinks to a chip and a pill while somebody presents, and opens on demand', async () => {
    const w = panel({ compact: true, requests: [req(1), req(2)] })
    expect(w.find('[data-testid=mode-open]').exists()).toBe(false)
    expect(w.get('[data-testid=join-pill]').text()).toBe('2 waiting')
    expect(w.get('[data-testid=join-chip]').text()).toBe('Join: Ask first')
    await w.get('[data-testid=join-pill]').trigger('click')
    expect(w.find('[data-testid=mode-open]').exists()).toBe(true)
    await w.get('section').trigger('keydown', { key: 'Escape' })
    expect(w.find('[data-testid=mode-open]').exists()).toBe(false)
  })

  it('shows the notice after locking and lets it be dismissed', async () => {
    const w = panel({ notice: { kind: 'locked_discarded', count: 3 } })
    expect(w.get('[data-testid=join-notice]').text()).toContain('3 waiting request(s) were discarded')
    await w.get('[data-testid=join-notice] button').trigger('click')
    expect(w.emitted('dismiss')).toHaveLength(1)
  })
})

describe('join preferences', () => {
  beforeEach(() => localStorage.clear())

  it('first run is open; the last choice is remembered', () => {
    expect(loadJoinMode()).toBe('open')
    saveJoinMode('confirm')
    expect(loadJoinMode()).toBe('confirm')
    localStorage.setItem(JOIN_MODE_KEY, 'bogus')
    expect(loadJoinMode()).toBe('open')
  })

  it('survives a localStorage that throws', () => {
    const spy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    const spy2 = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(loadJoinMode()).toBe('open')
    expect(() => saveJoinMode('locked')).not.toThrow()
    spy.mockRestore()
    spy2.mockRestore()
  })

  it('cleans and cuts names', () => {
    expect(cleanName('A‮b\u0007c​d  e')).toBe('Abcd e')
    expect(Array.from(cleanName('x'.repeat(100))).length).toBe(40)
    expect(shortName('abcdefghijklmnopqrstuvwxyz')).toBe('abcdefghijklmnopqr…')
    expect(shortName('short')).toBe('short')
  })
})
