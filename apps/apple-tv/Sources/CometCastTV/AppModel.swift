// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The display's brain on the main actor: runs the pure state machine of CometCastCore (ReceiverMachine), performs its
// effects (socket, ALTCHA, ICE list, WebRTC peer) and holds what the views show. The rules of the screens (wizard,
// key tables, reconnect schedule, controls, dimming, toasts) are CometCastCore's and tested on Linux; this class only
// binds them to the Apple APIs and the views. Stale async results are dropped with a generation counter, like the web
// client's `run`.

import AVFoundation
import Combine
import CometCastCore
import LiveKitWebRTC
import os
import SwiftUI

@MainActor
final class AppModel: ObservableObject {
    // MARK: published state: the receiver
    @Published private(set) var state = ReceiverState()
    @Published private(set) var videoTrack: LKRTCVideoTrack?
    @Published private(set) var content: ReceivedContent?
    @Published private(set) var transfer: (name: String, received: Int, size: Int)?
    @Published private(set) var contentError: TransferErrorCode?
    @Published private(set) var image: UIImage?
    @Published private(set) var pageImage: UIImage?
    /// A page that cannot be drawn (the document stays; spec 4.5, decision N41: always the `failed` text).
    @Published private(set) var pageError: String?
    @Published private(set) var choice = RelayChoice(relays: [], selected: nil, locked: false, policyUnusable: false)

    // MARK: published state: the screens
    /// What was set up; nil until the wizard finished.
    @Published private(set) var config: DisplayConfig?
    /// The wizard, while it runs.
    @Published private(set) var wizard: WizardState?
    @Published private(set) var settingsOpen = false
    @Published private(set) var aboutOpen = false
    @Published private(set) var licencesOpen = false
    /// Returning from About puts the focus on the About button (spec 15.9).
    @Published private(set) var settingsFocusAbout = false
    @Published private(set) var toast = ToastState()
    @Published private(set) var dimmed = false
    @Published private(set) var controlsShown = false
    @Published private(set) var presenting = false
    /// Idle, connecting / online / offline (spec 4.2).
    @Published private(set) var view: ConnectionView = .connecting
    @Published private(set) var cause: OfflineCause = .network
    /// The IPv4 address of an in-house box (decision N40).
    @Published private(set) var boxIPv4: String?
    let discovery = Discovery()
    let licences = LicenceStore()
    private(set) var activeRelay: Relay?

    // MARK: private
    private var machine = ReceiverMachine()
    private var generation = 0
    private var socket: SignalingSocket?
    private var peer: PeerSession?
    private var iceServers: [IceServerConfig] = []
    private var altchaCancelled = AtomicFlag()
    private let contentController = ContentController()
    // ControlReceiver keeps its handlers `unowned`: the sink must live as long as the model.
    private lazy var sink = ContentSink(model: self)
    private lazy var control = ControlReceiver(handlers: sink)
    private var pdf: PDFRenderer?
    private var pdfTask: Task<Void, Never>?
    private var tickTimer: Timer?
    private var policy: PolicyState = .absent
    /// Diagnostics for the device test plan: phase changes, error codes. Never the room code, SDP, ALTCHA payload or
    /// addresses of other devices.
    private let log = Logger(subsystem: "de.cometcast.tv", category: "receiver")
    private let announcer = Announcer()
    private var sceneActive = true
    // timers and small state machines of CometCastCore
    private var tracker = ConnectionTracker(now: AppModel.nowS)
    private var retryTask: Task<Void, Never>?
    private var controls = ControlsReveal()
    private var dimmer = IdleDimmer()
    private var inputGuard = InputGuard()
    private var lastHeadId: Int?
    private var lastActivitySignature = ""
    private var lastLinkUrl: String?
    private var expectJoinAfterAllow = false
    private var lastPageSent: (Int, Int)?
    private var bag: [Any] = []

    init() {
        refreshRelays()
        if let stored = ConfigStore.displayConfig, stored.relay != nil {
            // set up before: no wizard, the room opens at once (spec 4.1: "Finish stores ... then shows Idle")
            config = stored
            machine = ReceiverMachine(joinMode: stored.mode, roomName: stored.room)
            activeRelay = effectiveRelay(for: stored)
            resolveBoxAddress()
            start()
        } else {
            wizard = WizardState(firstSuggestion: L.s(nameSuggestionKeys[0]), choices: wizardChoices(), policyLocked: choice.locked)
            discovery.start(browseAnnouncements: !choice.locked)
        }
        // The late answers of the network (DNS-SRV after 3 s, announcements) refine the wizard's address list.
        bag.append(discovery.$srv.combineLatest(discovery.$announced).sink { [weak self] _, _ in
            Task { @MainActor in self?.discoveryChanged() }
        })
        tickTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
        refreshDerived()
    }

    static var nowMs: Double { Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000 }
    static var nowS: Double { Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000_000 }

    var isRunning: Bool { state.phase != .idle && state.phase != .error }

    // MARK: relays and setup

    /// Rebuilds the server list from MDM, DNS-SRV, announcements and the default list.
    func refreshRelays() {
        policy = ConfigStore.policyState()
        let merged = mergeServers(policy: policy, srv: discovery.srv, announced: discovery.announced, defaults: ConfigStore.defaultCandidates())
        choice = chooseRelay(merged: merged, lastUsedWsUrl: nil, customConfirmed: nil)
    }

    /// The addresses the wizard offers, in the order of the merge (policy, DNS, box announcement, default); the first
    /// is pre-selected and the person confirms it with "Looks right" (spec 4.1).
    private func wizardChoices() -> [AddressChoice] { choice.relays.map { AddressChoice(relay: $0) } }

    private func discoveryChanged() {
        guard wizard != nil else { return }
        refreshRelays()
        wizard?.setChoices(wizardChoices())
    }

    /// Under an administrator's lock only the managed servers are used, whatever was stored.
    private func effectiveRelay(for c: DisplayConfig) -> Relay? {
        if choice.locked { return choice.selected ?? c.relay }
        return c.relay
    }

    private func resolveBoxAddress() {
        boxIPv4 = nil
        guard let r = activeRelay, r.source == .displayAnnouncement else { return }
        let host = r.host.split(separator: ":").first.map(String.init) ?? r.host
        guard !isIPLiteral(host) else { return }
        Task { [weak self] in
            let ip = await HostResolver.ipv4(host)
            await MainActor.run { if self?.activeRelay?.host == r.host { self?.boxIPv4 = ip } }
        }
    }

    func wizardAct(_ a: WizardAction) {
        guard var w = wizard else { return }
        w.reduce(a)
        wizard = w
    }

    /// Step 4 "Go to idle screen": stores room name, default join mode and server, then Idle with the chosen mode.
    func finishSetup() {
        guard let w = wizard, let r = w.result else { return }
        let cfg = DisplayConfig(room: r.room, mode: r.mode, wsUrl: r.relay.wsUrl, source: r.relay.source)
        ConfigStore.displayConfig = cfg
        // "Looks right" is the confirmation of this server (an announced or typed one needs it, spec 4.1)
        var c = ConfigStore.confirmed
        c.insert(r.relay.wsUrl)
        ConfigStore.confirmed = c
        config = cfg
        wizard = nil
        discovery.stop()
        machine = ReceiverMachine(joinMode: cfg.mode, roomName: cfg.room)
        activeRelay = r.relay
        resolveBoxAddress()
        tracker = ConnectionTracker(now: Self.nowS)
        start()
    }

    // MARK: start / stop

    func start() {
        guard let r = activeRelay else { setLocalError("no_server"); publish(); return }
        run(with: r)
    }

    /// The session ends like End without a toast (background rule of native-apps.md 15); a new one with a new code
    /// starts when the app is active again.
    private func quietStop() {
        generation += 1
        retryTask?.cancel(); retryTask = nil
        perform(machine.handle(.stop))
        _ = machine.takeEvents()
        publish()
    }

    private func run(with relay: Relay) {
        generation += 1
        activeRelay = relay
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
        sceneActive = phase == .active
        syncAnnouncement()
        if phase == .background {
            quietStop()
        } else if phase == .active, config != nil, state.phase == .idle || state.phase == .error {
            // tvOS suspends the app in the background: a kiosk comes back with a new room and a new code.
            tracker = ConnectionTracker(now: Self.nowS)
            start()
        }
    }

    // MARK: join approval, Settings, End (the screens bind to these)

    var headRequest: JoinRequest? { state.requests.first }
    /// A dialog is on screen (the screen below is inert and its focus rows are off).
    var dialogOpen: Bool { headRequest != nil || settingsOpen || aboutOpen || licencesOpen || linkOpen }
    var linkOpen: Bool { if case .link? = content { return true } else { return false } }

    /// Wraps the action of a dialog button: ignored for 1 s after the dialog appeared or switched (spec 4.4).
    func guarded(_ action: () -> Void) {
        guard inputGuard.isArmed(now: Self.nowS) else { return }
        action()
    }

    func allowCurrent() {
        guard let r = headRequest else { return }
        expectJoinAfterAllow = true
        showToast(L.t("toast.allowed", ["name": L.quoted(r.name, unnamed: r.unnamed)]))
        hostAction(.decide(id: r.id, allow: true, block: false))
    }

    func denyCurrent(block: Bool) {
        guard let r = headRequest else { return }
        let name = L.quoted(r.name, unnamed: r.unnamed)
        showToast(L.t(block ? "toast.blocked" : "toast.denied", ["name": name]))
        hostAction(.decide(id: r.id, allow: false, block: block))
    }

    /// Settings: a tile of the join mode. Open and Approval on become the new default (replacing the one from setup);
    /// Locked is never stored (spec 4.3).
    func chooseMode(_ mode: JoinMode) {
        guard mode != state.joinMode else { return }
        if let storable = storableDefaultMode(mode), var c = config {
            c.mode = storable
            config = c
            ConfigStore.displayConfig = c
        }
        noteActivity()
        hostAction(.setJoinMode(mode))
    }

    func chooseAppearance(_ a: AppearanceChoice) {
        guard var c = config, c.appearance != a else { return }
        c.appearance = a
        config = c
        ConfigStore.displayConfig = c
        noteActivity()
    }

    func openSettings() {
        guard wizard == nil, !presenting, headRequest == nil else { return }
        settingsOpen = true
        settingsFocusAbout = false
        noteActivity()
    }
    func closeSettings() { settingsOpen = false; aboutOpen = false; licencesOpen = false; noteActivity() }
    func openAbout() { aboutOpen = true; noteActivity() }
    func closeAbout() { aboutOpen = false; settingsFocusAbout = true; noteActivity() }
    func openLicences() { licencesOpen = true; noteActivity() }
    func closeLicences() { licencesOpen = false; noteActivity() }

    /// Ends the room session: all presenters are disconnected, a new code is made, toast `toast.sessionEnded`
    /// (spec 4.5: no confirmation step).
    func endRoom() {
        controls.hideNow()
        controlsShown = false
        hostAction(.endRoom)
    }

    private func hostAction(_ input: ReceiverInput) {
        perform(machine.handle(input))
        publish()
    }

    // MARK: Siri Remote (spec 7.1, native-apps.md 5 and 6.1; the tables are CometCastCore's)

    var context: DisplayContext {
        if let w = wizard { return .setup(step: w.step) }
        if headRequest != nil { return .joinRequest }
        if linkOpen { return .linkCard }
        if licencesOpen { return .licences }
        if aboutOpen { return .about }
        if settingsOpen { return .settings }
        if presenting { return .presenting(controlsShown: controlsShown) }
        return .idle
    }

    /// Menu / Back is ours unless the system should handle it (Idle, wizard step 0: Menu leaves the app).
    var backHandled: Bool { dimmed || intentForBack(context) != .system }

    func handleBack() {
        if dimmed { wake(); return }
        noteActivity()
        perform(intent: intentForBack(context))
    }

    /// Play/Pause stands for the spec's Menu key.
    func handleMenu() {
        if dimmed { wake(); return }
        noteActivity()
        perform(intent: intentForMenu(context))
    }

    private func perform(intent: BackIntent) {
        switch intent {
        case .consume, .system: break
        case .openSettings: openSettings()
        case .closeSettings: closeSettings()
        case .closeAbout: closeAbout()
        case .closeLicences: closeLicences()
        case .dismissLink: dismissContent()
        case .wizardBack: wizardAct(.back)
        case .showControls: showControls()
        case .hideControls: controls.hideNow(); controlsShown = false
        }
    }

    private func showControls() {
        controls.poke(now: Self.nowS)
        controlsShown = controls.shown
    }

    /// A key while presenting (the catcher or End): the first input only reveals; Select activates End when shown.
    func presentingKey(_ key: RemoteKey) {
        if dimmed { wake(); return }
        let r = presentingResponse(key: key, controlsShown: controlsShown)
        if r.reveal { showControls() }
        if r.hide { controls.hideNow(); controlsShown = false }
        if r.activateEnd { endRoom() }
    }

    /// Any input while the idle screen is dimmed only restores the brightness.
    func wake() { _ = dimmer.activity(now: Self.nowS); dimmed = dimmer.dimmed }

    private func noteActivity() { _ = dimmer.activity(now: Self.nowS); dimmed = dimmer.dimmed }

    // MARK: toasts

    private func showToast(_ text: String) {
        toast.show(text, now: Self.nowS)
        AccessibilityNotification.Announcement(text).post()
    }

    private func handle(events: [HostEvent]) {
        for e in events {
            switch e {
            case .requestExpired(let n, let u): showToast(L.t("toast.expired", ["name": L.quoted(n, unnamed: u)]))
            case .requestWithdrawn(let n, let u): showToast(L.t("toast.withdrawn", ["name": L.quoted(n, unnamed: u)]))
            case .senderJoined(let n, let u):
                // After "Yes" the toast `toast.allowed` has said it already.
                if expectJoinAfterAllow { expectJoinAfterAllow = false } else { showToast(L.t("toast.joined", ["name": L.quoted(n, unnamed: u)])) }
            case .senderLeft(let n, let u): showToast(L.t("toast.left", ["name": L.quoted(n, unnamed: u)]))
            case .sessionEnded: showToast(L.s("toast.sessionEnded"))
            }
        }
        // The server only allows Approval on (spec 4.2): the pill changed, said once. Other notices have no copy.
        if let n = state.notice {
            if n == .openDisabled { showToast(L.s("toast.openDisabled")) }
            hostAction(.dismissNotice)
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
                    if gen == generation { iceServers = list }
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
            log.info("phase \(self.machine.state.phase.rawValue, privacy: .public) error \(self.machine.state.errorCode ?? "-", privacy: .public)")
        }
        state = machine.state
        let events = machine.takeEvents()
        let now = Self.nowS

        // the reconnect schedule (spec 4.2): 1 s, 2 s, 3 s, then every 4 s; busy 30 s; outdated never
        switch tracker.update(code: state.code, phase: state.phase, errorCode: state.errorCode, now: now) {
        case .after(let d): scheduleRetry(after: d)
        case .cancel, .never: retryTask?.cancel(); retryTask = nil
        case .unchanged: break
        }

        // one dialog at a time: a request closes Settings (changes already made stay); a new dialog arms the 1 s guard
        let headId = headRequest?.id
        if headId != lastHeadId {
            if headId != nil { settingsOpen = false; aboutOpen = false; licencesOpen = false; inputGuard.arm(now: now) }
            lastHeadId = headId
        }

        // joins, requests, connection and mode changes count as activity (spec 8.6)
        let signature = "\(state.requests.count)|\(state.peerName ?? "")|\(state.joinMode.rawValue)|\(state.phase.rawValue)|\(state.code ?? "")"
        if signature != lastActivitySignature { lastActivitySignature = signature; noteActivity() }

        handle(events: events)
        refreshDerived()
        syncAnnouncement()
    }

    private func scheduleRetry(after seconds: Double) {
        retryTask?.cancel()
        retryTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(max(0, seconds) * 1_000_000_000))
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard let self, self.state.phase == .error, self.sceneActive else { return }
                self.start()
            }
        }
    }

    /// Everything derived from the receiver state: Presenting, the wake lock (only while presenting), the connection
    /// view, the dimming switch, the page message.
    private func refreshDerived() {
        let now = Self.nowS
        let isPresenting = wizard == nil && (content != nil || transfer != nil || isPresentingVideo)
        if isPresenting != presenting {
            presenting = isPresenting
            if isPresenting { settingsOpen = false; aboutOpen = false; licencesOpen = false } else { controls.hideNow(); controlsShown = false }
        }
        // spec 8.7 / D5: the screen is kept awake only while presenting, never on Idle
        UIApplication.shared.isIdleTimerDisabled = isPresenting
        dimmer.setEnabled(wizard == nil && !isPresenting, now: now)
        dimmed = dimmer.dimmed
        view = tracker.view(code: state.code, phase: state.phase, now: now)
        cause = tracker.cause
        // a link card is a dialog: it arms the 1 s guard when it appears
        var url: String?
        if case .link(let u, _)? = content { url = u }
        if url != lastLinkUrl { lastLinkUrl = url; if url != nil { inputGuard.arm(now: now) } }
        sendPageMessageIfNeeded()
        syncStats()
    }

    /// Once a second tick of everything that has a time: toast, controls, dimming, the connection view, the stall check
    /// of the content channel.
    private func tick() {
        let now = Self.nowS
        if toast.text != nil, now >= toast.hideAt { toast.tick(now: now) } // republish only when the text goes
        if controls.tick(now: now) { controlsShown = false }
        if dimmer.tick(now: now) { dimmed = true }
        let v = tracker.view(code: state.code, phase: state.phase, now: now)
        if v != view { view = v }
        control.tick(nowMs: Self.nowMs)
    }

    /// Spec 5.6, decision 58: the display tells the presenter the shown page and the page count whenever they change.
    private func sendPageMessageIfNeeded() {
        guard case .document(_, _, let pages, let page)? = content, let msg = pageMessageJSON(page: page, pages: pages) else {
            lastPageSent = nil
            return
        }
        if let l = lastPageSent, l == (page, pages) { return }
        lastPageSent = (page, pages)
        peer?.sendControl(text: msg)
    }

    /// Spec 14.8: announce only while the display shows Idle or Presenting with a code, in the foreground, and the
    /// policy allows it; say goodbye in every other state (setup, no code yet, error, background, `stop`). The room
    /// name of the wizard is the announced `name`. The record is signed with the key of the display; the goodbye is
    /// a signed `state=gone` record first (the Announcer).
    private func syncAnnouncement() {
        let showing: Bool
        switch state.phase {
        case .waitingSender, .negotiating, .receiving, .peerLeft: showing = wizard == nil
        default: showing = false
        }
        let key = ConfigStore.displayKey
        let (mode, code, room) = (state.joinMode, state.code, config?.room)
        guard sceneActive, showing, announcementsAllowed(policy: policy), let relay = activeRelay,
              displayAnnouncementTxt(key: key, seq: 1, wsUrl: relay.wsUrl, mode: mode, code: code, roomName: room) != nil
        else { announcer.withdraw(); return }
        let wsUrl = relay.wsUrl
        // Every publication (also the refresh every 10 s) is signed anew with a higher `seq`.
        announcer.publish(instance: announcementInstanceName(id: ConfigStore.displayId)) { seq, announceState in
            displayAnnouncementTxt(key: key, seq: seq, state: announceState, wsUrl: wsUrl, mode: mode, code: code, roomName: room)
        }
    }

    // MARK: statistics (debug builds only, log lines, no screen)

    #if DEBUG
    private var statsTask: Task<Void, Never>?

    /// While a picture is received, the statistics are written to the log every 2 s for the device test plan: codec,
    /// size, frame rate, decoder, frames, rate, jitter, route, round trip. Never shown on the display (docs/design/
    /// native-apps.md 18) and not in release builds.
    private func syncStats() {
        let want = isPresentingVideo
        if want, statsTask == nil {
            statsTask = Task { [weak self] in
                var lastBytes: Double?
                var lastTime = Date()
                while !Task.isCancelled {
                    guard let self else { return }
                    let s = summarizeStats(await self.peer?.statistics() ?? [])
                    let now = Date()
                    let kbps = lastBytes.flatMap { bitrateKbps(previousBytes: $0, currentBytes: s.bytesReceived, seconds: now.timeIntervalSince(lastTime)) }
                    lastBytes = s.bytesReceived; lastTime = now
                    func f(_ d: Double?) -> String { d.map { String(format: "%.1f", $0) } ?? "-" }
                    self.log.info("stats video \(s.codec ?? "-", privacy: .public) \(s.width)x\(s.height) \(f(s.framesPerSecond)) fps | decoder \(s.decoder ?? "-", privacy: .public) | frames \(s.framesDecoded) decoded \(s.framesDropped) dropped \(s.keyFramesDecoded) key | rate \(f(kbps)) kbit/s jitter \(f(s.jitterMs)) ms lost \(s.packetsLost) | route \(s.route.rawValue, privacy: .public) \(s.transport ?? "-", privacy: .public) | rtt \(f(s.roundTripMs)) ms")
                    try? await Task.sleep(nanoseconds: 2_000_000_000)
                }
            }
        } else if !want {
            statsTask?.cancel(); statsTask = nil
        }
    }
    #else
    private func syncStats() {}
    #endif

    private var isPresentingVideo: Bool { state.hasVideo && videoTrack != nil }

    // MARK: content

    func clearContent() {
        pdfTask?.cancel(); pdfTask = nil
        pdf = nil
        contentController.clear()
        image = nil; pageImage = nil; pageError = nil
        syncContent()
    }

    func dismissContent() { clearContent() }

    func turnPage(_ cmd: ControlCmd) {
        contentController.onCmd(cmd)
        pageError = nil
        syncContent()
        renderCurrentPage()
    }

    private func syncContent() {
        content = contentController.content
        transfer = contentController.transfer.map { ($0.name, $0.received, $0.size) }
        // a failed transfer or a PDF that cannot be shown is a toast with the existing wording (spec 4.5)
        if let e = contentController.contentError, e != contentError { showToast(L.s("receive.transferError.\(e.rawValue)")) }
        contentError = contentController.contentError
        refreshDerived()
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
                pageError = nil
            } catch let PDFFailure.code(c) {
                guard !Task.isCancelled else { return }
                if c == "rejected" {
                    // one page that cannot be drawn: its text in a name-tag-look box, the document stays (decision N41)
                    pageError = "failed"
                    refreshDerived()
                } else { contentController.documentFailed(id: id, code: c); pdf = nil; syncContent() }
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
