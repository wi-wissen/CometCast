// SPDX-License-Identifier: GPL-3.0-or-later
// Size maths of the broadcast encoder, separate from FrameScaler so the tests can run it
// without Core Image. Compiled into the extension and (through the Shared folder) the app.

import Foundation
import ImageIO

enum FrameSize {
    /// Output size for a source of `width` x `height` pixels after `orientation`: longest edge
    /// at most `maxDimension`, both edges even (H.264 4:2:0), aspect ratio kept, never scaled up.
    static func fitted(width: Int, height: Int, orientation: CGImagePropertyOrientation,
                       maxDimension: Int) -> (width: Int, height: Int) {
        let swapped: Bool
        switch orientation {
        case .left, .leftMirrored, .right, .rightMirrored: swapped = true
        default: swapped = false
        }
        let w = swapped ? height : width
        let h = swapped ? width : height
        let longest = max(w, h)
        let scale = longest > maxDimension ? Double(maxDimension) / Double(longest) : 1.0
        func even(_ v: Double) -> Int { max(2, Int((v / 2).rounded()) * 2) }
        return (even(Double(w) * scale), even(Double(h) * scale))
    }
}
