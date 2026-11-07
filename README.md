<p align="center">
  <img src="assets/logo/cometcast-logo-light.svg" alt="Comet Cast logo" width="320">
</p>

# Comet Cast

Comet Cast is one app that is both sender and receiver. It sends your screen, documents, links and media, and files to Apple TV, Google TV / Android TV, mini PCs and browsers. It is meant for offices, meetings, clubs, workshops and schools, where many different devices meet in one room. The idea is similar to LocalSend, but for presenting on a display.

> **Status: v0.1 beta, in development.**
> Built and tested on one machine or one local network (unit tests, browser end-to-end tests, screenshots):
> - Web app (sender and display): screen, documents, links and files over WebRTC with a room code or QR code, three join modes chosen at the display (open, with approval, locked), a display toolbar (pen, marker, freeze, laser, black screen), dark, light and OLED themes, German (informal "du" by default) and English.
> - Relay and signalling server: TLS, rate limits, ALTCHA, STUN list, join approval. A Docker image build exists and ran locally. The operating package is in [CometCast-Relay](https://github.com/wi-wissen/CometCast-Relay).
> - Rust core with native peers and command-line tool, Flutter apps (desktop, Android, Android TV, iOS) with first-run, display, presenter, nearby screens and share entries, and an Apple TV app written in SwiftUI. Capture backends for Wayland, Windows and macOS, AirPlay receiving through UxPlay, Miracast and Cast/AirPlay sending exist as code written to specification.
>
> **Never run on a real device or service:** the native apps on Android, Android TV, iOS, Windows, macOS and Apple TV (Kotlin and Swift were never compiled), screen capture on those systems, AirPlay with an iPhone, Miracast, Cast, the iOS broadcast extension, STUN across real networks, the relay on a public server, and every GitHub release and image workflow (the check workflows are manual only, see [docs/ci.md](docs/ci.md)). No release or Docker image is published yet. Everything not listed above is a plan. See [docs/architecture.md](docs/architecture.md) for the milestone status and [docs/measurement-points.md](docs/measurement-points.md) for the unverified assumptions.

## Principles

- **As few app installs as possible, ideally on one side only.** The side with fewer devices gets the app. The other side can use a browser or a system feature it already has.
- **No up-front testing of unverified assumptions.** We build a beta on observed assumptions and add the fallback where something fails. All assumptions are listed in [docs/measurement-points.md](docs/measurement-points.md).
- **Say what we do not know.** Unsure statements are marked as such, in code and in docs.

## How devices connect

Transport is tried in this order:

1. Direct WebRTC between the two devices.
2. If that fails: the organisation's internal server with TURN (and optionally an SFU).
3. If that fails: the receiver's own Wi-Fi.

Devices find each other with a **room code** (or a QR code). The app finds a Comet Cast server in this order:

1. MDM policy
2. DNS-SRV record `_cometcast._tcp`
3. Announcement from the display device
4. Built-in default server list

mDNS runs in parallel for local discovery.

Details: [docs/architecture.md](docs/architecture.md).

## Repository layout

| Path | Purpose | Exists now |
| --- | --- | --- |
| `crates/` | Rust core (`cc-*` crates), written once | yes |
| `bins/` | Binaries: `cometcast-server`, `cometcast-cli` | yes |
| `apps/flutter` | Flutter app for desktop, mobile and Android TV / Google TV | yes |
| `apps/apple-tv` | Native SwiftUI receiver for Apple TV | yes |
| `apps/web` | Web app (Vue 3, TypeScript, Vite), plain static files that can be hosted anywhere (GitHub Pages, any web server) and talk to a relay chosen by `?server=`, `config.json`, a build-time default or the page's own origin ([apps/web/README.md](apps/web/README.md#static-hosting-and-relay-selection)); the relay server's Docker image still bundles it. PDF pages are rendered by `crates/cc-docs-wasm` (hayro as WASM, built separately with `npm run build:wasm`); the rest of the Rust core is not used in the browser yet | yes |
| `assets/` | Logo and other assets | yes |
| `docs/` | Documentation, see [docs/README.md](docs/README.md) | yes |
| `deploy/` | Docker, Compose and coturn files for the server, see [deploy/README.md](deploy/README.md) | yes |
| `site/` | The static website for cometcast.de, see [docs/website.md](docs/website.md) | yes |
| `tools/` | Helper scripts, for example the translation check | yes |
| `xtask/` | Build helpers | yes |

"Exists now" means the folder is part of the repository, not that it is finished.

## Technology

| Area | Choice |
| --- | --- |
| Core | Rust workspace of `cc-*` crates: protocol, discovery, WebRTC transport, capture, documents, files, sender and receiver logic |
| Bindings | flutter_rust_bridge for Flutter (in use). wasm-bindgen is in use for the web app's PDF renderer (`cc-docs-wasm`). UniFFI for Swift is planned, not built yet (the iOS broadcast extension uses the C ABI of `cc-ios`) |
| Apps | Flutter for Windows, macOS, Linux, Android, iOS, iPadOS and Android TV / Google TV |
| Apple TV | Native SwiftUI receiver. If Flutter on TV does not hold up in the beta, a native Kotlin receiver is the fallback for Android TV |
| Web app | Vue 3 + TypeScript + Vite (the PDF renderer is Rust compiled to WASM; the rest of the core is not used in the browser). Screen sharing is expected only in desktop browsers and needs HTTPS (assumption A17; mobile support is unchecked) |
| Server | `cometcast-server` (axum), coturn as a separate process, optional SFU |

Native code is written only where the platform forces it (for example share and broadcast extensions, screen capture on Android).

## What we deliberately do not do

- No extraction of keys or certificates.
- No FairPlay, Google Cast, AirServer or AirParrot material (no leaked code, keys or extracted files).
- No DRM circumvention.
- No Cast receiving in v1.
- No AirDrop.
- No DLNA / UPnP.
- No Miracast sender (we point to the system dialog instead).

Known platform limits and the replacement we offer are in [docs/measurement-points.md](docs/measurement-points.md#5-things-that-do-not-work-and-what-we-offer-instead).

## Build and run

Rust workspace:

```sh
cargo build --workspace
```

```sh
cargo test --workspace
```

Web app:

```sh
cd apps/web && npm ci && npm run dev
```

Native peers (developer CLI, M1). Start a server, then receive in one terminal and send in another:

```sh
cargo run -p cometcast-server -- --listen 127.0.0.1:8443
cargo run -p cometcast-cli -- receive            # prints the room code, then packet statistics
cargo run -p cometcast-cli -- send <CODE> --seconds 10   # sends a built-in VP8 test pattern
```

The browser app interoperates with them: `cd apps/web && npm run build && npm run e2e:native`.
Details and limits: [docs/transport.md](docs/transport.md).

Running a server: [bins/cometcast-server/README.md](bins/cometcast-server/README.md) (settings, modes, troubleshooting) and [deploy/README.md](deploy/README.md) (Docker Compose, reverse proxy, coturn, firewall).

More commands will be added here when the things they build exist.

Server image without a release: the manual workflow `.github/workflows/docker-image.yml` pushes `ghcr.io/<owner>/cometcast-server:edge` ([docs/ci.md](docs/ci.md)).

Releases: the GitHub workflow `.github/workflows/release.yml` builds all apps from a version number and creates a draft release. **UNCERTAIN: it has never run on GitHub; every job in it is unverified until its first real run.** See [docs/releasing.md](docs/releasing.md) (inputs, secrets, unsigned files, dry run).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) first. Contributions need agreement to the [CLA](CLA.md). Please follow the [Code of Conduct](CODE_OF_CONDUCT.md). Security issues: see [SECURITY.md](SECURITY.md).

## Licence

Comet Cast is licensed under **GPL-3.0-or-later**, see [LICENSE](LICENSE). Contributions from third parties are accepted under the [CLA](CLA.md), which allows the steward, Schulverwalter GmbH, to distribute the apps in app stores. The public version of the project stays GPL-3.0-or-later.

The GPL covers the code. It does not by itself grant rights to the name "Comet Cast" or the logo in `assets/logo`; a naming and logo policy is not written yet. Third-party components: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

This section is an overview, not legal advice.
