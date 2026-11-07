# Implementation questions: Flutter app

Asked by the Flutter implementer. Each item: what was found, the options, and what was done meanwhile (the minimal neutral behaviour). Nothing here was decided by the implementer.

**Section F and the second list of differences answered by the designer on 2026-11-07** after reviewing main f250cc2 and its 362 screenshots (`/tmp/shots-flutter2`, compared with the web's `/tmp/shots4`): decisions N29–N47 in [first-run-and-display.md](first-run-and-display.md) section 13 ("Decisions after the second Flutter review"), native-apps.md 20.5–20.6, work items in [fix-list-flutter-2.md](fix-list-flutter-2.md).

**Sections C and D answered by the designer on 2026-11-07** after reviewing main 66de24c and its 198 screenshots (`/tmp/shots-flutter`, compared with the web screenshots `/tmp/shots3`). The decisions are N1–N27 in [first-run-and-display.md](first-run-and-display.md) section 13 ("Decisions after the Flutter review") and in [native-apps.md](native-apps.md) (sections 3, 4, 8, 9, 10, 20); the work items are in [fix-list-flutter.md](fix-list-flutter.md) (part A: Flutter UI only, part B: needs core, protocol or platform work).

## Core and protocol gaps (not design, but they change what the screens can show)

- **C1 Room name to presenters.** `cc-ffi` does not pass `room_name` to `create_room` and does not report it on `join_pending` / `joined` (the protocol and `cc-receiver` / `cc-sender` have it). Options: add it to `start_receiver` and the sender events (needs a bridge regeneration). Meanwhile: the display shows its own room name; the presenter pages show no room (spinner only, `what.connected` header omitted, no `presenting.where` line), as the spec says for "no room name".
  - **Answered (N1):** build it. `start_receiver` gets `room_name: Option<String>` (the name from setup, sent raw; the server cleans it) and passes it to `ReceiverOptions.room_name`. The presenter events of the new presenter session (native-apps.md 20.1) carry `room_name: Option<String>` on `WaitingForApproval` and `Joined`, read from `join_pending.room_name` / `joined.room_name` with the lenient rule of decision 49 (missing, null, empty, blank, non-string = none). The UI then shows the waiting label, `what.connectedTo` and `presenting.where` (fix list B1, B6). The meanwhile behaviour is correct for "no room name".
- **C2 Security code.** The core computes no security code (the web does it in `core/sas.ts`). Meanwhile the name tag has no second line and `presenting.sas` is not shown.
  - **Answered (N2):** build it in the core, bit-for-bit the web algorithm so a web display and a native presenter (and the reverse) show the same digits: the first `a=fingerprint:sha-256` of the local and of the remote SDP (final offer/answer), lowercase hex, sorted, SHA-256 of `cometcast/connection-check/v1\n{x}\n{y}`, first 4 bytes big-endian mod 1 000 000, zero-padded, shown `ddd ddd`. Reported once per connection as `ConnectionCode { sas: String }` on the receiver events and on the presenter session events; `None`/no event when a fingerprint is missing. A cross-test against `apps/web/src/core/sas.ts` vectors is required. The UI shows it as the second name-tag line (display, only with the controls) and `presenting.sas` (presenter, Presenting page only).
- **C3 Page and page count from the display.** Not reported (decision 13.31), so no "Page n of m" and both page buttons stay enabled (as the spec says until it exists).
  - **Answered (N3, decision 58):** the native display sends `{"kind":"page","page":n,"pages":m}` (1-based) on the content control channel once the page count is known and after every page change; the native presenter reads it (same validation as `apps/web/src/core/control.ts`: positive integers, `page ≤ pages`, at most the PDF page limit; anything else = unknown) and reports `Page { page: u32, pages: u32 }` on the presenter session events. With it: `presenting.pageOf` above the row, Previous disabled on page 1, Next on the last page (disabled secondary look, fix list A10).
- **C4 Receive progress.** The core reports no percent for a running transfer. Meanwhile the transfer pill shows `receive.transfer` with 0 %.
  - **Answered (N4):** build it. New receiver event `ContentTransferProgress { received: u64, total: u64 }`, at most one per whole percent and at most 4 per second, after `ContentTransferStarted`. The pill shows `floor(received × 100 / total)`; 0 % until the first report is correct (it is true). The pill keeps decision 63 (one line, max 340 ref, name shortened in the middle, percentage never cut).
- **C5 Presenter joins only when it sends.** The core joins a room when it starts to send, so the designed order "waiting page, then What to show" cannot be built. Meanwhile: Connect leads to "What to show" without being joined (the "Connected to ..." header is left out because it would not be true); the waiting page appears after the person chose Screen, File or Link.
  - **Answered (N5):** the designed order stays; the core gets one presenter session per join (native-apps.md 20.1): `Connect` joins (waiting page in Approval on, then "What to show" with its header), the presentation is started and stopped inside the joined session, `Stop` keeps the join (the display returns to Idle with the same code, the presenter returns to "What to show" still connected), `Disconnect` leaves. The meanwhile behaviour is not acceptable for release: it asks the host to approve the same person again for every new thing shown, and a second join while one is open would hit `room_full`.
- **C6 Server address of a presenter.** The first-run page has no server field and there are no link handlers for `https://{dom}/{CODE}` yet. Meanwhile the presenter uses `AppSettings.serverAddress` (policy, DNS, last used, built-in). The relay confirmation page is built and tested but not wired to a link.
  - **Answered (N6):** no server field (as built). The server comes from, in this order: a link or scanned QR code, policy, DNS, the in-house box, last used, built-in. Build the link entry (native-apps.md 20.4): Android App Links and iOS Universal Links for the built-in relay's host only (the only domain we can verify), path `/{CODE}` with optional `?server={host}`; the Android install referrer with `code` and `server`. A code from a link fills the chip `app.codeFromLink`. A `server` that is not the built-in relay, not the policy or DNS server and not confirmed on this device before opens the relay confirmation page first (spec 7.2, presenter variant, body `relay.confirm.bodyApp`); Cancel continues with the server the app would use anyway, Use remembers the host as confirmed. A QR code of a display on an own server or box opens the browser, not the app (accepted; the app user types the code or, later, picks the screen from the nearby list).
- **C7 QR scanner.** `app.scanQr` needs a system scanner (ML Kit, VisionKit); none is built. Meanwhile the scan button is hidden and the code field of the computer variant is shown on phones and tablets too.
  - **Answered (N7):** build it on Android (phones and tablets: Google code scanner `GmsBarcodeScanning`, no camera permission) and iOS/iPadOS (VisionKit `DataScannerViewController`, `NSCameraUsageDescription` = `ios.cameraUsage`); not on computers. Devices with a scanner show **both** the secondary button `app.scanQr` and, below it, the code field (spec 5.3 as revised; the field is the way that always works). The scan result goes through the same rules as a link (C6): our URL form with a valid code → chip `app.codeScanned`; an unknown server → relay confirmation page; anything else → no change and the line `app.scanWrong` (13/700 `p-error`) under the scan button until the next tap. Cancelling the scanner changes nothing. "Scan QR code again" on the error pages appears only where the scanner exists. Until it ships, the meanwhile behaviour (code field only) is correct.
- **C8 Display leaves a presentation.** When the presenter leaves, the core's receiver ends; the app starts a new room, so the code changes (spec 4.5 says the same code stays). Toast `toast.left` is shown.
  - **Answered (N8):** the spec stays: the code changes only on End, a restart, a lost and re-made room, or the background rule of native-apps.md 15. `cc-receiver` must keep its signalling connection and room after `PeerLeft` (and after `remove_peer`): tear down the WebRTC session and content task, make a fresh session for the next presenter, keep join mode, blocks and queue, and keep emitting events; `cc-ffi` keeps the stream open. The display shows Idle with the same code and `toast.left`. A presenter who only **stops** (N5) is not a leave: no toast, the display returns to Idle, the presenter stays connected.
- **C9 Mapping of core errors to error pages.** The core reports English messages (`server error <code>: ...`, `signalling connection ...`); the page is chosen by that text (`lib/src/presenter/errors.dart`). Connect/closed/timeout map to `error.noInternet`, WebRTC errors to `error.displayOff`. Please confirm.
  - **Answered (N9):** the mapping is confirmed (it is spec 6), but not the way it is read. The presenter session reports `Error { code: String, message: String }` where `code` is the protocol code (`room_not_found`, `join_denied`, `room_locked`, `room_locked_waiting`, `approval_timeout`, `removed_by_host`, `room_full`, `rate_limited`, `banned`, `server_full`, `proto_mismatch`, `bad_hello`, `bad_message`, `forbidden`, `altcha_*`, `wrong_role`, `not_in_room`) or one of the local codes `connection_lost` (could not reach or lost the server, including timeouts), `peer_failed` (WebRTC failed), `ice_refused` / `ice_failed` (STUN/TURN list, decision 45), `capture_denied`, `capture_unavailable`, `unknown`. `message` is for logs only. `errors.dart` maps codes, never text; `error.other` gets the `errors.<code>` text as its "why". Same `code` field for the receiver's `Error` event (display: offline body choice of 4.2).

## Copy and design gaps

- **D1 `receive.link.hint`** says the link "opens in a new tab". In the apps there is no tab. Used unchanged.
  - **Answered (N10):** new key `receive.link.hintApp` ("… It opens in the browser, and only if you choose “Open”."; spec 11.9) for native displays that can open links; `receive.link.hintTv` where they cannot (native-apps.md 3). `receive.link.hint` stays web-only. Also: the address line in the card is `--fg`, not `--mut` (decision 61), and the Flutter card has no Play button this round (N11).
- **D2 `idle.appLine`.** The apps have no store link configuration, so the footer always says `idle.appLineWeb`.
  - **Answered:** correct as built (decision 6: no store is named that has no listing). When listings exist, the Flutter build gets the same three constants as the web (`STORE_URL_GOOGLE_PLAY`, `STORE_URL_APP_STORE`, `DOWNLOAD_URL_DESKTOP`, `apps/web/src/config/stores.ts`) and the footer follows 4.2. `idle.altAddress` is shown when the display uses an in-house box (native-apps.md 3); the screenshots have no box, so it could not be checked: add a box screenshot (fix list A8).
- **D3 Address source for policy.** `ServerSourceKind.policy` (administrator) uses `setup.address.sourceOwn` (company network); in-house box announcements use `sourceBox`; everything else `sourceDefault`. "Other address" is not offered when a policy locks the server.
  - **Answered (N13):** a policy is not "found via the company network". New key `setup.address.sourcePolicy` ("Set by the administrator of this device."). DNS → `sourceOwn`, box → `sourceBox`, built-in → `sourceDefault`, an address typed into the field → `sourceTyped`. "Other address" hidden when the policy locks the server: confirmed (`forbid_user_override`); a policy that does not lock keeps "Other address".
- **D4 Content errors as toasts.** `UiContentError` is mapped to the `receive.transferError.*` texts (`interrupted` and `transferTimeout` to `stalled`, `renderFailed` and `tooManyErrors` to `document_failed`, `workerUnavailable` to `document_unsupported`). Please confirm.
  - **Answered:** confirmed as built (decision 41: content errors are toasts with the existing wording).
- **D5 Request ended by the server.** `cancelled` and `left` both show `toast.withdrawn`; `timeout` shows `toast.expired`.
  - **Answered:** confirmed; it is what the web display does (`receiverSession.ts`).
- **D6 Codec card.** The H.264 download consent has no designed place. Meanwhile the existing Material card appears centred over the display while an H.264 picture cannot be decoded, and on its own page when a desktop presenter chooses Screen. The spec mentions codec notices as toasts (native-apps.md 18) but a consent needs a button.
  - **Answered (N17):** designed now: spec 4.7 (display: a dialog card of the link-card family, [Download, Not now], over Presenting only, once per presentation) and spec 5.8 (presenter: its own page in the relay-page layout, [Download, Back], then the same page offers `what.screen`, which opens the system picker). No Material `Card`, `FilledButton`, `LinearProgressIndicator` or Material icon remains. native-apps.md 18 ("codec notices as toasts") is replaced by this.
- **D7 Windows Miracast receiver.** No designed place. Meanwhile one small text button "Miracast" at the bottom right of the presenter first-run page (Windows only) opens the old page.
  - **Answered (N18):** the bottom-right button goes. Windows only: a second text link `app.miracast` ("Receive Miracast on this PC"), directly below `app.beDisplay`, same style (it is the same kind of choice: this device becomes a screen). The Miracast page keeps its content and flow (outside this design, spec 10) but is rebuilt with the presenter kit (tokens, Atkinson, `PPage`, `PField`, our buttons and icons, `common.back` as its last action; its consent question as a page in the relay-page layout). No placement conflict with spec 14: the nearby card is in the right column, both links stay at the end of the left column.
- **D8 Nearby screens and advertising.** The list of nearby screens is not designed (spec 12). The old "Nearby receivers" list is gone; the display no longer announces itself in the local network (it was part of the removed receive page).
  - **Answered:** the list is now designed (spec 14) and is the next round. Until then: no list and no announcement (as built). The announcement comes back only in the 14.8 form (instance id, room name, server, mode, code), together with the list; do not restore the old announcement meanwhile.
- **D9 Test video and monitor choice.** The old screen sender offered a VP8 test stream, a duration and a monitor list. These have no place in the designed pages and were removed from the UI (the core API is unchanged). Desktop screen sharing shares the whole desktop.
  - **Answered (N26):** removing test stream, duration and monitor list is confirmed. Sharing the whole desktop is not: `what.screenLaptop` promises "Whole screen or one window" and native-apps.md 8 names the system pickers. Build capture through the system picker on every desktop (xdg-desktop-portal ScreenCast on Linux, `GraphicsCapturePicker` on Windows, `SCContentSharingPicker` on macOS 14+), opened directly by the Screen row; cancel = back to "What to show" without an error (fix list B7).
- **D10 German legacy strings.** Strings of the pre-design UI that are still used (codec card, Miracast, content refusals, capture notification, discovery notes) were converted from Sie to du by changing only pronouns and verb endings. They are not in the spec tables.
  - **Answered (N27):** accepted as converted; the keys are now listed in spec 11.10 with the rule that both German sets carry them and the i18n check covers them. Strings no longer used anywhere (the old discovery notes) are deleted from all ARB files rather than kept.
- **D11 Annotation toolbar** (`lib/annotations.dart`) is kept but not reachable (native-apps.md 9: build flag off); its tests stay.
  - **Answered (N20):** confirmed. It stays unreachable in every build of this round, including tablets and desktops in the display role (owner question in spec 12).
- **D12 Lifecycle on tablets.** The display session ends when the app goes to the background on Android and iOS (`paused`), without a toast, as native-apps.md 15 says. Desktops ignore minimising.
  - **Answered:** confirmed as built. `inactive` (Control Center, a system sheet) does not end the session; only `paused`.
- **D13 Stopping a screen share on the presenter** returns to "What to show" (spec 5.6); on iOS the broadcast extension keeps its own lifecycle (never run).
  - **Answered:** confirmed. With N5 the person stays joined. When the system ends the share (iOS status pill, Android notification "Stop", the Windows/macOS picker's stop), the app treats it exactly like `presenting.stop`: back to "What to show", no error page.

## Visible differences listed in the hand-off

| Difference | Answer |
| --- | --- |
| Name tag without security-code line; presenter pages without room name, "Connected to" header, `presenting.where`, "Page n of m" | Core work C1, C2, C3, C5 (fix list B1–B6) |
| Transfer pill shows 0 % | C4 (B2) |
| Idle footer always `idle.appLineWeb`, no `idle.altAddress` | D2 (correct; add a box screenshot, A8) |
| Phones/tablets show the code field, never `app.scanQr` or a scanned chip | C7 (B5) |
| New code after the presenter leaves | C8 (B2) |
| Relay confirmation page never triggered | C6 (B4) |
| Material codec card; "Miracast" button bottom right | D6, D7 (A12, A13) |
| `receive.link.hint` says "new tab"; no media player in the link card | D1; N11: no Play in Flutter this round |
| Offline box dash pattern and timing approximations | Dash 6 on / 4 off is accepted; but in the screenshots the dashes are not drawn at all and the box fills the column: defect A1 |
| Card and toast shadows hard-edged in headless screenshots | Accepted as a test artefact (`flutter_test` sets `debugDisableShadows`); the shots must be taken with it off (A8) |
| Display text fields open the keyboard only on Enter/tap | Correct (spec 4.1, native-apps.md 8) |
| Android TV Back arrives via `didPopRoute` | Accepted as an implementation detail, provided the double-press exit of 7.1 and the Back column of the key table hold |

## E. Questions from the core round

Questions found while implementing part B of `fix-list-flutter.md` (core, protocol and platform work). Each has what was found, the options, and what was done meanwhile. None of them blocks the code that is in; the first one is the one that leaves a fix-list item open.

- **E1 B7: the system screen picker is not built.**
  - **Found:** the three picker APIs need code that cannot be built or run in this environment. Linux: xdg-desktop-portal ScreenCast over D-Bus plus a PipeWire stream (no PipeWire development files here; `ashpd`/`zbus`/`pipewire` are not in the offline registry and their licences were not checked). Windows: `GraphicsCapturePicker` (WinRT; `cc-miracast` has the `windows` crate but nothing for capture). macOS 14: `SCContentSharingPicker` (ScreenCaptureKit, Swift or Objective-C; no Xcode here). `cc-capture` has the X11 backend only; the Wayland, Windows and macOS backends are `unsupported` stubs.
  - **Options:** (a) a separate round per desktop with the right machine and the crate choices reviewed (licences, MSRV, binary size); (b) one Rust trait `CaptureSource` backend per system behind `present_screen(None)`; (c) keep X11 whole-desktop only on Linux.
  - **Done meanwhile:** the interface is in: `present_screen(screen: Option<ScreenRequest>)` (Rust, bridge, `CoreApi.presentScreen`, `PresenterSessionController.presentScreen`). `Some(request)` is the existing X11 capture; `None` is the system picker and answers `capture_unavailable` (no page for `capture_denied` is needed yet; both are non-fatal and keep the join). Cancelling a picker later maps to `capture_denied` with no change to the interface. Nothing else was changed in `cc-capture`. **Updated in the capture round (section H):** the three backends are built to specification behind `present_screen(None)` and a cancelled picker is `capture_denied`; none was run on its system.
  - **Answered (N55–N57):** design side complete: picker on every share, Windows owner = the app window, `error.capture` for `capture_unavailable` / `capture_os_old` / `capture_no_portal`, no UI for cancel, minimised or closed windows. B7 closes when the three manual test plans pass (fix list 3 B4–B6).
- **E2 B6: how the display learns that a presentation stopped while the presenter stays.**
  - **Found:** the protocol has no message for it (`peer_left` is a leave; the server relays only the setup). The spec leaves the mechanism to the engineers ("renegotiation or a pre-negotiated track").
  - **Options:** (a) a new server message (protocol change); (b) renegotiate one peer connection; (c) a fresh WebRTC session per presentation over the same signalling connection, with the end announced by the control channel.
  - **Done:** (c). Every presentation has its `control` channel next to the media. Stop sends `cmd stop` (a web display closes what it shows), then closes the channel gracefully; the native receiver reports the end of the channel as `PresentationStopped` and makes a fresh session; a new offer of a sender that already had one does the same. No server or protocol change. The room outlives a leaving presenter the same way (`PeerLeft`, fresh session, same code). **Not covered:** a web display does not make a fresh session for the second offer of a sender that is still in the room, so "stop, then show something else" works against native displays only. The web receiver needs the same reset (it is a web change, not in this round). Also: when a presenter disconnects, the display may see `PresentationStopped` and then `PeerLeft` (the channel ends a moment before the server says `peer_left`); the display ends in Idle with `toast.left`, as specified.
- **E3 B6: events and codes beyond the list of 20.1.**
  - **Found:** the list has no events for the preparation of a file (`file.converting`, a refused file) and for the bytes sent; those exist for the old content sender (`Converting`, `Rejected`, `Prepared`, `Progress`, `Sent`).
  - **Done:** `PresenterUiEvent` has `Converting`, `ContentPrepared`, `ContentRejected`, `Progress` and `ContentSent` as well (same meaning as before). `Error` ends the stream except for the two codes of a presentation that could not start (`capture_denied`, `capture_unavailable`); `PresenterSessionController` carries that rule (`presenterStartFailureCodes`). A code that is not a room code at all is reported as `room_not_found`. The receive progress has one event more than "one per percent and four per second" allows at most: the one that completes the transfer is always sent.
- **E4 B6: `ice_refused` and `ice_failed`.** The presenter session asks for `/ice-servers` once per join and, when the connection then fails (`peer_failed`), reports `ice_refused` (`rate_limited`, `banned`, `forbidden`) or `ice_failed` (any other failure); a missing list (404) changes nothing. `503 server_full` on that request counts as `ice_failed`, as `http_error` does on the web. The receiver's `Error.code` is read from the messages of `cc-receiver` (`error_code`, tested against them); it never reports the two ICE codes (the display has no page for them).
- **E5 B8: the verification files hold placeholders.** `deploy/well-known/assetlinks.json` needs the SHA-256 of the release signing certificate and `apple-app-site-association` needs the Apple team id; neither is known here, so both say `REPLACE_WITH_...` (verification fails safely until they are filled in). The host `relay.cometcast.de` is still the planned name (`crates/cc-discovery`); the manifest, the entitlement and the Dart constant `builtInRelay` carry it. Please give the real values or the final host.
- **E6 B9: scanner licences and de-Googled devices.** `GmsBarcodeScanning` (decision N7) is part of Google Play services (proprietary terms, no camera permission needed). `mobile_scanner` (Apache-2.0) was checked and not used: it opens the camera inside the app, which needs the permission the spec wants to avoid. Devices without Play services (de-Googled builds, F-Droid) get no scan button (`isAvailable` is false). Options: accept that, or add an open scanner for those builds later (it needs a design for the camera permission text, which the spec has for iOS only). Recorded in `THIRD_PARTY_NOTICES.md`.
- **E7 B9: the iOS deployment target is 15.0, `DataScannerViewController` needs 16.** Below 16 there is no scan button. Raising the target is a product decision (`ios/Config/CometCast.xcconfig`).
- **E8 B8: links are read by the app, not by Flutter's router.** The app has no routes, and with Flutter's own deep link handling a link would be pushed as a route name. `flutter_deeplinking_enabled` (Android) and `FlutterDeepLinkingEnabled` (iOS) are off; `LinkBridge` (Kotlin, Swift) hands the https link to Dart. The Android path filter is `/......` (six characters), the iOS component `/??????`; a room code with a different length is a browser link.
- **E9 B2: `keepRoomOnPeerLeft`.** The controller flag `restartOnPeerLeft` of the display is renamed `keepRoomOnPeerLeft` because it no longer restarts anything: the room (code, mode, queue, blocks) stays and the presenter's state is reset. The old receive page (flag off) stops the core on `PeerLeft`, as the core no longer ends by itself.

## F. Questions from the UI round

Questions found while implementing part A of `fix-list-flutter.md` and the wiring of part B. Each has what was found, the options, and what was done meanwhile (the minimal neutral behaviour). None blocks the code that is in.

- **F1 A1: the offline box centre and the QR box centre differ by about 14 ref.**
  - **Found:** the acceptance line asks for two things that cannot both hold: the box is "vertically centred in the main row like the steps it replaces" and "its vertical centre within 10 ref of the QR box centre". The QR column holds the QR box (212) and, below it, the caption line (kept in the layout so that nothing jumps), so the QR box sits about 14 ref above the row centre. The web has the same geometry.
  - **Options:** (a) centre the box in the main row (as built), 14 ref from the QR box centre; (b) align the box with the QR box (it then sits 14 ref above the centre of the row); (c) drop the invisible caption while offline (the QR box then moves down 14 ref and the code and QR jump when the connection returns).
  - **Done meanwhile:** (a). The test allows 16 ref.
  - **Answered (N29):** (a) is right and stays; the acceptance line of A1 was wrong and is withdrawn. Box centred in the main row, its centre about 14 ref below the QR box centre (the caption line stays in the layout), body on two lines: all as the web. Keep the test at 16 ref.
- **F2 B6 and iOS: the broadcast extension joins on its own connection.**
  - **Found:** the iOS extension (ReplayKit) makes its own signalling connection with the code and the relay that the app hands over. With a presenter session that joined once, a second sender in the same room would be refused (`room_full`) or asked of the host again.
  - **Options:** (a) the app leaves its session before the extension starts, the extension asks the host again (as built); after the broadcast the person is back at the first page with the same code; (b) the extension joins through the app's session (needs a shared connection or a hand-over of the session, core work); (c) the extension reports into the app, which presents the encoded frames inside the session (a new transport between the two processes).
  - **Done meanwhile:** (a). The "approve once" promise of N5 does not hold for iOS screen sharing; files and links on iOS use the session as everywhere else.
  - **Answered (N32, native-apps.md 20.5):** the promise holds on iOS too. Neither (b) nor (c) as described: the extension keeps its own connection (the app is suspended while the person shows another app), and the approval is handed over: before the broadcast the app gets a single-use token from the display over the control channel, puts it into `config.json`, leaves; the extension joins with the name and the token in an optional `handover` join field, which the display admits without a dialog and without `toast.left` / `toast.connected`; after the broadcast the same hand-over brings the app back to "What to show", still connected. What the iOS user sees: tap Screen → system "Start Broadcast" → the Presenting page (with room and security code from the extension's status) → Stop or the red pill → "What to show". Fallback without a token (old server, End, timeout): the waiting page with the extension's check number, then the normal pages; Locked refuses (accepted). Until it ships, (a) is acceptable in test builds only, with three fixes (no new copy): the extension sends the name; the app shows the waiting page with the extension's check number while it waits; after the broadcast the app joins again with the same code and lands on "What to show" (not the first-run page). Fix list 2 item B1.
- **F3 A8 and B7: the Screen row on desktops still uses the X11 capture.**
  - **Found:** the system picker (E1 of the core round) is not built, `present_screen(None)` answers `capture_unavailable`.
  - **Done meanwhile:** the Screen row calls `present_screen` with the existing `ScreenRequest` (X11, whole desktop); on a system without X11 the person gets `error.other`. The codec page of 5.8 comes first when the encoder is missing, then the same call.
  - **Answered (N33):** acceptable in test builds; release still needs the system picker (B7 stays open). Without X11, `error.other` gets the new `errors.capture_unavailable` as its "why" (spec 11.9) and `common.back` (to "What to show", still connected) instead of Try again; never a technical text.
- **F4 4.7: what the display shows while an H.264 presentation waits for the decoder.**
  - **Found:** the spec places the codec dialog "over Presenting", but without a decoder no picture exists, so nothing makes the display Presenting.
  - **Done meanwhile:** the display counts as Presenting from the moment a presenter is in the room and its track is H.264 and no decoder is ready: black stage, the controls under the scrim (visible, inert), the wake lock held. After "Not now" it stays in that state (the person can still use the controls and End) until the presenter leaves or the decoder is ready.
  - **Answered (N31, spec 4.7):** confirmed, with three changes. The dialog does not reveal the controls (visible ones stay under the scrim and fade on their timer; hidden ones stay hidden). Toasts stay above the scrim, uncovered (toast over anything); the card's top edge never goes above 66 ref so they do not touch. After "Not now", and while a download runs after it, the stage shows the new state line `codec.stageHint` (15/400 `--onblack`, centred, no box) until the picture appears, the presenter stops or leaves, or End (a stop returns the display to Idle as usual).
- **F5 4.5: the single-page error box has one trigger only.**
  - **Found:** `receive.document.pageError.*` has five texts (bad size, failed, memory, page out of range, timeout) but the core reports no per-page error: a page that cannot be taken is `ContentNote.renderFailed` in the controller. The other four have no trigger.
  - **Done meanwhile:** `receive.document.pageError.failed` is shown in the box when a page of the open document cannot be taken. The other four keys stay in the three files for when the core reports reasons.
  - **Answered (N41):** accepted; no core work this round.
- **F6 5.3: "first launch without a code" for `app.scanAgainBody`.**
  - **Found:** the spec says "only on the first launch after an install that brought no code". The only durable signal is the install-referrer flag (`referrerRead`), which is written on the first launch on both platforms.
  - **Done meanwhile:** the line is shown on the first launch (the referrer was not read before) when no link or referrer gave a code, and only where a scanner exists. It does not come back on later launches.
  - **Answered (N42):** accepted as built; a data reset counts as a first launch.
- **F7 B8: links while the app runs.**
  - **Found:** `AppLinks.links` (links that arrive while the app is open) had no consumer.
  - **Done meanwhile:** on the first-run, error and disconnected pages such a link is handled like the one that started the app (chip, or the relay page for an unknown server); while somebody is connected it is ignored.
  - **Answered (N43):** accepted; "connected" includes waiting and presenting; ignored silently (no prompt, no toast).
- **F8 12 / A8: the in-house box line `idle.altAddress` is not wired.**
  - **Found:** the idle screen can show the IP address of an in-house box, but nothing tells `DisplayRoot` that the server in use is a box or what its IP address is (the announcement only gives a name and a server).
  - **Done meanwhile:** the widget takes `altAddress`; the screenshot sets it by hand; the app passes none.
  - **Answered (N40):** wire it. The display's server came from the box when its source is `sourceBox` (the box announcement chosen in setup); `altAddress` = the IPv4 address that announcement resolved to (the A record of its host), passed only when `{dom}` is a host name, and the QR uses it. Re-shoot `display-*-idle-box` from the app path, not by hand (fix list 2 item A9).
- **F9 A14: Office conversion is "available" when the worker is found.**
  - **Done meanwhile:** `file.bodyApp` is used when `officeWorkerPath()` is not null, on every platform that has one; otherwise `file.bodyWeb`.
  - **Answered (N44):** accepted.
- **F10 A9: Windows Miracast page was rebuilt; the pairing choice is three option rows.**
  - **Found:** the old page used a Material dropdown for the pairing method; the presenter kit has no dropdown.
  - **Done meanwhile:** three selectable option rows (selected: 2 px `p-primary` border and a tick). The text and the flow are unchanged. A screenshot of both states is in the set.
  - **Answered (N39):** accepted (single-choice option rows: selected 2 px `p-primary` border, `check` 16 in `p-primary` before the title). Fix the layout around it (fix list 2 items A7, A8): phone actions pinned with ≥ 24 free above them, laptop actions in one row; consent page with only the PIN digits in mono.
- **F11 Strings deleted as unused.** Twenty-six keys that no screen uses any more were deleted from all three files: the web-only texts `bye.body`, `error.outdated.title` and `.now` (the page variants), `error.action.reload`, `error.action.getApp`, `error.action.openDom`, `error.noScreen.nowNoApp`, `file.title`, `file.body`, `receive.link.hint`, `receive.link.address`, `relay.confirm.body`, `relay.errors.mixed`, `setup.address.sourcePage`, `what.screenPhoneBrowser` and `.screenPhoneBrowserNoApp`, `common.or`, `common.cancel`, `idle.offline.bodyOutdated`, and the six `receive.transferError.document*` texts the display never uses (it maps the content errors to the shorter ones). The five `receive.document.pageError.*` texts of the spec were kept (F5). If any other should stay for a later round, restore it from git history.
  - **Answered (N45):** accepted for the Flutter ARB files (the web keeps its own keys).

## G. Visible differences listed in the UI-round hand-off

| # | Difference | Answer |
| --- | --- | --- |
| 1 | Offline box centre about 14 ref off the QR box centre; body on two lines | Accepted (N29, F1); the old acceptance line is withdrawn |
| 2 | Codec dialog about 760 px high at 1920 × 1080 (explain 4 lines, admin note 3) | Fix (N30, fix list 2 A2): 780 wide, explain 3 lines, source 1, admin note 2 in EN, ≈ 626 px; top edge ≥ 66 ref in every state, EN and DE |
| 3 | Codec dialog over a black stage, controls dimmed but visible, "connected" toast not under the scrim | Black stage and toast above the scrim are correct; the dialog must not reveal the controls; `codec.stageHint` after Not now (N31, A2, A3) |
| 4 | Laptop layout: Stop content width 160, left-aligned under the page row | Accepted, it is spec 7.4 and the web (N38) |
| 5 | Phone Miracast page: Start right after the notes | Fix (N39, A7): content scrolls, ≥ 24 free above the pinned actions |
| 6 | Security digits `123  456` look widely spaced in the mono font | Accepted: one mono space, as the web and the join code (N34) |
| 7 | End icon is the plain square stop icon | Accepted: that is the specified icon (N35) |
| 8 | Tablet/TV presenting: 16:9 picture on a 16:10 window with black bars | Accepted: picture letterboxed on black, controls on the canvas (N36) |
| 9 | Presenter tablet: content top-left in the 744 column | Accepted: the column is centred in the window, items left-aligned, as the web (N38) |
| 10 | Windows first-run links: left-aligned on the laptop layout, centred on the phone layout | Accepted; fix the spacing: 44-high tap areas without a gap, 12 (laptop) / 8 (phone) below Connect (N37, A6) |
| 11 | In-house box idle shot made by hand; setup shots by key navigation | Wire `altAddress` (N40, A9); key navigation for setup shots is fine |


## G. Questions from the nearby round

Questions found while building "screens on the same network" (spec 14, native-apps.md 4.1; branch `nearby-screens`). Each has what was found, the options, and what was done meanwhile (the minimal neutral behaviour). None blocks the code that is in.

- **G1 14.8 point 5: a record lifetime of at most 30 s cannot be set with the mDNS library in use.**
  - **Found:** `mdns-sd` 0.21.7 has no public setter for the TTL of a service's records (they go out with 4500 s for TXT and PTR, 120 s for SRV and addresses). A browser that trusts the TTL would keep a display that died without a goodbye for a long time, and `mdns-sd` itself reports a record only when it is new or changed, so an unchanged refresh is invisible to the browser.
  - **Options:** (a) patch or vendor `mdns-sd` to set a 30 s TTL; (b) add a heartbeat field to the TXT record (a protocol change, the record then changes on every refresh and every browser sees it); (c) another library; (d) keep the long TTL and do the expiry on the receiving side (as built).
  - **Done meanwhile:** (d). The display publishes its record again every 10 s; the list drops a row after 30 s without a refresh; the desktop browse is started again every 10 s so that unchanged records are reported again (checked on one machine over real multicast, including 45 s without a change); the Android and Apple bridges report their known services again every 10 s. Not checked: a display that vanishes without any goodbye on a real network; what `NsdManager` and `NWBrowser` do with a service whose records are still cached.
  - **Answered:** (d) accepted on desktops; on Android and iOS a refresh from the system's cache must not keep a row alive: set the record lifetime to ≤ 30 s (patched/newer `mdns-sd`, option (a)) or prove the screen answered; acceptance ≤ 35 s after a display loses power on each platform (N50, 14.8 point 5, fix list 3 B1).
- **G2 native-apps.md 4.1: the announcement of displays through `NsdManager.registerService` and `NWListener` is not built.**
  - **Found:** the table lists them for Android TV and tablets, iPad and Apple TV. The announcement is made once, in the Rust core (`mdns-sd`), for every display that runs the Flutter app. On iOS `mdns-sd` needs the multicast entitlement (docs/ios.md) and probably fails silently without it; on Android it may need a multicast lock that the display path does not hold. The Apple TV (Swift) app is not part of this round and keeps its own announcement.
  - **Options:** (a) native announcers per platform fed from the core's record (core to Dart to channel); (b) request the multicast entitlement and keep `mdns-sd`; (c) accept.
  - **Done meanwhile:** (c) as built; browse (the presenter side) is native on Android and iOS as the table says.
  - **Answered:** (c) accepted for Android (TV, tablets) and computers in the display role, with a `WifiManager.MulticastLock` held while announcing; iPad in the display role must announce with `NWListener` and Apple TV with the 14.8 TXT layout before release (N50, native-apps.md 4.1, fix list 3 B2). Test builds: iPads and Apple TVs are found by code only; nothing drawn changes.
- **G3 14.3 and 14.8 point 10 disagree about the 33rd screen.**
  - **Found:** 14.3 says "At most 32 rows; further screens are not shown", 14.8 point 10 says "drop the oldest-seen beyond that". "Oldest-seen" can mean first found or longest without a refresh.
  - **Done meanwhile:** the core keeps 32 rows and drops the **first found** when a 33rd arrives (the row leaves, the new one is added at the end). One of the two sentences should be changed.
  - **Answered:** Keep the first 32 found; ignore further screens until a row leaves (rows never leave because of others). 14.8 point 10 now says so; the built behaviour is a defect (N49, fix list 3 A5).
- **G4 14.2: no state for "the search cannot run at all" (no multicast, no interface).**
  - **Found:** the table has no row for it. The core reports `Unavailable`.
  - **Done meanwhile:** it is shown as "Nothing found" (`nearby.none` and `nearby.noneWhy`) at once, without waiting for 8 s. A separate text would need copy.
  - **Answered:** Accepted: "Nothing found" at once, no new copy; tried again on a network change or when the page is shown again (N48, 14.2).
- **G5 14.4: after a stale code the other rows are not kept.**
  - **Found:** the spec says the row is removed and the search starts again. A new search has an empty list in the core (rows of an earlier search are never kept, 14.8 point 9), so rows that stay on the page would never expire.
  - **Done meanwhile:** the other rows are cleared too; under the line `nearby.gone` the list fills again within about a second (the searching placeholder shows meanwhile).
  - **Answered:** Accepted (clear and refill), with one fix: `nearby.gone` stays while the list refills and goes only with the next tap or when the page is left; today `_put` and `Removed` clear it within a second (N51, 14.4, fix list 3 A1).
- **G6 14.2 and native-apps.md 4.1: macOS 15 "Local Network" cannot be detected.**
  - **Found:** the desktop apps browse with `mdns-sd` and raw sockets; a refusal by macOS shows as silence, not as `PolicyDenied`. The state "Not allowed" can therefore never appear on macOS, and `nearby.allow` (which would open the Privacy and Security pane) is only reachable if a state is reported.
  - **Done meanwhile:** macOS shows "Nothing found" in that case. The Dart side opens the pane `Privacy_LocalNetwork` when asked.
  - **Answered:** macOS browses with `NWBrowser` (the iOS bridge) so Not asked yet / Not allowed work as on iOS; required for release, "Nothing found" accepted in test builds (N50, native-apps.md 4.1, fix list 3 B3).
- **G7 14.10 "reused without change": `start.or` and `common.cancel` did not exist as Flutter keys.**
  - **Found:** the web has them, the Flutter ARB sets did not (`startOr`, `commonCancel`).
  - **Done meanwhile:** added with the values of the spec (`or`/`oder`, `Cancel`/`Abbrechen`) in the three sets.
  - **Answered:** Accepted (N52).
- **G8 `ios.localNetworkUsage`: the German `InfoPlist.strings` used "Sie".**
  - **Found:** `ios/Runner/de.lproj/InfoPlist.strings` was written in the formal form while the loaded German set is "du".
  - **Done meanwhile:** replaced with the "DE du (shipped)" text of 14.10; the Sie text is in `lib/l10n/sie/app_de.arb` as `iosLocalNetworkUsage`. The English text and `Info.plist` use the new text. The Apple TV prompt text is untouched.
  - **Answered:** Accepted: the shipped German is du; a Sie build takes `iosLocalNetworkUsage` from the Sie set (N52).
- **G9 14.4 and 14.8 point 11: is a confirmed relay also "last used"?**
  - **Found:** "never save an announced server as last used unless the person confirmed it" can be read as "after the confirmation it may be saved as last used". 14.4 says only "the host is remembered as confirmed on this device".
  - **Done meanwhile:** the confirmed host goes into the list of confirmed servers only (it is joined at once next time); it is never put in the server field, so it is never saved as last used. A server that is announced and trusted already (default relay, policy, DNS-SRV) is not saved either: what is saved is the server of the settings.
  - **Answered:** Accepted: a confirmed host goes only into the confirmed servers; never "last used", never into a field (14.8 point 11, N52).
- **G10 14.3: the accessible name of a locked row.**
  - **Found:** `nearby.aria` has a code, a locked row has none.
  - **Done meanwhile:** "{name}, {mode}" (for example "„Clubhouse“, Locked right now"), the row is not focusable.
  - **Answered:** Accepted: "{name}, {mode}" (nearby.aria without the code part), not focusable (14.3, N52).
- **G11 14.2: what the section shows while the platform is asked.**
  - **Found:** between showing the page and the first answer (a few milliseconds, longer on iOS) no state applies.
  - **Done meanwhile:** the section label alone, no placeholder.
  - **Answered:** Accepted: label alone (14.2, N48).
- **G12 mockup against 5.3: Connect.**
  - **Found:** the mockup draws Connect disabled until there is a code; 5.3 (N22) says Connect is always enabled and validates on submit.
  - **Done meanwhile:** as 5.3 (enabled).
  - **Answered:** Accepted: 5.3 wins; Connect always enabled with a code field. The mockup's disabled Connect was wrong (N52). While a row connects, Connect and Scan are disabled (N54).

## H. Questions from the capture round

Questions found while building the desktop system screen picker (B7, E1 above) in `cc-capture`: Linux/Wayland portal + PipeWire, Windows `GraphicsCapturePicker`, macOS `SCContentSharingPicker`. The engineering side is in [capture.md](../capture.md). Each question has what was found, and what was done meanwhile (a neutral behaviour). None of them blocks the code that is in. E1 is answered by the backends below for what can be built without the platforms; **nothing was run on a real Wayland compositor, Windows or macOS**, so B7's acceptance is still open until the manual plans ([Linux](../linux-wayland-test-plan.md), [Windows](../windows-capture-test-plan.md), [macOS](../macos-capture-test-plan.md)) are done.

- **H1 Wayland: does the portal's picker open every time, or once per session?**
  - **Found:** the ScreenCast portal can skip its picker for a share it was told to remember (a restore token). Measurement point spike 1 says "Wayland dialog once per session"; native-apps.md 8 and 20.3 say Screen opens the system picker "directly" and that the person chooses the screen or window, which reads as every time. A token would also re-share the previously chosen screen without asking, which is a privacy decision.
  - **Options:** (a) always show the picker (every share); (b) remember the choice for the running app session only (portal persist mode "transient") and show the picker again after the app restarts; (c) remember across restarts (persist until revoked).
  - **Done meanwhile:** (a). The token store exists, is tested (0600 in the per-user data directory) and is switched off (`PickerOptions::use_restore_token = false`, nothing sets it).
  - **Answered:** (a): the picker on every share, tokens stay off, no remember option. The measurement point "Wayland dialog once per session" is superseded (N55).
- **H2 Wayland: libpipewire is a hard run-time dependency of a build with the `wayland` feature.**
  - **Found:** the `pipewire` crate links `libpipewire-0.3.so.0` dynamically. A Linux app built with the feature does not start on a machine without it, also for X11 users. Every current Wayland desktop has the library; an X11-only or server install may not.
  - **Options:** (a) ship the feature on and declare the library as a package dependency; (b) load PipeWire at run time (`dlopen`) so its absence only turns the portal off (more code, not built); (c) ship two Linux builds.
  - **Done meanwhile:** the feature is not in any default feature set. Builds without it answer `capture_unavailable` on Wayland and use the X11 whole desktop on X11 sessions. Release packaging has to choose (a), (b) or (c); CI builds and tests the feature in its own job.
  - **Answered:** The app must start without libpipewire: load it at run time (b), or declare it in every Linux package format shipped (a); without it Wayland Screen shows `error.capture` with `errors.capture_no_portal` (N57, fix list 3 B5).
- **H3 Windows and macOS: which window owns the picker, and what does a refusal look like?**
  - **Found:** `GraphicsCapturePicker` needs an owner window handle from a desktop app; `cc-capture` does not know the Flutter window. `SCContentSharingPicker` is presented on the main queue of the app. Which error codes each system gives for a cancel were not observed.
  - **Done meanwhile:** Windows uses the foreground window (or `PickerOptions::owner_window`, which `cc-ffi` does not pass yet). Every plausible cancel code is `capture_denied`; any other failure is `capture_unavailable`. No UI is involved.
  - **Needs:** the first run on each system (test plans A1, A4).
  - **Answered:** Pass the Flutter window's `HWND` through `cc-ffi` as `owner_window`; the foreground window is not acceptable for release. Cancel and refusal = `capture_denied` (no page), other failures `capture_unavailable` (page `error.capture`); confirm the codes in test plans A1/A4 (N55, fix list 3 B4).
- **H4 macOS: below macOS 14 there is no system picker.**
  - **Found:** `SCContentSharingPicker` needs macOS 14. For 12.3 to 13 the only ScreenCaptureKit way is to list windows and displays and draw our own chooser, which the designer excluded (N26: no monitor list).
  - **Done meanwhile:** macOS before 14 answers `capture_unavailable` (the page of N33).
  - **Answered:** New code `capture_os_old` → page `error.capture` with `errors.capture_os_old` ("needs macOS 14 or later"); no own chooser (N56, 11.9, fix list 3 A6).
- **H5 Which sources the picker offers.**
  - **Found:** the portal and the macOS picker are asked for one monitor or one window. The Windows picker offers screens and windows by itself. Applications (macOS), multiple windows and virtual monitors are not requested.
  - **Done meanwhile:** as stated. `what.screenLaptop` ("Whole screen or one window") matches.
  - **Answered:** Accepted: what the system offers; `what.screenLaptop` stays (N55).
- **H6 What the viewers should see when a shared window is minimised or the compositor pauses the stream.**
  - **Found:** the stream pauses; the capture then returns "unchanged", so the display keeps the last picture. Nothing tells the viewers that the picture is stale, and the sender never ends the share for it.
  - **Done meanwhile:** keep the last picture, no timeout, no message (no UI was designed for it).
  - **Answered:** Accepted: last picture, no timeout, no message on either side (as browser sharing). A closed shared window or a share ended in the system UI = `PresentingStopped` (presenter to "What to show", display Idle, no toast) (N55, fix list 3 B6).
- **H7 `capture_info` and the Linux "Wayland" answer.**
  - **Found:** `capture_info` (cc-ffi, unchanged) still reports Wayland as not supported; the Flutter app does not call it for the screen flow any more (it uses `present_screen(None)`), so nothing breaks.
  - **Done meanwhile:** left as is. It can be removed or updated with the next bridge change.
  - **Answered:** Accepted; remove or update with the next bridge change (N57).

## I. Differences listed in the nearby and capture hand-off

| Difference | Answer |
| --- | --- |
| During platform resolution the section shows only the label | Accepted (G11, N48) |
| `unavailable` shown as "Nothing found" at once, not after 8 s | Accepted (G4, N48) |
| After a stale code the whole list is cleared | Accepted; the `nearby.gone` line must survive the refill (G5, N51, fix list 3 A1) |
| A locked row's accessible name has no code | Accepted (G10, N52) |
| Connect enabled while the mockup draws it disabled | Accepted, 5.3 wins (G12, N52); disabled only while a row connects (N54, A2) |
| Laptop card edge `#E2E4E9` | Correct: it is the card value of `p-divider` (3.3; dark `#2B2F38`) (N52) |
| iOS Local Network prompt and German InfoPlist use the du copy | Accepted (G8, N52) |
| macOS never shows "Not allowed" | Test builds only; release browses with `NWBrowser` on macOS (G6, N50, B3) |
| More than 32 screens: the first found leaves | Defect: keep the first 32, ignore the rest (G3, N49, A5) |
| After "Use this relay" the rows are those frozen when the relay page opened | Accepted: shown disabled only until the join answers (N51) |
| Relay page uses the generic icon and layout | Correct: it is the presenter variant of 7.2 (N52) |
| Debug banner visible in screenshots | Re-shoot without it (N58, C1) |

## I. Questions from the second UI round

Questions found while implementing [fix-list-flutter-2.md](fix-list-flutter-2.md) (parts A and B1). Each: what was found, the options, what was done meanwhile (the minimal neutral behaviour). Nothing here was decided by the implementer.

- **I1 4.7: the admin note wraps to three lines in English, not two.**
  - **Found:** at 780 wide (712 for the text) `codec.offlineNoDir` (and `codec.offline`) at 14 / 1.4 wraps to **3** lines, so the missing state is about 670 px high at 1920 × 1080, not 626 (spec: "admin 2"). The card still starts at 206 px (103 ref), so the 66 ref rule holds.
  - **Options:** (a) accept the three lines and correct the heights of 4.7 (about 335 ref); (b) shorten the English text; (c) widen the card.
  - **Done meanwhile:** as designed (780, 14 / 1.4), no text changed.
- **I2 4.7: the unsupported state is one line high.**
  - **Found:** `codec.unsupported` is one line at 780 wide, so the card is about 360 px (180 ref) high; spec: about 200 ref (two lines).
  - **Done meanwhile:** none needed; the card is smaller than the specified maximum, still centred.
- **I3 4.7 and fix list A1: what the downloading and failed states show besides their own line.**
  - **Found:** the acceptance lines of A1 name only the line and the row; section 4.7 and its heights (365 / 351 ref) include the explanation, source and admin note in both states.
  - **Done meanwhile:** as 4.7 (body stays in missing, downloading and failed; unsupported is label, text and Close only).
- **I4 20.5 step 6: the extension's alert text for a refused hand-over join.**
  - **Found:** `finishBroadcastWithError` needs a text. "No new copy" leaves nothing for `joinDenied`, `approvalTimeout`, `roomLocked`; the app shows the normal error pages for them.
  - **Options:** (a) the neutral "The broadcast has ended." (`error.none`); (b) three new sentences in the extension strings.
  - **Done meanwhile:** (a), in the extension's `localizedMessage`.
- **I5 20.5 step 3: the token is bound to the presenter's name, and the name may be empty.**
  - **Found:** the display binds a token to the name it knows from `peer_joined`. A person who gave no name is `Guest N` with a number that changes from join to join, so an exact comparison would never match.
  - **Options:** (a) two `Guest N` labels are the same person (the secret token carries the trust); (b) the app must always send a name (a default name is copy); (c) no name binding.
  - **Done meanwhile:** (a), in `cc-receiver`'s `HandoverStore`.
- **I6 20.5 step 3: the token in an Open room.**
  - **Found:** the spec relays `handover` in `join_request`. In mode `open` the server makes no request: the joiner is in at once and the display only sees `peer_joined`, so it could not suppress `toast.connected`.
  - **Options:** (a) the server also relays the field in `peer_joined` (what the display then checks); (b) the server sends a `join_request` for token joins in Open rooms (changes who is asked); (c) accept the toast in Open.
  - **Done meanwhile:** (a): optional `handover` in `peer_joined` for joins that need no decision. The server still neither stores nor checks tokens.
- **I7 20.5: the web display does not hold tokens yet.**
  - **Found:** fix list B1 asks the web receiver to accept the same messages (a web change). The web display has no store and ignores the control message and the field (it reads and drops them; tests added).
  - **Done meanwhile:** the web and Swift decoders tolerate the field (fixtures with `decoders_may_drop`); iOS against a web display uses step 6 (the waiting page with the extension's number). The web handover is not built.
- **I8 20.5: which page the app shows behind the system sheet.**
  - **Found:** the app has left the room before the sheet opens; the spec is silent about the page behind the sheet (the old code showed the start page).
  - **Done meanwhile:** unchanged (the start page); the person is back on "What to show" after the broadcast or after cancelling the sheet.
- **I9 20.5 step 2: "stays in Presenting on black".**
  - **Found:** with a hand-over pending the display keeps Presenting (black stage, wake lock) for at most 60 s when the leaving presenter had something shown; if nothing was shown, Idle at once. No text is shown on the black stage for this.
  - **Done meanwhile:** as written, no line on the black stage.
- **I10 N24 / A11: the emptied room name field selects the first suggestion again.**
  - **Found:** after typing a name and deleting it the wizard falls back to the first suggestion (as built before), so its chip is selected again next to the ringed empty field. The fix list asks only for the ring.
  - **Done meanwhile:** unchanged.
- **I11 N40: where the box IP comes from.**
  - **Found:** the announcement record carries the addresses of the announcing device; the spec says "the IPv4 address the announcement resolved to".
  - **Done meanwhile:** the first IPv4 address of the record (`ServerCandidate.ip`, only for `display-announcement`); shown only when the server's address is a host name, and the QR then encodes `https://{ip}/{CODE}`.
- **I12 Display start-up and the codec download.**
  - **Found:** the display waited for `CodecController.init` (which downloads the codec when a consent is on file) before it showed anything, so it stayed blank for the length of a download.
  - **Done meanwhile:** the display no longer waits for it. No design change.

## J. Questions from the third UI round

Questions found while implementing [fix-list-flutter-3.md](fix-list-flutter-3.md) (parts A and B). Each: what was found, the options, what was done meanwhile (the minimal neutral behaviour). Nothing here was decided by the implementer.

- **J1 A4: 14 or 12 below Connect?**
  - **Found:** the "What" of A4 says the first link starts **14** below Connect (the column gap); its acceptance and N37 (5.3) say the first 44-high tap area starts **12** below Connect (text centred in it, so the text is about 34 below).
  - **Options:** (a) 12 as N37 and the acceptance; (b) 14 as the "What".
  - **Done meanwhile:** (a). The nearby laptop layout now has exactly the same 12 as the layout without the card (the extra 4 that made it 16 is gone); the second link touches the first. Test: `nearby_page_test.dart`, "laptop: the links follow Connect".
  - **Answered:** (a), 12 (spec N60); the 14 in A4's "What" was a writing error. Accepted as built.
- **J2 A6/N55: the desktop app now calls the system picker.**
  - **Found:** the built `_startScreen` always passed an X11 `ScreenRequest` on desktops, so the system picker of the capture round was never reached from the UI (the test plans assumed it).
  - **Done meanwhile:** on desktops `present_screen(None)` (the picker on every tap) unless `COMETCAST_CAPTURE_DISPLAY` forces an X11 display (development and tests). Non-desktop builds are unchanged. Say if the forced-display exception is not wanted.
  - **Answered:** accepted (spec N67): the picker on every tap; `COMETCAST_CAPTURE_DISPLAY` stays as a development and test switch (no UI, not in user docs).
- **J3 B4: where the Windows owner window comes from.**
  - **Found:** "cc-ffi passes the Flutter window's HWND as `owner_window`". The Rust core runs inside the Flutter process, so a bridge function (and a platform channel to read the HWND) would only hand the process's own window back to Rust.
  - **Options:** (a) find the window in Rust (done); (b) a new bridge function plus a channel in the Windows runner.
  - **Done meanwhile:** (a): `cc_capture::windows_wgc::own_window()` (class `FLUTTER_RUNNER_WIN32_WINDOW`, else the first visible top-level window of the process); `PickerOptions::owner_window` still wins when set. No bridge change for it. Never run on Windows.
  - **Answered:** (a) accepted (spec N67); acceptance unchanged: the picker is owned by and in front of the Comet Cast window (Windows test plan).
- **J4 B5: plug-in library instead of a declared dependency; the desktop entry.**
  - **Found:** the spec allows "load PipeWire at run time, or a declared dependency in every package format shipped". This repository ships no Linux package format (only the Flutter bundle), so there is nothing to declare a dependency in. The portal also needs a `.desktop` file for the app id.
  - **Done meanwhile:** run-time loading: the portal and PipeWire code is `libcc_capture_pw.so` (`crates/cc-capture-pw`, loaded with `dlopen`), `libcc_ffi.so` does not link libpipewire (checked with `ldd`). The desktop entry `de.cometcast.cometcast.desktop` has only `Type`, `Name=Comet Cast`, `Exec`, `Terminal` and `StartupWMClass`: no `Icon`, `Comment`, `Categories` (those are an asset and copy decision). Which package formats Comet Cast ships on Linux, and the icon, are open.
  - **Answered:** run-time loading accepted (spec N68). Desktop entry: `Icon=de.cometcast.cometcast`, `GenericName`/`Comment`/`Keywords` EN and DE, `Categories=Office;Presentation;`; icons from `assets/logo/cometcast-app-icon.svg` and the 32/64/512 PNGs into hicolor (exact values in native-apps.md 20.3). Every Linux package format shipped later carries the same entry; which formats is a release choice, not a design one. Fix list 4 A5.
- **J5 B1: a patched mdns-sd in the repository.**
  - **Found:** `mdns-sd` 0.21.7 (the newest release) has no public TTL setter.
  - **Options:** (a) a vendored copy with two public setters (done, `vendor/mdns-sd`, `[patch.crates-io]`; `tools/gen-third-party.mjs` lists crates under `vendor/`); (b) wait for an upstream release; (c) the bridges prove that the screen answered (not possible with `NsdManager` / `NWBrowser`).
  - **Done meanwhile:** (a). Nothing is checked on a real network or device.
  - **Answered:** (a) accepted until an upstream release has a TTL setter (spec N69); expiry is proven by the device test plans.
- **J6 B3: the macOS permission text in German.**
  - **Found:** `NSLocalNetworkUsageDescription` and `NSBonjourServices` are now in the macOS `Info.plist` (English text = `ios.localNetworkUsage`). The macOS project has no `de.lproj/InfoPlist.strings` (N52 wants the du text for iOS); adding one needs a variant group in the macOS project file.
  - **Done meanwhile:** English only on macOS.
  - **Answered:** add the variant group and `de.lproj/InfoPlist.strings` with the du text (spec N70). Fix list 4 A4.
- **J7 B2: Apple TV announces without a room name.**
  - **Found:** the Apple TV app has no setup that asks for a room name (the setup is not designed there), and 14.8 allows "none set → empty".
  - **Done meanwhile:** no `name` key; the row shows `common.noName`. The announcer runs while the receiver shows a code (waiting for a sender, connecting, receiving, sender left), in the foreground, with the policy allowing it.
  - **Answered:** accepted for test builds only (spec N69). The Apple TV display uses the setup wizard of 4.1 (native-apps.md 1); once it sets a room name the `name` key carries it. No new design.
- **J8 A6: styles of the three buttons.**
  - **Found:** 6 names the order and "primary" for Show photo or file only.
  - **Done meanwhile:** Show a link and Back are secondary buttons; initial focus on the primary.
  - **Answered:** accepted as built (spec N65): Show a link and Back secondary, in that order.
- **J9 Not run.**
  - **Found:** B6 (the three capture test plans, Android, iPad, Apple TV, macOS announcements) needs the devices; none exist here.
  - **Done meanwhile:** the plans were updated for the new behaviour (error.capture, closed window, picker owner, app id); nothing was run on a device. Kotlin and Swift were syntax-checked only (tree-sitter).
  - **Answered:** not a design item (spec N69). The plans must pass on devices before release; no design review is needed for them.
- **J10 `-p cc-ffi` clippy with `-D warnings` alone fails on `cc-office` (dead code in `worker.rs`).**
  - **Found:** with `--no-default-features` the `cc-office` worker fields are unused; the same failure exists on the base commit. The CI lints the whole workspace.
  - **Done meanwhile:** clippy for `cc-ffi` was run with `-A dead-code`; no code changed for it.
  - **Answered:** not a design matter (spec N69); left to engineering.

## J. Differences listed in the third UI-round hand-off

Answered in spec 13, "Decisions after the third Flutter round" (N60–N72); work in [fix-list-flutter-4.md](fix-list-flutter-4.md).

1. German laptop card, long name broken mid-word: **Answered:** fix (N61 columns 4 : 5, N62 break rules and title column minimum 160). Fix list 4 A1.
2. "über cas / t.musterfirma.de": **Answered:** fix (N61 width; N62 a host breaks only after a dot). Fix list 4 A1, A2. Also found: the connecting row shrinks (N64, A3).
3. Two-line `app.welcome` in German: **Answered:** accepted (N63).
4. Three `error.capture` buttons in one row, two secondary: **Answered:** accepted (N65).
5. Unsupported codec as `error.capture` inside the codec page: **Answered:** accepted (N66).
6. Windows links 12 below Connect, 44 apart: **Answered:** accepted, correct (N60).
7. Expiry, multicast lock, picker owner not visible: **Answered:** accepted; device test plans (N69).


## K. Questions from the fourth UI round

Work of [fix-list-flutter-4.md](fix-list-flutter-4.md) (A1 to A6). Nothing here blocks; each item was built the plain way and is listed for a decision.

1. **Shot names of the 720-wide laptop pair (A6, N71).**
   - **Found:** N71 names `nearby-laptop-light-de-found` and `-de-connecting` for the 720-wide window, but the 1100-wide German shots already have these names, and `nearby-phone-light-de-found` (360 wide) has the name of the 400-wide shot of the German loop.
   - **Done meanwhile:** the 720-wide pair is written as `nearby-laptop-720-light-de-found` and `-de-connecting`; the 360-wide phone shot keeps the name `nearby-phone-light-de-found` and replaces the 400-wide one of that name (it is written later in the same run).
   - **Question:** keep these names, or should the 400-wide phone shot and the 1100-wide pair get a size in their names too?
2. **Gap between the subtitle and the code when it moves below (N62).**
   - **Found:** N62 says "same style, left-aligned with the title" and names no gap.
   - **Done meanwhile:** no extra gap; the code line (height at least 20, the spinner's height) follows the subtitle directly.
   - **Question:** is that right, or should it be 2 or 4 px lower?
3. **At a 720-wide window the code is under the subtitle in the first two rows.**
   - **Found:** at 720 the card is 338 wide, the row 304, the text column 156 (under 160), so the N62 rule puts the code below. N62 says this does not happen "at the widths of 7.3 and N61"; whether 720 belongs to them is not stated. At 1100 the column is 167 and the code stays on the right.
   - **Done meanwhile:** the rule as written (under 160, code below).
   - **Question:** is the code below at 720 intended?
4. **Dots of an IP address (A2).**
   - **Found:** the break opportunity is added after every dot of the inserted value, so `10.0.0.5:8443` can also break after its dots, when the host alone is wider than the line.
   - **Done meanwhile:** as written; no special case.
5. **When the dot breaks are used (A2, N62).**
   - **Found:** a break opportunity after every dot, always present, makes a host that fits on a line split at a dot in the middle of the line ("über cast." / "musterfirma.de" although "über cast.musterfirma.de" fits on the second line); that contradicts the acceptance of A1 ("with the host whole").
   - **Done meanwhile:** the app adds the U+200B characters only when the run around the host (the host with the "über" or "via" glued to it by the no-break space) is wider than the line alone; otherwise the text is drawn without them (`HostText`, `lib/src/presenter/host_text.dart`). Same for the relay page body.

## L. Questions from round 5 (core)

Asked by the round 5 core implementer (Part B of [fix-list-round5.md](fix-list-round5.md)). Nothing here was decided by the implementer; "did meanwhile" is the minimal neutral behaviour. Items C9 (Apple TV UI, another implementer's area) and the packaging of UxPlay for macOS and Windows (C10) were **not done**; C4 is only the Dart part (see L11).

- **L1 "Persisted switches via cc-ffi settings".** There is no settings store in `cc-ffi`; the app's settings are Dart (`SharedPreferences`, `settings.dart`, fix list F2). Options: (a) the switches (`airplay`, `miracast`, `cast`/TV, Office) stay in `settings.dart`; (b) a small settings file in the core. *Did meanwhile:* nothing in Rust; the core calls take the state as arguments (`start_airplay_receive(name, ask_first)`, `watch_tvs` only while the app runs it, `office_convert_available()` for the Office row). Part A stores `officeConvert`, `otherTvs` and the Beta switches in `settings.dart`. **Answered (N115):** (a), the switches stay in `settings.dart`; no settings file in the core.
- **L2 API additions beyond native-apps.md 21.4.** Locked and "Comet Cast first" need the core to know the join mode and the presentation state: added `airplay_set_locked(bool)` (stops UxPlay once no mirroring runs, starts it again on unlock), `airplay_set_refusing(bool)` (pairings are refused without a request while a Comet Cast presenter presents; with `airplay_answer(false)` alone Open mode would show the PIN) and the event `AirplayUiEvent::NotAnnouncing`. Options: only the spec's calls (the app stops and starts the receiver itself; a running mirror could not be kept). *Did:* the additions; the names are proposals. **Answered (N115):** the additions are accepted with these names (`airplay_set_locked`, `airplay_set_refusing`, `NotAnnouncing`); native-apps.md 21.4 and 21.10 item 2 list them.
- **L3 The mirroring signal.** Chosen and documented in `docs/airplay.md`: UxPlay's own output (`raop_rtp_mirror starting mirroring` after a shown PIN; end `Open connections: 0`, which needs `-d`), the receiver is only offered when the program file contains both texts. **Not verified with a real iPhone** (no device; the lines were read from the installed 1.68 binary). Options: also watch UxPlay's window. *Did:* output lines only. Please add a device test item (one start line per mirroring, the count reaching 0). **Answered (N115):** output lines only, accepted. Device test item added (native-apps.md 21.10 item 2): one start line per mirroring and the count reaching 0 with a real iPhone; AirPlay stays out of release builds until it passed.
- **L4 Cast needs trust anchors and cannot use the picker.** The program embeds no Google trust anchors (docs/cast.md), so a Cast device cannot be verified; `InsecureSkip` would send the screen to anything that answers. Options: ship anchors (needs a decision on licences and provenance), or an explicit "skip" switch. *Did:* `set_cast_trust_anchors(pem)`; without anchors Cast devices are listed with `can_screen` false (the app hides rows with nothing to show). For both ways the screen is the whole screen by the default capture, not the system picker (the Cast OFFER and the HLS stream need the size before the capture starts); on Wayland that fails with `capture_unavailable`. **Answered (N115):** no skip switch, ever. Cast rows exist only in builds that embed anchors whose source and licence are recorded in `THIRD_PARTY_NOTICES.md`; without them only AirPlay TVs are listed (meanwhile behaviour is right). Whole-screen default capture without the picker is accepted for this Beta; Wayland shows `error.capture` (unavailable). native-apps.md 21.10 item 3.
- **L5 TV details the spec leaves open.** (a) "A link is checked as a playable media URL": no network check, only http(s) and a path ending in a media extension (`.mp4 .m4v .mov .m3u8 .mp3 .m4a .aac .wav .mpg .ts .webm`); everything else is `tv_unsupported_content`. (b) JPEG re-encode: quality 90, longest side at most 3840, alpha flattened on white. (c) `tv_pair(pin) -> bool` is async and returns once the TV answered. (d) A Cast `ca` bit 0 (video out) decides "has a screen"; a missing key counts as a TV. (e) Pairing credentials: `set_tv_data_dir(path)`, else the user's config folder (Linux/macOS convention of `cc-airplay-sender`; Windows has no ACL handling, as before). **Answered (N115):** (a) to (e) accepted as built.
- **L6 Typed addresses.** `server_for_address` adds `:443` when no port is typed, also for IP literals and `.local` names; an in-house box on another port must be typed with it (the display shows `{dom}` with its port). `probe_server` treats a TLS failure (self-signed certificate) as `Unreachable`, per "no self-signed exception". **Answered (N115):** accepted; a TLS failure shows `start.addrError` like any unreachable address.
- **L7 iOS share extension.** (a) The spec says "dictionary form" and "limited by a predicate"; a dictionary rule cannot limit a file to PDFs and cannot be combined with a predicate, so `Info.plist` uses the predicate form (`UTI-CONFORMS-TO` image, PDF, url; a file URL also matches `public.url` and is then refused as "no link"). (b) The icons of section 13 are not ported to Swift: SF Symbols (`doc`, `link`, `exclamationmark`) stand in. (c) The address line shows `via {dom}` without "Change" (the extension has no `server_for_address` / probe; the app changes the address). (d) No nearby section (needs `NWBrowser` permission handling) and no scanner, as the spec says for the scanner. (e) The settings travel in `<App Group>/share/share-settings.json`, not in `broadcast/config.json`: the broadcast extension deletes that file after reading it. (f) Sie texts: only the du set is copied; a Sie build must regenerate the `.strings` from the Sie ARB. (g) The tokio stack of the share runtime is 2 MiB (512 KiB overflowed in a debug build); the 64 MiB PDF memory measurement is open (docs/ios.md). **Answered (N116):** (a) predicate accepted, but it must exclude `public.file-url` (no error afterwards for a type we did not register); (b) port the section 13 icons as vector template images, no SF Symbols; (c) accepted: `start.via` alone; (d) no nearby section in the extension (spec changed); (e) `share-settings.json` accepted (native-apps.md 21.3 amended); (f) accepted; (g) the measurement is a release gate. Fix list round 5-2, P5.
- **L8 Licence copies are committed.** The Flutter and Apple TV copies and `apps/web/public/third-party-licenses.txt` (1.5 MB each) are committed so that the builds need no generator (the server image builds the web app from `apps/web` alone, without `tools/`); `tools/gen-third-party.mjs` writes them and `tools/release-guard.sh licenses-fresh` checks them. Option: generate them at build time and git-ignore them (then the Docker web stage needs the root files in its context). UxPlay's licence files are not added (nothing ships UxPlay yet). **Answered (N115):** accepted (committed copies plus `licenses-fresh`).
- **L9 App Links.** `apple-app-site-association` now lists only `/app/??????` and the iOS associated domain is only `cometcast.de` (a second path `/??????` on the same file would also match six-character pages of the website). The Android `relay.cometcast.de/......` filter stays, as the fix list allows. **Answered (N115):** accepted.
- **L10 Tests that could not be run or fail for the environment.** `cargo test -p cc-airplay-sender --test uxplay_interop` fails here with "Connection refused": it starts the real UxPlay and this container has no working Avahi (the same reason as the fixture `uxplay-1.68-real-no-dnssd.txt`); not related to the round. Kotlin and Swift were only checked for balanced brackets (no compiler here). **Answered (N115):** not a design matter; the device tests cover them.
- **L11 Miracast (C4).** Only the Dart side exists: `displayMiracastPairing` (`pin_required`, always) and `miracastStateLine` (adapter unsupported -> `noHw`, feature missing -> `noFeature`, policy -> `policy`, else `failed`) in `lib/core/miracast.dart`, tested. Consent per join mode, frames into the display stage and the stop on Locked/End reuse the existing `cc-miracast-ffi` calls and belong to the display controller of F8; the cases of native-apps.md 21.9 item 3 are in `docs/miracast-device-test-plan.md` section I, **not run** (no Windows 11 hardware). **Answered (N115, N117):** accepted; the Windows device test (plan section I, plus native-apps.md 21.10 item 9b) runs before F9.

## M. Questions from round 5 (UI)

Asked by the round 5 Flutter implementer (Part A of [fix-list-round5.md](fix-list-round5.md), items F1 to F8, F10 to F12; F9 is not done, see M1). Nothing here was decided by the implementer; "did meanwhile" is the minimal neutral behaviour.

1. **F9 (retire the presenter Miracast page and `app.miracast`) is not done.**
   - **Found:** the fix list says "only after F8 passed `docs/miracast-device-test-plan.md` on real Windows hardware". No Windows hardware here; the cases (section I of the plan) are not run.
   - **Done meanwhile:** F8 is built (display Settings row, consent per join mode, frames in the display stage, stop on Locked/End); the link, `miracast_page.dart` and the `miracast*` keys stay. The display stage reuses `MiracastVideoView` from `miracast_page.dart`, so that widget has to move before the page is deleted.
   - **Question:** none, only the order: run the device test, then F9.
   - **Answered (N117):** right order. Run the device test, then F9; move `MiracastVideoView` into the display code first.
2. **`about.publisher` has no name in it.**
   - **Found:** the copy is "Published by Schulverwalter GmbH" (a fixed text), but `branding.g.dart` has `publisherName` (any string); a fork with another publisher would get the wrong sentence.
   - **Options:** (a) a `{name}` placeholder in the key; (b) show the line only for the official build's name.
   - **Did:** the key as given, shown whenever `Branding.publisherName` is not empty (the default build shows nothing). Same in the presenter About and the display About dialog.
   - **Answered (N108):** (a). `about.publisher` = "Publisher: {name}" / "Herausgeber: {name}", hidden when the name is empty; privacy and imprint links each only when their URL is set.
3. **Hiding a TV by the address of a Comet Cast row (N84, 15.5) cannot use IP addresses yet.**
   - **Found:** `NearbyScreenInfo` carries `server` (the relay address) but no addresses of the screen; the IP of an in-house screen that is also an AirPlay receiver is not available to Dart. The fix list names the rule "hidden when the IP belongs to a Comet Cast row".
   - **Options:** an `addresses` field on `NearbyScreenInfo` (changes the bridge); or the core hides the TV itself.
   - **Did:** `TvController.setHiddenAddresses` is fed with the host part of every Comet Cast row's `server`; it matches only where a screen announces its own IP as its server. Tested with that hook.
   - **Answered (N105):** add `addresses` to `NearbyScreenInfo` (native-apps.md 21.10 item 1); hide a TV when any of its addresses equals an address of a Comet Cast row or the host of its server. The hook stays meanwhile for Beta builds.
4. **Filter label of the file dialog.**
   - **Found:** `send.content.pickerTypes` says "Images, PDF and Office files"; the reduced sets (Office off, PDF only, photos for a TV) need no such text and no key exists for them.
   - **Did:** the label is passed only for the full set; the reduced sets pass an empty label (on GTK the filter shows no name). 
   - **Question:** do the reduced filters want names (new keys)?
   - **Answered (N109):** yes, three new keys: `send.content.pickerTypesNoOffice`, `send.content.pickerPdf`, `send.content.pickerPhotos` (15.13).
5. **Gaps the spec does not give.**
   - 5 between the code field and the address line (`start.via`), tap area 44 for the link; the Use/Cancel row below the field on the phone and on the right when the column is at least 480 wide on the laptop; the TV section's gap is 14 above its label and 8 below it.
   - The licence viewer rows: 40 high with 2 between them; the group labels have no fixed height (10 above, 4 below).
   - **Question:** are these right?
   - **Answered (N99, N100):** the address line is a 44-high box from the field's bottom edge (no 5 gap), the same box in the policy variant; Use/Cancel placement, TV gaps 14/8, licence rows 40 + 2 and labels 10/4 are right.
6. **Where "Flutter engine" sits in the list.**
   - **Found:** spec 15.9 names the entry but not its place. **Did:** last in "All parts" (presenter and display). Its text is read from `LicenseRegistry` when the entry is opened.
   - **Answered (N102):** last in "All parts" is right; give it the subtitle `about.flutterSub`.
7. **Licence entries from the notices.**
   - **Found:** the names of the featured notices are not in the files (`## OpenH264 notice`). **Did:** fixed names "Cisco OpenH264", "Atkinson Hyperlegible", "Open Screen Library", "PDFium Foxit font", "Adobe CMap"; the text of a notice is the section body without fences; of a `####` entry the fenced block (so the subtitle is the first licence line, not "Used by"). "Liberation, Carlito, Caladea" is the whole Office-fonts section (subtitle cut at 60 characters). Open Screen Library only on desktop builds, the fonts where the Office worker is found, UxPlay only when its licence asset exists (none yet: the name `UxPlay` is not wired to a file).
   - **Answered (N106):** names and text rules accepted; the UxPlay entry is required wherever UxPlay ships (name "UxPlay", subtitle "GPL-3.0", the licence file plus the bundled parts' notices).
8. **The Miracast PIN.**
   - **Found:** with `pin_required` the PIN comes with the pending request (`MiracastRequest.pin`); the plan item I2 does not say whether Windows draws a PIN of its own as well.
   - **Did:** the PIN dialog shows `pending.pin` (groups of 4); no dialog when the request has no PIN.
   - **Answered (N110):** right. No PIN, no dialog (the pairing is refused). Add the check "Windows draws no second PIN" to the device test. Set the PIN left-aligned in both dialogs.
9. **One queue for Comet Cast and AirPlay/Miracast requests.**
   - **Found:** the Comet Cast queue (`ReceiverController`) and the receive requests are separate; no common arrival time.
   - **Did:** a way request shows when no Comet Cast request waits; a way request that is showing is not replaced by a Comet Cast request that arrives later; both waiting at once: Comet Cast first.
   - **Answered (N111):** one FIFO by arrival time at the display controller (Dart timestamp); "Comet Cast first when both wait" is replaced. N88 refusal while Comet Cast presents stays.
10. **Display Settings height.** With both rows the dialog is 510 ref high (EN and DE) at 1920 x 1080 against "about 480" in 15.4; it never scrolls. Nothing was shrunk. **Answered (N97):** 510 is accepted as the maximum; every state line must fit one line (the DE `failed` text is shortened).
11. **Share page, Android.**
    - Cancel (and system Back) on the share page also leaves the app's activity (`SystemNavigator.pop`), to return to the app that shared, also when the app held a join; the join then ends with the app. "Show on {room}" while waiting for the approval waits for the join and then presents.
    - The not-connected variant shows no `app.beDisplay` link and no settings button (as 15.3 lists the parts).
    - **Answered (N112):** Cancel and Back move the activity to the back (`moveTaskToBack`), keeping the app and its join; finish only after a cold start. The other two points are accepted.
12. **Licence viewer focus.** On computers (with the search field) the first list row takes the initial focus too, so that Right reaches the pane at once; the spec gives the D-pad rules only. **Answered (N100):** accepted.
13. **`tv.aria` and the Beta table.** The key's text contains "Beta" literally; when `betaFeatures` loses `otherTvs` the row label falls back to "{name}, {way}" composed in code (no key). **Answered (N103):** `tv.aria` becomes "{name}, {way}"; ", Beta" is appended by the 15.10 rule while in the table.
14. **`idle.appLine` and `/app`.** The key text is "…{dom}/app"; where the join address already ends in `/app` the code passes `{dom}` without it (the same words). No new key. **Answered (N117):** accepted.
15. **The About web line on the display** (`about.webTv`, `{url}`): the host of the privacy URL of the branding file; hidden when the build has no privacy or imprint URL. **Answered (N108):** accepted.
16. **The iOS share settings channel** (`ios_share_settings.dart`) was not used by anything. It is now: read once at the start (the name the extension changed wins), written on Connect, on a settings change and after a joined session. Not run on a device. **Answered (N117):** accepted; verified in the iOS device test.

**Visible differences of the round 5 UI hand-off, answered** (spec 15.14): (1) Settings 510 ref: N97, accepted as the maximum, DE `failed` shortened. (2) Brand row in the left column: N98, it spans both columns. (3) 5 px above the address line: N99, a 44-high box from the field edge; Use/Cancel placement accepted. (4) Licence rows 40 + 2, labels unfixed: N100, accepted. (5) Name tag fade 400 ms: N101, stays; shots after 500 ms. (6) "Flutter engine" without subtitle: N102, `about.flutterSub`. (7) TV accessible name in code: N103, accepted with `tv.aria` = "{name}, {way}". (8) About pages not `PPage`: N104, accepted if Back is fixed at the bottom on phones. (9) TV hiding by IP: N105, core field `NearbyScreenInfo.addresses`. (10) No UxPlay entry: N106, required where UxPlay ships. (11) iOS sheet and web footer not drawn: N107, iOS shots during the device test; web footer reviewed (separator fix).

## N. Questions from round 5 fixes

Asked by the implementer of [fix-list-round5-2.md](fix-list-round5-2.md) (U1 to U16, P1 to P8). Nothing here was decided by the implementer; "did meanwhile" is the minimal neutral behaviour.

1. **U16 / F9: the presenter Miracast page is not retired.**
   - **Found:** U16 says "F9 after the Miracast device test" (P6, section I of `docs/miracast-device-test-plan.md`, native-apps.md 21.10 item 9b). No Windows hardware exists here, so the test did not run.
   - **Options:** wait (the order of N117); or delete the page and the `miracast*` keys now.
   - **Did meanwhile:** only the first step of N117: `MiracastVideoView` (with `FrameDecoder` and `decodeMiracastFrame`) moved to `lib/src/display/miracast_view.dart`; the link, `miracast_page.dart` and the keys stay. The plan has two new cases (I10 "no second PIN", I11).
2. **P2 / P8 (C10): what counts as "UxPlay ships".**
   - **Found:** no packaging bundles UxPlay (`THIRD_PARTY_NOTICES.md`: "no UxPlay binary or source is bundled"); the Linux `.deb` only has `Recommends: uxplay`. N106 asks for the entry "wherever UxPlay is shipped" and a guard that fails otherwise.
   - **Options:** (a) the guard looks at the packaged bundle for the program; (b) a `Recommends` counts too, so every build that can list the AirPlay row carries the licence file.
   - **Did meanwhile:** (a). The app lists "UxPlay · GPL-3.0" second when `assets/licenses/uxplay/UXPLAY_LICENSE.txt` exists in the build (the folder is declared in `pubspec.yaml`, empty otherwise); `node tools/gen-third-party.mjs --uxplay-licence FILE [--uxplay-notices FILE]` writes that file (licence file followed by the notices of the bundled parts); `tools/release-guard.sh uxplay-licence DIR` fails a bundle with a `uxplay` or `uxplay.exe` but no `UXPLAY_LICENSE.txt`, and runs in the Linux job on the `.deb` root. The macOS and Windows packaging of UxPlay and Miracast (C10) was not done: there is nothing to package yet.
3. **P3: `DeviceAuth::InsecureSkip` and `--insecure-skip-device-auth` still exist in `cc-cast` and `cometcast-cli`.**
   - **Found:** N115 and native-apps.md 21.10 item 3 say "no option that skips certificate verification". The app and `cc-ffi` never use it (`start_tv_presenter` builds `DeviceAuth::Verify`), but seven loopback tests of `cc-cast` and the developer flag of the CLI use it.
   - **Options:** remove it from the library and the CLI (the seven loopback tests then need generated anchors); or keep it for developers and tests.
   - **Did meanwhile:** kept; no way to reach it from the app. `THIRD_PARTY_NOTICES.md` has a row with the rule that a build may embed or ship Cast trust anchors only after a row names their source and licence, and says that the app has no such option. Without anchors the app lists no Cast TVs (`can_screen` false, the existing test `what_each_way_offers`).
4. **U13: arrival order is a sequence number, not a clock.**
   - **Found:** N111 allows "a Dart timestamp". Two requests in the same notification, or a frozen test clock, would tie.
   - **Did meanwhile:** `DisplayRoot` numbers every request as it first reaches the controller (`_orderRequests`); the way request is shown when no Comet Cast request that arrived before it waits. The "more waiting" count of the Comet Cast dialog (`request.queue`) still counts only Comet Cast requests; a way dialog shows none. **Question:** should the count include the other kind?
5. **U7: nothing separates the scrolling content from the fixed Back on the phone About pages.**
   - **Found:** the list passes under the fixed area without a divider or fade (the same as the other phone pages with a pinned action area). A row can be cut by the area's top edge.
   - **Did meanwhile:** no divider.
6. **U9: the not-connected share page while converting.**
   - **Found:** N113 says "both buttons are disabled". Only the connected variant has two buttons (Show on …, Another screen), which are disabled. The not-connected variant (start page with `share.show` and Scan) is not reachable while converting, because Connect is disabled while the join runs.
   - **Did meanwhile:** unchanged there.
7. **U2: the laptop layout without the nearby card.**
   - **Found:** N98 names "both columns (N61)". With no nearby card there is one column; the brand row there stays in the column (it already is the full content width).
   - **Did meanwhile:** the spanning row only in the two-column layout. In German the left column still wraps `app.welcome` to two lines (N63, accepted).
8. **U4: geometry of the pane's focus ring.**
   - **Found:** N100 says "outside the pane's border box" and names no numbers.
   - **Did meanwhile:** the ring of 3.5 (glow 9, outline 4 at offset 3) around the unchanged 1 px border. The current list row has a 2 px border always (clear when not current) with its padding reduced from 10 to 8, so the text keeps its place.
9. **P5: the error disc of the iOS share extension.**
   - **Found:** N116 bundles `file`, `link`, `info`; the app picks an icon per error kind (`x`, `clock`, `locked` ...).
   - **Did meanwhile:** the error pages of the extension use `info` (the old `exclamationmark` symbol is gone). The activation predicate excludes `public.file-url` only in the web-address branch, so an image or PDF that also registers a file URL still lists Comet Cast (the ZIP and Word cases do not). Never compiled or run.
10. **P8 (C9): the Apple TV part is Swift without a compiler; no tvOS screenshots.**
    - **Found:** no Xcode here. The About dialog and licence viewer already existed from round 5.
    - **Did meanwhile:** `AboutView.swift`: `about.publisher` with the name and only when the branding name is set, `about.sourceTv` from the branding source URL, `about.webTv` only with a privacy URL (its host), the current licence row with a 2 px `--acc` border (also in OLED black); the string catalogs were regenerated (`tools/gen-appletv-strings.mjs`). **Not done:** the tvOS screenshots in Dark, Light and OLED black.
11. **P1: reporting address changes of a row.**
    - **Found:** `NearbyScreen.addresses` is the union of the addresses of all instances of the row.
    - **Did meanwhile:** a second instance that adds an address makes one `Updated` event (the old test expected none); a goodbye of one instance shrinks the list without an event. Link-local IPv6 addresses are kept as the backend gives them; the Dart side compares normalised text (lower case, no brackets or zone, IPv4-mapped as IPv4).
12. **P7: what tells the web display that an offer is a new session.**
    - **Found:** a web presenter renegotiates on one peer connection (adds or removes its screen), the native presenter makes a new connection per presentation; the native receiver resets on every second offer, which would break the web sender.
    - **Did meanwhile:** the web receiver resets when the offer comes from another peer connection: the session id of the `o=` line or the ICE username fragment differs from the last offer. Offers without these lines count as renegotiation. No toast (as on the native receiver).
13. **U15: shots of the name tag.**
    - **Found:** one long `pump` draws only its first frame, so the fade was caught at its start.
    - **Did meanwhile:** the shot test pumps one frame and then 700 ms; `r5-display-miracast-presenting-*` shows the tag fully.
