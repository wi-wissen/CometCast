// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: this file and everything under Sources/CometCastTV was written without Xcode or a tvOS SDK and has
// never been compiled. The platform independent logic it uses (CometCastCore) is tested on Linux.
//
// App entry point of the Apple TV receiver.

import SwiftUI

@main
struct CometCastTVApp: App {
    @StateObject private var model = AppModel()
    @Environment(\.scenePhase) private var scenePhase

    init() {
        // Media links are played without cookies: nothing is stored from a media host, and nothing is sent
        // (UNCERTAIN: AVFoundation uses this shared storage; checked in the device test plan).
        HTTPCookieStorage.shared.cookieAcceptPolicy = .never
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(model)
                .onChange(of: scenePhase) { _, phase in model.scenePhaseChanged(phase) }
        }
    }
}
