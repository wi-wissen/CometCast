# Comet Cast web app

The no-install variant of Comet Cast: runs in a browser. Early version (v0.1): a browser
receiver and a browser sender work end to end (screen video over WebRTC, signalling over WebSocket
`/ws`, protocol in `docs/protocol.md`). Without a STUN/TURN list from the server it works over the local
network only and may not work across networks. Servers in open mode require a small proof-of-work
(ALTCHA) before a room is created; the page solves it by itself (see "Open servers and TURN" below). The receiver has a local annotation toolbar, and images and links can be sent over a WebRTC DataChannel
(milestone M2, see `docs/web-m2.md`). A PDF can be sent and is shown page by page on the receiver; it is
rendered by `crates/cc-docs-wasm` (hayro compiled to WebAssembly, in a Web Worker). Build it with
`npm run build:wasm` (needs the Rust target `wasm32-unknown-unknown` and a `wasm-bindgen` CLI of exactly the
version in `Cargo.lock`) **before** `npm run build`; it is written to `public/pdf/` (git-ignored). Without it
the app works as before and a received PDF only gives the notice that documents cannot be shown. Office
documents are not implemented. The rest of the Rust core is not used in the browser yet (see `src/core/index.ts`).
End-to-end test: `npm run build:wasm && npm run build && npm run e2e:pdf`. The Docker image (`deploy/Dockerfile`) does **not** contain the renderer (`public/pdf` is excluded from its build context and the image has no Rust target), so a PDF received by a web app served from that image only gives the "cannot show documents" notice.

Stack: Vue 3, TypeScript, Vite, Tailwind CSS 4, vue-i18n (English and German).

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
- `npm run e2e:m2` - end-to-end test for toolbar, freeze, images and links (needs `npm run build` first)
- `npm run e2e:static` - app on its own static origin, relay on another (builds by itself, see "Static hosting")

## Trying it

```sh
npm run build
cargo run -p cometcast-server -- --listen 127.0.0.1:8443 --web-dir apps/web/dist   # from the repo root
```

Open `http://127.0.0.1:8443` in two tabs/browsers: "Receive on this device" shows a room code; on the
other one choose "Send screen", enter the code and share. For development with `npm run dev`, add
`?server=127.0.0.1:8443` to the URL so the page talks to the running server (works in every build, see below).

## Code layout

- `src/core/signaling.ts` - WebSocket client and typed messages (mirrors `cc-proto`)
- `src/core/relay.ts`, `src/composables/useRelay.ts`, `src/components/RelayField.vue` - relay validation and selection, `config.json`, the relay field
- `src/core/peer.ts` - RTCPeerConnection wrapper (takes the STUN/TURN list from the server; none by default)
- `src/core/altcha.ts`, `altchaWorker.ts`, `altchaClient.ts` - ALTCHA solver (WebCrypto PBKDF2, runs in a Web Worker), `GET /altcha/challenge` and `GET /ice-servers`
- `src/core/receiverSession.ts`, `senderSession.ts` - explicit state machines
- `src/core/control.ts` - DataChannel "control": message format, chunking, reassembly, link validation
- `src/core/annotations.ts`, `toolbarState.ts` - local annotation model and toolbar state (pure, unit-tested)
- `src/composables/useReceiver.ts`, `useSender.ts` - Vue glue

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
relay serve it (`--web-dir`, the Docker image does). The relay (`cometcast-server`) only relays signalling.

**Which relay the page uses**, first match wins (`src/core/relay.ts`, tests in `tests/relay.test.ts`):

1. `?server=` in the page URL, or a relay typed into the relay field of the start page (kept for this tab only,
   `sessionStorage`). Accepts `wss://host[:port][/ws]`, `https://...` as a spelling of it, or a bare `host[:port]`.
   `ws://` is accepted only for localhost, single-label names, `.local`/`.lan` and private/loopback/link-local IP
   addresses (LAN, development). No user info, query, fragment or other path. An invalid value is ignored.
2. `config.json` next to `index.html` (optional; fetched at start with a 1.5 s timeout, no credentials):
   `{ "relays": ["wss://relay.example.org/ws", "wss://second.example.org/ws"] }` (or `{ "relay": "..." }`). Only ws/wss
   URLs, at most 8, invalid entries dropped, first = default, the others show up as buttons. It is parsed as JSON and
   never executed; a missing file, an HTML fallback page, an unusable file or a timeout mean "no config".
   Example: `deploy/config/web-app-config.example.json`.
3. Build-time default `VITE_DEFAULT_RELAY`, e.g. `VITE_DEFAULT_RELAY=wss://relay.cometcast.de/ws npm run build`.
4. The page's own origin (the relay serves the page: Docker image, `--web-dir`, `npm run dev` with `?server=`).

The start page shows the relay in use (host only) in a field with the tag "default" or "custom", lets you enter another
relay (validated, translated errors, German with Sie) and reset it. The send and receive pages show the host read-only
(the relay can only be changed on the start page, because the rooms exist on one relay). **When the relay in use is not
the default (`?server=` or a change in the field) a warning is shown**: a link can point to any relay, and the relay
sees room codes and connection data (not your screen or files). Sender and receiver must use the same relay.

**Base path.** `VITE_BASE=/app/ npm run build` builds for `https://cometcast.de/app/` (default `/`; must be an absolute
path, `./` is rejected because the PDF worker builds its URLs from it). Assets, the two Web Workers, the logo in the
header and `<base>pdf/` (the optional WASM renderer from `npm run build:wasm`) all follow it; there is no service
worker, web manifest, web font or other remote asset (system fonts only). Checked by building with `/app/` and
serving it at that path (`npm run e2e:static`). `config.json` is read from `<base>config.json`.

The relay must allow the page's origin: `allowed_origins = ["https://cometcast.de"]` (docs/server.md, "Web app on
another origin"); otherwise the browser blocks `/ice-servers` and `/altcha/challenge` and `/ws` is refused with
`403` when a list is set. The page then shows "The connection to the server was lost or could not be made".
`npm run e2e:static` (Chrome or Chromium, `E2E_CHROME=...`) builds the app with `VITE_BASE=/app/`, serves it from a
second plain static server, starts relays on other ports, and checks `?server=`, `config.json`, ALTCHA across origins,
video through the relay, and that a third origin is rejected. UNCERTAIN: real GitHub Pages and real DNS were not tried.

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

## Open mode and TURN test

`npm run build && npm run e2e:open` starts real servers (several processes) and checks, with headless Chromium and
the real `cometcast-cli`: a raw client without ALTCHA gets `altcha_required`; the web receiver shows the translated
"Checking your browser..." status (English and German), solves the puzzle in a Web Worker and creates a room on an
open-mode server, a web sender joins and video plays; `cometcast-cli receive` creates a room on the open server and
receives from a web sender; `cometcast-cli send` reaches a web receiver; repeated room creation ends in the translated
`rate_limited` message and failed joins in the translated `banned` message; on an internal-mode server with a TURN
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
(checked by a test). Screen sharing needs a desktop browser and HTTPS (or localhost).

License: GPL-3.0-or-later
