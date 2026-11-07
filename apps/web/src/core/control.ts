// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sender -> receiver control channel (WebRTC DataChannel "control", ordered, reliable).
// Message format: docs/web-m2.md.
// UNCERTAIN: this message set is local to the web client for milestone M2 plus the `document` kinds
// (PDF) that the native clients define. It should move to cc-proto once it has been exercised. The native
// implementation is crates/cc-files. This client receives PDFs (the receiver renders them with the WASM
// build of cc-docs, see pdfRenderer.ts) and sends them (ControlSender.sendDocument).

export const CONTROL_CHANNEL_LABEL = 'control'
export const CHUNK_SIZE = 16 * 1024
// UNCERTAIN: 10 MiB is a guess that keeps memory use on the receiver small; no measurements behind it.
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024
export const IMAGE_MIME_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'] as const
export type ImageMime = (typeof IMAGE_MIME_TYPES)[number]
export const MAX_LINK_LENGTH = 2048
/** Longest message text (the receiver refuses longer ones). */
export const MAX_TEXT_CHARS = 8192
/**
 * Longest file name, counted in Unicode characters (code points), the same way as the native clients
 * (`MAX_NAME_LENGTH` in crates/cc-files). `String.length` counts UTF-16 units and would differ for emoji.
 */
export const MAX_NAME_LENGTH = 255
/** Largest PDF a sender sends and a receiver accepts (64 MiB, the same value as in crates/cc-files). */
export const MAX_DOCUMENT_BYTES = 64 * 1024 * 1024
/** Stop writing when this much is queued in the channel; resume at the low-water mark. */
const HIGH_WATER = 1024 * 1024
const LOW_WATER = 256 * 1024
const PROGRESS_INTERVAL_MS = 100
/** UNCERTAIN: 30 s without a chunk drops a transfer in progress (a guess, not measured on slow links). */
export const STALL_TIMEOUT_MS = 30_000

/** Mirrors `ControlCmd` in cc-proto (serde default enum form). */
export type ControlCmd = 'next_page' | 'prev_page' | 'stop' | { go_to_page: { page: number } }

export type ControlMessage =
  | { kind: 'cmd'; cmd: ControlCmd }
  | { kind: 'image'; name: string; mime: ImageMime; size: number }
  | { kind: 'image_end' }
  | { kind: 'document'; name: string; mime: 'application/pdf'; size: number }
  | { kind: 'document_end' }
  | { kind: 'link'; url: string }

/** The part of RTCDataChannel that is used here (so tests can provide a fake). */
export interface ControlChannel {
  readonly label?: string
  readonly readyState: string
  readonly bufferedAmount: number
  bufferedAmountLowThreshold: number
  binaryType: string
  // `any` event types so that RTCDataChannel (Event / MessageEvent) is assignable.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  onopen: ((this: any, ev: any) => any) | null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  onclose: ((this: any, ev: any) => any) | null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  onmessage: ((this: any, ev: any) => any) | null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  onbufferedamountlow: ((this: any, ev: any) => any) | null
  send(data: string | ArrayBuffer): void
  close(): void
}

export function isImageMime(m: unknown): m is ImageMime {
  return typeof m === 'string' && (IMAGE_MIME_TYPES as readonly string[]).includes(m)
}

/**
 * Accepts only absolute http/https URLs without credentials. Returns the normalised href or null.
 * A link is never opened by code; the receiver's person has to click (see ReceiveView).
 */
export function validateLinkUrl(input: string): string | null {
  const n = checkLinkUrl(input)
  if (n === null) return null
  // Idempotent by construction (the native clients do the same): what does not survive a second pass is refused.
  return checkLinkUrl(n) === n ? n : null
}

function checkLinkUrl(input: string): string | null {
  const s = input.trim()
  if (s.length === 0 || s.length > MAX_LINK_LENGTH) return null
  // The URL parser silently drops tabs/newlines; reject them (and spaces, control chars) up front.
  if (/[\u0000- \u007f-\u009f]/.test(s)) return null
  let u: URL
  try {
    u = new URL(s)
  } catch {
    return null
  }
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return null
  if (u.username !== '' || u.password !== '') return null
  if (u.hostname === '') return null
  // The normalised form (percent-encoding, punycode) can be much longer than the input and must still fit
  // a link message that the receiver accepts.
  if (u.href.length > MAX_LINK_LENGTH) return null
  return u.href
}

/** Number of Unicode characters (code points) in a name. */
export function nameLength(s: string): number {
  return Array.from(s).length
}

/** `true` for a name of 1..MAX_NAME_LENGTH characters. */
function isValidNameLength(s: string): boolean {
  // Cheap bound first: a name of MAX_NAME_LENGTH characters has at most twice as many UTF-16 units.
  return s.length > 0 && s.length <= MAX_NAME_LENGTH * 2 && nameLength(s) <= MAX_NAME_LENGTH
}

function isPositiveInt(n: unknown, max: number): n is number {
  return typeof n === 'number' && Number.isInteger(n) && n >= 1 && n <= max
}

function parseCmd(c: unknown): ControlCmd | null {
  if (c === 'next_page' || c === 'prev_page' || c === 'stop') return c
  if (c && typeof c === 'object') {
    const g = (c as { go_to_page?: { page?: unknown } }).go_to_page
    if (g && typeof g === 'object' && isPositiveInt(g.page, 1_000_000)) return { go_to_page: { page: g.page } }
  }
  return null
}

/** Parses and validates one text message. Returns null for anything unknown or malformed. */
export function parseControlMessage(text: string): ControlMessage | null {
  if (text.length > MAX_TEXT_CHARS) return null
  let v: unknown
  try {
    v = JSON.parse(text)
  } catch {
    return null
  }
  if (!v || typeof v !== 'object') return null
  const o = v as Record<string, unknown>
  switch (o.kind) {
    case 'cmd': {
      const cmd = parseCmd(o.cmd)
      return cmd ? { kind: 'cmd', cmd } : null
    }
    case 'image': {
      if (typeof o.name !== 'string' || !isValidNameLength(o.name)) return null
      if (!isImageMime(o.mime)) return null
      if (!isPositiveInt(o.size, MAX_IMAGE_BYTES)) return null
      const name = sanitizeName(o.name)
      if (name.length === 0) return null
      return { kind: 'image', name, mime: o.mime, size: o.size }
    }
    case 'image_end':
      return { kind: 'image_end' }
    case 'document': {
      if (typeof o.name !== 'string' || !isValidNameLength(o.name)) return null
      if (o.mime !== 'application/pdf') return null
      if (!isPositiveInt(o.size, MAX_DOCUMENT_BYTES)) return null
      const name = sanitizeName(o.name)
      if (name.length === 0) return null
      return { kind: 'document', name, mime: 'application/pdf', size: o.size }
    }
    case 'document_end':
      return { kind: 'document_end' }
    case 'link': {
      if (typeof o.url !== 'string') return null
      const url = validateLinkUrl(o.url)
      return url ? { kind: 'link', url } : null
    }
    default:
      return null
  }
}

/** Splits a buffer into chunks of at most `size` bytes (views, no copy). */
export function chunkBytes(data: Uint8Array, size = CHUNK_SIZE): Uint8Array[] {
  const out: Uint8Array[] = []
  for (let o = 0; o < data.byteLength; o += size) out.push(data.subarray(o, Math.min(o + size, data.byteLength)))
  return out
}

/** Removes bidirectional control characters, which can make a displayed name or host read differently. */
export function stripBidi(s: string): string {
  return s.replace(/[\u200e\u200f\u061c\u202a-\u202e\u2066-\u2069]/g, '')
}

/**
 * Removes control characters, bidirectional controls and other invisible characters from a displayed name:
 * the Unicode "format" characters (Cf: zero-width space and joiners, BOM, soft hyphen, tag characters, ...),
 * line and paragraph separators and a few blank-looking letters. The list is the same as `sanitize_name` in
 * crates/cc-files/src/wire.rs (hand-written on both sides, UNCERTAIN: not generated from the Unicode data).
 */
export function sanitizeName(s: string): string {
  return s.replace(INVISIBLE, '')
}

// eslint-disable-next-line no-control-regex
const INVISIBLE = /[\u0000-\u001f\u007f-\u009f\u00ad\u0600-\u0605\u06dd\u070f\u0890\u0891\u08e2\u115f\u1160\u17b4\u17b5\u180e\u200b-\u200f\u2028-\u202e\u2060-\u206f\u2800\u3164\ufeff\uffa0\ufff9-\ufffb\u{110bd}\u{110cd}\u{13430}-\u{1343f}\u{1bca0}-\u{1bca3}\u{1d173}-\u{1d17a}\u{e0000}-\u{e007f}]/gu

export type TransferErrorCode =
  | 'unexpected_chunk'
  | 'too_large'
  | 'size_mismatch'
  | 'no_transfer'
  | 'bad_chunk'
  | 'stalled'
  | 'document_unsupported'
  | 'document_invalid'
  | 'document_encrypted'
  | 'document_too_large'
  | 'document_too_many_pages'
  | 'document_rejected'
  | 'document_memory'
  | 'document_timeout'
  | 'document_failed'

/**
 * Reassembles one image or PDF from its metadata message and binary chunks.
 * Memory is bounded: one buffer of the announced size is allocated up front and chunks are copied in.
 * Every chunk must be exactly CHUNK_SIZE long unless it completes the image, so the number of
 * messages is bounded by size / CHUNK_SIZE (a flood of tiny messages is rejected).
 */
export class ImageAssembler {
  private cur: { name: string; mime: string; size: number; buf: Uint8Array; received: number } | null = null
  private stallTimer: ReturnType<typeof setTimeout> | undefined

  /** `onStall` is called after `stallMs` without a chunk (or a start) while a transfer is in progress; the buffer is dropped first. */
  constructor(
    private readonly onStall?: () => void,
    private readonly stallMs: number = STALL_TIMEOUT_MS,
  ) {}

  private arm(): void {
    clearTimeout(this.stallTimer)
    this.stallTimer = undefined
    if (!this.cur || !this.onStall) return
    this.stallTimer = setTimeout(() => {
      this.stallTimer = undefined
      this.cur = null
      this.onStall?.()
    }, this.stallMs)
  }

  get active(): boolean {
    return this.cur !== null
  }

  get progress(): { name: string; received: number; size: number } | null {
    return this.cur ? { name: this.cur.name, received: this.cur.received, size: this.cur.size } : null
  }

  /** Begins a transfer; a transfer in progress is dropped. The size limit is enforced by parseControlMessage. */
  start(meta: { name: string; mime: string; size: number }): void {
    this.cur = { ...meta, buf: new Uint8Array(meta.size), received: 0 }
    this.arm()
  }

  /** Returns an error code (and drops the transfer) if the chunk is not acceptable. */
  push(chunk: Uint8Array): TransferErrorCode | null {
    const c = this.cur
    if (!c) return 'unexpected_chunk'
    const end = c.received + chunk.byteLength
    if (end > c.size) {
      this.reset()
      return 'too_large'
    }
    if (chunk.byteLength !== CHUNK_SIZE && end !== c.size) {
      this.reset()
      return 'bad_chunk'
    }
    c.buf.set(chunk, c.received)
    c.received = end
    this.arm()
    return null
  }

  finish(): { blob: Blob; name: string } | TransferErrorCode {
    const r = this.finishBytes()
    if (typeof r === 'string') return r
    return { blob: new Blob([r.bytes as BlobPart], { type: r.mime }), name: r.name }
  }

  /** Like `finish`, but hands out the received bytes (no copy) instead of a Blob. */
  finishBytes(): { bytes: Uint8Array; mime: string; name: string } | TransferErrorCode {
    const c = this.cur
    this.reset()
    if (!c) return 'no_transfer'
    if (c.received !== c.size) return 'size_mismatch'
    return { bytes: c.buf, mime: c.mime, name: c.name }
  }

  reset(): void {
    this.cur = null
    clearTimeout(this.stallTimer)
    this.stallTimer = undefined
  }
}

export interface ControlReceiverHandlers {
  onCmd(cmd: ControlCmd): void
  onProgress(p: { name: string; received: number; size: number } | null): void
  onImage(blob: Blob, name: string): void
  /** A complete PDF (the announced number of bytes arrived). Nothing has looked at the bytes yet. */
  onDocument(bytes: Uint8Array, name: string): void
  onLink(url: string): void
  onError(code: TransferErrorCode | 'bad_message'): void
}

/** Receiver side: feed it every message of the channel. */
export class ControlReceiver {
  private readonly asm = new ImageAssembler(() => {
    // No chunk for STALL_TIMEOUT_MS: the buffer is already dropped, tell the person.
    this.isDocument = false
    this.h.onProgress(null)
    this.h.onError('stalled')
  })
  /** The transfer in progress is a PDF (otherwise an image). */
  private isDocument = false
  private lastProgress = 0
  /** `now` is injectable for tests; progress callbacks are throttled to one per PROGRESS_INTERVAL_MS. */
  constructor(
    private readonly h: ControlReceiverHandlers,
    private readonly now: () => number = () => Date.now(),
  ) {}

  handle(data: unknown): void {
    if (typeof data === 'string') return this.handleText(data)
    if (data instanceof ArrayBuffer) return this.handleBinary(new Uint8Array(data))
    if (ArrayBuffer.isView(data)) return this.handleBinary(new Uint8Array(data.buffer, data.byteOffset, data.byteLength))
    this.h.onError('bad_message')
  }

  reset(): void {
    this.asm.reset()
    this.isDocument = false
  }

  private handleBinary(chunk: Uint8Array): void {
    const err = this.asm.push(chunk)
    if (err) {
      this.h.onProgress(null)
      this.h.onError(err)
      return
    }
    const t = this.now()
    if (t - this.lastProgress >= PROGRESS_INTERVAL_MS) {
      this.lastProgress = t
      this.h.onProgress(this.asm.progress)
    }
  }

  private handleText(text: string): void {
    const m = parseControlMessage(text)
    if (!m) return this.h.onError('bad_message')
    switch (m.kind) {
      case 'cmd':
        if (m.cmd === 'stop') this.reset()
        this.h.onCmd(m.cmd)
        break
      case 'document':
        // Same chunk rule and memory bound as for images (one buffer of the announced size, at most 64 MiB).
        this.isDocument = true
        this.asm.start(m)
        this.lastProgress = this.now()
        this.h.onProgress(this.asm.progress)
        break
      case 'document_end': {
        const r = this.isDocument ? this.asm.finishBytes() : 'no_transfer'
        if (!this.isDocument) this.asm.reset()
        this.isDocument = false
        this.h.onProgress(null)
        if (typeof r === 'string') this.h.onError(r)
        else this.h.onDocument(r.bytes, r.name)
        break
      }
      case 'image':
        this.isDocument = false
        this.asm.start(m)
        this.lastProgress = this.now()
        this.h.onProgress(this.asm.progress)
        break
      case 'image_end': {
        if (this.isDocument) {
          // `image_end` for a PDF transfer: drop it.
          this.asm.reset()
          this.isDocument = false
          this.h.onProgress(null)
          this.h.onError('no_transfer')
          break
        }
        const r = this.asm.finish()
        this.h.onProgress(null)
        if (typeof r === 'string') this.h.onError(r)
        else this.h.onImage(r.blob, r.name)
        break
      }
      case 'link':
        this.h.onLink(m.url)
        break
    }
  }
}

/** What the sender needs from a picked file (a Blob/File satisfies it). */
export interface ImageSource {
  name: string
  type: string
  size: number
  slice(start: number, end: number): { arrayBuffer(): Promise<ArrayBuffer> }
}

export type SendErrorCode = 'not_open' | 'bad_type' | 'too_large' | 'empty' | 'channel_closed' | 'not_pdf'
export class SendError extends Error {
  constructor(readonly code: SendErrorCode) {
    super(code)
  }
}

/** Sender side of the control channel. */
export class ControlSender {
  constructor(private readonly ch: ControlChannel) {
    ch.bufferedAmountLowThreshold = LOW_WATER
  }

  get isOpen(): boolean {
    return this.ch.readyState === 'open'
  }

  private sendJson(m: ControlMessage): void {
    if (!this.isOpen) throw new SendError('not_open')
    this.ch.send(JSON.stringify(m))
  }

  sendCmd(cmd: ControlCmd): void {
    this.sendJson({ kind: 'cmd', cmd })
  }

  /** Returns false if the URL is not an acceptable http(s) link (nothing is sent then). */
  sendLink(url: string): boolean {
    const v = validateLinkUrl(url)
    if (!v) return false
    const msg: ControlMessage = { kind: 'link', url: v }
    // The whole message must fit what the receiver accepts (JSON escaping can lengthen it).
    if (JSON.stringify(msg).length > MAX_TEXT_CHARS) return false
    this.sendJson(msg)
    return true
  }

  /** Resolves when the buffer is low, or when the channel is no longer open (close handler or poll). */
  private waitDrain(): Promise<void> {
    return new Promise((resolve) => {
      const prevClose = this.ch.onclose
      const poll = setInterval(() => {
        if (!this.isOpen) done()
      }, 200)
      const done = () => {
        clearInterval(poll)
        this.ch.onbufferedamountlow = null
        this.ch.onclose = prevClose
        resolve()
      }
      this.ch.onbufferedamountlow = done
      this.ch.onclose = (ev) => {
        prevClose?.call(this.ch, ev)
        done()
      }
      // The channel may have drained between the check and the handler assignment.
      if (this.ch.bufferedAmount <= LOW_WATER || !this.isOpen) done()
    })
  }

  /** Sends an image in 16 KiB chunks with back-pressure. `onProgress` gets bytes sent so far. */
  async sendImage(file: ImageSource, onProgress?: (sent: number, total: number) => void): Promise<void> {
    if (!isImageMime(file.type)) throw new SendError('bad_type')
    if (file.size <= 0) throw new SendError('empty')
    if (file.size > MAX_IMAGE_BYTES) throw new SendError('too_large')
    // Clip by characters (not UTF-16 units, which could cut an emoji in half) and clean like the receiver does.
    const name = sanitizeName(Array.from(file.name).slice(0, MAX_NAME_LENGTH).join('')) || 'image'
    this.sendJson({ kind: 'image', name, mime: file.type, size: file.size })
    await this.sendChunks(file, onProgress)
    this.sendJson({ kind: 'image_end' })
  }

  /**
   * Sends a PDF with the same protocol the native ContentReceiver (crates/cc-files) accepts: a `document`
   * message, 16 KiB binary chunks (all full except the last), `document_end`. The file must start with the
   * `%PDF-` marker within its first 1024 bytes (like the renderer requires); nothing else is checked here,
   * the receiver treats the file as hostile whatever the sender says.
   */
  async sendDocument(file: ImageSource, onProgress?: (sent: number, total: number) => void): Promise<void> {
    // Some systems give no MIME type for a PDF; the marker check below is what counts.
    if (file.type !== 'application/pdf' && file.type !== '') throw new SendError('bad_type')
    if (file.size <= 0) throw new SendError('empty')
    if (file.size > MAX_DOCUMENT_BYTES) throw new SendError('too_large')
    const head = new Uint8Array(await file.slice(0, Math.min(1024, file.size)).arrayBuffer())
    if (!hasPdfMarker(head)) throw new SendError('not_pdf')
    const name = sanitizeName(Array.from(file.name).slice(0, MAX_NAME_LENGTH).join('')) || 'document.pdf'
    this.sendJson({ kind: 'document', name, mime: 'application/pdf', size: file.size })
    await this.sendChunks(file, onProgress)
    this.sendJson({ kind: 'document_end' })
  }

  private async sendChunks(file: ImageSource, onProgress?: (sent: number, total: number) => void): Promise<void> {
    for (let o = 0; o < file.size; o += CHUNK_SIZE) {
      if (this.ch.bufferedAmount > HIGH_WATER) await this.waitDrain()
      if (!this.isOpen) throw new SendError('channel_closed')
      const buf = await file.slice(o, Math.min(o + CHUNK_SIZE, file.size)).arrayBuffer()
      if (!this.isOpen) throw new SendError('channel_closed')
      this.ch.send(buf)
      onProgress?.(Math.min(o + CHUNK_SIZE, file.size), file.size)
    }
  }
}

/** `true` if `%PDF-` occurs in the first 1024 bytes. */
export function hasPdfMarker(head: Uint8Array): boolean {
  const m = [0x25, 0x50, 0x44, 0x46, 0x2d]
  const end = Math.min(head.length, 1024) - m.length
  for (let i = 0; i <= end; i++) if (m.every((b, j) => head[i + j] === b)) return true
  return false
}
