// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Screen selection and layers (spec 4): setup wizard, or idle, or presenting; dialogs above them (one at a time:
// join request, link card, licence viewer, About, Settings); the toast above everything; the idle dimming overlay on
// top. Everything except the picture is placed on the 960 x 540 canvas, scaled by `min(w / 960, h / 540)` with the rest
// filled with `--bg` (spec 3.1); the tvOS safe area at the canvas edges is handed down as `canvasInsets`. Siri Remote:
// Menu / Back and Play/Pause (as Menu) are routed by CometCastCore's key tables (AppModel).

import CometCastCore
import SwiftUI

struct RootView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @FocusState private var dimFocused: Bool

    var body: some View {
        GeometryReader { geo in
            let scale = canvasScale(width: geo.size.width, height: geo.size.height)
            let letterX = (geo.size.width - canvasWidth * scale) / 2
            let letterY = (geo.size.height - canvasHeight * scale) / 2
            let insets = CanvasInsets(
                top: max(0, geo.safeAreaInsets.top - letterY) / scale,
                leading: max(0, geo.safeAreaInsets.leading - letterX) / scale,
                bottom: max(0, geo.safeAreaInsets.bottom - letterY) / scale,
                trailing: max(0, geo.safeAreaInsets.trailing - letterX) / scale)
            let palette = DisplayPalette(choice: model.config?.appearance ?? .default, systemIsDark: colorScheme == .dark)
            ZStack {
                palette.bg.ignoresSafeArea()
                layers(palette: palette)
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .environment(\.palette, palette)
            .environment(\.ref, Ref(scale: scale))
            .environment(\.canvasInsets, insets)
        }
        .ignoresSafeArea()
        .onExitCommand(perform: model.backHandled ? { model.handleBack() } : nil)
        .onPlayPauseCommand { model.handleMenu() }
        .onChange(of: model.dimmed) { _, d in if d { dimFocused = true } }
    }

    @ViewBuilder private func layers(palette p: DisplayPalette) -> some View {
        // the screen below the dialogs
        if let w = model.wizard {
            CanvasFrame { SetupView(wizard: w) }
        } else if model.presenting {
            PresentingView()
        } else {
            CanvasFrame { IdleView() }
                .disabled(model.dialogOpen || model.dimmed)
                .accessibilityHidden(model.dialogOpen)
        }
        // one dialog at a time
        if model.wizard == nil { dialogs }
        // the toast: above the scrim, uncovered, top 14 (moved by the safe area)
        if let t = model.toast.text {
            CanvasFrame { ToastLayer(text: t) }.allowsHitTesting(false)
        }
        // idle dimming: an overlay of `--bg` at 60 % (2 s in, 200 ms out); the first input only restores it
        p.bg.opacity(model.dimmed ? DisplayTiming.idleDimOverlayAlpha : 0).ignoresSafeArea()
            .animation(reduceMotion ? nil : (model.dimmed ? .easeOut(duration: DisplayTiming.idleDimFadeSeconds) : .linear(duration: DisplayTiming.idleRestoreSeconds)), value: model.dimmed)
            .allowsHitTesting(false)
        if model.dimmed {
            Button { model.wake() } label: { Color.clear.frame(maxWidth: .infinity, maxHeight: .infinity) }
                .buttonStyle(.plain).focusEffectDisabled().focused($dimFocused)
                .onMoveCommand { _ in model.wake() }
                .accessibilityHidden(true)
        }
    }

    @ViewBuilder private var dialogs: some View {
        if let r = model.headRequest {
            Scrim()
            CanvasFrame { JoinRequestView(request: r, more: model.state.requests.count - 1).id(r.id) }
        } else if case .link(let url, let host)? = model.content {
            Scrim()
            CanvasFrame { LinkCardView(url: url, host: host).id(url) }
        } else if model.licencesOpen {
            CanvasFrame { LicencesView(store: model.licences) }
        } else if model.aboutOpen {
            Scrim()
            CanvasFrame { AboutView() }
        } else if model.settingsOpen {
            Scrim()
            CanvasFrame { SettingsView() }
        }
    }
}

private struct ToastLayer: View {
    let text: String
    @Environment(\.ref) private var ref
    @Environment(\.canvasInsets) private var insets
    var body: some View {
        VStack { ToastView(text: text); Spacer() }
            .padding(.top, ref(insets.top(Metrics.toastTop)))
            .frame(maxWidth: .infinity)
    }
}
