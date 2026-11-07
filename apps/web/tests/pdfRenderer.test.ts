// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createWorkerRenderer, PdfError, type WorkerLike } from '../src/core/pdfRenderer'
import type { PdfReply, PdfRequest } from '../src/core/pdfWorker'

class FakeWorker implements WorkerLike {
  onmessage: WorkerLike['onmessage'] = null
  onerror: WorkerLike['onerror'] = null
  terminated = false
  requests: PdfRequest[] = []
  postMessage(m: PdfRequest): void {
    this.requests.push(m)
  }
  terminate(): void {
    this.terminated = true
  }
  reply(r: PdfReply): void {
    this.onmessage?.({ data: r })
  }
}

const bytes = () => new Uint8Array([0x25, 0x50, 0x44, 0x46])

describe('createWorkerRenderer', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('opens, renders pages one at a time on request, and returns RGBA pixels', async () => {
    const w = new FakeWorker()
    const r = createWorkerRenderer(() => w)
    const opened = r.open(bytes())
    expect(w.requests[0]).toMatchObject({ type: 'open' })
    w.reply({ type: 'opened', pages: 3 })
    await expect(opened).resolves.toBe(3)
    // Nothing is rendered before it is asked for.
    expect(w.requests.length).toBe(1)
    const p1 = r.render(0, 800)
    const p2 = r.render(1, 800)
    await Promise.resolve()
    await Promise.resolve()
    // The second request waits for the first.
    expect(w.requests.filter((q) => q.type === 'render').length).toBe(1)
    const first = w.requests[1] as Extract<PdfRequest, { type: 'render' }>
    w.reply({ type: 'rendered', id: first.id, width: 1, height: 1, rgba: new Uint8Array([1, 2, 3, 255]).buffer })
    await expect(p1).resolves.toMatchObject({ width: 1, height: 1 })
    await Promise.resolve()
    await Promise.resolve()
    const second = w.requests[2] as Extract<PdfRequest, { type: 'render' }>
    expect(second.index).toBe(1)
    w.reply({ type: 'rendered', id: second.id, width: 1, height: 1, rgba: new Uint8Array(4).buffer })
    await expect(p2).resolves.toBeTruthy()
    expect(w.terminated).toBe(false)
  })

  it('rejects an open that fails with the code of the wasm side and ends the worker', async () => {
    const w = new FakeWorker()
    const r = createWorkerRenderer(() => w)
    const opened = r.open(bytes())
    w.reply({ type: 'error', code: 'encrypted' })
    await expect(opened).rejects.toMatchObject({ code: 'encrypted' })
    expect(w.terminated).toBe(true)
    await expect(r.render(0, 100)).rejects.toBeInstanceOf(PdfError)
  })

  it('ends the worker when opening takes longer than the limit', async () => {
    const w = new FakeWorker()
    const r = createWorkerRenderer(() => w, { openMs: 1000 })
    const opened = r.open(bytes())
    const caught = opened.catch((e: PdfError) => e.code)
    await vi.advanceTimersByTimeAsync(1001)
    await expect(caught).resolves.toBe('timeout')
    expect(w.terminated).toBe(true)
  })

  it('ends the worker when one render takes longer than the limit', async () => {
    const w = new FakeWorker()
    const r = createWorkerRenderer(() => w, { renderMs: 500 })
    const opened = r.open(bytes())
    w.reply({ type: 'opened', pages: 1 })
    await opened
    const page = r.render(0, 100)
    const caught = page.catch((e: PdfError) => e.code)
    await vi.advanceTimersByTimeAsync(501)
    await expect(caught).resolves.toBe('timeout')
    expect(w.terminated).toBe(true)
  })

  it('keeps the document after a harmless page error but ends it after a crash', async () => {
    const w = new FakeWorker()
    const r = createWorkerRenderer(() => w)
    const opened = r.open(bytes())
    w.reply({ type: 'opened', pages: 2 })
    await opened
    const bad = r.render(9, 100)
    await Promise.resolve()
    w.reply({ type: 'error', id: (w.requests[1] as { id: number }).id, code: 'page_out_of_range' })
    await expect(bad).rejects.toMatchObject({ code: 'page_out_of_range' })
    expect(w.terminated).toBe(false)
    const crash = r.render(0, 100)
    await Promise.resolve()
    await Promise.resolve()
    w.onerror?.({}) // a trap in the module
    await expect(crash).rejects.toMatchObject({ code: 'failed' })
    expect(w.terminated).toBe(true)
  })

  it('reports "unavailable" when no worker can be created, and "closed" after dispose', async () => {
    const r = createWorkerRenderer(() => {
      throw new Error('no workers')
    })
    await expect(r.open(bytes())).rejects.toMatchObject({ code: 'unavailable' })
    const w = new FakeWorker()
    const r2 = createWorkerRenderer(() => w)
    const opened = r2.open(bytes())
    r2.dispose()
    await expect(opened).rejects.toMatchObject({ code: 'closed' })
    expect(w.terminated).toBe(true)
  })
})
