// SPDX-License-Identifier: GPL-3.0-or-later
// Waiting screen of the receiver. Placeholder only: no networking yet.

import SwiftUI

struct ContentView: View {
    var body: some View {
        VStack(spacing: 40) {
            // Placeholder for the logo (the real image is added via the asset catalog).
            Text("Comet Cast")
                .font(.system(size: 72, weight: .bold))

            VStack(spacing: 12) {
                // LocalizedStringKey lookups: the key is searched in Localizable.xcstrings.
                Text("room_code_label")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                // Placeholder room code; a real one will be generated later.
                Text("000 000")
                    .font(.system(size: 96, weight: .semibold, design: .monospaced))
            }

            Text("waiting_for_sender")
                .font(.title2)

            Text("preview_notice")
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .padding(80)
    }
}

#Preview {
    ContentView()
}
