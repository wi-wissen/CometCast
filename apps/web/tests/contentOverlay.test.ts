// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createAppI18n } from '../src/i18n'
import ContentOverlay from '../src/components/ContentOverlay.vue'
import type { ReceivedContent } from '../src/core/receiverSession'
import { REQUEST_GUARD_MS } from '../src/core/joinGuard'

// A new item replaces a playing media link (one player at a time). The presenter's pages can only replace
// content by stopping first, so the end-to-end test no longer reaches this; it is checked here.
const wav: ReceivedContent = { kind: 'link', url: 'https://media.example.org/a.wav', host: 'media.example.org' }
const mk = (content: ReceivedContent | null) =>
  mount(ContentOverlay, { attachTo: document.body, global: { plugins: [createAppI18n('en')] }, props: { content, renderPage: vi.fn() } })

// jsdom has no media playback.
HTMLMediaElement.prototype.play = (() => Promise.resolve()) as never
HTMLMediaElement.prototype.pause = (() => undefined) as never
HTMLMediaElement.prototype.load = (() => undefined) as never

beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] }))
afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
})
const guard = async () => {
  vi.advanceTimersByTime(REQUEST_GUARD_MS + 100)
  await nextTick()
}

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
      await guard()
      await w.get('[data-testid=media-play]').trigger('click')
      expect(document.querySelectorAll('[data-testid=media-element]')).toHaveLength(1)
      await w.setProps({ content: next })
      await nextTick()
      expect(document.querySelectorAll('[data-testid=media-element]'), `after ${next?.kind ?? 'close'}`).toHaveLength(0)
      expect(w.find('[data-testid=media-player]').exists()).toBe(false)
      w.unmount()
    }
  })

  it('the picture has nothing drawn over it: no close button, no page buttons', async () => {
    const image = mk({ kind: 'image', url: 'blob:x', name: 'a.png' })
    await nextTick()
    expect(image.find('[data-testid=received-image]').exists()).toBe(true)
    expect(image.findAll('button')).toHaveLength(0)
    image.unmount()
    const doc = mk({ kind: 'document', id: 1, name: 'a.pdf', pages: 3, page: 2 })
    await nextTick()
    expect(doc.find('[data-testid=document-box]').exists()).toBe(true)
    expect(doc.findAll('button')).toHaveLength(0)
    expect(doc.find('[data-testid=content-close]').exists()).toBe(false)
    expect(doc.find('[data-testid=doc-prev]').exists()).toBe(false)
    // the page number stays as a status line for screen readers, not drawn
    expect(doc.get('[data-testid=document-page]').classes()).toContain('sr-only')
    doc.unmount()
  })
})

describe('link card (a display dialog, spec 4.5)', () => {
  const link: ReceivedContent = { kind: 'link', url: 'https://example.org/a/very/long/path?x=1', host: 'example.org' }

  it('shows the title, the address, the hint, Open (primary) and Dismiss; Open has the focus', async () => {
    const w = mk(link)
    await nextTick()
    const card = w.get('[data-testid=link-card]')
    expect(card.attributes('role')).toBe('alertdialog')
    expect(card.classes()).toContain('cd-wz')
    expect(w.get('#link-card-title').text()).toBe('example.org wants to open a link')
    expect(w.get('[data-testid=link-url]').text()).toBe(link.kind === 'link' ? link.url : '')
    expect(w.get('[data-testid=link-url]').classes()).toContain('mono')
    expect(card.text()).toContain('Open it only if you expect it')
    const open = w.get('[data-testid=link-open]')
    expect(open.classes()).toContain('p')
    expect(open.attributes('target')).toBe('_blank')
    expect(open.attributes('rel')).toBe('noopener noreferrer')
    expect(w.get('[data-testid=link-dismiss]').classes()).not.toContain('p')
    expect(document.activeElement).toBe(open.element)
    expect(w.findAll('[data-row="0"]').map((b) => b.attributes('data-testid'))).toEqual(['link-open', 'link-dismiss'])
  })
  it('ignores input for 1 s: Open and Dismiss do nothing, then they work', async () => {
    const w = mk(link)
    await nextTick()
    const ev = new MouseEvent('click', { bubbles: true, cancelable: true })
    w.get('[data-testid=link-open]').element.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(true) // the browser does not follow the link yet
    await w.get('[data-testid=link-dismiss]').trigger('click')
    await w.get('[data-testid=link-card]').trigger('keydown', { key: 'Escape' })
    vi.advanceTimersByTime(REQUEST_GUARD_MS - 200)
    await w.get('[data-testid=link-dismiss]').trigger('click')
    expect(w.emitted('close')).toBeUndefined()
    await guard()
    const ev2 = new MouseEvent('click', { bubbles: true, cancelable: true })
    w.get('[data-testid=link-open]').element.dispatchEvent(ev2)
    expect(ev2.defaultPrevented).toBe(false) // the link opens only after the guard
    vi.advanceTimersByTime(10)
    expect(w.emitted('close')).toHaveLength(1) // Open closes the card after the navigation started
    await w.get('[data-testid=link-dismiss]').trigger('click')
    expect(w.emitted('close')).toHaveLength(2)
  })
  it('Back / Escape = Dismiss; the arrows move between Open and Dismiss', async () => {
    const w = mk(link)
    await guard()
    await w.get('[data-testid=link-modal]').trigger('keydown', { key: 'ArrowRight' })
    expect(document.activeElement).toBe(w.get('[data-testid=link-dismiss]').element)
    await w.get('[data-testid=link-modal]').trigger('keydown', { key: 'ArrowLeft' })
    expect(document.activeElement).toBe(w.get('[data-testid=link-open]').element)
    await w.get('[data-testid=link-modal]').trigger('keydown', { key: 'Escape' })
    expect(w.emitted('close')).toHaveLength(1)
  })
  it('a new link starts the guard again', async () => {
    const w = mk(link)
    await guard()
    await w.setProps({ content: { kind: 'link', url: 'https://example.org/b', host: 'example.org' } })
    await nextTick()
    await w.get('[data-testid=link-dismiss]').trigger('click')
    expect(w.emitted('close')).toBeUndefined()
  })
})
