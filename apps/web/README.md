# Comet Cast web app

The no-install variant of Comet Cast: runs in a browser. Early version (v0.1): a browser
receiver and a browser sender work end to end (screen video over WebRTC, signalling over WebSocket
`/ws`, protocol in `docs/protocol.md`). There are no STUN/TURN servers yet, so it works over the local
network only and may not work across networks. Sending files and links (M2) is not implemented. The
Rust-WASM core will be added later (see `src/core/index.ts`).

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

## Trying it

```sh
npm run build
cargo run -p cometcast-server -- --listen 127.0.0.1:8443 --web-dir apps/web/dist   # from the repo root
```

Open `http://127.0.0.1:8443` in two tabs/browsers: "Receive on this device" shows a room code; on the
other one choose "Send screen", enter the code and share. For development with `npm run dev`, add
`?server=127.0.0.1:8443` to the URL (development builds only; production builds ignore it) so the page talks to the running server.

## Code layout

- `src/core/signaling.ts` - WebSocket client and typed messages (mirrors `cc-proto`)
- `src/core/peer.ts` - RTCPeerConnection wrapper (no STUN/TURN by default)
- `src/core/receiverSession.ts`, `senderSession.ts` - explicit state machines
- `src/composables/useReceiver.ts`, `useSender.ts` - Vue glue

## End-to-end test

`npm run build && npm run e2e` builds and starts the real `cometcast-server` on a random local port,
opens a receiver page and a sender page in headless Chromium (Playwright), joins with the shown code,
and checks that the receiver's `<video>` has a size and its `currentTime` advances, then that
stopping on the sender shows "peer left" on the receiver. It also checks the translated
`room_not_found` error for a wrong code. Needs a Rust toolchain and Playwright's Chromium
(`npx playwright install chromium`).

What it proves: signalling, offer/answer/ICE and media over one machine's loopback/host candidates.
What it does not prove: behaviour across networks/NAT (no STUN/TURN), other browsers, real screen
content, audio, or encryption on the wire (no TLS yet). It uses the launch flag
`--disable-features=WebRtcHideLocalIpsWithMdns` so that host candidates are usable in a sandbox.

getDisplayMedia worked headless in the preinstalled Chromium with
`--use-fake-ui-for-media-stream --auto-select-desktop-capture-source=Entire screen`
(UNCERTAIN: observed only in this one environment; the captured source there is whatever the headless
browser offers, not a real desktop). As a fallback there is a **TEST-ONLY** query flag `?testVideo=1`
on the page: the sender then sends an animated canvas instead of the screen. Run
`E2E_TEST_VIDEO=1 npm run e2e` to use it. Do not use it for anything else.

## Translations

UI strings live in `src/locales/en.json` and `src/locales/de.json`. Both files must have identical keys
(checked by a test). Screen sharing needs a desktop browser and HTTPS (or localhost).

License: GPL-3.0-or-later
