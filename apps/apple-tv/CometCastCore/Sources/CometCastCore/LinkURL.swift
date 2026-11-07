// SPDX-License-Identifier: GPL-3.0-or-later
//
// http(s) URL validation and normalisation, the same decisions as `validateLinkUrl` in
// apps/web/src/core/control.ts, which uses the browser's WHATWG `URL` class.
//
// Foundation's `URL` is NOT used: on Apple platforms it follows RFC 3986 with exceptions and on Linux it
// differs again, so the same link could be accepted on one platform and refused on another. This is a small
// implementation of the WHATWG algorithm for the two special schemes http and https. It is STRICTER than the
// browser in these cases (the link is refused, never shown differently); each is marked in the shared
// fixtures (tests/fixtures/link-url.json, field `expect_apple_tv`):
//   * a non-ASCII host name (needs IDNA/punycode, not implemented),
//   * an "xn--" label in the host (punycode validation not implemented).
// Everything else is meant to give exactly the browser's `href`. UNCERTAIN: verified against Node's URL
// (WHATWG implementation "ada") with the shared fixtures only, not against a browser or against newer
// versions of the standard.

import Foundation

public let maxLinkLength = 2048

public struct HTTPURL: Equatable, Sendable {
    public var scheme: String      // "http" or "https"
    public var host: String        // lower case, IPv4 in dotted form
    public var port: Int?          // nil for the default port
    public var path: String        // always starts with "/"
    public var query: String?      // without "?"
    public var fragment: String?   // without "#"

    public var href: String {
        var s = "\(scheme)://\(host)"
        if let port { s += ":\(port)" }
        s += path
        if let query { s += "?\(query)" }
        if let fragment { s += "#\(fragment)" }
        return s
    }
    /// `URL.host` of the browser: host plus ":port" when the port is not the default.
    public var hostAndPort: String { port.map { "\(host):\($0)" } ?? host }
}

private func isJSWhitespace(_ u: Unicode.Scalar) -> Bool {
    switch u.value {
    case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000, 0xFEFF: return true
    default: return false
    }
}

/// JavaScript `String.prototype.trim`.
func jsTrim(_ s: String) -> String {
    var v = Array(s.unicodeScalars)
    while let l = v.last, isJSWhitespace(l) { v.removeLast() }
    var start = 0
    while start < v.count, isJSWhitespace(v[start]) { start += 1 }
    var out = String.UnicodeScalarView()
    out.append(contentsOf: v[start...])
    return String(out)
}

/// Parses and normalises an absolute http/https URL (WHATWG rules, see the notes above). `nil` for anything else,
/// including a URL with a user name or password.
public func parseHTTPURL(_ input: String) -> HTTPURL? {
    guard let r = parseHTTPURLDetailed(input), !r.credentials else { return nil }
    return r.url
}

/// Like `parseHTTPURL`, but a URL with credentials is returned with `credentials == true` (for relay addresses,
/// which report that case separately).
func parseHTTPURLDetailed(_ input: String) -> (url: HTTPURL, credentials: Bool)? {
    let scalars = Array(input.unicodeScalars)
    var i = 0
    // scheme
    guard i < scalars.count, isASCIIAlpha(scalars[i]) else { return nil }
    var scheme = ""
    while i < scalars.count, isASCIIAlpha(scalars[i]) || isASCIIDigit(scalars[i]) || "+-.".unicodeScalars.contains(scalars[i]) {
        scheme.unicodeScalars.append(scalars[i]); i += 1
    }
    guard i < scalars.count, scalars[i] == ":" else { return nil }
    i += 1
    scheme = scheme.lowercased()
    guard scheme == "http" || scheme == "https" else { return nil }
    // slashes: any number of "/" or "\" (also none) for special schemes
    while i < scalars.count, scalars[i] == "/" || scalars[i] == "\\" { i += 1 }
    // authority
    var auth = String.UnicodeScalarView()
    while i < scalars.count, !("/\\?#".unicodeScalars.contains(scalars[i])) { auth.append(scalars[i]); i += 1 }
    var authority = Array(String(auth).unicodeScalars)
    var hasCredentials = false
    if let at = authority.lastIndex(of: "@") {
        let cred = String(String.UnicodeScalarView(authority[..<at]))
        // An empty user name and password (`http://@host`, `http://:@host`) is dropped by the browser.
        hasCredentials = !(cred == "" || cred == ":")
        authority = Array(authority[(at + 1)...])
        if authority.isEmpty { return nil }
    }
    // host and port; an IPv6 literal is in brackets
    var hostScalars = authority
    var port: Int?
    var portScalars: ArraySlice<Unicode.Scalar>?
    if authority.first == "[" {
        guard let close = authority.firstIndex(of: "]") else { return nil }
        hostScalars = Array(authority[...close])
        let after = authority[(close + 1)...]
        if !after.isEmpty { guard after.first == ":" else { return nil }; portScalars = after.dropFirst() }
    } else if let colon = authority.lastIndex(of: ":") {
        hostScalars = Array(authority[..<colon])
        portScalars = authority[(colon + 1)...]
    }
    if let ps = portScalars, !ps.isEmpty {
        guard ps.allSatisfy({ isASCIIDigit($0) }) else { return nil }
        var v = 0
        for d in ps { v = v * 10 + Int(d.value - 0x30); if v > 65535 { return nil } }
        let def = scheme == "https" ? 443 : 80
        port = v == def ? nil : v
    }
    guard let host = canonicalHost(String(String.UnicodeScalarView(hostScalars))) else { return nil }
    // path, query, fragment
    var pathScalars = String.UnicodeScalarView()
    while i < scalars.count, scalars[i] != "?", scalars[i] != "#" { pathScalars.append(scalars[i]); i += 1 }
    var query: String?
    var fragment: String?
    if i < scalars.count, scalars[i] == "?" {
        i += 1
        var q = String.UnicodeScalarView()
        while i < scalars.count, scalars[i] != "#" { q.append(scalars[i]); i += 1 }
        query = percentEncode(String(q), set: .query)
    }
    if i < scalars.count, scalars[i] == "#" {
        i += 1
        var f = String.UnicodeScalarView()
        while i < scalars.count { f.append(scalars[i]); i += 1 }
        fragment = percentEncode(String(f), set: .fragment)
    }
    let path = normalizePath(String(pathScalars))
    return (HTTPURL(scheme: scheme, host: host, port: port, path: path, query: query, fragment: fragment), hasCredentials)
}

private func isASCIIAlpha(_ u: Unicode.Scalar) -> Bool { (u.value >= 0x41 && u.value <= 0x5A) || (u.value >= 0x61 && u.value <= 0x7A) }
private func isASCIIDigit(_ u: Unicode.Scalar) -> Bool { u.value >= 0x30 && u.value <= 0x39 }

private enum EncodeSet { case path, query, fragment }

private func percentEncode(_ s: String, set: EncodeSet) -> String {
    var out = ""
    for b in s.utf8 {
        let enc: Bool
        switch b {
        case 0x00...0x1F, 0x7F...0xFF: enc = true
        case 0x20, 0x22, 0x3C, 0x3E: enc = true // space " < >
        case 0x23: enc = set == .query || set == .path // #
        case 0x27: enc = set == .query // ' (special schemes)
        case 0x3F: enc = set == .path // ?
        case 0x60: enc = set == .path || set == .fragment // `
        case 0x7B, 0x7D: enc = set == .path // { }
        default: enc = false
        }
        if enc { out += String(format: "%%%02X", b) } else { out.unicodeScalars.append(Unicode.Scalar(b)) }
    }
    return out
}

private func normalizePath(_ raw: String) -> String {
    let replaced = raw.replacingOccurrences(of: "\\", with: "/")
    // The path as written starts with "/" or is empty (a path never starts anywhere else after the authority).
    var segments = replaced.split(separator: "/", omittingEmptySubsequences: false).map(String.init)
    if !segments.isEmpty { segments.removeFirst() } // before the first "/"
    var out: [String] = []
    func isDouble(_ s: String) -> Bool { ["..", ".%2e", "%2e.", "%2e%2e"].contains(s.lowercased()) }
    func isSingle(_ s: String) -> Bool { [".", "%2e"].contains(s.lowercased()) }
    for (idx, seg) in segments.enumerated() {
        let last = idx == segments.count - 1
        if isDouble(seg) {
            if !out.isEmpty { out.removeLast() }
            if last { out.append("") }
        } else if isSingle(seg) {
            if last { out.append("") }
        } else {
            out.append(percentEncode(seg, set: .path))
        }
    }
    return "/" + out.joined(separator: "/")
}

/// Host: lower case ASCII, IPv4 forms normalised, everything unusual refused.
private func canonicalHost(_ raw: String) -> String? {
    if raw.isEmpty { return nil }
    if raw.hasPrefix("[") {
        guard raw.hasSuffix("]"), let b = parseIPv6(String(raw.dropFirst().dropLast())) else { return nil }
        return "[" + whatwgIPv6(b) + "]"
    }
    // The host is percent-decoded first (`exa%6dple.com`); decoding to anything but ASCII is refused.
    var h = raw
    if raw.contains("%") {
        var bytes: [UInt8] = []
        let u = Array(raw.utf8)
        var i = 0
        while i < u.count {
            if u[i] == 0x25, i + 2 < u.count, let v = hexByte(u[i + 1], u[i + 2]) { bytes.append(v); i += 3 }
            else { bytes.append(u[i]); i += 1 }
        }
        guard let d = String(bytes: bytes, encoding: .utf8) else { return nil }
        h = d
    }
    for u in h.unicodeScalars {
        if u.value >= 0x80 { return nil } // IDNA not implemented
        if u.value <= 0x20 || u.value == 0x7F { return nil }
        if "#/:<>?@[\\]^|%".unicodeScalars.contains(u) { return nil }
    }
    let lower = h.lowercased()
    if lower.split(separator: ".", omittingEmptySubsequences: false).contains(where: { $0.hasPrefix("xn--") }) { return nil }
    // "ends in a number": the last non-empty label is all digits or starts with 0x
    var labels = lower.split(separator: ".", omittingEmptySubsequences: false).map(String.init)
    if labels.last == "" && labels.count > 1 { labels.removeLast() }
    if let last = labels.last, !last.isEmpty,
       last.allSatisfy({ $0.isASCII && $0.isNumber }) || (last.hasPrefix("0x") && last.dropFirst(2).allSatisfy({ $0.isHexDigit })) {
        return parseIPv4(lower)
    }
    return lower
}

private func hexByte(_ a: UInt8, _ b: UInt8) -> UInt8? {
    func v(_ c: UInt8) -> UInt8? {
        switch c {
        case 0x30...0x39: return c - 0x30
        case 0x41...0x46: return c - 0x41 + 10
        case 0x61...0x66: return c - 0x61 + 10
        default: return nil
        }
    }
    guard let x = v(a), let y = v(b) else { return nil }
    return x << 4 | y
}

/// WHATWG IPv6 serialisation: lower case hex groups, the first longest run of two or more zero groups becomes "::"
/// (an IPv4-mapped address is written in hex, never in dotted form).
private func whatwgIPv6(_ b: [UInt8]) -> String {
    let g = (0..<8).map { UInt16(b[$0 * 2]) << 8 | UInt16(b[$0 * 2 + 1]) }
    var bestStart = -1, bestLen = 0, i = 0
    while i < 8 {
        if g[i] == 0 {
            var j = i
            while j < 8, g[j] == 0 { j += 1 }
            if j - i > bestLen { bestStart = i; bestLen = j - i }
            i = j
        } else { i += 1 }
    }
    if bestLen < 2 { bestStart = -1 }
    var out = ""
    var k = 0
    while k < 8 {
        if k == bestStart { out += k == 0 ? "::" : ":"; k += bestLen; continue }
        out += String(g[k], radix: 16)
        if k != 7 { out += ":" }
        k += 1
    }
    return out
}

private func parseIPv4(_ s: String) -> String? {
    var parts = s.split(separator: ".", omittingEmptySubsequences: false).map(String.init)
    if parts.last == "" { if parts.count > 1 { parts.removeLast() } }
    if parts.count > 4 { return nil }
    var nums: [UInt64] = []
    for p in parts {
        guard let n = parseIPv4Number(p) else { return nil }
        nums.append(n)
    }
    for n in nums.dropLast() where n > 255 { return nil }
    guard let lastN = nums.last else { return nil }
    if lastN >= UInt64(pow(256.0, Double(5 - nums.count))) { return nil }
    var ipv4 = lastN
    for (i, n) in nums.dropLast().enumerated() { ipv4 += n * UInt64(pow(256.0, Double(3 - i))) }
    return "\((ipv4 >> 24) & 255).\((ipv4 >> 16) & 255).\((ipv4 >> 8) & 255).\(ipv4 & 255)"
}

private func parseIPv4Number(_ s: String) -> UInt64? {
    if s.isEmpty { return nil }
    var str = Substring(s)
    var radix = 10
    if str.count >= 2, str.hasPrefix("0x") || str.hasPrefix("0X") { str = str.dropFirst(2); radix = 16 }
    else if str.count >= 2, str.hasPrefix("0") { str = str.dropFirst(1); radix = 8 }
    if str.isEmpty { return 0 }
    guard str.allSatisfy({ $0.isASCII && $0.isHexDigit }) else { return nil }
    return UInt64(str, radix: radix) // nil on overflow or a digit outside the radix
}

// MARK: validation as in control.ts

private func checkLinkURL(_ input: String) -> String? {
    let s = jsTrim(input)
    if s.isEmpty || s.utf16.count > maxLinkLength { return nil }
    // Control characters, DEL, C1 and spaces are refused up front (the URL parser would silently drop some).
    for u in s.unicodeScalars where u.value <= 0x20 || (u.value >= 0x7F && u.value <= 0x9F) { return nil }
    guard let u = parseHTTPURL(s) else { return nil }
    let href = u.href
    if href.utf16.count > maxLinkLength { return nil }
    return href
}

/// Accepts only absolute http/https URLs without credentials. Returns the normalised href, or nil.
/// Idempotent by construction: what does not survive a second pass is refused.
public func validateLinkUrl(_ input: String) -> String? {
    guard let n = checkLinkURL(input) else { return nil }
    return checkLinkURL(n) == n ? n : nil
}
