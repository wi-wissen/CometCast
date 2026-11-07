# Measurement points and beta assumptions

## The rule of the project

**We do not test unverified assumptions up front.** What has been observed, or what we have all the information for, counts as working for now. Comet Cast is built as a beta with these observed assumptions, it is used, and where something does not work, the listed fallback ("extension") is added.

This file is the single list of those assumptions, the beta observations that will tell us whether they hold, and the fallbacks. Code and docs link here from every place marked `UNCERTAIN` (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## How to use and extend this file

- Keep the numbering. Assumptions are `A1` to `A18`; new ones get the next free number. Never renumber, so links stay valid.
- To add an assumption, add one row to the table in section 1 with all columns filled: assumption, basis, what we observe in the beta, fallback, status.
- **Status** uses only these words:
  - **sourced**: backed by a source (documentation, source code, vendor statement) that was read.
  - **assumption**: we believe it, but nothing was read or measured to back it.
  - **unchecked**: someone stated it, but it was not verified.
- If you are unsure about a statement, say so in the row. Never write an unsure statement as a fact.
- Effort figures are estimates in person-weeks (PW). Thresholds in the spikes are proposals, not measurements.
- A row with status "no label in source" had no certainty label in the planning document; treat it as an assumption until someone adds a source.

## 1. Beta assumptions A1 to A18

| # | Assumption | Basis | What we observe in the beta | Fallback if it does not work | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | Direct WebRTC between our own apps works | Standard path of every browser; Deskreen does it | Connection does not come up, frozen frames, share of connections that fall back to TURN | Internal server with TURN/SFU, then the receiver's own Wi-Fi | sourced |
| A2 | Flutter UI with Rust core carries all apps | Project decision; flutter_rust_bridge. Google TV: focus handling with the `dpad` package (built: Flutter's own focus traversal instead, see [android.md](android.md)), video via MediaCodec into a SurfaceProducer (built, never run), plus a 32-bit build (armeabi-v7a; builds, never run). Device checklist: [android-device-test-plan.md](android-device-test-plan.md) | Remote control is sluggish or video stutters on weak TV boxes; the share extension does not reach the core | Build only the TV receiver app natively (Kotlin). If the rest does not hold either: native UIs, or Tauri for the desktop | sourced |
| A3 | AirPlay receiving through UxPlay | iOS 26.6 works; iOS 27 with `-h265`; `-vrtp` breaks off (UxPlay issue #465); read the PIN through a PTY | RTP stream breaks off, PIN line is missing, iOS 27 does not connect | UxPlay window without toolbar, hourly password; later our own Rust receiver | sourced |
| A4 | Miracast receiving through MiracastReceiver without MSIX | An unpackaged WPF app runs stably | A session cannot be created, no PIN is shown | MSIX packaging; reserve: WiFiDisplay.dll (AirServer uses it, behaviour on Windows 11 unknown) | sourced |
| A5 | Cast mirroring as a sender to Google's mirroring app (ID `0F5096E8`), using Open Screen sender | Open Screen source code (flow, device auth, media path) read; analysis of AirParrot not checked; Rust rebuild estimated at 9 to 17 PW | `LAUNCH_ERROR` (`NOT_FOUND`, `SYSTEM_ERROR`), no `ANSWER`, latency over 400 ms | Default Media Receiver with HLS (several seconds of delay), then a Custom Web Receiver (US$ 5) | sourced (AirParrot analysis: unchecked; effort: estimate) |
| A6 | AirPlay sender on the desktop | Source code of doubletake read (port 7000, PIN pairing HKP 5, fp-setup); Rust rebuild with FairPlay behind an interface | Pairing (HKP 5, transient) or fp-setup fails on current tvOS | Media-link playback, then the receiver app on the Apple TV. Android and Chromebook to Apple TV: always the receiver app | sourced |
| A7 | Android TV boxes as receivers (brand-name models, 32-bit build) | Google TV requirements on 32/64 bit | Stutter, crashes, a box without app store | Minimum requirements in the docs; warning about cheap boxes | sourced |
| A8 | Server discovery through MDM, DNS-SRV, Android `getDomains`, iOS `lb._dns-sd`, mDNS | Android DnsResolver; RFC 6763 section 11 | The receiver does not show up in the list | Code or QR code on the receiver | sourced |
| A9 | Media links with yt-dlp (`--xff never`); updater only outside Google Play and the Mac App Store | yt-dlp | Hit rate on the test corpus, error messages | Direct link, or hand over to the app | sourced |
| A10 | Efficient image transfer: skip unchanged areas, static refresh, mouse pointer sent separately | RustDesk source code | CPU load, blurry text, pointer lags | Continuous video at a fixed bitrate | sourced |
| A11 | Hardware encoders | System encoders; RustDesk pattern with a test in a child process | Encoder test fails, image errors | Software encoder openh264 | no label in source (openh264 fallback: sourced) |
| A12 | Open server with overload protection (ALTCHA, room codes). The numbers are assumptions | RustDesk botnet wave of 2026 | Load, failed attempts, solve time on an old device | Adjust the limits | sourced (numbers: assumption) |
| A13 | Native tvOS receiver, no background operation | AppLock | App leaves the foreground, reception ends | Single-app mode through MDM; emergency fallback: browser receiver on another device | sourced |
| A14 | Browser receiver on smart TVs | Statements on Tizen and webOS (see the planning page, "Wie wenig App") | Live image breaks off | Slides and images only | no label in source |
| A15 | Cast receiving for Chrome with a developer certificate | Open Screen | Not in the beta | Later optional stage | sourced |
| A16 | AirPlay-1-style mirroring to current tvOS | AirParrot and AirServer announce themselves as `AppleTV3,2`; doubletake uses the newer path through port 7000 | Current tvOS refuses | Follow the flow of doubletake (port 7000, type 110) and pyatv | unchecked |
| A17 | A browser client is enough as the app-free path on the desktop | `getDisplayMedia` in desktop browsers; see the web app | Latency in the browser, behaviour on Chromebook and smart-TV browsers | Offer only the receiver in the browser; send through the app | sourced |
| A18 | The connection check code (hash of both DTLS fingerprints, shown on sender and receiver) reveals a relay that swapped fingerprints, and people compare it | Standard short-authentication-string idea (ZRTP); relay carries the signalling | Whether the code ever differs for honest connections (bug), whether people compare it at all | Stronger: a commitment step in the protocol, or fingerprints given out of band (QR) | unchecked: tested with fake peers and an honest relay only; a swapping relay was never built; 6 digits can in principle be ground |

## 2. The ten measurement points

These are measurements made during the beta. They are not preconditions for starting.

> **Scope note on points 6 and 9.** Some measurement steps in the planning page involve third-party products (AirServer, AirParrot, Wireshark captures, certificate chains). Under this project's rules we do not extract keys or certificates and we do not take over AirServer, AirParrot, FairPlay or Cast material. Such steps are private research outside this repository: no extracted artefacts or captures are committed, and the certificate and AirServer parts need the maintainer's explicit sign-off first. Until then they count as "not done in this project".


| # | Topic | What is measured | Next step | Assumption rows |
| --- | --- | --- | --- | --- |
| 1 | Flutter with Rust core | Spike 0: share extension on iOS/Android down to the core, video surface at 1080p30, operation on Google TV. Google TV Streamer is the yardstick, a 30 EUR box is the lower limit | Build the spike. If it fails: "Alternative A" (Tauri with a Vue UI; see the planning page, section "Oberfläche") | A2 |
| 2 | Desktop capture and WebRTC quality | Spikes 1 and 2: native capture on Windows, macOS, Linux (Wayland); 10 min of 1080p on lossy Wi-Fi: latency, CPU, frozen frames | | A1, A10, A11 |
| 3 | Apple TV, iPhone and iPad | Spike 3: core for tvOS (crypto), WebRTC receiving, hayro, comparison with LiveKitWebRTC. Spikes 4 and 5: memory of the broadcast extension, Safari 27 on the iPad | Mini app on a real Apple TV; 10 min of sending from a large iPad | A13 |
| 4 | Google TV and Linux mini PC as always-on receivers | Spikes 6 and 8: autostart, discovery in the background, one hour stable; `dpad` focus and 32-bit build on the Google TV Streamer; Apple TV in AppLock operation; confirm minimum requirements for Android TV boxes | Get the test devices | A7 |
| 5 | Third-party receiving on the mini PC | Miracast: minimal app with `CreateSession(None)` unpackaged on Windows 11, who shows the PIN, the "Wireless Display" feature, MS-MICE, firewall, Wi-Fi chips. UxPlay: iPadOS 27 with `-h265`, endurance test of `-vrtp` (issue #465), PIN line through PTY, uxplay-windows against our own MSYS2 build | Minimal app with the `windows` crate, two mini-PC models, real iPads | A3, A4 |
| 6 | Network on site and AirServer cross-check | mDNS forwarding, open ports between VLANs, split-DNS entry, SRV/domain search on Android and Apple, TURN exception for our own relay IP; cross-check the port list against a capture. AirServer chain: AuthChallenge on port 8009 (pychromecast or rust_cast), show `client_auth_certificate` and `intermediate_certificate` with `openssl x509` (issuer, subject, validity); check a test PC before/after for new root certificates and start parameters of the browser process. The mirroring negotiation can be read in the Open Screen source, so no capture is needed for it | Information from IT, test with `cometcast-cli`, one Windows PC with AirServer | A8, A1 |
| 7 | Content test corpus | 20 to 30 video links (hit rate, YouTube hand-over on tvOS and Google TV, IP binding), 20 PDFs (hayro, spike 7), 5 Office files (office2pdf) | Assemble a corpus from real classroom files | A9 |
| 8 | Load | Open server: brokering under bot load, ALTCHA solve time on the oldest device. Internal server: TURN with ten parallel streams, image quotas | Load test on a test server | A12 |
| 9 | AirPlay sender path | Read the doubletake code (FairPlay SAP, pairing); test on a real Apple TV with current tvOS; Wireshark capture AirParrot to Apple TV: port of the mirroring connection, fp-setup in the mirroring path, latency, resolution, where the pairing is stored | doubletake on Linux against our own Apple TV, AirParrot trial licence on Windows | A6, A16 |
| 10 | Cast mirroring, variant B | Spike 9 (high priority): `cast_sender` against `0F5096E8` on a Chromecast with Google TV, an older Chromecast and a TV with Cast; the block for third-party senders; H.264 Annex B or AVCC; AES-CTR counter overflow; heartbeat requirement; latency and resolution. If it fails: Default Media Receiver with HLS, or a Custom Web Receiver | Build Open Screen on Linux, one day at the device | A5 |

Low priority and not on the list: the official Google manufacturer programme for Cast receiving. The way is not publicly documented and can only be clarified by asking Google.

## 3. Measurement spikes

Spike 0 is the UI spike; spikes 1 to 9 are prototypes. They are no longer a run-up before the beta but measurements made during it. Success criteria and effort are proposals and estimates, not measurements.

| # | Question | Success criterion | Effort (estimate) |
| --- | --- | --- | --- |
| 0 | **Flutter with Rust core.** Does the chosen direction hold? One Flutter app (pick receiver, send, receive mode with a queue) with flutter_rust_bridge on Windows, an Android tablet, an iPad and Google TV; plus a share extension on iOS and a share intent on Android down to the core; plus a video surface that shows the stream decoded by the core | One code base runs on all four devices; a shared file arrives in the core; Google TV is usable with the remote; video at 1080p30 without visible stutter. Yardstick: Google TV Streamer or Chromecast with Google TV, a 30 EUR box as lower limit. Otherwise "Alternative A" | 1 to 2 PW |
| 1 | **Desktop capture.** Does native capture in Rust (windows-capture, screencapturekit, ashpd) with a hardware encoder hold on all three systems? | 1080p30 to the receiver side, under 200 ms delay, under 25 % CPU on a mid-range laptop, Wayland dialog once per session | 1 to 2 PW |
| 2 | **WebRTC stack.** Are congestion control and jitter buffer of webrtc-rs 0.21 enough, or does the sender need libwebrtc (LiveKit crate)? | 1080p screen, 10 min, Wi-Fi with loss: no frozen frame over 1 s, stable delay | 1 to 2 PW |
| 3 | **tvOS receiver.** Does the core build for `aarch64-apple-tvos` (including crypto) and receive through UniFFI + WebRTC + VideoToolbox? Or LiveKitWebRTC? | Stream from the browser plays on a real Apple TV, delay and memory measured; a PDF page with hayro in under 1 s | 1 to 2 PW |
| 4 | **iOS broadcast extension.** Do core + WebRTC + VideoToolbox fit the memory limit (about 50 MB, developer statement), or only encode and pass to the app through an app group? | 10 min of sending on a large iPad without a crash, peak memory under 40 MB | 1 PW. **Built to the first option (extension does the network), not measured**: [ios.md](ios.md), tests M1 and M2 in [ios-device-test-plan.md](ios-device-test-plan.md). "Pass to the app" only works while the app is in the foreground, so it is no real fallback |
| 5 | **Safari 27 on the iPad.** Does Safari on iPadOS 27 share the screen? Whole screen or only apps, abort on app switch? | System picker opens, video track arrives, behaviour on app switch documented | 1 to 2 days |
| 6 | **Google TV.** Is the Flutter app suitable as an always-on receiver: autostart, discovery in the background, decoding through MediaCodec? (Operation and video surface are already checked by spike 0) | Google TV Streamer receives after a restart without intervention, 1080p30 stable for an hour | 2 to 3 days |
| 7 | **hayro as WASM.** How big is the WASM file, how fast and how faithfully does hayro render real classroom PDFs on weak mini PCs and the Apple TV? | 20 real PDFs, no visible errors in 18, page in under 500 ms | 2 to 3 days |
| 8 | **Linux receiver.** Does the Flutter app run as an always-on receiver on a Linux mini PC (autostart, full screen)? Fallback without installation: Chromium kiosk with the web receiver page | Mini PC starts into receiving without intervention and is found through DNS-SD | 3 to 5 days |
| 9 | **Cast mirroring, high priority.** Does a real Chromecast or Google TV let a third-party sender into Google's mirroring app `0F5096E8` today? `cast_sender` of the Open Screen library against a Chromecast with Google TV, an older Chromecast and a TV with Cast | Image arrives: Rust rebuild. Otherwise Default Media Receiver with HLS or a Custom Web Receiver. Note `LAUNCH_ERROR`; measure latency and resolution | 1 day |

## 4. Related effort estimates (all estimates)

- Cast mirroring sender (milestone M4): 9 to 14 PW, with buffer 12 to 17 PW.
- AirPlay sender for the desktop (milestone M7): 12 to 25 PW; not deliverable without the FairPlay part.
- Native desktop capture 6 to 12 PW, broadcast extension 3 to 6 PW, tvOS 3 to 6 PW were calculated for "Alternative A" (Tauri with a Vue UI), not for Flutter. A Flutter estimate is still missing and is to be added after spike 0.

## 5. Things that do not work (and what we offer instead)

These limits cannot be programmed away. Comet Cast explains them and offers a replacement. Original certainty labels are kept.

| Limit | Replacement | Status |
| --- | --- | --- |
| Android Chrome does not support `getDisplayMedia` | Android app, or send content | sourced |
| Browser screen sharing on iPhone and iPad: not possible before iOS 27. Safari on iOS 27 can do it according to MDN, tested on iPhone; iPad not checked | App with broadcast extension for older versions | sourced (iPad: unchecked) |
| tvOS has no WKWebView, so no browser receiver on Apple TV | Native app only | sourced |
| Cast mirroring to a Comet Cast receiver: Chrome and Android mirror only to devices with a Google-signed certificate | Send media links to existing Cast devices; our own app on Google TV | sourced |
| AirPlay mirroring from Android and Chromebook to Apple TV: no sender found; AirParrot is Windows and macOS only | Windows and Linux may get an optional, later desktop sender (template doubletake, M7). Android: tvOS app | sourced |
| Miracast sending from our own app: Windows casting sends only media elements; Android removed Miracast | Point to the system dialog | sourced |
| Live image in the browser of smart TVs: WebRTC only safe from Tizen 10; on LG only for partner apps | Slides and images | sourced |
| Miracast receiving on Apple TV, Google TV, macOS: only Windows has a receiver interface | none | Android/macOS: unchecked |
| Wi-Fi Aware on Windows and Linux: Windows has no interface (community answer); no evidence found for Linux | none | Windows: sourced; Linux: unchecked |
| Direct connection without a network between Windows/Linux and Apple TV: AWDL is Apple-internal, Wi-Fi Aware is missing on the desktop, Apple TV cannot advertise over Bluetooth, only scan | none | no label in source |
| Live image over Bluetooth: about 50 KB/s is enough for control commands and finding the room, not for video | none | no label in source |
| New Chromecast dongles: sales ended. Successor is Google TV Streamer; Cast in TVs stays | none | sourced |
