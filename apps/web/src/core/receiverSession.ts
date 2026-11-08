// SPDX-License-Identifier: GPL-3.0-or-later
//
// Receiver state machine: creates a room, waits for a sender, answers its offer, exposes the stream.
import { createPeer, type Peer, type PeerFactory, type PeerState } from './peer'
import { CONTROL_CHANNEL_LABEL, ControlReceiver, type ControlChannel, type ControlCmd, type TransferErrorCode } from './control'
import { createWorkerRenderer, PdfError, type PageBitmap, type PdfRenderer, type PdfRendererFactory } from './pdfRenderer'
import { SignalingClient, type JoinMode, type ServerMessage, type SocketFactory } from './signaling'
import { BusyMemory } from './busyMemory'
import { checkImageSize } from './imageSize'
import type { IceFetchState } from './altchaClient'

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
  | { kind: 'document'; id: number; name: string; pages: number; page: number } // `id`: one per received transfer (also for the same name)

/** A sender that waits for the host's decision (join mode "confirm"). All text is untrusted: show as plain text. */
export interface JoinRequest {
  id: number
  /** Cleaned by the server (no control/bidi characters, at most 40 characters). */
  name: string
  /** Two-digit number that the sender's waiting screen shows too. */
  check: number
  /** Milliseconds since the epoch when it arrived (for the countdown). */
  receivedAt: number
}

/** A short message about something that happened to the room's join settings. */
export type JoinNotice =
  | { kind: 'locked_discarded'; count: number }
  | { kind: 'open_disabled' } // the server does not allow open rooms; "confirm" is used instead

/** How long the server lets a request wait (its default). UNCERTAIN: an operator can change it; the server decides. */
export const JOIN_REQUEST_SECS = 60

/** An entry of the room's block list (blocked for the rest of this room session). */
export interface BlockedEntry {
  id: number
  /** Cleaned name, for the list only. */
  name: string
}

/**
 * Things that happened that the display tells the host about in a short toast
 * (docs/design/first-run-and-display.md, 4.6). `name` is the sender's cleaned name (may be empty).
 */
export type ReceiverEvent =
  | { kind: 'joined'; name: string } // a sender is connected (open mode)
  | { kind: 'allowed'; name: string }
  | { kind: 'denied'; name: string }
  | { kind: 'blocked'; name: string }
  | { kind: 'expired'; name: string }
  | { kind: 'withdrawn'; name: string }
  | { kind: 'left'; name: string }
  | { kind: 'lockedRejected'; n: number } // not shown any more (a request closes Settings); the session still reports it
  | { kind: 'sessionEnded' }
  /** The server refused mode Open (`open_rooms_disabled`): the room is Approval on; the display toasts it. */
  | { kind: 'openDisabled' }

export interface ReceiverState {
  phase: ReceiverPhase
  code: string | null
  /** Who may join (as confirmed by the server). */
  joinMode: JoinMode
  /** Waiting join requests, oldest first. */
  requests: JoinRequest[]
  /** Name of the connected sender, or null. */
  peerName: string | null
  /** Blocked senders (the host can unblock them). */
  blocks: BlockedEntry[]
  notice: JoinNotice | null
  /** Server error code or a local one (connection_lost, peer_failed, altcha_failed). Set when phase is 'error'. */
  errorCode: string | null
  stream: MediaStream | null
  content: ReceivedContent | null
  /** Image transfer in progress (bytes received so far), else null. */
  transfer: { name: string; received: number; size: number } | null
  /** Last transfer problem (TransferErrorCode or bad_message), cleared by the next content. */
  contentError: TransferErrorCode | 'bad_message' | null
  /** Counts the problems reported in `contentError`, so the same code twice in a row is told twice. */
  contentErrorSeq?: number
  /** Connection check code once connected (compare with the sender), else absent. */
  sas?: string
  /** With `peer_failed`: how the last `GET /ice-servers` ended (decision 13.45). The display shows nothing of it; the presenter's page tells the person. */
  iceState?: IceFetchState
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
  /** How the last fetch of `iceServers` ended (read when the peer connection fails). */
  iceState?: () => IceFetchState | undefined
  /** Creates the renderer for one received PDF (default: a Web Worker with the WASM build of cc-docs). */
  pdfRenderer?: PdfRendererFactory
  /** Join mode of the room that is created (default "open"). Changed later with setJoinMode. */
  joinMode?: JoinMode
  /** The display's room name (from the setup). Sent with `create_room`; senders see it on their pages. */
  roomName?: string
  /** Called for the events of {@link ReceiverEvent} (the display shows them as toasts). */
  onEvent?: (e: ReceiverEvent) => void
  /** Test seam: the clock (default Date.now). */
  now?: () => number
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

/** Refuses an image whose header is unreadable or describes too many pixels to decode safely. */
function imagePixelCheck(bytes: Uint8Array, mime: string): TransferErrorCode | null {
  const r = checkImageSize(bytes, mime)
  return r === 'ok' ? null : r === 'too_many_pixels' ? 'image_too_large' : 'image_invalid'
}

/** After these the renderer is gone (worker ended), so the document is closed. */
const FATAL_RENDER_CODES = new Set(['timeout', 'failed', 'memory', 'closed', 'unavailable'])

/**
 * Which peer connection an offer comes from: the session id of its `o=` line and its first ICE
 * username fragment. Both stay the same across renegotiations of one connection and are new for a
 * new connection. `null` when the text has neither (then the offer is taken as a renegotiation).
 */
export function sdpIdentity(sdp: string): string | null {
  const sess = /^o=\S+ (\d+) /m.exec(sdp)?.[1]
  const ufrag = /^a=ice-ufrag:(\S+)/m.exec(sdp)?.[1]
  return sess === undefined && ufrag === undefined ? null : `${sess ?? ''}|${ufrag ?? ''}`
}

const EMPTY: ReceiverState = {
  phase: 'idle',
  code: null,
  joinMode: 'open',
  requests: [],
  peerName: null,
  blocks: [],
  notice: null,
  errorCode: null,
  stream: null,
  content: null,
  transfer: null,
  contentError: null,
  sas: undefined,
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
  }, undefined, imagePixelCheck)
  private get urls() {
    return this.deps.objectUrl ?? { create: (b: Blob) => URL.createObjectURL(b), revoke: (u: string) => URL.revokeObjectURL(u) }
  }
  private client: SignalingClient | null = null
  private peer: Peer | null = null
  private readonly peerFactory: PeerFactory
  private run = 0 // invalidates async work of an earlier start()/stop()
  private readonly busy = new BusyMemory()
  /** The control channel (for the `page` messages to the presenter). */
  private channel: ControlChannel | null = null
  private lastPageSent = ''
  /** Identity of the peer connection the last offer came from (see {@link sdpIdentity}). */
  private offerId: string | null = null
  private iceServers: RTCIceServer[] = []
  private iceReady = true
  private held: ServerMessage[] = [] // messages that arrived before the ICE server list
  private altchaTried = false
  private altchaAbort: AbortController | null = null
  private altchaPayload: string | undefined
  private mode: JoinMode
  /** The mode a new room (after the session ends) starts with: the last one that was not "locked". */
  private defaultMode: JoinMode
  /** The id of a request that was allowed, while its sender is about to connect (so "connected" is not toasted a second time). */
  private allowedPending: number | null = null
  /** Offers of the current peer are answered in order (see the `offer` case). */
  private answerChain: Promise<void> = Promise.resolve()

  constructor(private readonly deps: ReceiverDeps) {
    this.peerFactory = deps.peerFactory ?? ((h, c) => createPeer(h, c))
    this.mode = deps.joinMode ?? 'open'
    this.defaultMode = this.mode === 'locked' ? 'open' : this.mode
  }

  private emit(e: ReceiverEvent): void {
    this.deps.onEvent?.(e)
  }

  get snapshot(): ReceiverState {
    return this.state
  }

  private set(patch: Partial<ReceiverState>): void {
    if (patch.contentError) patch = { ...patch, contentErrorSeq: (this.state.contentErrorSeq ?? 0) + 1 }
    this.state = { ...this.state, ...patch }
    this.deps.onChange(this.state)
    if (patch.content !== undefined) this.reportPage()
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
    this.set({ content: { kind: 'document', id: run, name, pages: 0, page: 1 }, contentError: null })
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
    this.altchaPayload = undefined
    this.iceServers = []
    this.iceReady = !this.deps.iceServers
    this.held = []
    this.set({ ...EMPTY, phase: 'connecting', joinMode: this.mode })
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
        onReady: () => this.sendCreateRoom(),
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
    this.set({ ...EMPTY, phase: 'idle', joinMode: this.mode })
  }

  private sendCreateRoom(): void {
    this.client?.send({
      type: 'create_room',
      ...(this.altchaPayload ? { altcha: this.altchaPayload } : {}),
      join_mode: this.mode,
      ...(this.deps.roomName ? { room_name: this.deps.roomName } : {}),
    })
  }

  // --- Join approval (the display device decides who may join) ----------------------------------

  /** Changes who may join, at any time. The state changes when the server confirms. */
  setJoinMode(mode: JoinMode): void {
    // Locked lasts until the session ends or the display restarts: it never becomes the default.
    if (mode !== 'locked') this.defaultMode = mode
    // Locking rejects everybody who waits (the server tells them too).
    if (mode === 'locked' && this.mode !== 'locked' && this.state.requests.length > 0) {
      this.emit({ kind: 'lockedRejected', n: this.state.requests.length })
    }
    this.mode = mode
    if (this.state.code !== null) this.client?.send({ type: 'set_join_mode', join_mode: mode })
  }

  /**
   * Allows (`true`) or denies a waiting request. The request leaves the list at once. `block`
   * (only with a denial) also blocks the sender's address for the rest of this room session.
   */
  decide(id: number, allow: boolean, block = false): void {
    const req = this.state.requests.find((r) => r.id === id)
    if (!req) return
    this.client?.send({ type: 'join_decision', id, allow, ...(block && !allow ? { block: true } : {}) })
    if (allow) this.allowedPending = id
    this.emit({ kind: allow ? 'allowed' : block ? 'blocked' : 'denied', name: req.name })
    this.set({ requests: this.state.requests.filter((r) => r.id !== id) })
  }

  /** Denies everybody who waits. */
  denyAll(): void {
    if (this.state.requests.length === 0) return
    this.client?.send({ type: 'join_decision', allow: false })
    this.set({ requests: [] })
  }

  /** Ends the presenter's session: removes the connected sender (it has to ask again); `block` also blocks it. */
  removePeer(block = false): void {
    this.client?.send({ type: 'remove_peer', ...(block ? { block: true } : {}) })
  }

  /** Takes an entry off the block list. */
  unblock(id: number): void {
    this.client?.send({ type: 'unblock', id })
  }

  /** Ends the room session: everybody is dropped and a new room with a new code is created. */
  endRoom(): void {
    this.client?.send({ type: 'end_room' })
  }

  dismissNotice(): void {
    if (this.state.notice) this.set({ notice: null })
  }

  private iceLoaded(run: number, list: RTCIceServer[]): void {
    if (run !== this.run) return
    this.iceServers = list
    this.iceReady = true
    for (const m of this.held.splice(0)) {
      if (run !== this.run) return
      try {
        this.onMessage(m)
      } catch {
        this.fail('peer_failed')
        return
      }
    }
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
        this.altchaPayload = payload
        this.set({ phase: 'connecting' })
        this.sendCreateRoom()
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
    this.channel = null
    this.peer?.close()
    this.peer = null
    this.client?.close()
    this.client = null
  }

  private fail(code: string): void {
    if (this.state.phase === 'error' || this.state.phase === 'idle') return
    // For 60 s after a refusal a closed connection is the same refusal (the server closes it after a ban).
    const reported = this.busy.apply(code, (this.deps.now ?? Date.now)())
    const iceState = code === 'peer_failed' ? this.deps.iceState?.() : undefined
    this.teardown()
    this.set({ phase: 'error', errorCode: reported, stream: null, sas: undefined, iceState })
  }

  private newPeer(): Peer {
    this.peer?.close()
    this.channel = null
    this.offerId = null
    const handlers: Parameters<PeerFactory>[0] = {
        onIceCandidate: (candidate) => this.client?.send({ type: 'ice_candidate', candidate }),
        onTrack: (stream) => this.set({ stream }),
        onTrackEnded: (stream) => {
          if (this.state.stream === stream) this.set({ stream: null })
        },
        onDataChannel: (ch) => this.attachChannel(ch),
        onState: (s: PeerState) => {
          if (s === 'connected') {
            this.set({ phase: 'receiving' })
            void peer.connectionCode?.().then((c) => {
              if (c && this.peer === peer) this.set({ sas: c })
            })
          }
          else if (s === 'failed') this.fail('peer_failed')
        },
    }
    let peer: Peer
    try {
      peer = this.peerFactory(handlers, { iceServers: this.iceServers })
    } catch {
      // The browser refused the server's ICE list (RTCPeerConnection throws for a URI it cannot
      // parse). A direct connection without STUN still works on one network, so try that.
      peer = this.peerFactory(handlers, { iceServers: [] })
    }
    this.peer = peer
    return peer
  }

  private attachChannel(ch: ControlChannel): void {
    if (ch.label !== CONTROL_CHANNEL_LABEL) return // other labels are not defined; ignore them
    ch.binaryType = 'arraybuffer'
    ch.onmessage = (ev) => this.control.handle(ev.data)
    this.channel = ch
    this.lastPageSent = ''
  }

  /** Tells the presenter which page is shown and how many there are (decision 13.31), whenever either changes. */
  private reportPage(): void {
    const c = this.state.content
    if (c?.kind !== 'document' || c.pages < 1) return
    const key = `${c.id}:${c.page}/${c.pages}`
    if (key === this.lastPageSent) return
    const ch = this.channel
    if (!ch || ch.readyState !== 'open') return
    this.lastPageSent = key
    try {
      ch.send(JSON.stringify({ kind: 'page', page: c.page, pages: c.pages }))
    } catch {
      /* the channel is closing; the presenter learns that from the connection */
    }
  }

  private onMessage(m: ServerMessage): void {
    // A peer must not be made before the ICE server list is known (rarely waits at all).
    if (!this.iceReady && (m.type === 'peer_joined' || m.type === 'offer' || m.type === 'ice_candidate')) {
      this.held.push(m)
      return
    }
    switch (m.type) {
      case 'room_created':
        // A mode that was chosen while the room was being created is applied now.
        this.set({ phase: 'waiting_sender', code: m.code, joinMode: m.join_mode })
        if (this.mode !== m.join_mode) this.client?.send({ type: 'set_join_mode', join_mode: this.mode })
        break
      case 'join_request':
        if (this.state.requests.some((r) => r.id === m.id)) break
        this.set({
          requests: [
            ...this.state.requests,
            { id: m.id, name: m.name, check: m.check, receivedAt: (this.deps.now ?? Date.now)() },
          ],
        })
        break
      case 'join_request_cancelled': {
        // The allowed sender gave up before it arrived: the next one that joins is announced again.
        if (this.allowedPending === m.id) this.allowedPending = null
        const gone = this.state.requests.find((r) => r.id === m.id)
        const requests = this.state.requests.filter((r) => r.id !== m.id)
        if (gone && m.reason === 'timeout') this.emit({ kind: 'expired', name: gone.name })
        else if (gone && (m.reason === 'cancelled' || m.reason === 'left')) this.emit({ kind: 'withdrawn', name: gone.name })
        if (m.reason === 'locked') {
          const prev = this.state.notice
          const count = (prev?.kind === 'locked_discarded' ? prev.count : 0) + 1
          this.set({ requests, notice: { kind: 'locked_discarded', count } })
        } else this.set({ requests })
        break
      }
      case 'join_mode_changed':
        this.mode = m.join_mode
        this.set({
          joinMode: m.join_mode,
          notice: m.join_mode === 'locked' ? this.state.notice : null,
        })
        break
      case 'blocked':
        if (this.state.blocks.every((b) => b.id !== m.id)) this.set({ blocks: [...this.state.blocks, { id: m.id, name: m.name }] })
        break
      case 'unblocked':
        this.set({ blocks: this.state.blocks.filter((b) => b.id !== m.id) })
        break
      case 'room_ended':
        // The room and its code are gone; the connection stays, so ask for a new room at once.
        this.peer?.close()
        this.peer = null
        this.channel = null
        this.clearContent()
        this.control.reset()
        this.altchaPayload = undefined
        this.altchaTried = false
        // A new session starts in the default mode again ("locked" does not survive the end).
        this.mode = this.defaultMode
        this.allowedPending = null
        this.emit({ kind: 'sessionEnded' })
        this.set({ joinMode: this.mode, phase: 'connecting', code: null, stream: null, sas: undefined, peerName: null, requests: [], blocks: [] })
        this.sendCreateRoom()
        break
      case 'peer_joined':
        if (this.allowedPending === null) this.emit({ kind: 'joined', name: m.name })
        this.allowedPending = null
        this.newPeer()
        this.set({ phase: 'negotiating', stream: null, sas: undefined, peerName: m.name })
        break
      case 'offer': {
        // A sender that is still in the room makes a new peer connection for its next presentation
        // ("Stop", then show something else): that offer begins a fresh session, as on a native
        // display (spec 5.6, native-apps.md 21.10 item 8). An offer of the same connection (the web
        // sender adds or removes its screen) is a renegotiation and keeps the session.
        const id = sdpIdentity(m.sdp)
        if (this.peer && this.offerId !== null && id !== null && id !== this.offerId) {
          this.peer.close()
          this.peer = null
          this.channel = null
          this.clearContent()
          this.control.reset()
          this.set({ phase: 'negotiating', stream: null, sas: undefined })
        }
        const peer = this.peer ?? this.newPeer()
        this.offerId = id
        if (this.state.phase !== 'receiving') this.set({ phase: 'negotiating' })
        // A result that arrives after this peer was closed or replaced belongs to nobody: the
        // sender left, or a new connection began. It must not answer for, or fail, the next one.
        // Offers are answered one after another: a second offer while the first is still being
        // answered would find the connection in the wrong state and fail it.
        const sdpIn = m.sdp
        this.answerChain = this.answerChain.then(() =>
          this.peer !== peer
            ? undefined
            : peer
                .acceptOffer(sdpIn)
                .then((sdp) => {
                  if (this.peer === peer) this.client?.send({ type: 'answer', sdp })
                })
                .catch(() => {
                  if (this.peer === peer) this.fail('peer_failed')
                }),
        )
        break
      }
      case 'ice_candidate':
        this.peer?.addIceCandidate(m.candidate).catch(() => {
          // A single bad candidate is not fatal; the connection may still succeed with others.
        })
        break
      case 'peer_left':
        this.allowedPending = null
        if (this.state.peerName !== null) this.emit({ kind: 'left', name: this.state.peerName })
        this.peer?.close()
        this.peer = null
        this.channel = null
        this.clearContent()
        this.control.reset()
        this.set({ phase: 'peer_left', stream: null, sas: undefined, peerName: null })
        break
      case 'error':
        // Open servers answer the first create_room with altcha_required; the connection stays open.
        if (m.code === 'altcha_required' && this.deps.altcha && !this.altchaTried && this.state.phase === 'connecting') {
          this.solveAltcha()
          break
        }
        // The server forbids open rooms: ask for one that needs approval instead of failing.
        if (m.code === 'open_rooms_disabled') {
          this.emit({ kind: 'openDisabled' })
          if (this.state.phase === 'connecting' && this.mode === 'open') {
            this.mode = 'confirm'
            this.set({ joinMode: 'confirm', notice: { kind: 'open_disabled' } })
            this.sendCreateRoom()
          } else {
            this.set({ notice: { kind: 'open_disabled' } })
          }
          break
        }
        // A decision for a request that is already gone (timed out at the same moment).
        if (m.code === 'no_such_request') break
        // not_in_room can arrive when the sender has just left; the room itself survives.
        if (m.code === 'not_in_room' && this.state.phase !== 'connecting') break
        this.fail(m.code)
        break
      default:
        break // control, answer, joined: not used by the receiver in M1
    }
  }
}
