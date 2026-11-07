// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Received content on top of everything: image, link card, PDF page. A transfer error is a translated notice.
// Links: shown as a card with the host and the full address, NO open action (tvOS has no browser). Only a
// media link (same classification as the web app) gets a "Play" button; nothing is loaded before it is selected.

import CometCastCore
import SwiftUI

struct ContentOverlay: View {
    @EnvironmentObject var model: AppModel
    @Namespace private var ns

    var body: some View {
        ZStack {
            if let c = model.content {
                Color.black.opacity(0.55).ignoresSafeArea()
                switch c {
                case .image(_, let name, _): imageView(name: name)
                case .link(let url, let host): LinkCard(url: url, host: host)
                case .document(_, let name, let pages, let page): documentView(name: name, pages: pages, page: page)
                }
            }
            if let e = model.contentError, model.content == nil { notice(e) }
        }
        .focusScope(ns)
        .animation(.default, value: model.content)
    }

    private func closeButton() -> some View {
        Button { model.dismissContent() } label: { Label(L.s("content.close"), systemImage: "xmark") }
            .prefersDefaultFocus(true, in: ns)
    }

    private func imageView(name: String) -> some View {
        VStack(spacing: 30) {
            if let img = model.image {
                Image(uiImage: img).resizable().scaledToFit().accessibilityLabel(L.f("content.imageLabel", name))
            } else {
                ProgressView().scaleEffect(2)
            }
            closeButton()
        }
        .padding(60)
    }

    private func documentView(name: String, pages: Int, page: Int) -> some View {
        VStack(spacing: 24) {
            if let img = model.pageImage, pages > 0 {
                Image(uiImage: img).resizable().scaledToFit().accessibilityLabel(L.f("document.label", name))
            } else {
                VStack(spacing: 16) { ProgressView().scaleEffect(2); Text("document.opening").font(.title3) }
            }
            HStack(spacing: 40) {
                if pages > 0 { Text(L.f("document.page", page, pages)).font(.title3) }
                closeButton()
            }
        }
        .padding(60)
    }

    private func notice(_ e: TransferErrorCode) -> some View {
        VStack {
            Spacer()
            Text(LocalizedStringKey("transferError.\(e.rawValue)"))
                .font(.title3).padding(24).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
                .accessibilityAddTraits(.isStaticText)
        }
        .padding(60)
    }
}

struct LinkCard: View {
    @EnvironmentObject var model: AppModel
    let url: String
    let host: String
    @State private var player: PlayerState = .card

    private var kind: MediaKind? { classifyMediaUrl(url) }
    private var local: Bool { urlHostIsLocalOrIp(url) }

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            Text(L.f("link.title", host)).font(.title.bold()).accessibilityAddTraits(.isHeader)
            Text("link.hint").font(.callout).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 4) {
                Text("link.address").font(.caption).foregroundStyle(.secondary)
                // the whole address, selectable text is not offered on tvOS; long addresses wrap
                Text(url).font(.system(.body, design: .monospaced)).lineLimit(6).truncationMode(.middle)
            }
            if kind != nil { mediaSection }
            Button { model.dismissContent() } label: { Text("link.dismiss") }
        }
        .padding(50)
        .frame(maxWidth: 1400, alignment: .leading)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 30))
        .accessibilityElement(children: .contain)
    }

    @ViewBuilder private var mediaSection: some View {
        if isCleartextBlocked(url) {
            Text("media.cleartext").font(.callout).foregroundStyle(.orange)
        } else if !isPlayableOnTvOS(url) {
            Text("media.formatUnsupported").font(.callout).foregroundStyle(.orange)
        } else {
            if local { Text("media.localWarning").font(.callout).foregroundStyle(.orange) }
            switch player {
            case .card:
                Text(L.f("media.hint", shortHost(host))).font(.callout).foregroundStyle(.secondary)
                Button { player = playerReduce(player, .play) } label: { Text(L.f("media.play", shortHost(host))) }
            case .loading, .playing:
                MediaPlayerView(url: url, state: $player).frame(height: 600).accessibilityLabel(L.f("media.label", shortHost(host)))
                Button { player = playerReduce(player, .stop) } label: { Text("media.stop") }
            case .error(let code):
                Text(L.f("media.error.\(code.rawValue)", shortHost(host))).font(.callout).foregroundStyle(.orange)
                Button { player = playerReduce(player, .play) } label: { Text("media.retry") }
            }
        }
    }
}
