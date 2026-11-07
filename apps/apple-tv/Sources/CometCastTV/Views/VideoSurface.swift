// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here). LKRTCMTLVideoView is a UIView on tvOS (Metal, MTKView based) and a
// frame renderer; the alternative named in docs/appletv.md is AVSampleBufferDisplayLayer fed from an
// LKRTCVideoRenderer.

import LiveKitWebRTC
import SwiftUI

struct VideoSurface: UIViewRepresentable {
    let track: LKRTCVideoTrack?

    final class Coordinator { var track: LKRTCVideoTrack? }
    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> LKRTCMTLVideoView {
        let v = LKRTCMTLVideoView(frame: .zero)
        v.videoContentMode = .scaleAspectFit
        v.isUserInteractionEnabled = false
        return v
    }

    func updateUIView(_ view: LKRTCMTLVideoView, context: Context) {
        if context.coordinator.track === track { return }
        context.coordinator.track?.remove(view)
        track?.add(view)
        context.coordinator.track = track
    }

    static func dismantleUIView(_ view: LKRTCMTLVideoView, coordinator: Coordinator) {
        coordinator.track?.remove(view)
    }
}
