// SPDX-License-Identifier: GPL-3.0-or-later
// Universal Links for Dart (native-apps.md 20.4), MethodChannel `de.cometcast/links`:
//   initialLink     -> String?   the https link that started the app (handed out once)
//   installReferrer -> null      iOS has no install referrer: the app asks to scan again
// Dart receives `link` (String) when a link arrives while the app runs.
//
// Only cometcast.de/app/{CODE} opens the app (Runner.entitlements: associated domains, and the
// apple-app-site-association file on that host, deploy/well-known). Dart decides what a link
// means (code, server, trust); this class only passes https links on.
//
// UNCERTAIN: never compiled or run on a device.

import Flutter
import UIKit

final class LinkBridge: NSObject {
    /// SceneDelegate reaches the bridge through this: the scene connects before the engine
    /// registers the channels, so a launch link waits here.
    static var shared: LinkBridge?
    private static var pendingLaunchLink: String?

    private let channel: FlutterMethodChannel

    init(messenger: FlutterBinaryMessenger) {
        channel = FlutterMethodChannel(name: "de.cometcast/links", binaryMessenger: messenger)
        super.init()
        channel.setMethodCallHandler { [weak self] call, result in
            self?.handle(call, result: result)
        }
        LinkBridge.shared = self
    }

    private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "initialLink":
            let link = LinkBridge.pendingLaunchLink
            LinkBridge.pendingLaunchLink = nil
            result(link)
        case "installReferrer":
            result(nil)
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    /// The https link of a Universal Link activity, or nil for anything else.
    static func link(of activity: NSUserActivity) -> String? {
        guard activity.activityType == NSUserActivityTypeBrowsingWeb,
            let url = activity.webpageURL,
            url.scheme == "https",
            url.absoluteString.count <= 2048
        else { return nil }
        return url.absoluteString
    }

    /// The scene connected with a link (a cold start).
    static func launch(with activities: Set<NSUserActivity>) {
        for a in activities {
            if let l = link(of: a) {
                pendingLaunchLink = l
                return
            }
        }
    }

    /// A link while the app runs.
    static func deliver(_ activity: NSUserActivity) {
        guard let l = link(of: activity) else { return }
        if let bridge = shared {
            bridge.channel.invokeMethod("link", arguments: l)
        } else {
            pendingLaunchLink = l
        }
    }
}
