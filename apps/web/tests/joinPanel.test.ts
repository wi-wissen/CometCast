// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, nextTick, ref } from 'vue'
import { createAppI18n } from '../src/i18n'
import JoinPanel from '../src/components/JoinPanel.vue'
import JoinRequestCard from '../src/components/JoinRequestCard.vue'
import JoinTakeover from '../src/components/JoinTakeover.vue'
import { REQUEST_GUARD_MS } from '../src/core/joinGuard'
import { cleanName, JOIN_MODE_KEY, loadJoinMode, saveJoinMode, shortName } from '../src/core/joinPrefs'
import type { JoinRequest } from '../src/core/receiverSession'

const i18n = createAppI18n('en')
const req = (id: number, name = `Person ${id}`, receivedAt = Date.now() - 5000): JoinRequest => ({ id, name, check: 10 + id, receivedAt })

function panel(props: Record<string, unknown> = {}) {
  return mount(JoinPanel, {
    attachTo: document.body,
    global: { plugins: [i18n] },
    props: { mode: 'confirm', requests: [], peerName: null, blocks: [], notice: null, compact: false, ...props },
  })
}

beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] }))
afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
})

describe('JoinRequestCard', () => {
  const mk = (receivedAt: number, now: number) =>
    mount(JoinRequestCard, { global: { plugins: [i18n] }, props: { request: req(1, 'Ana', receivedAt), now } })

  it('ignores presses right after the request appeared, then reacts', async () => {
    const t0 = 1_000_000
    const early = mk(t0, t0 + REQUEST_GUARD_MS - 100)
    await early.get('[data-testid=allow]').trigger('click')
    await early.get('[data-testid=deny]').trigger('click')
    await early.get('[data-testid=deny-block]').trigger('click')
    expect(early.emitted('allow')).toBeUndefined()
    expect(early.emitted('deny')).toBeUndefined()
    expect(early.emitted('block')).toBeUndefined()
    expect(early.get('[data-testid=allow]').attributes('aria-disabled')).toBe('true')
    const late = mk(t0, t0 + REQUEST_GUARD_MS + 10)
    await late.get('[data-testid=allow]').trigger('click')
    await late.get('[data-testid=deny]').trigger('click')
    await late.get('[data-testid=deny-block]').trigger('click')
    expect(late.emitted('allow')).toHaveLength(1)
    expect(late.emitted('deny')).toHaveLength(1)
    expect(late.emitted('block')).toHaveLength(1)
  })

  it('shows the name in quotes as plain text, cut at 18 characters, and the check number', () => {
    const w = mount(JoinRequestCard, {
      global: { plugins: [i18n] },
      props: { request: req(1, '<img src=x onerror=alert(1)>'), now: Date.now() },
    })
    expect(w.find('img').exists()).toBe(false)
    expect(w.get('[data-testid=request-name]').text()).toContain('<img src=x one')
    expect(w.get('[data-testid=request-name]').text()).not.toContain('alert')
    expect(w.get('[data-testid=request-name]').attributes('title')).toContain('alert(1)')
    expect(w.get('[data-testid=request-check]').text()).toBe('11')
  })

  it('has No first in the DOM (and so in the focus order)', () => {
    const w = mk(0, 10_000)
    const ids = w.findAll('button').map((b) => b.attributes('data-testid'))
    expect(ids).toEqual(['deny', 'allow', 'deny-block'])
  })
})

describe('JoinTakeover', () => {
  const mk = (props: Record<string, unknown> = {}) =>
    mount(JoinTakeover, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: { request: req(1, 'Ana'), more: 0, banner: false, ...props },
    })

  it('is a dialog on the code page and a banner while somebody presents', () => {
    expect(mk().get('[data-testid=join-takeover]').attributes('role')).toBe('alertdialog')
    document.body.innerHTML = ''
    const b = mk({ banner: true })
    expect(b.get('[data-testid=join-takeover]').attributes('role')).toBe('alert')
    expect(b.get('[data-testid=join-takeover]').attributes('data-banner')).toBe('true')
  })

  it('shows nothing without a request', () => {
    expect(mk({ request: null }).find('[data-testid=join-takeover]').exists()).toBe(false)
  })

  it('puts the focus on No, never on Yes, and gives it back when it goes away', async () => {
    const before = document.createElement('button')
    document.body.appendChild(before)
    before.focus()
    const w = mk({ request: null })
    await w.setProps({ request: req(1, 'Ana') })
    await nextTick()
    const active = document.activeElement as HTMLElement
    expect(active.hasAttribute('data-deny')).toBe(true)
    expect(active.hasAttribute('data-allow')).toBe(false)
    await w.setProps({ request: null })
    await nextTick()
    expect(document.activeElement).toBe(before)
  })

  it('moves the focus to No of the next request after a decision', async () => {
    const w = mk()
    await w.setProps({ request: req(2, 'Ben') })
    await nextTick()
    expect((document.activeElement as HTMLElement).hasAttribute('data-deny')).toBe(true)
    expect(w.get('[data-testid=request-name]').text()).toContain('Ben')
  })

  it('Escape and Later hide it without deciding', async () => {
    const w = mk()
    await w.get('[data-testid=join-takeover]').trigger('keydown', { key: 'Escape' })
    await w.get('[data-testid=later]').trigger('click')
    expect(w.emitted('later')).toHaveLength(2)
    expect(w.emitted('allow')).toBeUndefined()
    expect(w.emitted('deny')).toBeUndefined()
  })

  it('counts the others and offers Deny all', async () => {
    const w = mk({ more: 2 })
    expect(w.get('[data-testid=takeover-more]').text()).toBe('+2 more waiting')
    await w.get('[data-testid=deny-all]').trigger('click')
    expect(w.emitted('denyAll')).toHaveLength(1)
    expect(mk({ more: 0 }).find('[data-testid=deny-all]').exists()).toBe(false)
  })
})

describe('JoinPanel', () => {
  it('lists requests that are not on the takeover, with a count of the rest', () => {
    const w = panel({ requests: [1, 2, 3, 4, 5, 6].map((i) => req(i)), takeoverId: 1 })
    expect(w.findAll('[data-testid=join-request]')).toHaveLength(3)
    expect(w.get('[data-testid=join-more]').text()).toBe('+2 more waiting')
    expect(w.text()).not.toContain('Person 1')
  })

  it('answers requests from the list with Yes, No and No and block', async () => {
    const w = panel({ requests: [req(7)] })
    await w.get('[data-testid=allow]').trigger('click')
    await w.get('[data-testid=deny]').trigger('click')
    await w.get('[data-testid=deny-block]').trigger('click')
    expect(w.emitted('allow')).toEqual([[7]])
    expect(w.emitted('deny')).toEqual([[7]])
    expect(w.emitted('block')).toEqual([[7]])
  })

  it('after a decision the focus goes to the next No, never to Yes', async () => {
    const Host = defineComponent({
      components: { JoinPanel },
      setup() {
        const requests = ref([req(1), req(2)])
        const drop = (id: number) => (requests.value = requests.value.filter((r) => r.id !== id))
        return { requests, drop }
      },
      template: '<JoinPanel mode="confirm" :requests="requests" :peer-name="null" :blocks="[]" :notice="null" :compact="false" @deny="drop" @allow="drop" />',
    })
    const w = mount(Host, { attachTo: document.body, global: { plugins: [i18n] } })
    await w.findAll('[data-testid=deny]')[0].trigger('click')
    await nextTick()
    const active = document.activeElement as HTMLElement
    expect(active.hasAttribute('data-deny')).toBe(true)
    await w.get('[data-testid=deny]').trigger('click')
    await nextTick()
    expect((document.activeElement as HTMLElement).getAttribute('role')).toBe('radio')
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

  it('ends the presentation with or without blocking', async () => {
    const w = panel({ peerName: 'Ana' })
    expect(w.get('[data-testid=peer-name]').text()).toContain('Ana')
    await w.get('[data-testid=remove-peer]').trigger('click')
    await w.get('[data-testid=remove-block]').trigger('click')
    expect(w.emitted('remove')).toEqual([[false], [true]])
  })

  it('lists blocked senders and unblocks them', async () => {
    const w = panel({ blocks: [{ id: 3, name: 'Pest' }] })
    expect(w.get('[data-testid=blocked-entry]').text()).toContain('Pest')
    await w.get('[data-testid=unblock]').trigger('click')
    expect(w.emitted('unblock')).toEqual([[3]])
  })

  it('End session needs a second press', async () => {
    const w = panel()
    await w.get('[data-testid=end-session]').trigger('click')
    expect(w.emitted('endSession')).toBeUndefined()
    expect(w.get('[data-testid=end-session]').text()).toBe('Press again to end the session')
    await w.get('[data-testid=end-session]').trigger('click')
    expect(w.emitted('endSession')).toHaveLength(1)
    // The confirmation lapses.
    await w.get('[data-testid=end-session]').trigger('click')
    vi.advanceTimersByTime(5000)
    await nextTick()
    await w.get('[data-testid=end-session]').trigger('click')
    expect(w.emitted('endSession')).toHaveLength(1)
  })

  it('compact: a chip that fades out, opens on demand, and closes with Escape', async () => {
    const w = panel({ compact: true, chipVisible: true, requests: [req(1), req(2)] })
    expect(w.find('[data-testid=mode-open]').exists()).toBe(false)
    expect(w.get('[data-testid=join-chip]').text()).toContain('Join: Ask first')
    expect(w.get('[data-testid=join-chip]').text()).toContain('2 waiting')
    expect(w.get('[data-testid=join-chip-row]').attributes('data-visible')).toBe('true')
    await w.setProps({ chipVisible: false })
    expect(w.get('[data-testid=join-chip-row]').classes()).toContain('opacity-0')
    expect(w.get('[data-testid=join-chip-row]').attributes('inert')).toBeDefined()
    await w.setProps({ chipVisible: true })
    await w.get('[data-testid=join-chip]').trigger('click')
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
