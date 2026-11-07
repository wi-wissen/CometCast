// SPDX-License-Identifier: GPL-3.0-or-later
// ReplayKit Broadcast Upload Extension: receives the screen as CMSampleBuffers, encodes H.264
// with VideoToolbox and sends it to the receiver through the Rust sender (libcc_ios.a). The
// extension makes the whole connection itself (docs/ios.md, "Architecture"): the app can be
// suspended or gone while the broadcast runs.
//
// Rules of this file (the extension is killed by iOS at roughly 50 MB, no warning):
//  * never queue frames without a bound: a frame is dropped when the encoder is busy, the pool
//    is empty, the frame rate limit says so, or the connection is not up;
//  * audio buffers are ignored at once;
//  * nothing is retained between frames except the encoder, the scaler pool (4 buffers) and the
//    last encoded picture (for key frame requests while the screen does not change);
//  * every failure ends the broadcast with `finishBroadcastWithError` and a translated message.
//
// UNCERTAIN: never compiled, never run on a device. Whether the budget holds is spike 4 of
// docs/measurement-points.md; docs/ios-device-test-plan.md says how to measure it.

import CoreImage
import CoreMedia
import CoreVideo
import Darwin
import os  // os_proc_available_memory (UNCERTAIN: which module exports it)
import ReplayKit

class SampleHandler: RPBroadcastSampleHandler, H264EncoderDelegate {
    // MARK: state

    /// All encoder work happens on this queue (one frame at a time, in order).
    private let encodeQueue = DispatchQueue(label: "de.cometcast.broadcast.encode", qos: .userInitiated)
    private let encoder = H264Encoder()
    private var scaler: FrameScaler?
    private var config: BroadcastConfig?
    private var store: BroadcastStore?
    private var stopObservation: DarwinObservation?
    private var timer: DispatchSourceTimer?
    private var memoryPressure: DispatchSourceMemoryPressure?

    private let stateLock = NSLock()
    private var connected = false          // the Rust status says "streaming"
    private var finished = false
    private var lastStatus = BroadcastStatus(
        seq: 0, state: .starting, error: .none, sent: 0, dropped: 0, bytes: 0, keyframes: 0,
        width: 0, height: 0, freeMemoryMB: -1, updatedAt: 0)
    private var seq = 0
    private var lastWrite = Date.distantPast

    // Touched only on `encodeQueue`.
    private var lastPresented: Double = -1          // seconds of the last frame that was encoded
    private var held: CVPixelBuffer?                // last picture we rendered (our own memory)
    private var heldPTS = CMTime.zero
    private var droppedLocal = 0
    private var degraded = false                    // memory warning: halve the frame rate

    // MARK: ReplayKit

    override func broadcastStarted(withSetupInfo setupInfo: [String: NSObject]?) {
        guard let store = BroadcastStore.shared() else {
            return fail(.internalError, "App Group container not available (entitlement or signing)")
        }
        self.store = store
        store.clearStopRequest()
        guard let cfg = store.readConfig() else { return fail(.noConfig, "no config.json") }
        // The file is consumed: a later broadcast started from Control Center must not reuse it.
        store.deleteConfig()
        guard cfg.isFresh() else { return fail(.configExpired, "config too old") }
        let c = cfg.clamped
        config = c
        scaler = FrameScaler(maxDimension: c.maxDimension)
        encoder.delegate = self

        stopObservation = DarwinNotifier.observe(BroadcastIdentity.stopNotification) { [weak self] in
            guard let self, self.store?.stopRequested() == true else { return }
            // A clean end, not a failure: the way back to the app is prepared first (20.5, step 5).
            self.finishBroadcast(error: .stoppedByApp, wayBack: true)
        }
        startTimerAndMemoryWatch()

        publish(.connecting)
        let relay = c.relay
        let dir = store.logDirectory.path
        // The name goes to the display's request; the token, when the app got one, lets this
        // join in without a dialog (docs/design/native-apps.md 20.5). Both may be empty.
        let personName = c.name ?? ""
        let token = (c.handover.flatMap { HandoverToken.isValid($0) ? $0 : nil }) ?? ""
        let result = c.code.withCString { code in
            relay.withCString { relay in
                dir.withCString { dir in
                    personName.withCString { personName in
                        token.withCString { token in
                            cc_ios_start(code, relay, dir, personName, token, statusCallback,
                                         Unmanaged.passUnretained(self).toOpaque())
                        }
                    }
                }
            }
        }
        if result != 0 {
            fail(result == CC_IOS_ERR_INVALID_ROOM_CODE ? .invalidRoomCode
                 : result == CC_IOS_ERR_INVALID_RELAY ? .invalidRelay : .internalError,
                 "cc_ios_start returned \(result)")
        }
    }

    override func broadcastPaused() { publish(.paused) }

    override func broadcastResumed() {
        encoder.requestKeyFrame()
        publish(connectedNow ? .streaming : .waiting)
    }

    override func broadcastFinished() {
        // The person ended the broadcast (red status pill, Control Center): ask the display for
        // the way back before the connection goes (20.5, step 5). Not when it already ended or
        // failed.
        teardown(wayBack: !isFinished && connectedNow)
        publish(.ended, error: lastStatus.error)
    }

    override func processSampleBuffer(_ sampleBuffer: CMSampleBuffer, with type: RPSampleBufferType) {
        guard type == .video else { return }  // audio is ignored on purpose
        // Backpressure before any work: nothing to do while the connection is not up, and the
        // encoder queue must not grow. The sample buffer is only valid during this call, so
        // work is either done now or the frame is dropped.
        guard connectedNow, let source = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        let orientation = FrameScaler.orientation(of: sampleBuffer)
        // The CVPixelBuffer is retained by `source` for the duration of the closure only if we
        // render synchronously: do the (GPU) render on this thread, encode on the queue.
        encodeQueue.sync { self.handle(source, pts: pts, orientation: orientation) }
    }

    // MARK: encoding

    private func handle(_ source: CVPixelBuffer, pts: CMTime, orientation: CGImagePropertyOrientation) {
        guard let config, let scaler, !isFinished else { return }
        let seconds = CMTimeGetSeconds(pts)
        let fps = Double(degraded ? max(config.fps / 2, 5) : config.fps)
        // Frame rate limit: ReplayKit can deliver 60 per second.
        if lastPresented >= 0, seconds - lastPresented < (1.0 / fps) * 0.9 { return }
        guard let (picture, sizeChanged) = scaler.render(source, orientation: orientation) else {
            droppedLocal += 1  // pool exhausted: the encoder has not returned buffers yet
            return
        }
        if sizeChanged || encoder.settings == nil {
            let size = scaler.outputSize
            let settings = H264Encoder.Settings(
                width: Int(size.width), height: Int(size.height), fps: config.fps, bitrate: config.bitrate)
            guard encoder.configure(settings) else { return }  // delegate reported the failure
            encoder.requestKeyFrame()
        }
        if cc_ios_keyframe_requested() { encoder.requestKeyFrame() }
        switch encoder.encode(picture, pts: pts) {
        case .submitted:
            lastPresented = seconds
            held = picture
            heldPTS = pts
        case .busy:
            droppedLocal += 1
        case .failed:
            fail(.encoderFailed, "VTCompressionSessionEncodeFrame failed")
        }
    }

    /// Key frame requests while the screen does not change: ReplayKit sends no frames then, so
    /// the last picture is encoded again (as a key frame) on a 250 ms timer.
    private func tick() {
        if connectedNow, cc_ios_keyframe_requested() {
            encoder.requestKeyFrame()
            repeatHeld()
        }
        publishThrottled()
        checkMemory()
    }

    private func repeatHeld() {
        guard let picture = held, !isFinished else { return }
        // A repeated picture needs a strictly later timestamp than the last real frame.
        heldPTS = CMTimeAdd(heldPTS, CMTime(value: 1, timescale: 30))
        if encoder.encode(picture, pts: heldPTS) == .busy { droppedLocal += 1 }
    }

    // MARK: H264EncoderDelegate

    func encoder(_ encoder: H264Encoder, didProduceParameterSets annexB: [UInt8]) {
        _ = annexB.withUnsafeBufferPointer { cc_ios_set_config($0.baseAddress, $0.count) }
    }

    func encoder(_ encoder: H264Encoder, didProduce annexB: [UInt8], isKey: Bool, ptsMicroseconds: Int64) {
        let r = annexB.withUnsafeBufferPointer {
            cc_ios_push_frame($0.baseAddress, $0.count, isKey, ptsMicroseconds)
        }
        if r == CC_IOS_PUSH_NO_SENDER { fail(.connectionLost, "push after the sender ended") }
        // QUEUE_FULL / WAITING_FOR_KEY_FRAME / NOT_STREAMING: dropped by the core and counted; the
        // core asks for a key frame itself (cc_ios_keyframe_requested).
    }

    func encoder(_ encoder: H264Encoder, didFail message: String) {
        fail(.encoderFailed, message)
    }

    // MARK: status from Rust

    /// Called on a background thread of the Rust runtime.
    fileprivate func rustStatus(_ s: CcIosStatus, message: String) {
        let state: BroadcastStateName
        var error = BroadcastErrorName.none
        switch s.state {
        case Int32(CC_IOS_STATE_PENDING): state = .pending
        case Int32(CC_IOS_STATE_STREAMING): state = .streaming
        case Int32(CC_IOS_STATE_RECONNECTING): state = .reconnecting
        case Int32(CC_IOS_STATE_WAITING): state = .waiting
        case Int32(CC_IOS_STATE_CONNECTING): state = .connecting
        case Int32(CC_IOS_STATE_ENDED):
            state = .ended
            error = s.error == Int32(CC_IOS_ERR_PEER_LEFT) ? .peerLeft : .none
        default:
            state = .failed
            switch s.error {
            case Int32(CC_IOS_ERR_ROOM_NOT_FOUND): error = .roomNotFound
            case Int32(CC_IOS_ERR_SERVER_UNREACHABLE): error = .serverUnreachable
            case Int32(CC_IOS_ERR_CONNECTION_LOST): error = .connectionLost
            case Int32(CC_IOS_ERR_INVALID_ROOM_CODE): error = .invalidRoomCode
            case Int32(CC_IOS_ERR_INVALID_RELAY): error = .invalidRelay
            case Int32(CC_IOS_ERR_JOIN_DENIED): error = .joinDenied
            case Int32(CC_IOS_ERR_APPROVAL_TIMEOUT): error = .approvalTimeout
            case Int32(CC_IOS_ERR_ROOM_LOCKED): error = .roomLocked
            default: error = .internalError
            }
        }
        stateLock.lock()
        let wasConnected = connected
        connected = state == .streaming
        let sent = Int(s.sent), dropped = Int(s.dropped) + droppedLocalSnapshot
        let bytes = Int(s.bytes_sent), keys = Int(s.keyframes_sent)
        stateLock.unlock()
        if state == .streaming, !wasConnected { encoder.requestKeyFrame() }
        let check = Int(s.check), timeout = Int(s.timeout_secs)
        let room = s.room_name.map { String(cString: $0) } ?? ""
        let sas = s.sas.map { String(cString: $0) } ?? ""
        update { st in
            st.state = state; st.error = error
            st.sent = sent; st.dropped = dropped; st.bytes = bytes; st.keyframes = keys
            st.check = check > 0 ? check : nil
            st.timeoutSecs = timeout > 0 ? timeout : nil
            st.roomName = room.isEmpty ? nil : room
            st.sas = sas.isEmpty ? nil : sas
        }
        if state == .ended || state == .failed {
            finishBroadcast(error: error)
        }
    }

    // MARK: finishing and failing

    private var droppedLocalSnapshot: Int { droppedLocal }  // racy read of a counter: display only
    private var connectedNow: Bool { stateLock.lock(); defer { stateLock.unlock() }; return connected && !finished }
    private var isFinished: Bool { stateLock.lock(); defer { stateLock.unlock() }; return finished }

    /// End the broadcast with a message the person can read (shown by iOS in an alert).
    private func fail(_ error: BroadcastErrorName, _ detail: String) {
        update { $0.state = .failed; $0.error = error }
        finishBroadcast(error: error)
        NSLog("cometcast broadcast failed: \(error.rawValue): \(detail)")
    }

    /// Callable from any thread, including the Rust status callback and the VideoToolbox output
    /// callback. The work moves to a global queue: `cc_ios_stop` must not run inside the Rust
    /// callback, and invalidating the encoder inside its own output callback would deadlock.
    private func finishBroadcast(error: BroadcastErrorName, wayBack: Bool = false) {
        stateLock.lock()
        let wasConnected = connected
        if finished { stateLock.unlock(); return }
        finished = true
        stateLock.unlock()
        DispatchQueue.global(qos: .userInitiated).async { [self] in
            teardown(wayBack: wayBack && wasConnected)
            if wayBack {
                // A clean end: the app reads "ended" and joins again with the token.
                update { $0.state = .ended; $0.error = .none }
            }
            flushStatus()
            let nsError = NSError(domain: "de.cometcast.broadcast", code: 1, userInfo: [
                NSLocalizedDescriptionKey: localizedMessage(for: error),
            ])
            finishBroadcastWithError(nsError)
        }
    }

    private func teardown(wayBack: Bool = false) {
        // First: no more status writes from the timer while the way back is prepared.
        timer?.cancel(); timer = nil
        memoryPressure?.cancel(); memoryPressure = nil
        stopObservation = nil
        encoder.invalidate()
        held = nil
        scaler?.reset()
        if wayBack { prepareWayBack() }
        cc_ios_stop()
    }

    /// Ask the display to hold a new single-use token for the app's way back (20.5, step 5) and
    /// put it into the status. The display answering nothing (an old or a web display) leaves
    /// the token out: the app then joins with the code like Connect.
    private func prepareWayBack() {
        let token = HandoverToken.make()
        guard HandoverToken.isValid(token) else { return }
        if token.withCString({ cc_ios_request_handover($0) }) == 1 {
            update { $0.nextHandover = token }
        }
    }

    private func localizedMessage(for error: BroadcastErrorName) -> String {
        NSLocalizedString("error.\(error.rawValue)", comment: "Shown by iOS when the broadcast ends")
    }

    // MARK: memory

    private func startTimerAndMemoryWatch() {
        let t = DispatchSource.makeTimerSource(queue: encodeQueue)
        t.schedule(deadline: .now() + 0.25, repeating: 0.25)
        t.setEventHandler { [weak self] in self?.tick() }
        t.resume()
        timer = t
        let m = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: encodeQueue)
        m.setEventHandler { [weak self, weak m] in
            guard let self, let m else { return }
            if m.data.contains(.critical) {
                self.fail(.memory, "memory pressure: critical")
            } else {
                self.degraded = true  // warning: halve the frame rate
                self.held = nil
            }
        }
        m.resume()
        memoryPressure = m
    }

    /// Below this much free memory the broadcast is ended cleanly instead of being killed.
    /// UNCERTAIN: 6 MB is a guess; iOS gives no warning before the kill (docs/ios.md).
    private static let emergencyFreeBytes = 6 * 1024 * 1024

    private func checkMemory() {
        let free = Int(os_proc_available_memory())
        update { $0.freeMemoryMB = free / (1024 * 1024) }
        if free > 0, free < SampleHandler.emergencyFreeBytes { fail(.memory, "only \(free) bytes free") }
    }

    // MARK: status file

    private func publish(_ state: BroadcastStateName, error: BroadcastErrorName = .none) {
        update { $0.state = state; $0.error = error }
        flushStatus()
    }

    private func update(_ change: (inout BroadcastStatus) -> Void) {
        stateLock.lock()
        change(&lastStatus)
        stateLock.unlock()
    }

    private func publishThrottled() {
        if Date().timeIntervalSince(lastWrite) >= 1.0 { flushStatus() }
    }

    private func flushStatus() {
        stateLock.lock()
        seq += 1
        lastStatus.seq = seq
        lastStatus.updatedAt = Date().timeIntervalSince1970
        if let size = scaler?.outputSize, size.width > 0 {
            lastStatus.width = Int(size.width); lastStatus.height = Int(size.height)
        }
        let status = lastStatus
        stateLock.unlock()
        lastWrite = Date()
        try? store?.writeStatus(status)
        DarwinNotifier.post(BroadcastIdentity.statusNotification)
    }
}

/// The C callback of cc_ios_start. A free function because a C function pointer cannot capture.
private let statusCallback: CcIosStatusCallback = { statusPointer, user in
    guard let statusPointer, let user else { return }
    let handler = Unmanaged<SampleHandler>.fromOpaque(user).takeUnretainedValue()
    let message = statusPointer.pointee.message.map { String(cString: $0) } ?? ""
    handler.rustStatus(statusPointer.pointee, message: message)
}
