# Architecture

Everything that is not UI is written once in the Rust core. Three bindings bring the core to all platforms. On top sits one Flutter app with both roles (sender and receiver). The Apple TV receiver and the browser page stay small and separate. Native code is written only where the platform forces it.

Statements that rest on an assumption are marked **Uncertain** and linked to [measurement-points.md](measurement-points.md).

## Layers

```
Rust core (Cargo workspace, crates/cc-*)
   |-- flutter_rust_bridge --> Flutter app (desktop, iOS, iPadOS, Android, Android TV / Google TV)
   |-- UniFFI (Swift package "CometKit") --> Apple TV receiver, iOS extensions
   |-- wasm-bindgen --> web app (only where needed, for example PDF)
Server: cometcast-server (axum) + coturn (separate process) + optional SFU
```

### Core crates

| Crate | Responsibility |
| --- | --- |
| `cc-proto` | Messages, IDs, capabilities, control commands, protocol versioning. Builds for all targets including WASM |
| `cc-discovery` | Find servers (MDM, SRV/DNS-SD, code/QR, default list) and LAN discovery |
| `cc-transport` | WebRTC session, route ladder, ICE configuration, data channel |
| `cc-media` | Encoder/decoder traits, openh264 fallback |
| `cc-capture` | `ScreenSource` trait and capture backends per desktop system |
| `cc-docs` | Render PDF pages, create and validate WebP, notes |
| `cc-office` | Office to PDF on the sender |
| `cc-files` | File transfer, certificate pinning |
| `cc-sender` | Sender core, presenter-view state |
| `cc-receiver` | Accepting content, queue, toolbar, deletion rule |
| `cc-ffi` | Thin, stable facade: flutter_rust_bridge for Flutter, UniFFI for tvOS and extensions |
| `cc-server` | Signalling, rooms, image storage, security, TURN credentials |
| `cc-sfu` | Optional 1-to-n forwarding (str0m) |

Binaries: `cometcast-server`, `cometcast-cli`. Build helpers: `xtask/`.

### Platform apps

| Platform | UI | Native parts |
| --- | --- | --- |
| Windows, macOS, Linux | Flutter, both roles | Capture runs in the Rust core |
| iPhone, iPad | Flutter, both roles | Share extension and broadcast extension (Swift) |
| Android, Google TV | Flutter, both roles | Share intent, MediaProjection service, MediaCodec (Kotlin) |
| Apple TV | SwiftUI, receiver only | Decoding with VideoToolbox. No background operation; always-on use needs the app in the foreground with AppLock through MDM |
| Browser | Vue 3 + TypeScript + Vite + Rust WASM | None. WebRTC and screen sharing come from the browser. No mDNS: room code or QR |

**Uncertain:** that Flutter carries the TV receivers (remote control, video on weak boxes). Fallback: native Kotlin receiver. See [A2](measurement-points.md#1-beta-assumptions-a1-to-a17).

### Server

- `cometcast-server` does brokering only: signalling and rooms. The open server stores no images and no files.
- coturn runs as a separate process for TURN.
- `cc-sfu` is optional.
- Two modes: open and internal. Overload protection with ALTCHA (always on for the open server, optional for the internal one, adaptive difficulty). The numbers are assumptions, see [A12](measurement-points.md#1-beta-assumptions-a1-to-a17).

## Route ladder

1. Direct WebRTC between the two apps.
2. Internal server with TURN (or SFU).
3. The receiver's own Wi-Fi.

Measured and extended during the beta ([A1](measurement-points.md#1-beta-assumptions-a1-to-a17)).

## Server discovery

1. MDM policy
2. DNS-SRV `_cometcast._tcp`
3. Announcement from the display device
4. Built-in default list

Details, trust rules and what is unverified: [discovery.md](discovery.md).

mDNS runs in parallel. Backends: `mdns-sd` on desktop, NWBrowser on Apple platforms, NsdManager on Android. **Uncertain:** parts of this are unchecked, see [A8](measurement-points.md#1-beta-assumptions-a1-to-a17).

## Decisions

- **UI:** Flutter with the Rust core, one app with both roles.
- **Apple TV:** native SwiftUI, receiver only (tvOS has no WebView and no official Flutter).
- **Web app:** Vue 3 + TypeScript + Rust WASM; small and without installation; Flutter Web would be several MB larger.
- **Licence:** GPL-3.0-or-later with a CLA for third-party contributions.
- **Language:** English in the repository; bilingual UI (de/en).
- **HTTPS on the local network:** public domain with split DNS as default; own CA through MDM for sites without internet.
- **Discovery on Android:** system NsdManager instead of own multicast.
- **Open server:** brokering only, no images, no files, no storage.
- **AirPlay receiving:** UxPlay as a separate process.
- **Media links:** page analysis in the core, yt-dlp with `--xff never` on desktop and mini PC; mobile devices send only the URL.
- **Google Cast:** sender yes (mirroring sender if spike 9 succeeds); no Cast receiving in v1.
- **Not pursued:** Wi-Fi Aware in v1, AirDrop, DLNA/UPnP, Miracast sender, LocalSend compatibility in the first version.
- **Version:** first release is 0.1.

The full list with reasons is in the planning document (German, kept outside this repository).

## Milestones

- [x] **M0** Project start: repo, translation scaffold (de/en), LICENSE (GPL-3.0), notices for third-party components, CLA text, source file headers. *Done. The CLA text is still a draft that needs a lawyer's review.*
- [ ] **M1** Own protocol: desktop to desktop and Android TV, direct and with room code. *Largely done on Linux: server, protocol, browser and native peers, X11 screen capture with H.264, Flutter Linux app, an Android debug APK builds. Not done: capture on Wayland/Windows/macOS, Android TV device test, STUN/TURN across real networks.*
- [ ] **M2** Toolbar and content mode. *Partly done in the web app: toolbar, annotations, freeze, image and link transfer, and PDF (the web receiver shows PDFs page by page with hayro compiled to WASM in a Web Worker; the web sender sends a PDF and turns pages with the `cmd` messages; checked with Chromium only, see [web-m2.md](web-m2.md) and [documents.md](documents.md)). Not done: Office documents, media links, toolbar in the Flutter apps, PDF fidelity and speed on weak devices (measurement point 7), a Docker image that contains the WASM renderer.*
- [x] **M2b** Web app: sender and receiver in the browser. *Done for screen sharing over WebRTC, local network only until TURN is used by the clients.*
- [ ] **M3** AirPlay receiving (UxPlay) on mini PC and Windows. *Partly done: the supervisor library and CLI (`cc-airplay`), tested on Linux with a fake and the real UxPlay 1.68. Not done: app integration, a real iPhone/iPad test, Windows.*
- [ ] **M4** Cast mirroring sender (Rust, after the public BSD-licensed Open Screen library). *Built as a beta: discovery, Cast channel, device authentication check, launch of the mirroring app, OFFER/ANSWER, encrypted RTP. Tested only against our own fake receiver on loopback, never against a real Chromecast (measurement point A5).*
- [ ] **M5** Miracast receiving on Windows. *Partly done: `cc-miracast` (policy, status, Windows backend over the documented WinRT API) is compile-checked only and has never run on Windows. No video surface in the app.*
- [ ] **M6** iOS broadcast extension and Android capture. *Android: capture service, encoder, and the path into the Rust sender (`push_encoded_frame`) are built and tested up to the APK build, but nothing ran on a device. iOS not started.*
- [ ] **M7** AirPlay sender for the desktop, without FairPlay ("M7 light"). *Built: discovery, pairing (legacy PIN and HomeKit style), media link, photo, and the screen as a live HLS stream. Tested against the real UxPlay for discovery and legacy PIN pairing only (UxPlay does not play links); everything else only against our own fake. Nothing tested on an Apple TV. Real mirroring (FairPlay) is deliberately not built.*
- [ ] **M8** Internal and open server. *Server side built and tested (TLS, limits, bans, ALTCHA, TURN credentials). Not built or run: Docker image, coturn. The default host `relay.cometcast.de` does not exist yet.*

Effort figures are estimates. Order of use is given by the milestones; the first content of 0.1 is M1 to M3.
