// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The Apple TV announces itself as a screen in the local network (spec 14.8, decision N50): a Bonjour service
// `_cometcast._tcp` published with `NWListener`, with the TXT layout of docs/discovery.md section 3, built by
// `displayAnnouncementTxt` (CometCastCore). Nothing listens for senders: every connection is closed at once; the
// service only exists to be found. The record is published again every 10 s (a platform cache must not keep a
// screen that died alive, and the presenter apps drop a row after 30 s without a refresh); `withdraw` is the goodbye.

import CometCastCore
import Foundation
import Network

@MainActor
final class Announcer {
    private static let refreshSeconds = 10.0
    private var listener: NWListener?
    private var instance: String?
    private var txt: [(String, String)] = []
    private var timer: Timer?

    /// Publish, or replace, the record. A different instance name (a data reset) is another service.
    func publish(instance: String, txt: [(String, String)]) {
        if instance != self.instance { stopListener() }
        self.instance = instance
        self.txt = txt
        if listener == nil {
            guard let l = try? NWListener(using: .tcp) else { return }
            l.newConnectionHandler = { $0.cancel() } // displays do not listen for senders
            l.start(queue: .main)
            listener = l
        }
        apply()
        if timer == nil {
            timer = Timer.scheduledTimer(withTimeInterval: Self.refreshSeconds, repeats: true) { [weak self] _ in
                Task { @MainActor in self?.apply() }
            }
        }
    }

    /// Goodbye: the service is withdrawn.
    func withdraw() {
        timer?.invalidate()
        timer = nil
        stopListener()
        instance = nil
    }

    private func apply() {
        guard let l = listener, let instance else { return }
        var record = NWTXTRecord()
        for (k, v) in txt { record[k] = v }
        l.service = NWListener.Service(name: instance, type: bonjourServiceType, domain: nil, txtRecord: record)
    }

    private func stopListener() {
        listener?.cancel()
        listener = nil
    }
}
