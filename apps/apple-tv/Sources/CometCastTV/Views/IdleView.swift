// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The idle screen (spec 4.2), on the 960 x 540 canvas: top row (logo 46, room name 30/700, mode pill), main row
// (step 1 address, step 2 code, or the offline box; the QR column 236 wide), footer (app line, Settings button).
// Edge paddings are max(spec, safe area) (native-apps.md 7). Idle, connecting: the code line keeps its height and is
// empty, the QR box is empty. Locked: code at 35 %, `idle.locked`, QR at 25 % with `idle.qrLocked`.

import CometCastCore
import SwiftUI

struct IdleView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    @Environment(\.canvasInsets) private var insets
    @FocusState private var settingsFocused: Bool

    private var locked: Bool { model.state.joinMode == .locked }
    private var offline: Bool { model.view == .offline }
    private var address: IdleAddress {
        guard let r = model.activeRelay else { return IdleAddress(dom: "", altIp: nil, qrHost: "") }
        return idleAddress(relayHost: r.host, fromBox: r.source == .displayAnnouncement, boxIPv4: model.boxIPv4)
    }
    private var shownCode: String? { model.view == .online ? model.state.code : nil }
    private var qrUrl: String? { shownCode.map { address.qrUrl(code: $0) } }

    var body: some View {
        VStack(alignment: .leading, spacing: ref(Metrics.idleGap)) {
            topRow
            mainRow.frame(maxHeight: .infinity)
            footer
        }
        .padding(.top, ref(insets.top(IdlePadding.top)))
        .padding(.bottom, ref(insets.bottom(IdlePadding.bottom)))
        .padding(.leading, ref(insets.leading(IdlePadding.side)))
        .padding(.trailing, ref(insets.trailing(IdlePadding.side)))
        .frame(width: ref(canvasWidth), height: ref(canvasHeight), alignment: .topLeading)
        .onAppear { settingsFocused = true }
        .onChange(of: model.dialogOpen) { _, open in if !open { settingsFocused = true } }
    }

    // MARK: top row

    private var topRow: some View {
        HStack(spacing: ref(14)) {
            LogoView(size: Metrics.logoIdle)
            Text(model.config?.room ?? "")
                .dtype(DisplayType.roomName).foregroundStyle(p.fg)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            modePill
        }
    }

    private var modePill: some View {
        let keys = joinModeKeys(model.state.joinMode)
        return HStack(spacing: ref(8)) {
            IconView(name: keys.icon, size: Metrics.pillIcon, color: p.acc)
            Text(L.s(keys.title)).dtype(DisplayType.modePill).foregroundStyle(p.fg)
        }
        .padding(.vertical, ref(7)).padding(.leading, ref(10)).padding(.trailing, ref(14))
        .background(Capsule().fill(p.panel2))
        .overlay(Capsule().strokeBorder(p.line, lineWidth: ref(1)))
        .accessibilityElement(children: .combine)
    }

    // MARK: main row

    private var mainRow: some View {
        HStack(alignment: .center, spacing: ref(Metrics.columnGap)) {
            Group {
                if offline { offlineBox } else { steps }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            VStack(spacing: ref(Metrics.qrCaptionGap)) {
                QRView(url: qrUrl, dimmed: locked && qrUrl != nil)
                Text(locked ? L.s("idle.qrLocked") : L.s("idle.qrCaption"))
                    .dtype(DisplayType.footer).foregroundStyle(p.mut).multilineTextAlignment(.center)
                    .opacity(qrUrl == nil ? 0 : 1) // keeps its line: nothing jumps when the code comes back
            }
            .frame(width: ref(236))
        }
    }

    private var steps: some View {
        VStack(alignment: .leading, spacing: ref(Metrics.stepGap)) {
            step("1") {
                Text(L.s("idle.step1")).dtype(DisplayType.stepLabel).foregroundStyle(p.mut)
                Text(address.dom).dtype(DisplayType.address).foregroundStyle(p.fg).fixedSize(horizontal: false, vertical: true)
                if let ip = address.altIp {
                    Text(L.t("idle.altAddress", ["ip": ip])).dtype(DisplayType.hint).foregroundStyle(p.mut)
                }
            }
            step("2") {
                Text(L.s("idle.step2")).dtype(DisplayType.stepLabel).foregroundStyle(p.mut)
                // The code line keeps its height while the code is missing (connecting).
                Text(shownCode.map(displayRoomCode) ?? " ")
                    .dtype(DisplayType.code).foregroundStyle(p.fg).opacity(locked ? 0.35 : 1)
                    .frame(minHeight: ref(80), alignment: .leading)
                    .accessibilityLabel(shownCode.map { $0.map(String.init).joined(separator: " ") } ?? "")
            }
            if locked {
                Text(L.s("idle.locked")).dtype(DisplayType.stepLabel).foregroundStyle(p.mut)
                    .padding(.leading, ref(54))
            }
        }
    }

    private func step<C: View>(_ n: String, @ViewBuilder _ content: () -> C) -> some View {
        HStack(alignment: .top, spacing: ref(Metrics.stepNumberGap)) {
            Text(n).dtype(DisplayType.stepNumber).foregroundStyle(p.bg)
                .frame(width: ref(Metrics.stepDisc), height: ref(Metrics.stepDisc))
                .background(Circle().fill(p.acc))
                .padding(.top, ref(4))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 0) { content() }
        }
    }

    private var offlineBox: some View {
        HStack(alignment: .top, spacing: ref(14)) {
            IconView(name: "wifiOff", size: 34, color: p.acc)
            VStack(alignment: .leading, spacing: ref(4)) {
                Text(L.t("idle.offline.title", ["dom": address.dom])).dtype(DisplayType.offlineTitle).foregroundStyle(p.fg)
                Text(L.s(model.cause.bodyKey)).dtype(DisplayType.offlineBody).foregroundStyle(p.fg)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.vertical, ref(16)).padding(.horizontal, ref(20))
        .background(RoundedRectangle(cornerRadius: ref(16)).fill(p.panel))
        .overlay(RoundedRectangle(cornerRadius: ref(16)).strokeBorder(p.line, style: StrokeStyle(lineWidth: ref(2), dash: [ref(6), ref(4)])))
        .accessibilityElement(children: .combine)
    }

    // MARK: footer

    private var footer: some View {
        HStack(spacing: ref(16)) {
            // The server line and, small in the secondary colour next to it (not near the code), the short tag of this
            // display's id (`display.tag`).
            VStack(alignment: .leading, spacing: ref(2)) {
                Text(appLine).dtype(DisplayType.footer).foregroundStyle(p.mut)
                Text(L.t("display.tag", ["tag": ConfigStore.displayTag]))
                    .dtype(DisplayType.hint).foregroundStyle(p.mut)
                    .accessibilityIdentifier("idle-tag")
            }
            Spacer(minLength: 0)
            DButton(kind: .menu, action: { model.openSettings() }) {
                HStack(spacing: ref(8)) {
                    IconView(name: "gear", size: Metrics.settingsGear, color: p.fg)
                    Text(L.s("settings.open")).dtype(DisplayType.settingsButton)
                }
            }
            .focused($settingsFocused)
        }
    }

    /// `idle.appLine` only when a store or download link exists, else `idle.appLineWeb` (spec 4.2). Where `{dom}`
    /// already ends in `/app` the text `{dom}/app` is written as `{dom}` (decision N85).
    private var appLine: String {
        guard StoreLinks.exist else { return L.s("idle.appLineWeb") }
        let dom = address.dom
        let s = L.t("idle.appLine", ["dom": dom])
        return dom.hasSuffix("/app") ? s.replacingOccurrences(of: dom + "/app", with: dom) : s
    }
}

/// Whether the Apple TV build names store listings on the idle screen. No presenter app is published yet, so no
/// store is named (spec 4.2: "never name a store that has no listing"); see docs/design/implementation-questions-appletv.md.
enum StoreLinks {
    static let exist = false
}
