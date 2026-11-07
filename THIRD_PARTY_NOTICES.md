# Third-party notices

This file is the hand-written overview: what Comet Cast uses, how it reaches the user (compiled in, shipped next to the app, or downloaded by the user's device at run time) and the special terms that need more than a licence name. The **generated, complete list** of Rust crates, npm and Flutter packages with their licence texts is [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (made by `tools/gen-third-party.mjs` from `Cargo.lock`, `apps/web/package-lock.json` and `apps/flutter/pubspec.lock`; a release is refused when it is missing or older than the lock files, see `docs/releasing.md`). Every release artefact carries both files and `LICENSE`.

**What the releases contain from third parties.** Rust crates (compiled into the binaries, see the generated list), the Flutter and web packages (generated list), the fonts of the Office converter (Liberation, Carlito, Caladea, see below and `crates/cc-office/fonts/FONTS.md`), the fonts and data embedded by `hayro` and `typst-assets`, and, in the Apple TV app, the LiveKit WebRTC xcframework. **Not part of any release and not redistributed:** Cisco's OpenH264 binary (downloaded by the user's device from Cisco), UxPlay, yt-dlp and coturn (separate programs a deployment installs itself). Rows below that say "planned" are not shipped yet.

Licence details marked **unchecked** were not verified and must be checked against the licence text before release. The generated list reads the licence files from the crate sources; 62 crates (among them the Typst crates, `rtc-*`, `openh264`, `office2pdf`, `krilla`) ship no licence file in their crates.io source, so only their declared SPDX expression is listed for them: the texts have to be taken from the upstream repositories before a release (open item).

| Component | Planned use | Licence | Status | Notes |
| --- | --- | --- | --- | --- |
| UxPlay | AirPlay receiving, run as a separate companion process on Windows, Linux and macOS. Not linked, not copied. `crates/cc-airplay` only starts and watches the program | GPL-3.0 (the Ubuntu package 1.68.2 lists parts as LGPL-2.1-or-later and MIT, from its copyright file) | supervisor code in use; **no UxPlay binary or source is bundled** | When a release ships UxPlay: ship the GPL-3.0 text, an offer of the matching source and the notices of its bundled parts. Legal classification of the FairPlay handshake is not finally settled, so this stays an optional, separate module. Details and what was tested: `docs/airplay.md`. Legal reading: unchecked. The supervisor was built and tested on Linux only; its signal and process-group handling is Unix-only code, and Windows and macOS are unverified (see `docs/airplay.md`) |
| yt-dlp | Media-link resolving on desktop and mini PC, run as a separate process, always with `--xff never` | Unlicense (GPLv3+ for the single-file build, as stated in the plan) | planned | Ship the licence text. Updater only outside Google Play and the Mac App Store. iOS apps do not resolve links themselves (App Store rule 5.2.3). Exact licence details: unchecked |
| openh264 (`openh264` crate 0.9.9 and Cisco's OpenH264 2.6.0) | Software H.264 encoder (screen sender) and a test-only decoder, `cc-media`. Two build modes: compiled from the source inside the crate (`h264-source`, development and CI only), or Cisco's prebuilt library loaded at run time (`h264-libloading`, releases). In releases the library is **not part of this project's files**: the user's device downloads it from Cisco (`https://ciscobinary.openh264.org/`) after the user agreed, verifies a pinned SHA-256 and stores it in the user's own data folder (`cc-codec`, docs/capture.md "Runtime codec") | BSD-2-Clause (crate and OpenH264, as stated in the crate metadata). The OpenH264 licence text is in Cisco's repository (https://github.com/cisco/openh264, file `LICENSE`); its text is reproduced in the section "OpenH264 notice" below (read from `https://raw.githubusercontent.com/cisco/openh264/master/LICENSE` on 2026-11-07). It must accompany anything of Cisco's that this project would distribute; today nothing of Cisco's is distributed | in use (compiled in for development; releases download Cisco's binary, nothing of Cisco's is bundled or redistributed by this project) | **AVC/H.264 patent statement:** Cisco's OpenH264 binary is licensed by Cisco under the AVC/H.264 patent pool terms Cisco pays for, for the binary that Cisco builds and distributes itself; the user's device gets it directly from Cisco, and this project does not redistribute it. That is why releases never build OpenH264 from source. **Uncertain:** not checked by a lawyer; see docs/capture.md, section "H.264 licensing" |
| cc-codec dependencies: ureq 3 (HTTPS client), rustls 0.23 and webpki-roots 1 (certificate roots), bzip2-rs 0.1 (pure-Rust bzip2 decoder), dirs 6 (per-user folders), sha2 0.10 | Download, decompression, verification and storage of Cisco's OpenH264 library (`cc-codec`) | ureq MIT OR Apache-2.0; bzip2-rs MIT OR Apache-2.0; dirs MIT OR Apache-2.0; sha2 MIT OR Apache-2.0; webpki-roots CDLA-Permissive-2.0 (root certificate data, Mozilla's list); rustls see the ISC row (licences from the crate metadata; the CDLA-Permissive-2.0 text of webpki-roots (it covers Mozilla's CA list; the licence asks that the text stays with redistributed copies of the data) is in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md); the other texts are there too) | in use (compiled into `cc-ffi` with `h264-libloading`) | |
| x11rb 0.14 | X11 screen capture in pure Rust (`cc-capture`) | MIT OR Apache-2.0 (as stated on crates.io; unchecked against the licence texts) | in use (compiled in) | |
| coturn | TURN server, separate process | BSD-style | planned | **Uncertain:** the plan says "BSD-like"; check the exact licence text before release and ship the licence file |
| Open Screen Library (commit `8b62b21479f1adb7663f182d97962b960c5c64ef`, chromium.googlesource.com/openscreen) | Specification reference for the Chromecast / Google Cast compatible sender `cc-cast`: message formats, field names, constants, framing, RTP/RTCP layout, AES-CTR nonce, device-auth steps. The code of `cc-cast` is written in Rust from reading these files; it is an adaptation, not a copy. Files read are listed in `docs/cast.md` | BSD-3-Clause, Copyright 2018 and later The Chromium Authors (text below) | in use as reference; adapted parts are marked in the `cc-cast` module headers | The notice below must stay with source and binary distributions of `cc-cast`. No Open Screen source file and none of its trust-anchor data is copied or embedded. UNCERTAIN: whether further per-file notices are needed was not checked with a lawyer |
| prost 0.14, mdns-sd 0.21, aes 0.8, ctr 0.9, ring 0.17, x509-parser 0.18 | `cc-cast`: protobuf derive, mDNS browsing, AES-128-CTR, RSA signature check, certificate parsing | Apache-2.0 / MIT OR Apache-2.0 / Apache-2.0 AND ISC (as stated in the crate metadata; licence texts not checked) | in use (compiled in) | Transitive crates not reviewed |
| rsa 0.9 (RustCrypto) | `cc-cast` tests only: generates the fake receiver's RSA key at test time | MIT OR Apache-2.0 (as stated in the crate metadata; licence texts not checked) | dev-dependency, not shipped | |
| hayro 0.9.0 (with hayro-syntax, hayro-interpret, hayro-font, hayro-cmap, hayro-jpeg2000, hayro-jbig2, hayro-ccitt, vello_cpu, kurbo and others) | PDF page rendering on the receiver (`cc-docs`); also compiled to WASM for the web receiver (`cc-docs-wasm`) | Apache-2.0 OR MIT (hayro, from `cargo info`; transitive licence fields read from `cargo tree` on 2026-11-06: mostly MIT / Apache-2.0, also Zlib, BSD-3-Clause, Unicode-3.0, Unlicense; licence texts not checked) | in use (compiled in; hayro's default features embed substitute fonts and CMaps) | Pure Rust (zlib-rs for flate). The embedded substitute fonts (Foxit fonts from PDFium, `hayro-font` 0.5.0 `assets/LICENSE_FOXIT`) and the Adobe CMaps (`hayro-cmap` 0.1.1 `assets/LICENSE.txt`) are BSD-3-Clause: their notices are reproduced below and must ship with binaries. The two ICC profiles in `hayro-interpret` (`assets/CGATS001Compat-v2-micro.icc`, `LAB.icc`) come with a CC0 1.0 statement (`assets/CGATS_LICENSE.txt`; no notice duty, noted for completeness; not independently verified). Experimental renderer, see `docs/documents.md`. Transitive crates not reviewed |
| wasm-bindgen 0.2.130 (with wasm-bindgen-macro, wasm-bindgen-shared and others; the `wasm-bindgen` CLI of the same version is a build tool, not shipped) | JavaScript glue of the WASM PDF renderer (`crates/cc-docs-wasm`, web app only) | MIT OR Apache-2.0 (from the crate metadata, not re-checked; licence texts not checked) | in use (web app build only; the generated `cc_docs_wasm.js` and `.wasm` are served from `public/pdf/`) | Transitive crates not reviewed |
| png 0.18.1, image-webp 0.2.4, flate2 1 | PNG and lossless WebP encoding of rendered pages (`cc-docs`), WebP header check, inflate pre-scan of PDF streams in the render worker (`cc-docs`), test-time compression | MIT OR Apache-2.0 (from the crate metadata; licence texts not checked) | in use (compiled in; flate2 with the pure-Rust miniz_oxide backend) | Pure Rust, no libwebp |
| rlimit 0.11.0, landlock 0.4.7 (with enumflags2 0.7.12), seccompiler 0.5.0, rustix 1.1 (process functions only), libc 0.2 | Self-restriction of the PDF render worker process (`cc-docs`, Unix only; Landlock and seccompiler on Linux x86_64 and aarch64 only): resource limits, no file system and no TCP access, a seccomp-bpf syscall allowlist, process-group kill, `getuid` | rlimit: MIT; landlock: MIT OR Apache-2.0; enumflags2: MIT OR Apache-2.0; seccompiler: Apache-2.0 OR BSD-3-Clause; rustix: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; libc: MIT OR Apache-2.0 (all from `cargo metadata` on 2026-11-06; licence texts not checked) | in use (compiled in, Unix targets) | Permissive. These crates contain the `unsafe` system calls so that `cc-docs` keeps `unsafe_code = forbid`. seccompiler is an Amazon (Firecracker) crate under Apache-2.0 OR BSD-3-Clause: if shipped under its BSD-3-Clause option its copyright notice must be reproduced (not yet added). Not compiled for Windows or wasm32. Transitive crates not reviewed |
| Test PDFs (`crates/cc-docs/tests/data`) | Small PDFs for tests, generated by `gen_test_pdfs.py` (our own work, GPL-3.0-or-later) | own work; one file embeds a subset of Bitstream Vera (from reportlab 5.0.1) | in use (bundled as test data only) | The Bitstream Vera licence permits embedding and redistribution (not re-checked against the text). Details: `crates/cc-docs/tests/data/PROVENANCE.md`. reportlab (BSD) and Pillow are dev-time tools and are not shipped |
| office2pdf 0.10.0 (crates.io; with `office2pdf-docx-rs` 0.2.0 and `office2pdf-umya-spreadsheet` 0.2.0, its forks of docx-rs and umya-spreadsheet, and the Typst crates `typst`, `typst-layout`, `typst-pdf`, `typst-kit`, `typst-assets` 0.15.1, `comemo` and about 250 more crates) | Pure-Rust Office to PDF engine on the sender (`cc-office`, feature `engine-rust`, default on): `.docx .pptx .xlsx`. Runs only inside the `cc-office-worker` child process | office2pdf: Apache-2.0 (crate metadata); Typst crates: Apache-2.0; forks: MIT (as remembered from the crate metadata, not re-read); the new transitive crates were listed from `cargo metadata` on 2026-11-07: MIT, Apache-2.0, BSD-2/3-Clause, 0BSD, CC0-1.0, Zlib, Unlicense, Unicode-3.0, BSL-1.0 (`ryu`, offered as Apache-2.0 OR BSL-1.0), MPL-2.0 (`option-ext`, see the entry below). `libfuzzer-sys` (MIT OR Apache-2.0, plus NCSA) appears in `Cargo.lock` but is **not** in the dependency tree of `cc-office` (`cargo tree -p cc-office -i libfuzzer-sys` finds nothing, checked 2026-11-07), so NCSA is not listed as in use. No GPL-incompatible licence was seen in that list; licence texts not checked | in use (compiled into `cometcast-cli` and into the separate `cc-office-worker` program, which the Linux, Windows and macOS release jobs ship next to the Flutter app; `libcc_ffi` itself is built without the engine (`cc-office` with `default-features = false`), so the engine is in the worker, not in the library) | Apache-2.0 is compatible with GPL-3.0-or-later (one direction: the combined work is GPL-3.0). **Bundled fonts** inside the binary: Noto Sans, Noto Serif (OFL-1.1) and Selawik (OFL-1.1, Microsoft, Reserved Font Name) from office2pdf; from `typst-assets`: Libertinus Serif (OFL-1.1), New Computer Modern (GUST Font License / distribution exception), DejaVu Sans Mono (Bitstream Vera licence), Foxit Type 1 fonts (BSD-3-Clause, PDFium Authors, same text as reproduced below), plus ICC profiles (CC0) and HTML/MathML data (W3C document licence). The `NOTICE` file of `typst-assets` 0.15.1 holds the full texts; it is reproduced in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (heading `typst-assets 0.15.1`), which ships with every release. Maintenance: the crate was created 2026-02 and had about 12 000 downloads and releases every 1 to 3 weeks when checked (2026-11-07); a young single-maintainer project. Fidelity unmeasured on real documents. Details: `docs/documents.md` |
| Liberation 2.1.5 (Sans, Serif, Mono), Carlito 20230309, Caladea 20200211 (fonts, 20 TrueType files, unmodified, stored gzip-compressed) | Metric-compatible replacements for Arial, Times New Roman, Courier New, Calibri and Cambria in the Office converter (`cc-office`, `crates/cc-office/fonts`). Compiled into `cc-office-worker` and `cometcast-cli`; never fetched at run time | SIL Open Font License 1.1 (Liberation: Google Corporation and Red Hat, Inc.; Carlito: The Carlito Project Authors; Caladea: The Caladea Project Authors). Licence and author files: `crates/cc-office/fonts/` | in use (compiled in) | The OFL texts must ship with binaries: they are in `crates/cc-office/fonts/` and, with the Reserved Font Names, in `FONTS.md`; the fonts are not modified and not sold on their own. Sources, archive and file SHA-256: `crates/cc-office/fonts/FONTS.md` (checked by `node tools/check-fonts.mjs`). The Atkinson Hyperlegible fonts of the web app are a separate row below |
| Atkinson Hyperlegible Next and Atkinson Hyperlegible Mono 5.3.0 (npm `@fontsource/atkinson-hyperlegible-next` and `@fontsource/atkinson-hyperlegible-mono`, pinned in `apps/web/package.json`; Copyright 2020-2024 The Atkinson Hyperlegible Next Project Authors and The Atkinson Hyperlegible Mono Project Authors) | The typefaces of the display and presenter pages of the web app (`apps/web`, weights 400 / 500 / 700 and 500 / 700, subsets latin and latin-ext, woff2 and woff). Bundled into the build by Vite and served from the app's own origin; nothing is loaded from a font service | SIL Open Font License 1.1 (text below, "Atkinson Hyperlegible notice") | in use (web app build, Flutter apps) | The fonts are used unmodified and are not sold on their own. The OFL text and the copyright lines are reproduced below and must ship with the web build. The Flutter apps use the same release (5.3.0) as bundled TrueType files (`apps/flutter/assets/fonts`, converted from the woff files of the npm packages, glyphs unchanged; Latin and Latin Extended, same weights; OFL texts next to them). The Apple TV app does not use them yet |
| qrcode-generator 2.0.4 (npm, pinned in `apps/web/package.json`; Copyright (c) 2009 Kazuhiko Arase) | Makes the QR code on the web display's idle screen (`apps/web/src/core/qr.ts`, error correction level M). Bundled by Vite, no network use | MIT (the package's `license` field and the header of `dist/qrcode.mjs`; text below, "qrcode-generator notice") | in use (web app build) | "QR Code" is a registered trademark of DENSO WAVE INCORPORATED (mentioned in the package header). Checked against the file header on 2026-11-07; the package ships no separate LICENSE file |
| option-ext 0.2.0 (via dirs-sys 0.5.0, dirs 6.0.0, typst-kit 0.15.1, office2pdf) | Transitive dependency of the pure-Rust Office engine (`cc-office`, compiled into `cometcast-cli`) | MPL-2.0 (crate metadata; licence text not checked) | in use (compiled in, unmodified, from crates.io) | **MPL-2.0 file-level copyleft: a binary release must make the Source Code Form of `option-ext` available** (the unmodified crate source from crates.io, version 0.2.0, with a pointer in the release notes or this file), keep its licence and notices, and tell recipients that they can get that source. **Source availability:** the unmodified source of `option-ext` 0.2.0 is available at https://crates.io/crates/option-ext/0.2.0 (download: https://static.crates.io/crates/option-ext/option-ext-0.2.0.crate) and in its repository (https://github.com/soc/option-ext); the MPL-2.0 text is in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md). Release notes of builds that contain it repeat this sentence. Our own code is not covered (MPL-2.0 applies to the files of `option-ext`, not to ours; UNCERTAIN: MPL-2.0 section 3.3 on a larger work and the GPL secondary-licence rules were not legally reviewed). If `option-ext` is ever modified, the modified files must be published under MPL-2.0 |
| ISC-licensed crates: ring 0.17 (Apache-2.0 AND ISC), rustls-webpki 0.103 (ISC), untrusted 0.9 (ISC), and rustls 0.23 (Apache-2.0 OR ISC OR MIT) | TLS and certificate checks in the server and client crates (not the Office engine) | ISC (licence fields from `cargo metadata`, 2026-11-07; text not checked) | in use (compiled in) | ISC requires the copyright and permission notice with copies. The texts of `ring`, `rustls-webpki`, `untrusted` and `rustls` are in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md), which ships with every release. `ring` additionally carries BoringSSL/OpenSSL-derived notices in its own licence file (also reproduced there) |
| ryu 1.0 (via csv, typst-library, office2pdf) | Float formatting inside the Typst crates used by the pure-Rust Office engine | Apache-2.0 OR BSL-1.0 (crate metadata; we take Apache-2.0; text not checked) | in use (compiled in) | BSL-1.0 would need no notice in binary form; with the Apache-2.0 choice the Apache-2.0 notice rules apply |
| webrtc-rs (`webrtc` and `rtc` crates, 0.21.1) | WebRTC transport (native peers, `cc-transport`) | MIT / Apache-2.0 (as stated on crates.io; unchecked against the licence texts) | in use (compiled in, nothing bundled yet) | Pinned in the workspace `Cargo.toml`. Its transitive crates (ring and others) are not reviewed yet |
| LiveKit `webrtc-xcframework` 150.7871.05 (a build of Google's WebRTC M150 from the `webrtc-sdk/webrtc` fork; Objective-C classes prefixed `LK`), pinned in `apps/apple-tv/project.yml` by commit `7d963f6fae5921cc8e594a5fef42db2bd2ea8563`, zip SHA-256 `b24d7df29733079038931698102fbb39cbc11b014e4352beb0d4f55a01c3c37e` | WebRTC receiving in the native Apple TV app (`apps/apple-tv`): peer connection, data channel, H.264 decoding through VideoToolbox, Metal video view. A binary dependency resolved by Swift Package Manager | The repository's own `LICENSE` is MIT ("Copyright (c) 2021 WebRTC SDKs"); the xcframework's `LICENSE` is the BSD-3-Clause licence of "The WebRTC project authors" (read on 2026-11-07). The binary also contains third-party code (for example libvpx, Opus, BoringSSL, abseil, libyuv) whose licences and notices are **not collected yet**: THIRD_PARTY_LICENSES.md does not cover the Apple TV app's Swift dependencies and the xcframework's bundled notices | planned (written into the project spec; never built, not distributed) | **Unchecked:** the complete third-party notice list of the binary must be assembled and shipped with any tvOS release; do not release the Apple TV app before that. `stasel/WebRTC` (BSD-3-Clause) was evaluated and rejected: it has no tvOS slice |
| swift-crypto 3.10.0 (Apple, with swift-asn1 and BoringSSL sources inside) | Linux only: SHA-2 and HMAC for the tests of `apps/apple-tv/CometCastCore` on machines without CryptoKit. Not part of any shipped app (the tvOS app uses CryptoKit) | Apache-2.0 (as stated in the package; licence text not checked) | dev and CI only (not shipped) | |
| Test video frames (`crates/cc-sender/assets/test-frames.ivf`) | Synthetic VP8 key frames for tests | Our own work (GPL-3.0-or-later); encoded once with libwebp (BSD-3-Clause) | in use (bundled in the sender crate) | Generated by `tools/gen-test-frames.py` with Python 3.13.18, Pillow 12.3.0 and its bundled libwebp 1.6.0. The frames are plain rectangles; no font and no third-party image is embedded. Pillow and libwebp are only dev-time tools and are not shipped |
| DejaVu Sans Bold (font) | Only used by `apps/flutter/android/tools/make_android_icons.py` (Pillow) to draw the words "Comet Cast" into the Android TV banner PNG | Bitstream Vera / DejaVu Fonts licence (free licence; permits use and redistribution of the font) | dev-time tool only; no font file is shipped, only the rendered pixels in `tv_banner.png` | Whether text rendered with the font needs an attribution was not checked; the banner is our own artwork |
| mdns-sd 0.21 | mDNS discovery on desktop (AirPlay receivers in `cc-airplay-sender`) | Apache-2.0 OR MIT (as stated in the crate metadata; licence texts not checked) | in use (compiled in, nothing bundled yet) | Transitive crates (flume, socket2, if-addrs, polling and others) not reviewed |
| AirPlay sender crates (`cc-airplay-sender`): plist 1.8 (MIT), num-bigint 0.4 (MIT OR Apache-2.0), sha1 0.11, sha2, hkdf 0.13, aes 0.8, ctr 0.9, aes-gcm 0.10, chacha20poly1305 0.10 (all MIT OR Apache-2.0), ed25519-dalek 2.2 and x25519-dalek 2.0 with curve25519-dalek 4.1 and subtle (BSD-3-Clause) | Pairing and encryption of the AirPlay sender without FairPlay (SRP-6a, HKDF-SHA-512, ChaCha20-Poly1305, Ed25519, X25519, AES-GCM/CTR), binary property lists | as listed (licence fields read from `cargo metadata` on 2026-11-06; licence texts not checked) | in use (compiled in, nothing bundled yet) | BSD-3-Clause needs the copyright notice and licence text when binaries are shipped. Transitive crates not reviewed. No FairPlay code, tables or keys; see `docs/airplay-sender.md` |
| pyatv 0.18.0 and srptools 2.0.0 (reference only) | Read as public protocol documentation for the AirPlay sender (message flows, byte conventions). **No code copied**, nothing bundled | MIT (pyatv), BSD-3-Clause (srptools) | reference only | Names of HKDF labels and message tags are protocol facts. If code is ever copied, the MIT/BSD notices must be added here |
| hickory-resolver 0.26.3 (with hickory-proto, hickory-net, resolv-conf 0.7; features `system-config` and `tokio` only, no DNSSEC, no TLS/DoH/QUIC) | DNS-SRV lookup of `_cometcast._tcp.<search-domain>` in `cc-discovery` | MIT OR Apache-2.0 (licence fields read from the crate metadata on 2026-11-06; licence texts not checked) | in use (compiled in, nothing bundled yet) | Transitive crates (ipnet, moka, prefix-trie, idna/ICU data and others) not reviewed. UNCERTAIN: 0.26.3 was the newest version found; it declares Rust 1.88, which is now also the project MSRV in the workspace manifest (not measured with exactly that toolchain). `cc-discovery` also uses plist 1.8 (MIT, see the AirPlay row) for the macOS policy file and mdns-sd 0.21 (see the row above) for announcements |
| url 2.5 (with idna, percent-encoding and the ICU data crates it pulls in) | WHATWG URL parsing and normalisation for link validation, so that the native clients agree with the browser's `new URL()` (`cc-files`) | MIT OR Apache-2.0 (url and idna; licence fields not re-read, texts not checked); the `icu_*` data crates are Unicode-3.0 (unchecked) | in use (compiled in) | Also in the dependency tree through the WebRTC stack and `hickory-resolver`. Transitive crates not reviewed. The Unicode-3.0 licence of the ICU crates needs its notice with binaries (not yet added) |
| lopdf | Writing annotations into PDFs | MIT | planned | |
| axum, tokio and other Rust crates | Server and core | mostly MIT / Apache-2.0 | in use | Complete list with licence texts: [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) |
| Server hardening crates (`cc-server`): altcha 1.1.0, axum-server 0.8.0, governor 0.10.4, moka 0.12.16, ipnet, toml, rustls 0.23 (ring backend), rcgen 0.14, hmac, sha1, base64 | ALTCHA proof-of-work, HTTPS, rate limits, replay cache, config, TURN credentials | MIT, MIT OR Apache-2.0 (rustls: Apache-2.0 OR ISC OR MIT), as stated in the crates' `Cargo.toml` metadata | in use (compiled in, nothing bundled yet) | Licence fields read from `cargo metadata` on 2026-11-06; licence texts not checked. Transitive crates not reviewed |
| `windows` crate 0.62.2 (Microsoft; with `windows-core`, `windows-future`, `windows-collections` and others) | Windows Runtime bindings for Miracast receiving (`cc-miracast`, Windows targets only) | MIT OR Apache-2.0 (as stated in the crate metadata; licence texts not checked) | in use (compiled in on Windows targets only; compile-checked, never run; nothing bundled) | Calls only the documented `Windows.Media.Miracast` API. Transitive crates not reviewed. Details: `docs/miracast.md` |
| Flutter packages `file_selector` 1.1 and `url_launcher` 6.3 (with their platform packages, for example `file_selector_linux`, `url_launcher_linux`) | File dialog of "Send file or link"; opening a received link after a click (`apps/flutter`) | BSD-style (the LICENSE files of the Flutter Authors were seen in the pub cache; the exact variant and the licences of the transitive packages were not checked) | in use (Linux build compiled) | Added for the content channel; not audited |
| Flutter packages `flutter_svg` 2.3.2 (with `vector_graphics` 1.2.6, `vector_graphics_codec`, `vector_graphics_compiler`, `path_parsing`, `xml`), `path_drawing` 1.0.1 | The logo (SVG with gradients) and the icon paths of the designed UI (`apps/flutter/lib/src/design`) | MIT (flutter_svg, path_drawing, as read from the LICENSE files in the pub cache); transitive packages are in the generated list | in use (tests run; Linux build not re-run for this change) | Pinned exactly |
| Flutter packages `qr_flutter` 4.1.0 and `qr` 3.0.3 | The QR code on the display's idle screen, error correction level M | BSD-3-Clause (LICENSE files read from the pub cache) | in use (tests run) | Pinned exactly |
| Flutter package `wakelock_plus` 1.8.2 (with `wakelock_plus_platform_interface`, `package_info_plus`, `dbus`, `win32`) | Keeps the screen awake while presenting on iOS, macOS, Windows and Linux (Android uses the existing window-flag channel) | BSD-3-Clause (wakelock_plus; transitive licences in the generated list, not audited one by one) | in use in code; never run on a device or desktop session | Pinned exactly. UNCERTAIN: platform behaviour untested |
| sha2 0.11 (RustCrypto; `cc-transport`) | SHA-256 of the connection check code (`cc-transport/src/sas.rs`, the same digits as `apps/web/src/core/sas.ts` and the Swift core; vectors in `tests/fixtures/sas.json`) | MIT OR Apache-2.0 (as stated in the crate metadata; licence texts not checked; the crate was already in the tree through `cc-airplay-sender`) | in use (compiled in) | Transitive crates not reviewed |
| `com.google.android.gms:play-services-code-scanner` 16.1.0 (Google Play services, Android only) | The system QR scanner (`GmsBarcodeScanning`, native-apps.md section 8): the scanner runs in Google's process, so the app asks for no camera permission (`apps/flutter/android/.../scanner/QrBridge.kt`) | Proprietary: Google APIs / Google Play services terms (not an open source licence; the library is distributed through Google's Maven repository). Checked against the design decision N7, which names this scanner. | in use in code (written for Android; not compiled or run in this environment) | Pinned exactly. The app works without it (`isAvailable` answers false without Play services: no scan button). UNCERTAIN: not usable on devices without Google Play services (de-Googled builds, F-Droid); the owner may prefer an open scanner there. The ML Kit module is downloaded by Play services on first use |
| `com.android.installreferrer:installreferrer` 2.2 (Google, Android only) | The Google Play install referrer: the code and server of the link that sent the person to the store (`.../links/LinkBridge.kt`, native-apps.md 20.4) | Apache-2.0 (as published by Google; licence text not copied here) | in use in code (not compiled or run in this environment) | Pinned exactly. The referrer is still an assumption of the design (spec 12) |
| Apple system frameworks VisionKit (`DataScannerViewController`) and AVFoundation (iOS, iPadOS) | The system QR scanner on iPhone and iPad (`apps/flutter/ios/Runner/Scanner/QrBridge.swift`) | Part of the operating system (no code is bundled) | in use in code (never compiled: no Xcode here) | iOS 16 or newer; before that no scan button. Camera permission text: `NSCameraUsageDescription` (`ios.cameraUsage`) |
| Flutter, Vue 3 and their packages | Apps and web app | see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (generated from the lock files; the Flutter section depends on the pub cache, the npm section lists SPDX identifiers only) | in use | Flutter SDK and engine notices (the engine's bundled third-party code) are not collected there |

## Source of this list

The planned components and licences come from the project planning document (Rust modules and libraries, "legal and licences" notes). Crate licences were queried from crates.io on 2 November 2026 according to that document. This is an overview, not legal advice.

## Names

Apple, AirPlay, tvOS, Google, Google Cast, Chromecast, Android, Miracast, AirServer, AirParrot and other names are trademarks of their owners. They are used only to describe compatibility.

## Additional notes (from an audit, not yet a full list)

- The full list of Rust crates is in `Cargo.lock`; all 372 external crates of that audit (the generated list now has more, see its summary) were reported as permissive
  (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, Unlicense, BSL-1.0) by an audit that read the crate
  metadata. Two oddities, neither a conflict: `allo-isolate` has no SPDX field but ships an Apache-2.0
  licence file; `r-efi` is offered under MIT, Apache-2.0 or LGPL-2.1-or-later (we pick MIT).
- npm build-time tools: `lightningcss` (MPL-2.0, three packages, pulled in by Tailwind). Runtime web
  dependencies are Vue and vue-i18n (MIT). Flutter package licences were not audited yet.
- Test fixtures `crates/cc-airplay/tests/fixtures/uxplay-1.68-real-*.txt` are output and help text of
  UxPlay (GPL-3.0). UNCERTAIN: whether quoting that text needs a notice; the MAC address in one file
  was replaced by zeros.
- Logo and icon files under `assets/logo` come from the owner's design hand-off. The author and licence
  of the artwork are not recorded in the repository yet (see the open questions).
- The full generated list is [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (`node tools/gen-third-party.mjs`).

## Terms that need a sentence

- **Unicode-3.0** (`icu_*`, `unicode-ident` and others): the licence text is in THIRD_PARTY_LICENSES.md and must stay with binaries and data files.
- **BSD-3-Clause** (`curve25519-dalek`, `ed25519-dalek`, `x25519-dalek`, `subtle`, `seccompiler` (as an option), `alloc-stdlib`, and others): copyright notice and text in THIRD_PARTY_LICENSES.md; where a crate ships no licence file its SPDX expression is all that is listed (see the note at the top).
- **ISC**: see the ISC row above.
- **CDLA-Permissive-2.0** (`webpki-roots`): see the `cc-codec` row.
- **MPL-2.0** (`option-ext`): see the `option-ext` row, with the source-availability sentence.
- **SIL OFL 1.1** fonts: Liberation, Carlito, Caladea (above), Libertinus Serif, Noto Sans/Serif, Selawik (inside `typst-assets` and `office2pdf`, texts in THIRD_PARTY_LICENSES.md).
- **AVC/H.264:** see the `openh264` row; the OpenH264 binary is Cisco's, downloaded by the user's device, and this project does not redistribute it.

## OpenH264 notice

Cisco's OpenH264 (the source of the prebuilt binary that the user's device downloads, and of the `openh264` crate's C sources when built from source in development). BSD-2-Clause, text copied from Cisco's repository:

```
Copyright (c) 2013, Cisco Systems
All rights reserved.

Redistribution and use in source and binary forms, with or without modification,
are permitted provided that the following conditions are met:

* Redistributions of source code must retain the above copyright notice, this
  list of conditions and the following disclaimer.

* Redistributions in binary form must reproduce the above copyright notice, this
  list of conditions and the following disclaimer in the documentation and/or
  other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

Cisco's binary is licensed to users under the AVC/H.264 patent terms Cisco pays for the binary it distributes itself; building OpenH264 from source does not carry that cover (see the `openh264` row).

## Open Screen Library notice

Used as the specification reference for `crates/cc-cast` (see the table above and `docs/cast.md`).

```
Copyright 2018 The Chromium Authors

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

   * Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.
   * Redistributions in binary form must reproduce the above
copyright notice, this list of conditions and the following disclaimer
in the documentation and/or other materials provided with the
distribution.
   * Neither the name of Google LLC nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## PDFium Foxit font notice (hayro-font)

The substitute fonts in `hayro-font` 0.5.0 (`assets/Foxit*.pfb`, embedded by hayro's default features) come with this licence (file `assets/LICENSE_FOXIT`, reproduced verbatim without the comment markers). BSD-3-Clause.

```
Copyright 2014 PDFium Authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

   * Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.
   * Redistributions in binary form must reproduce the above
copyright notice, this list of conditions and the following disclaimer
in the documentation and/or other materials provided with the
distribution.
   * Neither the name of Google Inc. nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Adobe CMap notice (hayro-cmap)

The compressed CMap data in `hayro-cmap` 0.1.1 (`assets/cmaps.brotli`) comes with this licence (file `assets/LICENSE.txt`). BSD-3-Clause.

```
Copyright 1990-2023 Adobe. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

Redistributions of source code must retain the above copyright notice,
this list of conditions and the following disclaimer.

Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

Neither the name of Adobe nor the names of its contributors may be
used to endorse or promote products derived from this software without
specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Atkinson Hyperlegible notice

Copyright lines of the two font packages (the licence text is the same for both; it is reproduced once). Read from the `LICENSE` files of `@fontsource/atkinson-hyperlegible-next` 5.3.0 and `@fontsource/atkinson-hyperlegible-mono` 5.3.0 on 2026-11-07.

```text
Copyright 2020-2024 The Atkinson Hyperlegible Mono Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next-mono) AtkinsonHyperlegibleMono-Italic[wght].ttf: Copyright 2020-2024 The Atkinson Hyperlegible Mono Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next-mono)
```

```text
Copyright 2020-2024 The Atkinson Hyperlegible Next Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next) AtkinsonHyperlegibleNext-Italic[wght].ttf: Copyright 2020-2024 The Atkinson Hyperlegible Next Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

## qrcode-generator notice

From the header of `dist/qrcode.mjs` of `qrcode-generator` 2.0.4: "Copyright (c) 2009 Kazuhiko Arase. URL: http://www.d-project.com/. Licensed under the MIT license: http://www.opensource.org/licenses/mit-license.php. The word 'QR Code' is registered trademark of DENSO WAVE INCORPORATED." The standard MIT licence text follows (the package carries no separate file; the wording is the one of the MIT licence).

```text
MIT License

Copyright (c) 2009 Kazuhiko Arase

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
