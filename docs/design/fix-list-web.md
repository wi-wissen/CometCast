# Fix list: web app after design review

From the review of 2026-11-07 (spec [first-run-and-display.md](first-run-and-display.md), section 13; answers in
[implementation-questions.md](implementation-questions.md)). In order: do the top first. Screenshot names refer to the
review set (`/tmp/shots`, 1280 × 720 and phone, dark / light / OLED); `*` means every theme. Paths are under `apps/web/src`
unless they start with `crates/` or `docs/`.

Every item that adds or changes copy changes `locales/en.json`, `locales/de.json` (Sie) and `locales/de-du.json` (du) with
the values of spec 11.

## Must fix

1. **Presenting is quiet: hide the annotation toolbar at rest.**
   Where: `components/display/PresentingStage.vue`, `components/ReceiverToolbar.vue`, `core/controlsReveal.ts`, `styles/display.css`.
   Change: the toolbar uses the same reveal / 5 s auto-hide / 400 ms fade as name tag and End; pointer strokes count as input. Restyle per spec 3.5 (top 12 centred, panel fill 92 %, 2 px `--line`, icons `--fg`, selected tool 2 px `--acc`; ink swatches unchanged). Remove grip/move and collapse. While the toolbar is visible, the toast sits 8 below it.
   Check: `display-*-presenting-rest` shows only the picture; `display-*-presenting-controls` shows toolbar, name tag and End in theme colours (light toolbar is light); the toast never overlaps the toolbar; `e2e:m2` updated and passing.

2. **Remove close and page buttons from the display; theme what stays.**
   Where: `components/display/PresentingStage.vue`, `components/DocumentView.vue`, `components/ContentOverlay.vue`, `styles/display.css`.
   Change: remove "Close image", "Close document" and the PDF page buttons. Frozen badge top-left in name-tag style; black-screen hint 15 `--mut`. Content clears when the presenter stops or disconnects.
   Check: nothing but the picture at rest (badge only while frozen); turning PDF pages works from the presenter page.

3. **Link card as a display dialog.**
   Where: `components/ContentOverlay.vue` (link card, media player), `styles/display.css`, locales (`what.linkSub`).
   Change: 600 card on the 55 % scrim like the join request; title `receive.link.title`, address 18 mono `--mut` wrapping anywhere, hint; Open (primary, initial focus) and Dismiss (secondary); 1 s input guard; Back/Escape = Dismiss; focus row [Open, Dismiss]. `what.linkSub` = "The screen asks before it opens the page".
   Check: sending a link shows the themed card in dark/light/OLED; a key press within 1 s does nothing; the page opens only after Open.

4. **Room name through the protocol (protocol requirement).**
   Where: `crates/cc-proto`, `crates/cc-server`, `docs/protocol.md`, `core/signaling.ts`, `core/receiverSession.ts`, `core/senderSession.ts`, `views/PresenterView.vue`, `components/presenter/WaitingPage.vue`, `WhatPage.vue`, `PresentingPage.vue`.
   Change: optional `room_name` on `create_room` (server cleans like `name`, cuts at 32, drops empty) and on `join_pending` / `joined`. The display sends its room name. Presenter pages use it; without it: waiting row = spinner only, `what.connected` ("Connected"), no `presenting.where` line. Never show the code in its place.
   Check: `presenter-*-waiting` shows "Meeting room Elbe" instead of "44S PER"; `presenter-*-what` says Connected to “Meeting room Elbe”; with an old server the pages show no room and no code; server test for cleaning/cutting.

5. **Device type out of the strings.**
   Where: locales (`request.subtitle`, `present.byline`), `components/display/RequestDialog.vue`, the name tag in `components/display/PresentingStage.vue`.
   Change: new values without `{device}`; remove the runtime cutting of " · ".
   Check: unit test renders "wants to connect" and "“Lena” is showing" from the locale values alone, in EN, DE Sie and DE du.

6. **Code field: no silent dropping, one format.**
   Where: `components/presenter/StartPage.vue`, `core/roomCode.ts`, locales (`error.code.why`).
   Change: the field shows every typed character in uppercase (max 9); on submit remove spaces and dashes; not 6 characters from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` → `error.code` page without a server call. New `error.code.why`.
   Check: typing "k7q-m2w" joins; "K7O M2W" shows the error page (the O stays visible in the field before submit); `presenter-phone-dark-error-code` shows the new text.

7. **Security code: renamed, smaller, in fewer places.**
   Where: `components/SasLine.vue`, `components/presenter/WhatPage.vue`, `PresentingPage.vue`, name tag in `components/display/PresentingStage.vue`, `styles/display.css`, locales (`present.sas`, `presenting.sas`; remove `sas.label` / `sas.explain` use).
   Change: display name tag line 2 = "Security code 171 852", 13/400 `--mut`, digits mono 700 `--fg`. Presenter: only on the Presenting page, one line of small print `presenting.sas`; not on "What to show".
   Check: `display-*-presenting-controls` name tag reads "Security code …" at the smaller size; `presenter-*-what` has no code line; `presenter-*-presenting-*` has one short line.

8. **App offers only when they exist.**
   Where: `components/presenter/StartPage.vue`, `components/display/IdleScreen.vue`, `config/stores.ts`, locales (`idle.appLineWeb`).
   Change: hide each platform's app button, its small print, the "or" divider and `start.downloadStarted` while its URL is empty; idle footer shows `idle.appLineWeb` ("No download needed.") until a store link exists. `start.downloadStarted`, when shown, is small print `p-fg2` (no green box).
   Check: with empty constants `presenter-*-start-*` ends after the detected line (laptop card: browser button, small print, detected line, beDisplay); `display-*-idle` footer says only "No download needed."; `presenter-1280-light-start-app-clicked` no longer exists as a state.

9. **Relay confirmation dialog themed.**
   Where: `components/RelayConfirm.vue`, `styles/display.css`, `styles/presenter.css`.
   Change: on `/display` the dialog card of spec 4.4 (Cancel secondary with initial focus, Use this relay primary; focus row [Cancel, Use this relay]); on presenter pages a presenter card (heading 24/700, body 15, risk text as small print, buttons as error pages). Copy unchanged.
   Check: open `/display?server=…` and `/?server=…` in dark/light/OLED; tokens only, no old colours; Enter on load cancels.

10. **New presenter error pages and copy.**
    Where: `core/presenterErrors.ts`, `components/presenter/ErrorPage.vue`, locales.
    Change: `error.occupied` (`room_full`), `error.busy` (`rate_limited`, `banned`, `server_full`), `error.outdated` (`proto_mismatch`, `bad_hello`, `bad_message`; button `error.action.reload` reloads); all with the Why / What now card. `error.timeout.why` = "Nobody answered your request in time."
    Check: unit test maps each code to its page; a second presenter joining a busy room sees "Someone else is connected".

11. **Laptop presenter pages: action row follows the content.**
    Where: `styles/presenter.css` (≥ 720 px), all presenter pages.
    Change: actions 24 px below the content, content-width buttons (min 160, padding 0 24), primary first, left-aligned, in a row; phone unchanged (pinned to the bottom). Keep 76 px bottom padding while Chrome's sharing bar is shown.
    Check: `presenter-1280-*-error-*`, `-what`, `-file`, `-link`, `-bye`, `-presenting-*`: no button at the window bottom far from its content; phone shots unchanged.

12. **File page in the browser.**
    Where: `components/presenter/FilePage.vue`, locales (`file.bodyWeb`, `file.pick`).
    Change: heading `what.file`, body `file.bodyWeb`, **primary** button `file.pick` ("Choose file"); progress, error, Back as built.
    Check: `presenter-*-file` shows the new heading, body and a blue primary button.

13. **Presenter PDF controls.**
    Where: `components/presenter/PresentingPage.vue`, locales (`presenting.prevPage`, `presenting.nextPage`, `presenting.pageOf`).
    Change: Previous / Next in one row (phone: half width each, min 48), "Page n of m" above when known, then Stop.
    Check: phone screenshot of a shown PDF: two buttons side by side above Stop.

14. **Disabled button readable.**
    Where: `styles/presenter.css` (`--p-disabled`, `--p-ondisabled`).
    Change: light `#DDE1E8` / text `#4A5160`; dark `#2A2F38` / text `#9EA6B3`. Also set the dark disc tints as hex (`#342A2C`, `#21312F`, `#332F26`).
    Check: `presenter-1280-light-error-denied`: "Ask again in 16 s" ≥ 4.5:1.

15. **Offline and connecting states on the display.**
    Where: `components/display/IdleScreen.vue`, `components/display/QrCode.vue`, `components/display/DisplayRunning.vue`, locales (`idle.offline.bodyBusy`, `idle.offline.bodyOutdated`, `toast.openDisabled`).
    Change: QR box empty (no modules) at 25 % while offline or connecting, no caption; offline box only after 3 s; body variant by cause (spec 4.2); connecting: code line empty with its height kept, after 8 s offline; toast `toast.openDisabled` once when the server refuses Open.
    Check: `display-*-idle-offline` shows an empty QR box; stopping and restarting the server never flashes the offline box for drops under 3 s.

16. **A request closes Settings.**
    Where: `components/display/DisplayRunning.vue`, `SettingsDialog.vue`, locales (remove `toast.lockedRejected`).
    Change: on a new request close Settings, focus Yes; after the last request focus the Settings button.
    Check: e2e: open Settings, send a request → only the request dialog is visible; answer it → focus on Settings.

17. **OLED black corrections.**
    Where: `styles/display.css` (`.cd[data-theme='oled'] .cd-mt.on`, `.cd-chk`).
    Change: no tinted fill for selected tiles in OLED; check-number box gets 1 px `--line` border in OLED.
    Check: `display-1280-oled-settings` selected tiles are black with amber border; `display-1280-oled-request*` shows the check box outline.

18. **"No and block" not stretched.**
    Where: `styles/display.css` (`.cd-acts`, `.cd-ghost`).
    Change: height 34, `align-self: center`.
    Check: `display-*-request*`: the ghost button is visibly lower than Yes/No and centred against them.

19. **Wizard: room name field and web address source.**
    Where: `components/display/SetupWizard.vue`, `core/displaySetup.ts`, locales (`setup.address.sourcePage`, `sourceTyped`, `fieldLabel`).
    Change: custom name field starts empty; typing deselects chips, choosing a chip clears the field. Source line per spec 4.1 (default / page / typed); field label "Server address".
    Check: `display-*-setup-1-name` shows the chip selected and an empty field; a typed address shows "Entered by hand."

20. **`start.beDisplay` link.**
    Where: `components/presenter/StartPage.vue`.
    Change: `{dom}/display` is a link (`p-primary`, underlined) to `/display` in the same tab; the rest of the sentence stays text.
    Check: `presenter-1280-*-start-*`: the address is a visible link; keyboard focus shows the outline.

21. **Focus ring rule on the display.**
    Where: `components/display/*`, `styles/display.css`.
    Change: keep the ring on load; the first mouse or touch input hides it, the next key press shows it again.
    Check: `display-*-setup-0-welcome` shows the ring; after a mouse move and click no ring; after an arrow key the ring is back.

22. **Static hosting note.**
    Where: `docs/website.md` (or wherever the static deployment is described).
    Change: say that `/display`, `/{CODE}` and `/app` must serve `index.html` (GitHub Pages: `404.html` copy).
    Check: the doc names the three paths.

## Accepted as built (no work)

Name tag second line (as changed in 7), fonts as bundled subsets, presenter pages without device frames, QR version,
portrait display letterboxed, detected line as plain text, offline box naming the relay host (`{dom}`), dark icon discs as
15 % tints, one secondary button instead of a thumbnail grid (as changed in 12), screen-reader status line, no language switch.
