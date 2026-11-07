// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Plays one media URL with AVPlayer, only after the person selected "Play" (the parent view creates this view
// only then). No cookies are sent (empty cookie list, shared cookie storage set to "never" at start). The
// Referer header is not controllable here (UNCERTAIN: AVFoundation sends none by default; checked in the device
// test plan). The media host learns the Apple TV's IP address and a User-Agent of AVFoundation; a sender-chosen
// host can be a LAN address (the card warns, see media.localWarning). A load timeout ends the attempt.

import AVFoundation
import AVKit
import CometCastCore
import Combine
import SwiftUI

struct MediaPlayerView: UIViewControllerRepresentable {
    let url: String
    @Binding var state: PlayerState

    func makeCoordinator() -> Coordinator { Coordinator(state: $state) }

    func makeUIViewController(context: Context) -> AVPlayerViewController {
        let vc = AVPlayerViewController()
        guard let u = URL(string: url) else { DispatchQueue.main.async { state = playerReduce(state, .error(.unsupported)) }; return vc }
        let asset = AVURLAsset(url: u, options: [AVURLAssetHTTPCookiesKey: [HTTPCookie]()])
        let item = AVPlayerItem(asset: asset)
        let player = AVPlayer(playerItem: item)
        vc.player = player
        context.coordinator.attach(item: item, player: player)
        player.play()
        return vc
    }

    func updateUIViewController(_ vc: AVPlayerViewController, context: Context) {}

    static func dismantleUIViewController(_ vc: AVPlayerViewController, coordinator: Coordinator) {
        coordinator.cancel()
        vc.player?.pause()
        vc.player?.replaceCurrentItem(with: nil)
        vc.player = nil
    }

    final class Coordinator {
        private var bag = Set<AnyCancellable>()
        private var timeout: DispatchWorkItem?
        private let state: Binding<PlayerState>

        init(state: Binding<PlayerState>) { self.state = state }

        func attach(item: AVPlayerItem, player: AVPlayer) {
            item.publisher(for: \.status).receive(on: DispatchQueue.main).sink { [weak self] s in
                guard let self else { return }
                switch s {
                case .readyToPlay: self.timeout?.cancel(); self.state.wrappedValue = playerReduce(self.state.wrappedValue, .playing)
                case .failed: self.timeout?.cancel(); self.state.wrappedValue = playerReduce(self.state.wrappedValue, .error(Self.map(item.error)))
                default: break
                }
            }.store(in: &bag)
            NotificationCenter.default.publisher(for: .AVPlayerItemFailedToPlayToEndTime, object: item).receive(on: DispatchQueue.main).sink { [weak self] n in
                guard let self else { return }
                self.state.wrappedValue = playerReduce(self.state.wrappedValue, .error(Self.map(n.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? Error)))
            }.store(in: &bag)
            let w = DispatchWorkItem { [weak self] in
                guard let self, case .loading = self.state.wrappedValue else { return }
                self.state.wrappedValue = playerReduce(self.state.wrappedValue, .error(.network))
            }
            timeout = w
            DispatchQueue.main.asyncAfter(deadline: .now() + mediaLoadTimeoutSeconds, execute: w)
        }

        func cancel() { timeout?.cancel(); bag.removeAll() }

        /// Maps an AVFoundation / URL loading error to the notice codes of the web client. Never contains URL details.
        static func map(_ e: Error?) -> MediaErrorCode {
            guard let ns = e as NSError? else { return .network }
            if ns.domain == NSURLErrorDomain {
                switch ns.code {
                case NSURLErrorCancelled: return .aborted
                case NSURLErrorAppTransportSecurityRequiresSecureConnection: return .blocked
                default: return .network
                }
            }
            if ns.domain == AVFoundationErrorDomain {
                switch ns.code {
                case AVError.decodeFailed.rawValue: return .decode
                case AVError.fileFormatNotRecognized.rawValue, AVError.failedToLoadMediaData.rawValue: return .unsupported
                default: break
                }
            }
            return .unsupported
        }
    }
}
