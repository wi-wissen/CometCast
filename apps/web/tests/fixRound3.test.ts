// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fix list 3 (docs/design/fix-list-web-3.md): frozen badge, black hint, media card words, transfer pill, wizard card,
// link address, selected tile, dialogs over a presentation, locked QR.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { createAppI18n } from '../src/i18n'
import { shortenMiddle } from '../src/core/fitName'
import type { ReceiverState } from '../src/core/receiverSession'
import PresentingStage from '../src/components/display/PresentingStage.vue'
import ContentOverlay from '../src/components/ContentOverlay.vue'

const read = (p: string) => readFileSync(fileURLToPath(new URL(p, import.meta.url)), 'utf8')
const css = read('../src/styles/display.css')
const rule = (sel: string) => {
  const i = css.indexOf(`${sel} {`)
  if (i < 0) throw new Error(`no rule ${sel}`)
  return css.slice(i, css.indexOf('}', i))
}
const loc = (n: string) => JSON.parse(read(`../src/locales/${n}.json`))
const i18n = () => ({ plugins: [createAppI18n('en')] })

describe('locale values (items 2 and 3)', () => {
  it('Play is the word only; the hint uses typographic quotes; opening has a space before the ellipsis in German', () => {
    for (const n of ['en', 'de', 'de-sie']) expect(loc(n).receive.media.hint).not.toContain('"')
    expect(loc('en').receive.media.play).toBe('Play')
    expect(loc('de').receive.media.play).toBe('Abspielen')
    expect(loc('de-sie').receive.media.play).toBe('Abspielen')
    expect(loc('en').receive.media.hint).toBe('This link points to a media file. It is loaded from {host} only if you click “Play”.')
    expect(loc('en').receive.document.loading).toBe('Opening {name}…')
    expect(loc('de').receive.document.loading).toBe('{name} wird geöffnet …')
    expect(loc('de-sie').receive.document.loading).toBe('{name} wird geöffnet …')
  })
})

describe('style rules', () => {
  it('--onblack in all three themes, used by the black hint', () => {
    expect(css.match(/--onblack: #a1a9ba/g)).toHaveLength(3)
    expect(rule('.cd-blackhint')).toContain('var(--onblack)')
  })
  it('link address in --fg; transfer pill one line max 340; selected mode card tinted (OLED none); badge fades', () => {
    expect(rule('.cd-wz .cd-addr')).toContain('color: var(--fg)')
    expect(rule('.cd-transfer')).toMatch(/max-width: 340px/)
    expect(rule('.cd-transfer')).toMatch(/white-space: nowrap/)
    expect(rule('.cd-mcard.on')).toContain('color-mix')
    expect(rule(".cd[data-theme='oled'] .cd-mcard.on")).toContain('#000000')
    expect(rule('.cd-badge.hid')).toContain('opacity: 0')
  })
  it('the locked QR dims the code, not the box edge', () => {
    expect(rule('.cd-qrbox.dim')).not.toContain('opacity')
    expect(rule('.cd-qrbox.dim svg')).toContain('opacity: 0.25')
  })
})

describe('shortenMiddle', () => {
  it('keeps the start and the extension', () => {
    expect(shortenMiddle('holiday-photos-from-2026.png', 19)).toBe('holiday-pho…2026.png')
    expect(shortenMiddle('a.png', 19)).toBe('a.png')
    const s = shortenMiddle('a-very-long-file-name-from-the-holiday-in-2026-final-version.png', 10)
    expect(s.endsWith('.png')).toBe(true)
    expect(s.startsWith('a-v')).toBe(true)
    expect(shortenMiddle('noextensionatallinthisname', 8)).toMatch(/^no.*…/)
  })
})

describe('presenting stage', () => {
  const base: ReceiverState = {
    phase: 'receiving', code: 'K7QM2W', joinMode: 'open', requests: [], peerName: 'Lena', blocks: [], notice: null,
    errorCode: null, stream: null, content: null, transfer: null, contentError: null,
  }
  beforeEach(() => {
    HTMLCanvasElement.prototype.getContext = (() => null) as never
    vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    document.body.innerHTML = ''
  })
  const mk = (state: ReceiverState, controlsShown: boolean, requestOpen = false) =>
    mount(PresentingStage, { attachTo: document.body, global: i18n(), props: { state, renderPage: async () => ({ width: 1, height: 1, rgba: new Uint8ClampedArray(4) }), controlsShown, requestOpen } })

  it('the frozen badge is hidden while the toolbar is visible and shows at rest', async () => {
    const img = { ...base, content: { kind: 'image' as const, url: 'blob:x', name: 'a.png' } }
    const w = mk(img, true)
    // freezing needs a video frame; the badge follows the toolbar state, so set that state directly
    const setup = (w.vm as unknown as { $: { setupState: { toolbar: { frozen: boolean } } } }).$.setupState
    setup.toolbar = { ...setup.toolbar, frozen: true }
    await w.vm.$nextTick()
    const badge = () => w.get('[data-testid=frozen-badge]')
    expect(badge().classes()).toContain('hid')
    await w.setProps({ controlsShown: false })
    expect(badge().classes()).not.toContain('hid')
  })

  it('a link card or a join request makes the picture layer inert', async () => {
    const link = mk({ ...base, content: { kind: 'link', url: 'https://example.org/a', host: 'example.org' } }, true)
    expect(link.get('.cd-stagehud').attributes('inert')).toBeDefined()
    const req = mk({ ...base, content: { kind: 'image', url: 'blob:x', name: 'a.png' } }, true, true)
    expect(req.get('.cd-stagehud').attributes('inert')).toBeDefined()
    const none = mk({ ...base, content: { kind: 'image', url: 'blob:x', name: 'a.png' } }, true)
    expect(none.get('.cd-stagehud').attributes('inert')).toBeUndefined()
  })

  it('a received PDF that is being opened shows the pill, not a separate line, until its first page is drawn', async () => {
    const w = mk({ ...base, content: { kind: 'document', id: 1, name: 'slides.pdf', pages: 0, page: 1 } }, false)
    await vi.waitFor(() => expect(w.get('[data-testid=transfer]').text()).toBe('Opening slides.pdf…'))
    expect(w.find('[data-testid=document-loading]').exists()).toBe(false)
  })
})

describe('media link card', () => {
  it('the Play button says only Play', () => {
    const w = mount(ContentOverlay, { global: i18n(), props: { content: { kind: 'link', url: 'https://media.example.org/a.mp4', host: 'media.example.org' }, renderPage: async () => ({ width: 1, height: 1, rgba: new Uint8ClampedArray(4) }) } })
    expect(w.get('[data-testid=media-play]').text()).toBe('Play')
    expect(w.get('[data-testid=media-hint]').text()).toContain('“Play”')
  })
})

describe('focus ring (decision 13.62)', () => {
  it('a modifier key alone shows the ring again after a pointer press', async () => {
    const { defineComponent, h, nextTick } = await import('vue')
    const { useFocusRing } = await import('../src/composables/useFocusRing')
    const C = defineComponent({
      setup() {
        const { hidden } = useFocusRing()
        return () => h('div', { 'data-hidden': String(hidden.value) })
      },
    })
    const w = mount(C, { attachTo: document.body })
    window.dispatchEvent(new Event('pointerdown'))
    await nextTick()
    expect(w.get('div').attributes('data-hidden')).toBe('true')
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Shift' }))
    await nextTick()
    expect(w.get('div').attributes('data-hidden')).toBe('false')
    w.unmount()
  })
})
