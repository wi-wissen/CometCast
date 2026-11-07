// SPDX-License-Identifier: GPL-3.0-or-later
//
// Web Worker that holds one opened PDF and renders pages on request, using the WASM build of
// cc-docs/hayro (crates/cc-docs-wasm, built by scripts/build-wasm.sh into /pdf/).
// Why a worker: on wasm32 the renderer has no time limit, no panic protection and a hostile file can
// make it trap or run long. The page (pdfRenderer.ts) ends this worker with terminate() on a timeout or
// failure; nothing in it can reach the page, the DOM or the network except the one script and one .wasm
// file that it loads from the same origin. The PDF bytes are only parsed and drawn: no script, no action
// and no link of the document is run or exposed.
// Protocol: see PdfRequest / PdfReply below.

export type PdfRequest =
  | { type: 'open'; bytes: ArrayBuffer }
  | { type: 'render'; id: number; index: number; width: number }

export type PdfReply =
  | { type: 'opened'; pages: number }
  | { type: 'rendered'; id: number; width: number; height: number; rgba: ArrayBuffer }
  | { type: 'error'; id?: number; code: string }

interface WasmDocument {
  page_count(): number
  render(index: number, widthPx: number): { width: number; height: number; rgba(): Uint8Array; free(): void }
  free(): void
}
interface WasmModule {
  default(input: { module_or_path: string }): Promise<unknown>
  PdfDocument: { open(bytes: Uint8Array): WasmDocument }
}

// UNCERTAIN: the files are expected at <base>pdf/ (copied from apps/web/public/pdf by Vite).
const BASE = new URL(`${import.meta.env.BASE_URL}pdf/`, self.location.origin).href

let mod: Promise<WasmModule> | null = null
function load(): Promise<WasmModule> {
  mod ??= (async () => {
    const m = (await import(/* @vite-ignore */ `${BASE}cc_docs_wasm.js`)) as WasmModule
    await m.default({ module_or_path: `${BASE}cc_docs_wasm_bg.wasm` })
    return m
  })()
  return mod
}

let doc: WasmDocument | null = null
const post = (m: PdfReply, transfer: Transferable[] = []) =>
  (self as unknown as { postMessage(m: PdfReply, t: Transferable[]): void }).postMessage(m, transfer)

/** The wasm side throws an Error whose message is a short code; anything else is a generic failure. */
function codeOf(e: unknown): string {
  const m = e instanceof Error ? e.message : ''
  return /^[a-z_]{3,20}$/.test(m) ? m : 'failed'
}

self.onmessage = async (ev: MessageEvent<PdfRequest>) => {
  const req = ev.data
  if (req.type === 'open') {
    let m: WasmModule
    try {
      m = await load()
    } catch {
      post({ type: 'error', code: 'unavailable' })
      return
    }
    try {
      doc?.free()
      doc = null
      doc = m.PdfDocument.open(new Uint8Array(req.bytes))
      post({ type: 'opened', pages: doc.page_count() })
    } catch (e) {
      post({ type: 'error', code: codeOf(e) })
    }
  } else if (req.type === 'render') {
    if (!doc) return post({ type: 'error', id: req.id, code: 'failed' })
    try {
      const page = doc.render(req.index, req.width)
      const rgba = page.rgba()
      const width = page.width
      const height = page.height
      page.free()
      const buf = rgba.buffer as ArrayBuffer
      post({ type: 'rendered', id: req.id, width, height, rgba: buf }, [buf])
    } catch (e) {
      post({ type: 'error', id: req.id, code: codeOf(e) })
    }
  }
}
