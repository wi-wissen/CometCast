// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here). The pure parts (SRV rdata, TXT, ordering, trust rules) are in
// CometCastCore and tested on Linux; this file is only the glue to the Apple APIs.
//
//   * mDNS: NWBrowser for _cometcast._tcp with TXT records (needs NSBonjourServices and the local network
//     permission, Info.plist).
//   * DNS-SRV: DNSServiceQueryRecord (dns_sd) for _cometcast._tcp. UNCERTAIN: whether the system appends the
//     search domains of the network to this name is not known (the Rust crate asks every search domain itself);
//     tvOS has no public API for the search domain list. Device test plan D-section checks it.

import Combine
import CometCastCore
import Foundation
import Network
import dnssd

@MainActor
final class Discovery: ObservableObject {
    @Published private(set) var announced: [ServerCandidate] = []
    @Published private(set) var srv: [ServerCandidate] = []
    @Published private(set) var searching = false
    @Published private(set) var localNetworkDenied = false

    private var browser: NWBrowser?
    private var receivers: [String: AnnouncedReceiver] = [:] // instance -> first one seen
    private let maxReceivers = 64

    func start(browseAnnouncements: Bool) {
        stop()
        searching = true
        startSRV()
        if browseAnnouncements { startBrowser() }
        // The search is bounded: after 3 s (the Rust default for DNS) the hint disappears. Browsing goes on.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.searching = false }
    }

    func stop() {
        browser?.cancel()
        browser = nil
    }

    // MARK: mDNS

    private func startBrowser() {
        let b = NWBrowser(for: .bonjourWithTXTRecord(type: bonjourServiceType, domain: nil), using: .tcp)
        b.stateUpdateHandler = { [weak self] state in
            if case .waiting(let err) = state, case .dns(let code) = err, code == DNSServiceErrorType(kDNSServiceErr_PolicyDenied) {
                Task { @MainActor in self?.localNetworkDenied = true }
            }
        }
        b.browseResultsChangedHandler = { [weak self] results, _ in
            var pairs: [(String, [(String, String)])] = []
            for r in results {
                guard case .service(let name, _, _, _) = r.endpoint else { continue }
                var txt: [(String, String)] = []
                if case .bonjour(let rec) = r.metadata { for (k, v) in rec.dictionary { txt.append((k, v)) } }
                pairs.append((name, txt))
            }
            Task { @MainActor in self?.apply(pairs) }
        }
        b.start(queue: .main)
        browser = b
    }

    private func apply(_ list: [(String, [(String, String)])]) {
        var out: [ServerCandidate] = []
        // The first receiver seen under an instance name is kept (a later one cannot take the entry over).
        for (name, txt) in list.prefix(maxReceivers) {
            if receivers[name] == nil, let r = announcedReceiver(instanceName: name, txt: txt) { receivers[name] = r }
            if let r = receivers[name], let a = r.announcedServer { out.append(ServerCandidate(address: a, source: .displayAnnouncement)) }
        }
        announced = out
    }

    // MARK: DNS-SRV

    private final class SRVBox {
        var records: [SrvRecord] = []
        var ref: DNSServiceRef?
    }

    private func startSRV() {
        let box = SRVBox()
        let ctx = Unmanaged.passRetained(box).toOpaque()
        var ref: DNSServiceRef?
        let err = DNSServiceQueryRecord(&ref, 0, 0, srvServiceName, UInt16(kDNSServiceType_SRV), UInt16(kDNSServiceClass_IN), { _, flags, _, errorCode, _, _, _, rdlen, rdata, _, context in
            guard errorCode == kDNSServiceErr_NoError, let rdata, let context else { return }
            let b = Unmanaged<SRVBox>.fromOpaque(context).takeUnretainedValue()
            if flags & DNSServiceFlags(kDNSServiceFlagsAdd) != 0, b.records.count < maxSrvRecords,
               let rec = parseSrvRdata([UInt8](UnsafeBufferPointer(start: rdata.assumingMemoryBound(to: UInt8.self), count: Int(rdlen)))) {
                b.records.append(rec)
            }
        }, ctx)
        guard err == kDNSServiceErr_NoError, let ref else { Unmanaged<SRVBox>.fromOpaque(ctx).release(); return }
        box.ref = ref
        DNSServiceSetDispatchQueue(ref, .main)
        // Collect for 3 s, then use what came.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
            if let r = box.ref { DNSServiceRefDeallocate(r); box.ref = nil }
            let recs = box.records
            Unmanaged<SRVBox>.fromOpaque(ctx).release()
            self?.srv = srvCandidates(records: recs, randBelow: { UInt32.random(in: 0..<$0) })
        }
    }
}
