// SPDX-License-Identifier: GPL-3.0-or-later
//
// Receiver state machine: creates a room, waits for a sender, answers its offer, exposes the stream.
import { createPeer, type Peer, type PeerFactory, type PeerState } from './peer'
import { CONTROL_CHANNEL_LABEL, ControlReceiver, type ControlChannel, type ControlCmd, type TransferErrorCode } from './control'
import { createWorkerRenderer, PdfError, type PageBitmap, type PdfRenderer, type PdfRendererFactory } from './pdfRenderer'
import { SignalingClient, type ServerMessage, type SocketFactory } from './signaling'

export type ReceiverPhase =
  | 'idle'
  | 'connecting' // opening the WebSocket / handshake / room creation
  | 'checking' // the server asked for a proof-of-work puzzle ("Checking your browser..."); solving it
  | 'waiting_sender' // room code is shown
  | 'negotiating' // a sender joined, WebRTC is being set up
  | 'receiving' // media is flowing
  | 'peer_left' // sender left; the room stays, a new sender may join
  | 'error'

/** Content pushed by the sender over the control channel. Shown only locally. */
export type ReceivedContent =
  | { kind: 'image'; url: string; name: string }
  | { kind: 'link'; url: string; host: string }
  /** A PDF. `pages` is 0 while it is being opened; `page` is the shown page, counted from 1. */
  | { kind: 'document'; name: string; pages: number; page: number }

export interface ReceiverState {
  phase: ReceiverPhase
  code: string | null
  /** Server error code or a local one (connection_lost, peer_failed, altcha_failed). Set when phase is 'error'. */
  errorCode: string | null
  stream: MediaStream | null
  content: ReceivedContent | null
  /** Image transfer in progress (bytes received so far), else null. */
  transfer: { name: string; received: number; size: number } | null
  /** Last transfer problem (TransferErrorCode or bad_message), cleared by the next content. */
  contentError: TransferErrorCode | 'bad_message' | null
}

export interface ReceiverDeps {
  url: string
  onChange: (s: ReceiverState) => void
  socketFactory?: SocketFactory
  peerFactory?: PeerFactory
  /**
   * Fetches and solves an ALTCHA puzzle; returns the text for create_room.altcha. Called only after
   * the server answered altcha_required. Reject with an object that has a string `code`
   * (rate_limited, banned, ...) to show that error; any other rejection becomes altcha_failed.
   */
  altcha?: (signal: AbortSignal) => Promise<string>
  /** STUN/TURN list (GET /ice-servers). Must not reject; an empty list means none. */
  iceServers?: () => Promise<RTCIceServer[]>
  /** Creates the renderer for one received PDF (default: a Web Worker with the WASM build of cc-docs). */
  pdfRenderer?: PdfRendererFactory
  /** Test seam; defaults to URL.createObjectURL / revokeObjectURL. */
  objectUrl?: { create(b: Blob): string; revoke(u: string): void }
}

/** Maps an error code of the renderer to the translation key suffix of the receiver. */
export function documentErrorCode(code: string): TransferErrorCode {
  switch (code) {
    case 'unavailable':
      return 'document_unsupported'
    case 'invalid':
    case 'encrypted':
    case 'too_large':
    case 'too_many_pages':
    case 'rejected':
    case 'memory':
    case 'timeout':
      return `document_${code}`
    default:
      return 'document_failed'
  }
}

/** After these the renderer is gone (worker ended), so the document is closed. */
const FATAL_RENDER_CODES = new Set(['timeout', 'failed', 'memory', 'closed', 'unavailable'])

const EMPTY: ReceiverState = {
  phase: 'idle',
  code: null,
  errorCode: null,
  stream: null,
  content: null,
  transfer: null,
  contentError: null,
}

export class ReceiverSession {
  private state: ReceiverState = { ...EMPTY, phase: 'idle' }
  private readonly control = new ControlReceiver({
    onCmd: (cmd) => this.onCmd(cmd),
    onProgress: (transfer) => this.set({ transfer }),
    onImage: (blob, name) => {
      this.clearContent()
      this.set({ content: { kind: 'image', url: this.urls.create(blob), name }, contentError: null })
    },
    onDocument: (bytes, name) => this.openDocument(bytes, name),
    onLink: (url) => {
      this.clearContent()
      this.set({ content: { kind: 'link', url, host: new URL(url).host }, contentError: null })
    },
    onError: (contentError) => this.set({ contentError }),
  })
  private get urls() {
    return this.deps.objectUrl ?? { create: (b: Blob) => URL.createObjectURL(b), revoke: (u: string) => URL.revokeObjectURL(u) }
  }
  private client: SignalingClient | null = null
  private peer: Peer | null = null
  private readonly peerFactory: PeerFactory
  private run = 0 // invalidates async work of an earlier start()/stop()
  private iceServers: RTCIceServer[] = []
  private iceReady = true
  private held: ServerMessage[] = [] // messages that arrived before the ICE server list
  private altchaTried = false
  private altchaAbort: AbortController | null = null

  constructor(private readonly deps: ReceiverDeps) {
    this.peerFactory = deps.peerFactory ?? ((h, c) => createPeer(h, c))
  }

  get snapshot(): ReceiverState {
    return this.state
  }

  private set(patch: Partial<ReceiverState>): void {
    this.state = { ...this.state, ...patch }
    this.deps.onChange(this.state)
  }

  private renderer: PdfRenderer | null = null
  private docRun = 0 // invalidates the completion of an earlier open

  private onCmd(cmd: ControlCmd): void {
    if (cmd === 'stop') this.clearContent()
    else if (cmd === 'next_page') this.goToPage((this.state.content?.kind === 'document' ? this.state.content.page : 0) + 1)
    else if (cmd === 'prev_page') this.goToPage((this.state.content?.kind === 'document' ? this.state.content.page : 0) - 1)
    else this.goToPage(cmd.go_to_page.page)
  }

  /** Shows page `n` (from 1, clamped to the page count). Ignored while no document is shown or opened. */
  goToPage(n: number): void {
    const c = this.state.content
    if (c?.kind !== 'document' || c.pages === 0) return
    const page = Math.min(Math.max(Math.trunc(n), 1), c.pages)
    if (page !== c.page) this.set({ content: { ...c, page } })
  }
  nextPage(): void {
    this.onCmd('next_page')
  }
  prevPage(): void {
    this.onCmd('prev_page')
  }

  private openDocument(bytes: Uint8Array, name: string): void {
    this.clearContent()
    const factory = this.deps.pdfRenderer ?? (typeof Worker === 'undefined' ? null : () => createWorkerRenderer())
    if (!factory) return this.set({ contentError: 'document_unsupported' })
    const run = ++this.docRun
    const renderer = factory()
    this.renderer = renderer
    this.set({ content: { kind: 'document', name, pages: 0, page: 1 }, contentError: null })
    renderer.open(bytes).then(
      (pages) => {
        const c = this.state.content
        if (run !== this.docRun || c?.kind !== 'document') return
        this.set({ content: { ...c, pages } })
      },
      (e: unknown) => {
        if (run !== this.docRun) return
        this.clearContent()
        this.set({ contentError: documentErrorCode(e instanceof PdfError ? e.code : 'failed') })
      },
    )
  }

  /**
   * Renders page `page` (from 1) about `width` pixels wide, on demand. Rejects with a PdfError. If the
   * renderer is gone afterwards (time limit, memory, crash) the document is closed with a notice.
   */
  async renderPage(page: number, width: number): Promise<PageBitmap> {
    const r = this.renderer
    const run = this.docRun
    if (!r) throw new PdfError('closed')
    try {
      return await r.render(page - 1, width)
    } catch (e) {
      const code = e instanceof PdfError ? e.code : 'failed'
      if (run === this.docRun && FATAL_RENDER_CODES.has(code) && code !== 'closed') {
        this.clearContent()
        this.set({ contentError: documentErrorCode(code) })
      }
      throw e
    }
  }

  /** Closes the shown image, link or document (also used by the receiver's close/dismiss button). */
  clearContent(): void {
    this.docRun++
    this.renderer?.dispose()
    this.renderer = null
    const c = this.state.content
    if (c?.kind === 'image') this.urls.revoke(c.url)
    if (c || this.state.transfer || this.state.contentError) this.set({ content: null, transfer: null, contentError: null })
  }

  start(): void {
    this.teardown()
    const run = ++this.run
    this.altchaTried = false
    this.iceServers = []
    this.iceReady = !this.deps.iceServers
    this.held = []
    this.set({ ...EMPTY, phase: 'connecting' })
    if (this.deps.iceServers) {
      // Fetched while the room is being created; a sender needs a person to type the code,
      // so the list is normally there long before the first peer is made.
      void this.deps.iceServers().then(
        (list) => this.iceLoaded(run, list),
        () => this.iceLoaded(run, []),
      )
    }
    this.client = new SignalingClient(
      this.deps.url,
      'receiver',
      {
        onReady: () => this.client?.send({ type: 'create_room' }),
        onMessage: (m) => this.onMessage(m),
        onClose: () => this.fail('connection_lost'),
      },
      this.deps.socketFactory,
    )
    this.client.open()
  }

  stop(): void {
    this.run++
    this.teardown()
    this.set({ ...EMPTY, phase: 'idle' })
  }

  private iceLoaded(run: number, list: RTCIceServer[]): void {
    if (run !== this.run) return
    this.iceServers = list
    this.iceReady = true
    for (const m of this.held.splice(0)) this.onMessage(m)
  }

  /** The server wants a solved puzzle before it creates a room (open mode). */
  private solveAltcha(): void {
    const run = this.run
    const ctl = new AbortController()
    this.altchaAbort = ctl
    this.altchaTried = true
    this.set({ phase: 'checking' })
    this.deps.altcha!(ctl.signal).then(
      (payload) => {
        if (run !== this.run || ctl.signal.aborted) return
        this.altchaAbort = null
        this.set({ phase: 'connecting' })
        this.client?.send({ type: 'create_room', altcha: payload })
      },
      (e: unknown) => {
        if (run !== this.run || ctl.signal.aborted) return
        const code = (e as { code?: unknown } | null)?.code
        this.fail(typeof code === 'string' ? code : 'altcha_failed')
      },
    )
  }

  private teardown(): void {
    this.altchaAbort?.abort()
    this.altchaAbort = null
    this.held = []
    this.clearContent()
    this.control.reset()
    this.peer?.close()
    this.peer = null
    this.client?.close()
    this.client = null
  }

  private fail(code: string): void {
    if (this.state.phase === 'error' || this.state.phase === 'idle') return
    this.teardown()
    this.set({ phase: 'error', errorCode: code, stream: null })
  }

  private newPeer(): Peer {
    this.peer?.close()
    const peer = this.peerFactory(
      {
        onIceCandidate: (candidate) => this.client?.send({ type: 'ice_candidate', candidate }),
        onTrack: (stream) => this.set({ stream }),
        onDataChannel: (ch) => this.attachChannel(ch),
        onState: (s: PeerState) => {
          if (s === 'connected') this.set({ phase: 'receiving' })
          else if (s === 'failed') this.fail('peer_failed')
        },
      },
      { iceServers: this.iceServers },
    )
    this.peer = peer
    return peer
  }

  private attachChannel(ch: ControlChannel): void {
    if (ch.label !== CONTROL_CHANNEL_LABEL) return // other labels are not defined; ignore them
    ch.binaryType = 'arraybuffer'
    ch.onmessage = (ev) => this.control.handle(ev.data)
  }

  private onMessage(m: ServerMessage): void {
    // A peer must not be made before the ICE server list is known (rarely waits at all).
    if (!this.iceReady && (m.type === 'peer_joined' || m.type === 'offer' || m.type === 'ice_candidate')) {
      this.held.push(m)
      return
    }
    switch (m.type) {
      case 'room_created':
        this.set({ phase: 'waiting_sender', code: m.code })
        break
      case 'peer_joined':
        this.newPeer()
        this.set({ phase: 'negotiating', stream: null })
        break
      case 'offer': {
        const peer = this.peer ?? this.newPeer()
        if (this.state.phase !== 'receiving') this.set({ phase: 'negotiating' })
        peer
          .acceptOffer(m.sdp)
          .then((sdp) => this.client?.send({ type: 'answer', sdp }))
          .catch(() => this.fail('peer_failed'))
        break
      }
      case 'ice_candidate':
        this.peer?.addIceCandidate(m.candidate).catch(() => {
          // A single bad candidate is not fatal; the connection may still succeed with others.
        })
        break
      case 'peer_left':
        this.peer?.close()
        this.peer = null
        this.clearContent()
        this.control.reset()
        this.set({ phase: 'peer_left', stream: null })
        break
      case 'error':
        // Open servers answer the first create_room with altcha_required; the connection stays open.
        if (m.code === 'altcha_required' && this.deps.altcha && !this.altchaTried && this.state.phase === 'connecting') {
          this.solveAltcha()
          break
        }
        // not_in_room can arrive when the sender has just left; the room itself survives.
        if (m.code === 'not_in_room' && this.state.phase !== 'connecting') break
        this.fail(m.code)
        break
      default:
        break // control, answer, joined: not used by the receiver in M1
    }
  }
}
