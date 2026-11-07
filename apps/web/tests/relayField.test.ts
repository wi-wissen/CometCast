// SPDX-License-Identifier: GPL-3.0-or-later
import { beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { createAppI18n } from '../src/i18n'
import RelayField from '../src/components/RelayField.vue'
import { initRelay, resetRelayState } from '../src/composables/useRelay'

const cfg = (async () => new Response('{"relays":["wss://relay.cometcast.de/ws","wss://other.example.org"]}')) as unknown as typeof fetch

describe('RelayField', () => {
  beforeEach(async () => {
    resetRelayState()
    await initRelay({ fetchImpl: cfg })
  })
  it('shows only the host of the relay in use, no warning for the default', () => {
    const w = mount(RelayField, { props: { editable: true }, global: { plugins: [createAppI18n('en')] } })
    expect(w.get('[data-testid=relay-host]').text()).toBe('relay.cometcast.de')
    expect(w.get('[data-testid=relay] p').text()).not.toContain('wss://')
    expect(w.find('[data-testid=relay-warning]').exists()).toBe(false)
  })
  it('changes the relay, warns about a non-default one (translated, formal German) and resets', async () => {
    const w = mount(RelayField, { props: { editable: true }, global: { plugins: [createAppI18n('de')] } })
    await w.get('[data-testid=relay-input]').setValue('ws://public.example.org')
    await w.get('form').trigger('submit')
    expect(w.get('[data-testid=relay-error]').text()).toContain('wss://')
    expect(w.get('[data-testid=relay-host]').text()).toBe('relay.cometcast.de')
    await w.get('[data-testid=relay-input]').setValue('wss://mine.example.org')
    await w.get('form').trigger('submit')
    expect(w.get('[data-testid=relay-host]').text()).toBe('mine.example.org')
    const warn = w.get('[data-testid=relay-warning]').text()
    expect(warn).toContain('Fahren Sie nur fort')
    expect(warn).toContain('relay.cometcast.de')
    await w.get('[data-testid=relay-reset]').trigger('click')
    expect(w.get('[data-testid=relay-host]').text()).toBe('relay.cometcast.de')
    expect(w.find('[data-testid=relay-warning]').exists()).toBe(false)
  })
  it('is read-only when not editable', () => {
    const w = mount(RelayField, { props: { editable: false }, global: { plugins: [createAppI18n('en')] } })
    expect(w.find('[data-testid=relay-input]').exists()).toBe(false)
    expect(w.get('[data-testid=relay-host]').text()).toBe('relay.cometcast.de')
  })
})
