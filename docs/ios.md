<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# iOS and iPadOS (Flutter app and screen broadcast)

Status: milestone M6 on iOS is **complete in code and built to specification, and unverified on any device and in Xcode**. There was no Mac, no Xcode and no iPhone while this was written. Everything that can be written and checked without them exists: a Rust crate with a C ABI for the broadcast extension (tested on Linux with a real server and a real receiver), the Swift sources of the app, of a ReplayKit Broadcast Upload Extension and of a VideoToolbox receiver, an XcodeGen project description, build scripts, translations, tests with fakes, and a test plan for a person with an iPhone, an iPad and a Mac. **No line of Swift has been compiled.** Everything that rests on an assumption is marked `UNCERTAIN` here and in the code; the assumptions that decide whether the design works are measurement point **spike 4** in [measurement-points.md](measurement-points.md) (memory limit of the extension) and the items in [ios-device-test-plan.md](ios-device-test-plan.md).

## What exists

| Part | Where | State |
| --- | --- | --- |
| Sender core with a C ABI (`cc_ios_start`, `cc_ios_set_config`, `cc_ios_push_frame`, `cc_ios_keyframe_requested`, `cc_ios_stop`, status callback), header | `crates/cc-ios`, `crates/cc-ios/include/cc_ios.h` | Built and tested on Linux (unit tests, loopback test with `cc-server` and `cc-receiver`, clippy). Type-checked for the three iOS targets. Never linked into an extension |
| ReplayKit Broadcast Upload Extension (`SampleHandler`, VideoToolbox encoder, frame scaler, status and failure handling, de/en messages) | `apps/flutter/ios/BroadcastExtension`, `apps/flutter/ios/Shared` | Parsed with a Swift grammar (syntax only). Never compiled or run |
| App side of the broadcast (platform channels, App Group hand-over, picker) | `apps/flutter/ios/Runner/Broadcast`, `apps/flutter/lib/core/ios_broadcast.dart`, `lib/sender_controller.dart`, `lib/send_page.dart` | Dart: tested with fake channels. Swift: never compiled |
| Receiver video: VideoToolbox decoder into a Flutter texture | `apps/flutter/ios/Runner/Video` | Same channel protocol as the Android bridge. Never compiled |
| Project description | `apps/flutter/ios/project.yml` (XcodeGen), `apps/flutter/ios/Config/CometCast.xcconfig` | Parsed as YAML. Never given to XcodeGen |
| Build scripts | `tools/ios-bootstrap.sh`, `tools/ios-build-rust.sh`, `tools/ios-check-no-sdk.sh` | Syntax- and shellcheck-clean; only `ios-check-no-sdk.sh` was run (on Linux) |
| Info.plist, entitlements, privacy manifests, `InfoPlist.strings`, `Localizable.strings` (de, en) | `apps/flutter/ios/Runner`, `.../BroadcastExtension` | Parsed with `plistlib` and a regex check; never seen by Xcode |
| Release workflow (iOS job) | `.github/workflows/release.yml` | Changed to build the core and the extension; **never run** |
| Test plan | [ios-device-test-plan.md](ios-device-test-plan.md) | Never run |

## Architecture: where the network connection lives

The question: ReplayKit hands the screen to a **Broadcast Upload Extension**, a separate process with a memory limit of roughly 50 MB (a statement of Apple engineers in the developer forums, not documented; UNCERTAIN). Who talks to the network?

**Design A (chosen): the extension does everything.** The extension links `libcc_ios.a` (the sender core: signalling, WebRTC, RTP packetiser) and joins the room itself. VideoToolbox encodes in the extension, the extension pushes finished H.264 into the Rust sender. The app only hands over the room code and server, opens the system sheet and shows the status.

```
 App (Flutter, may be suspended)                      Extension (own process, ~50 MB limit)
 +-----------------------------+                      +----------------------------------------+
 | Send screen                 |  config.json         | SampleHandler  <- ReplayKit sample     |
 |  room code, server --------------> App Group ------>|  FrameScaler (Core Image, pool of 4)   |
 |  RPSystemBroadcastPicker    |  status.json         |  H264Encoder (VideoToolbox, hardware)  |
 |  status text  <-------------------- App Group <-----|  AnnexB -> cc_ios_push_frame           |
 |  Darwin notifications <---------------------------->|  libcc_ios.a (tokio, WebRTC) ---------> network
 +-----------------------------+                      +----------------------------------------+
```

**Design B (rejected as the primary design): the extension only encodes and passes the units to the app**, and the app keeps the network. iOS suspends an app that is not in the foreground; a broadcast is by definition started while another app (or the home screen) is in front. A suspended app cannot send, and a background mode that would keep it alive (audio, VoIP) is not available for this purpose and would not pass App Review. B therefore only works while Comet Cast is in the foreground, which is exactly the case in which nobody needs a broadcast. This is the blocking reason; it was not measured, it follows from the documented iOS app life cycle.

**The risk of A** is the memory limit: a tokio runtime, a WebRTC stack (DTLS, SRTP, ICE) and a VideoToolbox session must fit with the frame pool and the system's own overhead into about 50 MB. Nobody has measured it for this code. The measures taken are in "Memory budget"; the measurement is test item **M1** of the test plan. If it does not fit, the fallback ladder (below) applies, and only then B-like designs come into question (for example: a smaller sender without tokio for the extension, written without the `webrtc` crate; that is a larger piece of work and deliberately not started).

### Fallback ladder if the extension is killed (decide with numbers from M1)

1. Lower `maxDimension` (1280 to 960 or 720), `fps` (30 to 15), bitrate. These are fields of `config.json`, no code change.
2. Replace Core Image by `VTPixelTransferSession` (scale) and `VTPixelRotationSession` (rotate, iOS 16+). Comment in `FrameScaler.swift`.
3. Pool size 4 to 3, `maxInFlight` 2 to 1 (`FrameScaler.poolSize`, `H264Encoder.maxInFlight`).
4. Smaller Rust build for the extension: `opt-level = "z"`, `panic = "abort"`, drop the `ring` crypto features that are not needed. Check `cargo tree -p cc-ios`.
5. Rewrite the extension's network path without tokio/`webrtc`. Large; only with a measured need.

## Hand-over through the App Group

App and extension exchange three files in the App Group container (`<container>/broadcast/`), written atomically (temporary file, rename), and wake each other with **Darwin notifications** (no payload; the receiver reads the file). Notifications are not delivered to a suspended app, so the app also reads the status when it becomes active and on a 2 s timer while it is in front.

| File | Writer | Content | Rule |
| --- | --- | --- | --- |
| `config.json` | app (`prepare`) | room code, server, size/fps/bit rate limits, request time, the person's `name` and the single-use hand-over token `handover` (both optional) | The extension **deletes it after reading** and refuses one older than 5 minutes (`configExpired`). A broadcast started from Control Center without the app finds none (`noConfig`) and says what to do. Values are clamped (320..1920 px, 5..30 fps, 0.2..8 Mbit/s) |
| `status.json` | extension | state (`pending` while the host decides), error, counters, picture size, free memory (`os_proc_available_memory`), time, sequence number, and optional `check`, `timeoutSecs`, `roomName`, `sas`, `nextHandover` | At least once per second while it runs. The app calls a status older than 5 s **lost** (the extension was killed, probably by the memory limit) |
| `stop` | app | empty flag | The extension watches the Darwin notification `<group>.broadcast.stop`, ends with `stoppedByApp` |

The room code is a secret that decides who sees the screen: the file lives in the App Group container (readable by the app and the extension only), is deleted after use, and is never logged (the log file in the container, `cc-ios.log`, 64 KB, two files, contains states and error texts but not the code). The App Group needs **no** network entitlement.

Names and raw values are shared between Swift and Dart; a Dart test pins them (`test/ios_broadcast_test.dart`, "the Dart enums match the Swift raw values"), and the Rust numbers are pinned by `abi_numbers_are_stable` in `cc-ios`.

**Stopping.** iOS has no API for an app to stop a broadcast. The extension can end itself with `finishBroadcastWithError`, which shows the error text in an alert (UNCERTAIN: whether iOS shows the alert for `stoppedByApp` or ends silently, test item **B8**). The person can always stop with the red status bar or Control Center. Leaving the Send screen stops the broadcast (like the Android capture).

### Hand-over of the approval (N32, native-apps.md 20.5)

The extension is a second connection of the same person, and the host approves a person once. The app therefore hands its approval over with a **single-use token** (128 random bits, base64url, made by the app with `Random.secure`, valid 60 s on the display):

1. Screen is tapped, the app asks the display over the content control channel (`{"kind":"handover","token":T}`; core: `request_handover`, event `HandoverResult`), then leaves the room. It writes the room code, the server, the **name** and `T` into `config.json` (`name`, `handover`; both optional, an old file without them still decodes).
2. The extension calls `cc_ios_start(code, relay, dir, name, handover, ...)`. The Rust sender joins with `join_room {name, handover}`; a display that holds `T` lets it in without a dialog and without `toast.left` / `toast.connected` (a display that does not know it, an old server that drops the field, a web display: an ordinary request, the **fallback** below).
3. While it streams the extension writes `room_name` and `sas` to `status.json`; the app shows the Presenting page (`presenting.where`, `presenting.sas`) from them.
4. Before the extension leaves (red status pill, Control Center, or **Stop** in the app) it makes a new token and calls `cc_ios_request_handover(token)` (blocks up to about four seconds, over the control channel of its media connection); on success it writes it as `nextHandover` to `status.json`. The app, when active again, joins with it and lands on "What to show", connected, without a dialog.

**Fallbacks (no new copy):** a join that is not let in at once is reported by the extension as state `pending` with `check` and `timeoutSecs` (the same number the display shows); the app shows the waiting page with them. `join_denied`, `approval_timeout` and `room_locked` come back as the extension errors `joinDenied`, `approvalTimeout`, `roomLocked` and show the normal error pages of the app (Locked refuses hand-over joins). If the rejoin after the broadcast fails or there is no `nextHandover`, the app joins with the code like Connect. If the person cancels the system sheet, the app joins again with its own token (still valid, so no dialog).

**Test builds only:** option (a) of the F2 answer (no hand-over; the extension joins like any sender) is switched on with `--dart-define=COMETCAST_IOS_OPTION_A=true`. A release build ignores the define (`iosOptionAAllowed`, tested), so it cannot be turned on in a shipped app.

C ABI **version 2** (`crates/cc-ios/include/cc_ios.h`): `cc_ios_start` has two new strings (`name`, `handover`), `cc_ios_request_handover` is new, `CcIosStatus` has `check`, `timeout_secs`, `room_name`, `sas` (88 bytes on 64-bit), the state `CC_IOS_STATE_PENDING` (7) and the error numbers 9 to 11 are new. Tests: `cargo test -p cc-ios` (the loopback test of the extension engine with a real display), `BroadcastTests.swift` (token format, config and status with and without the new fields; never run, no Xcode here).

## State machine

Extension (written to `status.json`), with the Rust states of `cc_ios.h` that cause them:

```
            cc_ios_start                      joined                 DTLS/ICE up
 starting ----------------> connecting ----------------> waiting ---------------> streaming <--+
                                |                           |                        |   |      | connected again
                                | server unreachable /      | receiver left          |   v      |
                                | room not found            v                        | reconnecting
                                v                          ended (peerLeft)         | (ICE disconnected)
                             failed <------------------------------------------------+
                                ^                 connection failed/closed, encoder error, memory, stop by app
 paused <-> (broadcastPaused / broadcastResumed by ReplayKit: calls, other recordings)
```

Every terminal state (`ended`, `failed`) ends the broadcast with `finishBroadcastWithError` and a translated message ("Failure modes"). App side: `SenderController` (`starting` until the extension reports `streaming`, `running`, then `ended` or back to `idle`); the Dart enum `IosBroadcastState` has one more value, `lost`, made by the app.

## What the extension does with a frame

1. `processSampleBuffer`: only `.video`; audio buffers are ignored at once. Nothing happens while the connection is not `streaming` (frames are dropped, not queued).
2. Frame rate limit: ReplayKit may deliver 60 per second; frames closer than 0.9 of the target interval are dropped.
3. `FrameScaler`: Core Image scales to at most `maxDimension` on the long edge (even edges, never scaled up) and applies the orientation that ReplayKit attaches (`RPVideoSampleOrientationKey`), into a pool of four NV12 buffers. **A request beyond the pool fails and the frame is dropped**: this is the backpressure that bounds memory. The pool also holds the last picture for step 6, so ReplayKit's own buffers are never retained.
4. `H264Encoder`: VideoToolbox hardware session; real time, **low-latency rate control** (iOS 14.5+, retried without if refused), **no B-frames** (`AllowFrameReordering = false`, output order is input order), Constrained Baseline (`profile-level-id=42e01f`, what the WebRTC offer announces; `Baseline_AutoLevel` UNCERTAIN: whether the level it picks for 1280x960 is accepted by every receiver; the offer says `level-asymmetry-allowed=1`), CAVLC, key frame at most every 2 s (`MaxKeyFrameIntervalDuration`), average bit rate with a 1.5x cap per second. At most **two frames in flight**; a third returns `.busy` and is dropped. A frame that VideoToolbox drops makes the next one a key frame.
5. Output: VideoToolbox writes **AVCC** (4-byte lengths, SPS/PPS in the format description). The extension converts to **Annex B** (`AnnexB.appendAnnexB`) and puts SPS and PPS in front of every key frame; `cc_ios_set_config` also gets them once per encoder session. Annex B is what `crates/cc-sender/src/external.rs` normalises to anyway; it also accepts AVCC with 4-byte lengths, so a mistake here shows up as `Invalid` or `MissingParameterSets` in the counters, not as garbage on the wire. The conversion and its test vectors are checked in `tools/ios-annexb-selfcheck.py` (a Python port, run on Linux) and in `RunnerTests/BroadcastTests.swift` (never run). No access unit delimiters are written.
6. **Key frame requests** (a receiver joins, sends PLI, or the core dropped frames) come from `cc_ios_keyframe_requested()`, polled before every frame and every 250 ms. ReplayKit sends **no frames while the screen does not change**, so on a request the last picture is encoded again as a key frame (`repeatHeld`). UNCERTAIN: whether a repeated picture with a later time stamp is accepted by VideoToolbox without complaint (test item **B6**).
7. `cc_ios_push_frame` never blocks; the core's queue holds three units; a full queue drops and waits for the next key frame (rules of `ExternalInput`, [android.md](android.md) uses the same).

**Orientation and size changes.** The output size is recomputed per frame from the buffer size and the orientation attachment. A different size rebuilds the compression session (new SPS/PPS) and forces a key frame. Receivers see a resolution change inside the stream; the Rust and OpenH264 receivers and a MediaCodec/VideoToolbox decoder must reconfigure at the new SPS. UNCERTAIN, test items **B4**, **C3**. Which orientation value ReplayKit attaches in landscape on iPhone and iPad (and whether the buffer size itself changes on iPad) is not known to me; `FrameSize.fitted` handles both a swapped buffer and an attachment.

**Audio** is not built (`.audioApp` and `.audioMic` are ignored), like on Android.

## Memory budget

The limit is about 50 MB and iOS kills without warning (UNCERTAIN). The table lists where memory goes, with **guesses, not measurements** (UNCERTAIN; the measurement is test item M1).

| Item | Guess | Why / what limits it |
| --- | --- | --- |
| Pixel pool, 4 NV12 buffers of 1280x720 | 5.5 MB | 1.38 MB each (12 bit per pixel); 1280x960 on a 4:3 iPad: 7.4 MB. Bounded by the pool threshold |
| Core Image context | 5 to 15 MB | The unknown. Plan B: `VTPixelTransferSession` |
| VideoToolbox session | a few MB | Hardware encoder; reference frames are in IOSurface memory, partly outside the count (UNCERTAIN) |
| ReplayKit buffers | counted by iOS or not | Source frames are IOSurface-backed; not copied |
| Rust: tokio (2 workers, 512 KiB stacks), WebRTC, buffers | 5 to 10 MB | Queue of three access units (each at most about the bit rate per second / fps, 4 to 40 KB typical, a key frame several times that); RTP packetiser copies; the unbounded event channel of the sender carries one small event per second |
| Code, symbols, Swift runtime | 5 to 10 MB | The `libcc_ios.a` of this build is 168 MB as an archive of unstripped LTO objects on the build machine; the linked and stripped size was **not** measured |
| Headroom | rest | |

Measures built in: bounded pool and in-flight counters (drops instead of queues), no copies of ReplayKit buffers, default 1280 px / 30 fps / 2.5 Mbit/s, release build of the Rust library always (a debug build would be far too large), `os_proc_available_memory()` read every 250 ms and written to the status (the test plan plots it), **emergency stop** below 6 MB free (`emergencyFreeBytes`, a guess: ends the broadcast with a translated message instead of a silent kill), `DispatchSource` memory pressure: *warning* halves the frame rate and drops the held picture, *critical* ends the broadcast (UNCERTAIN: whether iOS delivers pressure events to an extension before it kills it).

## Backpressure and drops, in one place

| Where | Rule | Counted as |
| --- | --- | --- |
| ReplayKit callback | not streaming: drop. Handled synchronously on the encode queue: ReplayKit waits, nothing queues in the extension | `dropped` (local) |
| Frame rate limit | drop | not counted (by design) |
| Pixel pool | exhausted: drop | `dropped` |
| Encoder | two in flight: drop; encoder drop: next frame is a key frame | `dropped` |
| `cc_ios_push_frame` | queue of 3; full: drop until the next key frame; not streaming: drop and request a key frame | `dropped` (core) |
| Receiver (iOS) | `VideoBridge` keeps at most 4 units; more: drop and `needKeyFrame` | key frame requests |

## Failure modes

| Failure | Detected by | What the person sees | Cleanup |
| --- | --- | --- | --- |
| Broadcast started without the app (Control Center) | no `config.json` | iOS alert: open Comet Cast, enter the code | extension ends |
| Config too old (5 min) | `isFresh` | alert: not started in time | extension ends |
| Room code not valid | `cc_ios_start` returns 1 | alert and app text | ends |
| Room not found | server error `room_not_found` | alert and app text: check the code | ends |
| Server unreachable / timeout | connect error, 10 s join timeout | alert and app text: check the network | ends |
| Receiver leaves | `PeerLeft` | app text "the receiver left" | ends |
| Host removes the sender (join approval, protocol version 2) | `RemovedByHost` | shown like "the receiver left" (no own code yet) | ends |
| Room asks for approval, or denies or locks (join approval) | `cc_ios_start` waits in `Connecting` for the host's decision (up to the server's time limit); a refusal comes back as `Internal` with the server code in the log | generic failure text | ends. **TODO:** the extension sends no sender name and shows no check number; own error codes and texts for denied, locked and timed out are missing |
| Connection fails or closes | state `Failed`/`Closed` | alert and app text | ends. `Disconnected` is shown as "reconnecting" and may recover |
| Encoder fails (session cannot be created, `EncodeFrame` fails, malformed output) | return codes | alert: could not encode | ends |
| Memory low | `os_proc_available_memory` < 6 MB, pressure event | alert: low memory | ends |
| Extension killed by iOS | status file older than 5 s | app text: "ended unexpectedly, maybe low memory" (`lost`) | the app removes the hand-over files (`stop`) |
| Phone call, other recording | `broadcastPaused` | app text "paused" | resumes with a key frame |
| App Group missing (signing) | `containerURL` is nil | `prepare` fails: "App Group container not available" | nothing starts. Technical text, a build problem |
| Picker cannot be opened from the app | no button found in `RPSystemBroadcastPickerView` | app shows the Control Center instructions; the hand-over is done | the person starts from Control Center |

The messages of the extension are in `BroadcastExtension/*.lproj/Localizable.strings` (iOS picks the system language); the app's own texts are ARB keys `iosError*`, `iosBroadcast*`, `sendShareScreenHintIos`. No technical terms in either.

**Starting the sheet.** `RPSystemBroadcastPickerView` is a button that the person must tap. The app places one in the window and sends a tap to its button (a widely used workaround that Apple does not document; UNCERTAIN: iOS version dependent, test item **B1**). If the button is not found, Dart shows the Control Center instructions; the hand-over works the same way from there.

## Receiving on iOS

The Rust receiver runs in the app (the receiver does not need the extension). Decoding: the core's **pass-through mode** (`set_video_passthrough`, `take_encoded_video`, the design of the Android MediaCodec path, [android.md](android.md#receiver-video-software-or-mediacodec-decision)) hands Annex B units to `VideoBridge.swift`, which builds a `VTDecompressionSession` from the SPS/PPS, converts to AVCC, decodes (hardware) into BGRA pixel buffers and shows them as a Flutter texture. A new SPS/PPS (the sender rotated) builds a new session. Dart reuses `ChannelAndroidVideoDecoder` (same channel names and protocol; the class name is historical).

**There is no software decoder on iOS.** The Rust build for iOS has no default features, so no OpenH264 is compiled in, and the `h264-libloading` mode would have to download and `dlopen` a library at run time, which the App Store does not allow (App Store Review Guideline 2.5.2: no downloaded code that changes the app's features; UNCERTAIN how a decoder library would be judged, but nobody should rely on it). `crates/cc-codec/src/table.rs` correctly has no iOS entry. Consequences, all implemented:

* the codec download card never appears on iOS (`CodecController(needed: false)` because a platform decoder is set); the receive screen says "The video is decoded by your device. No extra video codec is needed." (`codecNotNeededDevice`);
* the Rust side reports the codec as unsupported (`codec_state` returns `Unsupported`), which the screens do not show on iOS;
* if the VideoToolbox decoder fails (or cannot start), the receiver cannot fall back; it shows `videoDecoderFailedNoFallback`;
* the built-in VP8 test video is only counted (no VP8 decoder), as before;
* screen sharing from iOS to another iOS device works through the receiver's VideoToolbox decoder.

H.264 only: a sender that sends something else is shown the existing "this version only shows H.264" note.

## Local network, Bonjour, multicast

`Info.plist` of the app: `NSLocalNetworkUsageDescription` (translated in `InfoPlist.strings`, de and en; since the list of screens on the network it is the text of `ios.localNetworkUsage`, spec 14.10: "Comet Cast looks for screens on the same Wi-Fi so you do not have to type a code.") and `NSBonjourServices` = `_cometcast._tcp` (the service type of `cc-discovery`). Camera, microphone, photos and location are **not** requested.

Open points, all UNCERTAIN and in the test plan (section D):

* **The extension and the Local Network permission.** WebRTC host candidates and a direct connection to a receiver on the same Wi-Fi need the permission. The prompt cannot be shown by an extension. I believe the grant belongs to the app (and applies to its extension), but did not find that stated in Apple's documentation. Mitigation built in: none in code; the app triggers the prompt through discovery (the Send screen browses for receivers). If the extension is denied, the symptom is "waiting for the receiver" with a LAN receiver but success through the server's TURN relay. Test item **D2**.
* **mDNS on iOS needs the multicast entitlement.** `cc-discovery` uses the `mdns-sd` crate (raw multicast UDP sockets). Since iOS 14 an app that does multicast or broadcast networking with its own sockets needs the entitlement `com.apple.developer.networking.multicast`, which Apple grants on request. **It is not in the entitlements file** because a profile without it would make signing fail. Without it, receiver discovery and announcing on iOS will probably not work and the Send screen shows "no receivers found". The room code path and the server path are not affected. The proper fix is a small Swift `NWBrowser`/`NWListener` bridge (as [architecture.md](architecture.md) already says for Apple platforms). **Since the list of screens on the network the browse half is built:** `ios/Runner/Nearby/NearbyBridge.swift` browses with `NWBrowser` (`.bonjourWithTXTRecord`, no multicast entitlement needed) and hands the raw TXT records to the Rust core over the channel `de.cometcast/nearby`; `kDNSServiceErr_PolicyDenied` in the browser state is reported as the permission state `denied` (page state "Not allowed", link to `UIApplication.openSettingsURLString`); `NWPathMonitor` without `.wifi` or `.wiredEthernet` is "No local network". iOS has no call that reads the Local Network permission, so the bridge reports `notAsked` until a search was started once (a flag in `UserDefaults`); the first search, started by the person's tap on `nearby.search`, is what makes the system ask. The browser is started again every 10 s so that a display that refreshes without changing its record stays in the list (a service that is not seen any more expires in the core after 30 s). **Not built:** the announcement of an iPad in the display role with `NWListener` (displays announce through `mdns-sd` in the core, which needs the entitlement, so an iPad display is probably not found); Apple TV announces as before. Swift syntax-checked only (tree-sitter), never compiled. Test items **D1**, **D9** to **D12**.
* ATS: the app uses its own sockets (Rust), not `URLSession`, so App Transport Security does not apply to `ws://` development servers (UNCERTAIN; same question as Android E4).

## URL scheme and universal links (placeholders)

* Custom scheme: `cometcast://` is declared (`CFBundleURLTypes`, from `COMETCAST_URL_SCHEME`). **No code handles it yet.** Intended use: `cometcast://send?code=ABC234&server=...` opens the Send screen with the code filled in (a QR code on the receiver could carry it). Needs: a handler in `AppDelegate` (`application(_:open:options:)`) handing the URL to Dart.
* Universal links: not configured. Needs the Associated Domains capability (`applinks:cometcast.de`), an `apple-app-site-association` file on the website ([website.md](website.md)), and the `com.apple.developer.associated-domains` entitlement in `Runner.entitlements` and in the App ID. Then `https://cometcast.de/join?code=...` can open the app.

Both are product decisions; the placeholders cost nothing.

## Project generation

The committed Flutter project (`Runner.xcodeproj`) was **removed**; `apps/flutter/ios/project.yml` is the source and `Runner.xcodeproj` is generated by XcodeGen and ignored by git. The same approach as `apps/apple-tv` (and the same caveat: the key names follow the XcodeGen documentation from memory, nobody ran XcodeGen). A hand-edited `project.pbxproj` with a second target was the alternative; without Xcode it would be more fragile than a spec file.

Targets: `Runner` (the Flutter app, with Flutter's two build phases and the scheme pre-action copied from the old project), `BroadcastExtension`, `RunnerTests`. Identifiers live in **one** place, `apps/flutter/ios/Config/CometCast.xcconfig` (bundle id `de.cometcast.cometcast`, extension `...broadcast`, App Group `group.de.cometcast.cometcast`, display name, deployment target 15.0, URL scheme, signing). `Debug.xcconfig` and `Release.xcconfig` include it; the plists and entitlements read the values through build settings (`$(COMETCAST_APP_GROUP)` etc.), and Swift reads them from `Info.plist` (`CometCastAppGroup`, `CometCastExtensionBundleId`). `Config/Local.xcconfig` (not committed) holds the team id.

On a Mac: `tools/ios-bootstrap.sh` (needs Xcode, Flutter, rustup, `brew install xcodegen cocoapods`) runs `rustup target add` for the three iOS targets, `flutter config --no-enable-swift-package-manager`, `flutter pub get`, `xcodegen generate` and `flutter build ios --config-only` (which writes the Podfile and runs `pod install`). Why Swift Package Manager is turned off: the old project had Flutter's SwiftPM integration; Flutter edits the template's project file to add it, and a generated project is not the template. Turning it off sends plugins through CocoaPods. This is a **global** Flutter setting (it also affects the macOS project); `COMETCAST_IOS_KEEP_SPM=1` skips it. UNCERTAIN: not tried; if Flutter handles a generated project fine, the line can go.

**Rust in the Xcode build.** Each target has a pre-build script phase that calls `tools/ios-build-rust.sh <crate>`: `Runner` builds `cc-ffi` (as a static library through `cargo rustc --crate-type staticlib`, **without default features**, see above), `BroadcastExtension` builds `cc-ios`. Inside Xcode the script builds only the current platform from `PLATFORM_NAME` and `ARCHS` (device: `aarch64-apple-ios`; simulator: `aarch64-apple-ios-sim` and/or `x86_64-apple-ios`, joined with `lipo`) into `apps/flutter/ios/build/rust/<platform>/` and writes `native-static-libs.txt` (the system libraries to link; compare with `OTHER_LDFLAGS` on the first build). With `--xcframework` the script builds all targets and runs `xcodebuild -create-xcframework` for `CcIos.xcframework` (for caching in CI; the Xcode build does not use it). The app links `libcc_ffi.a` with `-force_load`, no dead stripping and `STRIP_STYLE = non-global`, because Dart finds the functions with `DynamicLibrary.process()` (`rust_loader.dart`, `ExternalLibrary.process`): on iOS the core is part of the executable, there is no file to load.

Both libraries are linked into **different processes** (app and extension), so the duplicate Rust code (WebRTC twice) is no symbol conflict; it does add to the download size.

### What was checked without a Mac

`tools/ios-check-no-sdk.sh` runs `cargo check` for `cc-ios` and for `cc-ffi --no-default-features` on `aarch64-apple-ios`, `aarch64-apple-ios-sim` and `x86_64-apple-ios` on Linux. The build scripts of `ring`, `oslog` and `dart-sys` need Apple C headers; the script builds a fake SDK root with a handful of stub headers so that clang can compile them to objects that `cargo check` never links. Result: **the Rust code type-checks for all three targets; nothing was linked and the C code of `ring` was not really built for Apple.** A real `cargo build --release --target aarch64-apple-ios -p cc-ios` (producing an archive without linking, with the same stubs) succeeded and produced the 168 MB archive mentioned above.

**A real finding:** `nix 0.31.3` (pulled in by `rtc-shared`, so by `webrtc`) does **not compile for iOS** with `libc 0.2.191` (`UTUN_OPT_IFNAME` and `SYSPROTO_CONTROL` are `macos`-only in that libc release, while `nix` expects them on every Apple target). `Cargo.lock` therefore pins `libc` to **0.2.189**. If a future `cargo update` moves it back, iOS builds break with those two errors; `tools/ios-check-no-sdk.sh` finds it. The proper fix is upstream (`nix` or `libc`); watch for a `nix` release after 0.31.3.

Feature check: `cargo tree -p cc-ios -e features` shows `cc-media` and the `openh264` crate in the graph (a non-optional dependency of `cc-sender`) **without** the `source` or `libloading` features: no C++ is compiled and no library is loaded. `cc-ffi` for iOS uses `--no-default-features`, the same.

## Signing and provisioning

Everything needs a paid Apple Developer Program membership. Create (names follow `Config/CometCast.xcconfig`; change them there first if you want other ones):

| What | Value | Capabilities |
| --- | --- | --- |
| App ID (explicit) | `de.cometcast.cometcast` | App Groups. (Not: Push, Background Modes, Associated Domains (placeholder), Multicast Networking (see above, needs a request to Apple)) |
| App ID (explicit) | `de.cometcast.cometcast.broadcast` | App Groups |
| App Group | `group.de.cometcast.cometcast` | enabled on **both** App IDs |
| Distribution certificate | Apple Distribution (the one `IOS_CERT_P12_BASE64` carries; the tvOS job reuses it) | |
| Provisioning profile, app | App Store (or Ad Hoc), for the app ID, with the certificate and the App Group | secret `IOS_PROVISIONING_PROFILE_BASE64` |
| Provisioning profile, extension | the same type, for the extension's App ID | **new** secret `IOS_EXTENSION_PROVISIONING_PROFILE_BASE64` |

Local development: put `COMETCAST_DEVELOPMENT_TEAM = <team id>` into `apps/flutter/ios/Config/Local.xcconfig`; automatic signing creates development profiles for both targets. A free account cannot use App Groups on a device for long (profiles expire after 7 days) and cannot be used for distribution; UNCERTAIN whether it works at all with the extension.

**Release workflow.** The iOS job in `.github/workflows/release.yml` was changed (mechanically, **never run**): it installs the pinned Rust toolchain with the three iOS targets and XcodeGen, runs `tools/ios-bootstrap.sh`, archives the `Runner` scheme (which builds both Rust libraries in build phases) and, with secrets, writes `Config/Local.xcconfig` for **manual signing with one profile per target**, regenerates the project and runs `flutter build ipa` with an `ExportOptions.plist` that lists both bundle ids. File names carry **`UNVERIFIED`** instead of `NO-CORE` (`tools/release-summary.mjs` lists them as not shippable): the core is now part of the project, but nothing was ever run. Remove the marker only after [ios-device-test-plan.md](ios-device-test-plan.md) passed. UNCERTAIN: the manual-signing settings via `Local.xcconfig`, the profile-name lookup, and `flutter build ipa` with a generated project. Secrets and their effect: [releasing.md](releasing.md).

## App Store review notes

* **Screen recording.** The broadcast is started by the person in the system sheet; iOS shows the red recording indicator and the countdown. The app never starts a broadcast by itself and has no hidden path to do so. Say that in the review notes.
* **Reviewer access.** A reviewer needs a receiver and a server. Provide: a web receiver link and a server address, or a second device, and the steps "Send screen, enter the code, start the broadcast". Without it the app looks broken.
* **Local network.** The usage string says why. Declared Bonjour service: `_cometcast._tcp`. Explain that discovery is optional.
* **Privacy.** No tracking, no developer-side collection; the screen goes to the receiver the user names, directly or through a relay the user (or the organisation) chose. `PrivacyInfo.xcprivacy` exists for the app and the extension; the required-reason categories (SystemBootTime, FileTimestamp, UserDefaults) were written from memory: compare with Xcode's privacy report (UNCERTAIN). The privacy policy and the App Privacy answers are the owner's.
* **Export compliance.** `ITSAppUsesNonExemptEncryption` is **not set**, so App Store Connect asks. The app uses standard protocols (DTLS-SRTP, TLS) through open implementations; whether that qualifies as exempt is a legal question for the owner (UNCERTAIN).
* **No downloaded code.** The decoder is the system's; no OpenH264 on iOS (see above). Guideline 2.5.2.
* **Extension.** A broadcast extension must use ReplayKit the way it is meant to (screen sharing). It does not read other apps' data. It has no UI.
* **Background.** No background modes are requested: the extension runs because ReplayKit runs it.
* **Minimum functionality / content.** The app is a tool; the first screen explains what it does.

## Commands that were run

```sh
# Rust (host), on Linux
cargo test -p cc-ios            # 6 unit tests, 4 loopback tests with cc-server and cc-receiver
cargo clippy -p cc-ios --all-targets
cargo test -p cc-ffi --no-default-features --lib capture_info
tools/ios-check-no-sdk.sh       # cargo check, cc-ios and cc-ffi, three iOS targets, fake SDK headers
cargo build -p cc-ios --release --lib --target aarch64-apple-ios   # with the stub headers: an archive, no link
# Swift and plists, on Linux
python3 tools/ios-annexb-selfcheck.py           # vectors of the byte logic (Python port of the Swift)
python3 tools/ios-swift-syntax-check.py apps/flutter/ios   # tree-sitter grammar: syntax only
python3 -c "import plistlib, yaml ..."          # plists, entitlements, privacy manifests, project.yml
# Dart
cd apps/flutter && flutter analyze && flutter test && node ../../tools/check-i18n.mjs
```

**Not run:** `swiftc`, Xcode, XcodeGen, CocoaPods, `flutter build ios`, `tools/ios-bootstrap.sh`, `tools/ios-build-rust.sh`, a link of `libcc_ios.a` or `libcc_ffi.a`, the iOS job of the release workflow, anything on a simulator or a device. The Swift files were parsed with a Swift grammar for syntax only: no type checking, no SDK symbol check. Expect compile errors on the first build (the Apple API names in `SampleHandler.swift`, `H264Encoder.swift`, `FrameScaler.swift`, `VideoBridge.swift` and `BroadcastBridge.swift` are from memory).

## Open questions (all UNCERTAIN)

1. Does the extension stay below the memory limit with this stack (spike 4, M1)?
2. Core Image or `VTPixelTransferSession`? NV12 render target accepted by `CIContext`?
3. Orientation values and buffer sizes ReplayKit delivers in landscape (iPhone, iPad).
4. Repeating the last picture for key frame requests while the screen is static.
5. Local Network permission for the extension; multicast entitlement for mDNS.
6. Is the programmatic picker tap still working on the current iOS?
7. What iOS shows when the extension ends with the `stoppedByApp` message.
8. `os_proc_available_memory` module/availability, memory pressure events in extensions.
9. XcodeGen key names, Flutter on a generated project (SwiftPM off), `registrar(forPlugin:)` on the implicit engine.
10. Whether a receiver decodes a mid-stream resolution change (the Rust/OpenH264 path and MediaCodec).
11. H.264 level chosen by `Baseline_AutoLevel` for tall iPad pictures.
12. Export compliance and the privacy manifest contents.

## Round 5: the share extension (native-apps.md 21.3, decision N82)

Status: **written to the specification, never compiled or run** (no Xcode). New target `ShareExtension` (`apps/flutter/ios/ShareExtension`, in `project.yml`, bundle id `COMETCAST_SHARE_BUNDLE_ID`): SwiftUI, no Flutter engine, the same `libcc_ios.a` as the broadcast extension. It takes one image, PDF or web link (activation rule in `Info.plist`: a predicate, because a dictionary rule cannot limit a file to PDFs; no Office types, N81), copies a file into `<App Group>/share/items/`, and **joins on its own connection** (name, code, server). In an Approval-on room the host is asked again (the app is not running and cannot hand its join over). The sheet stays open as the remote (page buttons for a PDF); **"Stop and close"**, Cancel and swiping the sheet away all call `cc_ios_share_stop` (the display sees the sender leave).

C ABI **version 3**: `cc_ios_share_start(config_json)`, `cc_ios_share_present(json)`, `cc_ios_share_cmd(json)`, `cc_ios_share_poll(buf, len)`, `cc_ios_share_stop()` (`crates/cc-ios/src/share.rs`, `include/cc_ios.h`). Polling, one JSON line per event: `waiting` (check, timeout_secs, room_name), `joined`, `connection_code`, `page`, `content_prepared`, `content_rejected` (reason), `progress`, `content_sent`, `presenting`, `stopped`, `ended` (code), `error` (code, message). One item per session. Tested on Linux with a real server and a real receiver (`cargo test -p cc-ios`: link, image, refusal, Approval on, stop, buffer too small). Its tokio runtime has 2 MiB thread stacks (512 KiB overflowed in a debug build); stack is address space, only the touched part counts.

App and extension share `<App Group>/share/share-settings.json` (`name`, `confirmed_servers`, `last_server`, written by the app through the channel `de.cometcast/ios_broadcast` methods `writeShareSettings` / `readShareSettings`, `lib/core/ios_share_settings.dart`; the extension writes back only the name). Texts: `ShareExtension/{en,de}.lproj/Localizable.strings`, copied from the shipped ARB set (du) and spec 15.13.

**Round 5 fixes (N116):** the activation predicate no longer matches a file URL (`public.file-url` is excluded in the web-address branch, so a ZIP or a Word file from Files does not list Comet Cast; an image or a PDF still matches by its own branch). The icons of native-apps.md 13 (`file`, `link`, `info`) are vector template images in `ShareExtension/Assets.xcassets`; the error pages of the extension use `info` in the disc. No SF Symbols. Device test items: [ios-device-test-plan.md](ios-device-test-plan.md) section H (including the share sheet screenshots of native-apps.md 21.10 item 9c). **Never compiled or run.**

**Memory measurement (open, test item H5 for a person with the oldest supported iPhone):** send a 64 MiB PDF from the extension and note the free memory (`os_proc_available_memory`) at the start, during and after; the share extension gets far less memory than the app and this was not measured. The file is read once into memory (`read_limited`); if it does not fit, lower the extension's PDF limit (`MAX_DOCUMENT_BYTES` use in `share.rs` and in `ShareViewController.swift`) and keep the existing too-large text. Result: **not measured**.
