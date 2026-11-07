# Screen capture and H.264 (native sender, Linux/X11)

This page describes how the native sender gets real screen content, encodes it as H.264 and
sends it through the existing WebRTC session. It is the first real implementation behind the
measurement points A10 (efficient image transfer) and A11 (hardware encoders) in
[measurement-points.md](measurement-points.md); it does not change that file. What was measured
here is a single-machine test under Xvfb, not a field measurement (see "What is not verified").

The ideas for sending only what changes come from the project plan (section on efficient image
transfer). Everything here is written for Comet Cast in its own words; no code of the projects
named there was used.

## Pieces

| Where | What |
| --- | --- |
| `crates/cc-capture` | `ScreenSource` trait; `X11Source` (pure Rust `x11rb`: `GetImage`, DAMAGE as a hint, RandR for the monitor list); `ChangeDetector` (64-bit hash of the picture); `Unsupported` stubs for Wayland/PipeWire, Windows, macOS; `testutil` (starts a private Xvfb, draws a coloured window; only for tests, feature `test-support`) |
| `crates/cc-media` | `H264Encoder` / `H264Decoder` (OpenH264 through the `openh264` crate), `StaticRefresh` (timing only), `H264Assembler` (RTP payloads back to Annex-B, tests/CLI only), `HardwareEncoderProbe` (trait only) |
| `crates/cc-transport` | H.264 registered as `packetization-mode=1;profile-level-id=42e01f`; PLI/FIR requests are delivered to the sender (`VideoSender::next_keyframe_request`); `Session::request_keyframe` on the receiving side; an optional tap that copies received RTP packets to a bounded channel (`SessionConfig::video_tap`, a `VideoTap`; packets are dropped and counted when the reader is too slow; for tests and the CLI) |
| `crates/cc-sender` | `SenderOptions::screen`: capture -> change check -> encode -> RTP, on one thread; counters (`SenderCore::screen_stats`) |
| `bins/cometcast-cli` | `send <CODE> --screen [--display :99] [--fps 15] [--bitrate-kbps 2000]`; `receive --decode` decodes the received H.264 with OpenH264 and prints size and mean colour |

## How a frame is made

1. The capture thread waits for the next frame time (`--fps`, default 15).
2. If the X server has DAMAGE and reported nothing since the last frame, the answer is "unchanged"
   and no pixels are read. Otherwise the picture is read with `GetImage`, the unused fourth byte is set to
   255, and its hash is compared with the previous one. Equal hash = unchanged.
3. An unchanged picture is **not encoded**. A changed one is converted BGRA -> YUV 4:2:0 (by the
   `openh264` crate) and encoded. The first picture is an IDR frame with SPS and PPS; there are no
   periodic key frames.
4. **One frame in flight:** the thread only looks at the screen again after the async side reported
   that the previous frame was handed to the WebRTC library (`write_sample` returned). The library's own
   queues behind that call are not observed, so this limits the sender's queue, not the network's.
5. **Key frames on request:** the sender takes PLI and FIR from the receiver (an interceptor passes them
   on) and then encodes the last picture again as an IDR frame, even if the screen is static. The first frame
   after the connection is established is an IDR frame, which covers a new receiver. The request flag is
   cleared only when a frame was really sent (the rate control may skip the picture; the request then stays
   and is retried on the next tick). **Rate limit:** at most one request is accepted per second (placeholder,
   not tuned); a request that arrives while one is pending or less than a second after the last accepted one
   is ignored and counted (`ScreenStats::keyframe_requests_ignored`), so a flood of PLI/FIR cannot make the
   sender produce a key frame per request. Only IDR frames count as key frames in the statistics.
6. **Static refresh:** 100 ms after the last change, the last picture is encoded again, at most 3 times,
   100 ms apart, then nothing is sent until the screen changes. Placeholders, not tuned.
   **Skipped pictures:** OpenH264's rate control may skip a picture (`encode` returns nothing; seen after a
   large first frame). Such a picture is "owed": it is offered to the encoder again on every tick (15 times a
   second) until a frame comes out, for at most 30 s (placeholder), and these retries do not use up the 3
   refreshes. Before this, the 3 refreshes were used up by skips and the final picture could stay unsent
   until the next screen change. The number of skipped encoder calls is `ScreenStats::skipped`.
7. **Failure reporting:** a panic in the capture thread and a failure to start it are reported as
   `SenderEvent::Error`; `Pipeline::stop` waits at most 3 s for the thread (it may sit in a call to an X
   server that died) and then leaves it to end on its own.
8. The RTP time step of a frame is the real time since the previous frame, so pauses on a static screen
   do not make the picture play too fast afterwards.

Codec settings: Baseline profile (OpenH264 writes `profile_idc=66`, constraint flags `0xc0` = Constrained
Baseline; level 3.1 at 1280x720, 4.0 at 1920x1080, as printed by a unit test), screen-content mode,
bit-rate rate control (default 2 Mbit/s), frame skipping on, no periodic key frames. Odd widths or heights lose
their last column or row.

## H.264 licensing (not checked)

**Uncertain: patent and licence points were not checked by a lawyer.** What is known from the
sources read:

- The `openh264` crate (BSD-2-Clause) wraps Cisco's OpenH264. It has two build modes, both implemented
  behind Cargo features of `cc-media`:
  - `h264-source`: OpenH264 is compiled from the C++ source that ships in the crate. Here this needed
    `g++` and `nasm`, both present, and compiled in about 8 s.
  - `h264-libloading`: the program loads Cisco's prebuilt `libopenh264` at run time from a path
    (`H264Backend::Library(path)`, or the environment variable `COMETCAST_OPENH264_LIB`). The crate only loads
    files whose SHA-256 is on its list of known Cisco builds; a unit test checks that `/bin/ls` is refused.
- The plan prefers the prebuilt binary downloaded at install time. The `openh264` crate's own documentation
  says the library should be downloaded from Cisco during installation to be "legal". **Uncertain:** that
  Cisco's patent licence covers only the binary Cisco builds and distributes (a statement from the crate
  documentation and Cisco's FAQ as remembered, not re-read here); whether a source build of an app is
  covered; whether downloading during installation is enough for a school deployment; what applies to the
  decoder-only use in tests. Do not ship releases built with `h264-source` before this is clarified.
- **Which build gets which feature.** The workspace dependencies on `cc-media` and `cc-sender` have
  `default-features = false`, so no library crate pulls in the source build by itself. The program crate
  decides: `cometcast-cli` has the features `h264-source` (in its `default`, meant for development and CI) and
  `h264-libloading`. (`cc-media` and `cc-sender` also list `h264-source` as their own default, only so that
  `cargo test -p cc-media` works on its own; the `cc-receiver` tests ask for it explicitly.)
- **Release build command (no OpenH264 source build):**

  ```
  cargo build --release -p cometcast-cli --no-default-features --features h264-libloading
  ```

  At run time the program needs Cisco's library; see "Runtime codec (download on first use)" below for how
  it finds or fetches it. Checked here for this command (debug profile, same features): the binary contains no
  OpenH264 code (`nm -C | grep -c WelsEnc` gives 0, against 807 matches for `WelsEnc` and 466 for `WelsDec` in
  the default build); `cargo tree -e features -i openh264` shows only `openh264 feature "libloading"`; and with
  no library available `cometcast-cli receive --decode` exits with status 1 and a message that still begins
  "OpenH264 backend not available: this program was built without the h264-source feature" (the CI step greps
  that phrase; the rest of the sentence now names the download and the two environment variables).
  `send --screen` creates the same encoder, so it should report the same error (not run).
- **CLI and the codec.** The CLI never downloads by itself. `send`/`receive` find a library from
  `COMETCAST_OPENH264_LIB`, an earlier download, or a **decompressed** file in `COMETCAST_CODEC_DIR`; a
  folder that holds only Cisco's `.bz2` file is not enough for them. Run `cometcast-cli codec install --yes` once
  (release variant only): it prints the host, file and size, then imports the `.bz2`/raw file from the
  administrator folder or downloads it, with progress on stderr; without `--yes` (or
  `COMETCAST_CODEC_AUTO=1`) it only explains and exits with 2. `cometcast-cli codec status` shows whether the
  library is available (exit 0 only if so). Builds with `h264-source` answer that they need no download.
- **macOS hardened runtime (UNCERTAIN, not tried):** the Developer ID build may refuse to load Cisco's
  dylib (library validation), see "Hardened runtime and library validation" in `docs/releasing.md`.
- **CI:** besides the job `release-variant`, the job `check` runs `cargo test -p cc-codec --features download`,
  clippy for it, and `cargo check -p cc-ffi --no-default-features --features h264-libloading`, so the download
  path is compiled in CI. The job `release-variant` in `.github/workflows/rust.yml` builds that release variant, fails if the
  dependency graph contains an OpenH264 source feature, and fails if the binary does not refuse to start the
  codec when `COMETCAST_OPENH264_LIB` is unset. The other CI job still builds and tests with the development
  default.
- Tried in an earlier session (not repeated for this revision): Cisco's `libopenh264-2.6.0-linux64.8.so` was downloaded from `ciscobinary.openh264.org`,
  its SHA-256 matched the crate's list, and the encoder/decoder round trip and a full CLI send/receive run
  worked with it (at that time `cargo build -p cometcast-cli --features h264-libloading`, which still included the source build; `COMETCAST_OPENH264_LIB=...`).
### Runtime codec (download on first use)

Releases use `h264-libloading` (owner's decision: Cisco's prebuilt binary, never OpenH264 built from
source in a release). A normal user cannot set environment variables, so the program follows the model of
Firefox: it downloads Cisco's library once, after the user agreed, and checks it. The code is the crate
`cc-codec` (`crates/cc-codec`); `H264Backend::from_env()` in `cc-media` uses it.

**Where the library is looked for, in this order:**

1. `COMETCAST_OPENH264_LIB`: path of a library (developers). The file must exist (otherwise the codec is
   reported as failed, not ready); `cc-codec` does not hash it, the `openh264` crate checks the hash when it
   loads the file.
2. The per-user folder: `$XDG_DATA_HOME` or `~/.local/share` (Linux), `~/Library/Application Support` (macOS),
   `%LOCALAPPDATA%` (Windows), each plus `CometCast/codecs/openh264-2.6.0/`. On Android the app passes its
   private files directory (`set_codec_dir`, from `lib/codec_setup.dart`). This is where downloads and imports
   end up, and the only place a library is loaded from (besides 1).
3. `COMETCAST_CODEC_DIR`: a folder that an administrator fills (networks without Internet access, schools). It
   must be an **absolute** path (a relative one is an error shown in the UI). It must contain the file under its
   **exact name** from the table below, either decompressed or exactly as Cisco serves it (name plus `.bz2`).
   **The file is never loaded in place.** `cc-codec` reads it (at most 8 MiB, or 4 MiB for the `.bz2`), checks
   the SHA-256 of the decompressed bytes, writes them into the per-user folder (0700, atomic), reads that file
   back, checks the hash again and returns the path of the copy. A decompressed file is copied by every lookup
   (`find_library`, so also by `cometcast-cli`); a `.bz2` file needs the decompressor and is imported only by
   `ensure_codec` (the app's setup, or `cometcast-cli codec install`). A wrong or oversized file there does not
   stop the download from Cisco (the error is added to the message if the download fails too); a wrong
   *folder* setting (relative path, see below) does.
4. Otherwise the download below.

**Who may own what (Unix):** the administrator folder and the file in it must belong to the current user or to
root and must not be writable by group or others, otherwise they are refused (message: file name only). The
per-user folder is created with mode 0700; if it exists it must belong to the current user and its mode is set
to 0700; the library in it must not be writable by others either. **UNCERTAIN: Windows** has no such check
(ACLs are not read; nothing was written or tried there), only the hash checks apply. **Residual risk
(UNCERTAIN):** after the last hash check of the copy and before `openh264` opens it, a process of the same
user (or root) could replace the file; the private folder keeps other users out, not the user's own
processes. `openh264-sys2` reads the file and hashes it again itself when loading (read from its source, not
tried with a swapped file), so the window is small but exists: hashing and loading are two opens of the path.

A symlink in the per-user folder is refused (the administrator's file may be a symlink; its content is
copied and hashed). **Version required: OpenH264 2.6.0**, because the pinned `openh264-sys2` 0.9.9 in Cargo.lock accepts
exactly these builds:

| Platform | File name (also the name inside `COMETCAST_CODEC_DIR`) | Compressed size |
| --- | --- | --- |
| Linux x86_64 | `libopenh264-2.6.0-linux64.8.so` | 634264 |
| Linux aarch64 | `libopenh264-2.6.0-linux-arm64.8.so` | 619371 |
| Windows x86_64 | `openh264-2.6.0-win64.dll` | 452053 |
| Windows aarch64 | `openh264-2.6.0-win-arm64.dll` | 378234 |
| macOS arm64 | `libopenh264-2.6.0-mac-arm64.dylib` | 482124 |
| macOS x86_64 | `libopenh264-2.6.0-mac-x64.dylib` | 529614 |
| Android arm64-v8a | `libopenh264-2.6.0-android-arm64.8.so` | 496662 |
| Android armeabi-v7a | `libopenh264-2.6.0-android-arm.8.so` | 540728 |
| Android x86_64 | `libopenh264-2.6.0-android-x64.8.so` | 562893 |

Not offered: 32-bit x86 and Linux ARM32 (not a target), iOS and tvOS (the library cannot be loaded at run
time there; the platform encoders would be used, not implemented here). The macOS file is chosen by the
architecture the program was compiled for. The names and SHA-256 values are copied from `src/blobs/hashes.txt` of
`openh264-sys2` 0.9.9 into `crates/cc-codec/src/table.rs`; **when `openh264` is updated in Cargo.lock, copy the
new table by hand** (the compressed sizes are only shown to the user).

**Download:** `https://ciscobinary.openh264.org/<name>.bz2`, HTTPS only (`ureq` 3 with rustls/ring and the
webpki root store, no OpenSSL; proxy settings of the environment are honoured), **no redirects at all**, timeouts
(15 s to connect, 30 s without any data = stall, 10 minutes in total per attempt, so a slow link can finish as
long as bytes keep arriving), 3 retries with backoff (2, 4, 8 s) for network errors and HTTP 5xx/408/429,
compressed size at most 4 MiB, decompressed size at most 8 MiB (the real files are 0.4 to 0.7 MB and 0.8 to
1.6 MB), progress reported to the UI. The file is recognised by its magic bytes (bzip2, or ELF/PE/Mach-O as
is), decompressed with the pure-Rust `bzip2-rs` (MIT/Apache-2.0), and the SHA-256 of the **decompressed**
library must equal the pinned value before anything is written. Writing is atomic: a temporary file in the same
folder (created exclusively, so a planted file or symlink is not followed), `fsync`, rename, `fsync` of the
folder; the folder is 0700 and the file 0755 (0555 on Android); a symlink as folder or target is refused.
Leftover `.name.pid.n.tmp` files of an interrupted installation that are older than one day are deleted
when the next installation starts. A panic in the download thread ends the "downloading" state with a failure
message (the UI cannot hang on it).

**Consent:** the Flutter app shows a card on the receive screen and, when "Share this screen" is on, on the
send screen (not on Android, where MediaCodec encodes): what the codec is, that it is Cisco's, the host, the
file name and the size, a "Download" button, a progress bar, a "Try again" button after a failure and a hint for
offline networks (the administrator folder and the file name). The first press is remembered in the settings
as `host|version` (`ciscobinary.openh264.org|2.6.0`); a new OpenH264 version or another host asks again, and
the boolean that the first release stored is not read (one more question after the update). Later downloads of
the same host and version start without asking. If the core says "downloading" (another screen or process owns
the download), the card follows it until it is ready, failed or missing, on both screens. An administrator can set `COMETCAST_CODEC_AUTO=1` to skip the question
(not remembered as consent). Nothing is ever downloaded before one of the two. The web app does not use any of
this (browser codecs). Development and CI builds with `h264-source` contain the codec and never ask.

**Android (UNCERTAIN, not tried):** the library is stored in the app-private directory and loaded with
`dlopen`. Android 10 and later forbid *executing* files from the app's data directory, which concerns
`exec`, not loading a shared library; whether `dlopen` of a downloaded `.so` from there works on every device
and for target SDK 34+ (where dynamically loaded files must be read-only, hence mode 0555) was not tested. A
bigger problem is **Google Play policy**, which does not allow an app distributed through Play to download
executable code from elsewhere. For a Play release the codec would have to be bundled by Cisco's own channel
or replaced by MediaCodec (the Android sender already uses MediaCodec; only receiving on Android needs the
decoder). Decide before a Play release; sideloaded builds are not affected.

**What was checked (2026-11-07, once, on one machine, Linux x86_64):** all nine files were downloaded from Cisco with HTTP 200 and
no redirect, are bzip2 (`BZh`), and their decompressed SHA-256 values equal the pinned table; the real path
(`ensure_codec` into a temporary folder, then `find_library`) and `cargo test -p cc-media --no-default-features
--features h264-libloading` with only `COMETCAST_CODEC_DIR` set (no `COMETCAST_OPENH264_LIB`) pass: the
encoder and decoder work through the managed library. Not checked: Windows, macOS, Android, Linux aarch64
(only downloaded and hashed, never loaded), a school network with a TLS-inspecting proxy (webpki roots do not
trust the proxy's CA; the download then fails with a TLS error and the offline hint applies; adding the system
roots is an open improvement).


- Hardware encoders: only the trait `HardwareEncoderProbe` exists (run the test in a child process with a
  time limit, store the result together with a GPU/driver fingerprint, fall back to OpenH264 on failure).
  No hardware encoder is implemented. The 1280x720 test size and 30 s limit are placeholders.

## Tests and what they prove

| Test | Command | Proves |
| --- | --- | --- |
| Change detection | `cargo test -p cc-capture` | A changed byte anywhere changes the hash; equal pictures are "unchanged"; the stubs return `Unsupported` |
| X11 capture under Xvfb | same (starts a private Xvfb; skips with a note if `Xvfb` is missing) | A window of a known colour is read with the right colour at the right place; many `Unchanged` calls while nothing happens; a colour change gives a frame again; DAMAGE is in use; pacing about right |
| Encoder / decoder | `cargo test -p cc-media` | Encode a solid frame, decode it, mean colour within 12 of the original; IDR on request only; SPS is Baseline; odd sizes cropped; RTP reassembly incl. FU-A and lost packets; static refresh timing |
| Same through Cisco's binary | `COMETCAST_OPENH264_LIB=<file> cargo test -p cc-media --no-default-features --features h264-libloading` (or `COMETCAST_CODEC_DIR=<folder with the exact file name>`) | The prebuilt library works through the same API; an unknown file is refused |
| Codec manager | `cargo test -p cc-codec --features download` (`-- --ignored` adds two tests that download from Cisco) | Hash check (tamper), platform table (and that it equals `hashes.txt` of `openh264-sys2`, found with `cargo metadata`; skipped loudly if the crate is not on disk), bzip2/raw sniffing, size caps and a decompression bomb, atomic write, symlink refusal, owner and mode checks, copy of the administrator file into the private folder, no redirects, https only, retries, stall timeout versus a slow link, a panic during a download; local HTTP server with a fake library |
| CLI codec command | `cargo test -p cometcast-cli --no-default-features --features h264-libloading` | `codec status`/`install` parse, need `--yes`, report a missing, existing and nonexistent `COMETCAST_OPENH264_LIB` |
| Screen pipeline with a scripted source | `cargo test -p cc-sender` | First frame is IDR; no capture while a frame is in flight; a static screen gives exactly one picture plus 3 refreshes and then silence; a key frame request is answered on a static screen. With a scripted encoder that skips pictures: 40 skipped encodes after the first frame still end with the 3 refreshes sent; a skipped new picture is retried until sent; a key frame request survives 5 skipped attempts and is cleared only when the key frame went out; 1000 requests within half a second give one accepted request and 999 counted as ignored; a panic in the capture thread is reported; `stop` returns within its time limit while the capture call is stuck |
| RandR change of a monitor | `cargo test -p cc-capture` (`a_monitor_source_follows_a_randr_change`) | Under Xvfb a second client gives the CRT controller a 400x300 mode; the `monitor:0` source then delivers 400x300 frames (without listening to RandR events it stayed at the old rectangle and delivered nothing; checked by disabling the event handling once) |
| End to end | `cargo test -p cc-receiver --test screen` | Real `cc-server`, Xvfb with a full-screen red window, `SenderCore` with the X11 source, `ReceiverCore` with the packet tap; the received H.264 is reassembled and decoded with OpenH264; the decoded picture is red, 640x480; after the window turns blue the decoded picture turns blue; then the sender produces no frames while the screen is static; a PLI from the receiver is answered with a key frame |
| Browser receiver | `cd apps/web && npm run build && npm run e2e:screen` | Chromium (playwright's, version 153.0.8010.12) receives `cometcast-cli send --screen` from an Xvfb screen and decodes it at 640x480; the canvas mean colour is close to the window colour, and follows a second window of another colour. The script checks `RTCRtpReceiver.getCapabilities('video')` first and prints `SKIPPED` (not OK) if the Chromium has no H.264 |

The existing `e2e`, `e2e:m2`, `e2e:native` and `e2e:open` scripts were run again and pass.

## Numbers seen here

All from this one virtual machine (4 cores), 1280x720 Xvfb unless noted, 15 fps. Not benchmarks; no
repetition beyond what is written; the machine was shared with other work.

**Screen end-to-end test (`tests/screen.rs`, 640x480, window red then blue, then static)**, counters of the
sender at the start and the end of the 2.5 s quiet window (three runs of this revision gave the same totals; only
`keyframe_requests` varied between 1 and 2 at the start, depending on how many picture loss requests the
receiver sent):

```
polls: 25 -> 63, changed: 2 -> 2, unchanged: 22 -> 60, encoded: 5 -> 5, refreshes: 3 -> 3, keyframes: 2 -> 2, skipped: 0
received access units: 5 (+0 during the quiet window); after a key frame request: encoded 6, keyframes 3
```

So during 2.5 s of a static screen the screen was looked at 38 times and nothing was encoded or sent. The 5
frames are the red picture, the blue picture (an IDR frame, because the encoder sees a scene change) and 3
refreshes. (An earlier revision of this page printed 6 encoded frames and 3 key frames at this point; that run
had counted I frames as key frames and had an extra key frame request, so the numbers are not comparable.)

**CPU of the `cometcast-cli send --screen` process** over 10 s (`/proc/<pid>/stat` ticks, 100 ticks = one core
second; window of 10 s starting 3 s after the sender started), together with the CPU of Xvfb and of
`receive --decode` over the same 10 s. Re-measured with this revision, release-like build (`opt-level=3`, no LTO,
default dev features; one run per row, 1280x720 Xvfb, 15 fps, 2 Mbit/s, loopback):

| Content | Sender | Xvfb | Receiver with decoding | Last progress line of the 16 s run |
| --- | --- | --- | --- | --- |
| Static (red xterm window) | 1 tick (0.1 % of a core) | 0 | 4 | 4 frames, 3264 bytes |
| Small 200x120 window moved 30 times a second | 123 ticks (12 %) | 26 | 33 | 240 frames, 44,844 bytes |
| Whole screen repainted 30 times a second | 125 ticks (12.5 %) | 24 | 39 | 237 frames (236 key frames: the scene-change detector), 657,767 bytes |

Earlier measurements on this page (4 / 168 / 150 ticks and the debug-build numbers) were from another run of the
same shared machine and are replaced by the table; the debug-build numbers were **not** re-measured and are not
quoted any more. Sender memory (RSS) was not re-measured (an earlier run saw 45 to 66 MB). Where the CPU goes
(the 3.6 MB `GetImage` copy, hashing, colour conversion, encoding, RTP) was not profiled.

**What a repeated still picture does with this encoder setup.** A unit test
(`repeated_encoding_of_a_still_picture_is_measured`, run with `--nocapture`, same output in two runs) encodes the
same synthetic 640x360 picture six times and decodes each result. **The test picture is random noise, not text**:
6x10 pixel blocks in which two thirds of the blocks are "inked" with 40 % black pixels at random positions on
light grey. It is a worst case for a video encoder and says little about real text. PSNR is computed over the red
channel only (the picture is grey).

```
300000 bit/s, frame skip on : sizes [119578, 0, 0, 96465, 96465, 96465]  PSNR [30.3, 25.6, 25.6, 25.6] dB
300000 bit/s, frame skip off: sizes [119578, 13, 13, 96465, 96465, 96465] PSNR [30.3, 30.3, 30.3, 25.6, 25.6, 25.6] dB
2000000 bit/s, frame skip on : sizes [138214, 0, 12, 110505, 106201, 103167] PSNR [33.0, 33.0, 28.9, 28.6, 27.9] dB
2000000 bit/s, frame skip off: sizes [138214, 12, 13, 110505, 106201, 103167] PSNR [33.0, 33.0, 33.0, 28.9, 28.6, 27.9] dB
(an IDR frame is forced for the 4th to 6th picture; a size of 0 = nothing came out, no PSNR for that picture)
```

What this shows, for this one noise picture: repeating the unchanged picture gives nothing (0 bytes) or 12 to 13
byte skip frames and the decoded picture stays exactly as bad as the first one (no sharpening). A forced IDR on
the same still picture gave a **worse** picture than the first frame in all four settings (30.3 -> 25.6 dB at
300 kbit/s; 33.0 -> 28.9, 28.6, 27.9 dB at 2 Mbit/s), with smaller frames. **Side effect for the product:** answering
a PLI on a static screen (a browser asked about every 4 to 5 seconds in the e2e run below) replaces a good
picture with a worse one, at least with this noise picture and these settings. **Untested hypothesis:** that
the forced IDR is blurrier because the rate control reacts to the budget overspent by the large first frame; no
experiment here separates this from other causes (for example the encoder choosing a higher quantiser for
IDR frames). The picture quality seems to be decided by the first frame. The helper for static refresh is kept
because its frames are tiny, but the idea "re-encode so the encoder sharpens the still" did not work by itself
here. **Uncertain:** options such as a lower quantiser for the first frame of a static picture, a short burst
of higher bit rate, or avoiding the forced IDR for a PLI when the last IDR is recent, were not tried; real
text (not noise) was not tried either.

**Browser run (`npm run e2e:screen`, run again with this revision: the same colours and 6 key frame requests in about 25 s, each answered by a key frame of about 1 KB)**: Chromium 153.0.8010.12 lists eight H.264 receive variants including
`packetization-mode=1;profile-level-id=42e01f`, decoded the stream at 640x480, and showed mean colour
(212, 47, 39) for a window of (200, 30, 40) and (13, 81, 195) for (20, 80, 200), i.e. up to 17 off per channel.
**Uncertain:** the offset is probably the colour matrix and range (no VUI information is written into the
stream, so the decoder assumes one); not investigated. While the screen was static the browser asked for a
key frame about every 4 to 5 seconds (`key_requests` rose to 5 or 6 in 25 s) and the sender answered each
with a key frame of about 1 KB; why the browser asks that often was not investigated.

## Limits

- **X11 only.** Wayland (desktop portal + PipeWire), Windows and macOS return `CaptureError::Unsupported`
  from stubs. Other X11 limits: only 32-bit-per-pixel little-endian BGRX roots (anything else is refused
  with a message); the root window and RandR monitors, no single windows; no hot-plug handling beyond
  following the size of the root; no multi-screen (Zaphod) setups.
- **No mouse pointer.** `GetImage` does not contain it. Sending it as separate messages (A10) is not done.
- **No audio.**
- **No dirty rectangles.** The whole picture is hashed, read and encoded; DAMAGE is only used as "something
  changed" hint. A 64-bit hash collision could in theory hide a change.
- **`GetImage` copies through the X connection.** MIT-SHM would be faster but needs shared memory handling
  with `unsafe`, which the workspace forbids.
- **One receiver per sender.** The core still has one peer; "one frame in flight per receiver" is
  therefore one frame in flight. No bit-rate adaptation: the encoder runs at the configured rate and
  the browser's bandwidth feedback is not used.
- Resolution changes of the root: the capture follows the new size and the encoder restarts at the new
  size on its own; this was **not** tested. Monitor sources (`monitor:N`) listen for RandR screen change
  events and look their rectangle up again; if reading the old rectangle fails they look it up once more and
  retry once; a monitor that is gone ends the capture with an error. Tested under Xvfb with one CRT controller
  whose mode changes (see the tests table), not with several monitors or a real driver.
- The pipeline ends (error event) if capture fails, panics or cannot start; there is no retry.

## What is not verified

- Any other machine, GPU, X server (Xorg, XWayland), desktop environment, or a real compositor.
- Real content such as slides, code or video, real network conditions (everything ran over loopback UDP),
  latency (no number), image quality beyond the PSNR experiment above.
- CPU on weak school PCs; the numbers above are from one shared VM.
- Browsers other than the one Chromium named; Safari and Firefox H.264 behaviour; hardware decoders.
- The licensing questions above.
- Whether `h264-libloading` works on Windows/macOS/Android (the crate lists the library names; the download path was only run on Linux x86_64).
