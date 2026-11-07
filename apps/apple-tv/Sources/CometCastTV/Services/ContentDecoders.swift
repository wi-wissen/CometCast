// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Turning received bytes into pictures. Everything received is hostile.
//   * Images: ImageIO. The type is taken from the bytes (not from the announced MIME type) and must be one of
//     PNG, JPEG, WebP, GIF; the pixel count is read from the header before decoding and capped
//     (imageMaxPixels, the value of cc-docs `max_image_pixels`); decoding goes through a thumbnail request so the
//     bitmap is at most imageMaxDecodeSide pixels on the long side. A GIF shows its first frame only.
//     UNCERTAIN: WebP support of ImageIO on tvOS (it exists from iOS 14 / tvOS 14, to our knowledge).
//   * PDF: Core Graphics (CGPDFDocument). PDFKit is, to our knowledge, not available on tvOS (UNCERTAIN: could not
//     check without the SDK); Core Graphics is. Only pixels are shown: no text layer, no links, no scripts. The
//     desktop apps render in a child process with a memory limit (cc-docs); tvOS cannot start processes, so a
//     hostile PDF runs inside the app. Limits are those of cc-docs (pages, output size); time is limited by a
//     watchdog that stops waiting (the drawing itself cannot be cancelled). Residual risk: see docs/appletv.md.

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

enum PDFFailure: Error { case code(String) }

/// One opened PDF. Rendering is serialised on a private queue.
final class PDFRenderer: @unchecked Sendable {
    private let doc: CGPDFDocument
    let pageCount: Int
    private let queue = DispatchQueue(label: "de.cometcast.tv.pdf", qos: .userInitiated)
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

    /// Renders page `page` (from 1) about `width` pixels wide with a white background.
    func render(page: Int, width: Int) async throws -> UIImage {
        try await withThrowingTaskGroup(of: UIImage.self) { group in
            group.addTask { try await self.renderOnQueue(page: page, width: width) }
            group.addTask {
                try await Task.sleep(nanoseconds: UInt64(Self.timeoutSeconds * 1_000_000_000))
                throw PDFFailure.code("timeout")
            }
            defer { group.cancelAll() }
            guard let first = try await group.next() else { throw PDFFailure.code("failed") }
            return first
        }
    }

    private func renderOnQueue(page: Int, width: Int) async throws -> UIImage {
        try await withCheckedThrowingContinuation { cont in
            queue.async {
                guard page >= 1, page <= self.pageCount, let p = self.doc.page(at: page) else { return cont.resume(throwing: PDFFailure.code("failed")) }
                var box = p.getBoxRect(.cropBox)
                if box.isEmpty || !box.width.isFinite || !box.height.isFinite { box = p.getBoxRect(.mediaBox) }
                let rotated = p.rotationAngle % 180 != 0
                let pw = rotated ? box.height : box.width, ph = rotated ? box.width : box.height
                guard pw >= 1, ph >= 1 else { return cont.resume(throwing: PDFFailure.code("rejected")) }
                var w = Double(max(16, width))
                var h = w * Double(ph / pw)
                let maxSide = Double(pdfMaxOutputSide), maxPixels = Double(pdfMaxOutputPixels)
                let shrink = min(1, maxSide / max(w, h), (maxPixels / (w * h)).squareRoot())
                w = (w * shrink).rounded(.down); h = (h * shrink).rounded(.down)
                guard w >= 1, h >= 1 else { return cont.resume(throwing: PDFFailure.code("rejected")) }
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
                cont.resume(returning: img)
            }
        }
    }
}
