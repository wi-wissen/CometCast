// SPDX-License-Identifier: GPL-3.0-or-later
// The Swift side of the screen broadcast in the app (Dart: lib/core/ios_broadcast.dart):
//   MethodChannel 'de.cometcast/ios_broadcast'         prepare / showPicker / stop / status / isCaptured
//   EventChannel  'de.cometcast/ios_broadcast/status'  maps with the state the extension reports
// The app does NOT capture or send anything itself: it writes the room code and the relay into
// the App Group container, shows the system broadcast picker, and displays what the extension
// writes back (docs/ios.md, "Hand-over through the App Group").
//
// UNCERTAIN: never compiled or run. In particular the programmatic "tap" on the picker's button
// (see `showPicker`) is a known workaround that Apple does not document.

import Flutter
import ReplayKit
import UIKit

final class BroadcastBridge: NSObject, FlutterStreamHandler {
    private let store: BroadcastStore?
    private var sink: FlutterEventSink?
    private var statusObservation: DarwinObservation?
    private var staleTimer: Timer?
    private var lastSeq = -1
    private var reportedLost = false
    private var activeObserver: NSObjectProtocol?

    init(messenger: FlutterBinaryMessenger) {
        store = BroadcastStore.shared()
        super.init()
        let method = FlutterMethodChannel(name: "de.cometcast/ios_broadcast", binaryMessenger: messenger)
        method.setMethodCallHandler { [weak self] call, result in self?.handle(call, result: result) }
        FlutterEventChannel(name: "de.cometcast/ios_broadcast/status", binaryMessenger: messenger)
            .setStreamHandler(self)
    }

    // MARK: methods

    private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "prepare":
            guard let store else {
                return result(FlutterError(code: "app_group_unavailable",
                                           message: "The App Group container is not available (signing).", details: nil))
            }
            guard let a = call.arguments as? [String: Any],
                  let code = a["code"] as? String, let relay = a["relay"] as? String
            else { return result(FlutterError(code: "bad_arguments", message: "code and relay needed", details: nil)) }
            // The name and the single-use hand-over token travel in config.json (App Group,
            // docs/design/native-apps.md 20.5): the extension sends the name and joins with
            // the token. A token that is not 1 to 64 base64url characters is not handed over.
            let token = (a["handover"] as? String).flatMap { HandoverToken.isValid($0) ? $0 : nil }
            let config = BroadcastConfig(
                code: code, relay: relay, requestedAt: Date().timeIntervalSince1970,
                maxDimension: a["maxDimension"] as? Int ?? 1280, fps: a["fps"] as? Int ?? 30,
                bitrate: a["bitrate"] as? Int ?? 2_500_000,
                name: a["name"] as? String, handover: token).clamped
            store.clearStopRequest()
            store.deleteStatus()
            do { try store.writeConfig(config); lastSeq = -1; reportedLost = false; result(true) }
            catch { result(FlutterError(code: "write_failed", message: "\(error)", details: nil)) }
        case "showPicker":
            DispatchQueue.main.async { result(self.showPicker()) }
        case "stop":
            store?.requestStop()
            store?.deleteConfig()
            DarwinNotifier.post(BroadcastIdentity.stopNotification)
            result(nil)
        case "status":
            result(store?.readStatus().map(Self.map))
        case "writeShareSettings":
            // What the share extension reads (native-apps.md 21.3): the name, the servers
            // confirmed on this device and the last used server. Failures are ignored: the
            // extension then asks for the name itself.
            guard let a = call.arguments as? [String: Any],
                  let shareStore = ShareSettingsStore.shared(group: BroadcastIdentity.appGroup)
            else { return result(false) }
            shareStore.write(ShareSettings(
                name: a["name"] as? String ?? "",
                confirmedServers: a["confirmedServers"] as? [String] ?? [],
                lastServer: a["lastServer"] as? String))
            result(true)
        case "readShareSettings":
            // The extension writes back only the name.
            let s = ShareSettingsStore.shared(group: BroadcastIdentity.appGroup)?.read()
            result(s.map { ["name": $0.name, "confirmedServers": $0.confirmedServers, "lastServer": $0.lastServer as Any] })
        case "isCaptured":
            result(UIScreen.main.isCaptured)
        case "info":
            result(["model": UIDevice.current.model, "systemVersion": UIDevice.current.systemVersion,
                    "name": UIDevice.current.name, "appGroup": BroadcastIdentity.appGroup,
                    "extension": BroadcastIdentity.extensionBundleId,
                    "appGroupAvailable": store != nil])
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    /// Opens the system sheet "Screen Broadcast" with Comet Cast preselected. Apple offers only
    /// `RPSystemBroadcastPickerView`, a button that the person has to tap. The usual workaround
    /// (used here) puts one into the window and sends a tap to its button. If a future iOS
    /// version changes the view hierarchy the button is not found and `false` is returned: the
    /// Dart side then tells the person to start the broadcast from Control Center (long press
    /// on the record button, choose Comet Cast), which works with the same hand-over.
    private func showPicker() -> Bool {
        guard let window = UIApplication.shared.connectedScenes
            .compactMap({ ($0 as? UIWindowScene)?.windows.first(where: { $0.isKeyWindow }) }).first
        else { return false }
        let picker = RPSystemBroadcastPickerView(frame: CGRect(x: 0, y: 0, width: 44, height: 44))
        picker.preferredExtension = BroadcastIdentity.extensionBundleId
        picker.showsMicrophoneButton = false
        picker.alpha = 0.01
        window.addSubview(picker)
        let button = picker.subviews.compactMap { $0 as? UIButton }.first
        button?.sendActions(for: .touchUpInside)
        // The sheet needs the view for a moment; remove it afterwards.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { picker.removeFromSuperview() }
        return button != nil
    }

    // MARK: status events

    func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        sink = events
        statusObservation = DarwinNotifier.observe(BroadcastIdentity.statusNotification) { [weak self] in
            self?.pushStatus()
        }
        activeObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main
        ) { [weak self] _ in self?.pushStatus(force: true) }
        // The extension writes at least once per second; a stale status means it is gone.
        staleTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            self?.pushStatus(force: true)
        }
        return nil
    }

    func onCancel(withArguments arguments: Any?) -> FlutterError? {
        sink = nil
        statusObservation = nil
        staleTimer?.invalidate(); staleTimer = nil
        if let o = activeObserver { NotificationCenter.default.removeObserver(o); activeObserver = nil }
        return nil
    }

    private func pushStatus(force: Bool = false) {
        guard let sink, let status = store?.readStatus() else { return }
        let stale = status.isStale()
        if stale {
            if reportedLost { return }
            reportedLost = true
            var m = Self.map(status)
            m["state"] = "lost"
            m["stale"] = true
            sink(m)
            return
        }
        reportedLost = false
        if !force && status.seq == lastSeq { return }
        lastSeq = status.seq
        sink(Self.map(status))
    }

    private static func map(_ s: BroadcastStatus) -> [String: Any] {
        var m: [String: Any] = [
            "seq": s.seq, "state": s.state.rawValue, "error": s.error.rawValue, "sent": s.sent,
            "dropped": s.dropped, "bytes": s.bytes, "keyframes": s.keyframes, "width": s.width,
            "height": s.height, "freeMemoryMB": s.freeMemoryMB, "updatedAt": s.updatedAt, "stale": false]
        // The page's fallbacks and the way back (20.5): only what the extension wrote.
        if let v = s.check { m["check"] = v }
        if let v = s.timeoutSecs { m["timeoutSecs"] = v }
        if let v = s.roomName { m["roomName"] = v }
        if let v = s.sas { m["sas"] = v }
        if let v = s.nextHandover { m["nextHandover"] = v }
        return m
    }
}
