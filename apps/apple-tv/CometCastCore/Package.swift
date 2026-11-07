// swift-tools-version:5.9
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CometCastCore: the platform independent logic of the Apple TV receiver (protocol messages, content
// channel parser and limits, ALTCHA, connection check code, relay rules, state machines). No UIKit,
// SwiftUI, WebRTC or Network.framework imports, so it builds and its tests run on Linux (CI-less, so far
// only checked by hand, see docs/appletv.md). On Apple platforms CryptoKit is used; on Linux
// swift-crypto provides the same API under the module name `Crypto`.
import PackageDescription

let package = Package(
    name: "CometCastCore",
    platforms: [.tvOS(.v17), .iOS(.v16), .macOS(.v13)],
    products: [
        .library(name: "CometCastCore", targets: ["CometCastCore"]),
    ],
    dependencies: [
        // Linux only (tests on a developer machine without a Mac). Pinned exactly.
        .package(url: "https://github.com/apple/swift-crypto.git", exact: "3.10.0"),
    ],
    targets: [
        .target(
            name: "CometCastCore",
            dependencies: [
                .product(name: "Crypto", package: "swift-crypto", condition: .when(platforms: [.linux])),
            ],
            path: "Sources/CometCastCore"
        ),
        .testTarget(
            name: "CometCastCoreTests",
            dependencies: ["CometCastCore"],
            path: "Tests/CometCastCoreTests"
        ),
    ]
)
