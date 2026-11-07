// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Screen selection. idle/error -> IdleView (instructions, server choice, start button, translated error);
// everything else -> ReceiverView (code, status, video, content). Dark mode follows the system: only semantic
// colours and materials are used.

import CometCastCore
import SwiftUI

struct RootView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        ZStack {
            Color.clear.background(.background).ignoresSafeArea()
            if model.state.phase == .idle || model.state.phase == .error {
                IdleView()
            } else {
                ReceiverView()
            }
        }
        .sheet(item: $model.pendingConfirmation) { relay in
            ConfirmRelayView(relay: relay)
        }
    }
}

extension Relay: Identifiable { public var id: String { wsUrl } }
