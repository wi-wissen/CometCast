// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { createAppI18n } from '../src/i18n'
import RelayConfirm from '../src/components/RelayConfirm.vue'
import { ensureRelayTrusted, initRelay, resetRelayState, useRelay } from '../src/composables/useRelay'

const cfg = (async () => new Response('{"relays":["wss://relay.cometcast.de/ws","wss://other.example.org"]}')) as unknown as typeof fetch

describe('relay from a link needs a confirmation', () => {
  const server = 'wss://crafted.example.org'
  async function boot(search: string) {
    resetRelayState()
    history.replaceState(null, '', `/${search}`)
    await initRelay({ fetchImpl: cfg })
  }
  afterEach(() => {
    history.replaceState(null, '', '/')
    resetRelayState()
  })
  const mountDialog = (locale: 'en' | 'de' = 'en') => mount(RelayConfirm, { global: { plugins: [createAppI18n(locale)] }, attachTo: document.body })

  it('blocks until answered, names the host, and remembers the choice for exactly that relay in this tab', async () => {
    await boot(`?server=${encodeURIComponent(server)}`)
    const w = mountDialog('de')
    expect(w.find('[data-testid=relay-confirm]').exists()).toBe(false)
    const r = ensureRelayTrusted()
    await w.vm.$nextTick()
    expect(w.get('[data-testid=relay-confirm-host]').text()).toBe('crafted.example.org')
    expect(w.get('[data-testid=relay-confirm]').text()).toContain('Betreiber')
    expect(w.get('[data-testid=relay-confirm-use]').text()).toBe('Dieses Relay verwenden')
    let settled = false
    void r.then(() => (settled = true))
    await new Promise((res) => setTimeout(res, 10))
    expect(settled).toBe(false)
    await w.get('[data-testid=relay-confirm-use]').trigger('click')
    expect(await r).toBe(true)
    expect(w.find('[data-testid=relay-confirm]').exists()).toBe(false)
    expect(sessionStorage.getItem('cometcast.relayConfirmed')).toBe('wss://crafted.example.org/ws')
    // Asking again needs no dialog.
    expect(await ensureRelayTrusted()).toBe(true)
    // A reload of the tab (new init) keeps the confirmation for that relay ...
    await initRelay({ fetchImpl: cfg })
    expect(await ensureRelayTrusted()).toBe(true)
    // ... but not for another relay.
    history.replaceState(null, '', `/?server=${encodeURIComponent('wss://other-crafted.example.org')}`)
    await initRelay({ fetchImpl: cfg })
    const r2 = ensureRelayTrusted()
    await w.vm.$nextTick()
    expect(w.get('[data-testid=relay-confirm-host]').text()).toBe('other-crafted.example.org')
    await w.get('[data-testid=relay-confirm-cancel]').trigger('click')
    expect(await r2).toBe(false)
    w.unmount()
  })

  it('cancel falls back to the default relay', async () => {
    await boot(`?server=${encodeURIComponent(server)}`)
    const w = mountDialog()
    const r = ensureRelayTrusted()
    await w.vm.$nextTick()
    expect(w.get('[data-testid=relay-confirm-cancel]').text()).toBe('Cancel (use the default relay)')
    await w.get('[data-testid=relay-confirm-cancel]').trigger('click')
    expect(await r).toBe(false)
    expect(useRelay().current.value.host).toBe('relay.cometcast.de')
    expect(sessionStorage.getItem('cometcast.relayConfirmed')).toBeNull()
    expect(await ensureRelayTrusted()).toBe(true) // the default is trusted
    w.unmount()
  })

  it('does not ask for config.json relays, the default, or a relay typed in the field', async () => {
    await boot('')
    expect(await ensureRelayTrusted()).toBe(true)
    await boot(`?server=${encodeURIComponent('wss://other.example.org')}`) // listed in config.json
    expect(await ensureRelayTrusted()).toBe(true)
    await boot('')
    expect(useRelay().set('wss://typed.example.org')).toBeNull()
    expect(await ensureRelayTrusted()).toBe(true)
  })

  it('a session start waits for the answer and a refusal never starts it', async () => {
    await boot(`?server=${encodeURIComponent(server)}`)
    const w = mountDialog()
    let started = 0
    const gate = ensureRelayTrusted().then((ok) => ok && started++)
    await w.vm.$nextTick()
    expect(started).toBe(0)
    await w.get('[data-testid=relay-confirm-cancel]').trigger('click')
    await gate
    expect(started).toBe(0)
    w.unmount()
  })
})

describe('config.json problems', () => {
  it('a failing config.json is reported by the composable, a missing one is not', async () => {
    resetRelayState()
    await initRelay({ fetchImpl: (async () => new Response('nonsense')) as unknown as typeof fetch })
    expect(useRelay().configFailed.value).toBe(true)
    resetRelayState()
    await initRelay({ fetchImpl: (async () => new Response('', { status: 404 })) as unknown as typeof fetch })
    expect(useRelay().configFailed.value).toBe(false)
    resetRelayState()
  })
})
