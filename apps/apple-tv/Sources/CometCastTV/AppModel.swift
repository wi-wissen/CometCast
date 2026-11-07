// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The receiver's brain on the main actor: runs the pure state machine of CometCastCore (ReceiverMachine),
// performs its effects (socket, ALTCHA, ICE list, WebRTC peer) and holds what the views show.
// Stale async results are dropped with a generation counter, like the web client's `run`.

import AVFoundation
import CometCastCore
import LiveKitWebRTC
import os
import SwiftUI

@MainActor
final class AppModel: ObservableObject {
    // MARK: published state
    @Published private(set) var state = ReceiverState()
    @Published private(set) var videoTrack: LKRTCVideoTrack?
    @Published private(set) var content: ReceivedContent?
    @Published private(set) var transfer: (name: String, received: Int, size: Int)?
    @Published private(set) var contentError: TransferErrorCode?
    @Published private(set) var image: UIImage?
    @Published private(set) var pageImage: UIImage?
    @Published private(set) var choice = RelayChoice(relays: [], selected: nil, locked: false, policyUnusable: false)
    @Published var pendingConfirmation: Relay?
    @Published var showStopConfirm = false
    @Published private(set) var iceServerCount = 0
    @Published var showStats = false { didSet { showStats ? startStats() : stopStats() } }
    @Published private(set) var statsText: [String] = []
    let discovery = Discovery()

    // MARK: private
    private var machine = ReceiverMachine()
    private var generation = 0
    private var socket: SignalingSocket?
    private var peer: PeerSession?
    private var iceServers: [IceServerConfig] = []
    private var activeRelay: Relay?
    private var altchaCancelled = AtomicFlag()
    private let contentController = ContentController()
    // ControlReceiver keeps its handlers `unowned`: the sink must live as long as the model.
    private lazy var sink = ContentSink(model: self)
    private lazy var control = ControlReceiver(handlers: sink)
    private var pdf: PDFRenderer?
    private var pdfTask: Task<Void, Never>?
    private var stallTimer: Timer?
    private var userTouchedRelay = false
    private var wasRunning = false
    private var policy: PolicyState = .absent
    private var statsTask: Task<Void, Never>?
    /// Diagnostics for the device test plan: phase changes, error codes, statistics. Never the room code, SDP,
    /// ALTCHA payload or addresses of other devices.
    private let log = Logger(subsystem: "de.cometcast.tv", category: "receiver")

    init() {
        refreshRelays()
        discovery.start(browseAnnouncements: !choice.locked)
        // The stall check of the content channel runs once a second (the core has no timers).
        stallTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.control.tick(nowMs: Self.nowMs) }
        }
    }

    static var nowMs: Double { Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000 }

    var isRunning: Bool { state.phase != .idle && state.phase != .error }

    // MARK: relays

    /// Rebuilds the server list from MDM, DNS-SRV, announcements and the default list.
    func refreshRelays() {
        policy = ConfigStore.policyState()
        let merged = mergeServers(policy: policy, srv: discovery.srv, announced: discovery.announced, defaults: ConfigStore.defaultCandidates())
        var c = chooseRelay(merged: merged, lastUsedWsUrl: ConfigStore.lastUsedWsUrl, customConfirmed: ConfigStore.customRelay)
        // Once the person chose something, a late network answer does not replace the selection.
        if userTouchedRelay, let cur = choice.selected, c.relays.contains(where: { $0.wsUrl == cur.wsUrl }) || cur.source == .user { c.selected = cur }
        choice = c
    }

    func select(_ r: Relay) {
        guard !choice.locked || r.source == .mdm else { return }
        userTouchedRelay = true
        var c = choice
        c.selected = r
        choice = c
    }

    /// Checks a typed address. Returns the error code to show, or nil when the relay was selected.
    func useCustomRelay(_ text: String) -> RelayErrorCode? {
        guard !choice.locked else { return .invalid }
        switch parseRelay(text, strictWs: false) {
        case .failure(let e): return e
        case .ok:
            guard let r = parseRelay(text).relay(source: .user) else { return .invalid }
            ConfigStore.customRelay = r
            var c = choice
            if !c.relays.contains(where: { $0.wsUrl == r.wsUrl }) { c.relays.append(r) }
            choice = c
            select(r)
            return nil
        }
    }

    // MARK: start / stop

    func start() {
        guard let r = choice.selected else { setLocalError("no_server"); return }
        if needsRelayConfirmation(r, confirmed: ConfigStore.confirmed) { pendingConfirmation = r; return }
        run(with: r)
    }

    func confirmPending() {
        guard let r = pendingConfirmation else { return }
        pendingConfirmation = nil
        var c = ConfigStore.confirmed
        c.insert(r.wsUrl)
        ConfigStore.confirmed = c
        run(with: r)
    }

    func stop() {
        showStats = false
        wasRunning = false
        generation += 1
        perform(machine.handle(.stop))
        publish()
        UIApplication.shared.isIdleTimerDisabled = false
    }

    private func run(with relay: Relay) {
        generation += 1
        activeRelay = relay
        // A relay announced by a display is never saved as "last used" (it would become the next pre-fill).
        if relay.source != .displayAnnouncement { ConfigStore.lastUsedWsUrl = relay.wsUrl }
        wasRunning = true
        discovery.stop() // no need to browse while a room is open
        altchaCancelled = AtomicFlag()
        perform(machine.handle(.start))
        publish()
    }

    private func setLocalError(_ code: String) {
        // no connection was started: show the error through the same path
        _ = machine.handle(.start)
        _ = machine.handle(.socketClosed)
        state = machine.state
        state.errorCode = code
    }

    func scenePhaseChanged(_ phase: ScenePhase) {
        if phase == .active {
            discovery.start(browseAnnouncements: !choice.locked)
            // tvOS suspends the app in the background and the socket dies: a kiosk comes back with a new room.
            if wasRunning, !isRunning, let r = activeRelay { run(with: r) }
        }
    }

    // MARK: effects

    private func feed(_ input: ReceiverInput, gen: Int) {
        guard gen == generation else { return } // an earlier start/stop: ignore
        perform(machine.handle(input))
        publish()
    }

    private func perform(_ effects: [ReceiverEffect]) {
        let gen = generation
        for e in effects {
            switch e {
            case .openSocket:
                guard let relay = activeRelay, let url = URL(string: relay.wsUrl) else { break }
                let s = SignalingSocket(url: url) { [weak self] ev in
                    Task { @MainActor in
                        switch ev {
                        case .ready: self?.feed(.serverReady, gen: gen)
                        case .message(let m): self?.feed(.message(m), gen: gen)
                        case .closed: self?.feed(.socketClosed, gen: gen)
                        }
                    }
                }
                socket = s
                s.open()
            case .closeSocket:
                socket?.close(); socket = nil
            case .send(let m):
                socket?.send(m)
            case .solveAltcha:
                guard let base = activeRelay?.httpBase else { break }
                let flag = altchaCancelled
                Task {
                    do {
                        let payload = try await ServerHTTP.altchaPayload(base: base, isCancelled: { flag.isSet })
                        feed(.altchaSolved(payload), gen: gen)
                    } catch let e as ServerError {
                        if e.code != "aborted" { feed(.altchaFailed(code: e.code), gen: gen) }
                    } catch { feed(.altchaFailed(code: "altcha_failed"), gen: gen) }
                }
            case .cancelAltcha:
                altchaCancelled.set()
            case .fetchIceServers:
                guard let base = activeRelay?.httpBase else { break }
                Task {
                    let list = await ServerHTTP.iceServers(base: base)
                    if gen == generation { iceServers = list; iceServerCount = list.count }
                    feed(.iceLoaded, gen: gen)
                }
            case .createPeer:
                makePeer(gen: gen)
            case .closePeer:
                peer?.close(); peer = nil
                videoTrack = nil
            case .acceptOffer(let sdp):
                guard let p = peer else { break }
                Task {
                    do {
                        let answer = try await p.accept(offer: sdp)
                        feed(.answerReady(sdp: answer), gen: gen)
                    } catch { feed(.answerFailed, gen: gen) }
                }
            case .addIceCandidate(let c):
                peer?.add(candidateJson: c)
            case .resetContent:
                control.reset()
                clearContent()
            }
        }
    }

    private func makePeer(gen: Int) {
        peer = PeerSession(iceServers: iceServers, handlers: .init(
            onIceCandidate: { [weak self] c in self?.socket?.send(.iceCandidate(candidate: c)) },
            onVideoTrack: { [weak self] t in self?.videoTrack = t; self?.feed(.videoArrived, gen: gen) },
            onControlText: { [weak self] t in self?.control.handleText(t, nowMs: Self.nowMs) },
            onControlBinary: { [weak self] b in self?.control.handleBinary(b, nowMs: Self.nowMs) },
            onConnected: { [weak self] in
                guard let self else { return }
                self.feed(.peerConnected, gen: gen)
                if let p = self.peer, let l = p.localSdp, let r = p.remoteSdp, let sas = connectionCode(localSdp: l, remoteSdp: r) {
                    self.feed(.sasReady(sas), gen: gen)
                }
            },
            onFailed: { [weak self] in self?.feed(.peerFailed, gen: gen) }
        ))
        if peer == nil { feed(.peerFailed, gen: gen) }
    }

    private func publish() {
        if state.phase != machine.state.phase || state.errorCode != machine.state.errorCode {
            log.info("phase \(self.machine.state.phase.rawValue, privacy: .public) error \(self.machine.state.errorCode ?? "-", privacy: .public) ice \(self.iceServerCount)")
        }
        state = machine.state
        UIApplication.shared.isIdleTimerDisabled = isRunning
        if !isRunning { discovery.start(browseAnnouncements: !choice.locked) }
    }

    // MARK: statistics (toggled with Play/Pause on the receive screen)

    private func startStats() {
        statsTask?.cancel()
        statsTask = Task { [weak self] in
            var lastBytes: Double?
            var lastTime = Date()
            while !Task.isCancelled {
                guard let self else { return }
                let list = await self.peer?.statistics() ?? []
                let s = summarizeStats(list)
                let now = Date()
                let kbps = lastBytes.flatMap { bitrateKbps(previousBytes: $0, currentBytes: s.bytesReceived, seconds: now.timeIntervalSince(lastTime)) }
                lastBytes = s.bytesReceived; lastTime = now
                self.statsText = Self.lines(s, kbps: kbps)
                self.log.info("stats \(self.statsText.joined(separator: " | "), privacy: .public)")
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
        }
    }

    private func stopStats() { statsTask?.cancel(); statsTask = nil; statsText = [] }

    private static func lines(_ s: StatsSummary, kbps: Double?) -> [String] {
        func f(_ d: Double?) -> String { d.map { String(format: "%.1f", $0) } ?? "-" }
        return [
            L.f("stats.video", s.codec ?? "-", s.width, s.height, f(s.framesPerSecond)),
            L.f("stats.decoder", s.decoder ?? "-"),
            L.f("stats.frames", s.framesDecoded, s.framesDropped, s.keyFramesDecoded),
            L.f("stats.rate", f(kbps), f(s.jitterMs), s.packetsLost),
            L.f("stats.route", L.s("stats.route." + s.route.rawValue), s.transport ?? "-"),
            L.f("stats.rtt", f(s.roundTripMs)),
        ]
    }

    // MARK: content

    func clearContent() {
        pdfTask?.cancel(); pdfTask = nil
        pdf = nil
        contentController.clear()
        image = nil; pageImage = nil
        syncContent()
    }

    func dismissContent() { clearContent() }

    func turnPage(_ cmd: ControlCmd) {
        contentController.onCmd(cmd)
        syncContent()
        renderCurrentPage()
    }

    private func syncContent() {
        content = contentController.content
        transfer = contentController.transfer.map { ($0.name, $0.received, $0.size) }
        contentError = contentController.contentError
    }

    fileprivate func contentEvent(_ f: (ContentController) -> Void) {
        f(contentController)
        syncContent()
    }

    fileprivate func receivedImage(bytes: [UInt8], mime: String, name: String) {
        let id = contentController.showImage(name: name, mime: mime)
        syncContent()
        Task {
            let result = await Task.detached(priority: .userInitiated) { ImageDecoder.decode(bytes) }.value
            guard case .image(let cur, _, _)? = contentController.content, cur == id else { return }
            switch result {
            case .success(let img): image = img
            case .failure(let code): contentController.clear(); contentController.setError(code); syncContent()
            }
        }
    }

    fileprivate func receivedDocument(bytes: [UInt8], name: String) {
        let id = contentController.openDocument(name: name)
        syncContent()
        pdfTask?.cancel()
        pdfTask = Task {
            do {
                let r = try await Task.detached(priority: .userInitiated) { try PDFRenderer(bytes: bytes) }.value
                guard case .document(let cur, _, _, _)? = contentController.content, cur == id else { return }
                pdf = r
                contentController.documentOpened(id: id, pages: r.pageCount)
                syncContent()
                renderCurrentPage()
            } catch let PDFFailure.code(c) {
                contentController.documentFailed(id: id, code: c); syncContent()
            } catch { contentController.documentFailed(id: id, code: "failed"); syncContent() }
        }
    }

    private func renderCurrentPage() {
        guard case .document(let id, _, let pages, let page)? = contentController.content, pages > 0, let r = pdf else { return }
        pdfTask?.cancel()
        let width = max(1280, Int(UIScreen.main.nativeBounds.width))
        pdfTask = Task {
            do {
                let img = try await r.render(page: page, width: width)
                guard case .document(let cur, _, _, let p)? = contentController.content, cur == id, p == page else { return }
                pageImage = img
            } catch let PDFFailure.code(c) {
                if !Task.isCancelled { contentController.documentFailed(id: id, code: c); pdf = nil; syncContent() }
            } catch {}
        }
    }
}

/// Receives the parsed content messages. Called by `ControlReceiver` from the main actor (the callers in AppModel
/// run there), hence `assumeIsolated`.
private final class ContentSink: ControlReceiverHandlers {
    unowned let model: AppModel
    init(model: AppModel) { self.model = model }
    func onCmd(_ cmd: ControlCmd) { MainActor.assumeIsolated { model.turnPage(cmd) } }
    func onProgress(_ p: (name: String, received: Int, size: Int)?) { MainActor.assumeIsolated { model.contentEvent { $0.setProgress(p) } } }
    func onImage(bytes: [UInt8], mime: String, name: String) { MainActor.assumeIsolated { model.receivedImage(bytes: bytes, mime: mime, name: name) } }
    func onDocument(bytes: [UInt8], name: String) { MainActor.assumeIsolated { model.receivedDocument(bytes: bytes, name: name) } }
    func onLink(url: String) { MainActor.assumeIsolated { model.contentEvent { $0.showLink(url: url) } } }
    func onError(_ code: TransferErrorCode) { MainActor.assumeIsolated { model.contentEvent { $0.setError(code) } } }
}

/// A flag that can be set from any thread (ALTCHA cancellation).
final class AtomicFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    var isSet: Bool { lock.lock(); defer { lock.unlock() }; return value }
    func set() { lock.lock(); value = true; lock.unlock() }
}
