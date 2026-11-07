# Fix list 2: Flutter app (display and presenter)

From the designer's review of main f250cc2 (first Flutter fix round) and its 362 screenshots in `/tmp/shots-flutter2`, compared with the final web screenshots in `/tmp/shots4`. Decisions: spec section 13, "Decisions after the second Flutter review" (N29–N47); answers: [implementation-questions-flutter.md](implementation-questions-flutter.md) section F and G; iOS hand-over: [native-apps.md](native-apps.md) 20.5.

**Already right, no work:** idle, connecting, locked (with toast), dimmed (now visibly dimmed), offline box (dashes on all sides in all three appearances, content height; N29), Settings, request (single and queue, OLED check border), link card with and without a browser (`hintApp` / `hintTv`, address `--fg`), presenting controls with the security-code line, page-error box, toasts and their soft shadows, wizard steps 0–4 (card top 40, step 2 selected look and equal heights), DE du layouts of setup 0–4 (every text fits, no clipping), TV legibility (all sizes are reference × 2 at 1080p), OLED black everywhere (black surfaces, `#2C313C` borders, no tints), focus rings on every first frame except A11, the whole presenter flow on phone and laptop layouts (first-run with brand row, placeholder, scan button and `app.scanWrong`, waiting with room name, "What to show" with header and `what.screenTablet`, presenting with `presenting.where`, `presenting.sas`, page line and disabled Previous, every error page, relay page with `bodyApp`, bye without body), the presenter codec page in its four states (apart from A10), the End icon, letterboxing on 16:10 and the laptop column (N34–N38).

Each item: **What**, **Where**, **Acceptance** (screenshot names in `/tmp/shots-flutter2`, re-shot with the harness of A12). Do part A top to bottom; part B is platform work and can run in parallel.

## Part A: Flutter UI

### A1. Display codec dialog: draw its states
- **What:** `display-desktop-{dark,light,oled}-codec-downloading` and `-failed` are byte-identical to `-missing`, and `-unsupported` shows the missing content too (Download, Not now, explain, source, admin note). The dialog shows one state only.
- **Where:** `lib/codec_notice.dart` (display dialog), `lib/src/display/display_root.dart` (`_CodecLayer`).
- **Acceptance:** downloading: line `codec.downloading` + `codec.progress` (16/700 `--fg`), 6 ref `--acc` bar, row [Not now] with the ring; failed: `info` 20 `--acc` + `codec.failed` (16/700 `--fg`, no red), admin note, [Try again (ring), Not now]; unsupported: label + `codec.unsupported` only, [Close (ring)]. Four different shots per appearance.

### A2. Display codec dialog: size and scrim rules
- **What (N30, N31):** 780 wide (today 600; about 377 ref high); explain 16/1.4, source 15/1.3, admin note 14/1.4; top edge never above 66 ref; failure texts without technical reason (N46). Its appearance does not reveal the name tag and End; toasts stay above the scrim (as built).
- **Where:** as A1.
- **Acceptance:** at 1920 × 1080 EN: missing ≈ 626 px high, explain 3 lines, source 1, admin 2; card top ≥ 132 px in all four states in EN and DE; a shot taken more than 5 s after the last input shows no name tag or End under the scrim; the "connected" toast is fully visible and does not touch the card.

### A3. `codec.stageHint` after Not now
- **What (N31):** after Not now (also while a download continues), the black stage shows `codec.stageHint` centred, 15/400 `--onblack`, no box, at rest; it goes when the picture appears, the presenter stops or leaves, or End. New key in the three ARB files (spec 11.9).
- **Where:** `display_root.dart`, ARB files.
- **Acceptance:** new shot `display-desktop-{dark,light,oled}-codec-later`: the line on black, same look as the web `display-1280-*-black` hint; DE "Noch kein Bild: Dieser Bildschirm braucht den Video-Codec."

### A4. Transfer pill width and position
- **What:** the pill spans the whole canvas width and sits about 4 ref from the bottom (`display-*-presenting-pill`). Spec 3.5: centred, content width, **max 340**, bottom 12, one line, name shortened in the middle, percentage never cut.
- **Where:** `lib/src/display/screens.dart` (transfer pill).
- **Acceptance:** `display-tv-dark-presenting-pill` ≈ `/tmp/shots4/display-1280-dark-transfer.png`: "Receiving slides.pdf… 42 %" in a pill of its text width, centred, bottom edge 24 px above the 1080 edge; `-transfer-long` equivalent stays ≤ 680 px wide at 1080p.

### A5. Wizard step 3: button row and field error
- **What:** on step 3 (address, field open, field error) [Back, Other address, Looks right] are left-aligned; every other step and the web right-align the row. `setup3-field-error` is identical to `setup3-field` (no value, no error line).
- **Where:** `lib/src/display/setup_wizard.dart`.
- **Acceptance:** `display-*-setup3-address`, `-setup3-field`, `-setup3-field-error`, `display-*-de-setup3-address`: row right-aligned to the card's content edge as `/tmp/shots4/display-1280-dark-setup-3-address.png`; the error shot shows a typed value and the `info` + error line under the field as `/tmp/shots4/display-1280-dark-setup-3-address-error.png`.

### A6. First-run links (Windows)
- **What (N37):** `app.beDisplay` and `app.miracast` are about 54 px apart with about 30 px above. Each link: tap area 44 high, text centred in it, areas touching; laptop: left-aligned, first area 12 below Connect; phone: centred, first area 8 below Connect.
- **Where:** `lib/src/presenter/pages.dart` (`FirstRunPage`), `lib/src/app_shell.dart`.
- **Acceptance:** `presenter-tablet-*-firstrun-windows`: link baselines 44 apart, first link text 12 + 11 px below Connect's bottom; a phone-width Windows window shows both centred under Connect.

### A7. Miracast page layout
- **What (N39):** phone: the last note runs into Start (`presenter-phone-*-miracast`); laptop: Start receiving and Back are stacked (`presenter-tablet-*-miracast`).
- **Where:** `lib/miracast_page.dart`.
- **Acceptance:** phone: content scrolls, [Start receiving, Back] pinned in the bottom action area, ≥ 24 free between the last note and Start when scrolled to the end; laptop: [Start receiving, Back] in one row, 24 below the last note, content width (min 160).

### A8. Miracast consent page: PIN line
- **What (N39):** the whole sentence "PIN: 4711. Enter it on the sending device if it asks for it." is set in mono 700.
- **Where:** `lib/miracast_page.dart` (consent).
- **Acceptance:** `presenter-*-miracast-consent`: the sentence in body text (15 `p-fg2`), only `4711` in mono 700 `p-fg`; [Decline, Allow] unchanged, initial focus Decline.

### A9. In-house box line from the app
- **What (N40):** `idle.altAddress` is passed only by the screenshot. Pass the IPv4 of the box announcement when the server's source is `sourceBox` and `{dom}` is a host name; the QR uses the IP.
- **Where:** `lib/src/display/display_root.dart`, the setup source mapping, `cc-discovery` result handling.
- **Acceptance:** a test with a box announcement (`cometcast.box`, 192.168.1.20) shows "or 192.168.1.20" under the address and a QR that decodes to `https://192.168.1.20/{CODE}`; `display-*-idle-box` is taken through this path.

### A10. No technical text on presenter pages
- **What (N46, N33):** the codec page's failed line reads "… could not be downloaded: network error"; the unsupported page's "why" ends in ": no library". Show the key text only; the reason goes to the log. The unsupported page and `capture_unavailable` get `common.back` (to "What to show", still connected) instead of Try again; `capture_unavailable` uses the new `errors.capture_unavailable` (spec 11.9).
- **Where:** `lib/src/presenter/pages.dart`, `presenter_root.dart`, `errors.dart`, ARB files.
- **Acceptance:** `codec.failed` and `codec.unsupported` lose `{reason}` in all three ARB files (new values in spec 11.9); `presenter-*-codec-failed` reads "The video codec could not be downloaded."; `-codec-unsupported` shows the new `codec.unsupported` text and the button Back; the display dialog (A1) uses the same texts; new shot `presenter-*-error-capture-unavailable`.

### A11. One ring after the room name is emptied
- **What (N24):** `display-*-setup1-name-empty` shows no ring at all (the web shows it on the emptied field).
- **Where:** `setup_wizard.dart` (step 1 field focus after clearing).
- **Acceptance:** the ring (no scale) on the Custom name field, as `/tmp/shots4/display-1280-dark-setup-1-name-emptied.png`.

### A12. Screenshot harness
- **What:** (1) `display-*-idle-offline-busy` and `-offline-outdated` show the connecting state, not the box with `bodyBusy` / `bodyOutdatedApp`; (2) `display-{tv,tablet}-dark-de-idle` is a copy of `-de-setup4-done`, and `-de-settings` / `-de-request` show the idle screen without a dialog; (3) the codec shots of A1; (4) `presenter-*-presenting-screen` has no room name and security code (add them like the PDF shot); (5) new shots: `-codec-later` (A3), DE codec dialog missing state, `presenter-*-error-capture-unavailable` (A10).
- **Where:** `test/screenshots_test.dart`, `test/shot.dart`.
- **Acceptance:** no two shots of different names are byte-identical except where the state is truly the same; each named state is visible in its shot.

## Part B: platform and core

### B1. iOS screen sharing hand-over (N32)
- **What:** native-apps.md 20.5 steps 1–6 (token from the display over the control channel, `config.json`, `handover` join field relayed in `join_request`, admitted without dialog or toasts; back to "What to show" after the broadcast). Until then the interim fixes of step 7: the extension sends the name; the app shows the waiting page with the extension's check number and countdown; after the broadcast the app joins again with the same code and lands on "What to show".
- **Where:** `cc-ffi` / `cc-sender` (presenter session), `cc-receiver` (token store, admission), `cc-server` (relay the optional field), `crates/cc-ios`, `apps/flutter/ios/BroadcastExtension`, `lib/core/ios_broadcast.dart`; the web receiver for the same messages (a web change).
- **Acceptance:** on an iPhone against a native display in Approval on: Connect → approve once → Screen → broadcast starts with no second request on the display and no "left"/"connected" toasts → Stop → "What to show" still connected → File works without a new request. Against an old server: the waiting page with the extension's check number, then Presenting.

### B2. System screen picker (still open, fix-list-flutter.md B7, N26, N33)
- Unchanged; release needs it on Linux, Windows and macOS. Until then the X11 path and A10's `capture_unavailable` page.
