// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// About (spec 15.9, "Displays"): a dialog of the link-card family opened from the Settings header, only over Idle:
// section label `about.title`; logo 40 + "Comet Cast" 21/700; lines 16 `--mut` (version, licence, publisher, source,
// web); buttons [Licences, Close]; initial focus Licences; Back = Close, back to Settings with focus on About. The
// licence viewer is the full canvas: top row (section label `about.licences`, Close), a list 300 wide and a text pane.
// D-pad: Up/Down move in the list (the pane shows the focused entry), Right moves to the pane where Up/Down scroll by
// half a pane and Left returns to the list; Back closes the viewer. The Apple TV has no search.

import CometCastCore
import SwiftUI

struct AboutView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    private enum Focus: Hashable { case licences, close }
    @FocusState private var focus: Focus?

    private var version: String { (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String) ?? "" }
    /// `github.com/wi-wissen/CometCast` from the branding file's source URL (spec 15.9).
    private var sourceUrl: String {
        Branding.sourceUrl.replacingOccurrences(of: #"^https?://"#, with: "", options: .regularExpression)
    }
    /// The host of the privacy page (`cometcast.de`); nil when the build has no privacy URL: then `about.webTv` is not
    /// shown (spec 15.14 N108).
    private var webHost: String? {
        let url = Branding.privacyUrlDe.isEmpty ? Branding.privacyUrlEn : Branding.privacyUrlDe
        guard let host = URL(string: url)?.host, !host.isEmpty else { return nil }
        return host
    }

    var body: some View {
        DialogCard {
            SectionLabel(text: L.s("about.title"))
            HStack(spacing: ref(12)) {
                LogoView(size: Metrics.logoWizard)
                Text("Comet Cast").dtype(DisplayType.choiceTitle).foregroundStyle(p.fg)
            }
            VStack(alignment: .leading, spacing: ref(6)) {
                Text(L.t("about.version", ["version": version]))
                Text(L.t("display.tag", ["tag": ConfigStore.displayTag]))
                Text(L.s("about.licence"))
                // "Publisher: {name}", as written; not shown when the branding file has no name (N108).
                if !Branding.publisherName.isEmpty {
                    Text(L.t("about.publisher", ["name": Branding.publisherName]))
                }
                if !Branding.sourceUrl.isEmpty { line("about.sourceTv", url: sourceUrl) }
                if let web = webHost { line("about.webTv", url: web) }
            }
            .dtype(DisplayType.hint).foregroundStyle(p.mut)
            HStack(spacing: ref(12)) {
                DButton(kind: .secondary, action: { model.openLicences() }) { Text(L.s("about.licences")).dtype(DisplayType.button) }
                    .focused($focus, equals: .licences)
                DButton(kind: .secondary, action: { model.closeAbout() }) { Text(L.s("settings.closeRemote")).dtype(DisplayType.button) }
                    .focused($focus, equals: .close)
            }
            .focusSection()
        }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
        .accessibilityLabel(L.s("about.title"))
        .defaultFocus($focus, Focus.licences)
        .onAppear { focus = .licences }
    }

    /// "Quelltext: {url}" with the address in 16 mono `--fg`.
    private func line(_ key: String, url: String) -> some View {
        let parts = L.t(key, ["url": "\u{1}"]).components(separatedBy: "\u{1}")
        return (Text(parts[0])
            + Text(url).font(.custom(FontFace.monoMedium.rawValue, fixedSize: ref(16))).foregroundColor(p.fg)
            + Text(parts.count > 1 ? parts[1] : ""))
    }
}

/// The licence viewer (spec 15.9).
struct LicencesView: View {
    @EnvironmentObject private var model: AppModel
    /// The list is parsed off the main actor; the view must observe the store itself (it is not part of `AppModel`).
    @ObservedObject var store: LicenceStore
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    @Environment(\.canvasInsets) private var insets
    private enum Focus: Hashable { case close, row(Int), pane }
    @FocusState private var focus: Focus?
    @State private var selected = 0
    @State private var topLine = 0

    private var entries: [LicenceEntry] { store.entries ?? [] }
    private var lines: [Substring] {
        guard entries.indices.contains(selected) else { return [] }
        return entries[selected].text.split(separator: "\n", omittingEmptySubsequences: false)
    }
    /// Up/Down in the pane scroll by half a pane: about 12 lines of 13 ref px at line height 1.4.
    private let halfPaneLines = 12

    var body: some View {
        VStack(alignment: .leading, spacing: ref(Metrics.idleGap)) {
            HStack {
                SectionLabel(text: L.s("about.licences"))
                Spacer(minLength: 0)
                DButton(kind: .close, action: { model.closeLicences() }) {
                    HStack(spacing: ref(6)) {
                        IconView(name: "close", size: 16, color: p.mut)
                        Text(L.s("settings.closeRemote")).dtype(DisplayType.closeButton)
                    }
                }
                .focused($focus, equals: .close)
            }
            HStack(alignment: .top, spacing: ref(16)) {
                list.frame(width: ref(Metrics.licenceListWidth))
                pane.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(.top, ref(insets.top(IdlePadding.top)))
        .padding(.bottom, ref(insets.bottom(IdlePadding.bottom)))
        .padding(.leading, ref(insets.leading(IdlePadding.side)))
        .padding(.trailing, ref(insets.trailing(IdlePadding.side)))
        .frame(width: ref(canvasWidth), height: ref(canvasHeight))
        .background(p.bg)
        .task { await store.loadIfNeeded() }
        .onAppear { focus = .row(0) }
        .onChange(of: store.entries?.count) { _, _ in if focus == nil || focus == .close { focus = .row(0) } }
        .onChange(of: selected) { _, _ in topLine = 0 }
    }

    // MARK: list

    private var list: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if store.entries == nil {
                        Text("…").dtype(DisplayType.licenceName).foregroundStyle(p.mut)
                    }
                    group("about.featured", featured: true)
                    group("about.all", featured: false)
                }
            }
            .focusSection()
            .onChange(of: selected) { _, n in withAnimation(nil) { proxy.scrollTo(n, anchor: nil) } }
        }
    }

    @ViewBuilder private func group(_ labelKey: String, featured: Bool) -> some View {
        let rows = entries.enumerated().filter { $0.element.featured == featured }
        if !rows.isEmpty {
            SectionLabel(text: L.s(labelKey)).padding(.vertical, ref(8))
            ForEach(rows, id: \.offset) { i, e in
                Button { focus = .pane } label: {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(e.name).dtype(DisplayType.licenceName).foregroundStyle(p.fg).lineLimit(1)
                        Text(e.licence).dtype(DisplayType.licenceSub).foregroundStyle(p.mut).lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, minHeight: ref(Metrics.licenceRowHeight), alignment: .leading)
                    .padding(.horizontal, ref(8))
                    // The entry the pane shows has the selected-tile look of 3.2: fill (none in OLED black) and a 2 px
                    // `--acc` border; the border is always there (clear when not selected) so nothing moves (N100).
                    .background(RoundedRectangle(cornerRadius: ref(8)).fill(selected == i ? p.selectedTile : .clear))
                    .overlay(RoundedRectangle(cornerRadius: ref(8)).strokeBorder(selected == i ? p.acc : .clear, lineWidth: ref(2)))
                }
                .buttonStyle(.plain)
                .focusEffectDisabled()
                .focused($focus, equals: .row(i))
                .id(i)
                .onChange(of: focus) { _, f in if f == .row(i) { selected = i } }
                .modifier(RowRing(focused: focus == .row(i)))
            }
        }
    }

    // MARK: text pane

    private var pane: some View {
        let shape = RoundedRectangle(cornerRadius: ref(14))
        return ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, l in
                        Text(l.isEmpty ? " " : String(l))
                            .dtype(DisplayType.licenceText).foregroundStyle(p.fg)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .id(i)
                    }
                }
                .padding(ref(16))
            }
            .background(shape.fill(p.panel))
            .overlay(shape.strokeBorder(p.line, lineWidth: ref(1)))
            .focusable()
            .focused($focus, equals: .pane)
            .focusEffectDisabled()
            .dfocusRing(focused: focus == .pane, radius: 14, scales: false)
            .onMoveCommand { dir in
                switch dir {
                case .left: focus = .row(selected)
                case .down: topLine = min(max(lines.count - 1, 0), topLine + halfPaneLines); proxy.scrollTo(topLine, anchor: .top)
                case .up: topLine = max(0, topLine - halfPaneLines); proxy.scrollTo(topLine, anchor: .top)
                default: break
                }
            }
        }
    }
}

/// The ring of a list row: the same outline as a button, no scale.
private struct RowRing: ViewModifier {
    let focused: Bool
    func body(content: Content) -> some View { content.dfocusRing(focused: focused, radius: 8, scales: false) }
}
