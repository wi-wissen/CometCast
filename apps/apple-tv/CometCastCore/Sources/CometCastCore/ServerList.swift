// SPDX-License-Identifier: GPL-3.0-or-later
//
// Server (relay) discovery rules, a Swift port of crates/cc-discovery (address.rs, policy.rs, merge.rs, srv.rs
// ordering, announce.rs TXT parsing). docs/discovery.md describes the rules and what is unverified. Order:
//   1. managed configuration (MDM)   2. DNS-SRV _cometcast._tcp   3. announcement by a display (mDNS)
//   4. build default / built-in list
// Only the pure parts are here. The network parts (dns_sd, NWBrowser) are in the app target.

import Foundation
#if canImport(Glibc)
import Glibc
#elseif canImport(Darwin)
import Darwin
#endif

public let maxServerLen = 300
public let maxPolicyServers = 16
public let maxSrvRecords = 32
public let maxSrvServers = 8
public let maxNameChars = 48
/// The built-in list. Placeholder: relay.cometcast.de does not exist yet (docs/server.md).
public let defaultServers = ["relay.cometcast.de:443"]
public let srvServiceName = "_cometcast._tcp"
public let bonjourServiceType = "_cometcast._tcp"

public enum Trust: Sendable { case admin, network }


// MARK: addresses

/// Strict IPv4 (four decimal parts, no leading zeros) as Rust's `Ipv4Addr::from_str`.
func parseStrictIPv4(_ s: String) -> [UInt8]? {
    let p = s.split(separator: ".", omittingEmptySubsequences: false)
    guard p.count == 4 else { return nil }
    var out: [UInt8] = []
    for x in p {
        guard !x.isEmpty, x.count <= 3, x.allSatisfy({ $0.isASCII && $0.isNumber }), !(x.count > 1 && x.hasPrefix("0")),
              let v = UInt8(x) else { return nil }
        out.append(v)
    }
    return out
}

/// IPv6 text to its 16 bytes and back (RFC 5952 form) with the C library, so no hand-written parser is needed.
func parseIPv6(_ s: String) -> [UInt8]? {
    var addr = in6_addr()
    guard s.withCString({ inet_pton(AF_INET6, $0, &addr) }) == 1 else { return nil }
    return withUnsafeBytes(of: &addr) { Array($0) }
}

func formatIPv6(_ b: [UInt8]) -> String {
    var addr = in6_addr()
    withUnsafeMutableBytes(of: &addr) { $0.copyBytes(from: b) }
    var buf = [CChar](repeating: 0, count: 64)
    guard inet_ntop(AF_INET6, &addr, &buf, 64) != nil else { return "" }
    return String(cString: buf)
}

private func isPrivateV4(_ o: [UInt8]) -> Bool {
    o[0] == 10 || (o[0] == 172 && (16...31).contains(o[1])) || (o[0] == 192 && o[1] == 168) || (o[0] == 100 && (64..<128).contains(o[1]))
}

/// Check a DNS host name (letters, digits, hyphens; labels of 1 to 63 characters); lower case, no trailing dot.
/// The last label must contain a letter and not start with `0x` (so no spelling of an IPv4 address gets through).
public func validateHostname(_ input: String) -> String? {
    var name = input
    if name.hasSuffix(".") { name.removeLast() }
    if name.isEmpty || name.utf8.count > 253 { return nil }
    name = name.lowercased()
    let labels = name.split(separator: ".", omittingEmptySubsequences: false)
    for l in labels {
        let b = Array(l.utf8)
        if b.isEmpty || b.count > 63 { return nil }
        if !b.allSatisfy({ ($0 >= 0x30 && $0 <= 0x39) || ($0 >= 0x61 && $0 <= 0x7A) || $0 == 0x2D }) || b.first == 0x2D || b.last == 0x2D { return nil }
    }
    let last = labels.last!
    if last.hasPrefix("0x") || !last.utf8.contains(where: { $0 >= 0x61 && $0 <= 0x7A }) { return nil }
    return name
}

/// Check `input` and return its canonical text `[scheme://]host:port` (host lower case, IPv6 in brackets).
/// A port is required unless an administrator gave a scheme.
public func normalizeServer(_ input: String, trust: Trust) -> String? {
    if input.utf8.count > maxServerLen { return nil }
    let input = jsTrim(input)
    if input.isEmpty { return nil }
    if !input.utf8.allSatisfy({ $0 >= 0x21 && $0 <= 0x7E }) { return nil }
    var scheme: String?
    var rest = Substring(input)
    if let r = input.range(of: "://") {
        if trust == .network { return nil }
        let s = input[..<r.lowerBound].lowercased()
        guard ["ws", "wss", "http", "https"].contains(s) else { return nil }
        scheme = s
        rest = input[r.upperBound...]
    }
    if rest.contains(where: { "/@?#\\%".contains($0) }) { return nil }
    // host and port
    var host: Substring
    var port: Int?
    if rest.hasPrefix("[") {
        guard let close = rest.firstIndex(of: "]") else { return nil }
        host = rest[rest.index(after: rest.startIndex)..<close]
        let after = rest[rest.index(after: close)...]
        if !after.isEmpty {
            guard after.hasPrefix(":"), let p = parsePort(after.dropFirst()) else { return nil }
            port = p
        }
    } else if let c = rest.lastIndex(of: ":") {
        host = rest[..<c]
        if host.contains(":") { return nil }
        guard let p = parsePort(rest[rest.index(after: c)...]) else { return nil }
        port = p
    } else { host = rest }
    var portN = 0
    if let port { portN = port } else if scheme == nil { return nil }
    guard let h = canonicalHost(String(host), trust: trust) else { return nil }
    let authority: String
    switch (portN, h.contains(":")) {
    case (0, true): authority = "[\(h)]"
    case (0, false): authority = h
    case (let p, true): authority = "[\(h)]:\(p)"
    case (let p, false): authority = "\(h):\(p)"
    }
    return scheme.map { "\($0)://\(authority)" } ?? authority
}

private func parsePort(_ s: Substring) -> Int? {
    guard !s.isEmpty, s.count <= 5, s.allSatisfy({ $0.isASCII && $0.isNumber }), let p = Int(s), p != 0, p <= 65535 else { return nil }
    return p
}

private func canonicalHost(_ host: String, trust: Trust) -> String? {
    if host.isEmpty { return nil }
    if let v4 = parseStrictIPv4(host) {
        if trust == .network, !networkIPv4Allowed(v4) { return nil }
        return v4.map(String.init).joined(separator: ".")
    }
    if let v6 = parseIPv6(host) {
        if trust == .network, !networkIPv6Allowed(v6) { return nil }
        return formatIPv6(v6)
    }
    if host.contains(":") { return nil }
    guard let name = validateHostname(host) else { return nil }
    if trust == .network, name == "localhost" || name.hasSuffix(".localhost") { return nil }
    return name
}

private func networkIPv4Allowed(_ o: [UInt8]) -> Bool {
    if o == [0, 0, 0, 0] || o[0] == 127 || (224...239).contains(o[0]) || o == [255, 255, 255, 255] { return false }
    if o[0] == 169 && o[1] == 254 { return false }
    if o == [100, 100, 100, 200] { return false }
    return isPrivateV4(o)
}

private func networkIPv6Allowed(_ b: [UInt8]) -> Bool {
    let seg = (0..<8).map { UInt16(b[$0 * 2]) << 8 | UInt16(b[$0 * 2 + 1]) }
    if seg.allSatisfy({ $0 == 0 }) || (seg[0..<7].allSatisfy({ $0 == 0 }) && seg[7] == 1) { return false } // :: and ::1
    if b[0] == 0xff { return false } // multicast
    if seg[0..<5].allSatisfy({ $0 == 0 }) && seg[5] == 0xffff { return false } // IPv4-mapped
    if seg[0] & 0xffc0 == 0xfe80 { return false }
    if seg[0] == 0xfd00 && seg[1] == 0x0ec2 && seg[2] == 0 && seg[7] == 0x254 { return false }
    return seg[0] & 0xfe00 == 0xfc00
}

private func isLocalHost(_ host: String) -> Bool {
    parseStrictIPv4(host) != nil || parseIPv6(host) != nil || host.hasSuffix(".local")
}

/// Check a server address that came from the network and decide about TLS. `tls`: true = TLS, false = explicit plain
/// ws, nil = not stated (DNS-SRV). Plain ws is only accepted for a private IP literal or a `.local` name; every
/// other host needs TLS. Returns the canonical address WITH scheme and whether it is TLS.
public func networkServer(_ input: String, tls: Bool?) -> (address: String, tls: Bool)? {
    guard let addr = normalizeServer(input, trust: .network) else { return nil }
    var host = addr
    if let c = addr.lastIndex(of: ":") { host = String(addr[..<c]) }
    host = host.trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
    let local = isLocalHost(host)
    let useTLS: Bool
    switch tls {
    case .some(true): useTLS = true
    case .some(false): if local { useTLS = false } else { return nil }
    case .none: useTLS = !local
    }
    return ("\(useTLS ? "wss" : "ws")://\(addr)", useTLS)
}

/// Removes control (Cc), format (Cf: bidi overrides, zero width), line/paragraph separators and surrounding blanks
/// from text that came from the network and cuts it to `maxChars` characters. Same list as the web client's
/// `sanitizeName`, plus U+0001..U+0009 handled as controls.
public func cleanText(_ s: String, maxChars: Int) -> String {
    var out = String.UnicodeScalarView()
    for u in s.unicodeScalars where !(u.value <= 0x1F || (u.value >= 0x7F && u.value <= 0x9F) || isFormatChar(u.value)) { out.append(u) }
    let t = String(out).trimmingCharacters(in: .whitespaces)
    return String(t.unicodeScalars.prefix(maxChars).map(Character.init))
}

private func isFormatChar(_ v: UInt32) -> Bool {
    switch v {
    case 0xAD, 0x0600...0x0605, 0x061C, 0x06DD, 0x070F, 0x0890...0x0891, 0x08E2, 0x180E, 0x200B...0x200F, 0x2028...0x202E,
         0x2060...0x2064, 0x2066...0x206F, 0xFEFF, 0xFFF9...0xFFFB, 0x110BD, 0x110CD, 0x13430...0x1343F, 0x1BCA0...0x1BCA3,
         0x1D173...0x1D17A, 0xE0001, 0xE0020...0xE007F: return true
    default: return false
    }
}

// MARK: policy (managed configuration)

public enum ModeHint: String, Sendable { case open, `internal` }

public struct ServerPolicy: Equatable, Sendable {
    public var servers: [String] = []
    public var forbidDefaultList = false
    public var forbidUserOverride = false
    public var forbidAnnouncement = false
    public var mode: ModeHint?

    public init() {}

    /// Validates a managed configuration dictionary (already converted to JSONValue). Unknown keys make the whole
    /// policy invalid (a typo must not switch off a security setting without notice). nil = invalid.
    public static func parse(_ v: JSONValue) -> ServerPolicy? {
        guard case .object(let o) = v else { return nil }
        let known: Set<String> = ["version", "servers", "forbid_default_list", "forbid_user_override", "forbid_announcement", "mode"]
        if !Set(o.keys).isSubset(of: known) { return nil }
        if let ver = o["version"], ver != .null { guard ver.integerValue == 1 else { return nil } }
        var p = ServerPolicy()
        if let s = o["servers"], s != .null {
            guard case .array(let a) = s, a.count <= maxPolicyServers else { return nil }
            for e in a {
                guard case .string(let str) = e, let n = normalizeServer(str, trust: .admin) else { return nil }
                if !p.servers.contains(n) { p.servers.append(n) }
            }
        }
        for (key, set) in [("forbid_default_list", \ServerPolicy.forbidDefaultList), ("forbid_user_override", \.forbidUserOverride), ("forbid_announcement", \.forbidAnnouncement)] {
            if let b = o[key], b != .null { guard case .bool(let x) = b else { return nil }; p[keyPath: set] = x }
        }
        if let m = o["mode"], m != .null {
            guard case .string(let ms) = m, let mode = ModeHint(rawValue: ms) else { return nil }
            p.mode = mode
        }
        return p
    }
}

public enum PolicyState: Equatable, Sendable {
    /// No managed configuration.
    case absent
    case active(ServerPolicy)
    /// Present but unusable: fail closed.
    case invalid
}

// MARK: merge

public struct ServerCandidate: Equatable, Sendable {
    public var address: String
    public var source: RelaySource
    public init(address: String, source: RelaySource) { self.address = address; self.source = source }
}

public struct MergedServers: Equatable, Sendable {
    public var servers: [ServerCandidate] = []
    /// The policy forbids other servers than its own (or is unusable): the UI must not let people type another one.
    public var locked = false
    public var modeHint: ModeHint?
    /// The managed configuration exists but is unusable (shown as a notice).
    public var policyUnusable = false
}

/// Puts the answers of all sources into one ordered list (same rules as `merge` in crates/cc-discovery/src/merge.rs).
public func mergeServers(policy: PolicyState, srv: [ServerCandidate], announced: [ServerCandidate], defaults: [ServerCandidate]) -> MergedServers {
    var out = MergedServers()
    func push(_ s: ServerCandidate) {
        let key = s.address.lowercased()
        if !out.servers.contains(where: { $0.address.lowercased() == key }) { out.servers.append(s) }
    }
    var active: ServerPolicy?
    switch policy {
    case .absent: break
    case .invalid:
        out.locked = true
        out.policyUnusable = true
        return out
    case .active(let p): active = p
    }
    if let p = active {
        out.modeHint = p.mode
        for a in p.servers { push(ServerCandidate(address: a, source: .mdm)) }
        if p.forbidUserOverride { out.locked = true; return out }
    }
    for s in srv { push(s) }
    if !(active?.forbidAnnouncement ?? false) { for s in announced { push(s) } }
    if !(active?.forbidDefaultList ?? false) { for s in defaults { push(s) } }
    return out
}

// MARK: SRV

public struct SrvRecord: Equatable, Sendable {
    public var priority: UInt16, weight: UInt16, port: UInt16
    public var target: String
    public init(priority: UInt16, weight: UInt16, port: UInt16, target: String) {
        self.priority = priority; self.weight = weight; self.port = port; self.target = target
    }
}

/// Parses the rdata of a DNS SRV record as `DNSServiceQueryRecord` hands it out: priority, weight, port (big endian)
/// and the target as uncompressed DNS labels. A compression pointer, a label over 63 bytes, a name over 253 bytes or
/// trailing bytes make the record invalid (answers from the network are untrusted). The target is returned in
/// text form without a trailing dot ("." for the root name, which means "service not available").
public func parseSrvRdata(_ b: [UInt8]) -> SrvRecord? {
    guard b.count >= 7 else { return nil }
    let prio = UInt16(b[0]) << 8 | UInt16(b[1]), weight = UInt16(b[2]) << 8 | UInt16(b[3]), port = UInt16(b[4]) << 8 | UInt16(b[5])
    var i = 6
    var labels: [String] = []
    var total = 0
    while true {
        guard i < b.count else { return nil }
        let len = Int(b[i]); i += 1
        if len == 0 { break }
        if len > 63 { return nil } // also rejects compression pointers (0xC0..)
        guard i + len <= b.count else { return nil }
        total += len + 1
        if total > 253 { return nil }
        guard let l = String(bytes: b[i..<i + len], encoding: .ascii) else { return nil }
        labels.append(l)
        i += len
    }
    guard i == b.count else { return nil }
    return SrvRecord(priority: prio, weight: weight, port: port, target: labels.isEmpty ? "." : labels.joined(separator: "."))
}

/// RFC 2782 order: by priority, inside one priority by weighted random choice. `randBelow(n)` returns 0..<n.
public func orderSrv(_ records: [SrvRecord], randBelow: (UInt32) -> UInt32) -> [SrvRecord] {
    // stable sort by priority
    let sorted = records.enumerated().sorted { ($0.element.priority, $0.offset) < ($1.element.priority, $1.offset) }.map(\.element)
    var out: [SrvRecord] = []
    var i = 0
    while i < sorted.count {
        let prio = sorted[i].priority
        var end = i
        while end < sorted.count, sorted[end].priority == prio { end += 1 }
        var group = Array(sorted[i..<end])
        // weight 0 first (stable)
        group = group.enumerated().sorted { (($0.element.weight != 0 ? 1 : 0), $0.offset) < (($1.element.weight != 0 ? 1 : 0), $1.offset) }.map(\.element)
        while !group.isEmpty {
            let total = group.reduce(UInt32(0)) { $0 + UInt32($1.weight) }
            let pick = min(randBelow(total + 1), total)
            var running: UInt32 = 0
            var chosen = group.count - 1
            for (idx, r) in group.enumerated() {
                running += UInt32(r.weight)
                if running >= pick { chosen = idx; break }
            }
            out.append(group.remove(at: chosen))
        }
        i = end
    }
    return out
}

/// Turns the SRV answers of one domain into validated servers (at most `maxSrvServers`). A single record with
/// target "." means "service not available here" and gives no server.
public func srvCandidates(records: [SrvRecord], randBelow: (UInt32) -> UInt32) -> [ServerCandidate] {
    let recs = Array(records.prefix(maxSrvRecords))
    if recs.count == 1, recs[0].target == "." { return [] }
    var out: [ServerCandidate] = []
    for r in orderSrv(recs, randBelow: randBelow) {
        var t = r.target
        if t.hasSuffix(".") { t.removeLast() }
        guard r.port != 0, let (addr, _) = networkServer("\(t):\(r.port)", tls: nil) else { continue }
        if !out.contains(where: { $0.address == addr }), out.count < maxSrvServers {
            out.append(ServerCandidate(address: addr, source: .dnsSrv))
        }
    }
    return out
}

// MARK: announcement

public struct AnnouncedReceiver: Equatable, Sendable {
    public var instance: String
    public var name: String
    /// The server it announced, normalised, always with scheme; nil when it announced nothing usable.
    public var announcedServer: String?
}

/// Builds a receiver entry from what the network said: DNS-SD instance name and TXT pairs (`v`, `srv`, `tls`, `name`).
/// Only the first 32 TXT entries are read, the first occurrence of a key wins (`tls` is true if any entry says 1).
public func announcedReceiver(instanceName: String, txt: [(String, String)]) -> AnnouncedReceiver? {
    let instance = cleanText(instanceName, maxChars: 63)
    if instance.isEmpty { return nil }
    var version: String?, srv: String?, name: String?
    var tls = false
    for (k, v) in txt.prefix(32) {
        switch k {
        case "v": if version == nil { version = v }
        case "srv": if srv == nil { srv = v }
        case "tls": tls = tls || v == "1"
        case "name": if name == nil { name = v }
        default: break
        }
    }
    var announced: String?
    if let s = srv, version == nil || version == "1" { announced = networkServer(s, tls: tls)?.address }
    let n = name.map { cleanText($0, maxChars: maxNameChars) }.flatMap { $0.isEmpty ? nil : $0 } ?? instance
    return AnnouncedReceiver(instance: instance, name: n, announcedServer: announced)
}

// MARK: choosing

/// Converts a candidate to a Relay. Candidates with a scheme (`ws://`, `wss://`) keep it; a bare `host:port` follows
/// the relay rule (ws for local hosts, else wss). nil if the relay rules refuse it.
public func relay(for c: ServerCandidate) -> Relay? {
    parseRelay(c.address).relay(source: c.source)
}

/// What the UI shows and uses.
public struct RelayChoice: Equatable, Sendable {
    public var relays: [Relay]
    public var selected: Relay?
    public var locked: Bool
    public var policyUnusable: Bool
}

/// Builds the list to show and the pre-selection. The pre-fill is NEVER an announced server (any device in the
/// local network can announce one, and the room code would be sent to it): it is `lastUsed` when that is still in
/// the list (or was typed by the person and confirmed), else the first entry that is not an announcement.
/// Under a lock only the managed servers are offered and `lastUsed` is used only if it is one of them.
public func chooseRelay(merged: MergedServers, lastUsedWsUrl: String?, customConfirmed: Relay? = nil) -> RelayChoice {
    var relays = merged.servers.compactMap { relay(for: $0) }
    var seen = Set<String>()
    relays = relays.filter { seen.insert($0.wsUrl).inserted }
    var selected: Relay?
    if let last = lastUsedWsUrl {
        selected = relays.first(where: { $0.wsUrl == last && $0.source != .displayAnnouncement })
        if selected == nil, !merged.locked, let c = customConfirmed, c.wsUrl == last { selected = c }
    }
    if selected == nil { selected = relays.first(where: { $0.source != .displayAnnouncement }) }
    return RelayChoice(relays: relays, selected: selected, locked: merged.locked, policyUnusable: merged.policyUnusable)
}

/// A relay from an announcement or typed by the person must be confirmed once before any connection is made
/// (the web app asks the same for a relay named only by a link). `confirmed` holds wsUrls the person accepted before.
public func needsRelayConfirmation(_ r: Relay, confirmed: Set<String>) -> Bool {
    (r.source == .displayAnnouncement || r.source == .user) && !confirmed.contains(r.wsUrl)
}
