# Third-party notices

**Current state: nothing is bundled yet.** The repository does not ship any third-party binary. All entries below are **planned** uses according to the project plan unless a row says otherwise. This file will be updated when a component is actually added; at that point the licence texts and source offers listed here must be shipped with it.

Licence details marked **unchecked** were not verified and must be checked against the licence text before release.

| Component | Planned use | Licence | Status | Notes |
| --- | --- | --- | --- | --- |
| UxPlay | AirPlay receiving, run as a separate companion process on Windows, Linux and macOS. Not linked, not copied. `crates/cc-airplay` only starts and watches the program | GPL-3.0 (the Ubuntu package 1.68.2 lists parts as LGPL-2.1-or-later and MIT, from its copyright file) | supervisor code in use; **no UxPlay binary or source is bundled** | When a release ships UxPlay: ship the GPL-3.0 text, an offer of the matching source and the notices of its bundled parts. Legal classification of the FairPlay handshake is not finally settled, so this stays an optional, separate module. Details and what was tested: `docs/airplay.md`. Legal reading: unchecked. The supervisor was built and tested on Linux only; its signal and process-group handling is Unix-only code, and Windows and macOS are unverified (see `docs/airplay.md`) |
| yt-dlp | Media-link resolving on desktop and mini PC, run as a separate process, always with `--xff never` | Unlicense (GPLv3+ for the single-file build, as stated in the plan) | planned | Ship the licence text. Updater only outside Google Play and the Mac App Store. iOS apps do not resolve links themselves (App Store rule 5.2.3). Exact licence details: unchecked |
| openh264 (`openh264` crate 0.9.9 and Cisco's OpenH264 2.6.0) | Software H.264 encoder (screen sender) and a test-only decoder, `cc-media`. Two build modes: compiled from the source inside the crate (`h264-source`, default), or Cisco's prebuilt library loaded at run time (`h264-libloading`) | BSD-2-Clause (crate and OpenH264, as stated in the crate metadata; licence texts not checked) | in use (compiled in; no Cisco binary is bundled) | **Uncertain: H.264 patent coverage.** The crate documentation says the Cisco binary should be downloaded during installation; whether a source build is covered was not checked. See docs/capture.md, section "H.264 licensing". Hardware encoders of the systems are to be used first (plan) |
| x11rb 0.14 | X11 screen capture in pure Rust (`cc-capture`) | MIT OR Apache-2.0 (as stated on crates.io; unchecked against the licence texts) | in use (compiled in) | |
| coturn | TURN server, separate process | BSD-style | planned | **Uncertain:** the plan says "BSD-like"; check the exact licence text before release and ship the licence file |
| Open Screen Library (commit `8b62b21479f1adb7663f182d97962b960c5c64ef`, chromium.googlesource.com/openscreen) | Specification reference for the Chromecast / Google Cast compatible sender `cc-cast`: message formats, field names, constants, framing, RTP/RTCP layout, AES-CTR nonce, device-auth steps. The code of `cc-cast` is written in Rust from reading these files; it is an adaptation, not a copy. Files read are listed in `docs/cast.md` | BSD-3-Clause, Copyright 2018 and later The Chromium Authors (text below) | in use as reference; adapted parts are marked in the `cc-cast` module headers | The notice below must stay with source and binary distributions of `cc-cast`. No Open Screen source file and none of its trust-anchor data is copied or embedded. UNCERTAIN: whether further per-file notices are needed was not checked with a lawyer |
| prost 0.14, mdns-sd 0.21, aes 0.8, ctr 0.9, ring 0.17, x509-parser 0.18 | `cc-cast`: protobuf derive, mDNS browsing, AES-128-CTR, RSA signature check, certificate parsing | Apache-2.0 / MIT OR Apache-2.0 / Apache-2.0 AND ISC (as stated in the crate metadata; licence texts not checked) | in use (compiled in) | Transitive crates not reviewed |
| rsa 0.9 (RustCrypto) | `cc-cast` tests only: generates the fake receiver's RSA key at test time | MIT OR Apache-2.0 (as stated in the crate metadata; licence texts not checked) | dev-dependency, not shipped | |
| hayro 0.9.0 (with hayro-syntax, hayro-interpret, hayro-font, hayro-cmap, hayro-jpeg2000, hayro-jbig2, hayro-ccitt, vello_cpu, kurbo and others) | PDF page rendering on the receiver (`cc-docs`); later also as WASM | Apache-2.0 OR MIT (hayro, from `cargo info`; transitive licence fields read from `cargo tree` on 2026-11-06: mostly MIT / Apache-2.0, also Zlib, BSD-3-Clause, Unicode-3.0, Unlicense; licence texts not checked) | in use (compiled in; hayro's default features embed substitute fonts and CMaps) | Pure Rust (zlib-rs for flate). The embedded substitute fonts (Foxit fonts from PDFium, `hayro-font` 0.5.0 `assets/LICENSE_FOXIT`) and the Adobe CMaps (`hayro-cmap` 0.1.1 `assets/LICENSE.txt`) are BSD-3-Clause: their notices are reproduced below and must ship with binaries. The two ICC profiles in `hayro-interpret` (`assets/CGATS001Compat-v2-micro.icc`, `LAB.icc`) come with a CC0 1.0 statement (`assets/CGATS_LICENSE.txt`; no notice duty, noted for completeness; not independently verified). Experimental renderer, see `docs/documents.md`. Transitive crates not reviewed |
| png 0.18.1, image-webp 0.2.4, flate2 1 | PNG and lossless WebP encoding of rendered pages (`cc-docs`), WebP header check, inflate pre-scan of PDF streams in the render worker (`cc-docs`), test-time compression | MIT OR Apache-2.0 (from the crate metadata; licence texts not checked) | in use (compiled in; flate2 with the pure-Rust miniz_oxide backend) | Pure Rust, no libwebp |
| rlimit 0.11.0, landlock 0.4.7 (with enumflags2 0.7.12), seccompiler 0.5.0, rustix 1.1 (process functions only), libc 0.2 | Self-restriction of the PDF render worker process (`cc-docs`, Unix only; Landlock and seccompiler on Linux x86_64 and aarch64 only): resource limits, no file system and no TCP access, a seccomp-bpf syscall allowlist, process-group kill, `getuid` | rlimit: MIT; landlock: MIT OR Apache-2.0; enumflags2: MIT OR Apache-2.0; seccompiler: Apache-2.0 OR BSD-3-Clause; rustix: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; libc: MIT OR Apache-2.0 (all from `cargo metadata` on 2026-11-06; licence texts not checked) | in use (compiled in, Unix targets) | Permissive. These crates contain the `unsafe` system calls so that `cc-docs` keeps `unsafe_code = forbid`. seccompiler is an Amazon (Firecracker) crate under Apache-2.0 OR BSD-3-Clause: if shipped under its BSD-3-Clause option its copyright notice must be reproduced (not yet added). Not compiled for Windows or wasm32. Transitive crates not reviewed |
| Test PDFs (`crates/cc-docs/tests/data`) | Small PDFs for tests, generated by `gen_test_pdfs.py` (our own work, GPL-3.0-or-later) | own work; one file embeds a subset of Bitstream Vera (from reportlab 5.0.1) | in use (bundled as test data only) | The Bitstream Vera licence permits embedding and redistribution (not re-checked against the text). Details: `crates/cc-docs/tests/data/PROVENANCE.md`. reportlab (BSD) and Pillow are dev-time tools and are not shipped |
| office2pdf | Office to PDF on the sender | Apache-2.0 | planned | |
| webrtc-rs (`webrtc` and `rtc` crates, 0.21.1) | WebRTC transport (native peers, `cc-transport`) | MIT / Apache-2.0 (as stated on crates.io; unchecked against the licence texts) | in use (compiled in, nothing bundled yet) | Pinned in the workspace `Cargo.toml`. Its transitive crates (ring and others) are not reviewed yet |
| Test video frames (`crates/cc-sender/assets/test-frames.ivf`) | Synthetic VP8 key frames for tests | Our own work (GPL-3.0-or-later); encoded once with libwebp (BSD-3-Clause) | in use (bundled in the sender crate) | Generated by `tools/gen-test-frames.py` with Python 3.13.18, Pillow 12.3.0 and its bundled libwebp 1.6.0. The frames are plain rectangles; no font and no third-party image is embedded. Pillow and libwebp are only dev-time tools and are not shipped |
| DejaVu Sans Bold (font) | Only used by `apps/flutter/android/tools/make_android_icons.py` (Pillow) to draw the words "Comet Cast" into the Android TV banner PNG | Bitstream Vera / DejaVu Fonts licence (free licence; permits use and redistribution of the font) | dev-time tool only; no font file is shipped, only the rendered pixels in `tv_banner.png` | Whether text rendered with the font needs an attribution was not checked; the banner is our own artwork |
| mdns-sd 0.21 | mDNS discovery on desktop (AirPlay receivers in `cc-airplay-sender`) | Apache-2.0 OR MIT (as stated in the crate metadata; licence texts not checked) | in use (compiled in, nothing bundled yet) | Transitive crates (flume, socket2, if-addrs, polling and others) not reviewed |
| AirPlay sender crates (`cc-airplay-sender`): plist 1.8 (MIT), num-bigint 0.4 (MIT OR Apache-2.0), sha1 0.11, sha2, hkdf 0.13, aes 0.8, ctr 0.9, aes-gcm 0.10, chacha20poly1305 0.10 (all MIT OR Apache-2.0), ed25519-dalek 2.2 and x25519-dalek 2.0 with curve25519-dalek 4.1 and subtle (BSD-3-Clause) | Pairing and encryption of the AirPlay sender without FairPlay (SRP-6a, HKDF-SHA-512, ChaCha20-Poly1305, Ed25519, X25519, AES-GCM/CTR), binary property lists | as listed (licence fields read from `cargo metadata` on 2026-11-06; licence texts not checked) | in use (compiled in, nothing bundled yet) | BSD-3-Clause needs the copyright notice and licence text when binaries are shipped. Transitive crates not reviewed. No FairPlay code, tables or keys; see `docs/airplay-sender.md` |
| pyatv 0.18.0 and srptools 2.0.0 (reference only) | Read as public protocol documentation for the AirPlay sender (message flows, byte conventions). **No code copied**, nothing bundled | MIT (pyatv), BSD-3-Clause (srptools) | reference only | Names of HKDF labels and message tags are protocol facts. If code is ever copied, the MIT/BSD notices must be added here |
| hickory-resolver 0.26.3 (with hickory-proto, hickory-net, resolv-conf 0.7; features `system-config` and `tokio` only, no DNSSEC, no TLS/DoH/QUIC) | DNS-SRV lookup of `_cometcast._tcp.<search-domain>` in `cc-discovery` | MIT OR Apache-2.0 (licence fields read from the crate metadata on 2026-11-06; licence texts not checked) | in use (compiled in, nothing bundled yet) | Transitive crates (ipnet, moka, prefix-trie, idna/ICU data and others) not reviewed. UNCERTAIN: 0.26.3 was the newest version found; it declares Rust 1.88, which is now also the project MSRV in the workspace manifest (not measured with exactly that toolchain). `cc-discovery` also uses plist 1.8 (MIT, see the AirPlay row) for the macOS policy file and mdns-sd 0.21 (see the row above) for announcements |
| lopdf | Writing annotations into PDFs | MIT | planned | |
| axum, tokio and other Rust crates | Server and core | mostly MIT / Apache-2.0 | planned | A full list will be generated from `Cargo.lock` once the workspace has dependencies |
| Server hardening crates (`cc-server`): altcha 1.1.0, axum-server 0.8.0, governor 0.10.4, moka 0.12.16, ipnet, toml, rustls 0.23 (ring backend), rcgen 0.14, hmac, sha1, base64 | ALTCHA proof-of-work, HTTPS, rate limits, replay cache, config, TURN credentials | MIT, MIT OR Apache-2.0 (rustls: Apache-2.0 OR ISC OR MIT), as stated in the crates' `Cargo.toml` metadata | in use (compiled in, nothing bundled yet) | Licence fields read from `cargo metadata` on 2026-11-06; licence texts not checked. Transitive crates not reviewed |
| `windows` crate 0.62.2 (Microsoft; with `windows-core`, `windows-future`, `windows-collections` and others) | Windows Runtime bindings for Miracast receiving (`cc-miracast`, Windows targets only) | MIT OR Apache-2.0 (as stated in the crate metadata; licence texts not checked) | in use (compiled in on Windows targets only; compile-checked, never run; nothing bundled) | Calls only the documented `Windows.Media.Miracast` API. Transitive crates not reviewed. Details: `docs/miracast.md` |
| Flutter, Vue 3 and their packages | Apps and web app | not recorded in the plan | planned | A list will be generated from the lock files |

## Source of this list

The planned components and licences come from the project planning document (Rust modules and libraries, "legal and licences" notes). Crate licences were queried from crates.io on 2 November 2026 according to that document. This is an overview, not legal advice.

## Names

Apple, AirPlay, tvOS, Google, Google Cast, Chromecast, Android, Miracast, AirServer, AirParrot and other names are trademarks of their owners. They are used only to describe compatibility.

## Additional notes (from an audit, not yet a full list)

- The full list of Rust crates is in `Cargo.lock`; all 372 external crates were reported as permissive
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
- A full generated list (for example with cargo-about or cargo-deny and `npm ls`) is still to do.

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
