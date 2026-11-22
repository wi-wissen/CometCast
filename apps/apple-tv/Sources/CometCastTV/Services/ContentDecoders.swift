// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Turning received bytes into pictures. Everything received is hostile.
//   * Images: ImageIO. The type is taken from the bytes (not from the announced MIME type) and must be one of
//     PNG, JPEG, WebP, GIF; the pixel count is read from the header before decoding and capped
//     (imageMaxPixels, the value of cc-docs `max_image_pixels`); decoding goes through a thumbnail request so the
//     bitmap is at most imageMaxDecodeSide pixels on the long side. A GIF shows its first frame only. One image
//     is decoded at a time (ImageDecodeQueue), and one that is not the shown image any more is not decoded at all.
//     UNCERTAIN: WebP support of ImageIO on tvOS (it exists from iOS 14 / tvOS 14, to our knowledge).
//   * PDF: Core Graphics (CGPDFDocument). PDFKit is, to our knowledge, not available on tvOS (UNCERTAIN: could not
//     check without the SDK); Core Graphics is. Only pixels are shown: no text layer, no links, no scripts. The
//     desktop apps render in a child process with a memory limit (cc-docs); tvOS cannot start processes, so a
//     hostile PDF runs inside the app. Limits are those of cc-docs (pages, output size); time is limited by a
//     watchdog that stops waiting (the drawing itself cannot be cancelled). All pages are drawn on one queue
//     (PDFRenderGate), and while a draw that ran into the watchdog still occupies it, nothing new is drawn: stuck
//     draws do not pile up. Residual risk: see docs/appletv.md.

import CometCastCore
import CoreGraphics
import Foundation
import ImageIO
import UIKit

enum ImageDecoder {
    private static let allowedTypes: Set<String> = ["public.png", "public.jpeg", "org.webmproject.webp", "com.compuserve.gif"]

    static func decode(_ bytes: [UInt8]) -> Result<UIImage, TransferErrorCode> {
        let data = Data(bytes) as CFData
        guard let src = CGImageSourceCreateWithData(data, [kCGImageSourceShouldCache: false] as CFDictionary),
              let type = CGImageSourceGetType(src) as String?, allowedTypes.contains(type), CGImageSourceGetCount(src) >= 1,
              let props = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any],
              let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int, w > 0, h > 0
        else { return .failure(.imageUndecodable) }
        if w.multipliedReportingOverflow(by: h).overflow || w * h > imageMaxPixels { return .failure(.imageTooManyPixels) }
        let opts: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: imageMaxDecodeSide,
        ]
        guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return .failure(.imageUndecodable) }
        return .success(UIImage(cgImage: cg))
    }
}

/// Decodes the received images one after another, so that a sender who sends many images cannot make the display
/// decode many at once. At most one image is being decoded and one waits: a newer one replaces the waiting one, and
/// an image that is not the shown one any more when its turn comes is skipped.
final class ImageDecodeQueue: @unchecked Sendable {
    static let shared = ImageDecodeQueue()

    typealias Outcome = Result<UIImage, TransferErrorCode>

    private struct Job {
        let id: Int
        let bytes: [UInt8]
        let cont: CheckedContinuation<Outcome?, Never>
    }

    private let queue = DispatchQueue(label: "de.cometcast.tv.image", qos: .userInitiated)
    private let lock = NSLock()
    // Guarded by `lock`.
    private var currentId: Int?
    private var waiting: Job?
    private var busy = false

    /// The id of the image that is shown now (`ReceivedContent.image`), nil when no image is shown.
    func setCurrent(_ id: Int?) {
        lock.lock()
        currentId = id
        lock.unlock()
    }

    /// Decodes the image with content id `id`. nil when it was not decoded because it is not the shown image any
    /// more, or a newer image took its place in the queue.
    func decode(_ bytes: [UInt8], id: Int) async -> Outcome? {
        await withCheckedContinuation { (cont: CheckedContinuation<Outcome?, Never>) in
            self.enqueue(Job(id: id, bytes: bytes, cont: cont))
        }
    }

    private func enqueue(_ job: Job) {
        lock.lock()
        let replaced = waiting
        waiting = job
        let start = !busy
        if start { busy = true }
        lock.unlock()
        if let replaced { replaced.cont.resume(returning: nil) }
        if start { queue.async { self.drain() } }
    }

    /// Runs on `queue` until nothing waits.
    private func drain() {
        while true {
            lock.lock()
            guard let job = waiting else {
                busy = false
                lock.unlock()
                return
            }
            waiting = nil
            let current = currentId == job.id
            lock.unlock()
            if current {
                job.cont.resume(returning: ImageDecoder.decode(job.bytes))
            } else {
                job.cont.resume(returning: nil)
            }
        }
    }
}

enum PDFFailure: Error { case code(String) }

/// The one place where PDF pages are drawn: a serial queue for the whole process and the watchdog.
///
/// A draw cannot be cancelled. The watchdog only stops waiting for it; the draw goes on and keeps the queue. While
/// such a draw (or one that waits behind it and ran into its own watchdog) has not left the queue, `run` refuses
/// new work at once, so a document that hangs the renderer costs one thread and not one per page or document.
final class PDFRenderGate: @unchecked Sendable {
    static let shared = PDFRenderGate()

    /// One request. Its fields are guarded by the gate's lock.
    private final class Ticket: @unchecked Sendable {
        /// The waiting caller, until it got its answer (a picture, an error, the timeout or the cancellation).
        var cont: CheckedContinuation<UIImage, Error>?
        var cancelled = false
        /// The work has left the queue (drawn or skipped).
        var left = false
        /// The watchdog fired before the work left the queue: counted in `abandoned`.
        var counted = false
    }

    private let queue = DispatchQueue(label: "de.cometcast.tv.pdf", qos: .userInitiated)
    private let lock = NSLock()
    /// Requests whose watchdog fired and whose work is still in the queue. Guarded by `lock`.
    private var abandoned = 0

    /// A draw that ran into the watchdog still occupies the queue.
    var isBlocked: Bool {
        lock.lock()
        defer { lock.unlock() }
        return abandoned > 0
    }

    /// Runs `work` on the queue and waits at most `timeout` seconds for it. Throws `PDFFailure.code("timeout")` when
    /// the watchdog fires first, `PDFFailure.code("failed")` at once while the queue is blocked, `CancellationError`
    /// when the task is cancelled (work that has not begun is then skipped). The caller is answered exactly once.
    func run(timeout: Double, _ work: @escaping @Sendable () -> Result<UIImage, Error>) async throws -> UIImage {
        if isBlocked { throw PDFFailure.code("failed") }
        let ticket = Ticket()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (cont: CheckedContinuation<UIImage, Error>) in
                guard self.register(cont, for: ticket) else {
                    cont.resume(throwing: CancellationError())
                    return
                }
                self.queue.async {
                    guard self.begin(ticket) else { return }
                    self.finish(ticket, work())
                }
                DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout) {
                    self.timeOut(ticket)
                }
            }
        } onCancel: {
            self.cancel(ticket)
        }
    }

    /// False when the task was cancelled before it began to wait: nothing is queued then.
    private func register(_ cont: CheckedContinuation<UIImage, Error>, for t: Ticket) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if t.cancelled { return false }
        t.cont = cont
        return true
    }

    /// The work's turn has come. False when nobody waits for it any more (timeout or cancellation): it is skipped.
    private func begin(_ t: Ticket) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if t.cont != nil { return true }
        leave(t)
        return false
    }

    /// The work has ended.
    private func finish(_ t: Ticket, _ result: Result<UIImage, Error>) {
        lock.lock()
        let cont = t.cont
        t.cont = nil
        leave(t)
        lock.unlock()
        if let cont { cont.resume(with: result) }
    }

    /// The watchdog: stop waiting. Work that is still in the queue blocks the gate until it has left.
    private func timeOut(_ t: Ticket) {
        lock.lock()
        let cont = t.cont
        t.cont = nil
        if !t.left, !t.counted {
            t.counted = true
            abandoned += 1
        }
        lock.unlock()
        if let cont { cont.resume(throwing: PDFFailure.code("timeout")) }
    }

    private func cancel(_ t: Ticket) {
        lock.lock()
        t.cancelled = true
        let cont = t.cont
        t.cont = nil
        lock.unlock()
        if let cont { cont.resume(throwing: CancellationError()) }
    }

    /// Call with the lock held.
    private func leave(_ t: Ticket) {
        t.left = true
        if t.counted {
            t.counted = false
            abandoned -= 1
        }
    }
}

/// One opened PDF. Pages are drawn on the queue of `PDFRenderGate`, one at a time for all documents.
final class PDFRenderer: @unchecked Sendable {
    private let doc: CGPDFDocument
    let pageCount: Int
    /// UNCERTAIN: 20 s is a placeholder (the desktop renderer has its own time limits).
    static let timeoutSeconds: Double = 20

    /// Checks the bytes and opens the document. Error codes are the ones of `documentErrorCode`.
    init(bytes: [UInt8]) throws {
        let head = bytes.prefix(1024)
        let marker = Array("%PDF-".utf8)
        guard head.count >= marker.count, (0...(head.count - marker.count)).contains(where: { i in marker.indices.allSatisfy { head[head.startIndex + i + $0] == marker[$0] } })
        else { throw PDFFailure.code("invalid") }
        guard bytes.count <= maxDocumentBytes, let provider = CGDataProvider(data: Data(bytes) as CFData), let d = CGPDFDocument(provider)
        else { throw PDFFailure.code("invalid") }
        if d.isEncrypted && !d.isUnlocked { throw PDFFailure.code("encrypted") }
        let n = d.numberOfPages
        if n < 1 { throw PDFFailure.code("invalid") }
        if n > pdfMaxPages { throw PDFFailure.code("too_many_pages") }
        doc = d
        pageCount = n
    }

    /// Renders page `page` (from 1) about `width` pixels wide with a white background. Throws `PDFFailure`:
    /// `timeout` when the watchdog fires (the draw itself goes on, see `PDFRenderGate`), `failed` at once while an
    /// earlier draw that timed out is still running.
    func render(page: Int, width: Int) async throws -> UIImage {
        try await PDFRenderGate.shared.run(timeout: Self.timeoutSeconds) { self.draw(page: page, width: width) }
    }

    /// Runs on the queue of `PDFRenderGate`.
    private func draw(page: Int, width: Int) -> Result<UIImage, Error> {
        guard page >= 1, page <= pageCount, let p = doc.page(at: page) else { return .failure(PDFFailure.code("failed")) }
        var box = p.getBoxRect(.cropBox)
        if box.isEmpty || !box.width.isFinite || !box.height.isFinite { box = p.getBoxRect(.mediaBox) }
        let rotated = p.rotationAngle % 180 != 0
        let pw = rotated ? box.height : box.width, ph = rotated ? box.width : box.height
        guard pw >= 1, ph >= 1 else { return .failure(PDFFailure.code("rejected")) }
        var w = Double(max(16, width))
        var h = w * Double(ph / pw)
        let maxSide = Double(pdfMaxOutputSide), maxPixels = Double(pdfMaxOutputPixels)
        let shrink = min(1, maxSide / max(w, h), (maxPixels / (w * h)).squareRoot())
        w = (w * shrink).rounded(.down); h = (h * shrink).rounded(.down)
        guard w >= 1, h >= 1 else { return .failure(PDFFailure.code("rejected")) }
        let size = CGSize(width: w, height: h)
        let fmt = UIGraphicsImageRendererFormat()
        fmt.scale = 1
        fmt.opaque = true
        let img = UIGraphicsImageRenderer(size: size, format: fmt).image { ctx in
            UIColor.white.setFill()
            ctx.fill(CGRect(origin: .zero, size: size))
            let cg = ctx.cgContext
            cg.translateBy(x: 0, y: size.height)
            cg.scaleBy(x: 1, y: -1)
            cg.concatenate(p.getDrawingTransform(.cropBox, rect: CGRect(origin: .zero, size: size), rotate: 0, preserveAspectRatio: true))
            cg.drawPDFPage(p)
        }
        return .success(img)
    }
}
