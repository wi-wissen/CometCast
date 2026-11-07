// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The link card (spec 4.5, native-apps.md 3): a dialog in the style of the join request: card 600 wide, scrim, section
// label `receive.link.title`, the address in 18 mono `--fg` (wraps anywhere), the hint `receive.link.hintTv`. Apple
// TV has no browser, so there is never an "Open" button: focus row [Dismiss], or [Play, Dismiss] for a media link
// that the system can play; initial focus Dismiss (Play for media). Same 1 s input guard; Back = Dismiss. Warning and
// error lines are 16/700 `--fg` led by the info icon in `--acc` (no red). While playing the card shows the section
// label, the address and the player: a header row (`receive.media.from` left, Stop right) and the AVPlayer view,
// full card width, at most 300 high, on black. Stop, Back and Dismiss close the whole card. Nothing is loaded from the
// media host before Play.

import CometCastCore
import SwiftUI

struct LinkCardView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    let url: String
    let host: String
    @State private var player: PlayerState = .card
    private enum Focus: Hashable { case play, dismiss, stop }
    @FocusState private var focus: Focus?

    private var kind: MediaKind? { classifyMediaUrl(url) }
    private var cleartext: Bool { kind != nil && isCleartextBlocked(url) }
    private var unsupported: Bool { kind != nil && !cleartext && !isPlayableOnTvOS(url) }
    private var playable: Bool { kind != nil && !cleartext && !unsupported }
    private var playing: Bool {
        switch player { case .loading, .playing: return true; default: return false }
    }

    var body: some View {
        DialogCard {
            SectionLabel(text: L.t("receive.link.title", ["host": host]))
            Text(url).dtype(DisplayType.linkAddress).foregroundStyle(p.fg)
                .lineLimit(Metrics.linkMaxAddressLines).truncationMode(.middle)
                .fixedSize(horizontal: false, vertical: true)
            if playing { playerArea } else { hints }
            buttons
        }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
        .accessibilityLabel(L.t("receive.link.title", ["host": host]))
        .defaultFocus($focus, playable ? Focus.play : Focus.dismiss)
        .onAppear { focus = playable ? .play : .dismiss }
    }

    @ViewBuilder private var hints: some View {
        Text(L.s("receive.link.hintTv")).dtype(DisplayType.hint).foregroundStyle(p.mut).fixedSize(horizontal: false, vertical: true)
        if cleartext { InfoLine(text: L.s("receive.media.cleartext")) }
        else if unsupported { InfoLine(text: L.s("receive.media.formatUnsupported")) }
        else if playable {
            if urlHostIsLocalOrIp(url) { InfoLine(text: L.s("receive.media.localWarning")) }
            if case .error(let code) = player { InfoLine(text: L.t("receive.media.error.\(code.rawValue)", ["host": shortHost(host)])) }
        }
    }

    private var playerArea: some View {
        VStack(alignment: .leading, spacing: ref(10)) {
            HStack {
                Text(L.t("receive.media.from", ["host": shortHost(host)]))
                    .dtype(DisplayType.mediaHost).foregroundStyle(p.fg).lineLimit(1)
                Spacer(minLength: ref(12))
                DButton(L.s("receive.media.stop"), kind: .secondary) { model.guarded { model.dismissContent() } }
                    .focused($focus, equals: .stop)
            }
            MediaPlayerView(url: url, state: $player)
                .frame(maxWidth: .infinity).frame(height: ref(300))
                .background(DisplayPalette.picture)
                .clipShape(RoundedRectangle(cornerRadius: ref(12)))
                .accessibilityLabel(L.t("receive.media.label", ["host": shortHost(host)]))
        }
        .onAppear { focus = .stop }
    }

    @ViewBuilder private var buttons: some View {
        if !playing {
            HStack(spacing: ref(12)) {
                if playable {
                    DButton(L.s("receive.media.play"), kind: .secondary) { model.guarded { player = playerReduce(player, .play) } }
                        .focused($focus, equals: .play)
                }
                DButton(L.s("receive.link.dismiss"), kind: .secondary) { model.guarded { model.dismissContent() } }
                    .focused($focus, equals: .dismiss)
            }
            .focusSection()
        }
    }
}
