// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// While a room is open: the big room code and status until the screen arrives, then the video full screen with
// content (image, link card, PDF) on top. Siri Remote: Menu asks before stopping; left/right turn PDF pages
// locally; the content overlay takes the focus (Close / Play buttons).

import CometCastCore
import SwiftUI

struct ReceiverView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        ZStack {
            if model.state.hasVideo, model.videoTrack != nil {
                VideoSurface(track: model.videoTrack)
                    .ignoresSafeArea()
                    .accessibilityLabel(L.s("a11y.video"))
            } else {
                CodeScreen()
            }
            ContentOverlay()
            if let t = model.transfer { TransferBadge(transfer: t) }
            if model.state.hasVideo, let sas = model.state.sas { SasBadge(sas: sas) }
            if model.showStats { StatsBadge(lines: model.statsText) }
        }
        .focusable(model.content == nil)
        .onExitCommand { if model.content != nil { model.dismissContent() } else { model.showStopConfirm = true } }
        .onPlayPauseCommand { model.showStats.toggle() } // statistics for the device test plan
        .onMoveCommand { dir in
            guard case .document? = model.content else { return }
            if dir == .left { model.turnPage(.prevPage) } else if dir == .right { model.turnPage(.nextPage) }
        }
        .alert(L.s("receiver.stopConfirmTitle"), isPresented: $model.showStopConfirm) {
            Button(L.s("receiver.stop"), role: .destructive) { model.stop() }
            Button(L.s("receiver.keepReceiving"), role: .cancel) {}
        } message: { Text("receiver.stopConfirmBody") }
    }
}

/// The room code, status line, connection check code and the server in use.
struct CodeScreen: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        VStack(spacing: 36) {
            Text("receiver.codeLabel").font(.title2).foregroundStyle(.secondary)
            if let code = model.state.code {
                Text(displayRoomCode(code))
                    .font(.system(size: 190, weight: .bold, design: .monospaced))
                    .minimumScaleFactor(0.5).lineLimit(1)
                    .accessibilityLabel(L.f("a11y.roomCode", code.map(String.init).joined(separator: " ")))
                Text("receiver.codeHint").font(.title3).foregroundStyle(.secondary)
            } else {
                ProgressView().scaleEffect(2).accessibilityHidden(true)
            }
            Text(LocalizedStringKey("receiver.status.\(model.state.phase.rawValue)")).font(.title2)
                .accessibilityAddTraits(.updatesFrequently)
            if let sas = model.state.sas {
                VStack(spacing: 6) {
                    Text("sas.label").font(.callout).foregroundStyle(.secondary)
                    Text(sas).font(.system(size: 64, weight: .semibold, design: .monospaced))
                    Text("sas.explain").font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center).frame(maxWidth: 1000)
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel(L.f("a11y.sas", sas))
            }
            if let r = model.choice.selected {
                Text(L.f("receiver.relayUsing", r.host)).font(.callout).foregroundStyle(.secondary)
                if r.insecure { Text("receiver.unencrypted").font(.callout).foregroundStyle(.orange) }
            }
            Text("receiver.menuHint").font(.footnote).foregroundStyle(.secondary)
        }
        .padding(80)
    }
}

/// The connection check code stays visible over the video (small, in a corner) so it can be compared.
struct SasBadge: View {
    let sas: String
    var body: some View {
        VStack {
            HStack {
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text("sas.label").font(.caption)
                    Text(sas).font(.system(size: 36, weight: .semibold, design: .monospaced))
                }
                .padding(14).background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14))
                .accessibilityElement(children: .combine)
                .accessibilityLabel(L.f("a11y.sas", sas))
            }
            Spacer()
        }
        .padding(40)
    }
}

struct TransferBadge: View {
    let transfer: (name: String, received: Int, size: Int)
    var body: some View {
        let percent = transfer.size > 0 ? Int(Double(transfer.received) / Double(transfer.size) * 100) : 0
        VStack {
            Spacer()
            Text(L.f("receiver.transfer", transfer.name, percent))
                .font(.title3).padding(20).background(.regularMaterial, in: Capsule())
                .accessibilityLabel(L.f("a11y.progress", percent))
        }
        .padding(60)
    }
}

/// Statistics (Play/Pause toggles them): picture, decoder, frames, rate, route, round trip time.
struct StatsBadge: View {
    let lines: [String]
    var body: some View {
        VStack {
            Spacer()
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("stats.title").font(.caption.bold())
                    ForEach(lines, id: \.self) { Text($0).font(.system(.callout, design: .monospaced)) }
                }
                .padding(16).background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14))
                .accessibilityElement(children: .combine)
                Spacer()
            }
        }
        .padding(40)
    }
}
