<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Apple TV receiver (native SwiftUI, tvOS 17+)

The Apple TV app is a **display only** (never a presenter): it shows the screen of the room (idle screen with address, code and QR code, the setup wizard, Settings and About, the join request dialog), answers the sender's WebRTC offer, plays the H.264 screen stream and shows the content the sender pushes (images, links, PDF pages). Its screens are built as designed in [design/first-run-and-display.md](design/first-run-and-display.md) and [design/native-apps.md](design/native-apps.md); the questions that the implementer left to the designer are in [design/implementation-questions-appletv.md](design/implementation-questions-appletv.md). It is built **to specification without a Mac, an Xcode or an Apple TV**: the platform independent part is tested on Linux, the app target has never been compiled. Everything that needs the hardware is in [appletv-device-test-plan.md](appletv-device-test-plan.md). Decision (owner): native SwiftUI, the Rust core is **not** used on tvOS (the earlier plan with UniFFI is dropped for this app).

Statements that rest on an assumption are marked **UNCERTAIN** (in the code `// UNCERTAIN:`) and linked to [measurement-points.md](measurement-points.md): assumptions [A13](measurement-points.md#1-beta-assumptions-a1-to-a20) (no background operation), A19 and A20 (new, below), spike 3.

## At a glance

| Part | Where | Status |
| --- | --- | --- |
| Protocol, content parser and limits, ALTCHA, connection check code, relay rules, state machine; and the rules of the screens (design numbers, wizard, key tables, reconnect schedule, QR code, licence list) | `apps/apple-tv/CometCastCore` (Swift package, no Apple-only imports) | **Built and tested on Linux** (Swift 6.0.3, swift-crypto): 140 XCTest cases, same vectors as the web client |
| Shared test vectors | `tests/fixtures/*.json` | Checked by the web client (vitest), by the Rust crate `cc-files` (two tests) and by the Swift package |
| SwiftUI app (design tokens, components, screens), WebRTC wrapper, discovery glue, ImageIO/CoreGraphics decoding, AVPlayer | `apps/apple-tv/Sources/CometCastTV` | **Written, syntax-checked only** (`swiftc -parse`; the pure Foundation files also type-check against the built core); not compiled, not run |
| XcodeGen project, Info.plist, string catalogs (du loaded, Sie alternate), fonts, licence copies, icons, logos, app icon layers | `apps/apple-tv/project.yml`, `Resources/`, `Assets.xcassets`, `Sources/CometCastTV/*.xcstrings` | Written; plist, YAML, JSON parse; `xcodegen` never run |
| Release job `tvos` | `.github/workflows/release.yml` | Names match the project (see [below](#project-and-release-workflow)); never run |

## Architecture

```
SwiftUI views ── AppModel (main actor) ── ReceiverMachine (pure state machine, CometCastCore)
 (Design/, Views/)  │  also drives the pure screen rules of CometCastCore: WizardState, ConnectionTracker,
                    │  ControlsReveal, IdleDimmer, InputGuard, ToastState, the key tables
                    │  performs the effects:
                    ├─ SignalingSocket      URLSessionWebSocketTask, JSON text frames (docs/protocol.md)
                    ├─ ServerHTTP           GET /altcha/challenge (solved in CometCastCore), GET /ice-servers
                    ├─ PeerSession          LiveKitWebRTC: answer, video track, "control" data channel
                    ├─ Discovery            NWBrowser (mDNS), dns_sd (DNS-SRV)
                    ├─ ControlReceiver      content channel parser and reassembly (CometCastCore)
                    ├─ ImageDecoder         ImageIO
                    ├─ PDFRenderer          Core Graphics (CGPDFDocument)
                    ├─ Announcer            NWListener service with the TXT record (spec 14.8)
                    ├─ LicenceStore         About: parses the bundled licence files off the main actor
                    └─ HostResolver         getaddrinfo, the IPv4 address of an in-house box
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
| `ContentController.swift` | what is shown, page turning, error mapping, PDF/image limits; the page message `{"kind":"page",...}` | `receiverSession.ts` |
| `Design.swift` | the numbers of the design: the three display palettes (hex), the selected-tile blend, appearance choice and Automatic, the reference scale and safe-area padding, the type table with the PostScript names, timings, focus ring, surface metrics, join mode keys | spec 3, native-apps 11, 12, 14, 15 |
| `DisplayText.swift` | room name cleaning, names of people (18 characters + "…", `common.noName`), the join address `{dom}` (`cometcast.de/app`, no `:443`), the QR URL and the in-house box rule, security code grouping, transfer pill shortening, `{placeholder}` filling | `clean_room_name`, `fitName.ts`, spec 4.2, 4.4 |
| `ConnectionView.swift` | what the idle screen shows (connecting, online, offline), the reconnect schedule (1 s, 2 s, 3 s, then 4 s; busy 30 s; outdated never), the busy memory | `connectionView.ts`, `busyMemory.ts`, `DisplayRunning.vue` |
| `Wizard.swift` | the setup wizard as a state machine (name chips and field, mode, address step with sources and "Other address", focus rows) | `displaySetup.ts` |
| `Interaction.swift` | Siri Remote key tables (Back, Menu = Play/Pause, presenting keys), controls reveal (5 s), idle dimming (7 min), the 1 s input guard, the toast | `controlsReveal.ts`, `idleDim.ts`, `joinGuard.ts` |
| `QR.swift` | QR code generator (byte mode, level M, versions 1 to 10), tested against the web display's encoder | `qr.ts` (`qrcode-generator`) |
| `Licences.swift` | the licence list of About, parsed from `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.md` | spec 15.9 |
| `DisplayConfig.swift` | what is remembered: room name, default mode (never Locked), server, appearance | `loadSetup` |

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
| `room_ended` (after End) | a new room on the same connection, in the **default mode** (Open or Approval on): Locked lasts only until End (spec 4.3) |
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

The display does not show the `errors.<code>` texts any more: when the connection fails the idle screen shows the offline box whose body is chosen by the code (network, busy: `rate_limited`, `banned`, `server_full`, `forbidden`; outdated: `proto_mismatch`, `bad_hello`) and the display tries again by itself (spec 4.2); the catalog has no `errors.*` keys. The machine also collects `HostEvent`s (request expired or withdrawn, sender joined or left, session ended) with `takeEvents()`, which the app turns into toasts.

A test checks that every key the core names (transfer errors, relay errors, media errors, appearance and mode tiles, offline bodies, name suggestions, address sources) and every key the Swift views pass to `L.s` / `L.t` exists in both catalogs, in English and German, in the du set and in the Sie set, and that `{placeholders}` agree.

## Servers (relays)

Order of sources, the same as the owner's rule in [discovery.md](discovery.md):

1. **Managed app configuration (MDM)**: the `com.apple.configuration.managed` dictionary in `UserDefaults`, with the keys of the desktop policy file (`servers`, `forbid_default_list`, `forbid_user_override`, `forbid_announcement`, `mode`, `version`). Unknown keys or wrong types make the whole configuration invalid and the app **fails closed**: no server, locked, a notice. With `forbid_user_override` only the managed servers are offered and the text field is hidden. **UNCERTAIN:** how a real MDM product delivers this to tvOS (Apple's managed app configuration and an Info.plist key `com.apple.configuration.managed` are used from memory); not tried.
2. **DNS-SRV** `_cometcast._tcp`: `DNSServiceQueryRecord` (dns_sd). SRV rdata is parsed strictly (no compression pointer, label and name limits); ordering by priority and weighted random choice (RFC 2782); targets pass the same host rules as in Rust (no loopback, link-local, metadata addresses, no spelling of an IPv4 address as a name); `wss://` unless the target is a private IP or `.local`. **UNCERTAIN:** whether the system adds the network's search domains to this name; tvOS has no public API to read them (the Rust crate asks every search domain itself). Test plan G3.
3. **Announcement by a display** (mDNS): `NWBrowser` with TXT records (`v`, `srv`, `tls`, `name`); first announcement per instance name wins; an announced server needs `tls=1` unless it is a private IP or `.local`. Needs `NSBonjourServices` and the local network permission (Info.plist). The browsing runs only while the setup wizard runs. The Apple TV announces itself too (`Announcer`, `NWListener` service, spec 14.8): only while Idle or Presenting with a code, in the foreground and when the policy allows it; the TXT record carries the room name of the wizard as `name`.
4. **Build default**: Info.plist key `CCDefaultRelay`, filled from the build setting `CC_DEFAULT_RELAY` (empty by default), replaces the built-in list (`relay.cometcast.de:443`, which does not exist yet).

Rules shared with the Flutter app and the web app:

- **The setup wizard chooses the server** (spec 4.1, step 3): the addresses found are offered in the order policy, DNS, announcement ("in-house box"), default; the first is shown with its source line (`setup.address.sourceOwn`, `sourceBox`, `sourcePolicy`, `sourceDefault`, or `sourceTyped` for an address typed into the field) and "Looks right" confirms it; "Other address" cycles the found ones and then opens the text field (not offered under a policy lock). Finish stores the server with the room name and the default mode (`DisplayConfig`); later launches use the stored server (under a policy lock the managed one) and open the room at once. A server chosen in the wizard counts as confirmed. This replaces the earlier rule "the pre-fill is never an announced server, an announced or typed server is confirmed on a separate page": there is no relay confirmation page on a native display (decision N19), and a link never names a server. Observation for the designer: an announcing device in the network can be the wizard's first suggestion (question B5 in [design/implementation-questions-appletv.md](design/implementation-questions-appletv.md)).
- `ws://` only for local/private hosts (the web rule `parseRelay`); the idle and receive screens show an "unencrypted" note for those.
- The source of the selected server is always shown.

Differences from the web app: there is no `?server=` link, so no link-based relay; the web app's `config.json` has the managed configuration as counterpart. After setup there is no way to change the server in the app (spec 10: reset the app's data).

## ALTCHA

The same scheme as the web client and the Rust cores (PoW v2, PBKDF2 with SHA-256/384/512; password = nonce bytes followed by the counter as 4 bytes big endian; the search starts at 0). Before any work the puzzle is checked: cost at most 10 000, key length at most 64, key prefix not longer than the key, `expiresAt` not in the past, all numbers integers (placeholders shared with the other clients, UNCERTAIN). The challenge is returned **unchanged** (unknown fields included) in the payload. `GET /altcha/challenge` times out after 8 s, redirects are not followed, bodies are capped at 256 KiB; HTTP 429/403/503 map to `rate_limited`, `banned`/`forbidden`, `server_full`. Solving runs in a detached task and can be cancelled; the overall limit is 60 s.

**UNCERTAIN:** how long the solver takes on an Apple TV. On Apple platforms PBKDF2 uses `CCKeyDerivationPBKDF` (not compiled yet); the portable fallback (used on Linux and in the tests) solved the 777-attempt, cost-1000 puzzle in about 4.7 s in a debug build on a Linux machine, which is not a statement about the Apple TV (measurement point 8, test plan I2).

## Connection check code

Identical to `sas.ts`: SHA-256 over `cometcast/connection-check/v1\n<fp1>\n<fp2>` with the two lower-case DTLS fingerprints sorted, first four bytes as a number modulo 1 000 000, shown as `123 456`. Computed from the answer's SDP and the offer's SDP when the peer connects; shown on the code screen and as a small badge over the video; the explanation text is the web app's (the app does not enforce a comparison). The fingerprint is read as the DTLS stack reads the SDP: lines end at LF (one CR before it is part of the line end); an SDP with any other CR, U+2028 or U+2029, or with `sha-256` fingerprint lines that disagree or are malformed, gives no code (the same rule in `sas.ts`, `crates/cc-transport/src/sas.rs` and `Sas.swift`). Known-answer vectors are in `tests/fixtures/sas.json` and were produced by the web implementation; the refusal cases were added with the Rust implementation. **The Swift version of the stricter rule was written without a Swift compiler and has not been run.**

## Join approval (protocol version 2)

The Swift client speaks **protocol version 2** (`protoVersion = 2` in `Signaling.swift`), like the server, the web app, the Rust cores and the CLI ([protocol.md](protocol.md#join-approval)). The TV is the **owner of its room**: only the connection that created the room may decide who joins. Sender-side messages (`join_room` with `name`, `cancel_join`) exist in `ClientMessage` for the shared vectors, but the tvOS app is not a sender.

**Messages** (`CometCastCore/Signaling.swift`; strict decoding like `decodeServerMessage`: a missing or mistyped field gives `nil`, ids and check numbers must be integers):

| Direction | Message | Swift |
| --- | --- | --- |
| to server | `hello {proto_version: 2, role}` | `ClientMessage.hello` |
| to server | `create_room {altcha?, join_mode, room_name?}` | `.createRoom(altcha:joinMode:roomName:)`, always with a mode (`open` by default); `roomName` is optional (`ReceiverMachine(joinMode:roomName:)`, nothing in the app sets it yet) and is only written when present; the server cleans it and cuts it at 32 characters |
| to server | `set_join_mode`, `join_decision {id?, allow, block?}`, `remove_peer {block?}`, `unblock {id}`, `end_room` | `.setJoinMode`, `.joinDecision`, `.removePeer`, `.unblock`, `.endRoom`; `block` is only written when true, and for `join_decision` only with a denial of one request |
| from server | `room_created {code, join_mode}` (mode required), `peer_joined {name}` (name required) | `.roomCreated(code:joinMode:)`, `.peerJoined(name:)` |
| from server | `join_request {id, name, check}`, `join_request_cancelled {id, reason}`, `join_mode_changed`, `blocked {id, name}`, `unblocked {id}`, `room_ended`, `join_pending` | `.joinRequest`, `.joinRequestCancelled`, `.joinModeChanged`, `.blocked`, `.unblocked`, `.roomEnded`, `.joinPending(timeoutSecs:check:roomName:)` and `.joined(roomName:)` (the receiver ignores both; `roomName` is read leniently: absent, not a string or blank is `nil`, the message stays valid) |

**State machine** (`ReceiverMachine`, a port of the web `receiverSession.ts` join logic). New state in `ReceiverState`: `joinMode` (default `open`), `requests` (oldest first: `id`, cleaned `name`, two-digit `check`), `peerName`, `blocks`, `notice`. New inputs for the host's actions: `setJoinMode`, `decide(id:allow:block:)`, `denyAll`, `removePeer(block:)`, `unblock(id:)`, `endRoom`, `dismissNotice`; each returns a `send` effect and does nothing without a room (no code yet).

- **Mode.** The wanted mode survives start, stop and new rooms. `create_room` carries it; if the server's `room_created.join_mode` differs, a `set_join_mode` follows. `join_mode_changed` is what changes `state.joinMode`. If the server answers `open_rooms_disabled` while the room is being created, the machine asks again with `confirm` (same ALTCHA payload) and sets the notice `openDisabled`.
- **Requests.** A queue, no timer: the server ends a request after 60 s (`joinRequestSecs`, UNCERTAIN: an operator can change it) with `join_request_cancelled` (`timeout`, `left`, `cancelled`, `locked`, `room_full`, `blocked`). A decision removes the request at once (as the web client does); an unknown id is not sent. `locked` cancellations add up in the notice `lockedDiscarded(count:)` until the mode changes to something else.
- **Block list.** `blocked` adds, `unblocked` removes, duplicates are ignored. It is cleared when the room ends.
- **End the presentation / the session.** `removePeer` ends the connected sender's presentation (server answers `peer_left`, phase `peer_left`, the code stays). `endRoom` sends `end_room`; on `room_ended` the machine drops peer and content, clears requests and blocks and sends `create_room` again on the same connection (new code).
- **Errors.** `no_such_request` and `no_such_block` are ignored, `block_list_full` sets the notice `blockListFull`, all other errors are terminal as before (and now also clear requests, blocks and sender name). `joinApprovalErrorCodes` lists the new codes; they have **no string-catalog entries** and so show the generic message (see TODO below). Deviation from the web client: the web client ends the session on `no_such_block` and `block_list_full`; the Swift machine does not, because the room is intact.
- **Names.** `sanitizeDisplayName` repeats the server's rules (control, bidi, zero-width, invisible and private-use characters removed, white space collapsed, 40 Unicode scalars, `Guest N` when nothing is left; `N` is the request id, or the count of senders for `peer_joined`) as defence in depth. Markup is not removed: show every name as plain text.

**App target** (`AppModel`): `state` carries all of the above and the views of the join request dialog, Settings and the presenting controls bind to it (see [User interface](#user-interface)): `allowCurrent`, `denyCurrent(block:)` (with the toasts `allowed`, `denied`, `blocked`), `chooseMode` (Settings; Open and Approval on become the stored default, Locked is never stored), `endRoom` (End on the presenting controls, no confirmation). The designed screens replaced the earlier TODO list (hold to allow, banner while presenting, block list menu, "deny all"), which spec section 10 excludes; `denyAll`, `removePeer` and `unblock` stay in the machine and are not bound to any screen. `SignalingSocket` needed no change (the version check uses `protoVersion`).

**Tests.** `JoinApprovalTests.swift` reads `tests/fixtures/join-approval/` (messages, flows replayed against the machine, error codes, name vectors); `FixtureTests.testSignalingDecode` uses the shared `signaling.json` without any `expect_apple_tv` deviation. `LicenceAndEventTests.swift` (`HostEventTests`) tests the host events, the default mode after End and the unnamed flags.


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

Built as designed: [design/first-run-and-display.md](design/first-run-and-display.md) (sections 3 to 6, 11, 14, 15; decisions N1 to N96 for the display role) and [design/native-apps.md](design/native-apps.md). The Apple TV is a display only. Everything is placed on the 960 x 540 canvas (`Ref`: `1 ref px = scale`, `scale = min(w / 960, h / 540)`, no `.scaleEffect`); the picture alone fills the screen. The rules live in CometCastCore and are tested on Linux; the SwiftUI files only draw and move the focus.

| Part | File | What |
| --- | --- | --- |
| Tokens | `Design/DesignTokens.swift`, `Core/Design.swift` | `DisplayPalette` (Dark, Light, OLED black from hex values in sRGB; Automatic follows the system and never picks OLED black), `Ref`, `CanvasInsets` (safe area at the canvas edges: `max(spec padding, inset)`), the display type scale with `Font.custom(_:fixedSize:)`, `IconView`, `LogoView`. No SwiftUI semantic colours or materials |
| Fonts | `Resources/Fonts`, `Info.plist` `UIAppFonts` | Atkinson Hyperlegible Next 400, 500, 700 and Mono 500, 700 (OFL, unmodified, from `apps/flutter/assets/fonts`); the Latin Extended files are not bundled (same PostScript names, question A1) |
| Icons and logos | `Assets.xcassets/Icons`, `Logo`; source `assets/icons/*.svg` | The stroke icons of `Icon.vue`, template images with preserved vector data, generated by `node tools/gen-appletv-icons.mjs` (`--check`); the logos as vectors |
| Components | `Design/Components.swift` | Focus ring of spec 3.5 (outline 4, offset 3, glow 9 at 18 %, scale 1.03 in 120 ms, no scale on text fields, reduced motion: no scale, no animation) drawn by `FocusRingModifier` with `.focusEffectDisabled()` (no tvOS lift or parallax); button looks (primary, secondary, ghost, chip, tile, card, menu, close, overlay); `DialogCard`, `Scrim`, `InfoLine`, `ToastView` |
| Setup wizard | `Views/SetupView.swift`, `Core/Wizard.swift` | 5 steps, card 780 wide, top edge fixed at 40 from step 1, name chips and field (decision 13.39), Open / Approval on cards, address step with the source line and "Other address", step dots |
| Idle | `Views/IdleView.swift`, `QRView.swift` | Logo, room name, mode pill; step 1 address (58/700, `idle.altAddress` for a box), step 2 code (80 mono), QR code at level M (`https://{dom}/{CODE}`; empty box while connecting and offline, 25 % when Locked), offline box (`idle.offline.*` by cause), footer line and Settings button; padding `max(spec, safe area)` |
| Settings and About | `Views/SettingsView.swift`, `AboutView.swift` | Join mode tiles and description, appearance tiles, About button and Close (`settings.closeRemote`); About dialog and the licence viewer (list 300 wide, text pane, no search) |
| Join request | `Views/JoinRequestView.swift` | Alert dialog over Idle or Presenting: name in quotes (18 characters + "…"), check number, queue, [Yes, No] and "No and block"; 1 s input guard; focus on Yes after each decision; Back and Menu do nothing |
| Presenting | `Views/PresentingView.swift` | Full-bleed video (the existing `VideoSurface`), image or PDF page on black; name tag and End appear on any remote input for 5 s with 400 ms fades (the first input only reveals); transfer pill and page-error box at rest; nothing is focusable at rest except an invisible catcher |
| Link card | `Views/LinkCardView.swift` | Never an "Open" button (tvOS has no browser): [Dismiss] or [Play, Dismiss] for media (AVPlayer rules unchanged), initial focus Dismiss (Play for media), 1 s guard, Back = Dismiss |
| Toasts, dimming | `Views/RootView.swift`, `AppModel` | Toast above everything (3.2 s, a new one replaces the old); idle dimming after 7 minutes (overlay of `--bg` at 60 %, 2 s in, 200 ms out, the first input only restores) |

**Siri Remote** (spec 7.1, native-apps.md 6.1): Menu / Back (`onExitCommand`) and Play/Pause as the spec's Menu (`onPlayPauseCommand`) are routed by the key tables of `Interaction.swift`: Idle: Back is the system's (leaves the app), Play/Pause opens Settings; Settings: both close; join request: both do nothing; link card: Back dismisses; wizard: Back goes one step back (step 0: system); presenting: Back shows or hides the controls, Play/Pause shows them. The remote's Settings, Home, voice and app keys are not bound. Rows are `HStack`s with `.focusSection()`; initial focus with `.defaultFocus` and `@FocusState`.

**Wake lock and lifecycle.** `UIApplication.shared.isIdleTimerDisabled` is `true` only while presenting (never on Idle). When the app goes to the background the session ends without a toast and a new one with a new code starts on return (native-apps.md 15). The room opens by itself after setup and on every launch; there is no start button.

**Announcement.** The display announces itself with the existing `Announcer` only while Idle or Presenting with a code, in the foreground and when the policy allows it; the record carries the room name as `name` (spec 14.8). AirPlay is tvOS's own receiver: nothing about AirPlay is shown (decision N84).

**Languages.** English and German in `Localizable.xcstrings` (**du**, the shipped set) and `InfoPlist.xcstrings`; `Localizable-sie.xcstrings` is the formal set, complete and **not loaded** unless the build setting `CC_STRINGS_TABLE` (Info.plist `CCStringsTable`) is set to `Localizable-sie` (the German `InfoPlist` text has no address form). Keys are the dotted keys of the spec; `{name}` placeholders are filled by CometCastCore (`fillPlaceholders`). Both catalogs are generated by `node tools/gen-appletv-strings.mjs` (`--check`) from the web and Flutter locale files and the spec's table 15.13; `node tools/check-i18n.mjs` checks them (same keys and placeholders in du and Sie, no formal form in the loaded set).

**Accessibility.** The join request is a modal alert dialog labelled "Request: “{name}”" (screen change posted when it appears and for the next request), tiles and chips carry the selected trait, toasts are announced (`AccessibilityNotification.Announcement`), the QR code is labelled "QR: {url}", the Settings button is labelled by its word. **UNCERTAIN:** never run with VoiceOver.

Not built: AirPlay or Miracast switches (decision N84), About search (TVs have none), the toolbar and annotations (no pointer on tvOS), audio, receiving more than one sender, a way to change room name or server after setup (spec 10), the on-screen statistics badge (the statistics go to the log in debug builds).

## Project and release workflow

| What | Value |
| --- | --- |
| Generator | XcodeGen, `apps/apple-tv/project.yml` (no `.xcodeproj` is committed; `.gitignore` excludes it) |
| Targets | `CometCastTV` (application, tvOS 17.0), `CometCastTVTests` (unit tests: the sources of `CometCastCore/Tests/CometCastCoreTests`) |
| Scheme | `CometCastTV` (build, run, test, archive) |
| Packages | `CometCastCore` (local), `LiveKitWebRTC` (commit pin) |
| Bundle ids | `de.cometcast.tv`, tests `de.cometcast.tv.tests` |
| Info.plist | `Resources/Info.plist`: `NSLocalNetworkUsageDescription` (translated by `InfoPlist.xcstrings`), `NSBonjourServices` = `_cometcast._tcp`, `NSAppTransportSecurity` / `NSAllowsLocalNetworking`, `CCDefaultRelay`, `CCStringsTable` (`$(CC_STRINGS_TABLE)`), `UIAppFonts` (five Atkinson files), `CFBundleLocalizations` en, de |
| Resources | `Resources/Fonts` and `Resources/Licenses` are resource folders of the app target (`buildPhase: resources`); `Resources/Licenses` holds copies of `THIRD_PARTY_LICENSES.md` and `THIRD_PARTY_NOTICES.md` written by `node tools/gen-third-party.mjs` (`--check` fails when a copy differs) |
| Version | `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` appear once, in the top level `settings`; `tools/set-version.mjs` rewrites them (tested on a copy with 1.2.3-rc.1 build 42) |

The workflow job `tvos` runs `xcodegen generate`, then `xcodebuild archive -project CometCastTV.xcodeproj -scheme CometCastTV -configuration Release -destination 'generic/platform=tvOS'` and reads the first `PRODUCT_BUNDLE_IDENTIFIER` of `project.yml` for the export options. All three names match the new project (checked by reading both files; `actionlint` passes on the workflow, which was **not changed**). The first line of `project.yml` that has `PRODUCT_BUNDLE_IDENTIFIER` is the app's, which the job relies on. The job needs the network (SPM resolves the 686 MB WebRTC zip). Never run.

**App icon (tvOS layered image, placeholders from `assets/logo`):** `Assets.xcassets/App Icon & Top Shelf Image.brandassets` with two image stacks: `App Icon - Large` (1280x768, 1x) and `App Icon - Small` (400x240 at 1x, 800x480 at 2x). Each stack has three layers, `Front` (the logo on a transparent background, 80 % of the height), `Middle` (a soft glow, transparent) and `Back` (opaque navy gradient). tvOS needs at least two layers for the parallax effect, and the back layer must be opaque. **Top Shelf images are not provided** (not needed; the asset catalog compiler may warn). Replace the placeholders with finished artwork from `assets/logo/cometcast-app-icon.svg` before a store build; the launch screen is the empty `UILaunchScreen` dictionary.

## Tests

| What | Command | Result today |
| --- | --- | --- |
| CometCastCore, Linux | `cd apps/apple-tv/CometCastCore && swift test` | 140 tests, 0 failures (Swift 6.0.3, x86_64 Ubuntu 24.04, swift-crypto 3.10.0), about 6 s |
| Same vectors, web | `cd apps/web && npx vitest run tests/fixtures.test.ts` (part of `npm test`) | 10 tests pass; the complete web suite (215) and `vue-tsc` pass |
| Same vectors, Rust | `cargo test -p cc-files --test shared_fixtures` | 2 tests pass; `cargo fmt` and `clippy` clean for the new file |
| i18n | `node tools/check-i18n.mjs`; `node --test tools/check-i18n.test.mjs` | pass (du and Sie catalogs: same keys, same placeholders, no formal form in the loaded set) |
| Generated files | `node tools/gen-appletv-strings.mjs --check`, `node tools/gen-appletv-icons.mjs --check`, `node tools/gen-third-party.mjs --check` | up to date |
| Project files | Info.plist read with Python `plistlib`, `project.yml` with PyYAML, the `.xcstrings` and `Contents.json` files as JSON; `tools/set-version.mjs` on a copy | parse; version rewritten |
| App syntax | `swiftc -parse` on every file of `Sources/CometCastTV` | no errors; `ConfigStore.swift` and `L.swift` also type-check against the built core on Linux (the SwiftUI files cannot: no SDK) |
| CI | `.github/workflows/appletv-core.yml` (Linux in a `swift:6.0.3` container, macOS 15) | actionlint passes; **never run**; the macOS job would be the first run of the CryptoKit and CommonCrypto paths |
| Xcode build, XCTest on the simulator, anything on a device | | **not run** |

What the Linux tests cover: message parser and limits (107 vectors), chunk reassembly and stall (19 scenarios), link validation (131 vectors), connection check code (33 code and 33 fingerprint vectors), ALTCHA (solving, payload, limits, errors; SHA-256/384/512; the server-made puzzle of `crates/cc-transport/testdata`), PBKDF2 against OpenSSL results, relay parsing (72 vectors), signalling decode, ICE list parsing, the state machine (happy path, ALTCHA, every error class, stale events), the server list rules (merge, policy, address rules, SRV, TXT, pre-fill, confirmation), the content controller, the player state machine, the statistics summary, the string catalogs; and, for the designed screens: the palettes and the selected-tile blend, the scale and safe-area padding, the room name and person name rules, the join address and QR URL rules, the reconnect schedule and the busy memory, the wizard (steps, chips, address cycling, field validation, late discovery, policy lock), the key tables of the Siri Remote, controls reveal, idle dimming, the 1 s guard, the toast, the QR matrices (equal to the web display's encoder at the same mask, lowest penalty of the eight masks, structure), the licence list parsed from the real files and the bundled copies, the host events and the default mode after End, every key used by the Swift views.

## Not verified (summary)

Everything under `Sources/CometCastTV` compiles and works: unknown. In particular (UI): `Font.custom(_:fixedSize:)` with the bundled files and `UIAppFonts`, `View.tracking`, `.focusEffectDisabled()` and `.defaultFocus` removing the tvOS focus look, `TextField` in the plain style with our ring, `onExitCommand(perform: nil)` leaving the app at Idle, the invisible catcher button receiving every remote event while presenting, safe area insets from a `GeometryReader` that ignores the safe area, the 1.03 scale and glow clipping, the SVG template images, `AccessibilityNotification`, the licence viewer's focus moves, the line heights (`lineSpacing`), the transfer pill width; the full list of questions is [design/implementation-questions-appletv.md](design/implementation-questions-appletv.md). Also: the XcodeGen keys (`revision`, scheme keys), SPM resolution of the binary package for tvOS, every `LKRTC` Swift name, the Network and dns_sd glue (including the search domain question), ImageIO WebP, Core Graphics PDF speed and memory, AVPlayer behaviour (cookies, Referer, formats), managed configuration delivery, the focus behaviour of the Siri Remote, VoiceOver, dark mode, German layout (long words), the local network permission flow, Single App Mode, the 12 MB slice's behaviour, the ALTCHA solver speed, and the build of the `tvos` release job.
