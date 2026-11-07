# Implementation questions: Flutter app

Asked by the Flutter implementer. Each item: what was found, the options, and what was done meanwhile (the minimal neutral behaviour). Nothing here was decided by the implementer.

## Core and protocol gaps (not design, but they change what the screens can show)

- **C1 Room name to presenters.** `cc-ffi` does not pass `room_name` to `create_room` and does not report it on `join_pending` / `joined` (the protocol and `cc-receiver` / `cc-sender` have it). Options: add it to `start_receiver` and the sender events (needs a bridge regeneration). Meanwhile: the display shows its own room name; the presenter pages show no room (spinner only, `what.connected` header omitted, no `presenting.where` line), as the spec says for "no room name".
- **C2 Security code.** The core computes no security code (the web does it in `core/sas.ts`). Meanwhile the name tag has no second line and `presenting.sas` is not shown.
- **C3 Page and page count from the display.** Not reported (decision 13.31), so no "Page n of m" and both page buttons stay enabled (as the spec says until it exists).
- **C4 Receive progress.** The core reports no percent for a running transfer. Meanwhile the transfer pill shows `receive.transfer` with 0 %.
- **C5 Presenter joins only when it sends.** The core joins a room when it starts to send, so the designed order "waiting page, then What to show" cannot be built. Meanwhile: Connect leads to "What to show" without being joined (the "Connected to ..." header is left out because it would not be true); the waiting page appears after the person chose Screen, File or Link.
- **C6 Server address of a presenter.** The first-run page has no server field and there are no link handlers for `https://{dom}/{CODE}` yet. Meanwhile the presenter uses `AppSettings.serverAddress` (policy, DNS, last used, built-in). The relay confirmation page is built and tested but not wired to a link.
- **C7 QR scanner.** `app.scanQr` needs a system scanner (ML Kit, VisionKit); none is built. Meanwhile the scan button is hidden and the code field of the computer variant is shown on phones and tablets too.
- **C8 Display leaves a presentation.** When the presenter leaves, the core's receiver ends; the app starts a new room, so the code changes (spec 4.5 says the same code stays). Toast `toast.left` is shown.
- **C9 Mapping of core errors to error pages.** The core reports English messages (`server error <code>: ...`, `signalling connection ...`); the page is chosen by that text (`lib/src/presenter/errors.dart`). Connect/closed/timeout map to `error.noInternet`, WebRTC errors to `error.displayOff`. Please confirm.

## Copy and design gaps

- **D1 `receive.link.hint`** says the link "opens in a new tab". In the apps there is no tab. Used unchanged.
- **D2 `idle.appLine`.** The apps have no store link configuration, so the footer always says `idle.appLineWeb`.
- **D3 Address source for policy.** `ServerSourceKind.policy` (administrator) uses `setup.address.sourceOwn` (company network); in-house box announcements use `sourceBox`; everything else `sourceDefault`. "Other address" is not offered when a policy locks the server.
- **D4 Content errors as toasts.** `UiContentError` is mapped to the `receive.transferError.*` texts (`interrupted` and `transferTimeout` to `stalled`, `renderFailed` and `tooManyErrors` to `document_failed`, `workerUnavailable` to `document_unsupported`). Please confirm.
- **D5 Request ended by the server.** `cancelled` and `left` both show `toast.withdrawn`; `timeout` shows `toast.expired`.
- **D6 Codec card.** The H.264 download consent has no designed place. Meanwhile the existing Material card appears centred over the display while an H.264 picture cannot be decoded, and on its own page when a desktop presenter chooses Screen. The spec mentions codec notices as toasts (native-apps.md 18) but a consent needs a button.
- **D7 Windows Miracast receiver.** No designed place. Meanwhile one small text button "Miracast" at the bottom right of the presenter first-run page (Windows only) opens the old page.
- **D8 Nearby screens and advertising.** The list of nearby screens is not designed (spec 12). The old "Nearby receivers" list is gone; the display no longer announces itself in the local network (it was part of the removed receive page).
- **D9 Test video and monitor choice.** The old screen sender offered a VP8 test stream, a duration and a monitor list. These have no place in the designed pages and were removed from the UI (the core API is unchanged). Desktop screen sharing shares the whole desktop.
- **D10 German legacy strings.** Strings of the pre-design UI that are still used (codec card, Miracast, content refusals, capture notification, discovery notes) were converted from Sie to du by changing only pronouns and verb endings. They are not in the spec tables.
- **D11 Annotation toolbar** (`lib/annotations.dart`) is kept but not reachable (native-apps.md 9: build flag off); its tests stay.
- **D12 Lifecycle on tablets.** The display session ends when the app goes to the background on Android and iOS (`paused`), without a toast, as native-apps.md 15 says. Desktops ignore minimising.
- **D13 Stopping a screen share on the presenter** returns to "What to show" (spec 5.6); on iOS the broadcast extension keeps its own lifecycle (never run).
