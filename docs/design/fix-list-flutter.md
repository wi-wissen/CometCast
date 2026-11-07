# Fix list: Flutter app (display and presenter)

From the designer's review of `apps/flutter` on main 66de24c (2026-11-07) and its 198 screenshots in `/tmp/shots-flutter` (display: TV 1920 × 1080 and tablet 1280 × 800 in Dark, Light, OLED black; presenter: phone 400 × 800 and tablet 1280 × 800 in Light and Dark), compared with the web screenshots in `/tmp/shots3`. Decisions: spec section 13, "Decisions after the Flutter review" (N1–N28); answers: [implementation-questions-flutter.md](implementation-questions-flutter.md); core interface: [native-apps.md](native-apps.md) section 20.

What already matches the spec and the web build and needs no work: the idle screen geometry (padding, steps, address, code, QR box, pill, footer, Settings button; measured against `display-1280-dark-idle`), Locked (code at 35 %, QR at 25 %, `idle.locked`, `idle.qrLocked`), the connecting state with the empty QR box, the Settings dialog and the join request in all three appearances (card 600, tiles, selected-tile fills, OLED check box border, "+1 waiting", `settings.closeRemote` on TV), the presenting controls (name tag and End 12 from the edges, 46 high), toasts (position, colours), setup steps 0, 3 and 4, the waiting page, "What to show", the link page, the file page, the presenting page, the disconnected page without `bye.body`, every error page, the relay page layout, the icons, OLED black everywhere (no tints, `--line` borders only), TV legibility (all sizes are the spec's reference sizes × 2 at 1080p). The Material look is gone everywhere except the codec card (A12).

Order: do part A top to bottom; part B can run in parallel (it is core and platform work) and ends with B10, the UI wiring that needs it. Each item: **What**, **Where**, **Acceptance** (screenshot names are in `/tmp/shots-flutter` unless a path is given; re-shoot with the harness of A8).

## Part A: Flutter UI only

### A1. Offline box: draw the dashes, hug the content
- **What:** the dashed 2 px `--line` border is not drawn at all (the painter paints under the box's own fill), and the box stretches over the whole main row (from below the room name to above the footer). It must be a box of its content height, vertically centred in the main row like the steps it replaces, padding 16 20, title 22/700, body 18 / line height 1.35 (as `.cd-offline` on the web). Dash 6 on / 4 off is accepted.
- **Where:** `lib/src/display/screens.dart`, `_OfflineBox` (`CustomPaint(painter: …)` → `foregroundPainter`) and its parent in `IdleScreen` (do not let it take the full height).
- **Acceptance:** `display-{tv,tablet}-{dark,light,oled}-idle-offline`: dashes visible on all four sides in all three appearances (OLED: `#2C313C` dashes on black); box height about 116 ref; its vertical centre within 10 ref of the QR box centre; compare `/tmp/shots3/display-1280-{dark,light,oled}-idle-offline.png`.

### A2. Initial focus visible in the first frame, one ring only
- **What:** in every shot taken right after a screen or dialog appears, no control has the ring and the Settings button under the scrim still has it (decision N24, spec 7.1: exactly one ring). The initial focus is requested in a post-frame callback, so the first frame is wrong. Take the initial focus while the dialog or step is first built (an autofocus node in the dialog's own `FocusScope`), so the frame that shows the dialog shows the ring on the right control and none under the scrim.
- **Where:** `lib/src/display/kit.dart` (`DisplayControl` initial focus), the dialogs in `screens.dart`, `setup_wizard.dart`.
- **Acceptance:** with A8's harness (at most one extra frame): `display-*-settings` ring on the current-mode tile (Approval on), `-request` on Yes, `-linkcard` on Open, `-setup0-welcome` Start, `-setup1-name` Next, `-setup2-mode` the selected card, `-setup3-address` Looks right, `-setup4-done` Finish, `-idle-connecting` and `display-tablet-*-idle` the Settings button; in every dialog shot the Settings button under the scrim has no ring.

### A3. Wizard card top edge 40
- **What:** steps 1–4 have the card's top at 64; decision 64 moved it to 40, with at least 24 free below in EN and DE. Today step 1 on TV ends 8 ref above the canvas edge.
- **Where:** `lib/src/display/setup_wizard.dart`.
- **Acceptance:** `display-*-setup1-name` … `-setup4-done`: card top at 40 ref (TV 80 px; tablet 40 + 53 = 93 px); step 1 bottom ≤ 516 ref in EN and DE.

### A4. Wizard choice cards: selected look, equal height
- **What:** the selected card (Open) has the `--acc` border but the plain `--panel2` fill, and it is shorter than the other card (the cards are centred, not stretched). Decision 66: the selected card uses the selected-tile look (Dark `#3E4053`, Light `#D7D3CF`, OLED black no fill), both cards have the same top and height (the taller one's).
- **Where:** `setup_wizard.dart`, step 2.
- **Acceptance:** `display-*-setup2-mode`: both cards top-aligned and equal height; selected fill as stated; focus on the unselected card shows the ring, not a second amber border.

### A5. Link card: address colour and hint text
- **What:** the address is `--mut`; decision 61 makes it 18 mono `--fg`. The hint says "opens in a new tab"; use `receive.link.hintApp` where Open exists and `receive.link.hintTv` where it does not (N10). No Play button this round (N11).
- **Where:** `screens.dart`, link card.
- **Acceptance:** `display-*-linkcard`: address in `--fg` (same colour as the name in `-request`); hint "… It opens in the browser, and only if you choose “Open”."; a TV shot without a browser shows only [Dismiss] and `hintTv`.

### A6. `{dom}` without the default port
- **What:** setup step 3 shows `relay.cometcast.de:443`. `:port` only when it is not 443 (N12), the same everywhere `{dom}` appears and in the QR URL.
- **Where:** wherever the server address is turned into display text or the QR URL (`display_logic.dart`, `setup_wizard.dart`, `screens.dart`).
- **Acceptance:** `display-*-setup3-address` shows `relay.cometcast.de`; the idle QR decodes to `https://{host}/{CODE}` without `:443`; a non-443 test server keeps its port.

### A7. Policy source line
- **What:** a policy server says `setup.address.sourceOwn`; use the new `setup.address.sourcePolicy` (N13). "Other address" stays hidden only when the policy locks the server.
- **Where:** `setup_wizard.dart` (source mapping).
- **Acceptance:** a test with `ServerSourceKind.policy` shows "Set by the administrator of this device."

### A8. Screenshot harness
- **What:** (1) set `debugDisableShadows = false` while shooting (the hard-edged slabs under toasts and dialogs are a `flutter_test` artefact, N21; the product's shadows stay as in spec 3.5); (2) pump one more frame before every shot; (3) the dimmed shot is not dimmed: pump the 7 min, then 2.1 s of frames, and check the overlay (`--bg` at 60 %); (4) add the missing states: display: idle Locked without toast, request queue, setup1 with the field emptied, setup3 field open and field error, offline busy and outdated (app text), idle with an in-house box (`idle.altAddress`), presenting controls with the security-code line, transfer pill with a percentage, PDF page error box, toast over a presentation, TV link card without a browser, the codec dialog in its four states (1920 × 1080 desktop display), and DE (du) shots of setup 0–4, idle, Settings and request in Dark; presenter: first-run with scan button and code field (phone), with `app.scanWrong`, tablet first-run with `app.beDisplay` and (Windows) `app.miracast`, waiting with a room name, "What to show" with its header, presenting with `presenting.where`, `presenting.sas` and "Page 1 of 3" (Previous disabled), presenting a link, error locked-while-waiting, displayOff with the relay "why", busy, the codec page in its four states, the Miracast page, the relay page with `bodyApp`.
- **Where:** `test/screenshots_test.dart`, `test/shot.dart`.
- **Acceptance:** the new set exists; `display-*-idle-dimmed` is visibly dimmed like `/tmp/shots3/display-1280-*-idle-dimmed.png`; toast and dialog shadows are soft.

### A9. Presenter first-run page
- **What (N22, spec 5.3):** (1) the brand row (logo 28 + "Comet Cast", as on the web start page) is missing; (2) the code field has no placeholder: `PField.hint` is never drawn (`EditableText` has no hint); draw `XXX XXX` in `p-fg3` with the code style (22 mono, .12em) while the field is empty; (3) order: brand, `app.welcome`, name field, then the code part (chip, or scan button + code field), `app.connect`; (4) tablets and computers: `app.beDisplay` comes **after** Connect (it is above it today), Windows: `app.miracast` below it (A13); (5) `app.scanAgainBody` only on the first launch without a code.
- **Where:** `lib/src/presenter/pages.dart` (`FirstRunPage`), `kit.dart` (`PField`), `presenter_root.dart`.
- **Acceptance:** `presenter-{phone,tablet}-{light,dark}-firstrun-code`, `-chip`, `-empty-error`: brand row and placeholder as in `/tmp/shots3/presenter-phone-light-start-typed.png`; name above the code part; on the tablet the link is the last item, below Connect.

### A10. Secondary buttons: laptop page row and disabled look
- **What (N23):** on the laptop layout the two PDF page buttons are full width; the web makes them content width (min 160), side by side, left-aligned. A disabled secondary button uses `p-disabled` fill, no border, `p-ondisabled` text (it shows once the page message exists, B4). Secondary text stays 16/500 (as the web; native-apps.md 10 is corrected).
- **Where:** `pages.dart` (`PresentingPage`), `kit.dart` (`PSecondaryButton`).
- **Acceptance:** `presenter-tablet-*-presenting-pdf` matches `/tmp/shots3/presenter-1280-dark-presenting-pdf.png` in button widths; a test with page 1 of 3 shows Previous in the disabled look.

### A11. Screen row subtitle on tablets
- **What (N15):** tablets get `what.screenApp` ("Your whole phone display"). Choose by device kind: phone `what.screenApp`, tablet `what.screenTablet`, computer `what.screenLaptop` (today a `laptop` flag decides, which is wrong for tablets).
- **Where:** `pages.dart` (`WhatPage`), `presenter_root.dart`.
- **Acceptance:** Android tablet and iPad show "Your whole tablet display, live"; phones and computers unchanged.

### A12. Video codec dialog and page
- **What (N17):** replace `CodecNotice` (Material `Card`, `Icons.movie_outlined`, `FilledButton`, `LinearProgressIndicator`) by the display dialog of spec 4.7 (over Presenting only, once per presentation, [Download, Not now], 1 s guard, Back = Not now; downloading, failed and unsupported states) and the presenter page of spec 5.8 (relay-page layout, [Download, Back], then the primary becomes `what.screen` and opens the system picker).
- **Where:** `lib/codec_notice.dart` (replace), `lib/src/display/display_root.dart` (`_CodecLayer`), `lib/src/presenter/presenter_root.dart` (`PresenterPage.codec`).
- **Acceptance:** the eight codec shots of A8 match 4.7 / 5.8; no Material widget or icon in them; the display dialog never appears over Idle and not twice in one presentation.

### A13. Miracast entry
- **What (N18):** remove the small "Miracast" text button at the bottom right of the first-run page; Windows only, add the text link `app.miracast` below `app.beDisplay` (same style). Rebuild the Miracast page with the presenter kit (tokens, Atkinson, `PPage`, `PField`, `PPrimaryButton` / `PSecondaryButton`, our icons; `common.back` as its last action; its consent question as a page in the relay-page layout). Content and flow unchanged.
- **Where:** `lib/src/app_shell.dart`, `lib/miracast_page.dart`.
- **Acceptance:** Windows first-run shot shows the two links stacked under Connect; no Material component left in the Miracast page shot.

### A14. File page text
- **What (N16):** apps use the browser variant (as built). Where Office conversion is available (desktops with the conversion component), the body is `file.bodyApp`; while converting, the progress line shows `file.converting` (small print, no bar).
- **Where:** `pages.dart` (`FilePage`), `presenter_root.dart`.
- **Acceptance:** desktop file page names Office files; a `.pptx` pick shows "Converting slides.pptx …" until `Prepared`.

### A15. "Out of date" in the apps
- **What (N14):** the presenter page says "This page is out of date" with "Reload page"; the display's offline box says "This page is out of date. Reload it". Apps: `error.outdated.titleApp`, why `error.outdated.why`, now `error.outdated.nowApp`, button `error.action.retry`; display `idle.offline.bodyOutdatedApp`.
- **Where:** `pages.dart` (`ErrorPage`), `screens.dart` (`_OfflineBox`).
- **Acceptance:** `presenter-*-error-outdated` reads "This app is out of date" with "Try again"; the outdated offline shot of A8 reads "This app is out of date. Update it to get a code."; the word "Reload" appears nowhere in the apps.

### A16. Relay confirmation text in the app
- **What (N6):** the body says "A link asks this page …"; use `relay.confirm.bodyApp`.
- **Where:** `pages.dart` (`RelayConfirmPage`).
- **Acceptance:** `presenter-*-relay` reads "A link asks this app to use the relay relay.example.org …".

### A17. Strings
- **What:** add every key of spec 11.9 to `app_en.arb`, `app_de.arb` (du) and `sie/app_de.arb` (Sie) with the given values (`codec.downloading` gets the typographic ellipsis); delete strings nothing uses any more (old discovery notes, test-stream and monitor texts, N27); the i18n check covers the legacy keys of 11.10 in both German sets.
- **Where:** `lib/l10n/`.
- **Acceptance:** `flutter gen-l10n` clean; `l10n_sets_test.dart` passes for du and Sie; switching to the Sie set changes only addressing strings (native-apps.md 19.5).

## Part B: needs core, protocol or platform work

### B1. Room name to the server and to presenters (C1, N1)
- **What:** `cc-ffi` `start_receiver` gets `room_name: Option<String>` → `ReceiverOptions.room_name` → `create_room.room_name` (the server cleans it, max 32). The display app passes the setup name.
- **Where:** `crates/cc-ffi/src/api.rs`, bridge regeneration, `apps/flutter` receiver controller.
- **Acceptance:** the web presenter joined to a Flutter display shows "Meeting room Elbe" on the waiting page, in `what.connectedTo` and in `presenting.where`.

### B2. Same code after the presenter leaves (C8, N8)
- **What:** `cc-receiver` keeps its signalling connection and room after `PeerLeft` and after `remove_peer`: tear down the WebRTC session and the content task, create a fresh session for the next presenter, keep join mode, blocks and queue, keep emitting events; `cc-ffi` keeps the stream open. The display app stops starting a new room on `PeerLeft`.
- **Where:** `crates/cc-receiver/src/lib.rs`, `crates/cc-ffi/src/api.rs`, `apps/flutter/lib/receiver_controller.dart`.
- **Acceptance:** a presenter disconnects → display Idle with the **same** code, toast `toast.left`; a second presenter joins with that code; End still makes a new code.

### B3. Security code on both sides (C2, N2)
- **What:** compute the code in the core exactly as `apps/web/src/core/sas.ts` (first `a=fingerprint:sha-256` of local and remote SDP, lowercase, sorted, SHA-256 of `cometcast/connection-check/v1\n{x}\n{y}`, first 4 bytes big-endian mod 1 000 000, `ddd ddd`); report `ConnectionCode { sas }` once per connection on the receiver events and the presenter session events.
- **Where:** `crates/cc-transport` (or `cc-proto`), `cc-receiver`, `cc-sender`, `cc-ffi`.
- **Acceptance:** a Rust test with vectors produced by `sas.ts`; Flutter display + web presenter, and web display + Flutter presenter, show the same six digits (name-tag line 2 / `presenting.sas`).

### B4. Page message (C3, N3, decision 58)
- **What:** the receiver sends `{"kind":"page","page":n,"pages":m}` on the control channel once the PDF page count is known and after every page change; the presenter session validates it like `apps/web/src/core/control.ts` and reports `Page { page, pages }`.
- **Where:** `cc-receiver` content handling, `cc-sender/src/content.rs`, `cc-ffi`.
- **Acceptance:** web presenter on a Flutter display shows "Page 1 of 3" and a disabled Previous; Flutter presenter on the web display the same.

### B5. Receive progress (C4, N4)
- **What:** receiver event `ContentTransferProgress { received, total }`, at most one per whole percent and four per second, between `ContentTransferStarted` and the frame.
- **Where:** `cc-receiver`, `cc-ffi`.
- **Acceptance:** sending a 20 MB PDF shows the pill counting up from 0 % to 100 %, then the page.

### B6. Presenter session: join once, stop keeps the join, structured errors (C5, C9, N5, N9)
- **What:** native-apps.md 20.1: `start_presenter` joins on Connect; `present_screen` / `present_content` / `send_content_cmd` / `stop_presenting` inside the session; `stop_presenter` leaves. Events with `room_name`, `check`, `timeout_secs`, `ConnectionCode`, `Page`, `Progress`, `Presenting`, `PresentingStopped`, `Ended { code }`, `Error { code, message }` (codes in implementation-questions-flutter.md C9). On the receiver side a stopped presentation is `PresentationStopped` (display → Idle, no toast), not a leave.
- **Where:** `crates/cc-sender`, `crates/cc-ffi`, bridge regeneration; Flutter `presenter_root.dart`, `errors.dart` (map by `code`, delete the text parsing).
- **Acceptance:** in Approval on, the host approves once; the person shows a file, stops, shares the screen, stops, shows a link without a second request; the display goes Idle (same code, no toast) between them; Disconnect shows `toast.left`; every error page of spec 6 is reached from its code in a unit test.

### B7. Desktop capture through the system picker (D9, N26)
- **What:** native-apps.md 20.3: xdg-desktop-portal ScreenCast (Linux), `GraphicsCapturePicker` (Windows), `SCContentSharingPicker` (macOS 14+), opened directly by the Screen row (after the codec page when the encoder is missing); cancel = `capture_denied` (no page).
- **Where:** `crates/cc-capture`, `cc-ffi`, Flutter desktop runners.
- **Acceptance:** on each desktop the system picker offers screens and windows; picking a window shows only that window on the display; cancelling returns to "What to show" without an error.

### B8. Links, install referrer and relay confirmation (C6, N6)
- **What:** native-apps.md 20.4: App Links / Universal Links for the built-in relay's host (`/{CODE}`, optional `?server=`), the Android install referrer (`code`, `server`), the trust rule, and the relay confirmation page wired to an unknown server (Cancel → the server the app would use anyway; Use → remembered as confirmed).
- **Where:** `android/app/src/main/AndroidManifest.xml` (intent filter, `autoVerify`), `ios/Runner` (associated domains), the relay's `/.well-known/` files (deploy), Flutter `app_shell.dart` / `presenter_root.dart`.
- **Acceptance:** tapping `https://{relay}/K7QM2W` on a phone with the app opens the first-run page with the chip "K7Q M2W · taken from your link"; `…?server=cast.example.org` first shows the relay page with `bodyApp`; Cancel lands on the first-run page with the chip and the default server.

### B9. QR scanner (C7, N7)
- **What:** Android `GmsBarcodeScanning` (phones and tablets, no camera permission), iOS/iPadOS VisionKit `DataScannerViewController` with `NSCameraUsageDescription` = `ios.cameraUsage`; scan button above the code field; results as spec 5.3 (chip `app.codeScanned`, relay page for an unknown server, `app.scanWrong` otherwise); "Scan QR code again" on the error pages only where the scanner exists.
- **Where:** a small platform channel or plugin, `presenter_root.dart` (`scanQr`), iOS `Info.plist` (DE strings from the chosen German set).
- **Acceptance:** scanning a display's QR fills the chip and Connect joins; a foreign QR shows `app.scanWrong`; cancelling changes nothing; computers show no scan button.

### B10. UI wiring once B1–B6 land
- **What:** waiting page room label (spinner + name, 5.4); "What to show" header (green tick disc 40 + `what.connectedTo` / `what.connected`, 5.5); `presenting.where`; `presenting.sas` (small print, digits mono 700 `p-fg`); `presenting.pageOf` above the page row and the disabled ends (A10); display name-tag line 2 `present.sas` (13/400 `--mut`, digits mono 700 `--fg`, only with the controls); transfer pill percentage; the Connect → waiting → "What to show" order; toasts `toast.left` only on a leave.
- **Where:** `pages.dart`, `presenter_root.dart`, `screens.dart`, `display_root.dart`.
- **Acceptance:** the A8 shots "waiting with a room name", "What to show with its header", "presenting with where, sas and Page 1 of 3" and "presenting controls with the security-code line" match `/tmp/shots3/presenter-phone-light-waiting.png`, `-what.png`, `-presenting-pdf.png` and `/tmp/shots3/display-1280-dark-presenting-controls.png` (name tag) in layout, tokens and copy.

## Design status

With part A and part B done, the Flutter display (TV, tablet, desktop) and the Flutter presenter (phone, tablet, desktop) are **design-complete for this round** (decision N28). The only remaining design work is the nearby list of spec 14, which is the next round and fits into the first-run order of A9 without moving anything. No further design review is needed: the engineer checks each item against its acceptance line and the re-shot screenshots.
