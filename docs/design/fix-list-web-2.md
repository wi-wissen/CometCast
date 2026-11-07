# Fix list 2: web app after the second design review

From the review of 2026-11-07 of main 4a13254 (screenshots `/tmp/shots2`, 1280 × 720 and phone 390 × 844; display dark / light / OLED, presenter light / dark). Decisions: spec [first-run-and-display.md](first-run-and-display.md) section 13, items 30–53; answers to D1 and E1–E14 in [implementation-questions.md](implementation-questions.md). The first fix list ([fix-list-web.md](fix-list-web.md)) is done; its 22 items were checked against the screenshots and pass, except where an item below says otherwise.

In order: do the top first. `*` in a screenshot name means every theme. Paths are under `apps/web/src` unless they start with `crates/`, `docs/` or `tools/`. Every item that adds or changes copy changes `locales/en.json`, `locales/de.json` (Sie) and `locales/de-du.json` (du) with the values of spec 11. Every check that names a screenshot means: retake it with `e2e:shots` and look at it.

## Must fix

1. **Relay confirmation on the display: on the canvas, alone.**
   Where: `components/RelayConfirm.vue`, `components/display/DisplayRunning.vue` (or wherever `/display` decides what to draw first), `styles/display.css`.
   Found: the card is laid out in CSS px, not on the 960 × 540 canvas: at 1280 it is 600 CSS px wide with ~12 px text (spec: 600 **ref** px, body 16 ref); on a phone it fills the width and "Use this relay" wraps onto three lines. It also sits over the idle screen, which already shows the unconfirmed relay host as `{dom}` ("crafte…") and an empty QR box.
   Change: spec 7.2. Draw only `--bg`, the logo (46, idle top-left position) and the 600 ref dialog card on the scaled canvas; body and risk 16 `--mut`; host 18 mono `--fg` on its own line; [Cancel (secondary, initial focus), Use this relay (primary)], 50 high, left-aligned. Back/Escape = Cancel. Idle or setup appears only after the decision.
   Check: `display-1280-*-relay-confirm` and `display-phone-*-relay-confirm` look like the join request card at the same scale, no idle content behind, no wrapped button label; Enter on load cancels.

2. **Relay confirmation on presenter pages: its own page.**
   Where: `components/RelayConfirm.vue`, `views/PresenterView.vue`, `styles/presenter.css`.
   Found: a floating card over the start page; behind it the `start.beDisplay` link already names `crafted.example.org/display`, and the code taken from the link is shown unformatted in the field ("7UZFHX") instead of the chip.
   Change: spec 7.2: a page in the error-page layout (icon disc `info` in `p-warn`, heading `relay.confirm.title`, body 15 `p-fg2`, host 17 mono 700 `p-fg`, risk as small print). Actions [Cancel secondary, Use this relay primary]: phone stacked at the bottom, Cancel on top; laptop in a row 24 below the content, Cancel left. Initial focus Cancel. After the decision show the start page; a code from the link appears as the code chip (`XXX XXX`, `start.fromQr`).
   Check: `presenter-*-relay-confirm` show a page, not a card over the start page; the host appears only on this page; after Cancel the start page shows the chip "7UZ FHX".

3. **Complete the "du" set.**
   Where: `locales/de-du.json`, `tools/check-i18n.mjs`.
   Found: 42 keys whose Sie value addresses the reader are missing in `de-du.json`, so the du build shows Sie text (relay confirmation, link card hint, media lines, the `errors.*` texts of the `error.other` page, status lines).
   Change: add the values of spec 11.8 (and the new keys of items 6, 7, 10). Add the check rule of 11.8 (Sie/Ihr…/Ihnen in `de` → key must exist in `de-du`; exception `receive.media.hint`).
   Check: `node tools/check-i18n.mjs` passes and fails when one of those keys is removed from `de-du.json`.

4. **QR box: visible edge in Light, real empty state everywhere.**
   Where: `components/display/QrCode.vue`, `components/display/IdleScreen.vue`, `styles/display.css`.
   Found: in Light the empty box (connecting, offline) is white at 25 % on white and invisible, and the QR box has no edge; in Dark and OLED the empty box is a light-grey slab (in OLED a non-black surface).
   Change: spec 3.2. Light: 1 px `--line` border on the QR box always. Empty box: `--panel2` fill, 1 px `--line` border, full opacity, no modules, no caption (OLED: `#000000` + border). Locked keeps the 25 % QR.
   Check: `display-*-idle-connecting`, `display-*-idle-offline`: a quiet box in theme colours, visible in Light; `display-1280-light-idle` QR has a thin grey edge; OLED shows no grey fill.

5. **Phone: PDF page buttons directly above Stop.**
   Where: `components/presenter/PresentingPage.vue`, `styles/presenter.css`.
   Found: on the phone the Previous/Next row sits under the text at the top, Stop is pinned at the bottom (`presenter-phone-*-presenting-pdf`).
   Change: spec 5.6: on the phone the row is part of the bottom action area, 8 above Stop ("Page n of m" above the row when known). Laptop unchanged.
   Check: `presenter-phone-*-presenting-pdf`: the two half-width buttons sit right above "Stop showing" at the bottom.

6. **No app, no promise.**
   Where: `components/presenter/ErrorPage.vue`, `components/presenter/WhatPage.vue`, `config/stores.ts` (`appUrlFor`), locales (`error.noScreen.nowNoApp`, `what.screenPhoneBrowserNoApp`).
   Found: `presenter-phone-*-error-noscreen` says "Or get the app, then the screen works too." but has no app button; the Screen row says "On a phone only with the app" although there is no app to get.
   Change: when `appUrlFor` gives nothing, use `error.noScreen.nowNoApp` and `what.screenPhoneBrowserNoApp` (spec 11.5, 11.6).
   Check: with empty store constants both screenshots mention no app; with a filled Google Play constant they show the old texts and the button.

7. **New error page `error.other`; `capture_denied` has no page.**
   Where: `core/presenterErrors.ts` (`generic` → `other`), `components/presenter/ErrorPage.vue`, `views/PresenterView.vue`, locales (`error.other.title`, `error.other.now`).
   Change: spec 6: icon (i) grey, heading `error.other.title`, card Why = `errors.<code>`, What now = `error.other.now`, button Try again; for `forbidden`, `altcha_*`, `wrong_role`, `not_in_room`, `unknown` and any unlisted code. `capture_denied` returns to "What to show" without a page.
   Check: unit test maps each listed code to `other` and `capture_denied` to no page; e2e: cancel the share dialog → "What to show" again.

8. **Reconnect schedule of the display.**
   Where: `components/display/DisplayRunning.vue` (retry timers), `core/receiverSession.ts` if the timers live there.
   Change: spec 4.2 / decision 13.30: retries at 1 s, 2 s, 3 s, then every 4 s; the offline box appears when the attempt at 3 s fails (remove the separate 3 s timer). Busy refusals and `forbidden`: box at once with `bodyBusy`, retry after 30 s.
   Check: unit test with fake timers: server back at 2.5 s → no box ever; server back at 3.5 s → box from 3 s until the attempt at 7 s; e2e: stop the server for 2 s → no box in a screenshot series taken every 250 ms.

9. **Busy stays busy (E14) and the STUN/TURN refusal (D1).**
   Where: `core/presenterErrors.ts`, `core/senderSession.ts`, `core/receiverSession.ts`, `core/altchaClient.ts` (`IceFetchState`), `views/PresenterView.vue`, `components/display/DisplayRunning.vue`, locales (`error.displayOff.whyRelay`); the Rust `SenderCore` / `ReceiverCore` expose the same state for the native apps.
   Change: (a) for 60 s after `rate_limited` / `banned` / `server_full`, `connection_lost` maps to `error.busy` (presenter) and to `bodyBusy` with the 30 s retry (display). (b) Sessions keep the last `IceFetchState`; on `peer_failed`: `rate_limited` / `refused` → `error.busy`; `http_error` / `bad_answer` / `unreachable` → `error.displayOff` with why `error.displayOff.whyRelay`; `ok` / `none` → `error.displayOff` as now. Nothing is shown before the failure.
   Check: unit tests for both mappings; the screenshot run captures `presenter-*-error-busy` for all four variants by triggering a ban and then reconnecting (the second attempt must show busy, not "No internet").

10. **Content states while presenting.**
    Where: `components/display/PresentingStage.vue`, `components/DocumentView.vue`, `components/ContentOverlay.vue`, `core/toolbarState.ts`, `styles/display.css`.
    Found (E13): PDF pages on grey (`bg-neutral-700`); progress and content-error pills black/red; the PDF page error box red; black screen stays on into the next presentation.
    Change: spec 4.5 and 3.5: PDF on `#000000`; transfer progress pill (name-tag look, bottom 12 centred, 15/700 `--fg`, `receive.transfer`); content errors as toasts; page error text in a centred name-tag-look box; black screen, frozen picture and strokes are reset when the presentation ends (stop, disconnect, End).
    Check: new screenshots `display-*-presenting-pdf`, `display-*-transfer`, `display-*-pdf-page-error` in all three themes show only theme tokens; e2e: black screen on → presenter stops → presenter shares again → picture visible.

11. **Wizard: empty field brings back the chip; text fields get the focus ring.**
    Where: `components/display/SetupWizard.vue`, `core/displaySetup.ts`, `styles/display.css`.
    Change: spec 4.1: when the custom name field is emptied (or only spaces), the chip selected before typing is selected again. Text fields (room name, server address) show the display focus ring (outline 4 `--focus`, offset 3, glow) without scale; the thin bluish focus border goes. Up/Down leave the field. After Back closes the address field, focus goes to "Other address". An invalid address line is 16 `--fg` with the `info` icon in `--acc`.
    Check: e2e: type "X", delete it → "Meeting room Elbe" chip selected again; `display-*-setup-3-address-field` (taken after a key press) shows the amber ring around the field.

12. **Everything on the canvas (portrait and odd windows).**
    Where: `components/display/ToastLayer.vue`, `components/display/PresentingStage.vue`, `components/ReceiverToolbar.vue`, `styles/display.css`.
    Found: on the phone viewport the toolbar, toast, name tag and End sit at the viewport edges while being scaled to the canvas (`display-phone-*-presenting-controls`, toast in `display-phone-*-request`).
    Change: spec 3.5: toasts, dialogs, toolbar, name tag, End, frozen badge and pills are positioned on the centred 16:9 canvas; only the picture and its black background use the full viewport.
    Check: `display-phone-*-presenting-controls` shows the controls at the corners of the centred 16:9 area; 1280 shots unchanged.

13. **Link card: media lines.**
    Where: `components/ContentOverlay.vue`, `components/MediaPlayer.vue`, `styles/display.css`.
    Change: spec 4.5: warning and error lines 16/700 `--fg` with the `info` icon 20 in `--acc` in front; player header `receive.media.from` left, Stop right; media on `#000000`; while playing the focus row is [Stop]. The link address is `--mut` (check the colour: in the screenshots it reads close to `--fg`).
    Check: new screenshots `display-*-linkcard-media` (card with Play) and `display-*-linkcard-media-local` (local-address warning) in all themes.

## Should fix

14. **PDF page state from the display (protocol requirement, E2).**
    Where: the content control channel (`docs/protocol.md` content section, `core/receiverSession.ts`, `core/senderSession.ts`, `components/presenter/PresentingPage.vue`), the Rust and Swift cores when they implement the channel.
    Change: the display sends `page` and `pages` whenever they change; the presenter shows `presenting.pageOf` and disables Previous on page 1 and Next on the last page (`p-disabled`).
    Check: unit test of the message; `presenter-*-presenting-pdf` shows "Page 1 of n" and a disabled Previous.

15. **Laptop presenter pages: room above the content.**
    Where: `styles/presenter.css` (≥ 720 px).
    Change: top padding `max(40px, 10vh)`, bottom 40 (spec 3.5).
    Check: `presenter-1280-*` (start, what, waiting, file, link, error, bye): the heading starts about 72 px from the top, not 20.

16. **Wizard card does not jump.**
    Where: `components/display/SetupWizard.vue`, `styles/display.css`.
    Change: steps 1–4: card top fixed at 64 ref; the welcome card stays centred (spec 4.1).
    Check: `display-*-setup-1` to `-4`: logo and step dots at the same position in every step.

## Screenshot set (needed for the next review)

17. **Complete the capture set; capture focus with the keyboard.**
    Where: the `e2e:shots` script.
    Change: take every display screenshot after one arrow key press (so the focus ring shows where a control has focus; most of the current set was taken after a click, which hides it by design). Add the missing states: `presenter-1280-dark-error-busy`, `presenter-phone-*-error-busy` (item 9), `presenter-*-error-timeout`, `presenter-*-error-expired`, `presenter-*-error-ended`, `presenter-*-error-noInternet`, `presenter-*-error-displayOff` (both "why" variants), `presenter-*-error-locked` while waiting, `presenter-*-error-other`, `display-*-idle-offline-busy`, `display-*-idle-offline-outdated`, `display-*-frozen`, `display-*-black`, `display-*-toast-with-toolbar`, `display-*-setup-3-address-error`, and the new shots of items 10 and 13.
    Check: the set exists in all listed themes; the review can open each name.
