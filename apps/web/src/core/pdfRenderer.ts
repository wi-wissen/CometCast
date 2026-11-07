// SPDX-License-Identifier: GPL-3.0-or-later
//
// Main-thread side of the PDF renderer: owns one Web Worker (pdfWorker.ts) with one opened document and
// renders pages ON REQUEST, one at a time. Limits (all UNCERTAIN, guesses for classroom PDFs, not measured):
// opening and every render have a wall-clock limit; when it passes, or the worker fails, the worker is
// terminated and the renderer is dead (the document is closed; the person can ask the sender again).
// The renderer never runs anything from the PDF and never opens a link.
import type { PdfReply, PdfRequest } from './pdfWorker'

export const OPEN_TIMEOUT_MS = 20_000
export const RENDER_TIMEOUT_MS = 20_000

/** A rendered page: RGBA, `width * height * 4` bytes. */
export interface PageBitmap {
  width: number
  height: number
  rgba: Uint8ClampedArray<ArrayBuffer>
}

/** The part of Worker that is used here (tests provide a fake). */
export interface WorkerLike {
  postMessage(m: PdfRequest, transfer?: Transferable[]): void
  terminate(): void
  onmessage: ((ev: { data: PdfReply }) => void) | null
  onerror: ((ev: unknown) => void) | null
}

export class PdfError extends Error {
  /** A code of the wasm side (`invalid`, `encrypted`, ...), or `unavailable`, `timeout`, `failed`, `closed`, `superseded` (a queued render replaced by a newer one). */
  constructor(readonly code: string) {
    super(code)
  }
}

export interface PdfRenderer {
  /** Resolves with the page count. Takes over the buffer (it is transferred to the worker). */
  open(bytes: Uint8Array): Promise<number>
  /** Renders page `index` (0-based), about `width` pixels wide. One runs at a time; a request that is still waiting when a newer one arrives is dropped (`superseded`). */
  render(index: number, width: number): Promise<PageBitmap>
  /** Ends the worker; pending calls reject with `closed`. */
  dispose(): void
}

export type PdfRendererFactory = () => PdfRenderer

export function createWorkerRenderer(
  makeWorker: () => WorkerLike = () =>
    new Worker(new URL('./pdfWorker.ts', import.meta.url), { type: 'module' }) as unknown as WorkerLike,
  limits: { openMs?: number; renderMs?: number } = {},
): PdfRenderer {
  const openMs = limits.openMs ?? OPEN_TIMEOUT_MS
  const renderMs = limits.renderMs ?? RENDER_TIMEOUT_MS
  let worker: WorkerLike | null = null
  let dead: PdfError | null = null
  let nextId = 1
  let pendingOpen: { resolve(n: number): void; reject(e: PdfError): void; timer: ReturnType<typeof setTimeout> } | null = null
  const pendingRender = new Map<
    number,
    { resolve(b: PageBitmap): void; reject(e: PdfError): void; timer: ReturnType<typeof setTimeout> }
  >()
  // The render that waits for the one in flight (see render()).
  let queued: { index: number; width: number; resolve(b: PageBitmap): void; reject(e: PdfError): void } | null = null

  function pump(): void {
    if (!queued || pendingRender.size > 0 || dead || !worker) return
    const q = queued
    queued = null
    const id = nextId++
    const w = worker
    const timer = setTimeout(() => kill(new PdfError('timeout')), renderMs)
    pendingRender.set(id, {
      resolve: (b) => {
        q.resolve(b)
        pump()
      },
      reject: (e) => {
        q.reject(e)
        pump()
      },
      timer,
    })
    w.postMessage({ type: 'render', id, index: q.index, width: q.width })
  }

  function kill(err: PdfError): void {
    if (dead) return
    dead = err
    worker?.terminate()
    worker = null
    if (pendingOpen) {
      clearTimeout(pendingOpen.timer)
      pendingOpen.reject(err)
      pendingOpen = null
    }
    for (const p of pendingRender.values()) {
      clearTimeout(p.timer)
      p.reject(err)
    }
    pendingRender.clear()
    queued?.reject(err)
    queued = null
  }

  function onReply(r: PdfReply): void {
    if (r.type === 'opened') {
      if (pendingOpen) {
        clearTimeout(pendingOpen.timer)
        pendingOpen.resolve(r.pages)
        pendingOpen = null
      }
    } else if (r.type === 'rendered') {
      const p = pendingRender.get(r.id)
      if (!p) return
      pendingRender.delete(r.id)
      clearTimeout(p.timer)
      p.resolve({ width: r.width, height: r.height, rgba: new Uint8ClampedArray(r.rgba) })
    } else if (r.type === 'error') {
      if (r.id === undefined) {
        // An open that failed leaves no document: the renderer is finished (rejects the pending open).
        kill(new PdfError(r.code))
        return
      }
      const p = pendingRender.get(r.id)
      if (!p) return
      // A page that fails to render does not end the document (an invalid page index or size is local),
      // except after a memory or generic failure, where the wasm instance may be unusable: then the
      // worker is ended first (this also rejects the page and anything queued with the same code).
      if (r.code === 'memory' || r.code === 'failed') return kill(new PdfError(r.code))
      pendingRender.delete(r.id)
      clearTimeout(p.timer)
      p.reject(new PdfError(r.code))
    }
  }

  return {
    open(bytes) {
      if (dead) return Promise.reject(dead)
      if (worker) return Promise.reject(new PdfError('failed')) // one document per renderer
      return new Promise<number>((resolve, reject) => {
        let w: WorkerLike
        try {
          w = makeWorker()
        } catch {
          dead = new PdfError('unavailable')
          reject(dead)
          return
        }
        worker = w
        w.onmessage = (ev) => onReply(ev.data)
        // A script or wasm load that fails, or a trap in the module, ends up here or in a timeout.
        w.onerror = () => kill(new PdfError('failed'))
        const timer = setTimeout(() => kill(new PdfError('timeout')), openMs)
        pendingOpen = { resolve, reject, timer }
        const buf = bytes.buffer as ArrayBuffer
        w.postMessage({ type: 'open', bytes: buf }, [buf])
      })
    },
    render(index, width) {
      if (dead) return Promise.reject(dead)
      if (!worker) return Promise.reject(new PdfError('closed'))
      return new Promise<PageBitmap>((resolve, reject) => {
        // At most one render in flight and one waiting: a newer request replaces the waiting one, whose
        // caller gets `superseded` (it is no longer wanted).
        queued?.reject(new PdfError('superseded'))
        queued = { index, width, resolve, reject }
        pump()
      })
    },
    dispose() {
      kill(new PdfError('closed'))
    },
  }
}
