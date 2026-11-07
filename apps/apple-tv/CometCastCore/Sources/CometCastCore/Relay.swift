// SPDX-License-Identifier: GPL-3.0-or-later
//
// Which relay (signalling server) the app talks to: validation and normalisation of a relay address, the same
// rules as apps/web/src/core/relay.ts (`parseRelay`). Differences (stricter): non-ASCII and punycode host
// names are refused (see LinkURL.swift). The order of sources is in ServerList.swift.

import Foundation

public enum RelaySource: String, Equatable, Sendable {
    /// Set by an administrator (managed app configuration / MDM).
    case mdm
    /// DNS-SRV `_cometcast._tcp`.
    case dnsSrv = "dns-srv"
    /// Announced by a display device on the local network (not verified).
    case displayAnnouncement = "display-announcement"
    /// Build default (Info.plist key `CCDefaultRelay`) or the built-in list.
    case defaultList = "default-list"
    /// Typed by the person at this device.
    case user
}

public struct Relay: Equatable, Sendable {
    /// ws:// or wss:// URL of the signalling endpoint, always ending in `/ws`.
    public var wsUrl: String
    /// http(s) origin of the same server (ALTCHA and /ice-servers requests go here).
    public var httpBase: String
    /// host[:port] for display. Never the path or anything else.
    public var host: String
    /// ws:// to a local/private address (no TLS).
    public var insecure: Bool
    public var source: RelaySource

    public init(wsUrl: String, httpBase: String, host: String, insecure: Bool, source: RelaySource) {
        self.wsUrl = wsUrl; self.httpBase = httpBase; self.host = host; self.insecure = insecure; self.source = source
    }
}

public enum RelayErrorCode: String, Equatable, Sendable { case empty, invalid, insecure, credentials, path }

public enum ParsedRelay: Equatable, Sendable {
    case ok(wsUrl: String, httpBase: String, host: String, insecure: Bool)
    case failure(RelayErrorCode)

    public func relay(source: RelaySource) -> Relay? {
        if case .ok(let w, let h, let ho, let i) = self { return Relay(wsUrl: w, httpBase: h, host: ho, insecure: i, source: source) }
        return nil
    }
}

private let maxRelayInput = 300

/// localhost, single-label names, .local/.lan/.home.arpa and private/loopback/link-local addresses.
public func isLocalOrPrivateHost(_ hostname: String) -> Bool {
    var h = hostname.lowercased()
    if h.hasPrefix("[") { h.removeFirst() }
    if h.hasSuffix("]") { h.removeLast() }
    if h == "localhost" || h.hasSuffix(".localhost") || h.hasSuffix(".local") || h.hasSuffix(".lan") || h.hasSuffix(".home.arpa") { return true }
    let p = h.split(separator: ".", omittingEmptySubsequences: false)
    if p.count == 4, p.allSatisfy({ !$0.isEmpty && $0.count <= 3 && $0.allSatisfy { $0.isASCII && $0.isNumber } }) {
        let a = Int(p[0])!, b = Int(p[1])!
        return a == 127 || a == 10 || (a == 172 && (16...31).contains(b)) || (a == 192 && b == 168) || (a == 169 && b == 254)
    }
    if h.contains(":") {
        if h == "::1" { return true }
        let l = Array(h.utf8)
        if l.count >= 5, l[4] == UInt8(ascii: ":"), l[0] == UInt8(ascii: "f"), (l[1] == UInt8(ascii: "c") || l[1] == UInt8(ascii: "d")) { return true }
        if l.count >= 5, l[4] == UInt8(ascii: ":"), l[0] == UInt8(ascii: "f"), l[1] == UInt8(ascii: "e"), [UInt8(ascii: "8"), UInt8(ascii: "9"), UInt8(ascii: "a"), UInt8(ascii: "b")].contains(l[2]) { return true }
        return false
    }
    // A name without a dot is an intranet name (never a public DNS name).
    return !h.contains(".")
}

private func hasSpaceOrControl(_ s: String) -> Bool {
    s.unicodeScalars.contains { u in
        u.properties.isWhitespace || u.value <= 0x1F || u.value == 0x7F || u == "\\"
    }
}

/// Validates and normalises a relay given by a person, a file or the network.
/// - `wss://host[:port][/ws]` always; `ws://` only for local/private hosts (LAN, development).
/// - A bare `host[:port]` means wss, except for local/private hosts (ws).
/// - `https://` and `http://` are accepted as spellings of wss/ws unless `strictWs`.
/// - No user info, query or fragment; the path may only be empty or `/ws`.
public func parseRelay(_ input: String, strictWs: Bool = false) -> ParsedRelay {
    let text = jsTrim(input)
    if text.isEmpty { return .failure(.empty) }
    if text.utf16.count > maxRelayInput || hasSpaceOrControl(text) { return .failure(.invalid) }
    var withScheme = text
    if !hasScheme(text) {
        if strictWs { return .failure(.invalid) }
        guard isBareHostPort(text) else { return .failure(.invalid) }
        var hostPart = text
        if hostPart.hasPrefix("[") { if let c = hostPart.firstIndex(of: "]") { hostPart = String(hostPart[...c]) } }
        else if let c = hostPart.lastIndex(of: ":") { hostPart = String(hostPart[..<c]) }
        let plain = isLocalOrPrivateHost(hostPart)
        withScheme = "\(plain ? "ws" : "wss")://\(text)"
    }
    // Map ws/wss/http/https to http/https: all four are "special" schemes with the same parsing rules.
    guard let colon = withScheme.range(of: "://") else { return .failure(.invalid) }
    let scheme = withScheme[..<colon.lowerBound].lowercased()
    let secure: Bool
    switch scheme {
    case "wss": secure = true
    case "ws": secure = false
    case "https", "http":
        if strictWs { return .failure(.invalid) }
        secure = scheme == "https"
    default: return .failure(.invalid)
    }
    let asHTTP = (secure ? "https" : "http") + String(withScheme[colon.lowerBound...])
    guard let r = parseHTTPURLDetailed(asHTTP) else { return .failure(.invalid) }
    if r.credentials { return .failure(.credentials) }
    if r.url.query != nil || r.url.fragment != nil || withScheme.contains("?") || withScheme.contains("#") { return .failure(.path) }
    guard ["/", "/ws", "/ws/"].contains(r.url.path) else { return .failure(.path) }
    if r.url.host.isEmpty { return .failure(.invalid) }
    if !secure && !isLocalOrPrivateHost(r.url.host) { return .failure(.insecure) }
    let hp = r.url.hostAndPort
    return .ok(wsUrl: "\(secure ? "wss" : "ws")://\(hp)/ws", httpBase: "\(secure ? "https" : "http")://\(hp)", host: hp, insecure: !secure)
}

private func hasScheme(_ s: String) -> Bool {
    // ^[A-Za-z][A-Za-z0-9+.-]*://
    var it = s.unicodeScalars.makeIterator()
    guard let f = it.next(), f.isASCII, Character(f).isLetter else { return false }
    var rest = String.UnicodeScalarView()
    while let c = it.next() { rest.append(c) }
    var i = rest.startIndex
    while i < rest.endIndex, rest[i].isASCII, Character(rest[i]).isLetter || Character(rest[i]).isNumber || "+.-".unicodeScalars.contains(rest[i]) { i = rest.index(after: i) }
    return String(rest[i...]).hasPrefix("://")
}

private func isBareHostPort(_ s: String) -> Bool {
    // ^(\[[0-9A-Fa-f:.]+\]|[A-Za-z0-9.-]+)(:\d{1,5})?$
    var host = Substring(s)
    var port: Substring?
    if s.hasPrefix("[") {
        guard let close = s.firstIndex(of: "]") else { return false }
        let inner = s[s.index(after: s.startIndex)..<close]
        guard !inner.isEmpty, inner.allSatisfy({ $0.isASCII && ($0.isHexDigit || $0 == ":" || $0 == ".") }) else { return false }
        host = s[...close]
        let after = s[s.index(after: close)...]
        if !after.isEmpty { guard after.hasPrefix(":") else { return false }; port = after.dropFirst() }
    } else {
        if let c = s.lastIndex(of: ":") { host = s[..<c]; port = s[s.index(after: c)...] }
        guard !host.isEmpty, host.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "." || $0 == "-") }) else { return false }
    }
    if let port { guard (1...5).contains(port.count), port.allSatisfy({ $0.isASCII && $0.isNumber }) else { return false } }
    return true
}
