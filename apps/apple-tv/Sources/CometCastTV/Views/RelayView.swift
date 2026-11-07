// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Server list: each entry shows where it came from. Under an administrator's lock only the managed servers are
// listed and the text field is not offered. A server that is not trusted (announced by a display, or typed here)
// is confirmed once before anything is connected (ConfirmRelayView).

import CometCastCore
import SwiftUI

struct RelayView: View {
    @EnvironmentObject var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var custom = ""
    @State private var error: RelayErrorCode?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                Text("relay.title").font(.largeTitle.bold()).accessibilityAddTraits(.isHeader)
                Text("relay.hint").font(.callout).foregroundStyle(.secondary)
                if model.choice.locked { Text("relay.locked").font(.callout) }
                if model.choice.policyUnusable { Text("relay.policyUnusable").font(.callout).foregroundStyle(.orange) }
                if !model.choice.relays.isEmpty { Text("relay.choose").font(.title3) }
                ForEach(model.choice.relays) { r in
                    Button { model.select(r) } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(r.host).font(.title3)
                                Text(LocalizedStringKey("relay.source.\(r.source.rawValue)")).font(.callout).foregroundStyle(.secondary)
                                if r.insecure { Text("receiver.unencrypted").font(.caption).foregroundStyle(.orange) }
                            }
                            Spacer()
                            if model.choice.selected?.wsUrl == r.wsUrl { Image(systemName: "checkmark.circle.fill").accessibilityLabel(L.s("relay.selected")) }
                        }
                    }
                }
                if !model.choice.locked {
                    Text("relay.customLabel").font(.title3).padding(.top, 20)
                    TextField("wss://relay.example.org", text: $custom)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                    Text("relay.customHint").font(.caption).foregroundStyle(.secondary)
                    Button("relay.customApply") { error = model.useCustomRelay(custom) }
                    if let e = error { Text(LocalizedStringKey("relay.errors.\(e.rawValue)")).font(.callout).foregroundStyle(.orange) }
                }
                Button("relay.done") { dismiss() }.padding(.top, 20)
            }
            .padding(80)
        }
    }
}

struct ConfirmRelayView: View {
    @EnvironmentObject var model: AppModel
    let relay: Relay

    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            Text("relay.confirm.title").font(.largeTitle.bold()).accessibilityAddTraits(.isHeader)
            Text(L.f("relay.confirm.body", relay.host, ConfigStore.defaultCandidates().first.flatMap { CometCastCore.relay(for: $0)?.host } ?? "-")).font(.title3)
            Text(LocalizedStringKey("relay.source.\(relay.source.rawValue)")).font(.callout).foregroundStyle(.secondary)
            if relay.insecure { Text("receiver.unencrypted").font(.callout).foregroundStyle(.orange) }
            Text("relay.confirm.risk").font(.callout)
            HStack(spacing: 40) {
                Button("relay.confirm.cancel") { model.pendingConfirmation = nil }
                Button("relay.confirm.use") { model.confirmPending() }
            }
        }
        .padding(90)
    }
}
