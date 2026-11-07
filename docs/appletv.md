<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Apple TV receiver (native SwiftUI, tvOS 17+)

The Apple TV app is a **receiver only**: it shows a room code, answers the sender's WebRTC offer, plays the H.264 screen stream and shows the content the sender pushes (images, links, PDF pages). It is built **to specification without a Mac, an Xcode or an Apple TV**: the platform independent part is tested on Linux, the app target has never been compiled. Everything that needs the hardware is in [appletv-device-test-plan.md](appletv-device-test-plan.md). Decision (owner): native SwiftUI, the Rust core is **not** used on tvOS (the earlier plan with UniFFI is dropped for this app).

Statements that rest on an assumption are marked **UNCERTAIN** (in the code `// UNCERTAIN:`) and linked to [measurement-points.md](measurement-points.md): assumptions [A13](measurement-points.md#1-beta-assumptions-a1-to-a20) (no background operation), A19 and A20 (new, below), spike 3.

## At a glance

| Part | Where | Status |
| --- | --- | --- |
| Protocol, content parser and limits, ALTCHA, connection check code, relay rules, state machine | `apps/apple-tv/CometCastCore` (Swift package, no Apple-only imports) | **Built and tested on Linux** (Swift 6.0.3, swift-crypto): 62 XCTest cases, same vectors as the web client |
| Shared test vectors | `tests/fixtures/*.json` | Checked by the web client (vitest), by the Rust crate `cc-files` (two tests) and by the Swift package |
| SwiftUI app, WebRTC wrapper, discovery glue, ImageIO/CoreGraphics decoding, AVPlayer | `apps/apple-tv/Sources/CometCastTV` | **Written, syntax-checked only** (`swiftc -parse`); not compiled, not run |
| XcodeGen project, Info.plist, string catalogs, app icon layers | `apps/apple-tv/project.yml`, `Resources/`, `Assets.xcassets` | Written; plist and YAML parse; `xcodegen` never run |
| Release job `tvos` | `.github/workflows/release.yml` | Names match the project (see [below](#project-and-release-workflow)); never run |

## Architecture

```
SwiftUI views ── AppModel (main actor) ── ReceiverMachine (pure state machine, CometCastCore)
                    │  performs the effects:
                    ├─ SignalingSocket      URLSessionWebSocketTask, JSON text frames (docs/protocol.md)
                    ├─ ServerHTTP           GET /altcha/challenge (solved in CometCastCore), GET /ice-servers
                    ├─ PeerSession          LiveKitWebRTC: answer, video track, "control" data channel
                    ├─ Discovery            NWBrowser (mDNS), dns_sd (DNS-SRV)
                    ├─ ControlReceiver      content channel parser and reassembly (CometCastCore)
                    ├─ ImageDecoder         ImageIO
                    └─ PDFRenderer          Core Graphics (CGPDFDocument)
```

`CometCastCore` has no UIKit, SwiftUI, WebRTC or Network imports. It uses CryptoKit on Apple platforms and swift-crypto (pinned 3.10.0) on Linux, so the logic can be tested without a Mac. Modules:

| File | What it holds | Port of |
| --- | --- | --- |
| `JSON.swift` | strict JSON parser and writer (no Foundation `JSONSerialization`, whose behaviour differs between Apple and Linux) | `JSON.parse` |
| `Control.swift` | message parser, limits, name sanitising, chunk reassembly, stall timeout, receiver | `apps/web/src/core/control.ts` |
| `LinkURL.swift` | http(s) URL validation and normalisation (WHATWG rules for the two special schemes) | `validateLinkUrl` |
| `Media.swift` | media link classification, player state, local/IP host rule | `media.ts` |
| `Sas.swift` | connection check code | `sas.ts` |
| `Altcha.swift`, `Pbkdf2.swift` | proof of work solver, PBKDF2 (CommonCrypto on Apple, portable HMAC elsewhere) | `altcha.ts`, `altchaClient.ts` |
| `Signaling.swift` | client/server messages, error codes, ICE candidate JSON, `/ice-servers` parser | `signaling.ts`, `altchaClient.ts` |
| `Relay.swift` | relay address validation | `relay.ts` |
| `ServerList.swift` | managed policy, address rules, merge order, DNS-SRV ordering and rdata, announcement TXT | `crates/cc-discovery` |
| `ReceiverMachine.swift` | connection state machine | `receiverSession.ts` |
| `ContentController.swift` | what is shown, page turning, error mapping, PDF/image limits | `receiverSession.ts` |

## State machine

Phases are the web app's: `idle`, `connecting`, `checking` (ALTCHA), `waiting_sender` (the code is shown), `negotiating`, `receiving`, `peer_left`, `error`. Inputs and effects are plain values (`ReceiverInput`, `ReceiverEffect`), so the tests drive it without sockets.

| Event | Result |
| --- | --- |
| start | `connecting`; open socket; fetch ICE servers; on the server's `hello` send `create_room` without a puzzle |
| `room_created` | `waiting_sender`, show the code |
| `error altcha_required` while `connecting`, first time | `checking`; fetch and solve the puzzle; send `create_room` again **on the same connection** with `altcha` |
| `error altcha_required` again | `error` (no endless loop) |
| `peer_joined` | new peer; `negotiating` |
| `offer` | answer it (the answer is sent when ready); candidates are fed to the peer |
| peer connected | `receiving`; the connection check code is computed from the final SDPs |
| `peer_left` | `peer_left`; peer and content closed; the room and its code stay (a new sender may join) |
| `error not_in_room` after the room exists | ignored |
| any other `error`, socket closed, peer failed, answer failed | `error` with the code; everything torn down |
| messages before the ICE list is known | held (`peer_joined`, `offer`, `ice_candidate`) and processed afterwards |

Error codes and what the person sees (key `errors.<code>`, German and English in `Localizable.xcstrings`; the server's own text is never shown):

| Code | Source | Meaning for the person |
| --- | --- | --- |
| `altcha_required` | server | shown only if a fresh puzzle was refused again |
| `altcha_invalid` | server | the check was not accepted, try again |
| `altcha_failed` | client | puzzle could not be fetched or solved (timeout 60 s, bad answer) |
| `altcha_too_hard` | client | cost over 10 000 or key length over 64: not started |
| `altcha_expired` | client | the puzzle's `expiresAt` was in the past (also: device clock far ahead) |
| `rate_limited` | server (WebSocket or HTTP 429) | too many requests, wait |
| `banned` | server (`banned`, or HTTP 403 with `Retry-After`) | network blocked for a while |
| `forbidden`, `server_full` | HTTP 403 without `Retry-After`, HTTP 503 | not allowed / full |
| `bad_hello`, `proto_mismatch`, `bad_message`, `wrong_role` | server | restart; update the app |
| `connection_lost` | client | socket closed or never opened (also: server close reasons `room_expired`, `idle_timeout`, ...) |
| `peer_failed` | client | WebRTC failed (answer error or connection state `failed`); without STUN/TURN only the same network works |
| `no_server`, `local_network_denied` | client (Apple TV only) | no server selected / the local network permission was refused |
| anything else | | `errors.unknown` |

A test checks that every code the core can produce, every transfer error, phase, relay source and media error has an English and a German entry in the catalog, and that placeholders agree.

## Servers (relays)

Order of sources, the same as the owner's rule in [discovery.md](discovery.md):

1. **Managed app configuration (MDM)**: the `com.apple.configuration.managed` dictionary in `UserDefaults`, with the keys of the desktop policy file (`servers`, `forbid_default_list`, `forbid_user_override`, `forbid_announcement`, `mode`, `version`). Unknown keys or wrong types make the whole configuration invalid and the app **fails closed**: no server, locked, a notice. With `forbid_user_override` only the managed servers are offered and the text field is hidden. **UNCERTAIN:** how a real MDM product delivers this to tvOS (Apple's managed app configuration and an Info.plist key `com.apple.configuration.managed` are used from memory); not tried.
2. **DNS-SRV** `_cometcast._tcp`: `DNSServiceQueryRecord` (dns_sd). SRV rdata is parsed strictly (no compression pointer, label and name limits); ordering by priority and weighted random choice (RFC 2782); targets pass the same host rules as in Rust (no loopback, link-local, metadata addresses, no spelling of an IPv4 address as a name); `wss://` unless the target is a private IP or `.local`. **UNCERTAIN:** whether the system adds the network's search domains to this name; tvOS has no public API to read them (the Rust crate asks every search domain itself). Test plan G3.
3. **Announcement by a display** (mDNS): `NWBrowser` with TXT records (`v`, `srv`, `tls`, `name`); first announcement per instance name wins; an announced server needs `tls=1` unless it is a private IP or `.local`. Needs `NSBonjourServices` and the local network permission (Info.plist). The Apple TV does not announce itself (the old plan to advertise through `NWListener` is not built).
4. **Build default**: Info.plist key `CCDefaultRelay`, filled from the build setting `CC_DEFAULT_RELAY` (empty by default), replaces the built-in list (`relay.cometcast.de:443`, which does not exist yet).

Rules shared with the Flutter app and the web app:

- The **pre-fill is never an announced server** (any device in the network can announce one and would receive the room code). It is the server used last (if it is still in the list, or was typed and confirmed), else the first entry that is not an announcement. A server that came from an announcement is not saved as "last used".
- A server from an **announcement or typed by the person is confirmed once** (host, source, risk text) before anything connects, like the web app's confirmation for `?server=`. Confirmed servers are remembered (at most 16).
- `ws://` only for local/private hosts (the web rule `parseRelay`); the idle and receive screens show an "unencrypted" note for those.
- The source of the selected server is always shown.

Differences from the web app: there is no `?server=` link, so no link-based relay; the web app's `config.json` has the managed configuration as counterpart.

## ALTCHA

The same scheme as the web client and the Rust cores (PoW v2, PBKDF2 with SHA-256/384/512; password = nonce bytes followed by the counter as 4 bytes big endian; the search starts at 0). Before any work the puzzle is checked: cost at most 10 000, key length at most 64, key prefix not longer than the key, `expiresAt` not in the past, all numbers integers (placeholders shared with the other clients, UNCERTAIN). The challenge is returned **unchanged** (unknown fields included) in the payload. `GET /altcha/challenge` times out after 8 s, redirects are not followed, bodies are capped at 256 KiB; HTTP 429/403/503 map to `rate_limited`, `banned`/`forbidden`, `server_full`. Solving runs in a detached task and can be cancelled; the overall limit is 60 s.

**UNCERTAIN:** how long the solver takes on an Apple TV. On Apple platforms PBKDF2 uses `CCKeyDerivationPBKDF` (not compiled yet); the portable fallback (used on Linux and in the tests) solved the 777-attempt, cost-1000 puzzle in about 4.7 s in a debug build on a Linux machine, which is not a statement about the Apple TV (measurement point 8, test plan I2).

## Connection check code

Identical to `sas.ts`: SHA-256 over `cometcast/connection-check/v1\n<fp1>\n<fp2>` with the two lower-case DTLS fingerprints sorted, first four bytes as a number modulo 1 000 000, shown as `123 456`. Computed from the answer's SDP and the offer's SDP when the peer connects; shown on the code screen and as a small badge over the video; the explanation text is the web app's (the app does not enforce a comparison). Known-answer vectors are in `tests/fixtures/sas.json` and were produced by the web implementation.

## Join approval (protocol version 2): TODO

**TODO, not done: the Swift client in `apps/apple-tv/CometCastCore` still speaks protocol version 1** (`protoVersion = 1` in `Signaling.swift`). The server, the web app, the Rust cores, `cc-ios` and `cometcast-cli` speak **protocol version 2** ([protocol.md](protocol.md#join-approval)). A version 1 client is answered with `proto_mismatch`, so the Apple TV app **cannot connect to a current server until it adopts version 2**. What it must do (nothing of this is implemented here):

- Send `proto_version: 2` in `hello`; `create_room` may carry `join_mode`, and `room_created` now carries `join_mode` (`open`, `confirm`, `locked`).
- Receiver side (the TV): decode `join_request` (`id`, `name`, `check`), `join_request_cancelled`, `join_mode_changed`, `blocked`, `unblocked`; answer with `join_decision` (`allow`, optional `block`) and `set_join_mode`; show the sender name as unverified text, the two-digit check number, and a decision that is safe with a Siri Remote (hold to allow). `peer_joined` carries the sender `name`.
- Sender side (not planned for tvOS): `join_room` with `name`, `join_pending`, `join_denied`, `room_locked`, `approval_timeout`.
- Take the vectors of `tests/fixtures/join-approval/` into `FixtureTests.swift`, and remove the `expect_apple_tv` overrides (marked `note_apple_tv`, "TODO proto 2") from `tests/fixtures/signaling.json` when the decoder returns the version 2 shapes (`room_created.join_mode`, `peer_joined.name`).

Until then the Swift fixture tests keep passing on the protocol 1 shapes through those overrides. Join approval is not part of the tvOS device test plan yet.

## Content channel

The data channel with the label `control` (other labels are ignored) carries text messages (JSON) and binary chunks. The Swift parser is a port of `control.ts`; both are checked against the same vectors.

| Limit | Value | Notes |
| --- | --- | --- |
| Chunk size | 16 KiB; every chunk exactly 16 KiB except the one that completes the transfer | bounds the number of messages, rejects floods of tiny chunks |
| Largest image | 10 MiB | UNCERTAIN (a guess, no measurements) |
| Image types | `image/png`, `image/jpeg`, `image/webp`, `image/gif` | exact match, case sensitive |
| Largest PDF | 64 MiB | same as `crates/cc-files` |
| Largest text message | 8192 characters, counted in UTF-16 units | the web unit; Rust counts code points |
| Name length | 1 to 255 Unicode characters (510 UTF-16 units at most), control, bidirectional and invisible characters removed | list identical to the web client's |
| Link | at most 2048 characters (input and normalised form), http/https only, no credentials, no whitespace or control characters, no empty host | normalised with the WHATWG rules |
| `go_to_page` | integer 1 to 1 000 000 | integral floats such as `2.0` count as integers (like the web client) |
| Stall timeout | 30 s without a chunk drops the transfer (`stalled`) | UNCERTAIN; Rust uses 60 s; the app polls once a second |
| Progress updates | at most one per 100 ms | |
| One transfer at a time | a new `image`/`document` drops the one in progress; `stop` drops it too | |
| Memory | one buffer of the announced size, allocated at the start | at most 64 MiB for a PDF |

Error codes of a transfer (`transferError.<code>`): `bad_message`, `unexpected_chunk`, `too_large`, `size_mismatch`, `no_transfer`, `bad_chunk`, `stalled`, the `document_*` codes, and two Apple TV additions, `image_undecodable` and `image_too_many_pixels`.

**Differences between the three implementations**, found by running the shared fixtures (documented in the fixtures as `expect_apple_tv` and `expect_rust`; the web client is the reference):

| Case | Web | Rust (`cc-files`) | Swift (Apple TV) |
| --- | --- | --- | --- |
| `size` or `page` written as `2.0` or `1e1` | accepted | refused (serde) | accepted |
| extra key next to `go_to_page` inside `cmd` | accepted | refused | accepted |
| message of 8194 UTF-16 units made of astral characters | refused | accepted (counts code points) | refused |
| link without `://` (`https:example.com`), `http://@host` | accepted | refused | accepted |
| link with a non-breaking space, a bidi override or a leading Unicode space | accepted (percent-encoded or trimmed) | refused | accepted |
| IDN host (`münchen.de`), full-width host, soft hyphen in host, `xn--` labels | accepted (punycode) | accepted | **refused** (IDNA is not implemented) |
| JSON nested deeper than 1000 levels | accepted | not tested (serde_json's default limit is 128 levels) | refused |
| lone UTF-16 surrogate escape in a name | kept | not tested | replaced by U+FFFD |

No case was found where the Swift code accepts something the browser refuses; its differences only refuse more (apart from the lone surrogate). UNCERTAIN: only the vectors in `tests/fixtures` were compared (131 links, 107 control messages); this is not a proof of equivalence.

### Images

ImageIO, with the type taken **from the bytes** (PNG, JPEG, WebP, GIF), not from the announced MIME type. The pixel count is read from the header before decoding and refused above 64 Mpixel (`imageMaxPixels`, the value of `cc-docs`' `max_image_pixels`; the web client has no cap because the browser decides). The picture is decoded through a thumbnail request limited to 3840 pixels on the long side. A GIF shows its first frame. UNCERTAIN: WebP in ImageIO on tvOS, memory on older Apple TV models.

### PDF

Core Graphics (`CGPDFDocument`). **UNCERTAIN: PDFKit is, to our knowledge, not part of tvOS** (not checked without the SDK), so it is not used; Core Graphics is. Checks: `%PDF-` in the first 1024 bytes, encrypted and locked is refused (`document_encrypted`), 5000 pages at most, rendering at most 8192 pixels on a side and 7680x4320 pixels, white background, a watchdog of 20 s. Only pixels are shown: no text layer, no links, no scripts. Pages are rendered on demand at the screen width; left/right on the Siri Remote and the sender's `next_page`/`prev_page`/`go_to_page` turn pages.

**Risk the desktop apps do not have:** they render in a child process with a memory limit (`cc-docs`). tvOS cannot start processes, so a hostile PDF is parsed inside the app by Core Graphics; the watchdog stops waiting but cannot cancel the drawing, and memory is not limited. This is assumption A20.

### Links

A link is shown as a **card with the host and the full address and no open action** (tvOS has no browser). Only a **media link** gets a Play button, with the classification of the web client:

| Extension | Kind | Played on tvOS |
| --- | --- | --- |
| `mp4`, `webm`, `ogv` | video | `mp4` yes; `webm`, `ogv` no (notice "format not supported") |
| `m3u8` | hls | yes (AVPlayer plays HLS natively) |
| `mp3`, `ogg`, `wav`, `m4a` | audio | `mp3`, `wav`, `m4a` yes; `ogg` no |

(Case-insensitive, query and fragment ignored. The tvOS column is UNCERTAIN: from Apple's list of formats, not tried.) Nothing is loaded before the person selects **Play from <host>**. A plain `http:` URL to a non-local host gets no Play button (App Transport Security would block it; the web client's mixed-content rule). Hosts that are IP addresses or local names get the web client's warning. Playback uses an `AVPlayer` with an empty cookie list and the shared cookie storage set to "never"; the Referer is not controllable (AVFoundation sends none by default, UNCERTAIN); the media host learns the IP address and an AVFoundation User-Agent. A 20 s load timeout ends the attempt; the player state machine (`card`, `loading`, `playing`, `error`) is the web client's.

## Video

**WebRTC library: LiveKit's `webrtc-xcframework` 150.7871.05 (Google WebRTC M150 build), pinned by commit.**

| Candidate | Licence | tvOS slices | Result |
| --- | --- | --- | --- |
| `stasel/WebRTC` (SPM, tag 157.0.0, M157, newest at the time) | BSD-3-Clause (its `LICENSE.md`) | **No.** Its README lists iOS (device, simulator), macOS and macOS Catalyst; its build script has no tvOS target; its `Package.swift` declares iOS and macOS only | not usable on tvOS |
| `livekit/webrtc-xcframework` 150.7871.05 (SPM binary target) | repository: MIT (wrapper); the xcframework carries the WebRTC project's BSD-3-Clause `LICENSE` | **Yes.** The downloaded zip (686 363 521 bytes) has the SHA-256 `b24d7df29733079038931698102fbb39cbc11b014e4352beb0d4f55a01c3c37e`, equal to the checksum in its `Package.swift`; its `Info.plist` lists `tvos-arm64` and `tvos-arm64-simulator` next to iOS, macOS, Catalyst and visionOS; the tvOS arm64 slice (12 290 952 bytes) has headers for `RTCMTLVideoView`, `RTCDefaultVideoDecoderFactory`, `RTCVideoDecoderH264` | **chosen** |

What this check established (2026-11-07, by reading the repository and the zip): the slices exist, the headers are there, the LiveKit Swift SDK declares tvOS 17 and depends on exactly this package. What it did **not** establish: that the app links and runs on a real Apple TV, that H.264 hardware decoding works there, the real behaviour of the library on tvOS. That is assumption A19 and test plan section D. Notes:

- The classes carry the prefix `LK` (`LKRTCPeerConnection`, `LKRTCMTLVideoView`, ...); the module is `LiveKitWebRTC`. The build is from the `webrtc-sdk/webrtc` fork, not byte-for-byte Google's source (its README says so), and it turns the network path monitor field trial on.
- `Package.swift` of the package lists iOS, macOS and Catalyst in `platforms` but not tvOS. UNCERTAIN whether Xcode accepts it for a tvOS target without complaint (the LiveKit SDK does it).
- **Pinning:** `project.yml` pins the **commit** `7d963f6fae5921cc8e594a5fef42db2bd2ea8563` (tag `150.7871.05`); that commit's `Package.swift` names the zip and its checksum, so the binary is pinned too. To check by hand: `shasum -a 256 LiveKitWebRTC.xcframework.zip` must give the value above.
- **Licences:** the WebRTC source is BSD-3-Clause; the binary also contains third-party code (libvpx, opus, BoringSSL, abseil, libyuv and others) with their own notices that are **not yet collected** ([THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)). The 12 MB slice must not be distributed in a release before the notices are checked.
- **Fallback if the slices do not work on a device:** (1) build WebRTC for tvOS ourselves (the WebRTC GN build has no first-party tvOS target we know of; UNCERTAIN, days of work); (2) ship the Apple TV without the app: iPhones and Macs can use the system's AirPlay to the Apple TV (milestone M3 is the other direction), and the browser receiver can run on another device; (3) mark tvOS as "not delivered in 0.1". There is no pure-Swift WebRTC receiver we know of.

Rendering: `LKRTCMTLVideoView` (a Metal view, `videoContentMode = .scaleAspectFit`) inside `UIViewRepresentable`. The header shows the `TARGET_OS_IPHONE` branch, which covers tvOS. The other way, if it misbehaves: an `AVSampleBufferDisplayLayer` fed from an `LKRTCVideoRenderer` (not written). Decoding: the library's default decoder factory (H.264 through VideoToolbox; VP8/VP9/AV1 in software); the answer prefers H.264 through the codec preferences of the video transceiver (UNCERTAIN: the exact API names). Audio is not handled (milestone M1 has none) and the audio session is not configured.

## User interface

- **Idle screen:** three steps, the server in use and its source, "Show room code", "Change server". No QR code and no code entry on the TV: the receiver shows the six character code and the **sender** types it. After an error the same screen shows the translated message and "Create new room".
- **Receive screen:** the code in very large monospaced type (`ABC 234`, spoken letter by letter), status line, connection check code, server and unencrypted note, then the video full screen with the check code as a small badge. Content (image, PDF page, link card) is shown over the video with a Close button that has the default focus.
- **Siri Remote:** Menu on the receive screen asks "Stop receiving?" (Menu with content open closes the content); left/right turn PDF pages; Select presses buttons; the server list and the typed-server field are reachable with the focus engine (text entry uses the system keyboard). `idle timer` is disabled while a room is open.
- **Dark mode:** follows the system; only semantic colours and materials are used (no fixed light/dark colours except orange notices).
- **Languages:** English and German through `Localizable.xcstrings` (136 keys) and `InfoPlist.xcstrings`; German addresses the person with "Sie". `node tools/check-i18n.mjs` checks the catalog; the Swift tests check every key the core can produce.
- **Accessibility:** labels for the room code, check code, video, images and documents, progress as a percentage, headings, grouped elements; **UNCERTAIN:** never run with VoiceOver.
- **Lifecycle:** tvOS does not run the app in the background ([A13](measurement-points.md#1-beta-assumptions-a1-to-a20)). When the app comes back to the foreground after having been running, it starts again with a **new** room code. Always-on use needs the app in the foreground, with Single App Mode (AppLock) through MDM.

Not built: toolbar and annotations (the web receiver has them; there is no touch or pointer on tvOS), mDNS announcement by the Apple TV, auto-start without a button, audio, receiving more than one sender, a QR code.

## Project and release workflow

| What | Value |
| --- | --- |
| Generator | XcodeGen, `apps/apple-tv/project.yml` (no `.xcodeproj` is committed; `.gitignore` excludes it) |
| Targets | `CometCastTV` (application, tvOS 17.0), `CometCastTVTests` (unit tests: the sources of `CometCastCore/Tests/CometCastCoreTests`) |
| Scheme | `CometCastTV` (build, run, test, archive) |
| Packages | `CometCastCore` (local), `LiveKitWebRTC` (commit pin) |
| Bundle ids | `de.cometcast.tv`, tests `de.cometcast.tv.tests` |
| Info.plist | `Resources/Info.plist`: `NSLocalNetworkUsageDescription` (translated by `InfoPlist.xcstrings`), `NSBonjourServices` = `_cometcast._tcp`, `NSAppTransportSecurity` / `NSAllowsLocalNetworking`, `CCDefaultRelay`, `CFBundleLocalizations` en, de |
| Version | `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` appear once, in the top level `settings`; `tools/set-version.mjs` rewrites them (tested on a copy with 1.2.3-rc.1 build 42) |

The workflow job `tvos` runs `xcodegen generate`, then `xcodebuild archive -project CometCastTV.xcodeproj -scheme CometCastTV -configuration Release -destination 'generic/platform=tvOS'` and reads the first `PRODUCT_BUNDLE_IDENTIFIER` of `project.yml` for the export options. All three names match the new project (checked by reading both files; `actionlint` passes on the workflow, which was **not changed**). The first line of `project.yml` that has `PRODUCT_BUNDLE_IDENTIFIER` is the app's, which the job relies on. The job needs the network (SPM resolves the 686 MB WebRTC zip). Never run.

**App icon (tvOS layered image, placeholders from `assets/logo`):** `Assets.xcassets/App Icon & Top Shelf Image.brandassets` with two image stacks: `App Icon - Large` (1280x768, 1x) and `App Icon - Small` (400x240 at 1x, 800x480 at 2x). Each stack has three layers, `Front` (the logo on a transparent background, 80 % of the height), `Middle` (a soft glow, transparent) and `Back` (opaque navy gradient). tvOS needs at least two layers for the parallax effect, and the back layer must be opaque. **Top Shelf images are not provided** (not needed; the asset catalog compiler may warn). Replace the placeholders with finished artwork from `assets/logo/cometcast-app-icon.svg` before a store build; the launch screen is the empty `UILaunchScreen` dictionary.

## Tests

| What | Command | Result today |
| --- | --- | --- |
| CometCastCore, Linux | `cd apps/apple-tv/CometCastCore && swift test` | 62 tests, 0 failures (Swift 6.0.3, x86_64 Ubuntu 24.04, swift-crypto 3.10.0), about 5 s |
| Same vectors, web | `cd apps/web && npx vitest run tests/fixtures.test.ts` (part of `npm test`) | 10 tests pass; the complete web suite (215) and `vue-tsc` pass |
| Same vectors, Rust | `cargo test -p cc-files --test shared_fixtures` | 2 tests pass; `cargo fmt` and `clippy` clean for the new file |
| i18n | `node tools/check-i18n.mjs`; `node --test tools/check-i18n.test.mjs` | pass |
| Project files | Info.plist read with Python `plistlib`, `project.yml` with PyYAML; `tools/set-version.mjs` on a copy | parse; version rewritten |
| App syntax | `swiftc -parse` on every file of `Sources/CometCastTV` | no errors; `ConfigStore.swift` also type-checks against the built core on Linux |
| CI | `.github/workflows/appletv-core.yml` (Linux in a `swift:6.0.3` container, macOS 15) | actionlint passes; **never run**; the macOS job would be the first run of the CryptoKit and CommonCrypto paths |
| Xcode build, XCTest on the simulator, anything on a device | | **not run** |

What the Linux tests cover: message parser and limits (107 vectors), chunk reassembly and stall (19 scenarios), link validation (131 vectors), connection check code (24 vectors), ALTCHA (solving, payload, limits, errors; SHA-256/384/512; the server-made puzzle of `crates/cc-transport/testdata`), PBKDF2 against OpenSSL results, relay parsing (72 vectors), signalling decode, ICE list parsing, the state machine (happy path, ALTCHA, every error class, stale events), the server list rules (merge, policy, address rules, SRV, TXT, pre-fill, confirmation), the content controller, the player state machine, the statistics summary, the string catalog.

## Not verified (summary)

Everything under `Sources/CometCastTV` compiles and works: unknown. In particular: the XcodeGen keys (`revision`, scheme keys), SPM resolution of the binary package for tvOS, every `LKRTC` Swift name, the Network and dns_sd glue (including the search domain question), ImageIO WebP, Core Graphics PDF speed and memory, AVPlayer behaviour (cookies, Referer, formats), managed configuration delivery, the focus behaviour of the Siri Remote, VoiceOver, dark mode, German layout (long words), the local network permission flow, Single App Mode, the 12 MB slice's behaviour, the ALTCHA solver speed, and the build of the `tvos` release job.
