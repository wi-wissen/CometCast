// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here). LKRTCMTLVideoView is a UIView on tvOS (Metal, MTKView based) and a
// frame renderer; the alternative named in docs/appletv.md is AVSampleBufferDisplayLayer fed from an
// LKRTCVideoRenderer.

import LiveKitWebRTC
import SwiftUI

struct VideoSurface: UIViewRepresentable {
    let track: LKRTCVideoTrack?
    /// Paused (docs/design/pause.md): the view is taken off the track, so it keeps the last picture it drew while the
    /// decoder goes on. Live again: it is added back. UNCERTAIN: that the Metal view keeps showing its last frame
    /// after it was removed as a renderer is assumed, not tried on a device (docs/measurement-points.md).
    var frozen = false

    final class Coordinator { var track: LKRTCVideoTrack?; var attached = false }
    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> LKRTCMTLVideoView {
        let v = LKRTCMTLVideoView(frame: .zero)
        v.videoContentMode = .scaleAspectFit
        v.isUserInteractionEnabled = false
        return v
    }

    func updateUIView(_ view: LKRTCMTLVideoView, context: Context) {
        let c = context.coordinator
        if c.track !== track {
            if c.attached { c.track?.remove(view) }
            c.attached = false
            c.track = track
        }
        let want = !frozen && track != nil
        if want, !c.attached { track?.add(view); c.attached = true }
        if !want, c.attached { track?.remove(view); c.attached = false }
    }

    static func dismantleUIView(_ view: LKRTCMTLVideoView, coordinator: Coordinator) {
        if coordinator.attached { coordinator.track?.remove(view) }
    }
}
