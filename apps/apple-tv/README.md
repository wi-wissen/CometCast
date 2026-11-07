# Comet Cast - Apple TV receiver (native SwiftUI, tvOS 17+)

> **UNVERIFIED: written without Xcode, a Mac or an Apple TV.** The platform independent part
> (`CometCastCore`) is tested on Linux; the app target has never been compiled. Read
> [docs/appletv.md](../../docs/appletv.md) first and test with
> [docs/appletv-device-test-plan.md](../../docs/appletv-device-test-plan.md).

A **receiver only** app (License: GPL-3.0-or-later). The Rust core is not used on tvOS: signalling, the content
channel, ALTCHA and the relay rules are implemented in Swift and checked against the web client with shared vectors
(`tests/fixtures`).

## Layout

| Path | What |
| --- | --- |
| `CometCastCore/` | Swift package: protocol messages, content parser and limits, ALTCHA, connection check code, relay and server-list rules, state machines. No Apple-only imports; `swift test` runs on Linux |
| `Sources/CometCastTV/` | SwiftUI app: `AppModel`, `Services/` (socket, HTTP, WebRTC, discovery, decoders, configuration), `Views/` |
| `Sources/CometCastTV/Localizable.xcstrings`, `InfoPlist.xcstrings` | English and German (formal "Sie") |
| `Resources/Info.plist` | `NSLocalNetworkUsageDescription`, `NSBonjourServices`, local networking exception, `CCDefaultRelay` |
| `Assets.xcassets/` | tvOS layered app icon (placeholders from `assets/logo`; see docs/appletv.md) |
| `project.yml` | XcodeGen spec (app and unit-test targets, scheme `CometCastTV`, packages) |

## Tests

```sh
cd CometCastCore && swift test        # Linux or macOS; 62 tests, the same fixtures as the web client
```

## Open it on a Mac (Xcode 15+ and Homebrew)

```sh
brew install xcodegen
cd apps/apple-tv
xcodegen generate          # creates CometCastTV.xcodeproj from project.yml
open CometCastTV.xcodeproj
```

In Xcode: select the CometCastTV target, Signing & Capabilities, choose your Team, pick an Apple TV simulator
(or the paired Apple TV) as destination, press Run (Cmd+R). The first build downloads the 686 MB WebRTC package.
Expect to fix compile errors in the app target on the first build and write them down (the test plan, part A,
lists the likely places).

## Settings you may want

- `CC_DEFAULT_RELAY` in `project.yml` (build setting, empty by default): the server used when nothing else is
  configured, the counterpart of the web app's `VITE_DEFAULT_RELAY`. The built-in default `relay.cometcast.de` does
  not exist yet.
- Managed app configuration (MDM): `servers`, `forbid_default_list`, `forbid_user_override`, `forbid_announcement`
  (see docs/appletv.md, "Servers").

## UNCERTAIN

Everything in `Sources/CometCastTV` and `project.yml`; see "Not verified" in docs/appletv.md.
