// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { documentErrorCode, ReceiverSession } from '../src/core/receiverSession'
import type { Peer, PeerHandlers } from '../src/core/peer'
import { PdfError, type PdfRenderer } from '../src/core/pdfRenderer'
import { FakeSocket } from './helpers/fakeSocket'
import { pair, type FakeChannel } from './helpers/fakeChannel'
import type { ControlChannel } from '../src/core/control'

const tick = () => new Promise((r) => setTimeout(r, 0))

class P implements Peer {
  constructor(public h: PeerHandlers) {}
  addStream() {}
  openChannel(): ControlChannel {
    throw new Error('unused')
  }
  async createOffer() {
    return 'O'
  }
  async acceptOffer() {
    return 'A'
  }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

class FakeRenderer implements PdfRenderer {
  opened: Uint8Array | null = null
  disposed = false
  rendered: Array<[number, number]> = []
  constructor(private readonly outcome: { pages?: number; openError?: string; renderError?: string }) {}
  async open(bytes: Uint8Array) {
    this.opened = bytes
    if (this.outcome.openError) throw new PdfError(this.outcome.openError)
    return this.outcome.pages ?? 3
  }
  async render(index: number, width: number) {
    this.rendered.push([index, width])
    if (this.outcome.renderError) throw new PdfError(this.outcome.renderError)
    return { width: 1, height: 1, rgba: new Uint8ClampedArray(4) }
  }
  dispose() {
    this.disposed = true
  }
}

function setup(outcome: ConstructorParameters<typeof FakeRenderer>[0]) {
  const sock = new FakeSocket()
  let rp!: P
  const renderers: FakeRenderer[] = []
  const r = new ReceiverSession({
    url: 'ws://x/ws',
    onChange: () => {},
    socketFactory: () => sock,
    peerFactory: (h) => (rp = new P(h)),
    pdfRenderer: () => {
      const fr = new FakeRenderer(outcome)
      renderers.push(fr)
      return fr
    },
  })
  r.start()
  sock.recv({ type: 'room_created', code: 'ABC234' })
  sock.recv({ type: 'peer_joined' })
  const [a, b] = pair()
  rp.h.onDataChannel!(b)
  return { r, a, renderers }
}

function sendPdf(a: FakeChannel, size = 20) {
  a.send(`{"kind":"document","name":"plan.pdf","mime":"application/pdf","size":${size}}`)
  a.send(new Uint8Array(size).buffer)
  a.send('{"kind":"document_end"}')
}

describe('PDF documents in the receiver session', () => {
  it('opens the received bytes, then turns pages with the cmd messages (clamped, from 1)', async () => {
    const { r, a, renderers } = setup({ pages: 3 })
    sendPdf(a)
    await tick()
    expect(renderers[0]!.opened?.byteLength).toBe(20)
    expect(r.snapshot.content).toEqual({ kind: 'document', name: 'plan.pdf', pages: 3, page: 1 })
    const page = () => (r.snapshot.content as { page: number }).page
    a.send('{"kind":"cmd","cmd":"next_page"}')
    await tick()
    expect(page()).toBe(2)
    a.send('{"kind":"cmd","cmd":"next_page"}')
    a.send('{"kind":"cmd","cmd":"next_page"}')
    await tick()
    expect(page()).toBe(3) // clamped at the last page
    a.send('{"kind":"cmd","cmd":"prev_page"}')
    await tick()
    expect(page()).toBe(2)
    a.send('{"kind":"cmd","cmd":{"go_to_page":{"page":999}}}')
    await tick()
    expect(page()).toBe(3)
    a.send('{"kind":"cmd","cmd":{"go_to_page":{"page":1}}}')
    await tick()
    expect(page()).toBe(1)
    a.send('{"kind":"cmd","cmd":"prev_page"}')
    await tick()
    expect(page()).toBe(1)
    // Pages are rendered only when the view asks for them.
    expect(renderers[0]!.rendered).toEqual([])
    await r.renderPage(2, 640)
    expect(renderers[0]!.rendered).toEqual([[1, 640]])
    a.send('{"kind":"cmd","cmd":"stop"}')
    await tick()
    expect(r.snapshot.content).toBeNull()
    expect(renderers[0]!.disposed).toBe(true)
  })

  it('ignores page commands while no document is shown', async () => {
    const { r, a } = setup({})
    a.send('{"kind":"cmd","cmd":"next_page"}')
    await tick()
    expect(r.snapshot.content).toBeNull()
    expect(r.snapshot.contentError).toBeNull()
  })

  it('shows a translated notice and no content when the PDF cannot be opened', async () => {
    const { r, a, renderers } = setup({ openError: 'encrypted' })
    sendPdf(a)
    await tick()
    expect(r.snapshot.content).toBeNull()
    expect(r.snapshot.contentError).toBe('document_encrypted')
    expect(renderers[0]!.disposed).toBe(true)
  })

  it('closes the document with a notice when the renderer dies while rendering a page', async () => {
    const { r, a, renderers } = setup({ renderError: 'timeout' })
    sendPdf(a)
    await tick()
    await expect(r.renderPage(1, 100)).rejects.toMatchObject({ code: 'timeout' })
    expect(r.snapshot.content).toBeNull()
    expect(r.snapshot.contentError).toBe('document_timeout')
    expect(renderers[0]!.disposed).toBe(true)
  })

  it('keeps the document after a page error that does not end the renderer', async () => {
    const { r, a } = setup({ renderError: 'bad_size' })
    sendPdf(a)
    await tick()
    await expect(r.renderPage(1, 100)).rejects.toMatchObject({ code: 'bad_size' })
    expect(r.snapshot.content).toMatchObject({ kind: 'document' })
  })

  it('a new document or an image replaces the old one and ends its renderer', async () => {
    const { r, a, renderers } = setup({})
    sendPdf(a)
    await tick()
    sendPdf(a)
    await tick()
    expect(renderers.length).toBe(2)
    expect(renderers[0]!.disposed).toBe(true)
    expect(renderers[1]!.disposed).toBe(false)
    a.send('{"kind":"link","url":"https://example.org/"}')
    await tick()
    expect(renderers[1]!.disposed).toBe(true)
    expect(r.snapshot.content?.kind).toBe('link')
  })

  it('maps renderer codes to translation keys', () => {
    expect(documentErrorCode('unavailable')).toBe('document_unsupported')
    expect(documentErrorCode('too_many_pages')).toBe('document_too_many_pages')
    expect(documentErrorCode('whatever')).toBe('document_failed')
  })
})
