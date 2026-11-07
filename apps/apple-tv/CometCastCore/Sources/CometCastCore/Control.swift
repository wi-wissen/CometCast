// SPDX-License-Identifier: GPL-3.0-or-later
//
// Content channel (WebRTC DataChannel "control"): message parser, limits and chunk reassembly.
// A port of apps/web/src/core/control.ts (receiver side only; Apple TV never sends). The limits and
// rules must stay equal to the web client's and to crates/cc-files; the shared fixtures in
// tests/fixtures/ check this on both sides. Message format: docs/web-m2.md.

import Foundation

public let controlChannelLabel = "control"
public let chunkSize = 16 * 1024
/// UNCERTAIN: like the web client, 10 MiB is a guess; no measurements.
public let maxImageBytes = 10 * 1024 * 1024
public let imageMimeTypes = ["image/png", "image/jpeg", "image/webp", "image/gif"]
/// Longest text message in UTF-16 units (JavaScript `String.length`).
public let maxTextChars = 8192
/// Longest file name in Unicode characters (code points).
public let maxNameLength = 255
public let maxDocumentBytes = 64 * 1024 * 1024
/// UNCERTAIN: 30 s without a chunk drops a transfer (the web client's guess; crates/cc-files uses 60 s).
/// All times in this file are milliseconds (like `Date.now()` in the web client).
public let stallTimeoutMs: Double = 30_000
public let progressIntervalMs: Double = 100

public enum ControlCmd: Equatable, Sendable {
    case nextPage, prevPage, stop
    case goToPage(Int)

    var json: JSONValue {
        switch self {
        case .nextPage: return .string("next_page")
        case .prevPage: return .string("prev_page")
        case .stop: return .string("stop")
        case .goToPage(let p): return .object(["go_to_page": .object(["page": .number(Double(p))])])
        }
    }
}

public enum ControlMessage: Equatable, Sendable {
    case cmd(ControlCmd)
    case image(name: String, mime: String, size: Int)
    case imageEnd
    case document(name: String, size: Int)
    case documentEnd
    case link(url: String)

    /// The same JSON shape the web client's `ControlMessage` has (used by the shared fixtures).
    public var json: JSONValue {
        switch self {
        case .cmd(let c): return .object(["kind": .string("cmd"), "cmd": c.json])
        case .image(let n, let m, let s):
            return .object(["kind": .string("image"), "name": .string(n), "mime": .string(m), "size": .number(Double(s))])
        case .imageEnd: return .object(["kind": .string("image_end")])
        case .document(let n, let s):
            return .object(["kind": .string("document"), "name": .string(n), "mime": .string("application/pdf"), "size": .number(Double(s))])
        case .documentEnd: return .object(["kind": .string("document_end")])
        case .link(let u): return .object(["kind": .string("link"), "url": .string(u)])
        }
    }
}

public enum TransferErrorCode: String, Equatable, Sendable {
    case unexpectedChunk = "unexpected_chunk"
    case tooLarge = "too_large"
    case sizeMismatch = "size_mismatch"
    case noTransfer = "no_transfer"
    case badChunk = "bad_chunk"
    case stalled
    case documentUnsupported = "document_unsupported"
    case documentInvalid = "document_invalid"
    case documentEncrypted = "document_encrypted"
    case documentTooLarge = "document_too_large"
    case documentTooManyPages = "document_too_many_pages"
    case documentRejected = "document_rejected"
    case documentMemory = "document_memory"
    case documentTimeout = "document_timeout"
    case documentFailed = "document_failed"
    case badMessage = "bad_message"
    // Apple TV only (no web equivalent): an image the system decoder refused or that is over the pixel cap.
    case imageUndecodable = "image_undecodable"
    case imageTooManyPixels = "image_too_many_pixels"
}

// MARK: names

/// Number of Unicode characters (code points) in a name.
public func nameLength(_ s: String) -> Int { s.unicodeScalars.count }

private func isValidNameLength(_ s: String) -> Bool {
    let u = s.utf16.count
    return u > 0 && u <= maxNameLength * 2 && nameLength(s) <= maxNameLength
}

private func isInvisible(_ v: UInt32) -> Bool {
    switch v {
    case 0x00...0x1F, 0x7F...0x9F, 0xAD, 0x0600...0x0605, 0x06DD, 0x070F, 0x0890, 0x0891, 0x08E2, 0x115F, 0x1160,
         0x17B4, 0x17B5, 0x180E, 0x200B...0x200F, 0x2028...0x202E, 0x2060...0x206F, 0x2800, 0x3164, 0xFEFF, 0xFFA0,
         0xFFF9...0xFFFB, 0x110BD, 0x110CD, 0x13430...0x1343F, 0x1BCA0...0x1BCA3, 0x1D173...0x1D17A, 0xE0000...0xE007F:
        return true
    default: return false
    }
}

/// Removes control, bidirectional and other invisible characters from a displayed name (same list as the web
/// client's `sanitizeName` and `sanitize_name` in crates/cc-files).
public func sanitizeName(_ s: String) -> String {
    var out = String.UnicodeScalarView()
    for u in s.unicodeScalars where !isInvisible(u.value) { out.append(u) }
    return String(out)
}

/// Removes bidirectional control characters (for hosts shown to people).
public func stripBidi(_ s: String) -> String {
    var out = String.UnicodeScalarView()
    for u in s.unicodeScalars {
        switch u.value {
        case 0x200E, 0x200F, 0x061C, 0x202A...0x202E, 0x2066...0x2069: continue
        default: out.append(u)
        }
    }
    return String(out)
}

// MARK: parser

private func positiveInt(_ v: JSONValue?, max: Int) -> Int? {
    guard let d = v?.integerValue, d >= 1, d <= Double(max) else { return nil }
    return Int(d)
}

private func parseCmd(_ c: JSONValue?) -> ControlCmd? {
    guard let c else { return nil }
    switch c {
    case .string(let s):
        switch s {
        case "next_page": return .nextPage
        case "prev_page": return .prevPage
        case "stop": return .stop
        default: return nil
        }
    case .object(let o):
        guard case .object(let g)? = o["go_to_page"], let p = positiveInt(g["page"], max: 1_000_000) else { return nil }
        return .goToPage(p)
    default: return nil
    }
}

/// Parses and validates one text message. nil for anything unknown or malformed.
public func parseControlMessage(_ text: String) -> ControlMessage? {
    if text.utf16.count > maxTextChars { return nil }
    guard case .object(let o)? = JSON.parse(text) else { return nil }
    guard case .string(let kind)? = o["kind"] else { return nil }
    switch kind {
    case "cmd":
        guard let cmd = parseCmd(o["cmd"]) else { return nil }
        return .cmd(cmd)
    case "image":
        guard case .string(let raw)? = o["name"], isValidNameLength(raw) else { return nil }
        guard case .string(let mime)? = o["mime"], imageMimeTypes.contains(mime) else { return nil }
        guard let size = positiveInt(o["size"], max: maxImageBytes) else { return nil }
        let name = sanitizeName(raw)
        if name.isEmpty { return nil }
        return .image(name: name, mime: mime, size: size)
    case "image_end": return .imageEnd
    case "document":
        guard case .string(let raw)? = o["name"], isValidNameLength(raw) else { return nil }
        guard case .string("application/pdf")? = o["mime"] else { return nil }
        guard let size = positiveInt(o["size"], max: maxDocumentBytes) else { return nil }
        let name = sanitizeName(raw)
        if name.isEmpty { return nil }
        return .document(name: name, size: size)
    case "document_end": return .documentEnd
    case "link":
        guard case .string(let u)? = o["url"], let url = validateLinkUrl(u) else { return nil }
        return .link(url: url)
    default: return nil
    }
}

// MARK: reassembly

/// Reassembles one image or PDF from its announcement and binary chunks. One buffer of the announced size is
/// allocated up front; every chunk must be exactly `chunkSize` bytes unless it completes the transfer (so the
/// number of messages is bounded by size / chunkSize). Time is passed in (`nowMs`) so tests need no timers:
/// the owner of the assembler calls `stalled(nowMs:)` regularly (the app does it once a second).
public final class ImageAssembler {
    struct Current {
        var name: String
        var mime: String
        var size: Int
        var buf: [UInt8]
        var received: Int
        var lastActivity: Double
    }
    var cur: Current?

    public init() {}

    public var active: Bool { cur != nil }
    public var progress: (name: String, received: Int, size: Int)? {
        cur.map { ($0.name, $0.received, $0.size) }
    }

    /// Begins a transfer; one in progress is dropped. The size limit is enforced by `parseControlMessage`.
    public func start(name: String, mime: String, size: Int, nowMs: Double) {
        cur = Current(name: name, mime: mime, size: size, buf: [UInt8](repeating: 0, count: size), received: 0, lastActivity: nowMs)
    }

    /// Returns an error code (and drops the transfer) if the chunk is not acceptable.
    public func push(_ chunk: [UInt8], nowMs: Double) -> TransferErrorCode? {
        guard var c = cur else { return .unexpectedChunk }
        let end = c.received + chunk.count
        if end > c.size { reset(); return .tooLarge }
        if chunk.count != chunkSize && end != c.size { reset(); return .badChunk }
        c.buf.replaceSubrange(c.received..<end, with: chunk)
        c.received = end
        c.lastActivity = nowMs
        cur = c
        return nil
    }

    /// True (and the transfer is dropped) when nothing arrived for `stallTimeoutMs`.
    public func stalled(nowMs: Double, timeout: Double = stallTimeoutMs) -> Bool {
        guard let c = cur, nowMs - c.lastActivity >= timeout else { return false }
        cur = nil
        return true
    }

    public enum Finished: Equatable {
        case done(bytes: [UInt8], mime: String, name: String)
        case failed(TransferErrorCode)
    }

    public func finish() -> Finished {
        let c = cur
        reset()
        guard let c else { return .failed(.noTransfer) }
        if c.received != c.size { return .failed(.sizeMismatch) }
        return .done(bytes: c.buf, mime: c.mime, name: c.name)
    }

    public func reset() { cur = nil }
}

public protocol ControlReceiverHandlers: AnyObject {
    func onCmd(_ cmd: ControlCmd)
    func onProgress(_ p: (name: String, received: Int, size: Int)?)
    func onImage(bytes: [UInt8], mime: String, name: String)
    /// A complete PDF (the announced number of bytes arrived). Nothing has looked at the bytes yet.
    func onDocument(bytes: [UInt8], name: String)
    func onLink(url: String)
    func onError(_ code: TransferErrorCode)
}

/// Receiver side: feed it every message of the channel (`handleText`, `handleBinary`) and call `tick(nowMs:)`.
public final class ControlReceiver {
    private let asm = ImageAssembler()
    private var isDocument = false
    private var lastProgress: Double = 0
    private unowned let h: ControlReceiverHandlers

    public init(handlers: ControlReceiverHandlers) { self.h = handlers }

    public func reset() { asm.reset(); isDocument = false }

    /// Drops a stalled transfer and reports `stalled`.
    public func tick(nowMs: Double) {
        if asm.stalled(nowMs: nowMs) {
            isDocument = false
            h.onProgress(nil)
            h.onError(.stalled)
        }
    }

    public func handleBinary(_ chunk: [UInt8], nowMs: Double) {
        if let err = asm.push(chunk, nowMs: nowMs) {
            h.onProgress(nil)
            h.onError(err)
            return
        }
        if nowMs - lastProgress >= progressIntervalMs {
            lastProgress = nowMs
            h.onProgress(asm.progress)
        }
    }

    public func handleText(_ text: String, nowMs: Double) {
        guard let m = parseControlMessage(text) else { return h.onError(.badMessage) }
        switch m {
        case .cmd(let c):
            if c == .stop { reset() }
            h.onCmd(c)
        case .document(let name, let size):
            isDocument = true
            asm.start(name: name, mime: "application/pdf", size: size, nowMs: nowMs)
            lastProgress = nowMs
            h.onProgress(asm.progress)
        case .documentEnd:
            let wasDocument = isDocument
            let r = wasDocument ? asm.finish() : .failed(.noTransfer)
            if !wasDocument { asm.reset() }
            isDocument = false
            h.onProgress(nil)
            switch r {
            case .failed(let e): h.onError(e)
            case .done(let bytes, _, let name): h.onDocument(bytes: bytes, name: name)
            }
        case .image(let name, let mime, let size):
            isDocument = false
            asm.start(name: name, mime: mime, size: size, nowMs: nowMs)
            lastProgress = nowMs
            h.onProgress(asm.progress)
        case .imageEnd:
            if isDocument {
                // `image_end` for a PDF transfer: drop it.
                asm.reset()
                isDocument = false
                h.onProgress(nil)
                h.onError(.noTransfer)
                break
            }
            let r = asm.finish()
            h.onProgress(nil)
            switch r {
            case .failed(let e): h.onError(e)
            case .done(let bytes, let mime, let name): h.onImage(bytes: bytes, mime: mime, name: name)
            }
        case .link(let url):
            h.onLink(url: url)
        }
    }
}
