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

  At run time `COMETCAST_OPENH264_LIB` must point to Cisco's `libopenh264` (SHA-256 on the crate's list). Checked
  here for this command (debug profile, same features): the binary contains no OpenH264 code (`nm -C | grep
  -c WelsEnc` gives 0, against 807 matches for `WelsEnc` and 466 for `WelsDec` in the default build);
  `cargo tree -e features -i openh264` shows only `openh264 feature "libloading"`; and with the variable unset
  `cometcast-cli receive --decode` exits with status 1 and the message "OpenH264 backend not available: this
  program was built without the h264-source feature (no OpenH264 source build); set COMETCAST_OPENH264_LIB to
  the path of Cisco's libopenh264 (h264-libloading)". `send --screen` creates the same encoder, so it should report the same error (not run).
  **Not run here:** a round trip with Cisco's library in the new feature layout (downloading and loading a
  prebuilt binary was not allowed in this session); the earlier run with `--features h264-libloading` is
  described below.
- **CI:** the job `release-variant` in `.github/workflows/rust.yml` builds that release variant, fails if the
  dependency graph contains an OpenH264 source feature, and fails if the binary does not refuse to start the
  codec when `COMETCAST_OPENH264_LIB` is unset. The other CI job still builds and tests with the development
  default.
- Tried in an earlier session (not repeated for this revision): Cisco's `libopenh264-2.6.0-linux64.8.so` was downloaded from `ciscobinary.openh264.org`,
  its SHA-256 matched the crate's list, and the encoder/decoder round trip and a full CLI send/receive run
  worked with it (at that time `cargo build -p cometcast-cli --features h264-libloading`, which still included the source build; `COMETCAST_OPENH264_LIB=...`).
- Hardware encoders: only the trait `HardwareEncoderProbe` exists (run the test in a child process with a
  time limit, store the result together with a GPU/driver fingerprint, fall back to OpenH264 on failure).
  No hardware encoder is implemented. The 1280x720 test size and 30 s limit are placeholders.

## Tests and what they prove

| Test | Command | Proves |
| --- | --- | --- |
| Change detection | `cargo test -p cc-capture` | A changed byte anywhere changes the hash; equal pictures are "unchanged"; the stubs return `Unsupported` |
| X11 capture under Xvfb | same (starts a private Xvfb; skips with a note if `Xvfb` is missing) | A window of a known colour is read with the right colour at the right place; many `Unchanged` calls while nothing happens; a colour change gives a frame again; DAMAGE is in use; pacing about right |
| Encoder / decoder | `cargo test -p cc-media` | Encode a solid frame, decode it, mean colour within 12 of the original; IDR on request only; SPS is Baseline; odd sizes cropped; RTP reassembly incl. FU-A and lost packets; static refresh timing |
| Same through Cisco's binary | `COMETCAST_OPENH264_LIB=<file> cargo test -p cc-media --no-default-features --features h264-libloading` | The prebuilt library works through the same API; an unknown file is refused |
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

**Browser run (`npm run e2e:screen`)**: Chromium 153.0.8010.12 lists eight H.264 receive variants including
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
- Whether `h264-libloading` works on Windows/macOS (the crate lists the library names; not tried).
