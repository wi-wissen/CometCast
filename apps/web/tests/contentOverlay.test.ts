// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createAppI18n } from '../src/i18n'
import ContentOverlay from '../src/components/ContentOverlay.vue'
import type { ReceivedContent } from '../src/core/receiverSession'

// A new item replaces a playing media link (one player at a time). The presenter's pages can only replace
// content by stopping first, so the end-to-end test no longer reaches this; it is checked here.
const wav: ReceivedContent = { kind: 'link', url: 'https://media.example.org/a.wav', host: 'media.example.org' }
const mk = (content: ReceivedContent | null) =>
  mount(ContentOverlay, { attachTo: document.body, global: { plugins: [createAppI18n('en')] }, props: { content, renderPage: vi.fn() } })

// jsdom has no media playback.
HTMLMediaElement.prototype.play = (() => Promise.resolve()) as never
HTMLMediaElement.prototype.pause = (() => undefined) as never
HTMLMediaElement.prototype.load = (() => undefined) as never

afterEach(() => (document.body.innerHTML = ''))

describe('content overlay', () => {
  it('a new link, an image or a document closes a playing media player and releases the element', async () => {
    for (const next of [
      { kind: 'link', url: 'https://media.example.org/b.wav', host: 'media.example.org' },
      { kind: 'image', url: 'blob:x', name: 'a.png' },
      { kind: 'document', id: 9, name: 'a.pdf', pages: 0, page: 1 },
      null,
    ] as Array<ReceivedContent | null>) {
      const w = mk(wav)
      await nextTick()
      expect(w.find('[data-testid=link-card]').exists()).toBe(true)
      expect(document.querySelectorAll('[data-testid=media-element]')).toHaveLength(0) // nothing loads before the click
      await w.get('[data-testid=media-play]').trigger('click')
      expect(document.querySelectorAll('[data-testid=media-element]')).toHaveLength(1)
      await w.setProps({ content: next })
      await nextTick()
      expect(document.querySelectorAll('[data-testid=media-element]'), `after ${next?.kind ?? 'close'}`).toHaveLength(0)
      expect(w.find('[data-testid=media-player]').exists()).toBe(false)
      w.unmount()
    }
  })
})
