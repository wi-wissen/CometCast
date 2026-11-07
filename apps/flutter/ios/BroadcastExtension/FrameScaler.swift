// SPDX-License-Identifier: GPL-3.0-or-later
// Brings a ReplayKit picture to the encoder size and orientation, into pixel buffers from a
// small fixed pool. The pool is the extension's frame memory budget: a request beyond the pool
// fails and the caller drops the frame (backpressure, nothing queues without a bound).
//
// UNCERTAIN: never compiled or run. Open points, all in docs/ios-device-test-plan.md:
//  * whether CIContext keeps the extension below ~50 MB (plan B: VTPixelTransferSession for the
//    scale and VTPixelRotationSession (iOS 16+) for the rotation, no Core Image);
//  * whether CIContext can render into the bi-planar 4:2:0 format below on every device (plan
//    B: change `FrameScaler.pixelFormat` to kCVPixelFormatType_32BGRA, 2.7x the memory per buffer);
//  * which orientation ReplayKit attaches in landscape on iPhone and iPad (see `orientation(of:)`).

import CoreImage
import CoreMedia
import CoreVideo
import ImageIO
import ReplayKit

final class FrameScaler {
    /// Format of the pool buffers: what the hardware encoder takes without conversion.
    static let pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange

    /// Buffers in the pool. Frames in flight (encoder) plus the one held for key frame repeats.
    static let poolSize = 4

    private let context = CIContext(options: [
        .cacheIntermediates: false,
        .useSoftwareRenderer: false,
    ])
    private var pool: CVPixelBufferPool?
    private(set) var outputSize = CGSize.zero
    private let maxDimension: Int

    init(maxDimension: Int) { self.maxDimension = maxDimension }

    /// The orientation ReplayKit attached to the buffer (`.up` if none).
    static func orientation(of sampleBuffer: CMSampleBuffer) -> CGImagePropertyOrientation {
        guard let raw = CMGetAttachment(sampleBuffer, key: RPVideoSampleOrientationKey as CFString,
                                        attachmentModeOut: nil) as? NSNumber,
              let o = CGImagePropertyOrientation(rawValue: raw.uint32Value)
        else { return .up }
        return o
    }

    /// A new picture in the pool, scaled and rotated; nil if the pool is exhausted (drop the
    /// frame) or rendering failed. `sizeChanged` is true when the output size differs from the
    /// previous call (the encoder must be reconfigured before the picture is encoded).
    func render(_ source: CVPixelBuffer, orientation: CGImagePropertyOrientation)
        -> (buffer: CVPixelBuffer, sizeChanged: Bool)? {
        let size = FrameSize.fitted(
            width: CVPixelBufferGetWidth(source), height: CVPixelBufferGetHeight(source),
            orientation: orientation, maxDimension: maxDimension)
        let target = CGSize(width: size.width, height: size.height)
        var changed = false
        if target != outputSize || pool == nil {
            guard makePool(width: size.width, height: size.height) else { return nil }
            outputSize = target
            changed = true
        }
        guard let pool else { return nil }
        var out: CVPixelBuffer?
        let aux = [kCVPixelBufferPoolAllocationThresholdKey: FrameScaler.poolSize] as CFDictionary
        let status = CVPixelBufferPoolCreatePixelBufferWithAuxAttributes(nil, pool, aux, &out)
        guard status == kCVReturnSuccess, let dst = out else { return nil }  // WouldExceedAllocationThreshold: drop

        var image = CIImage(cvPixelBuffer: source).oriented(orientation)
        let extent = image.extent
        let sx = target.width / extent.width
        let sy = target.height / extent.height
        image = image
            .transformed(by: CGAffineTransform(translationX: -extent.origin.x, y: -extent.origin.y))
            .transformed(by: CGAffineTransform(scaleX: sx, y: sy))
        context.render(image, to: dst)
        return (dst, changed)
    }

    private func makePool(width: Int, height: Int) -> Bool {
        let attributes: [CFString: Any] = [
            kCVPixelBufferPixelFormatTypeKey: FrameScaler.pixelFormat,
            kCVPixelBufferWidthKey: width,
            kCVPixelBufferHeightKey: height,
            kCVPixelBufferIOSurfacePropertiesKey: [String: Any](),
        ]
        let poolAttributes = [kCVPixelBufferPoolMinimumBufferCountKey: 2] as CFDictionary
        var newPool: CVPixelBufferPool?
        guard CVPixelBufferPoolCreate(nil, poolAttributes, attributes as CFDictionary, &newPool) == kCVReturnSuccess
        else { return false }
        pool = newPool
        return true
    }

    func reset() {
        pool = nil
        outputSize = .zero
    }
}
