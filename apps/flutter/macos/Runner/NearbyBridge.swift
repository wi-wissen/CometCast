// SPDX-License-Identifier: GPL-3.0-or-later
// macOS: screens on the same network for Dart (spec 14, native-apps.md 4.1, decision N50),
// MethodChannel `de.cometcast/nearby`. The same bridge as iOS (ios/Runner/Nearby/NearbyBridge.swift),
// so the Local Network permission of macOS 15 and later gives "Not asked yet" and "Not allowed" like
// iOS. Before macOS 15 there is no such permission: `permission` is always "granted".
// Browses `_cometcast._tcp` with NWBrowser (`.bonjourWithTXTRecord`) and
// hands the raw records to Dart, which passes them to the Rust core; the core checks and cleans
// every field, keeps the list and the 30 s expiry (crates/cc-discovery, nearby.rs). Nothing is
// decided here.
//
// From Dart:
//   permission   -> "notAsked" | "granted"  (there is no call that reads the Local Network
//                   permission: "notAsked" until a search was started once; a refusal is learned
//                   when a browse runs and is reported with `permission` "denied")
//   start / stop    browse; `start` is what makes the system ask (Info.plist:
//                   NSLocalNetworkUsageDescription = ios.localNetworkUsage, NSBonjourServices has
//                   _cometcast._tcp). Dart calls it only as the direct result of the person's tap
//                   the first time (spec 14.2, "Not asked yet"), and by itself on later visits.
//   hasNetwork   -> Bool   Wi-Fi or Ethernet exists (NWPathMonitor; mobile data only = "No local
//                   network")
//   openSettings -> Bool   the Local Network pane of System Settings
// To Dart: `seen` {instance, addresses: [], txt}, `gone` instance, `reset`, `permission` state,
// `network` Bool.
//
// Refresh: NWBrowser reports a service when it appears or changes, but a display refreshes its
// record without changing it, and the core drops a row after 30 s without a refresh. So the
// browser is started again every `refreshSeconds` and answers again with what it sees; a service
// that is not seen any more is not refreshed and leaves the core's list by expiry.
//
// Expiry (decision N50): NWBrowser answers from the system's cache until a record's lifetime has
// passed, so the displays announce with records that live 30 s (`RECORD_TTL_SECS` in
// crates/cc-discovery): a service that died without a goodbye is removed by the system within 30 s.
//
// UNCERTAIN: never compiled or run on a device (docs/ios.md). The Local Network prompt and the
// PolicyDenied mapping are as documented by Apple, not seen.

import Cocoa
import FlutterMacOS
import Network

final class NearbyBridge: NSObject {
    private static let serviceType = "_cometcast._tcp"
    private static let askedKey = "nearby.asked"
    private static let refreshSeconds = 10.0

    private let channel: FlutterMethodChannel
    private let queue = DispatchQueue(label: "de.cometcast.nearby")
    private var browser: NWBrowser?
    private var refresh: DispatchSourceTimer?
    private var running = false
    private let monitor = NWPathMonitor()
    private var hasNetworkNow = true
    private var lastReported: Bool?

    init(messenger: FlutterBinaryMessenger) {
        channel = FlutterMethodChannel(name: "de.cometcast/nearby", binaryMessenger: messenger)
        super.init()
        channel.setMethodCallHandler { [weak self] call, result in
            self?.handle(call, result: result)
        }
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self = self else { return }
            let local =
                path.status == .satisfied
                && (path.usesInterfaceType(.wifi) || path.usesInterfaceType(.wiredEthernet))
            self.hasNetworkNow = local
            if local != self.lastReported {
                self.lastReported = local
                DispatchQueue.main.async { self.channel.invokeMethod("network", arguments: local) }
            }
        }
        monitor.start(queue: queue)
    }

    private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "permission":
            if #available(macOS 15.0, *) {
                let asked = UserDefaults.standard.bool(forKey: NearbyBridge.askedKey)
                result(asked ? "granted" : "notAsked")
            } else {
                result("granted")
            }
        case "start":
            // Starting the browse is what makes the system show its prompt.
            UserDefaults.standard.set(true, forKey: NearbyBridge.askedKey)
            queue.async { self.startBrowse() }
            result(nil)
        case "stop":
            queue.async { self.stopBrowse() }
            result(nil)
        case "hasNetwork":
            result(hasNetworkNow)
        case "openSettings":
            // System Settings, Privacy & Security, Local Network.
            guard
                let url = URL(
                    string: "x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork")
            else {
                result(false)
                return
            }
            result(NSWorkspace.shared.open(url))
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    // MARK: browsing (on `queue`)

    private func startBrowse() {
        guard !running else { return }
        running = true
        makeBrowser()
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(
            deadline: .now() + NearbyBridge.refreshSeconds, repeating: NearbyBridge.refreshSeconds)
        timer.setEventHandler { [weak self] in self?.restartBrowser() }
        timer.resume()
        refresh = timer
    }

    private func stopBrowse() {
        running = false
        refresh?.cancel()
        refresh = nil
        browser?.cancel()
        browser = nil
    }

    private func restartBrowser() {
        guard running else { return }
        browser?.cancel()
        makeBrowser()
    }

    private func makeBrowser() {
        let b = NWBrowser(
            for: .bonjourWithTXTRecord(type: NearbyBridge.serviceType, domain: nil),
            using: NWParameters())
        b.stateUpdateHandler = { [weak self] state in
            self?.onState(state)
        }
        b.browseResultsChangedHandler = { [weak self] _, changes in
            self?.onChanges(changes)
        }
        browser = b
        b.start(queue: queue)
    }

    private func onState(_ state: NWBrowser.State) {
        switch state {
        case .ready:
            send("permission", "granted")
        case .waiting(let error), .failed(let error):
            // The Local Network permission was refused: `kDNSServiceErr_PolicyDenied`.
            if case .dns(let code) = error, code == DNSServiceErrorType(kDNSServiceErr_PolicyDenied) {
                send("permission", "denied")
            }
        default:
            break
        }
    }

    private func onChanges(_ changes: Set<NWBrowser.Result.Change>) {
        for change in changes {
            switch change {
            case .added(let r), .changed(_, let r, _):
                seen(r)
            case .removed(let r):
                if case .service(let name, _, _, _) = r.endpoint {
                    send("gone", name)
                }
            default:
                break
            }
        }
    }

    private func seen(_ r: NWBrowser.Result) {
        guard case .service(let name, _, _, _) = r.endpoint else { return }
        var txt: [String: String] = [:]
        if case .bonjour(let record) = r.metadata {
            for (k, v) in record.dictionary.prefix(32) where v.utf8.count <= 255 {
                txt[k] = v
            }
        }
        // Apple's browser hands over endpoints, not addresses: the core merges by the id in the
        // record, and takes the first one it saw for a name.
        send("seen", ["instance": name, "addresses": [String](), "txt": txt])
    }

    private func send(_ method: String, _ arguments: Any?) {
        DispatchQueue.main.async { self.channel.invokeMethod(method, arguments: arguments) }
    }
}
