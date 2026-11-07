<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# iOS device test plan (iPhone, iPad, Mac with Xcode)

For a person with **one iPhone, one iPad (the larger the better), a Mac with Xcode and a free Wi-Fi network**. It tests the iOS side of milestone M6 (screen broadcast from iPhone and iPad) and the iOS parts of M1 (the Flutter app, receiving) that were built to specification **without a Mac or a device**: nothing in this plan has ever been run, **and the Swift code has never been compiled**, so part A may mostly consist of fixing compile errors (note each one: they show where the code was written from memory). Every expected result below is an expectation, not an observation. Where the real behaviour might differ, the last column says what to look for.

Measurement points served: **spike 4** (iOS broadcast extension: do core, WebRTC and VideoToolbox fit the memory limit, 10 minutes of sending on a large iPad without a crash, peak memory under 40 MB), **spike 0** (Flutter with Rust core on iOS: video surface, share extension is separate), **A2** (Flutter UI with Rust core), **A8** (server discovery on iOS) in [measurement-points.md](measurement-points.md). Background on what was built and why: [ios.md](ios.md).

Time: about 4 hours for everything with one phone and one tablet; parts A, B1 to B3 and M1 alone (build, broadcast, memory) about 90 minutes. **Do M1 first** after the first working broadcast: if the memory limit kills the extension, the rest is moot until [ios.md](ios.md#fallback-ladder-if-the-extension-is-killed-decide-with-numbers-from-m1) was worked through.

## 0. What you need

| Item | Notes |
| --- | --- |
| iPhone and iPad, iOS or iPadOS 15 or newer (the deployment target), better the newest version and one older (16.x) | Record model, exact version, free storage. **Developer Mode** on (Settings, Privacy and Security) |
| Mac with current Xcode, Flutter, rustup, `brew install xcodegen cocoapods` | See [ios.md](ios.md#project-generation). Record Xcode, Flutter, Rust, XcodeGen versions |
| Apple Developer Program membership and the identifiers of [ios.md](ios.md#signing-and-provisioning) | A free account probably cannot do the App Group on a device (UNCERTAIN): note what happens |
| A Comet Cast server reachable by all devices | Your own (`cometcast-server`, [server.md](server.md)) or the default host once it exists. Put its address in the server field. Everything on the same Wi-Fi at first; section D separates the routes |
| A receiver: the web receiver in a browser, or a Linux/desktop receiver | The partner for the sender tests (B) |
| Instruments (comes with Xcode), the Console app | Memory soak (M1), logs |
| A second iPhone or iPad, or a screen-recording tool on a laptop | Receiving tests (C) need a sender (a desktop sender, the web sender, or the other iOS device) |
| A stopwatch web page and a camera that films two screens at 60 fps | Latency (B7, C4) |

Build, install, run:

```sh
tools/ios-bootstrap.sh                               # once, and after every change of project.yml
# put your team id into apps/flutter/ios/Config/Local.xcconfig
open apps/flutter/ios/Runner.xcworkspace             # scheme Runner, your device, Run
# or from the terminal:
cd apps/flutter && flutter run -d <device id> --release     # release: real speed and real memory
flutter build ios --simulator --debug                       # simulator: build check only (ReplayKit broadcast does not run there as on a device: UNCERTAIN)
```

Always test the **release** configuration for memory and speed numbers (M1, B7); the debug build of the Rust core is much bigger and slower (for the extension the script always builds release).

**Debugging the extension:** scheme `BroadcastExtension`, Run, Xcode asks for the host app: choose `Runner`. Start the broadcast as in B1; the debugger attaches when the extension launches. If the extension is killed by the memory limit while attached, Xcode reports "terminated due to memory issue" (jetsam), which is also a result.

Logs, started **before** each test, saved to a file named `<device>-<test>.log`:

* Xcode console for `Runner` (Dart `print` lines and `NSLog` of the extension: filter `cometcast`);
* Console app, device selected, filter process `BroadcastExtension` and `Runner`;
* the extension's own file `cc-ios.log` (states and error texts, 64 KB, no room code) in the App Group container. Get it with `xcrun devicectl device copy from --device <id> --domain-type appGroupDataContainer --domain-identifier group.de.cometcast.cometcast --source broadcast/cc-ios.log --destination <file>` (UNCERTAIN: flags of `devicectl`; on the simulator `xcrun simctl get_app_container booted de.cometcast.cometcast groups`). Also keep `status.json` next to it: it holds the last state, counters and `freeMemoryMB`.

## 1. What to record (for every test)

A table row per test run in a file `results/<date>-<device>.md` (not committed; attach to the issue):

`test id | device | iOS version | build (debug/release, commit) | result (pass / fail / partly) | numbers | log file | screenshot or photo | what surprised you`

Numbers that matter and how to get them:

| Number | How |
| --- | --- |
| Memory of the extension | Instruments, template **Allocations** plus **VM Tracker** (or Activity Monitor), attached to process `BroadcastExtension`: Product, Profile with scheme `BroadcastExtension`. Record the **peak "Memory Footprint"** (the number iOS enforces) and plot `freeMemoryMB` from `status.json`. See M1 |
| Time to first picture | Stopwatch from tapping "Start Broadcast" (the end of the countdown) to the first picture at the receiver |
| Glass-to-glass latency | Show a running millisecond clock on the sending screen, film sender and receiver together at 60 fps or more, subtract the two readings on a still frame. Repeat 5 times, record the median |
| Frame rate at the receiver | The statistics overlay of the receive view (key `I` on desktop; the web receiver's overlay) |
| Sender counters | The Send screen shows frames sent; `status.json` has `sent`, `dropped`, `bytes`, `keyframes`, width and height |
| CPU, battery, heat | Xcode energy gauge or Instruments **Energy Log**; battery percentage over 30 minutes of mirroring |
| Crash | Xcode Organizer crashes, `Settings, Privacy, Analytics, Analytics Data` (look for `JetsamEvent` and `BroadcastExtension` files), always attach |

## A. Build, install, first start

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| A1 | `tools/ios-bootstrap.sh` | Ends with "Done. Next:". `apps/flutter/ios/Runner.xcodeproj` and a `Podfile` exist | XcodeGen key names (spec written from memory), `flutter build ios --config-only` on a generated project, `flutter config --no-enable-swift-package-manager`. Write down every message |
| A2 | Open `Runner.xcworkspace`; build scheme `Runner` for the device | Builds. The "Build Rust core (cc-ffi)" and "(cc-ios)" phases run `cargo` and finish; `apps/flutter/ios/build/rust/iphoneos/` contains `libcc_ffi.a` and `libcc_ios.a` and `native-static-libs.*.txt` | **Expect Swift compile errors** (never compiled) and **link errors for system libraries**: compare `native-static-libs.*.txt` with `OTHER_LDFLAGS` in `project.yml` and add what is missing (`-lresolv`, frameworks). The Rust build also needs `libc` 0.2.189 in `Cargo.lock` (see ios.md) |
| A3 | Install on the iPhone and the iPad; start | Home screen appears. Console: no `UnsatisfiedLinkError`-like Dart FFI error. First start asks for **Local Network** access (answer Allow) | `DynamicLibrary.process()` fails to find symbols (stripped): look for `Failed to lookup symbol` in the log; check `STRIP_STYLE` and `-force_load` |
| A4 | In the app list of Settings, open Comet Cast | Only "Local Network" is offered. No camera, microphone, photos, location | Anything else is a bug |
| A5 | Settings, language to German, start the app again | The app is German. The Local Network dialog text (first start after reinstall) is German. App name under the icon is "Comet Cast" in both | `InfoPlist.strings` not picked up (missing `CFBundleLocalizations`) |
| A6 | Control Center: add "Screen Recording" if missing; long press it | **"Comet Cast"** is in the list of broadcast apps | The extension is not embedded or has the wrong `NSExtension` keys: check `Runner.app/PlugIns/BroadcastExtension.appex` |
| A7 | Archive (Product, Archive), then "Validate App" in Organizer | Validation passes, or lists missing icons, privacy manifest problems, entitlements | Record all messages: input for the store checklist (section G) |
| A8 | `RunnerTests` (Product, Test) | `BroadcastTests` pass (App Group files, freshness, Annex B, frame size) | Compile errors are likely in the first run; a failing test of the byte logic would be a real bug (the same vectors pass in `tools/ios-annexb-selfcheck.py`) |

## B. iPhone or iPad as a sender (screen broadcast, M6)

Receiver for all B tests: the web receiver in a browser, or a desktop receiver. It must show the room code. Use the **release** build for numbers.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| B1 | Receiver: start receiving, read the code. Phone: Send screen, switch on "Share this screen", enter the code, Start | The system sheet "Screen Broadcast" opens with **Comet Cast** preselected (the app text says "Waiting for you to start the broadcast"). Tap "Start Broadcast": countdown 3-2-1, red status bar. Within a few seconds the receiver shows the phone screen. The app (when you return to it) says streaming and counts frames | The programmatic tap on `RPSystemBroadcastPickerView`: if the sheet does not open, the app must show the Control Center instructions (that is test B1b: start from Control Center instead; the hand-over must work the same). Look for the first message of the extension in the log |
| B1b | As B1, but start from Control Center (long press Screen Recording, choose Comet Cast, Start) within 5 minutes after pressing Start in the app | Same result | Config freshness (5 min). After more than 5 minutes the alert must say it was not started in time (B9) |
| B2 | Let it run 60 seconds; scroll, play a video, open another app | The receiver follows. Picture sharp enough to read text. No freeze beyond a second | Static screen: stop touching the phone for 10 seconds, then reload the receiver page (a new receiver joins): **a picture must appear within ~2 s** without touching the phone (repeat of the held picture on key frame requests). If it only appears after you touch the screen, that is the B6 problem |
| B3 | Stop from the app (Stop button on the Send screen) | The red bar disappears. iOS shows an alert or nothing. Receiver says the sender left. App status "stopped" | What iOS shows for `stoppedByApp` (ios.md open question 7). Note it |
| B3b | Start again, stop from the red status bar | The app status becomes "ended" within ~5 s (the extension wrote `ended`) or "ended unexpectedly" (lost) if iOS killed it without a final write | Both are acceptable; record which |
| B4 | Rotate the phone portrait to landscape and back, three times, during streaming | The receiver shows the correct orientation after each change (a new size). The sender counters keep counting. No crash | Orientation value and buffer size that ReplayKit delivers (the code handles an attachment and a swapped buffer). A wrong rotation (picture sideways or stretched) is the likely bug: record the receiver picture, the logged size in `status.json` and the device |
| B4b | The same on the iPad, including upside-down portrait and split view or Stage Manager window resize if available | Picture follows or stays sane | Resolution changes in the stream: receivers must reconfigure at the new SPS (see C3) |
| B5 | Lock the screen for 30 seconds, unlock | Broadcast continues; the receiver shows the last picture while locked (no frames), then updates. The app status stays streaming | Does the extension keep running while locked, and does the connection survive? A drop to "reconnecting" and back is fine; note how long |
| B5b | Incoming call (call the phone from another phone), answer, hang up | During the call the status shows "paused" (the system calls `broadcastPaused`); after it the picture is back within ~3 s (key frame after resume) | If the broadcast ends with the call, note it |
| B5c | Press Home, use other apps for 5 minutes; also switch to Comet Cast and back | Broadcast continues while Comet Cast is in the background; the app shows the right status on return | This is the reason for design A (ios.md): if it stops when the app is suspended, design A failed |
| B6 | Static screen, then join with a **second receiver** (open the receiver page in another browser tab) | The new receiver shows a picture within ~2 s without touching the phone | Repeating the held picture as a key frame (`repeatHeld`): VideoToolbox might reject a repeated picture, or the picture might be missing. Record whether the key frame counter in `status.json` rises |
| B7 | Latency: 5 runs of the stopwatch method | Median glass-to-glass under 500 ms on the same Wi-Fi is a good result; record the number whatever it is | Low-latency rate control (iOS 14.5+) on or off (check the log), level chosen for the picture size |
| B8 | Wi-Fi off during streaming, on again after 20 s | Status "reconnecting" then streaming, or a clear failure message; no hang. Receiver recovers or shows a calm message | Whether `Disconnected` recovers (the transport comment says UNCERTAIN) |
| B9 | Wrong room code | The broadcast starts and the iOS alert says no receiver with that code was found; the app says the same, in the system language / app language | |
| B9b | Server unreachable (wrong address) | Alert: server cannot be reached, within ~15 s | |
| B9c | Start from Control Center without ever pressing Start in the app | Alert: open Comet Cast and start from "Send screen" | `noConfig` path |
| B9d | Press Start in the app, wait 6 minutes, then start the sheet | Alert: not started in time | `configExpired` |
| B10 | Receiver ends the session (closes the page) | The broadcast ends; alert or app text says the receiver left | |
| B11 | German UI and system language: repeat B9 and B9b | Alerts and app texts are German, no raw English technical text | Strings of the extension come from its own `Localizable.strings` |
| B12 | iPad: 1080p-class source (largest iPad), 10 minutes, scrolling content | Stable, no crash. This is the **spike 4** run: see M1 | |

### B13. Hand-over of the approval (N32, [native-apps.md](design/native-apps.md) 20.5)

Receiver: a **native** display (desktop or Android TV app) in **Approval on**, and the same with **Open**. Use the release build. Record the iOS version.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| B13a | Phone: Connect (the host allows once), Screen, confirm "Start Broadcast" | **No second request** on the display, no "connected" or "left" toast. The phone shows the Presenting page with the room name and the security code (the same digits as on the display). The display shows the picture | Whether the extension joined with the token at all (look at `cc-ios.log`: no `pending`). If the host is asked again, record whether the display is old, or the token timed out (60 s) |
| B13b | Stop in the app (and in another run: red status pill, and Control Center) | The phone is back on "What to show", still connected (header with the room name). The display is Idle with the same code, no "left" toast and no request | `nextHandover` in `status.json` appears before `ended`; if the app ends up on the start page instead, the rejoin with the code is the fallback: record why |
| B13c | Then show a file | Works without a new request | |
| B13d | Cancel the system sheet instead of starting | After the app's start timeout the phone is back on "What to show" without a request (its own token is still valid for 60 s); after more than 60 s it joins like Connect (waiting page in Approval on) | |
| B13e | Against an **old server** (or a web display): Connect, Screen | The extension is an ordinary request: the waiting page with the extension's check number and countdown (the same number as on the display), then Presenting after the host allows | Name shown on the display's request is the person's name |
| B13f | Display in **Locked**, then Screen | The error page of a locked room (no hand-over) | |
| B13g | Open the sheet and wait 70 s before tapping Start | An ordinary request (the token expired) | |

### M1. Memory (the decisive test, spike 4)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| M1 | iPhone, release build. Profile scheme `BroadcastExtension` with Allocations + VM Tracker, or attach Instruments to the running `BroadcastExtension` process. Broadcast a busy screen (scrolling a web page, a video) for **3 minutes**. Record peak memory footprint and the lowest `freeMemoryMB` in `status.json` | **Pass: no kill, peak footprint under 40 MB, `freeMemoryMB` never below 10.** Memory flat after the first 30 seconds (no growth: allocate-and-never-free would show as a staircase) | The ~50 MB limit is a developer statement, not documentation. If the extension is killed: record at what footprint and after how long (Jetsam report), then work down the fallback ladder in ios.md, **one step per run**, and record each |
| M2 | iPad (largest), release, **10 minutes**, scrolling content, maximum resolution of the iPad | Same criteria. Spike 4's criterion: 10 minutes without a crash, peak under 40 MB | iPad buffers are larger (2732 x 2048 source): the Core Image scale reads them. Compare the footprint with M1 |
| M3 | Repeat M1 with the Wi-Fi switched off for 60 s in the middle, then on | No growth while disconnected (frames are dropped, not queued) | An unbounded queue would show as growth during the outage |
| M4 | Repeat M1 five times in a row (start, 1 minute, stop) | No growth of the footprint from run to run, no leftover process after the stop | Leaks of the Rust runtime on `cc_ios_stop` (threads, sockets) |
| M5 | Look at `status.json` of M1: `dropped` against `sent` | Few drops on a good network (a few percent at most) | Many drops mean the pool or the encoder is the limit: record |

## C. Receiving on iPhone or iPad (VideoToolbox into a texture)

Partner: a desktop sender (`cometcast-cli send --screen`, or the Linux app), the web sender, or the other iOS device broadcasting.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| C1 | iPad: Receive on this device, start. The room code shows. Start the sender with the code | The sender's screen fills the view, sharp. Statistics overlay works. The text "The video is decoded by your device. No extra video codec is needed." is shown on the form **before** the picture; **no codec download card** at any time | `VideoBridge` (VTDecompressionSession, texture): a black view or `needKeyFrame` loops are the likely first bugs. Look at the log for `VTDecompressionSessionDecodeFrame failed` |
| C2 | Sender: static screen, then switch to a moving one | Updates follow without a long freeze | Decoder output callback and `textureFrameAvailable` |
| C3 | Sender rotates or resizes its window (desktop sender: resize the shared window; iOS sender: B4) | The picture adapts after the next key frame, with the right aspect ratio | New SPS/PPS builds a new `VTDecompressionSession`; a freeze until the next key frame is acceptable, a permanent black view is not |
| C4 | Latency, 5 runs | Median glass-to-glass; record | |
| C5 | Receiver in the background for 30 s (Home), then back | The picture returns, at the latest after a key frame request (`kVTInvalidSessionErr` handled) | iOS invalidates decoder sessions in the background |
| C6 | iPhone/iPad as receiver of a **VP8** test sender (Send screen without sharing: built-in test video, from another device) | The receiver counts packets and says the picture type is not shown (no VP8 decoder); no crash | |
| C7 | Make the decoder fail on purpose (a sender with a broken stream, or a sender that sends something else than H.264) | A translated note ("This device could not decode the video ..." or "only shows H.264"); no crash, no download card | `videoDecoderFailedNoFallback` |
| C8 | iPad receiving the iPhone's broadcast (iOS to iOS), 5 minutes | Works; both sides stay stable. The sender extension's memory unaffected | |
| C9 | Receiver with the toolbar: pen, highlighter, eraser over the picture | Works as on Android and Linux (Flutter code, not iOS specific) | Touch handling on iPad (Apple Pencil is not specifically handled) |

## D. Network: local network permission, discovery, routes

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| D1 | iPhone Send screen with a Comet Cast receiver announcing itself on the same Wi-Fi (Linux or desktop app with the receive screen open) | **Probably the list "Nearby receivers" stays empty** (mDNS through `mdns-sd` raw multicast sockets needs the multicast entitlement on iOS, which is not in the app). Record what happens, and whether the app logs a socket error | If the list works: the entitlement is not needed here, remove the doubt from ios.md. If not: the `NWBrowser` bridge ([ios.md](ios.md#local-network-bonjour-multicast)) becomes a task. Either way the room code path must still work |
| D2 | Settings, Comet Cast, Local Network **off**; broadcast to a receiver on the same Wi-Fi via the server | Record whether the broadcast reaches "streaming" and whether media flows (direct host candidates blocked, relay path may work) | Does the Local Network setting of the app apply to the extension? If the extension is denied silently the symptom is "waiting for the receiver" forever on a LAN receiver |
| D3 | Local Network on again; same test | Streams. Record the ICE route if the receiver shows it (statistics): direct or relayed | |
| D4 | Phone on cellular (Wi-Fi off), receiver on another network, with a TURN-capable server | Streams through the relay, or a calm failure message after the connection timeout | Relay needs the server's TURN ([server.md](server.md)); ICE across NAT is not verified anywhere yet (A1) |
| D5 | Both on the same Wi-Fi, no TURN configured | Direct connection works | |
| D6 | Server over `ws://` on the local network (development) | Works (the Rust core uses its own sockets; App Transport Security should not apply, UNCERTAIN) | Same question as Android E4 |
| D7 | VPN on | Record: works, blocked, or relay | |
| D8 | iPad on a captive-portal or isolated guest Wi-Fi | A clear failure, no hang | |
| D9 | (new) Fresh install, open the app start page (no code from a link) | The section "Screens on this network" shows only the button "Search this network" (state "Not asked yet"); nothing was asked yet. Tap it: the system shows "Comet Cast would like to find and connect to devices on your local network" with the text of `NSLocalNetworkUsageDescription`. Allow: the search runs at once and starts by itself on every later visit | The `UserDefaults` flag of the bridge; that `NWBrowser.start` is what triggers the prompt; the prompt text (en and de) |
| D10 | (new) With a Comet Cast display (desktop app, Idle) on the same Wi-Fi after D9 | The display is listed (quoted room name, mode, its code); a tap joins; a code change, Locked and goodbye on the display update or remove the row (see Android E6) | NWBrowser delivers `.changed` for a changed TXT record? Does it keep a display that died without a goodbye longer than 30 s (the app restarts the browser every 10 s)? |
| D11 | (new) Settings, Comet Cast, Local Network **off**, then open the start page | "Comet Cast may not search this network." with the link "Allow in Settings", which opens the app's page in the Settings app; after turning it on and coming back the search starts again | `kDNSServiceErr_PolicyDenied` reaches the bridge as `.waiting(.dns(...))` or `.failed`? Record which |
| D12 | (new) Cellular only, then Wi-Fi on | "Join the room's Wi-Fi to find screens here." and no search; the search starts by itself when Wi-Fi comes | `NWPathMonitor` with `.wifi` / `.wiredEthernet` |

## E. App life cycle, interruptions, robustness

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| E1 | Force-quit Comet Cast (app switcher) while the broadcast runs | The broadcast keeps running (separate process); reopen the app: it shows idle or the real status, and Stop works | The app does not re-attach to a running broadcast on start (not built): the Send screen starts idle. Record what the person sees and whether Stop from the red bar still works |
| E2 | Start a broadcast, then start a second one from the app | The first ends or the second is refused with a calm message; no crash | `AlreadyRunning` path in `cc-ios` |
| E3 | Start, stop, start, stop 10 times quickly | Each works; no stale state, no leftover files (`config.json` deleted after use, `stop` flag cleared at start) | Race between the extension's teardown and the next start |
| E4 | Airplane mode on during the broadcast, off after 2 minutes | Behaves like B8 | |
| E5 | Low Power Mode on during a 5-minute broadcast | Stable; frame rate may drop | |
| E6 | Other screen recording (Control Center, not Comet Cast) while a Comet Cast broadcast runs | iOS allows only one; record what happens | |
| E7 | Reboot the device with a status file present; open the app | No crash; status idle or stale handled | |
| E8 | Screenshot, AirPlay mirroring on, Guided Access on during a broadcast | Record behaviour; no crash | |

## F. Languages and accessibility

| Id | Steps | Expected |
| --- | --- | --- |
| F1 | German and English: walk through Home, Send, Receive, errors | No missing strings, no English in the German UI, de/en alerts of the extension |
| F2 | VoiceOver on the Send screen | The status text is announced when it changes (live region); buttons have names |
| F3 | Dynamic Type large, dark mode | Layout holds |

## G. Release and store checks (needs the Apple account)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| G1 | Create the App IDs, the App Group, the two profiles as in [ios.md](ios.md#signing-and-provisioning); put team id and profile names into `Local.xcconfig` with manual signing (see the release workflow) | Archive and export succeed | Signing via xcconfig settings (`PROVISIONING_PROFILE_SPECIFIER` per target) |
| G2 | Run the release workflow as a dry run with the iOS secrets set (Environment `release`) | The iOS job builds the archive and the `.ipa` named `...-UNVERIFIED.ipa` | The whole job is untested ([releasing.md](releasing.md)) |
| G3 | Upload to TestFlight; install on a device that never had a debug build | Works; all of A to C again with this build | App Group in a distribution profile |
| G4 | App Store Connect: privacy answers, export compliance, review notes (see ios.md, "App Store review notes") | The review notes and the privacy manifest agree with Xcode's **privacy report** | Required-reason API list in `PrivacyInfo.xcprivacy` (from memory) |
| G5 | Remove the `UNVERIFIED` marker from the file names only after A to C, M1, M2 passed | | |

## H. Share extension (round 5, native-apps.md 21.3 and 21.10 items 7 and 9c; spec 15.3, N107, N116)

**Status: not run** (no Xcode and no device). The extension is written to the specification: activation predicate without `public.file-url` for the web-address branch, the section 13 icons `file`, `link` and `info` as vector template images (`ShareExtension/Assets.xcassets`), no SF Symbols, the address line `start.via` alone, no nearby section, settings in `share/share-settings.json`.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| H1 | In Files share a ZIP, a `.docx` and a video | **Comet Cast is not in the share sheet** for any of them | That the predicate (`UTI-CONFORMS-TO` with `NOT (... "public.file-url")` in the web-address branch) hides it; if it does not, report the registered type identifiers of the item |
| H2 | Share a PDF from Files, a photo from Photos, a web address from Safari | Comet Cast is listed for each; a PDF that also registers a file URL still matches | Whether Files registers `public.file-url` next to `com.adobe.pdf` |
| H3 | Share two items at once | Not listed | |
| H4 | Open the sheet for a PDF (not connected) and while presenting (`share.stopClose`); Light and Dark; English and German | Screenshots of all eight, reviewed against spec 15.3 before release: the icon disc with the `file` icon in `p-primary`, the card, the address line (`via {dom}` only), the buttons | Any difference to spec 15.3, listed |
| H5 | On the oldest supported iPhone: send a 64 MiB PDF from the extension; note `os_proc_available_memory` at the start, during and after | The extension is not killed | The numbers. If it does not fit, lower the extension's PDF limit (`MAX_DOCUMENT_BYTES` in `share.rs` and `ShareViewController.swift`); the refusal keeps the existing too-large text |

## Pass criteria

| Area | Pass |
| --- | --- |
| Build | A1 to A6 pass on both devices; A8 tests pass |
| Broadcast | B1 to B5c pass on iPhone and iPad; B6 shows a picture for a late joiner within ~2 s of a static screen |
| Memory (spike 4) | M1 and M2: no kill, peak footprint under 40 MB; M3 and M4: no growth |
| Receiving | C1 to C3, C5 pass; no codec card anywhere |
| Failures | B9 to B11 give translated messages |
| Network | D3 and D5 pass; D1, D2 and D4 are **recorded**, whatever the result |
| Share extension | H1 to H3 pass; H4 screenshots reviewed; H5 measured and recorded |

Anything marked "UNCERTAIN" in [ios.md](ios.md) that this plan settled should be turned into a statement there, with the device, the version and the number.
