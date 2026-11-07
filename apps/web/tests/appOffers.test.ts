// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createAppI18n } from '../src/i18n'

// A store link exists: the idle footer names the stores (spec 4.2). The shipped constants are empty (misc tests).
vi.mock('../src/config/stores', () => ({
  STORE_URLS: { googlePlay: 'https://play.example/app', appStore: '', desktop: '' },
  appUrlFor: () => 'https://play.example/app',
  anyAppLink: () => true,
}))

import IdleScreen from '../src/components/display/IdleScreen.vue'

describe('idle footer with a store link', () => {
  it('names the stores and {dom}/app', () => {
    const w = mount(IdleScreen, {
      global: { plugins: [createAppI18n('en')] },
      props: { theme: 'dark', room: 'R', mode: 'open', code: 'K7QM2W', dom: 'cometcast.de', view: 'online', cause: 'network', dimmed: false, scheme: 'https' },
    })
    expect(w.get('[data-testid=idle-foot]').text()).toBe('No download needed. Prefer an app: App Store · Google Play · cometcast.de/app')
  })
})
