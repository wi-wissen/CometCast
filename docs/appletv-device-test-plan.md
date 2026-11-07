<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Apple TV device test plan

For a person with **an Apple TV, a Mac with Xcode, and an iPhone or any browser as the sender**. It tests the tvOS receiver of milestone M1 (and the tvOS parts of M2) that was **built to specification without any of this hardware or software**: the SwiftUI app has never been compiled, let alone run. Every expected result is an *expectation*, not a known fact; where the code or Apple's documentation leaves something open it says `UNCERTAIN`. The aim is to find out what is true and to write it down so the docs can change. Background: [appletv.md](appletv.md).

Measurement points served: **A13** (native tvOS receiver, no background operation, AppLock), **A19** and **A20** (new, [measurement-points.md](measurement-points.md)), **A8** (discovery), **A12** (ALTCHA solve time on the oldest device), **A1** (direct versus relayed), **A18** (connection check code), spike 3 (tvOS receiver, LiveKitWebRTC) and spike 7 (PDF on the Apple TV), measurement points 3, 4, 6, 7, 8.

Time: about 4 hours for everything; part A to D (build, first start, pairing, video) takes about 90 minutes if the first build needs few fixes.

## 0. What you need

| Item | Notes |
| --- | --- |
| Apple TV with tvOS 17 or newer (the project's deployment target); ideally two models | Record model, tvOS version, network (Ethernet or Wi-Fi). Any Apple TV 4K from 2017 on can run tvOS 17 |
| Mac with Xcode 15 or newer, Homebrew, `xcodegen` (`brew install xcodegen`) | Record the Xcode version. The release workflow uses the Xcode that comes with the `macos-15` runner |
| An Apple Developer team for signing to a device | Pair the Apple TV in Xcode: Window, Devices and Simulators, "+" for network pairing; the Apple TV shows a code in Settings, Remotes and Devices, Remote App and Devices |
| A Comet Cast server reachable by Apple TV and sender | `cometcast-server` in **internal mode** (no ALTCHA) for the first runs, a second one in **open mode** (ALTCHA) for part I; see [server.md](server.md). Plain `ws://` on the LAN is fine; for TLS use a certificate the Apple TV trusts |
| A sender | The **web app** in Safari on an iPhone (iOS 27 may share the screen; older iOS cannot), or in a desktop browser with screen sharing (Chrome on a laptop is the most mature path), or the native Linux sender. For content tests (E) the web sender's image, link and PDF functions |
| A second sender (optional) | For "a new sender joins after the first left" |
| A stopwatch web page and a phone camera at 60 fps | For latency (section 1) |
| DNS you control (a record `_cometcast._tcp` for part G) and, optionally, an MDM or Apple Configurator | G3, G5 |
| A way to read logs | Xcode: Window, Devices and Simulators, select the Apple TV, "Open Console"; or Console.app with the Apple TV connected. Filter `subsystem:de.cometcast.tv` |

## 1. What to record (for every test)

One table row per run in a file `results/<date>-<device>.md` (not committed; attach to the issue):

`test id | Apple TV model | tvOS | Xcode | build (Debug/Release, commit) | result (pass / fail / partly) | numbers | log file | photo or screenshot | what surprised you`

Numbers that matter and how to get them:

| Number | How |
| --- | --- |
| Time to first picture | Stopwatch from "peer joined" to the first frame (film the TV and the sender together), or the log timestamps of `phase negotiating` and `phase receiving` plus the first `stats` line |
| Glass-to-glass latency | A running millisecond clock on the sender, filmed together with the TV at 60 fps or more; subtract the two readings on a still frame; 5 times, record the median |
| Picture size, frame rate, decoder, route, jitter, round trip | On the receive screen press **Play/Pause**: the statistics overlay shows codec, size, fps, decoder implementation (`VideoToolbox` is the hardware decoder), frames decoded/dropped/key frames, kbit/s, jitter, packets lost, the **route** (direct same network / direct through NAT / relay) and the round trip time. The same lines go to the log as `stats ...` every 2 seconds |
| Memory and CPU | Xcode, Debug Navigator while attached (Run), or Instruments (Allocations, Time Profiler) on the Apple TV; record the peak in MB at the start and after 10 minutes |
| ALTCHA solve time | The log has the phase changes `checking` and `connecting`; the time between them is the solve time (part I2) |
| Crash | Xcode Organizer or the device's crash logs (Window, Devices and Simulators, View Device Logs); always attach |

## A. Build, install, expected first-build problems

The app target was written without a compiler. Plan for fixes; **write every fix down** (file, error, change) because the list tells us which assumptions were wrong. The most likely places:

| Where | Why it may fail |
| --- | --- |
| `project.yml` / `xcodegen generate` | key names (`revision` for a package, scheme `build.targets` with `[test]`) written from memory |
| Swift Package resolution of `LiveKitWebRTC` for tvOS | the package lists iOS, macOS and Catalyst but not tvOS in `platforms`; the 686 MB zip is downloaded; check the checksum in the log |
| `PeerSession.swift` | every `LKRTC...` Swift name was derived from Objective-C headers by hand: `peerConnection(with:constraints:delegate:)`, `LKRTCIceServer(urlStrings:username:credential:)`, `answer(for:)` and `setLocalDescription` as async methods, `rtpReceiverCapabilities(forKind:)`, `kLKRTCMediaStreamTrackKindVideo`, `setCodecPreferences`, `statistics()` and `LKRTCStatistics.values` |
| `VideoSurface.swift` | `LKRTCMTLVideoView` on tvOS (a `UIView`), `track.add(view)` |
| `Discovery.swift` | `NWBrowser` TXT access (`NWTXTRecord.dictionary`), the `dns_sd` callback signature, `kDNSServiceErr_PolicyDenied` for the local network permission |
| Swift concurrency | `MainActor.assumeIsolated`, `@MainActor` classes used from `Timer` and `DispatchQueue` closures; the project is set to Swift 5.9 language mode, so Sendable problems should only warn |
| `ContentDecoders.swift` | `CGPDFDocument` calls, `UIGraphicsImageRenderer` on tvOS |
| `MediaPlayerView.swift` | `AVURLAssetHTTPCookiesKey`, `AVPlayerViewController` on tvOS |
| Asset catalog | the layered icon structure (warnings about the missing Top Shelf image are expected) |

| Id | Steps | Expected | Look for |
| --- | --- | --- | --- |
| A1 | `cd apps/apple-tv/CometCastCore && swift test` on the Mac | 140 tests pass (the Linux result). On the Mac this is the first run of the CryptoKit path and of the CommonCrypto PBKDF2 path | `testPlatformPathEqualsPortable` failing means the CommonCrypto call is wrong (UNCERTAIN, never compiled) |
| A2 | `cd apps/apple-tv && xcodegen generate && open CometCastTV.xcodeproj` | The project opens with the targets `CometCastTV` and `CometCastTVTests`, the scheme `CometCastTV`, and two packages | List of errors |
| A3 | Build for an Apple TV Simulator: `xcodebuild -project CometCastTV.xcodeproj -scheme CometCastTV -destination 'platform=tvOS Simulator,name=Apple TV 4K (3rd generation)' build` | Builds after your fixes | Record the fixes |
| A4 | Run the XCTest bundle in the Simulator (Cmd+U) | The same 140 tests pass. They read `tests/fixtures`, the string catalogs, the app's Swift files and the licence files through `#filePath` | If they cannot find the fixtures: the Simulator's file access; run `swift test` instead and note it |
| A5 | Release archive like the workflow: `xcodebuild archive -project CometCastTV.xcodeproj -scheme CometCastTV -configuration Release -destination 'generic/platform=tvOS' -archivePath /tmp/c.xcarchive CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=""` | Succeeds. This is exactly the unsigned path of the `tvos` job | The embedded `LiveKitWebRTC.framework` is present in the archive (`Frameworks/`); size of the archive |
| A6 | Install on the Apple TV (Run, destination: the paired Apple TV) | The app icon shows with the layered logo; the app starts | **A19 first check:** the app launches at all with the 12 MB tvOS slice. A crash at launch with a dyld message means the framework is not embedded or signed |
| A7 | Check the icon layers: Home screen, focus the icon | Three layers move against each other; no black edges; small and large icon both look right | Replace the placeholder artwork later |
| A8 | `node tools/set-version.mjs 1.2.3 --build-number 5 --root <scratch copy>`, then generate and archive | The archive has version 1.2.3 (5) | `CFBundleShortVersionString` in the archive's Info.plist |

## B. First start, screens, languages (M1)

> **Superseded by section M** (the designed screens): B2 (idle screen), B3 to B7 (the screens are now the wizard, the idle screen with QR code, Settings; there is no "Show room code" button, no server list, the room opens by itself). B1 (local network prompt) still applies: it now appears when the announcement starts or the wizard browses.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| B1 | Launch. Wait for the local network permission prompt | A system prompt with the text "Comet Cast uses the local network to find servers and to receive the screen of a sending device directly." (English system) or the German text. Allow | The prompt may appear only when the browser starts (`NWBrowser`). If you deny it: the idle screen shows the translated notice `errors.local_network_denied`. Settings, Privacy, Local Network lets you allow it later |
| B2 | Look at the idle screen | Title, three steps, the server (host) and its source line, "Show room code" focused, "Change server". Text follows the Apple TV language; German uses "Sie" | Text cut off at TV overscan; line breaks in German; long hosts |
| B3 | Change the Apple TV language to German (Settings, General, Language and Region) and restart the app | Everything is German; the permission text too | Missing keys show as the key itself (`receiver.how1`): record each |
| B4 | Switch appearance: Settings, General, Appearance, Dark / Light | The screens are readable in both; orange notices stay visible | Contrast of secondary text on the material backgrounds |
| B5 | VoiceOver on (Settings, Accessibility): move over the idle screen and, after starting, over the code screen | The room code is read letter by letter ("A B C 2 3 4"); the check code as a whole; the status line is announced when it changes; buttons have names | UNCERTAIN: never tried. Record what is read and what is missing |
| B6 | Siri Remote only (arrows, Select, Menu, Play/Pause): can you reach every button on the idle screen, the server list and the confirm screen? | Yes; the focus is visible; Menu closes sheets | Focus traps; the default focus on "Show room code" |
| B7 | Select "Show room code" (internal-mode server, plain `ws://`) | After a moment the big code appears with "Waiting for a sender…"; the server line is shown; an "unencrypted" note for `ws://` | If an error appears: its translated text and the log line `phase error ...`; part I lists what each code should mean |

## C. Pairing with a room code (M1, A1, A18)

> **Superseded by section M** (the designed screens): C6 and C7 (there is no stop confirmation; Menu leaves the app at Idle, End on the presenting controls ends the session), and every place that says "the code screen": the code is on the idle screen. The connection check code is the second line of the name tag, visible only with the controls.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| C1 | In the web app (Send), choose the same server and type the code | The Apple TV changes to "A sender joined…", then "Receiving the screen." The sender's screen appears | Time to first picture; whether `peer_joined` is handled before the ICE list arrives |
| C2 | While receiving, read the connection check code on the TV and on the sender | Both show the same six digits | **A18**: a mismatch is a bug in the Swift code or in the SDP used (the answer text versus the library's local description). Record both fingerprint lines (not the whole SDP) |
| C3 | Stop the sender (close the tab) | The TV shows "The sender left…" with the same code; the code screen returns | The video track is released (no frozen last frame) |
| C4 | Start a new sender with the same code | It connects again; the picture appears | |
| C5 | A second sender while the first is connected | The server answers `room_full` to the second; the first continues | |
| C6 | On the TV press Menu, then "Stop" | Idle screen; the room is gone (a sender typing the code gets "no room") | |
| C7 | Menu, then "Keep receiving" | Nothing changes | |
| C8 | Type the code in lower case on the sender | Accepted by the sender; the TV unaffected | |

## D. Video (M1, A19, spike 3)

> The statistics are no longer on screen: **Play/Pause is the Menu key now.** In a Debug build the log has a `stats video ...` line every 2 s while a picture is received (codec, size, frame rate, decoder, frames, rate, jitter, route, round trip); filter the console for `stats`. Release builds log no statistics.

Press **Play/Pause** on the receive screen to see the statistics; record them for each test.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| D1 | Web sender, a window or the whole screen, 1080p with a moving picture (a video or scrolling text) | Smooth picture; statistics: codec `video/H264`, decoder `VideoToolbox`, roughly the sender's frame rate | **A19**: if the codec is VP8 or VP9 the H.264 preference did not work (`preferH264`); if the decoder is not VideoToolbox the hardware path is not used |
| D2 | A static screen for 30 seconds, then scroll | The last picture stays; no freeze when scrolling starts | key frame handling |
| D3 | 1440p and 4K sender (a 4K display) | The picture appears; record size, fps, dropped frames, CPU | Apple TV models differ (A12 versus A15) |
| D4 | Portrait sender (a phone window) and resize the shared window while receiving | The picture re-fits (aspect fit), no stretching, no crash | `scaleAspectFit` |
| D5 | 30 minutes of receiving | No freeze, memory flat (Instruments), no thermal drops | Record memory at start and end |
| D6 | Latency (section 1), 5 times | Record the median. No target is promised; "smooth enough to present" | |
| D7 | Bad network: sender on weak Wi-Fi or with the Network Link Conditioner | The picture stalls and recovers with a key frame; counters show lost packets | |
| D8 | Time to first picture after `peer_joined` | Record | Stopwatch or logs |
| D9 | The sender sends audio (screen share with system audio in Chrome) | No sound is expected (audio is not handled); no crash | Honest "not done" check |

## E. Content channel (M2, A20, measurement point 7)

> **Superseded by section M** (the designed screens): E1 to E3, E6, E10 to E12 and E16 where they describe the screens: images and PDF pages have **no Close button** (the presenter stops, or the host presses End; a PDF's pages are turned on the presenter's page); the link card has [Dismiss] or [Play, Dismiss], initial focus Dismiss (Play for a playable media link), no Open; progress is the pill at the bottom centre. See M30 to M40.

Sender: the web app's "Send content" (image, link, PDF). Keep the statistics overlay off for these.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| E1 | Send a PNG (about 200 KB) | The image fills the screen on a dimmed background with a Close button; the video continues behind | Focus on Close; Menu also closes |
| E2 | Send a JPEG of 8 MB (near the 10 MiB limit), a WebP and a GIF | All show (the GIF as its first frame); a progress badge "Receiving … %" shows during the transfer | **UNCERTAIN:** WebP in ImageIO on tvOS; time and memory (record) |
| E3 | Send a file over the limit (11 MiB) | The sender refuses; if forced (CLI), the TV shows the translated "could not be understood" notice and nothing breaks | |
| E4 | Send an image with a lying type: a PNG announced as `image/jpeg` (CLI or a hand-made message) | It shows (the type is taken from the bytes); a text file announced as `image/png` shows "The image could not be decoded…" | |
| E5 | Send a huge image: 12000 x 12000 pixels of a few KB (a repeated-pattern PNG) | "The image has too many pixels…" (64 Mpixel limit), no crash, memory stays flat | decompression bomb |
| E6 | Send a PDF of 3 pages | The first page appears ("Opening PDF…" first), "Page 1 of 3"; left/right on the Siri Remote turn pages; the sender's next/previous commands turn them too | **A20:** render time per page (record), sharpness at 1080p and 4K output, fonts |
| E7 | Send 20 real PDFs (the spike 7 corpus), including a scanned one, one with forms, one with transparency | Pages render without visible errors in 18 of 20 (the plan's proposal); record per file: pages, size, time per page, peak memory | Core Graphics' fidelity; what hayro on the web shows differently |
| E8 | A password-protected PDF; a damaged PDF; a PDF with more than 5000 pages; a PDF over 64 MiB | The matching translated notice ("password protected", "damaged or not supported", "too many pages", refused by the limit) | |
| E9 | A hostile PDF (a deeply nested object tree, a huge page size such as 14400 x 14400 pt) | Either refused (`rejected`) or the 20 s watchdog ends it with "took too long"; the app stays alive | **A20:** in-process rendering cannot be killed. Record whether memory spikes or the system terminates the app |
| E10 | A link `https://example.com/path` | A card "example.com sent a link" with the full address and a Dismiss button, **no open button** | |
| E11 | A media link to an MP4 on a public HTTPS server | The card shows "Play from <host>"; nothing is requested before you select it (check the server log or a proxy: no request until Select) | |
| E12 | Select Play | Playback starts with the system transport controls; Stop returns to the card | Network request: User-Agent, **no `Cookie`**, **no `Referer`** (use a proxy such as mitmproxy on the Mac with the Apple TV using it, or the media server's log). Record the headers: this resolves the UNCERTAIN notes |
| E13 | An HLS link (`.m3u8`), an MP3, an M4A, a WebM, an OGG, a plain `http://` link | HLS, MP3 and M4A play; WebM and OGG show "format not supported"; `http://` to a public host shows the "blocked" text with no Play button | AVFoundation formats |
| E14 | A media link pointing at a LAN IP (`http://192.168.x.x/a.mp4`) and one at `nas.local` | The orange local-address warning shows; playing works (local networking is allowed by ATS) | |
| E15 | A media link that fails (404, a server that resets, a non-media file named `.mp4`) | The translated error text for the kind of failure; "Try again" works | Which failures map to which notice (network, decode, unsupported) |
| E16 | Send a link, then an image, then a PDF, then `stop` from the sender | Each replaces the previous; `stop` clears the content | |
| E17 | A transfer that stalls: start sending a large PDF and cut the sender's network after half | After 30 seconds "The transfer … stopped and was discarded" | stall timeout, polled once a second |
| E18 | A hostile sender (CLI test sender or a fuzzer): oversize announcement, junk text, chunk flood, interleaved transfers | The receiver shows the translated notices, never crashes, memory stays bounded | |

## F. Route: direct versus relayed (A1, measurement points 2 and 6)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| F1 | Sender and Apple TV on the same LAN, server without TURN | Statistics route: **direct, same network**; log `stats ... Route` | |
| F2 | The server hands out STUN/TURN (`/ice-servers`, [server.md](server.md)); same LAN | Direct is still chosen. The log shows the ICE list count (`ice N`) | The ICE list is fetched before the first peer is made |
| F3 | Sender on mobile data, Apple TV on Wi-Fi at another place, server with coturn | Route **direct through NAT** or **through a relay (TURN)**; record which and the time to first picture | A1 |
| F4 | Block direct UDP between them (firewall) | Route **through a relay**; the picture continues | TURN over UDP only was tested on the server side |
| F5 | Server without TURN, different networks | The connection fails after a while with `peer_failed`: "The direct connection to the other device failed. Without STUN/TURN servers this only works in the same local network." | The time until the message |
| F6 | Wi-Fi to Ethernet switch while receiving | Either survives or ends with a translated message | |

## G. Server selection and discovery (A8, measurement point 6)

> **Superseded by section M** (the designed screens): G1, G2, G9, G11 and K4: the server is chosen in the setup wizard (step 3), not on the idle screen, and cannot be changed afterwards without resetting the app's data; the local network refusal has no orange notice any more. G3 to G8, G10 and G12 still apply to what the wizard offers (M5 to M9).

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| G1 | No configuration: idle screen | The server is the built-in default (`relay.cometcast.de`, which does not exist yet) unless the build default `CC_DEFAULT_RELAY` was set; source "Default list". Starting shows `connection_lost` | Set `CC_DEFAULT_RELAY` in `project.yml` to your server and rebuild once |
| G2 | "Change server": type `ws://<LAN IP>:8443` in the field, "Check and use" | The server is selected with source "Entered by you"; Start shows the **confirmation** (host, source, risk text); "Use this server" connects and the server is remembered; the next start does not ask again | |
| G3 | A DNS-SRV record `_cometcast._tcp.<your domain>` (priority 10, weight 5, port 8443, target `relay.<your domain>.`) in the network's DNS; the Apple TV uses that DNS with your domain as search domain | After a few seconds the list shows the server with source "Found in the network (DNS)"; it becomes the pre-fill (the first non-announcement entry) | **UNCERTAIN:** the system may not append the search domain to `_cometcast._tcp`. If nothing is found, try with the full name (change `srvServiceName` in `ServerList.swift` for a test) and record that; test with two domains in the search list |
| G4 | Hostile DNS answers: SRV target `localhost`, `127.0.0.1`, `169.254.169.254`, a name with an underscore, port 0 | Those records are dropped; nothing crashes | |
| G5 | **Managed configuration** (MDM or Apple Configurator): `{"servers": ["wss://relay.example.org:443"], "forbid_user_override": true}` | Only that server is listed; the text field is hidden; "Your administrator has set the server." | **UNCERTAIN:** how the MDM delivers the dictionary (app configuration payload) and under which `UserDefaults` key. If nothing arrives, look at `UserDefaults.standard.dictionary(forKey: "com.apple.configuration.managed")` in the debugger |
| G6 | Managed configuration with a typo (`{"serverz": [...]}`) | Fail closed: no server, the notice "The configuration of your administrator could not be read…" | |
| G7 | Managed `forbid_default_list` and `forbid_announcement` | The default entry and announced entries do not appear | |
| G8 | Announce a receiver in the LAN: `cometcast-cli discover --announce 192.168.1.9:8443` on a Linux machine | After a few seconds an entry "Announced by a display on this network, not verified" shows in the server list; it is **not** pre-selected | NWBrowser with TXT; the first announcement per instance name wins |
| G9 | Select the announced server, Start | The confirmation appears every time until accepted once; it is not saved as "last used" | |
| G10 | Announcement with `srv` pointing to a public IP, to `127.0.0.1`, or `tls=0` for a public name | Ignored | |
| G11 | Deny the local network permission, then open the idle screen | The orange notice about the local network; typed servers still work (connections to LAN addresses may also be blocked: record) | |
| G12 | The same server chosen on the TV, but the sender uses a different one | The sender gets "room not found"; the TV keeps waiting | |

## H. Siri Remote and focus (measurement point 4)

> **Superseded by section M** (the designed screens): H2 to H5: see M12 to M16 and M30 to M40 for the designed key behaviour.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| H1 | Walk through every screen with the remote only, including the text field of the server list and the system keyboard | No screen needs a pointer; the keyboard types; Menu goes back | Focus lost after a state change (for example after `peer_left`) |
| H2 | While receiving, press Select | Nothing happens (the video surface is not interactive) | |
| H3 | While a PDF is shown, press left and right | Pages turn locally; at the ends they stop | `onMoveCommand` |
| H4 | While an image is shown, press Menu | The content closes (a second Menu asks to stop) | |
| H5 | The Close, Play and Dismiss buttons have the focus by default when content arrives | Yes | `prefersDefaultFocus` in a focus scope |
| H6 | Long press and swipes on the touch surface | No crash, no unexpected action | |
| H7 | Use the Remote app on an iPhone or a CEC TV remote instead of the Siri Remote | Same behaviour | |

## I. Errors (A12, measurement point 8)

> The display no longer shows the `errors.<code>` texts or a "Create new room" button: a failed connection shows the offline box on the idle screen and the display tries again by itself (M22 to M26). The server cases of this table are still the way to produce them.

Make the server produce each case ([server.md](server.md) for the limits) and check that the **translated** text appears, the app stays usable and "Create new room" works.

| Id | Steps | Expected message (key) | UNCERTAIN / look for |
| --- | --- | --- | --- |
| I1 | Server in **open mode** (ALTCHA always on) | The state goes `connecting`, `checking`, `connecting`, `waiting_sender`; the room is created | The same connection is kept after `altcha_required` |
| I2 | Same, measure the time of `checking` on each Apple TV model | Record the solve time (cost 1000 is the server default). A12 | On Apple platforms PBKDF2 should use `CCKeyDerivationPBKDF`; optionally compare against the portable path (`PBKDF2.derivePortable`) |
| I3 | A server that asks for a cost of 20 000 (set `altcha.cost`) | `errors.altcha_too_hard`; nothing is solved | |
| I4 | An expired puzzle (a server whose clock is hours ahead, or set the Apple TV date years ahead) | `errors.altcha_expired` | |
| I5 | Too many room creations from this address (loop on the sender side or the CLI) | `errors.rate_limited` | HTTP 429 and WebSocket error both |
| I6 | The address is banned in the server configuration | `errors.banned` or `errors.forbidden` (403 without `Retry-After`) | |
| I7 | Server full (`max_rooms` 1, fill it) | `errors.rate_limited` (the WebSocket says rate_limited for the room cap); HTTP 503 gives `errors.server_full` | |
| I8 | Server down or wrong address | `errors.connection_lost` after the timeout | The time until the message (15 s request timeout) |
| I9 | Kill the server while receiving | `errors.connection_lost`; the screen returns to the idle screen with the message | Video stops |
| I10 | A server with a different protocol version | `errors.proto_mismatch` | |
| I11 | TLS: `wss://` to a server with a certificate from a private CA that is **not** installed on the Apple TV | `connection_lost` (the TLS error is not shown separately); install the CA with a profile and retry: works | A profile for the Apple TV |
| I12 | A relay that sends malformed JSON and unknown message types | Ignored; no crash | |
| I13 | A sender that never joins | The TV waits (no timeout on the TV side; the server closes the idle room after its timeout, which shows as `connection_lost`) | |

## J. Lifecycle and always-on use (A13, measurement point 4)

> J2 and J3 changed: the idle timer is disabled **only while presenting**, never on Idle (the system screensaver may follow its own timer on Idle). J1: the session now ends without a toast when the app goes to the background and a new code is made on return, whether or not someone presented.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| J1 | Receiving, press the TV button (home) and return after 1 minute | The connection is gone; on return the app starts a **new** room with a **new** code automatically (the room was running before) | tvOS may keep the app suspended for a while: record what happens at 10 seconds, 1 minute, 10 minutes |
| J2 | Idle screen: leave for 1 hour | The screen saver may start (the idle timer is enabled when no room is open); return by pressing a button | |
| J3 | Waiting for a sender with the code shown for 30 minutes, then for 3 hours | The screen does **not** sleep while the room is open (`isIdleTimerDisabled`); the server's idle timeout (30 minutes without a sender) ends the room: the TV then shows `connection_lost` | A13: always-on needs a foreground app and a way to restart rooms; record what really happens |
| J4 | **Single App Mode** (AppLock) through MDM or Apple Configurator on a supervised Apple TV | The app stays in the foreground; after a power cycle it comes back; record whether the room restarts by itself (it does not: the idle screen waits for Select) | **A13:** auto-start without a button is not built; record whether it is needed |
| J5 | Network down for a minute while receiving, then up | `connection_lost`; "Create new room" works | |
| J6 | Put the Apple TV to sleep and wake it | Record the app's state | |
| J7 | App switcher: swipe the app away | No crash on relaunch; no stale room | |
| J8 | Change the network (Wi-Fi to Ethernet) while the idle screen is shown | Discovery continues to work | |

## K. Security spot checks

> K1: the log has `phase ...` lines and, in Debug builds, `stats ...` lines; still no room code, SDP, link, file name, ALTCHA payload.

| Id | Steps | Expected | Why |
| --- | --- | --- | --- |
| K1 | Check what the app logs: attach the console while receiving, an image, a link and a PDF | `phase ...` and `stats ...` lines only: no room code, no SDP, no link, no file name, no ALTCHA payload | the logger writes only those fields |
| K2 | A link with a deceptive host: `https://evil.example@good.example/`, bidi override characters, punycode | Credentials are refused; bidi controls are removed from the shown host; punycode hosts are refused | |
| K3 | An image with a lying MIME type, with huge dimensions in the header, truncated, an animated GIF with thousands of frames | See E4 and E5; no crash; the GIF shows its first frame only | |
| K4 | A relay from an announcement is selected, then you press Start | The confirmation first; **no** socket is opened before "Use this server" (check with a network monitor) | |
| K5 | Network capture of the Apple TV at the start (proxy or capture on the router) with the default configuration | Only the chosen server (HTTP for `/ice-servers`; `/altcha/challenge` only after `altcha_required`; the WebSocket), mDNS and DNS-SD queries, DNS; **no analytics, no other hosts** | |
| K6 | Check `HTTPCookieStorage.shared.cookieAcceptPolicy` is `.never` after a media playback | No cookies stored | |

## L. Performance (A19, measurement point 3)

| Id | Steps | Record |
| --- | --- | --- |
| L1 | 1080p30 for 10 minutes on each Apple TV model | CPU (Instruments), memory, dropped frames, decoder name, temperature |
| L2 | 4K sender | the same |
| L3 | 20 images in a row (5 MB each) | peak memory, time per image, no leak (memory returns) |
| L4 | A 200-page PDF, page through 50 pages quickly | time per page, memory, whether the watchdog fires |
| L5 | Start and stop receiving 20 times | memory flat; no thread or socket leak (Instruments, Leaks) |

## M. The designed screens (spec 3 to 6, 11, 14, 15; native-apps.md)

Everything in this section is **UNCERTAIN: the SwiftUI views were never compiled or run.** Expected results come from [design/first-run-and-display.md](design/first-run-and-display.md) and [design/native-apps.md](design/native-apps.md); the questions already asked are in [design/implementation-questions-appletv.md](design/implementation-questions-appletv.md). Take a screenshot of every state at 1920 x 1080 points in Dark, Light and OLED black, English and German (du), and compare with the web display's screenshots of the same state: layout, tokens, type, copy (font rendering may differ, no other difference is accepted). Reset the app's data (delete and reinstall) for M1 to M11.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| **Fonts, tokens, ring** | | | |
| M1 | Launch; look at every screen | Atkinson Hyperlegible Next and Mono are used (not San Francisco) for the Latin text; digits of the code, check number and security code are mono | The five fonts register under their PostScript names; `Font.custom(_:fixedSize:)` falls back silently when a name is wrong (the text then looks like SF) |
| M2 | Move the focus over every kind of control (button, chip, tile, card, text field, licence row) | Exactly one control has the ring: outline 4 in the focus colour, offset 3, glow 9, scale 1.03 in 120 ms (no scale on text fields and rows); **no** tvOS lift, parallax or highlight anywhere | `.focusEffectDisabled()` on every focusable view; a system effect on `TextField` |
| M3 | Settings, Appearance: Dark, Light, OLED black, Automatic; switch the system appearance (Settings, General, Appearance) with Automatic selected | The three palettes match spec 3.2 (colour-pick a few pixels: Dark bg `#0E1630`, Light `#FFFFFF`, OLED `#000000`); Automatic follows the system and never shows OLED black; the selected tile fill is `#3E4053` (Dark), `#D7D3CF` (Light), none in OLED | sRGB, not Display P3; the logo is the light file on Light only |
| M4 | System setting Reduce Motion on | Fades are instant, the ring does not scale | `accessibilityReduceMotion` |
| **Setup wizard (first launch)** | | | |
| M5 | First launch | Welcome card centred vertically, "Set up" focused; no step dots | |
| M6 | Press Select; step 1 | Card top edge at 40 ref (does not move on steps 2 to 4); chips (the first selected), custom field empty, Next focused; Back/Next row | Chips wrap onto two lines in German and stay one row for the focus |
| M7 | Choose a chip, type a name in the field (Select opens the full-screen keyboard), clear it again | Typing deselects the chips; clearing it selects the chip that was selected before typing; after Done on the keyboard the focus is on Next; 32 characters at most | The keyboard flow of `TextField` on tvOS; `onSubmit` |
| M8 | Step 2 | Two cards (Few people / Many people), the selected one with the accent border and tile fill, initial focus on it (Open by default); a focused unselected card does not look selected | |
| M9 | Step 3 on a network with no special server | One card: globe, the join address, "Default server. Nothing to set up."; the buttons [Back, Other address, Looks right], focus on Looks right. With a DNS-SRV record or an announcing in-house box (G3, G8): that address first with its source line; "Other address" cycles, then opens the field; an invalid address shows the info line (no red); Back closes the field with focus on Other address; with a managed `forbid_user_override` there is no Other address and the source says "Set by the administrator" | The late answers of DNS (about 3 s) and the announcement refine the list |
| M10 | Step 4, Finish | Idle screen with the chosen name and mode, the room opens at once; the app remembers: kill it and relaunch, no wizard | |
| M11 | Menu on steps 1 to 4 and on step 0 | Steps 1 to 4: one step back; step 0: the system leaves the app | `onExitCommand(perform: nil)` hands Menu to the system |
| **Idle screen** | | | |
| M12 | Look at the idle screen after the code arrives | Logo 46, room name, mode pill (Open / Approval on / Locked with its icon); step 1 with `cometcast.de/app` (the built-in relay) or the server's host; step 2 with the code as `XXX XXX` (80 mono); the QR box with "Or scan"; footer "No download needed." and the Settings button (gear + word) focused | Margins: top 30, bottom 30, sides 40 ref at 1920 x 1080 (safe area); nothing inside the overscan |
| M13 | Scan the QR code with an iPhone camera | The system chip offers `https://cometcast.de/app/CODE` (or `https://{host}/CODE`); the web page opens with the code filled in | The QR is the app's own generator (level M, version 3 or 4); compare against a code from the web display for the same URL: same modules except possibly the mask |
| M14 | Right after launch (before the code) | The code line is empty but keeps its height, the QR box is empty (no modules, no caption), no spinner | |
| M15 | Switch Wi-Fi off or pull the cable and wait | After about 3 s (the attempt at 3 s failed) the dashed box "No connection to {dom}" with the wifi-off icon and the body text; the QR box stays empty; no flicker for a drop shorter than 3 s; when the network is back the code and QR return by themselves (within 4 s) | The schedule 1 s, 2 s, 3 s, then every 4 s; the log shows the phases |
| M16 | A server that answers `rate_limited` / `banned` / `server_full`; one with another protocol version | Box at once with the busy text, retry after 30 s; box with the "app out of date" text and no retry | |
| M17 | In a network with an in-house box announcement (G8): finish setup with it | `idle.altAddress` "or 192.168.x.x" under the host name and the QR code with that IP | `getaddrinfo` on `.local` names |
| M18 | Locked (Settings): look at the idle screen | Code at 35 %, "Locked right now." under it (indented), QR at 25 % (the Light border stays), caption "Locked" | |
| M19 | Leave the idle screen untouched for 7 minutes | The whole screen fades to 40 % over 2 s; the first press of any key only restores it (200 ms) and does nothing else; a join, a request or a mode change also restores it | Time with a stopwatch; the system screensaver may follow later (no wake lock on Idle) |
| **Settings and About** | | | |
| M20 | On Idle press Play/Pause, and Select on the Settings button | Settings opens over a 55 % scrim, focus on the tile of the current mode; rows [About, Close (Back key)] · [Open, Approval on, Locked] · [Dark, Light, OLED black, Automatic] | Play/Pause is `onPlayPauseCommand` |
| M21 | Choose Approval on, then Locked, then Open; choose an appearance | Applies at once, the dialog stays open; the pill and the description line change; Open and Approval on are remembered after a relaunch, **Locked is not** (a relaunch starts in the last Open or Approval on) | |
| M22 | Menu or Play/Pause while Settings is open | Settings closes, focus on the Settings button | |
| M23 | Select About | The About card: logo, version, GPL, publisher, source, web; [Licences, Close]; focus on Licences; Back closes About and Settings shows with focus on About | The version is `CFBundleShortVersionString` |
| M24 | Licences | Full-canvas viewer: list (300 wide) with "With their own notice" first (Cisco OpenH264, Atkinson Hyperlegible, ...) and "All parts"; the text pane shows the focused entry; Right moves the ring to the pane, Up/Down scroll by half a pane, Left returns; Back closes the viewer | Parsing 1.4 MB off the main thread (time to first list); focus returns to the selected row (`onMoveCommand`) |
| **Join request** | | | |
| M25 | Approval on; a web sender joins | A card over the idle screen: "Request", the name in quotes, "wants to connect", the check number (same as on the sender), Yes focused; Back and Menu do nothing | The accessibility label "Request: “{name}”" |
| M26 | Press Select on Yes within 1 s of the dialog appearing, then after 1 s | Within 1 s nothing happens; after 1 s Yes allows (toast "allowed"); a second request: the dialog shows "+1 waiting" and, after a decision, the next request with focus on Yes and the 1 s guard again | |
| M27 | No; No and block; the same sender tries again | Toasts "denied" / "denied and blocked"; a blocked device is not shown again until End | |
| M28 | A request while Settings is open; a request while a picture is shown | Settings closes (changes stay); over a picture the picture stays visible behind the scrim and the name tag and End are under the scrim | |
| M29 | Let a request wait for the server's timeout (60 s); let the sender withdraw one | Toast "Request from … expired" / "… withdrew the request"; a name of only invisible characters shows "No name" | |
| **Presenting** | | | |
| M30 | A web sender shares a screen | Full-bleed picture on black, aspect fit; **nothing else** at rest; `isIdleTimerDisabled` is true (the screen does not sleep, checked past the screensaver time) and false again on Idle | Wake lock only while presenting |
| M31 | Press any key (swipe, click, Menu, Play/Pause) | The name tag (bottom left, with the security code as the second line) and End (bottom right, stop icon) fade in together (400 ms), End has the ring; **the first input never activates anything**; after 5 s without input they fade out together | The invisible catcher gets the focus again after the fade; nothing else is focusable at rest |
| M32 | Controls shown: arrows, Select, Back, Menu | Arrows keep them (restart 5 s), Select activates End (the session ends: toast "Session ended · new code", Idle with a new code, presenters see "Presentation ended"), Back hides them, Menu does nothing | |
| M33 | Controls hidden: Back; Menu | Both show the controls | |
| M34 | The security code on the name tag vs the sender's page | Same six digits as `ddd ddd`, digits mono | |
| M35 | The presenter stops sharing but stays connected | **Expected by the spec:** the display returns to Idle with the same code. Likely **not** implemented (question B3): the last frame stays until the presenter leaves; record what happens | Detection of the end of the track |
| M36 | The presenter disconnects | Idle, toast "… disconnected", same code | |
| M37 | Send a PNG, a JPEG; a PDF of several pages | The image fills the screen on black, **no Close button**; the PDF page is drawn on black, the presenter's Next/Previous turn pages, the presenter's page shows "Page n of m" (the display sends the page message); while the PDF opens the pill "Opening …" at the bottom centre | The pill is one line, at most 340 ref, the name shortened in the middle keeping the extension; measure the width |
| M38 | A large image transfer | The pill "Receiving {name} … n %" at the bottom centre with a black stage; the screen is kept awake | Question B2 |
| M39 | A damaged PDF, an encrypted PDF, a PDF with a page that cannot be drawn | A toast with the existing text; for one bad page the text in a name-tag-look box in the centre and the document stays | |
| M40 | Send a link, a media link (MP4 on HTTPS), an HLS link, a WebM, a `http://` media link, a LAN IP | The card: section label "{host} wants to open a link", the address in mono, the hint "This screen cannot open web pages…", **no Open button**; [Dismiss] focused for a plain link; [Play, Dismiss] with Play focused for a playable media link; warning lines with the info icon for a LAN address or an unplayable format; Play shows the header "Playing from {host}", [Stop] and the player; Stop, Back and Dismiss close the card; 1 s guard on every button | Back while the AVPlayer has the focus (question C3) |
| **Announcement, languages, lifecycle** | | | |
| M41 | With a presenter app or `cometcast-cli discover` browse the network | The Apple TV appears with its room name only while Idle or Presenting with a code; it disappears (goodbye) in setup, offline, Locked keeps name and mode without a code; a managed `forbid_announcement` stops it | `name` key in the TXT record |
| M42 | System language German | Every string is German in the du form ("Du lässt jede Person einzeln zu."); Settings' Back key text "Schließen (Zurück-Taste)"; long German texts do not clip or overlap (wizard step 3 with a long host, the offline box, the request name) | |
| M43 | Build with `CC_STRINGS_TABLE=Localizable-sie` | German addresses the person with "Sie"; nothing else changes | |
| M44 | Home button while presenting or on Idle, return after a minute | No toast; a new session with a new code | |
| M45 | VoiceOver on | The idle screen reads the room name, mode, address and code; the join request is announced as a modal alert; toasts are announced; the QR code reads "QR: {url}" | |
| M46 | Idle for a long time at 4K output | Layout identical (scale 4); no blur on text | No `.scaleEffect` for the canvas |

## Pass criteria

- **Build (A):** the project generates, builds for the Simulator and as a release archive; the 140 core tests pass on a Mac. Every fix is recorded.
- **M1 on tvOS (B to D):** a sender in the web app reaches the Apple TV by room code on the same LAN, the H.264 picture plays smoothly with the hardware decoder, the check codes match, "sender left" and a new sender work. This confirms **A19** for the pinned library.
- **Content (E):** images, links (no open action) and PDF pages work with the stated limits; no hostile input crashes the app. **A20** is confirmed if the PDFs of the corpus render acceptably and the hostile cases end without the system killing the app; otherwise record the failures and see the fallback below.
- **Servers (G, I):** managed configuration, SRV and announcements behave as the tables say (record which of them do not work on tvOS: those are **A8** facts); every error shows its translated message.
- **Lifecycle (J):** record what really happens; there is no pass or fail, the numbers decide the docs for A13.
- **Designed screens (M):** every state of spec 4 matches the web display's screenshot of the same state in Dark, Light and OLED black, English and German; the focus rows and keys of spec 7.1 work with the Siri Remote and show the spec ring and no tvOS focus effect; the wake lock is held only while presenting; Reduce Motion removes the fades and the ring scale; switching the German set from du to Sie changes every addressing string and nothing else. Every deviation is written down (and may need a designer's answer).

Fallbacks if a part fails: A19 (the library does not run on tvOS): build WebRTC for tvOS ourselves or deliver tvOS later ([appletv.md](appletv.md#video)); A20 (in-process PDF is unsafe or slow): drop PDF on tvOS and show "cannot show documents" (the translated `document_unsupported` notice exists), or render PDFs on the sender; A8 (SRV or mDNS missing on tvOS): the typed server and the managed configuration still work.

## What to send back

The results file (section 1), the fix list from part A, the logs, the proxy captures from E12 and K5, the statistics lines of D1, F1 and F3, and photos of the screens. With them the UNCERTAIN markers in [appletv.md](appletv.md) and in the code can be turned into facts.
