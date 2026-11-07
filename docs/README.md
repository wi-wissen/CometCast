# Documentation

| Page | Content |
| --- | --- |
| [measurement-points.md](measurement-points.md) | The 20 beta assumptions, the ten measurement points and the measurement spikes. Most important page: it says what is unverified |
| [architecture.md](architecture.md) | Layers, Rust core crates, platform apps, server, route ladder, decisions, milestones |
| [protocol.md](protocol.md) | The signalling protocol between sender, receiver and server |
| [server.md](server.md) | Operating `cometcast-server`: configuration, open and internal mode, TLS, limits, ALTCHA, TURN, deployment, `allowed_origins` for a web app on another origin |
| [../bins/cometcast-server/README.md](../bins/cometcast-server/README.md) | Server README: quick start, settings table, modes, hardening checklist, troubleshooting |
| [../deploy/README.md](../deploy/README.md) | Deployment guide: Docker Compose, reverse proxy (Caddy, nginx), TLS, coturn, firewall, upgrade |
| [website.md](website.md) | The static website (cometcast.de): layout, GitHub Pages setup the owner does once, DNS, web app and relay hosting |
| [capture.md](capture.md) | Real screen content from the native sender (X11) as H.264: capture, change detection, encoder, key frames, numbers seen, limits, licensing questions |
| [transport.md](transport.md) | The native WebRTC path (`cc-transport`, receiver and sender cores, CLI): what works and what does not |
| [web-m2.md](web-m2.md) | Web app toolbar, image, link and PDF transfer over the control DataChannel (message format, limits, PDF rendering in the browser) |
| [android.md](android.md) | Android and Android TV build (cargo-ndk), TV readiness, screen capture, receiver video, PDF/Office workers, what is untested |
| [android-device-test-plan.md](android-device-test-plan.md) | Checklist for testing the Android app on a phone and a Google TV (what to do, expected results, what to record) |
| [ios.md](ios.md) | iOS and iPadOS: the ReplayKit broadcast extension, why the extension does the network connection, App Group hand-over, memory budget, receiving with VideoToolbox, project generation, signing, App Store notes; written to specification, never compiled |
| [ios-device-test-plan.md](ios-device-test-plan.md) | Checklist for testing the iOS app on an iPhone, an iPad and a Mac with Xcode (build, broadcast, rotation, interruptions, memory soak, receiving, network, store checks) |
| [appletv.md](appletv.md) | The native tvOS receiver: Swift package `CometCastCore` (protocol, content limits, ALTCHA, check code, relay rules), WebRTC package choice with evidence, SwiftUI app, project and release job, limits table, verified versus not |
| [appletv-device-test-plan.md](appletv-device-test-plan.md) | Checklist for testing the Apple TV receiver with an Apple TV, a Mac with Xcode and a sender (what to do, expected results, what to record, pass criteria) |
| [flutter-bridge.md](flutter-bridge.md) | How the Flutter app talks to the Rust core (flutter_rust_bridge), codegen, build integration |
| [airplay.md](airplay.md) | AirPlay receiving through the separate UxPlay process (`cc-airplay`): supervisor, policy, licence boundary, what is unverified |
| [airplay-sender.md](airplay-sender.md) | AirPlay sender without FairPlay (`cc-airplay-sender`): discovery, pairing, media link, photo, the screen as an HLS stream; what was tested against UxPlay, what is unverified; real mirroring is not possible (FairPlay) |
| [miracast.md](miracast.md) | Miracast receiving on Windows 10 and 11 through `MiracastReceiver` (`cc-miracast`, `cc-miracast-ffi`, Flutter screen): design, consent rules, video path, requirements and hints, C interface, limits; built to specification, never run |
| [miracast-device-test-plan.md](miracast-device-test-plan.md) | Checklist for a person with a Windows 11 PC and an Android phone: optional feature, start, senders, PIN modes, video and audio numbers, firewall, adapters; what to record and pass criteria |
| [cast.md](cast.md) | Chromecast / Google Cast compatible sender (`cc-cast`, BETA): what is implemented from which Open Screen files, device-auth rules, what is unverified, fallbacks, legal notes |
| [discovery.md](discovery.md) | How the app finds the server (`cc-discovery`): MDM policy file, DNS-SRV, display announcement over mDNS, built-in list; receiver discovery; trust rules; what is unverified |
| [documents.md](documents.md) | PDF documents (`cc-docs`): render on the receiver with hayro, limits, hostile-input tests, measured numbers, WASM check, what is unverified |
| [i18n.md](i18n.md) | How the bilingual (de/en) UI works |
| [releasing.md](releasing.md) | The release workflow: version input, dry run, signing secrets, what is unsigned, tag protection; every job UNCERTAIN until its first real run |

Other project files: [README](../README.md), [CONTRIBUTING](../CONTRIBUTING.md), [CLA](../CLA.md), [SECURITY](../SECURITY.md), [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md).

Docs are English. Unsure statements are written as "**Uncertain:** ..." and linked to the measurement points.
