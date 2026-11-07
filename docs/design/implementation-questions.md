# Implementation questions for the designer

Written while building the web app (`apps/web`) from [first-run-and-display.md](first-run-and-display.md) and the approved
mockup. The implementer made no design decisions. Each item says what was found, the options that were seen, and what the
code does **meanwhile** (the smallest neutral behaviour; no styling or wording was invented). A designer answers; the
code is then changed in one place each time.

**Status (2026-11-07): every question is answered** (lines "Answered:"). The decisions are also in the spec, section 13; the resulting work is in [fix-list-web.md](fix-list-web.md).

## A. The spec and the protocol disagree

### A1. Join code format
- **Found:** the spec shows 8 characters in two groups of 4 (`K7QM 2W4P`) on the default server and 6 digits (`482 913`) on
  an own server. The real protocol (`crates/cc-proto`, `docs/protocol.md`) has one format: **6 characters** from
  `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (no 0, O, 1, I). It also contains `L`, which the spec says to read as `1`.
- **Options:** (a) change the protocol to the spec's formats; (b) change the spec to the 6-character code (3 + 3); (c) keep both
  (display shows what the server gives).
- **Meanwhile:** the code is shown as given, in two halves (`K7Q M2W`); the field placeholder is `XXX XXX`; typing
  keeps only characters of the alphabet (an `O` or `I` is dropped, as before; `L` stays an `L`, not a `1`); the error page
  "This code belongs to another address" (`error.otherServer`) cannot occur and is never shown. The idle-screen code
  line, the QR URL and the check are written so that only `formatCode` (`core/codeFormat.ts`) changes with the format.
- **Answered:** option (b). The 6-character code of the protocol is the only format on every server, shown `XXX XXX` (keep `formatCode`). Do not drop characters while typing; on submit remove spaces/dashes, and anything that is not 6 characters from the alphabet goes to `error.code` without asking the server. New `error.code.why`: "Usually a typo. A code has 6 characters and never contains O, 0, I or 1." Placeholder `XXX XXX`, max 9 characters. `error.otherServer` stays unused. No protocol change. Spec 2, 4.2, 5.2, 11.6, decision 13.1.

### A2. The room name does not reach the presenter
- **Found:** the protocol carries no room name to a presenter, but the spec uses it in `start.titleRoom`, the waiting page
  label, `what.connectedTo` and `presenting.where`. The QR URL is fixed by the spec to `https://{dom}/{CODE}` (no name).
- **Options:** (a) add the room name to the protocol (`joined` / `join_pending`) and the server; (b) put it into the QR URL;
  (c) drop it from those pages.
- **Meanwhile:** the pages say `“K7Q M2W”` (the code) where the room name would be; `start.titleGeneric` is always used
  (`start.titleRoom` is unused).
- **Answered:** option (a), minimal. **Protocol requirement:** `create_room` gets optional `room_name` (string, cleaned like `name`, cut at 32 characters, dropped when empty); `join_pending` and `joined` carry it as optional `room_name`. Additive; clients must work without it. Not in the QR URL. Presenter: waiting label, `what.connectedTo`, `presenting.where` use it; without it show no room at all (new `what.connected` "Connected"; omit the `presenting.where` line; spinner only on the waiting page), never the code. `start.titleRoom` is not used. Spec 5.4–5.6, decision 13.2.

### A3. The device type does not reach the display
- **Found:** `request.subtitle` and `present.byline` have `{device}` (Laptop, Android phone ...); `join_room` and
  `join_request` carry only a name.
- **Options:** add a device field to the protocol; or drop the device from the texts.
- **Meanwhile:** the strings are filled with an empty `{device}` and the separator is cut: "wants to connect",
  "“Lena” is showing".
- **Answered:** drop the device from the texts; no protocol change. `request.subtitle` = "wants to connect" / "möchte sich verbinden", `present.byline` = "{name} is showing" / "{name} zeigt" (change the locale values; remove the runtime cutting of " · "). Decision 13.3.

### A4. `idle.altAddress` (the in-house box with an IP address) and the address detection of the wizard
- **Found:** a web page cannot see DNS entries or boxes on the network; there is no IP to show.
- **Meanwhile:** `idle.altAddress` is never shown. Wizard step 3 shows the relay in use (host); "Other address" cycles through
  the relays of `config.json` or, with only one, opens a text field. The source line is `setup.address.sourceDefault` for the
  default relay and `setup.address.sourceOwn` ("Found via the company network (DNS entry)") for any other, which is not literally
  true for a typed address. **Question:** a text for "typed or taken from the page's own address"?
- **Answered:** `idle.altAddress` is native-only; never on the web. Wizard source lines on the web: `setup.address.sourceDefault` (default relay), new `setup.address.sourcePage` "Set by the server of this page." (page's own server or a `config.json` relay), new `setup.address.sourceTyped` "Entered by hand." (typed); `sourceOwn`/`sourceBox` native-only. The text field gets the label `setup.address.fieldLabel` "Server address". Spec 4.1, decisions 13.4, 13.5.

### A5. `{dom}/app`
- **Found:** `idle.appLine` prints `{dom}/app`; the spec does not say what the page is.
- **Meanwhile:** `/app` shows the presenter's start page (same as `/`).
- **Answered:** as built: `/app` is the start page. The idle footer names stores only when their links exist (see B10). Spec 4.2, decision 13.6.

## B. Behaviour the spec leaves open

### B1. Controls on the picture that existed before the spec
- **Found:** section 10 says no other control during a presentation than End. The web display already has: the annotation
  toolbar (pen, marker, eraser, laser, freeze, black screen, full screen; tested by `e2e:m2`), "Close image", the link card
  (Open / Dismiss; the link is opened only by a click, a safety rule), the media player and the PDF page controls. The spec's
  `what.linkSub` says "The screen opens the page itself", which would remove the link card.
- **Options:** keep (as now), show them only together with the name tag and End, or remove them (and decide how a link is
  opened and how a PDF is turned).
- **Meanwhile:** all kept and unchanged. They are drawn at rest (the toolbar is a visible bar at the top). Their buttons that
  sat at the bottom corners (Close image, PDF page buttons, the "frozen" badge) moved to the **top** corners, because the
  name tag and End own the bottom corners (otherwise End covers "Close image" and the mouse cannot reach it).
- **Answered:** keep the annotation toolbar but **hidden at rest**: it fades in and out together with name tag and End (5 s, 400 ms); drawing counts as input. Restyle it with display tokens (spec 3.5: top 12 centred, panel fill, `--line` border, `--acc` border on the selected tool; ink swatches keep their colours); remove its grip and collapse buttons. The toast moves below the visible toolbar. **Remove** "Close image", "Close document" and the display's PDF page buttons (pages are turned on the presenter page, B7). The frozen badge stays (top left, name-tag style) while frozen. The link card stays (safety rule: never open by itself) as a themed dialog like the join request (Open primary with initial focus, Dismiss secondary, 1 s guard, Back = Dismiss); `what.linkSub` becomes "The screen asks before it opens the page". The media player stays inside the card. Spec 4.5, 10, decisions 13.7, 13.8. Owner may still decide to drop the drawing tools (spec 12).

### A6/B2. Settings and the request dialog at the same time; Locked while requests wait
- **Found:** Settings opens only over Idle and not while a request is open (4.3), so "Locked while requests are waiting"
  (toast `toast.lockedRejected`) cannot happen through the screen: a request closes the way to Settings. If Settings is already
  open when a request arrives the spec does not say what happens.
- **Meanwhile:** the request dialog is drawn above Settings; Settings stays open below. The toast and the rejection exist
  in the code (`ReceiverSession.setJoinMode`, unit test) but are not reachable by a person.
- **Answered:** one dialog at a time: a request **closes Settings** (changes already made stay), focus goes to Yes, and after the last request back to the Settings button. Locked therefore cannot be chosen while requests wait: remove `toast.lockedRejected` from the locales and the UI path (the session logic may stay). Spec 4.3, decision 13.9.

### B3. Which changes in Settings are remembered
- **Found:** "after a restart the default mode from setup applies" is said only for Locked.
- **Meanwhile:** Open and Approval on chosen in Settings become the new default (stored in the setup); Locked is never
  stored and also ends with the session (after End the default applies again).
- **Answered:** as built (appearance and Open/Approval on stored as the new default; Locked never stored, ends with End). Spec 4.3, decision 13.10.

### B4. Request expiry
- **Found:** the spec says 60 s on the display; the server already ends a request after `join_approval_secs` (60 s by default,
  an operator can change it).
- **Meanwhile:** no second timer on the display; the display follows the server's cancel message (toast `toast.expired`), the
  presenter's countdown uses the server's value (`join_pending.timeout_secs`).
- **Answered:** as built: the server's timer is the only one. `error.timeout.why` no longer names 60 seconds ("Nobody answered your request in time."). Spec 4.4, 5.4, decision 13.11.

### B5. States of the display that have no design
- **Connecting** (no code yet): the idle screen is drawn with an empty code and no QR code.
- **Any error** (connection lost, rate limit, ban, server full, protocol error): the offline box
  ("No connection to {dom}") is shown for all of them and the connection is tried again after 4 s (30 s after a rate limit
  or a ban).
- **The server refuses open rooms** (`open_rooms_disabled`, the room becomes Approval on): no message, only the pill changes.
- **Content that cannot be shown** (damaged PDF ...): shown as a toast with the existing wording; the progress of a
  transfer is shown only while presenting (in the picture), not on the idle screen.
- **Idle dimming overlay** covers the idle screen only; nothing else is dimmed.
- **Question:** a design for these, in particular "reconnecting".
- **Answered:** Connecting: new state "Idle, connecting" (code line empty but keeps its height, QR box empty at 25 %, no caption, no spinner; after 8 s → offline). Offline: show the box only after 3 s; the QR box is **empty** while offline (no stale, scannable code). Body variants: `idle.offline.body` (network), new `idle.offline.bodyBusy` (rate limit, ban, full; retry after 30 s), new `idle.offline.bodyOutdated` (protocol mismatch; no retry). `open_rooms_disabled`: pill changes plus toast `toast.openDisabled` once. Content errors as toasts, progress only while presenting, dimming only on Idle: as built. Reconnecting while presenting: nothing shown. Spec 4, 4.2, decision 13.12.

### B6. The connection check code (SAS)
- **Found:** the web app shows a 6-digit connection check code on both sides to detect a swapped key exchange
  (`core/sas.ts`). The spec has no place for it.
- **Meanwhile:** the presenter pages show it as the existing small line (page "What to show" and "Presenting"); the display
  shows it as a second line in the name tag, so it is visible only when the controls are shown. **Question:** where it belongs.
- **Answered:** keep it, renamed **"Security code" / "Sicherheitscode"** ("check" is the two-digit number). Display: second line of the name tag, `present.sas` "Security code {sas}", 13/400 `--mut`, digits mono 700. Presenter: only on the "Presenting" page as one line of small print `presenting.sas` ("Security code {sas}. The screen shows the same code under your name."); remove it from "What to show" and drop the long `sas.explain` text. Spec 4.5, 5.6, decision 13.13.

### B7. Presenter page for PDF pages
- **Found:** `what.fileSub` says "You turn pages" but the presenting page (5.6) has only "Stop showing".
- **Meanwhile:** while a PDF is shown the presenting page has two secondary buttons, "Previous page" and "Next page", with
  the existing texts, above "Stop showing".
- **Answered:** accepted: two secondary buttons in a row (phone: half width each) above Stop, new keys `presenting.prevPage` / `presenting.nextPage`, and `presenting.pageOf` "Page {n} of {total}" above them when the count is known. Spec 5.6, decision 13.14.

### B8. File page
- **Found:** a browser cannot list "recent files" (5.5). Picking works only through the system file dialog.
- **Meanwhile:** the page has the heading, the body text, a secondary button with the text of `what.file` that opens the system
  dialog (PNG, JPEG, WebP, GIF or PDF), a progress line while sending, an inline error, and "Back". A picked file starts presenting.
- **Answered:** accepted with changes: heading `what.file`, new body `file.bodyWeb`, the button becomes **primary** with new text `file.pick` "Choose file"; progress, inline error and Back as built. The thumbnail grid is for the apps. Spec 5.5, decision 13.15.

### B9. Replacing what is shown
- **Found:** the presenting page has only "Stop showing"; there is no way to show something else without stopping first.
- **Meanwhile:** replacing goes through "Stop showing" and "What to show" (the display shows the idle screen in between).
- **Answered:** as built (Stop, then pick again). Spec 5.5, decision 13.16.

### B10. Store and download links
- **Found:** no addresses in the spec ("the system store page, not designed by us").
- **Meanwhile:** `apps/web/src/config/stores.ts` has three empty constants (`STORE_URL_GOOGLE_PLAY`, `STORE_URL_APP_STORE`,
  `DOWNLOAD_URL_DESKTOP`). With an empty value the store buttons do nothing; the desktop button shows `start.downloadStarted`
  although nothing is downloaded yet. Fill the constants when the listings exist.
- **Answered:** never show a button that does nothing or a download that did not start: while a constant is empty, hide that platform's app offer (button, its small print, the "or" divider, `start.downloadStarted`), and the idle footer shows only `idle.appLineWeb` "No download needed." until at least one store link exists. `start.downloadStarted` is small print (`p-fg2`), not a green box. Spec 4.2, 5.2, decision 13.6. The URLs come from the owner.

### B11. Buttons that only make sense in an app
- **Found:** "Scan QR code again" (error pages, phone) needs a camera inside the app; the browser page cannot open the camera app.
- **Meanwhile:** the web error pages have no "Scan QR code again" button (`error.action.rescan`, `app.*` strings are unused).
- **Answered:** as built: no rescan button on web pages. Spec 6, decision 13.17.

### B12. Error pages for failures that the spec does not list
- **Found:** rate limits, bans, a full server, a protocol mismatch, a full room.
- **Meanwhile:** same layout (icon disc, heading, "Try again"); the heading is the existing wording
  (`errors.<code>`), there is no "Why / What now" card. Mappings used: `connection_lost` -> "No internet on this device",
  `peer_failed` -> "The screen does not answer", `removed_by_host` and a sender that lost its room -> "Presentation ended",
  `room_not_found` after waiting for approval -> "This code is no longer valid", otherwise "This code does not exist right now".
- **Answered:** same layout **with** the Why / What now card. New pages: `error.occupied` (`room_full`), `error.busy` (`rate_limited`, `banned`, `server_full`), `error.outdated` (`proto_mismatch`, `bad_hello`, `bad_message`; button `error.action.reload`). Your other mappings stay (`removed_by_host` → `error.ended`). Copy in spec 11.6, mapping in 6, decision 13.18.

### B13. Dark presenter pages: icon discs
- **Found:** 3.3 gives only the foreground colour of the error, ok and warn icons in dark mode (the mockup leaves the light discs).
- **Meanwhile:** the discs are the same colour at 15 % opacity; the grey and accent discs follow the table.
- **Answered:** as built (15 % tints), now fixed as hex: error `#342A2C`, ok `#21312F`, warn `#332F26`. Also: `p-disabled` light `#DDE1E8` with text `#4A5160`, dark `#2A2F38` with text `#9EA6B3` (the built light disabled button has white text at 1.8:1). Spec 3.3, decision 13.19.

### B14. Things in the product that the spec does not mention
- The status line for screen readers (`role="status"`, not drawn) on both page kinds; it also serves the automated tests.
- No language switch (the spec has none): German or English by the browser language.
- The relay field (tag, warning for a non-default relay, `config.json` hint, typed relay) was removed; the confirmation dialog
  for a relay that only a link asked for stays with its older look (it is not themed by the spec).
- The page asks for fullscreen on the first click (7.2) on the display; a refusal is ignored.
- **Answered:** status line, no language switch, removed relay field and fullscreen on first click: accepted. The relay confirmation dialog must be themed (display dialog card on `/display` with Cancel focused; presenter card on presenter pages); copy unchanged. Spec 7.2, decision 13.20.

### B15. Names of keys
- `settings.join` (the section label) and `settings.join.openText` ... cannot both exist in nested JSON; the label is stored as
  `settings.join.label`. Flutter's ARB (`settingsJoin`) is not affected.
- **Answered:** accepted; the spec now uses `settings.join.label` (ARB `settingsJoinLabel`). Decision 13.21.

## C. Technical note, not a question

- `cometcast-server` answers `/display`, `/<CODE>` and `/app` (paths without a file extension that are no file) with
  `index.html` (`crates/cc-server/src/lib.rs`, test `healthz_and_static_files`). A static host needs the same (GitHub Pages: a
  copy of `index.html` named `404.html`). Without it the QR code of the display leads to a 404.
- The ReceiverSession and SenderSession got: events for the toasts, `defaultMode` (Locked does not survive End),
  and a screen that is added to and removed from a connected session by renegotiation (so "Stop showing" returns to "What to
  show" without leaving the room). A direct stream removal is detected with the `removetrack` event of the remote stream
  (works in Chrome; other browsers were not tried).
- **Answered:** acknowledged; the static-host requirement is now in spec 7.2 (decision 13.22), and the session changes need no design decision.
