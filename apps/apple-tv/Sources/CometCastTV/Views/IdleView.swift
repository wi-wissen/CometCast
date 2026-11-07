// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Idle screen: what to do, which server is used (and where it came from), the start button, and the translated
// error of the last attempt. No QR code: the sender types the six character room code that the receiver shows
// after "Show room code". Siri Remote: the start button has the default focus; the server button opens the list.

import CometCastCore
import SwiftUI

struct IdleView: View {
    @EnvironmentObject var model: AppModel
    @Namespace private var ns
    @State private var showRelays = false
    @FocusState private var startFocused: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 100) {
            VStack(alignment: .leading, spacing: 28) {
                Text("app.name").font(.system(size: 76, weight: .bold)).accessibilityAddTraits(.isHeader)
                Text("receiver.howTitle").font(.title2).foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 18) {
                    step("1", "receiver.how1")
                    step("2", "receiver.how2")
                    step("3", "receiver.how3")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            VStack(alignment: .leading, spacing: 30) {
                if model.state.phase == .error, let code = model.state.errorCode {
                    // The message is translated here; the server's own text is never shown.
                    VStack(alignment: .leading, spacing: 8) {
                        Label(L.s("receiver.status.error"), systemImage: "exclamationmark.triangle.fill").font(.headline).foregroundStyle(.orange)
                        Text(LocalizedStringKey(errorMessageKey(code))).font(.body)
                    }
                    .padding(24)
                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 20))
                    .accessibilityElement(children: .combine)
                }
                relaySummary
                Button { model.start() } label: {
                    Text(model.state.phase == .error ? "receiver.retry" : "receiver.start").font(.title2).frame(maxWidth: .infinity)
                }
                .focused($startFocused)
                .prefersDefaultFocus(true, in: ns)
                .disabled(model.choice.selected == nil && !model.choice.locked)
                Button { showRelays = true } label: { Text("relay.change").frame(maxWidth: .infinity) }
                if model.discovery.localNetworkDenied {
                    Text(LocalizedStringKey(errorMessageKey("local_network_denied"))).font(.callout).foregroundStyle(.orange)
                }
            }
            .frame(width: 720)
            .focusScope(ns)
        }
        .padding(90)
        .onAppear { startFocused = true }
        .sheet(isPresented: $showRelays) { RelayView() }
    }

    private func step(_ n: String, _ key: LocalizedStringKey) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 20) {
            Text(n).font(.title.bold()).frame(width: 44, height: 44).background(Circle().fill(.tint.opacity(0.25)))
            Text(key).font(.title3)
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder private var relaySummary: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let r = model.choice.selected {
                Text(L.f("receiver.relayUsing", r.host)).font(.title3)
                Text(LocalizedStringKey("relay.source.\(r.source.rawValue)")).font(.callout).foregroundStyle(.secondary)
                if r.insecure { Text("receiver.unencrypted").font(.callout).foregroundStyle(.orange) }
            } else if model.choice.policyUnusable {
                Text("relay.policyUnusable").font(.callout).foregroundStyle(.orange)
            } else {
                Text("relay.noSelection").font(.callout).foregroundStyle(.secondary)
            }
            if model.choice.locked, !model.choice.policyUnusable { Text("relay.locked").font(.callout).foregroundStyle(.secondary) }
            if model.discovery.searching { Text("relay.searching").font(.callout).foregroundStyle(.secondary) }
        }
        .accessibilityElement(children: .combine)
    }
}
