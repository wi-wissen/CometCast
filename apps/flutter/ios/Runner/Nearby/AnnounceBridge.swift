// SPDX-License-Identifier: GPL-3.0-or-later
// The iPad in the display role announces itself as a screen (spec 14.8, decision N50), MethodChannel
// `de.cometcast/announce`. The multicast entitlement is not counted on, so the Rust core's mDNS
// announcement stays off here and the system announces with `NWListener`: the core builds the record
// (crates/cc-discovery, display.rs: code, mode, name, server) and signs it with the key of the display
// (signed announcements, docs/design/signed-announcements.md); Dart (lib/core/native_announce.dart) hands it
// over. Nothing is decided here and there is no crypto in Swift on this path: the record is published as it is.
//
// From Dart:
//   publish {instance: String, txt: [String: String]}   publish or replace the Bonjour service
//                                                       `_cometcast._tcp` with this TXT record; sent at
//                                                       once on a change and every 10 s
//   withdraw                                           goodbye. The core publishes the signed `state=gone`
//                                                       record first; the listener is cancelled a moment
//                                                       later so that record is on the wire before the
//                                                       goodbye (a presenter removes the row on the signed
//                                                       record only)
//
// Nothing listens for senders: every connection is closed at once. UNCERTAIN: never compiled or run on a
// device (docs/ios.md); whether setting `service` on a running listener replaces the record (as Apple's
// documentation says) was not seen.

import Flutter
import Network

final class AnnounceBridge: NSObject {
    private static let serviceType = "_cometcast._tcp"

    private let channel: FlutterMethodChannel
    private let queue = DispatchQueue(label: "de.cometcast.announce")
    private var listener: NWListener?
    private var instance: String?
    /// Counts publishes: a goodbye that waits a moment is dropped when a new publish came meanwhile.
    private var generation = 0
    /// UNCERTAIN: 0.4 s is a guess for "the `gone` record has left"; not measured on a device.
    private static let goodbyeDelay = DispatchTimeInterval.milliseconds(400)

    init(messenger: FlutterBinaryMessenger) {
        channel = FlutterMethodChannel(name: "de.cometcast/announce", binaryMessenger: messenger)
        super.init()
        channel.setMethodCallHandler { [weak self] call, result in
            self?.handle(call, result: result)
        }
    }

    private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "publish":
            guard let args = call.arguments as? [String: Any],
                let instance = args["instance"] as? String,
                let txt = args["txt"] as? [String: String]
            else {
                result(FlutterError(code: "bad_args", message: nil, details: nil))
                return
            }
            queue.async { self.publish(instance: instance, txt: txt) }
            result(nil)
        case "withdraw":
            queue.async {
                let g = self.generation
                self.queue.asyncAfter(deadline: .now() + AnnounceBridge.goodbyeDelay) {
                    if self.generation == g { self.withdraw() }
                }
            }
            result(nil)
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    private func publish(instance: String, txt: [String: String]) {
        generation += 1
        if instance != self.instance { withdraw() }  // another id (a data reset) is another service
        self.instance = instance
        if listener == nil {
            guard let l = try? NWListener(using: .tcp) else { return }
            l.newConnectionHandler = { $0.cancel() }  // displays do not listen for senders
            l.start(queue: queue)
            listener = l
        }
        var record = NWTXTRecord()
        for (k, v) in txt { record[k] = v }
        listener?.service = NWListener.Service(
            name: instance, type: AnnounceBridge.serviceType, domain: nil, txtRecord: record)
    }

    private func withdraw() {
        listener?.cancel()
        listener = nil
        instance = nil
    }
}
