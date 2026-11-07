# Fix list 3: nearby screens and desktop screen sharing

From the designer's review of main 01ea595 (branches `nearby-screens` and `capture-backends`) and the 47 screenshots in `/tmp/shots-nearby`, against spec 14, the mockup (https://claude.ai/artifact/2o5vZaZk9zuzBkPcYeKtWF) and native-apps.md 4.1, 8, 20.3. Decisions: spec 13, "Decisions after the nearby and capture round" (N48–N59); answers: [implementation-questions-flutter.md](implementation-questions-flutter.md) G, H, I.

**Already right, no work:** every state of 14.2 on phone and laptop, light and dark (Not asked yet, Searching with the dashed placeholder, Found, Nothing found, Paused, Not allowed, No local network, hidden with a chip and by policy); row anatomy (icon, quoted name up to 2 lines, `common.noName` in 17/500 `p-fg2`, mode, `via`, mono code), locked row (dashed, `p-fg3`, no code), connecting row (spinner, others at 55 %), laptop card (`p-surface`, `#E2E4E9` / `#2B2F38`, radius 14, padding 16, rows on `p-bg`), compact button, relay page with `relay.confirm.bodyNearby` and `common.cancel`, DE du found and relay.

Each item: **What**, **Where**, **Acceptance**. Do part A top to bottom; part B is platform work and can run in parallel; part C last.

## Part A: Flutter UI and core

### A1. `nearby.gone` survives the refill
- **What (N51):** after `room_not_found` the line disappears as soon as the first row of the new search arrives (about a second), because `_put` and the `Removed` case set `_goneLine = false`.
- **Where:** `lib/nearby_controller.dart` (`_put`, `_onEvent` Removed; clear only in `beginConnect`, `stop`, a tap on the page, and `start` that is not `restartKeepingLine`), the start page's tap handling in `lib/src/presenter/presenter_root.dart`.
- **Acceptance:** widget test: tap a row → `room_not_found` → rows arrive again → the line is still shown under the list; it goes with the next tap anywhere on the page (row, field, Scan, Connect) or when the page is left. Screenshot `nearby-{phone,laptop}-light-gone-refilled`.

### A2. While a row connects, nothing else starts a join
- **What (N54):** in `*-connecting` Connect is disabled but `app.scanQr` stays active and the code field still submits on Enter.
- **Where:** `lib/src/presenter/pages.dart` (`FirstRunPage`), `presenter_root.dart`.
- **Acceptance:** `nearby-*-connecting`: Connect disabled primary, Scan disabled secondary (N23), name and code fields read-only with unchanged look; Enter in the code field does nothing; all return when the start page comes back.

### A3. `nearby.via` keeps "via" with the host
- **What (N53):** `Approval on · via` / `cast.musterfirma.de` breaks between "via" and the host (`nearby-*-found`, row "Room 204").
- **Where:** `nearbyVia` in `lib/l10n/app_en.arb`, `app_de.arb`, `sie/app_de.arb`: a no-break space (U+00A0) instead of the space.
- **Acceptance:** the second line of that row reads "via cast.musterfirma.de" / "über cast.musterfirma.de" in all phone and laptop shots.

### A4. Laptop links under Connect
- **What (N37):** in the laptop layout with the nearby card the links follow the 14 gap of the column: `app.beDisplay` starts 14 below Connect and, on Windows, `app.miracast` 14 below it.
- **Where:** `FirstRunPage` laptop branch (`column(...)` in `pages.dart`).
- **Acceptance:** first 44-high tap area 12 below Connect, the second directly under it (no gap), both left-aligned; shot `nearby-laptop-light-found` (with `app.beDisplay`) and `nearby-laptop-light-found-windows` (both links).

### A5. The 33rd screen does not push a row out
- **What (N49):** the core drops the first found screen when a 33rd arrives.
- **Where:** `cc-discovery` nearby list (and `NearbyController.maxRows` stays as the guard).
- **Acceptance:** core test: 33 announcements → rows 1–32 unchanged in order, the 33rd not reported; row 5 leaves → the 33rd is reported at the end with its next refresh.

### A6. Screen-sharing page `error.capture`
- **What (N56):** one page for every "cannot share" case on desktops, replacing `error.other` + Back (N33) and the unsupported codec page (5.8). Codes `capture_os_old` (macOS before 14) and `capture_no_portal` (Wayland without the ScreenCast portal or libpipewire) are new, non-fatal like `capture_unavailable` (`presenterStartFailureCodes`).
- **Where:** `cc-capture` / `cc-ffi` (report the two codes), `lib/presenter_session_controller.dart`, `lib/src/presenter/errors.dart`, `presenter_root.dart`, the codec page's unsupported state; ARB: new `errorCaptureTitle`, `errorsCaptureOsOld`, `errorsCaptureNoPortal`, changed `errorsCaptureUnavailable`, restored `errorNoScreenNowNoApp` (values in spec 11.9, all three sets).
- **Acceptance:** error layout of 6: (i) in grey, `error.capture.title`, why = `errors.<code>` (or `codec.unsupported`), now = `error.noScreen.nowNoApp`; buttons Show photo or file (primary, initial focus) → file page, Show a link → link page, Back → "What to show"; still connected; phone stacked, laptop one row. Shots `capture-{phone,laptop}-{light,dark}-{unavailable,os-old,no-portal,codec}` and DE du of each on the laptop.

### A7. A closed shared window ends the presentation quietly
- **What (N55):** closing the shared window, or ending the share in the system UI, must behave like Stop.
- **Where:** `cc-capture` backends (PipeWire stream removed, `GraphicsCaptureItem.Closed`, `SCStream` stopped) → `PresentingStopped`.
- **Acceptance:** presenter back on "What to show" (connected), display to Idle with the same code, no toast, no error page; a minimised window keeps the last picture with no message. Checked in the three test plans.

## Part B: platform work (no visible change)

- **B1 (N50) Expiry on Android and iOS:** a row leaves ≤ 35 s after a display loses power without goodbye, on Android, iOS, Linux, Windows, macOS; if the system's cache keeps answering, set the record lifetime to ≤ 30 s (patched or newer `mdns-sd`).
- **B2 (N50) Announcers:** Android display role holds a `WifiManager.MulticastLock` while announcing; iPad display role announces with `NWListener`; Apple TV announces the 14.8 TXT layout. Acceptance: each appears as a row on an Android phone and an iPhone in the same network.
- **B3 (N50) macOS browse with `NWBrowser`:** Not asked yet → `nearby.search` → system prompt; Don't Allow → Not allowed with `nearby.allow` opening `Privacy_LocalNetwork`.
- **B4 (N55) Windows picker owner:** `cc-ffi` passes the Flutter window's `HWND` as `owner_window`; test plan A1 confirms the cancel codes map to `capture_denied`.
- **B5 (N57) Linux:** the app starts on a machine without libpipewire (run-time loading, or a declared dependency in every package format shipped); the app id is registered with the portal so the picker names "Comet Cast"; restore tokens stay off.
- **B6:** run the three capture test plans (Linux Wayland, Windows, macOS 14 and 13); B7 of fix list 1 closes with them.

## Part C: screenshots (N58)

- **C1:** re-shoot `/tmp/shots-nearby` without the debug banner (`debugShowCheckedModeBanner: false`), with `app.beDisplay` on every laptop start page (a computer always has it) and one Windows variant with `app.miracast`.
- **C2:** add DE du for none, paused, denied, no-wifi, connecting and gone (phone and laptop), the shots of A1 and A6.
