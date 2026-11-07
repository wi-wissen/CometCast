// SPDX-License-Identifier: GPL-3.0-or-later
//
// Text the display shows that is decided by rules, not by layout: the room name, the name of a person in the
// request dialog and the name tag, the join address `{dom}` and the QR URL, the security code, the shortened
// file name of the transfer pill. Ports of crates/cc-discovery (`clean_room_name`), apps/web/src/core
// (`fitName.ts`, `codeFormat.ts`) and the rules of docs/design/first-run-and-display.md 2, 4.2, 4.4, 4.5.

import Foundation

// MARK: room name (spec 13.2, 14.8)

/// Longest room name in characters (spec 13.2, 14.3).
public let maxRoomNameChars = 32

private func isInvisibleFormat(_ v: UInt32) -> Bool {
    switch v {
    case 0x00AD, 0x0600...0x0605, 0x061C, 0x06DD, 0x070F, 0x0890...0x0891, 0x08E2, 0x180E, 0x200B...0x200F,
         0x2028...0x202E, 0x2060...0x2064, 0x2066...0x206F, 0xFEFF, 0xFFF9...0xFFFB, 0x110BD, 0x110CD,
         0x13430...0x1343F, 0x1BCA0...0x1BCA3, 0x1D173...0x1D17A, 0xE0001, 0xE0020...0xE007F: return true
    default: return false
    }
}

private func isPrivateUse(_ v: UInt32) -> Bool {
    (0xE000...0xF8FF).contains(v) || (0xF0000...0xFFFFD).contains(v) || (0x100000...0x10FFFD).contains(v)
}

private func isControl(_ v: UInt32) -> Bool { v <= 0x1F || (0x7F...0x9F).contains(v) }

/// Cleans a room name like `clean_room_name` of cc-discovery: control, format (bidi, zero-width), invisible and
/// private-use characters are removed, white space runs become one blank, the result is trimmed and cut at 32
/// characters. May be empty.
public func cleanRoomName(_ s: String) -> String {
    var out: [Unicode.Scalar] = []
    var pendingBlank = false
    for c in s.unicodeScalars {
        if c.properties.isWhitespace || c == "\t" || c == "\n" || c == "\r" {
            pendingBlank = !out.isEmpty
            continue
        }
        if isControl(c.value) || isInvisibleFormat(c.value) || isPrivateUse(c.value) { continue }
        if pendingBlank { out.append(" "); pendingBlank = false }
        out.append(c)
    }
    var cut = Array(out.prefix(maxRoomNameChars))
    while let l = cut.last, l.properties.isWhitespace { cut.removeLast() }
    var r = String.UnicodeScalarView()
    r.append(contentsOf: cut)
    return String(r)
}

// MARK: names of people (spec 4.4)

/// A person's name for the request dialog, the name tag and the toasts: control and invisible characters are
/// stripped, white space collapsed, cut to 18 characters plus "…". `nil` for an empty name: the screens then show
/// `common.noName` (spec 4.4). The accessible label gets the full name (`fullPersonName`).
public func shortPersonName(_ raw: String, unnamed: Bool = false) -> String? {
    if unnamed { return nil }
    guard let full = sanitizedDisplayNameOrNil(raw) else { return nil }
    let chars = Array(full.unicodeScalars)
    if chars.count <= 18 { return full }
    var r = String.UnicodeScalarView()
    r.append(contentsOf: chars.prefix(18))
    return String(r) + "…"
}

/// The full cleaned name for accessible labels; `nil` for an empty name.
public func fullPersonName(_ raw: String, unnamed: Bool = false) -> String? {
    unnamed ? nil : sanitizedDisplayNameOrNil(raw)
}

// MARK: join address (spec 2 `{dom}`, native-apps 21.6, 4.2 N40)

/// The host of the built-in relay (`defaultServers`).
public let builtInRelayHost = "relay.cometcast.de"
/// Where a browser reaches the web app for the built-in relay (decision N85).
public let builtInJoinAddress = "cometcast.de/app"

private func splitHostPort(_ hostAndPort: String) -> (host: String, port: Int?) {
    if hostAndPort.hasPrefix("[") {
        guard let close = hostAndPort.firstIndex(of: "]") else { return (hostAndPort, nil) }
        let host = String(hostAndPort[...close])
        let rest = hostAndPort[hostAndPort.index(after: close)...]
        return (host, rest.hasPrefix(":") ? Int(rest.dropFirst()) : nil)
    }
    if let c = hostAndPort.lastIndex(of: ":") {
        return (String(hostAndPort[..<c]), Int(hostAndPort[hostAndPort.index(after: c)...]))
    }
    return (hostAndPort, nil)
}

/// Is `host` an IP literal (IPv4 dotted or IPv6 in brackets)?
public func isIPLiteral(_ host: String) -> Bool {
    if host.hasPrefix("[") { return true }
    return parseStrictIPv4(host) != nil
}

/// `{dom}`: the join address of a server. The built-in relay is `cometcast.de/app`; every other server is its
/// host, with `:port` only when the port is not 443 (decision N12).
public func joinAddress(host hostAndPort: String) -> String {
    let (host, port) = splitHostPort(hostAndPort)
    if host.lowercased() == builtInRelayHost, port == nil || port == 443 { return builtInJoinAddress }
    if let port, port != 443 { return "\(host):\(port)" }
    return host
}

/// What the idle screen shows as address (spec 4.2).
public struct IdleAddress: Equatable, Sendable {
    /// `{dom}`, 58/700 on step 1.
    public var dom: String
    /// `idle.altAddress`: the IPv4 address of the in-house box, only when `dom` is a host name (decision N40).
    public var altIp: String?
    /// The host of the QR URL: the box IP when there is one, else `dom`.
    public var qrHost: String

    public func qrUrl(code: String) -> String { "https://\(qrHost)/\(code)" }
}

/// `boxIPv4`: the address the box announcement resolved to (nil when unknown or when the server did not come from
/// a box announcement). The port of the relay is kept on both.
public func idleAddress(relayHost hostAndPort: String, fromBox: Bool, boxIPv4: String?) -> IdleAddress {
    let dom = joinAddress(host: hostAndPort)
    let (host, port) = splitHostPort(hostAndPort)
    guard fromBox, !isIPLiteral(host), let ip = boxIPv4, parseStrictIPv4(ip) != nil else {
        return IdleAddress(dom: dom, altIp: nil, qrHost: dom)
    }
    let withPort = (port != nil && port != 443) ? "\(ip):\(port!)" : ip
    return IdleAddress(dom: dom, altIp: withPort, qrHost: withPort)
}

/// The QR URL for a normal server: `https://{dom}/{CODE}` (code without spaces; spec 4.2).
public func qrUrl(dom: String, code: String) -> String { "https://\(dom)/\(code)" }

// MARK: security code (spec 4.5, decision N34)

/// "123456" to "123 456": one ordinary space (set in the mono font by the view).
public func groupedSas(_ sas: String) -> String {
    let digits = Array(sas.filter { !$0.isWhitespace })
    guard digits.count == 6 else { return sas }
    return String(digits[0..<3]) + " " + String(digits[3..<6])
}

// MARK: transfer pill (spec 3.5, decision 13.63)

/// Shortens a long file name in the middle with `…`, keeping its start and its extension (port of `shortenMiddle`
/// in apps/web/src/core/fitName.ts). `keep` is the number of characters that stay.
public func shortenMiddle(_ name: String, keep: Int) -> String {
    let chars = Array(name)
    if keep >= chars.count { return name }
    let n = max(4, keep)
    var extLen = 0
    if let dot = name.lastIndex(of: "."), dot != name.startIndex {
        let ext = name[dot...]
        if ext.utf16.count <= 8 { extLen = ext.count }
    }
    let tail = min(extLen + 4, n - 3)
    let head = n - tail
    return String(chars[0..<head]) + "…" + String(chars[(chars.count - tail)...])
}

/// The percentage of a transfer, 0 to 100 (never above).
public func transferPercent(received: Int, size: Int) -> Int {
    guard size > 0 else { return 0 }
    return min(100, max(0, Int((Double(received) / Double(size) * 100).rounded(.down))))
}

// MARK: copy placeholders

/// Replaces `{name}` placeholders of a catalog string. Values are inserted as given (the caller has already
/// quoted names with `common.quoted`). A placeholder without a value stays as it is.
public func fillPlaceholders(_ template: String, _ values: [String: String]) -> String {
    var out = ""
    var i = template.startIndex
    while i < template.endIndex {
        if template[i] == "{", let close = template[i...].firstIndex(of: "}") {
            let key = String(template[template.index(after: i)..<close])
            if let v = values[key] { out += v; i = template.index(after: close); continue }
        }
        out.append(template[i])
        i = template.index(after: i)
    }
    return out
}
