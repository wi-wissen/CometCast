# Comet Cast - Apple TV receiver (native SwiftUI, tvOS 17+)

> **UNVERIFIED: written without a Flutter/Xcode toolchain; first build may need fixes.**

No Flutter on Apple TV. This is a small native SwiftUI app that will be a
**receiver only**. License: GPL-3.0-or-later.

## Current state

A waiting screen: logo placeholder text, room code placeholder (`000 000`) and
"Waiting for a sender", translated to English and German through
`Sources/CometCastTV/Localizable.xcstrings` (language follows the system).

## Planned (not built)

- Decode video with VideoToolbox.
- Bonjour advertising + TLS server via `NWListener` (Network framework).
- No background operation on tvOS. Long-running use only in the foreground with
  Single App Mode (AppLock via MDM). This is assumption A13.

## Open it on a Mac (Xcode 15+ and Homebrew)

```sh
brew install xcodegen
cd apps/apple-tv
xcodegen generate          # creates CometCastTV.xcodeproj from project.yml
open CometCastTV.xcodeproj
```

In Xcode: select the CometCastTV target, Signing & Capabilities, choose your
Team, pick an Apple TV simulator as destination, press Run (Cmd+R).
For a real Apple TV: pair it via Xcode > Window > Devices and Simulators.

Without XcodeGen: Xcode > File > New > Project > tvOS > App (SwiftUI), name
`CometCastTV`, delete the generated Swift files and drag in everything from
`Sources/CometCastTV/`.

## Manual steps (not automated)

- Asset catalog: create `Assets.xcassets` in Xcode with an App Icon & Top Shelf
  Image set. Source artwork: `assets/logo/cometcast-app-icon.svg` and
  `cometcast-app-icon-1024.png` (tvOS needs layered icons; make them in Xcode).
- Optional: show the logo in `ContentView.swift` via `Image("Logo")` once added.

## UNCERTAIN

- `Localizable.xcstrings` follows the documented string catalog format
  (`sourceLanguage`, `strings` > key > `localizations` > language > `stringUnit`
  with `state` and `value`, `version` "1.0"). Xcode may reformat it on first open.
- `project.yml` settings (e.g. `INFOPLIST_KEY_UILaunchScreen_Generation`) are untested.
