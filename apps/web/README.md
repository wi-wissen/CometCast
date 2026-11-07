# Comet Cast web app

The no-install variant of Comet Cast: runs in a browser. Early version (v0.1). Its pages follow the approved design
spec [docs/design/first-run-and-display.md](../../docs/design/first-run-and-display.md) (colours, sizes, texts, timings and
behaviour come from there; nothing visual was invented in the code). Two kinds of page:

- **The display** at `/display`: the setup wizard (first run), the idle screen with the address, the join code and a QR code,
  Settings (join mode, appearance), the join request dialog, and the full-bleed presentation with the name tag and End.
- **The presenter** at `/` and `/{CODE}` (the QR code opens the latter with the code filled in): browser or app, waiting for
  approval, what to show (screen, photo or file, link), presenting, and the error pages.

Screen video goes over WebRTC, signalling over WebSocket `/ws` (protocol in `docs/protocol.md`). Without a STUN/TURN list from
the server it works over the local network only and may not work across networks. Servers in open mode require a small
proof-of-work (ALTCHA) before a room is created; the page solves it by itself (see "Open servers and TURN" below). The display
has a local annotation toolbar (hidden at rest, it shows with the name tag and End), and images, PDFs and links can be sent over a WebRTC DataChannel (milestone M2, see
`docs/web-m2.md`). A PDF is rendered by `crates/cc-docs-wasm` (hayro compiled to WebAssembly, in a Web Worker). Build it with
`npm run build:wasm` (needs the Rust target `wasm32-unknown-unknown` and a `wasm-bindgen` CLI of exactly the
version in `Cargo.lock`) **before** `npm run build`; it is written to `public/pdf/` (git-ignored). Without it
the app works as before and a received PDF only gives the notice that documents cannot be shown. Office
documents are not implemented. The rest of the Rust core is not used in the browser yet (see `src/core/index.ts`).
End-to-end test: `npm run build:wasm && npm run build && npm run e2e:pdf`. The Docker image (`deploy/Dockerfile`) does **not** contain the renderer (`public/pdf` is excluded from its build context and the image has no Rust target), so a PDF received by a web app served from that image only gives the "cannot show documents" notice.

Stack: Vue 3, TypeScript, Vite, Tailwind CSS 4 (only for the older toolbar and card components; the pages use the spec's tokens in plain CSS), vue-i18n (English and German), qrcode-generator (MIT, QR code), Atkinson Hyperlegible Next and Mono (OFL, bundled, no font service).

## Commands

```sh
cd apps/web && npm ci && npm run dev
```

- `npm run typecheck` - type-check with vue-tsc
- `npm test` - unit tests (vitest)
- `npm run build` - production build into `dist/`
- `npm run preview` - serve the production build (no signalling server; use the Rust server below)
- `npm run e2e` - end-to-end test, see below (not part of `npm test`)
- `npm run e2e:native` - interop test with the native Rust peers, see below
- `npm run e2e:m2` - end-to-end test for the toolbar (hidden at rest), freeze, images and the link card (needs `npm run build` first)
- `npm run e2e:static` - app on its own static origin, relay on another (builds by itself, see "Static hosting")
- `npm run e2e:display` - the display and presenter pages, see "Display and presenter tests" (needs `npm run build` first; builds a second copy by itself for the dimming test)
- `npm run e2e:join` - join approval with the request dialog, see "Join approval test"
- `npm run e2e:shots` - writes a PNG of every state (display: Dark / Light / OLED black, presenter: light / dark; 1280 x 720 and a phone viewport) to `$E2E_SHOTS` (default `/tmp/shots`); every display shot is taken after one arrow key press (focus ring); needs `npm run build:wasm` first; checks almost nothing (the ban test asserts that the second attempt still says busy)

## Trying it

```sh
npm run build
cargo run -p cometcast-server -- --listen 127.0.0.1:8443 --web-dir apps/web/dist   # from the repo root; it answers /display and /<CODE> with index.html
```

Open `http://127.0.0.1:8443/display` in one tab (click through the setup, it asks once) and `http://127.0.0.1:8443/` in
another: the display shows a join code and a QR code; type the code on the other page, choose "Continue in the browser" and
then "Share screen". For development with `npm run dev`, add
`?server=127.0.0.1:8443` to the URL so the page talks to the running server (works in every build, see below).

## Code layout

- `src/core/signaling.ts` - WebSocket client and typed messages (mirrors `cc-proto`). The optional `room_name` (display's room name, sent with `create_room`, back in `join_pending` and `joined`) is read leniently: absent, null, not a string or blank means no room name; the presenter pages then show no room at all and never the code
- `src/core/relay.ts`, `src/composables/useRelay.ts`, `src/components/RelayConfirm.vue` - relay validation and selection, `config.json`, the confirmation dialog for a relay from a link
- `src/core/peer.ts` - RTCPeerConnection wrapper (takes the STUN/TURN list from the server; none by default)
- `src/core/altcha.ts`, `altchaWorker.ts`, `altchaClient.ts` - ALTCHA solver (WebCrypto PBKDF2, runs in a Web Worker), `GET /altcha/challenge` and `GET /ice-servers`
- `src/core/receiverSession.ts`, `senderSession.ts` - explicit state machines
- `src/core/control.ts` - DataChannel "control": message format, chunking, reassembly, link validation
- `src/core/annotations.ts`, `toolbarState.ts` - local annotation model and toolbar state (pure, unit-tested)
- `src/composables/useReceiver.ts`, `useSender.ts` - Vue glue
- `src/core/router.ts` - the page is chosen by the path (`/display`, `/{CODE}`, anything else = presenter start), also under a `VITE_BASE`
- `src/styles/display.css`, `presenter.css` - the tokens and sizes of the spec (display: reference px on a 960 x 540 canvas scaled with `transform`, `--s` set by `useDisplayScale`)
- `src/views/DisplayView.vue`, `components/display/*` - setup wizard, idle screen, Settings, request dialog, presenting stage, toast
- `src/views/PresenterView.vue`, `components/presenter/*` - the presenter pages
- `src/core/appearance.ts`, `displaySetup.ts` - appearance (Dark default, OLED black, Light, Automatic; kept in `localStorage` of the display device) and the first-run setup (room name, default join mode, server)
- `src/core/idleDim.ts`, `controlsReveal.ts`, `wakeLock.ts`, `dpad.ts` - idle dimming, controls on input while presenting, Screen Wake Lock, focus moving in rows
- `src/core/busyMemory.ts` - 60 s memory of a busy refusal; `src/core/qr.ts`, `platform.ts`, `presenterErrors.ts`, `codeFormat.ts` - QR code, "Detected: ...", which error page, code grouping
- `src/config/stores.ts` - the store and download addresses: **placeholders** (empty) until the apps are published. An empty address means no offer at all (no button, small print, "or" divider or download note on the start page, no "Get the app" on the error page; the idle footer says only "No download needed.")
- `src/core/connectionView.ts` - what the idle screen shows about the connection: the code, "connecting" (empty code line and QR box) or, once the retry at 3 s has failed (8 s at start), the offline box; the retry schedule (1 s, 2 s, 3 s after the loss, then every 4 s; 30 s for a busy server, none for a page that is out of date)
- `src/core/focusRing.ts`, `composables/useFocusRing.ts` - the display's focus ring rule (shown on load, hidden by the first mouse or touch input, shown again by a key)
- `src/core/textSlot.ts` - cuts a translated sentence where a styled value (security code, link) belongs

## Open servers and TURN

- **ALTCHA.** The receiver sends `create_room` as before. If the server answers `altcha_required` (open
  mode always does), the page fetches `GET /altcha/challenge`, solves it and sends `create_room` again
  with the `altcha` field. While it works the status reads "Checking your browser..." (`receive.status.checking`).
  The scheme (read from the `altcha` crate 1.1.0 that the server uses): password = nonce bytes followed by
  the counter as 4 bytes big-endian; `PBKDF2-HMAC-SHA256(password, salt, cost iterations, keyLength bytes)`;
  the first counter from 0 whose result starts with `keyPrefix` is the answer. Each attempt is one WebCrypto
  derivation, in a module Web Worker (`altchaWorker.ts`); if workers are unavailable the same code runs on the
  main thread. The payload is the base64 of `{"challenge": ..., "solution": {counter, derivedKey, time}}`.
  Refusals map to translated messages: `altcha_invalid`, `rate_limited`, `banned` (also from HTTP 429 and 403 with
  `Retry-After` on the challenge request), plus `forbidden` (403 without `Retry-After`), `server_full` (503) and
  `altcha_failed` (network problem or solver failure).
  UNCERTAIN: how long solving takes on weak devices is not measured (docs/measurement-points.md, point 8);
  the 60 s give-up time is a placeholder. The server closes a connection that is in no room after 60 s by default
  (`no_room_secs`), so a very slow device could be cut off while solving.
- **STUN/TURN.** Both sender and receiver ask `GET /ice-servers` once per session and pass the list to
  `RTCPeerConnection` (`iceServers`). `404` (no TURN configured, always the case in open mode), a failed request or
  an unusable answer all mean "no list"; the page then connects as before. Entries with a `turn:` URL but without
  user name and credential are dropped. UNCERTAIN: relaying through a real coturn was not tried; the end-to-end test
  only checks that the list reaches the peer connection (with correct credentials).
- **Web app on another origin.** `GET /altcha/challenge` and `GET /ice-servers` go to the http(s) origin of the
  **selected relay** (see the next section), which needs `allowed_origins` on the relay (docs/server.md).

## Static hosting and relay selection

The build (`dist/`) is plain static files (Vue + Rust WASM): host it on GitHub Pages, any web server, or let the
relay serve it (`--web-dir`, the Docker image does). The relay (`cometcast-server`) relays signalling (room codes, offers/answers, ICE candidates); media goes peer to peer.

**Which relay the page uses**, first match wins (`src/core/relay.ts`, tests in `tests/relay.test.ts`):

1. `?server=` in the page URL, or an address typed in the display's setup wizard (step 3, "Other address"; kept for this
   tab in `sessionStorage`, and in the display's setup in `localStorage`). Accepts `wss://host[:port][/ws]`, `https://...` as a spelling of it, or a bare `host[:port]`.
   `ws://` is accepted only for localhost, single-label names, `.local`/`.lan` and private/loopback/link-local IP
   addresses (LAN, development). No user info, query, fragment or other path. An invalid value is ignored.
2. `config.json` next to `index.html` (optional; fetched at start with a 1.5 s timeout, no credentials):
   `{ "relays": ["wss://relay.example.org/ws", "wss://second.example.org/ws"] }` (or `{ "relay": "..." }`). Only ws/wss
   URLs, at most 8, invalid entries dropped, first = default, the others are what "Other address" in the wizard cycles through. It is parsed as JSON and
   never executed and read with a streaming reader that stops at 8 KiB. A missing file (404) or an HTML fallback page
   means "no config" silently; a timeout, a network or server error, a body over 8 KiB, invalid JSON or a file without
   a usable relay is reported by `useRelay().configFailed` and the default is used (the spec has no place for a visible hint,
   see `docs/design/implementation-questions.md`). A `ws://` entry is dropped on an https page (mixed content).
   Example: `deploy/config/web-app-config.example.json`.
3. Build-time default `VITE_DEFAULT_RELAY`, e.g. `VITE_DEFAULT_RELAY=wss://relay.cometcast.de/ws npm run build`.
4. The page's own origin (the relay serves the page: Docker image, `--web-dir`, `npm run dev` with `?server=`).

The display's setup wizard shows the address in use (host only) in step 3, with the source line `setup.address.sourceDefault` (the
default relay built into the page), `sourcePage` (the page's own server, `config.json` or a link) or `sourceTyped`
("Entered by hand."); "Other address" goes through the relays of `config.json` and then opens a text field in place (Back
cancels it). The same address is the big address on the
idle screen and inside the QR code (`https://{dom}/{CODE}`, plain `http://` for a local relay). The presenter pages take the relay
from the same rules; a link (`?server=`) is the only way to point a presenter somewhere else. Presenter and display must use
the same relay. The earlier relay field (tag "default" or "custom", a warning for a non-default relay) is gone: the spec has no
such element.

**A relay chosen only by a link (`?server=`) is never used silently.** Before anything else is shown (the display, the presenter's
start page) a confirmation (on `/display` a dialog card on the canvas, on presenter pages its own page) names the host and says that the operator of that
relay can see who connects, the room codes and network addresses, and can redirect the connection. "Use this relay"
remembers the confirmation for this tab (`sessionStorage`) and for exactly that relay; "Cancel (use the default relay)"
drops the `?server=` for this page view and starts nothing. `config.json` relays, the build default, the page's own
origin and an address typed in the wizard are trusted without the dialog. An address that came from a link is never written
into the display's setup. There is no URL parameter or other switch that
skips it (the e2e tests click through it). A `ws://` relay on an https page is refused when entered or in `?server=`
(mixed content, translated message).

**What the relay can and cannot do (UNCERTAIN, not audited).** Screen, images and documents travel directly between
the two browsers, encrypted (DTLS-SRTP), unless a TURN relay is configured, which then forwards the encrypted packets.
The relay carries the signalling, including the DTLS certificate fingerprints, so a malicious relay could in
principle interfere with the key exchange (swap fingerprints) and sit in the middle. **Security code**
(`src/core/sas.ts`; "Security code" / "Sicherheitscode", not "check", which is the two-digit number): when connected, sender and
receiver show a 6 digit code (display: second line of the name tag, only while the controls are shown; presenter: one line of
small print on the "Presenting" page only), the SHA-256 (domain-separated
`cometcast/connection-check/v1`) of the two DTLS fingerprints of the final offer and answer, sorted, reduced to 6 digits.
Both sides show the same code unless the fingerprints were swapped. Limits: people have to compare it (nothing is
enforced and the page cannot tell if they did); a short code without a commitment step can in principle be ground by an
active attacker who has time to try many certificates, so it is a check against casual interception, not a proof;
tested with fake peers (vitest) and in `npm run e2e` / `npm run e2e:static` (both sides show the same code), never
against a relay that really swaps fingerprints.

**Branding.** The links "Privacy" and "Imprint" of the start page footer (and the source link) come from the branding file selected at
build time by `COMETCAST_BRANDING` (default `branding/default.json`: no privacy or imprint link; `branding/cometcast.json` for the official
cometcast.de build), embedded by `vite.config.ts`. A `config.json` with `privacy_url` / `imprint_url` overrides them at runtime. There is no
host check any more. See `branding/README.md`.

**Base path.** `VITE_BASE=/app/ npm run build` builds for `https://cometcast.de/app/` (default `/`; must be an absolute
path, `./` is rejected because the PDF worker builds its URLs from it). Assets, the two Web Workers, the logo in the
logos and fonts and `<base>pdf/` (the optional WASM renderer from `npm run build:wasm`) all follow it; there is no service
worker, web manifest or other remote asset (the Atkinson Hyperlegible fonts are bundled). Checked by building with `/app/` and
serving it at that path (`npm run e2e:static`). `config.json` is read from `<base>config.json`. The pages are chosen by
path (`<base>display`, `<base><CODE>`), so a **static host must answer those paths with `index.html`**: `cometcast-server` does
(docs/server.md), GitHub Pages needs a copy of `index.html` named `404.html`, `e2e:static` does it in its own static server.

The relay must allow the page's origin: `allowed_origins = ["https://cometcast.de"]` (docs/server.md, "Web app on
another origin"); otherwise the browser blocks `/ice-servers` and `/altcha/challenge` and `/ws` is refused with
`403` when a list is set. The display then shows its offline box ("No connection to {dom}") and tries again; the presenter shows "No internet on this device".
`npm run e2e:static` (Chrome or Chromium, `E2E_CHROME=...`) builds the app with `VITE_BASE=/app/`, serves it from a
second plain static server, starts relays on other ports, and checks `?server=`, `config.json`, ALTCHA across origins,
video through the relay, the confirmation dialog (nothing connects before it, Cancel falls back to the default), the
hint for an unusable `config.json`, the same connection check code on both sides, that a third origin is rejected, and
that a rate-limited client on the allowed origin can read `429` and `Retry-After`. UNCERTAIN: real GitHub Pages and real DNS were not tried.

## End-to-end test

`npm run build && npm run e2e` builds and starts the real `cometcast-server` on a random local port,
opens a receiver page and a sender page in headless Chromium (Playwright), joins with the shown code,
and checks that the receiver's `<video>` has a size and its `currentTime` advances, then that
stopping on the sender shows "peer left" on the receiver. It also checks the translated
`room_not_found` error for a wrong code. Needs a Rust toolchain and Playwright's Chromium
(`npx playwright install chromium`).

What it proves: signalling, offer/answer/ICE and media over one machine's loopback/host candidates.
What it does not prove: behaviour across networks/NAT (no STUN/TURN), other browsers, real screen
content, audio, or encryption on the wire (the test server speaks plain HTTP). It uses the launch flag
`--disable-features=WebRtcHideLocalIpsWithMdns` so that host candidates are usable in a sandbox.

getDisplayMedia worked headless in the preinstalled Chromium with
`--use-fake-ui-for-media-stream --auto-select-desktop-capture-source=Entire screen`
(UNCERTAIN: observed only in this one environment; the captured source there is whatever the headless
browser offers, not a real desktop). As a fallback there is a **TEST-ONLY** query flag `?testVideo=1`
on the page: the sender then sends an animated canvas instead of the screen. Run
`E2E_TEST_VIDEO=1 npm run e2e` to use it. Do not use it for anything else.

## Join approval test

`npm run build && E2E_CHROME=/usr/bin/google-chrome npm run e2e:join` starts a real server (join requests wait 5 s) and
several Chromium pages and checks join approval ([protocol.md](../../docs/protocol.md#join-approval)) with the request dialog
of the spec: the default mode is Open; in Approval on a request appears over the idle screen with the same check number as
on the presenter's waiting page, the name is plain text, Yes has the focus and presses inside the first second answer
nothing; Yes, No, No and block and keyboard-only answers (arrows, Enter); Locked and Open again; a blocked address is refused at
once and not shown; a request nobody answers times out (presenter: "No answer", display: toast); withdrawing tells the host;
German texts (du) on both pages; the default mode is kept on the display. Not covered any more because the spec removed
the controls: "End presentation" and the block list with Unblock, "Later", the join menu and its notice. Their logic
(`ReceiverSession.removePeer`, `unblock`, `denyAll`) stays and is covered by `tests/joinApproval.test.ts`.

## Display and presenter tests

`npm run build && E2E_CHROME=/usr/bin/google-chrome npm run e2e:display` (real server, real Chrome) covers: the setup wizard
(focus per step, Back, the empty custom name field and the chips, the address source lines and the address field, the focus ring
rule: shown on load, hidden by the first mouse input, back after a key; what Finish stores), the idle screen (address, code, QR URL, pill), Settings (join mode, appearance
Dark / Light / OLED black / Automatic, kept after a reload; arrow keys in rows; Escape and the Menu key), the request dialog
(queue "+1 waiting", first come first served, 1 s guard), Yes -> presenting full bleed (the picture fills 1280 x 720, nothing
else drawn) -> mouse and key reveal the name tag and End (first key only reveals, hides after 5 s) -> End -> new code and toast ->
the presenter sees "Presentation ended"; "Stop showing" leaves the display on idle with the same code; No and No and block;
Locked; the screen wake lock (`navigator.wakeLock` is replaced by a counting mock: nothing on idle, one `acquire` while presenting, `release`
afterwards, also for a link); idle dimming; the presenter's start page (empty code, QR path, German, dark, a phone viewport with
Android user agent, no horizontal scroll), the code field (every typed character stays, a code with an O is the error page without a
server call), the room name on the presenter's waiting, "What to show" and "Presenting" pages, the security code on both sides,
that a request closes Settings, that no app offer is drawn without a store link, and the error pages. `E2E_SHOTS=/tmp/shots` also writes a PNG of each state.

**Idle dimming in tests.** The delay is 7 minutes (`IDLE_DIM_MS`). Only a build made with the environment variable
`VITE_E2E_DIM_MS` (milliseconds) changes it; `e2e:display` makes such a build in a temporary folder. The normal build has no
such value and no URL parameter, storage key or other runtime switch can change the delay.

## Open mode and TURN test

`npm run build && npm run e2e:open` starts real servers (several processes) and checks, with headless Chromium and
the real `cometcast-cli`: a raw client without ALTCHA gets `altcha_required`; the web receiver shows the translated
"Checking your browser..." status (English and German), solves the puzzle in a Web Worker and creates a room on an
open-mode server, a web sender joins and video plays; `cometcast-cli receive` creates a room on the open server and
receives from a web sender; `cometcast-cli send` reaches a web receiver; repeated room creation ends in the translated
`rate_limited` message and failed joins in the "The server is busy" page (a ban is told like a rate limit); on an internal-mode server with a TURN
secret both web pages put the server's list (with credentials that match HMAC-SHA1 of the user name) into their
`RTCPeerConnection` (read by wrapping the constructor in the test page) and the CLI reports the list; the CLI connects
over `wss://` to a self-signed server with `--ca-file`. TURN is tested on a separate internal-mode server because open
mode refuses a `[turn]` section. The TURN host in the test does not exist: nothing is relayed.

## Interop test with the native Rust peers

`npm run build && npm run e2e:native` starts the real server and the real `cometcast-cli`, then checks
two directions in headless Chromium: (A) the web sender page sends to `cometcast-cli receive`, and the
CLI's per-second packet counts must keep increasing; (B) `cometcast-cli send` sends a VP8 test pattern
to the web receiver page, whose `<video>` must decode it (320x180) and advance. It uses the same Chromium
flags as `npm run e2e`. See [../../docs/transport.md](../../docs/transport.md).

## Translations

UI strings live in `src/locales/en.json` and `src/locales/de.json`. Both files must have identical keys
(checked by a test). `de.json` is the informal **"du"** set (shipped, loaded by the app). The formal **"Sie"** set is kept complete in
`src/locales/de-sie.json` (identical keys, not loaded). To ship it, import it instead of `de.json` in `src/i18n/index.ts`
(one import; no other code changes). `tests/locales.test.ts` reads the copy tables of the spec and checks every row word
for word (EN, DE du in `de.json`, DE Sie in `de-sie.json`), plus identical keys and `{placeholders}`;
`node tools/check-i18n.mjs` checks that `de-sie.json` has the keys and placeholders of `de.json` and that `de.json` has no formal forms (Sie, Ihr..., Ihnen). The key `settings.join` of the spec is stored as
`settings.join.label` because nested JSON cannot hold a text and a group under the same name. The language follows the
browser (`navigator.language`, German or English); there is no language switch in the spec. Screen sharing needs a desktop browser and HTTPS (or localhost).

License: GPL-3.0-or-later
