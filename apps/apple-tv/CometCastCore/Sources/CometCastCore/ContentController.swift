// SPDX-License-Identifier: GPL-3.0-or-later
//
// What is shown for received content: the state part of apps/web/src/core/receiverSession.ts (content, page
// turning, error codes). Rendering (ImageIO, PDFKit) is done by the app; this class holds the decisions.

import Foundation

public enum ReceivedContent: Equatable, Sendable {
    case image(id: Int, name: String, mime: String)
    /// `host` is the host (with port when not the default) shown on the card.
    case link(url: String, host: String)
    /// A PDF. `pages` is 0 while it is being opened; `page` is the shown page, counted from 1.
    case document(id: Int, name: String, pages: Int, page: Int)
}

/// Maps a renderer error code to the translation key suffix (web `documentErrorCode`).
public func documentErrorCode(_ code: String) -> TransferErrorCode {
    switch code {
    case "unavailable": return .documentUnsupported
    case "invalid": return .documentInvalid
    case "encrypted": return .documentEncrypted
    case "too_large": return .documentTooLarge
    case "too_many_pages": return .documentTooManyPages
    case "rejected": return .documentRejected
    case "memory": return .documentMemory
    case "timeout": return .documentTimeout
    default: return .documentFailed
    }
}

/// Limits for PDF rendering, the values of cc-docs (crates/cc-docs/src/document.rs `Limits::default`).
/// UNCERTAIN: copied, not measured on an Apple TV.
public let pdfMaxPages = 5000
public let pdfMaxOutputSide = 8192
public let pdfMaxOutputPixels = 7680 * 4320
/// Largest image the decoder is asked to decode (cc-docs `max_image_pixels`, 64 Mpixel). UNCERTAIN: memory on tvOS.
public let imageMaxPixels = 64 * 1024 * 1024
/// Images are decoded down to at most this many pixels on the long side (a TV shows 3840 at most).
public let imageMaxDecodeSide = 3840

public final class ContentController {
    public private(set) var content: ReceivedContent?
    public private(set) var transfer: (name: String, received: Int, size: Int)?
    public private(set) var contentError: TransferErrorCode?
    private var nextId = 0

    public init() {}

    /// Closes the shown image, link or document (also used by the Close button). Returns true if something changed.
    @discardableResult
    public func clear() -> Bool {
        let changed = content != nil || transfer != nil || contentError != nil
        content = nil; transfer = nil; contentError = nil
        return changed
    }

    public func setProgress(_ p: (name: String, received: Int, size: Int)?) { transfer = p }
    public func setError(_ e: TransferErrorCode) { contentError = e }

    public func showImage(name: String, mime: String) -> Int {
        clear()
        nextId += 1
        content = .image(id: nextId, name: name, mime: mime)
        return nextId
    }

    public func showLink(url: String) {
        clear()
        let host = parseHTTPURL(url)?.hostAndPort ?? ""
        content = .link(url: url, host: stripBidi(host))
    }

    /// A PDF arrived: shown with 0 pages until the renderer has opened it. Returns the id of this transfer.
    public func openDocument(name: String) -> Int {
        clear()
        nextId += 1
        content = .document(id: nextId, name: name, pages: 0, page: 1)
        return nextId
    }

    public func documentOpened(id: Int, pages: Int) {
        if case .document(let i, let n, _, let p)? = content, i == id { content = .document(id: i, name: n, pages: pages, page: min(max(p, 1), max(pages, 1))) }
    }

    public func documentFailed(id: Int, code: String) {
        if case .document(let i, _, _, _)? = content, i == id {
            clear()
            contentError = documentErrorCode(code)
        }
    }

    /// Shows page `n` (from 1, clamped to the page count). Ignored while no document is shown or opened.
    public func goToPage(_ n: Int) {
        guard case .document(let id, let name, let pages, let page)? = content, pages > 0 else { return }
        let p = min(max(n, 1), pages)
        if p != page { content = .document(id: id, name: name, pages: pages, page: p) }
    }

    public func onCmd(_ cmd: ControlCmd) {
        let current: Int
        if case .document(_, _, _, let p)? = content { current = p } else { current = 0 }
        switch cmd {
        case .stop: clear()
        case .nextPage: goToPage(current + 1)
        case .prevPage: goToPage(current - 1)
        case .goToPage(let p): goToPage(p)
        }
    }
}

/// The page message of the content control channel, display to presenter (spec 5.6, decisions 13.31 and 58):
/// `{"kind":"page","page":n,"pages":m}`, 1-based, sent once the page count is known and on every page change.
/// nil when the values are not `1 <= page <= pages` (that would mean "unknown": nothing is sent).
public func pageMessageJSON(page: Int, pages: Int) -> String? {
    guard pages >= 1, page >= 1, page <= pages else { return nil }
    return JSON.serialize(.object(["kind": .string("page"), "page": .number(Double(page)), "pages": .number(Double(pages))]))
}
