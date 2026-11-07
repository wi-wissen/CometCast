// SPDX-License-Identifier: GPL-3.0-or-later
//
// A small strict JSON parser and writer. Why not JSONSerialization: its behaviour for booleans,
// integral numbers and lone surrogates differs between Apple platforms and Linux, and the parser rules of
// the content channel must be the same as the web app's `JSON.parse` + checks (apps/web/src/core/control.ts).
// Differences from JavaScript's JSON.parse that are known and accepted:
//   * nesting deeper than `maxDepth` (1000) is refused (JS has no limit; messages are at most 8192 characters),
//   * a lone UTF-16 surrogate escape (\ud800) becomes U+FFFD because a Swift String cannot hold it.

import Foundation

public enum JSONValue: Equatable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    public var stringValue: String? { if case .string(let s) = self { return s }; return nil }
    public var objectValue: [String: JSONValue]? { if case .object(let o) = self { return o }; return nil }
    public var arrayValue: [JSONValue]? { if case .array(let a) = self { return a }; return nil }
    public var numberValue: Double? { if case .number(let n) = self { return n }; return nil }
    public var boolValue: Bool? { if case .bool(let b) = self { return b }; return nil }

    /// A number with an integral value (JavaScript `Number.isInteger`).
    public var integerValue: Double? {
        if case .number(let n) = self, n.isFinite, n == n.rounded(.towardZero) { return n }
        return nil
    }
}

public struct JSONParseError: Error, Equatable {}

public enum JSON {
    public static let maxDepth = 1000

    public static func parse(_ text: String) -> JSONValue? {
        var p = Parser(bytes: Array(text.utf8))
        do {
            p.skipWS()
            let v = try p.value(depth: 0)
            p.skipWS()
            return p.i == p.bytes.count ? v : nil
        } catch { return nil }
    }

    /// Canonical text: object keys sorted by UTF-8 bytes, no spaces.
    public static func serialize(_ v: JSONValue) -> String {
        var out = ""
        write(v, &out)
        return out
    }

    private static func write(_ v: JSONValue, _ out: inout String) {
        switch v {
        case .null: out += "null"
        case .bool(let b): out += b ? "true" : "false"
        case .number(let n): out += formatNumber(n)
        case .string(let s): writeString(s, &out)
        case .array(let a):
            out += "["
            for (i, e) in a.enumerated() { if i > 0 { out += "," }; write(e, &out) }
            out += "]"
        case .object(let o):
            out += "{"
            let keys = o.keys.sorted { Array($0.utf8).lexicographicallyPrecedes(Array($1.utf8)) }
            for (i, k) in keys.enumerated() {
                if i > 0 { out += "," }
                writeString(k, &out)
                out += ":"
                write(o[k]!, &out)
            }
            out += "}"
        }
    }

    static func formatNumber(_ n: Double) -> String {
        if n.isFinite, n == n.rounded(.towardZero), abs(n) < 1e15 { return String(Int64(n)) }
        return n.isFinite ? "\(n)" : "null"
    }

    private static func writeString(_ s: String, _ out: inout String) {
        out += "\""
        for u in s.unicodeScalars {
            switch u {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if u.value < 0x20 { out += String(format: "\\u%04x", u.value) } else { out.unicodeScalars.append(u) }
            }
        }
        out += "\""
    }

    private struct Parser {
        let bytes: [UInt8]
        var i = 0

        init(bytes: [UInt8]) { self.bytes = bytes }

        mutating func skipWS() {
            while i < bytes.count, bytes[i] == 0x20 || bytes[i] == 0x09 || bytes[i] == 0x0a || bytes[i] == 0x0d { i += 1 }
        }

        mutating func value(depth: Int) throws -> JSONValue {
            if depth > JSON.maxDepth { throw JSONParseError() }
            guard i < bytes.count else { throw JSONParseError() }
            switch bytes[i] {
            case UInt8(ascii: "{"):
                i += 1
                var o: [String: JSONValue] = [:]
                skipWS()
                if i < bytes.count, bytes[i] == UInt8(ascii: "}") { i += 1; return .object(o) }
                while true {
                    skipWS()
                    guard i < bytes.count, bytes[i] == UInt8(ascii: "\"") else { throw JSONParseError() }
                    let k = try string()
                    skipWS()
                    guard i < bytes.count, bytes[i] == UInt8(ascii: ":") else { throw JSONParseError() }
                    i += 1
                    skipWS()
                    o[k] = try value(depth: depth + 1) // a repeated key: the last one wins, like JavaScript
                    skipWS()
                    guard i < bytes.count else { throw JSONParseError() }
                    if bytes[i] == UInt8(ascii: ",") { i += 1; continue }
                    if bytes[i] == UInt8(ascii: "}") { i += 1; return .object(o) }
                    throw JSONParseError()
                }
            case UInt8(ascii: "["):
                i += 1
                var a: [JSONValue] = []
                skipWS()
                if i < bytes.count, bytes[i] == UInt8(ascii: "]") { i += 1; return .array(a) }
                while true {
                    skipWS()
                    a.append(try value(depth: depth + 1))
                    skipWS()
                    guard i < bytes.count else { throw JSONParseError() }
                    if bytes[i] == UInt8(ascii: ",") { i += 1; continue }
                    if bytes[i] == UInt8(ascii: "]") { i += 1; return .array(a) }
                    throw JSONParseError()
                }
            case UInt8(ascii: "\""):
                return .string(try string())
            case UInt8(ascii: "t"): try literal("true"); return .bool(true)
            case UInt8(ascii: "f"): try literal("false"); return .bool(false)
            case UInt8(ascii: "n"): try literal("null"); return .null
            default:
                return try number()
            }
        }

        mutating func literal(_ s: String) throws {
            let u = Array(s.utf8)
            guard i + u.count <= bytes.count, Array(bytes[i..<i + u.count]) == u else { throw JSONParseError() }
            i += u.count
        }

        mutating func number() throws -> JSONValue {
            let start = i
            if i < bytes.count, bytes[i] == UInt8(ascii: "-") { i += 1 }
            guard i < bytes.count else { throw JSONParseError() }
            if bytes[i] == UInt8(ascii: "0") { i += 1 }
            else if bytes[i] >= 0x31 && bytes[i] <= 0x39 { while i < bytes.count, isDigit(bytes[i]) { i += 1 } }
            else { throw JSONParseError() }
            if i < bytes.count, bytes[i] == UInt8(ascii: ".") {
                i += 1
                guard i < bytes.count, isDigit(bytes[i]) else { throw JSONParseError() }
                while i < bytes.count, isDigit(bytes[i]) { i += 1 }
            }
            if i < bytes.count, bytes[i] == UInt8(ascii: "e") || bytes[i] == UInt8(ascii: "E") {
                i += 1
                if i < bytes.count, bytes[i] == UInt8(ascii: "+") || bytes[i] == UInt8(ascii: "-") { i += 1 }
                guard i < bytes.count, isDigit(bytes[i]) else { throw JSONParseError() }
                while i < bytes.count, isDigit(bytes[i]) { i += 1 }
            }
            guard let s = String(bytes: bytes[start..<i], encoding: .utf8), let d = Double(s) else { throw JSONParseError() }
            return .number(d == 0 ? 0 : d) // "-0" is 0 for every check here
        }

        func isDigit(_ b: UInt8) -> Bool { b >= 0x30 && b <= 0x39 }

        mutating func hex4() throws -> UInt32 {
            guard i + 4 <= bytes.count else { throw JSONParseError() }
            var v: UInt32 = 0
            for _ in 0..<4 {
                let c = bytes[i]
                let d: UInt32
                switch c {
                case 0x30...0x39: d = UInt32(c - 0x30)
                case 0x41...0x46: d = UInt32(c - 0x41 + 10)
                case 0x61...0x66: d = UInt32(c - 0x61 + 10)
                default: throw JSONParseError()
                }
                v = v << 4 | d
                i += 1
            }
            return v
        }

        mutating func string() throws -> String {
            i += 1 // opening quote
            var out = String.UnicodeScalarView()
            var chunkStart = i
            func flush(_ end: Int, _ p: inout Parser) throws {
                if end > chunkStart {
                    guard let s = String(bytes: p.bytes[chunkStart..<end], encoding: .utf8) else { throw JSONParseError() }
                    out.append(contentsOf: s.unicodeScalars)
                }
            }
            while i < bytes.count {
                let c = bytes[i]
                if c == UInt8(ascii: "\"") {
                    try flush(i, &self)
                    i += 1
                    return String(out)
                }
                if c < 0x20 { throw JSONParseError() }
                if c == UInt8(ascii: "\\") {
                    try flush(i, &self)
                    i += 1
                    guard i < bytes.count else { throw JSONParseError() }
                    let e = bytes[i]
                    i += 1
                    switch e {
                    case UInt8(ascii: "\""): out.append("\"")
                    case UInt8(ascii: "\\"): out.append("\\")
                    case UInt8(ascii: "/"): out.append("/")
                    case UInt8(ascii: "b"): out.append("\u{08}")
                    case UInt8(ascii: "f"): out.append("\u{0c}")
                    case UInt8(ascii: "n"): out.append("\n")
                    case UInt8(ascii: "r"): out.append("\r")
                    case UInt8(ascii: "t"): out.append("\t")
                    case UInt8(ascii: "u"):
                        let u = try hex4()
                        if u >= 0xD800 && u <= 0xDBFF {
                            if i + 1 < bytes.count, bytes[i] == UInt8(ascii: "\\"), bytes[i + 1] == UInt8(ascii: "u") {
                                let save = i
                                i += 2
                                let lo = try hex4()
                                if lo >= 0xDC00 && lo <= 0xDFFF {
                                    out.append(Unicode.Scalar(0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00))!)
                                } else {
                                    out.append("\u{FFFD}")
                                    i = save // reprocess the second escape on its own
                                }
                            } else { out.append("\u{FFFD}") }
                        } else if u >= 0xDC00 && u <= 0xDFFF {
                            out.append("\u{FFFD}")
                        } else {
                            out.append(Unicode.Scalar(u)!)
                        }
                    default: throw JSONParseError()
                    }
                    chunkStart = i
                    continue
                }
                i += 1
            }
            throw JSONParseError()
        }
    }
}
