// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The Apple TV announces itself as a screen in the local network (spec 14.8, decision N50): a Bonjour service
// `_cometcast._tcp` published with `NWListener`, with the signed TXT layout `v=2` of docs/discovery.md section 3 and
// docs/design/signed-announcements.md, built by `displayAnnouncementTxt` (CometCastCore) with the key of the display
// (`ConfigStore.displayKey`, Ed25519, CryptoKit). Nothing listens for senders: every connection is closed at once; the
// service only exists to be found. The record is signed and published again every 10 s with a new `seq` (a platform
// cache must not keep a screen that died alive, and the presenter apps drop a row after 30 s without an accepted
// record). `withdraw` publishes the signed `state=gone` record first and cancels the listener a moment later
// (UNCERTAIN: 0.4 s is a guess), because a presenter removes a row only on the signed record, never on the goodbye.

import CometCastCore
import Foundation
import Network

@MainActor
final class Announcer {
    private static let refreshSeconds = 10.0
    private static let goodbyeDelayNanoseconds: UInt64 = 400_000_000
    private var listener: NWListener?
    private var instance: String?
    /// Builds the signed record for a `seq` and a state; nil when it cannot be announced.
    private var record: ((UInt64, AnnounceState) -> [(String, String)]?)?
    private var timer: Timer?
    private var seq = AnnouncementSeq(floor: ConfigStore.announcementSeqFloor)
    /// Counts publishes: a goodbye that waits a moment is dropped when a new publish came meanwhile.
    private var generation = 0

    /// Publish, or replace, the record. `record` is called with a new `seq` for every publication (also the refresh
    /// every 10 s), so every record is signed anew. A different instance name (a data reset) is another service.
    func publish(instance: String, record: @escaping (UInt64, AnnounceState) -> [(String, String)]?) {
        generation += 1
        if instance != self.instance { stopListener() }
        self.instance = instance
        self.record = record
        if listener == nil {
            guard let l = try? NWListener(using: .tcp) else { return }
            l.newConnectionHandler = { $0.cancel() } // displays do not listen for senders
            l.start(queue: .main)
            listener = l
        }
        apply(.live)
        if timer == nil {
            timer = Timer.scheduledTimer(withTimeInterval: Self.refreshSeconds, repeats: true) { [weak self] _ in
                Task { @MainActor in self?.apply(.live) }
            }
        }
    }

    /// Goodbye: the signed `gone` record first, then the service is withdrawn.
    func withdraw() {
        timer?.invalidate()
        timer = nil
        guard listener != nil else { instance = nil; record = nil; return }
        apply(.gone)
        let g = generation
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: Self.goodbyeDelayNanoseconds)
            guard self.generation == g else { return } // published again meanwhile
            self.stopListener()
            self.instance = nil
            self.record = nil
        }
    }

    private func apply(_ state: AnnounceState) {
        guard let l = listener, let instance, let record else { return }
        let next = seq.next(nowMs: UInt64(Date().timeIntervalSince1970 * 1000))
        // Persisted with a reserve, so a clock that goes back after a restart cannot reuse values.
        ConfigStore.announcementSeqFloor = next &+ announcementSeqReserve
        guard let txt = record(next, state) else { return }
        var rec = NWTXTRecord()
        for (k, v) in txt { rec[k] = v }
        l.service = NWListener.Service(name: instance, type: bonjourServiceType, domain: nil, txtRecord: rec)
    }

    private func stopListener() {
        listener?.cancel()
        listener = nil
    }
}
