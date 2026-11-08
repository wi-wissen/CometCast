// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Presenting (spec 4.5): the picture fills the screen edge to edge on black (the existing WebRTC video surface, an
// image or a PDF page), letterboxed in its own aspect ratio. Nothing else is shown at rest. On any Siri Remote event
// the name tag (bottom left) and End (bottom right) fade in together for 5 s (400 ms fades); the first input only
// reveals and never activates anything. At rest nothing is focusable except an invisible catcher that receives the
// remote's events (native-apps.md 6.1). The transfer pill (bottom centre) and the page-error box are states and are
// drawn at rest. The controls sit on the canvas with max(spec padding, safe area) from the edges. Dialogs (join
// request, link card) are drawn above this view by RootView; while one is open the controls are under the scrim and
// inert.

import CometCastCore
import SwiftUI

struct PresentingView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    @Environment(\.canvasInsets) private var insets
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private enum Focus: Hashable { case catcher, end, pause }
    @FocusState private var focus: Focus?

    var body: some View {
        ZStack {
            DisplayPalette.picture.ignoresSafeArea()
            if model.state.hasVideo, let track = model.videoTrack {
                VideoSurface(track: track, frozen: model.pause.isFrozen).ignoresSafeArea().accessibilityHidden(true)
            }
            contentPicture
            CanvasFrame { hud }
            catcher
        }
        .onAppear { focus = model.controlsShown ? .end : .catcher }
        .onChange(of: model.controlsShown) { _, shown in focus = shown ? .end : .catcher }
        .onChange(of: model.dialogOpen) { _, open in if !open { focus = model.controlsShown ? .end : .catcher } }
    }

    // MARK: picture

    @ViewBuilder private var contentPicture: some View {
        switch model.content {
        case .image(_, let name, _):
            if let img = model.image {
                Image(uiImage: img).resizable().scaledToFit().ignoresSafeArea().accessibilityLabel(name)
            }
        case .document(_, let name, let pages, _):
            if pages > 0, model.pageError == nil, let img = model.pageImage {
                Image(uiImage: img).resizable().scaledToFit().ignoresSafeArea().accessibilityLabel(name)
            }
        default: EmptyView()
        }
    }

    // MARK: controls and states on the canvas

    private var hud: some View {
        ZStack {
            // states, drawn at rest
            if let badge = frozenBadge(model.pause) {
                // Who paused (docs/design/pause.md, rule 5): top left, like the name tag; it steps aside while the
                // controls are shown (the Pause control's state says the rest).
                VStack {
                    HStack { tag(L.s(badge == .byPresenter ? "pause.display.byPresenter" : "toolbar.frozenHint")); Spacer() }
                    Spacer()
                }
                .padding(.leading, ref(insets.leading(Metrics.tagEdge)))
                .padding(.top, ref(insets.top(Metrics.tagEdge)))
                .opacity(model.controlsShown ? 0 : 1)
                .animation(reduceMotion ? nil : .easeInOut(duration: DisplayTiming.controlsFadeSeconds), value: model.controlsShown)
                .accessibilityElement(children: .combine)
            }
            if let err = model.pageError {
                tag(L.s("receive.document.pageError.\(err)")).frame(maxWidth: ref(880))
            }
            if let text = pillText {
                VStack { Spacer(); tag(text).frame(maxWidth: ref(Metrics.progressPillMaxWidth)).lineLimit(1) }
                    .padding(.bottom, ref(insets.bottom(Metrics.tagEdge)))
            }
            // controls, with the remote
            VStack {
                Spacer()
                HStack(alignment: .bottom) {
                    nameTag
                    Spacer(minLength: ref(16))
                    // Pause next to End, on its left: only while a live screen is shown (docs/design/pause.md).
                    if model.canPause { pauseButton }
                    endButton
                }
            }
            .padding(.leading, ref(insets.leading(Metrics.tagEdge)))
            .padding(.trailing, ref(insets.trailing(Metrics.tagEdge)))
            .padding(.bottom, ref(insets.bottom(Metrics.tagEdge)))
            .opacity(model.controlsShown ? 1 : 0)
            .animation(reduceMotion ? nil : .easeInOut(duration: DisplayTiming.controlsFadeSeconds), value: model.controlsShown)
            .allowsHitTesting(model.controlsShown && !model.dialogOpen)
        }
    }

    /// The name-tag look: `--panel` at 92 %, 2 px `--line`, radius 10, min height 46, padding 6 x 14, 15/700.
    private func tag(_ text: String) -> some View {
        Text(text).dtype(DisplayType.nameTagLine1).foregroundStyle(p.fg).multilineTextAlignment(.center)
            .padding(.vertical, ref(6)).padding(.horizontal, ref(14))
            .frame(minHeight: ref(Metrics.tagMinHeight))
            .background(RoundedRectangle(cornerRadius: ref(Metrics.tagRadius)).fill(p.overlayFill))
            .overlay(RoundedRectangle(cornerRadius: ref(Metrics.tagRadius)).strokeBorder(p.line, lineWidth: ref(Metrics.tagBorder)))
    }

    private var pillText: String? {
        if let t = model.transfer {
            let template = L.s("receive.transfer")
            let pct = String(transferPercent(received: t.received, size: t.size))
            return fillPlaceholders(template, ["name": fit(t.name, template), "percent": pct])
        }
        if case .document(_, let name, 0, _)? = model.content {
            return fillPlaceholders(L.s("receive.document.loading"), ["name": fit(name, L.s("receive.document.loading"))])
        }
        return nil
    }

    /// One line, at most 340 wide: the name is shortened in the middle, keeping its start and extension; the rest of
    /// the sentence is never cut. About 8.2 ref px per character at 15/700 (UNCERTAIN: measured in the device plan).
    private func fit(_ name: String, _ template: String) -> String {
        let fixed = fillPlaceholders(template, ["name": "", "percent": "100"]).count
        let room = Int((Metrics.progressPillMaxWidth - 2 * 14 - 2 * Metrics.tagBorder) / 8.2) - fixed
        return shortenMiddle(name, keep: max(8, room))
    }

    private var nameTag: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.t("present.byline", ["name": L.quoted(model.state.peerName ?? "", unnamed: model.state.peerUnnamed || model.state.peerName == nil)]))
                .dtype(DisplayType.nameTagLine1).foregroundStyle(p.fg)
            if let sas = model.state.sas { sasLine(sas) }
        }
        .padding(.vertical, ref(6)).padding(.horizontal, ref(14))
        .frame(minHeight: ref(Metrics.tagMinHeight), alignment: .leading)
        .background(RoundedRectangle(cornerRadius: ref(Metrics.tagRadius)).fill(p.overlayFill))
        .overlay(RoundedRectangle(cornerRadius: ref(Metrics.tagRadius)).strokeBorder(p.line, lineWidth: ref(Metrics.tagBorder)))
        .accessibilityElement(children: .combine)
    }

    /// "Security code 171 852": 13/400 `--mut`, the digits mono 700 `--fg`, one ordinary space between the groups.
    private func sasLine(_ sas: String) -> some View {
        let parts = L.t("present.sas", ["sas": "\u{1}"]).components(separatedBy: "\u{1}")
        return (Text(parts[0]).font(.custom(FontFace.regular.rawValue, fixedSize: ref(13))).foregroundColor(p.mut)
            + Text(groupedSas(sas)).font(.custom(FontFace.monoBold.rawValue, fixedSize: ref(13))).foregroundColor(p.fg)
            + Text(parts.count > 1 ? parts[1] : "").font(.custom(FontFace.regular.rawValue, fixedSize: ref(13))).foregroundColor(p.mut))
    }

    /// The display's own pause flag; the words are the presenter's (`pause.presenter.button` / `.resume`).
    private var pauseButton: some View {
        DButton(kind: .overlay, action: { model.togglePause() }) {
            // Text only: the asset catalog has no pause icon yet (tools/gen-appletv-icons.mjs).
            Text(L.s(model.pause.display ? "pause.presenter.resume" : "pause.presenter.button")).dtype(DisplayType.nameTagLine1)
        }
        .focused($focus, equals: .pause)
        .disabled(!model.controlsShown || model.dialogOpen)
        .onMoveCommand { model.presentingKey(Self.key($0)) }
    }

    private var endButton: some View {
        DButton(kind: .overlay, action: { model.presentingKey(.select) }) {
            HStack(spacing: ref(6)) {
                IconView(name: "stop", size: 18, color: p.fg)
                Text(L.s("present.end")).dtype(DisplayType.nameTagLine1)
            }
        }
        .focused($focus, equals: .end)
        .disabled(!model.controlsShown || model.dialogOpen)
        .onMoveCommand { model.presentingKey(Self.key($0)) }
    }

    // MARK: remote catcher

    /// Invisible, the only focusable thing at rest. Any event shows the controls (and nothing else happens).
    private var catcher: some View {
        Button { model.presentingKey(.select) } label: { Color.clear.frame(maxWidth: .infinity, maxHeight: .infinity) }
            .buttonStyle(.plain)
            .focusEffectDisabled()
            .focused($focus, equals: .catcher)
            .disabled(model.controlsShown || model.dialogOpen)
            .onMoveCommand { model.presentingKey(Self.key($0)) }
            .accessibilityHidden(true)
    }

    private static func key(_ d: MoveCommandDirection) -> RemoteKey {
        switch d {
        case .up: return .up
        case .down: return .down
        case .left: return .left
        default: return .right
        }
    }
}
