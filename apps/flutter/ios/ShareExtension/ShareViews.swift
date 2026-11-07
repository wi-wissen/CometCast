// SPDX-License-Identifier: GPL-3.0-or-later
// The pages of the share sheet (spec 15.3): the share page, the relay confirmation, waiting,
// Presenting, the error pages. Presenter tokens and fonts of spec 3.3 / 3.4; texts from the
// extension's Localizable.strings (the shipped set). UNCERTAIN: never compiled (no Xcode).

import SwiftUI

func T(_ key: String) -> String {
    NSLocalizedString(key, comment: "")
}

func T(_ key: String, _ args: CVarArg...) -> String {
    String(format: NSLocalizedString(key, comment: ""), arguments: args)
}

// MARK: - parts

private struct PrimaryButton: View {
    let title: String
    var enabled = true
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Text(title).font(PresenterPalette.ui(17, .bold))
                .frame(maxWidth: .infinity, minHeight: 50)
                .foregroundColor(enabled ? PresenterPalette.onPrimary : PresenterPalette.onDisabled)
                .background(enabled ? PresenterPalette.primary : PresenterPalette.disabled)
                .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .disabled(!enabled)
    }
}

private struct SecondaryButton: View {
    let title: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Text(title).font(PresenterPalette.ui(16, .medium))
                .frame(maxWidth: .infinity, minHeight: 48)
                .foregroundColor(PresenterPalette.fg)
                .background(PresenterPalette.surface)
                .overlay(RoundedRectangle(cornerRadius: 10).stroke(PresenterPalette.border, lineWidth: 1))
                .clipShape(RoundedRectangle(cornerRadius: 10))
        }
    }
}

private struct Disc: View {
    let symbol: String
    var color = PresenterPalette.primary
    var fill = PresenterPalette.primarySoft
    var body: some View {
        ZStack {
            Circle().fill(fill).frame(width: 56, height: 56)
            Image(systemName: symbol).font(.system(size: 28)).foregroundColor(color)
        }
        .accessibilityHidden(true)
    }
}

private struct Field: View {
    let label: String
    @Binding var text: String
    var mono = false
    var invalid = false
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label).font(PresenterPalette.ui(13, .bold)).foregroundColor(PresenterPalette.fg2)
            TextField("", text: $text)
                .font(mono ? PresenterPalette.mono(20) : PresenterPalette.ui(17))
                .textInputAutocapitalization(mono ? .characters : .words)
                .autocorrectionDisabled()
                .padding(.horizontal, 12)
                .frame(minHeight: 46)
                .background(PresenterPalette.surface)
                .overlay(RoundedRectangle(cornerRadius: 8)
                    .stroke(invalid ? PresenterPalette.error : PresenterPalette.borderInput, lineWidth: invalid ? 2 : 1))
        }
    }
}

// MARK: - pages

struct ShareRootView: View {
    @ObservedObject var model: ShareModel

    var body: some View {
        ZStack {
            PresenterPalette.bg.ignoresSafeArea()
            switch model.page {
            case .share: SharePageView(model: model)
            case .relayConfirm: RelayConfirmView(model: model)
            case .waiting: WaitingView(model: model)
            case .presenting: PresentingView(model: model)
            case let .error(p): ErrorPageView(model: model, page: p)
            }
        }
        .foregroundColor(PresenterPalette.fg)
    }
}

struct SharePageView: View {
    @ObservedObject var model: ShareModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text(T("shareTitle")).font(PresenterPalette.ui(26, .bold))
                if let item = model.item { ItemCard(item: item, refusal: model.refusal) }
                if model.refusal != nil || model.item == nil {
                    SecondaryButton(title: T("commonCancel")) { model.stopAndClose() }
                } else {
                    Field(label: T("startNameLabel"), text: $model.name)
                    Field(label: T("startCodeLabel"), text: $model.code, mono: true, invalid: model.codeError)
                    if model.codeError {
                        Text(T("startCodeEmpty")).font(PresenterPalette.ui(13, .bold))
                            .foregroundColor(PresenterPalette.error)
                    }
                    // The address line (spec 15.7). The extension only shows the address it
                    // uses; "Change" is in the app.
                    Text(T("shareVia", model.via)).font(PresenterPalette.ui(13)).foregroundColor(PresenterPalette.fg3)
                    PrimaryButton(title: T("shareShow"), enabled: !model.connecting) { model.show() }
                    SecondaryButton(title: T("commonCancel")) { model.stopAndClose() }
                }
            }
            .padding(16)
        }
    }
}

struct ItemCard: View {
    let item: SharedItem
    let refusal: String?

    var body: some View {
        HStack(spacing: 12) {
            Disc(symbol: item.kind == .link ? "link" : "doc")
            VStack(alignment: .leading, spacing: 4) {
                if item.kind == .link {
                    Text(item.title).font(PresenterPalette.mono(15)).lineLimit(3)
                } else {
                    Text(shortenedFileName(item.title)).font(PresenterPalette.ui(17, .bold)).lineLimit(1)
                }
                if let r = refusal {
                    Text(T(r)).font(PresenterPalette.ui(13, .bold)).foregroundColor(PresenterPalette.error)
                } else {
                    Text(meta).font(PresenterPalette.ui(13)).foregroundColor(PresenterPalette.fg3)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .background(PresenterPalette.surface)
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(PresenterPalette.divider, lineWidth: 1))
        .clipShape(RoundedRectangle(cornerRadius: 14))
    }

    private var meta: String {
        switch item.kind {
        case .pdf: return T("shareMetaPdf", shareSizeText(item.size))
        case .image: return T("shareMetaImage", shareSizeText(item.size))
        case .link: return T("shareMetaLink")
        }
    }
}

struct RelayConfirmView: View {
    @ObservedObject var model: ShareModel
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(T("relayConfirmTitle")).font(PresenterPalette.ui(26, .bold))
            Text(T("relayConfirmBodyApp", model.server)).font(PresenterPalette.ui(15)).foregroundColor(PresenterPalette.fg2)
            Text(T("relayConfirmRisk")).font(PresenterPalette.ui(13)).foregroundColor(PresenterPalette.fg3)
            Spacer()
            PrimaryButton(title: T("relayConfirmUse")) { model.useRelay() }
            SecondaryButton(title: T("relayConfirmCancel")) { model.useDefaultRelay() }
        }
        .padding(16)
    }
}

struct WaitingView: View {
    @ObservedObject var model: ShareModel
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(T("waitTitle")).font(PresenterPalette.ui(26, .bold))
            Text(T("waitCheckLabel")).font(PresenterPalette.ui(13, .bold)).foregroundColor(PresenterPalette.fg2)
            Text("\(model.check)").font(PresenterPalette.mono(48, bold: true)).foregroundColor(PresenterPalette.primary)
            Text(T("waitCheckHint")).font(PresenterPalette.ui(13)).foregroundColor(PresenterPalette.fg3)
            Text(T("waitExpires", model.timeoutSecs)).font(PresenterPalette.ui(15)).foregroundColor(PresenterPalette.fg2)
            Spacer()
            SecondaryButton(title: T("waitWithdraw")) { model.stopAndClose() }
        }
        .padding(16)
    }
}

struct PresentingView: View {
    @ObservedObject var model: ShareModel
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(model.item?.kind == .link ? T("presentingLink") : T("presentingFile"))
                .font(PresenterPalette.ui(26, .bold))
            if let room = model.roomName {
                Text(T("presentingWhere", room)).font(PresenterPalette.ui(15)).foregroundColor(PresenterPalette.fg2)
            }
            if let sas = model.sas {
                Text(T("presentingSas", sas)).font(PresenterPalette.ui(13)).foregroundColor(PresenterPalette.fg3)
            }
            if model.item?.kind == .pdf, model.pageCount > 0 {
                Text(T("presentingPageOf", model.pageNumber, model.pageCount)).font(PresenterPalette.ui(15))
                HStack(spacing: 12) {
                    SecondaryButton(title: T("presentingPrevPage")) { model.previousPage() }
                    SecondaryButton(title: T("presentingNextPage")) { model.nextPage() }
                }
            }
            Spacer()
            PrimaryButton(title: T("shareStopClose")) { model.stopAndClose() }
        }
        .padding(16)
    }
}

struct ErrorPageView: View {
    @ObservedObject var model: ShareModel
    let page: ShareErrorPage
    var body: some View {
        let n = page.rawValue.prefix(1).uppercased() + page.rawValue.dropFirst()
        VStack(alignment: .leading, spacing: 14) {
            Disc(symbol: "exclamationmark", color: PresenterPalette.error, fill: PresenterPalette.errorDisc)
            Text(T("error\(n)Title")).font(PresenterPalette.ui(26, .bold))
            Text(T("errorWhy")).font(PresenterPalette.ui(13, .bold)).foregroundColor(PresenterPalette.fg2)
            Text(T("error\(n)Why")).font(PresenterPalette.ui(15)).foregroundColor(PresenterPalette.fg2)
            Text(T("errorNow")).font(PresenterPalette.ui(13, .bold)).foregroundColor(PresenterPalette.fg2)
            Text(T("error\(n)Now")).font(PresenterPalette.ui(15)).foregroundColor(PresenterPalette.fg2)
            Spacer()
            // The primary returns to the share page with the item kept (spec 15.3, 5).
            PrimaryButton(title: T("errorActionRetry")) { model.backToShare() }
            SecondaryButton(title: T("commonCancel")) { model.stopAndClose() }
        }
        .padding(16)
    }
}
