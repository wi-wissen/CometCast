# Miracast receiving (Windows 10 and 11)

Milestone M5. Crates: `crates/cc-miracast` (receiver), `crates/cc-miracast-ffi` (C interface for the Flutter app). Flutter: `apps/flutter/lib/miracast_page.dart`, `miracast_controller.dart`, `core/miracast.dart`. CLI: `cometcast-cli miracast [--name NAME]` (events and audio only, no picture). Test plan for a person with a Windows PC: [miracast-device-test-plan.md](miracast-device-test-plan.md).

**State in one sentence: built to specification, never run on Windows.** Everything that does not touch the Windows API is tested on Linux (policy, connection engine, consent, frame mailbox and scaler, diagnosis, the C interface logic, the Dart state machine and screens, and the Dart binding against the real Rust library). Everything that calls Windows (`MiracastReceiver`, `MediaPlayer`, Direct3D) is compile- and clippy-checked for `x86_64-pc-windows-gnu` and linked into a real `cc_miracast_ffi.dll` there, but nobody has seen a Miracast sender connect to it. Every statement below about run-time behaviour is **Uncertain** until [miracast-device-test-plan.md](miracast-device-test-plan.md) has been done. This page belongs to measurement assumption A4 and measurement point 5 in [measurement-points.md](measurement-points.md).

## How it is meant to work

A Windows PC runs Comet Cast and opens "Receive via Miracast (Windows)". An Android phone ("Cast", "Smart View") or a Windows laptop (Windows key + K) sees the PC in its list and connects. The person at the PC is asked, with the sender's name, whether the screen may be shown. After "Allow", the sender's screen and audio appear in the app window. No app is needed on the sender.

```text
Flutter UI (Dart)        miracast_page.dart  <-  miracast_controller.dart  (state machine, polls every 16 ms / 100 ms)
      | dart:ffi (polled; JSON for state, raw BGRA for pictures)
cc_miracast_ffi.dll      crates/cc-miracast-ffi: Controller (lifecycle on own threads, request, counters) + abi.rs
      |
cc_miracast (Rust)       Engine (consent, one session, timeout, status)   <- platform independent, tested on Linux
      |                  windows_backend.rs: MiracastReceiver, session, handlers
      |                  player.rs: MediaPlayer (frame server) -> Direct3D 11 texture -> SoftwareBitmap -> BGRA -> mailbox
Windows.Media.Miracast / Windows.Media.Playback
```

## Design: why a Rust DLL polled by Dart (and not a native Flutter texture plugin)

The question was how to get the pictures of the incoming `MediaSource` into the Flutter window. Options considered:

| Option | Verdict |
| --- | --- |
| A. C++/WinRT plugin in `apps/flutter/windows/runner` with a Flutter `TextureRegistrar` (pixel buffer or GPU surface texture, `CopyFrameToVideoSurface` into a shared DXGI texture) | **Best performance** (a GPU surface texture needs no CPU copy), but: a second language in the app, C++ that the runner builds with `/W4 /WX`, and it cannot be compiled or even type-checked here (no MSVC, no Windows SDK). It would be a large block of never-compiled code |
| B. **Rust `windows`-crate DLL, polled by Dart, BGRA pictures through a pull mailbox** | **Chosen.** One language for all Windows code; it is the same pattern as the existing video path (`crates/cc-ffi/src/video.rs`: one slot, newest wins, Dart pulls); the logic around it is testable on Linux and the Windows calls are type-checked and linked to a real DLL on the gnu target. Cost: a GPU-to-CPU-to-Dart copy per picture |
| C. `MediaPlayerElement` / a platform view / XAML island | Needs a XAML host or a native child window in Flutter's view; not possible with Flutter's public Windows embedder API without C++, and unverifiable here |
| D. Undocumented `WiFiDisplay.dll` | Only a reserve named in the plan if `MiracastReceiver` fails unpackaged. **Not implemented.** Would need its own legal and technical review |

Why a separate DLL and not part of `cc-ffi`: `cc-ffi` is the flutter_rust_bridge facade of the cross-platform core (WebRTC, H.264). The receiver is Windows-only, needs the `windows` crate and fails in different ways (the optional feature can be missing, the adapter can lack support). A separate library can be missing or fail to load without touching the rest of the app: the Miracast screen then shows a translated hint "component not found" and everything else works.

**Upgrade path if the copy is too slow (not built):** replace step "SoftwareBitmap to BGRA mailbox" by a shared Direct3D texture and a native Flutter `GpuSurfaceTexture` in option A. The Dart side (`MiracastController`) and the C interface would stay; only the picture transport changes. Whether that is needed is a measurement (test plan part E).

### Video path in detail

1. `MediaSourceCreated` fires for an accepted connection. The handler takes the event's `Deferral` (UNCERTAIN, see below), attaches the `MediaSource` to a `MediaPlayer` and calls `Play()`, then completes the deferral. So the source is attached before Windows continues.
2. The `MediaPlayer` has `IsVideoFrameServerEnabled = true` (frame-server mode): it decodes and plays the **audio** itself, but does not draw the picture. `RealTimePlayback` is set to ask for low latency (UNCERTAIN: effect not measured).
3. For every `VideoFrameAvailable` (at most one per 15 ms; a frame that arrives while the previous one is being copied is skipped), `CopyFrameToVideoSurface` copies the picture into a `B8G8R8A8` Direct3D 11 texture of the natural video size (created on a hardware device, WARP as fallback; recreated when the size changes). `SoftwareBitmap::CreateCopyFromSurfaceAsync` reads it back, `CopyToBuffer` and a `DataReader` move the bytes into a `Vec<u8>`.
4. The picture is scaled down (nearest neighbour, aspect ratio kept, never enlarged) to at most 1920 x 1080 (`cc_miracast_set_max_size` changes this) and published to the mailbox. A picture the UI has not taken yet is replaced and counted as `dropped`, so memory is bounded however slow the UI is.
5. Dart calls `cc_miracast_poll_frame` every 16 ms while connected, copies the picture out and draws it with `decodeImageFromPixels` and `RawImage`.

Costs (not measured): a 1080p picture is 8.3 MB; at 30 pictures per second that is about 250 MB/s moved through RAM several times (GPU read-back, scale, FFI copy, Dart copy). On a weak mini PC this may limit the frame rate. The statistics line on screen (picture size, pictures skipped) and the counters in the state JSON exist to measure exactly this.

**Audio** is played by the `MediaPlayer` to the default output device; the screen has a mute button (`cc_miracast_set_muted`). UNCERTAIN: audio latency and lip sync were not observed.

## Rules (all tested on Linux except where marked)

### Consent and privacy

* **Every foreign device needs a confirmation by a person.** There is no "accept all" setting and no input that makes the engine accept without asking (`ConnectionPolicy`, `Engine`). This holds **assuming the `Deferral` really keeps the connection waiting** (test plan item F3, **Uncertain**). If Windows gives no `Deferral`, the connection is refused (fail closed) and reported as `ConnectionRejected { ConfirmationUnavailable }`.
* **Nothing is shown or played before the answer.** The player only starts after the connection is accepted (`MediaSourceCreated` for a connection that is not active is refused and disconnected). The Dart video view is only built while the status is `connected`, and the controller does not even fetch a picture in any other state (tested).
* The consent dialog shows the sender name (cleaned) and, if Windows gave one, the PIN. "Decline" has the keyboard focus first, so an accidental Enter key does not share a screen. The dialog closes by itself when the request goes away (30-second timeout counts as "no", or the sender left).
* The sender's name is attacker-controlled text. `sanitize_device_name` removes control, format, bidi and zero-width characters, collapses white space (so no fake extra lines), cuts it to 64 characters and rejects names that are empty afterwards. Only the cleaned name reaches the dialog.
* **At most one session** (active or waiting). A second sender is rejected; the rejection is the informational event `ConnectionRejected`, reported at most once every 5 seconds per reason, never changes the status (so a nearby second sender cannot knock an active receiver into `Failed`); the screen shows a small note with the count.
* **The input back-channel (UIBC) is not enabled.** The code never touches `InputDevices`, game controllers or the cursor channel, so nothing from the sender can control this PC. The screen says so.
* The CLI reads the answer from stdin only when stdin is a terminal and rejects every request at once when stdin is piped.
* Constants (name length 32, timeout 30 s, one session) are guesses or product decisions. **Uncertain:** the real limits of Windows were not measured.

### Pairing modes (`PairingPolicy`)

| Mode in the UI | Windows value | Meaning |
| --- | --- | --- |
| Ask me for every connection (default) | `ConfirmConnection` | Our dialog only |
| Ask me, and show a PIN if the sender asks for one | `PinDisplayIfRequested` | Our dialog; a PIN is shown in it if Windows gives one |
| Ask me, and always require a PIN | `PinDisplayRequired` | Our dialog; the PIN must be typed on the sender |

`RequireAuthorizationFromKnownTransmitters` is always set to true (UNCERTAIN: assumed to make Windows ask again for devices that connected before). **Uncertain, and the point of measurement 5:** which side draws the PIN (Windows or our app), when `Pin` is non-empty, and whether Windows shows an additional prompt of its own on top of ours (test plan part F).

### Name

The receiver name (Windows `FriendlyName`, shown in the sender's list) defaults to the host name cut to 32 characters and can be changed on the screen before starting. Windows has the final say (`FriendlyNameTooLong`).

### Requirements and hints on the screen

The crate never produces user-facing text. It reports an `Issue` (data); the Flutter screen shows a translated hint (English and German, formal "Sie").

| Situation | How it is detected | Hint shown |
| --- | --- | --- |
| Not Windows | `Unsupported` on other targets (the entry is not even offered there) | Only Windows 10 or 11 |
| Optional feature "Wireless Display" missing | `MiracastReceiver` could not be created and the feature marker file was not seen; or Windows said "not supported" and a check found the feature missing | Settings path (System, Optional features, Add an optional feature, "Wireless Display") and the DISM capability `App.WirelessDisplay.Connect~~~~0.0.1.0`; worded as "may be missing" |
| Adapter or driver cannot receive Miracast | `MiracastReceiverWiFiStatus.MiracastNotSupported`, or `Unsupported` from `DisconnectAllAndApplySettings` or `Start` | Update the Wi-Fi driver or use another adapter |
| Adapter supported but not optimised | `MiracastReceiverWiFiStatus.MiracastSupportNotOptimized` | Warning: poor picture likely |
| Policy | `MiracastReceiverListeningStatus.DisabledByPolicy` | Ask the administrator |
| Temporarily unavailable | `TemporarilyDisabled` | Check Wi-Fi, other apps |
| Access denied | `E_ACCESSDENIED` in an error text | May need packaging |
| DLL missing or other version | library cannot be loaded, or `cc_miracast_abi_version` differs | Reinstall |

**Uncertain:** (1) The marker file for "feature installed" is `%SystemRoot%\System32\WirelessDisplay.exe`, written from memory. A missing file proves nothing, so the code only ever reports "installed" or "could not tell" and never "not installed"; the hint is therefore worded as "may be missing" and appears only after an actual failure or a clear Windows verdict. (2) The Settings menu path and the capability name `App.WirelessDisplay.Connect~~~~0.0.1.0` are from memory of the Microsoft documentation; check with `DISM /Online /Get-CapabilityInfo /CapabilityName:App.WirelessDisplay.Connect~~~~0.0.1.0` (test plan D1). The German name of the feature in the Windows UI is unknown to us. (3) Whether the feature is needed for the receiver role at all is open (the plan lists it as a question).

**Firewall:** Miracast uses Wi-Fi Direct and an RTSP connection (TCP port 7236 is typical). The Windows firewall may prompt for an unpackaged program or block it silently. The screen carries a note; the exact behaviour (prompt, profile, ports) is test plan part G. **Uncertain.**

### Side effects on the machine (all **Uncertain**)

* `DisconnectAllAndApplySettings` disconnects **every** Miracast receiver session on the machine, including sessions of other applications, not only ours. Starting the receiver can therefore interrupt another app's Miracast session.
* It may change `FriendlyName`, `AuthorizationMethod` and `RequireAuthorizationFromKnownTransmitters` persistently, i.e. beyond the life of our process. On drop the backend remembers the values it read at start and tries to put them back with a second `DisconnectAllAndApplySettings` (best effort, errors ignored; this too disconnects everything). Whether Windows persists the settings and whether the restore works was not measured; check the receiver name and authorization setting in Windows after a run.

## C interface (`cc_miracast_ffi.dll`)

Everything is polled; no callbacks cross the boundary. Return codes: `>= 0` success (some return a length or 1/0), negative: `-1` busy, `-2` bad argument, `-3` buffer too small, `-4` not running, `-5` no such request, `-6` Windows refused, `-7` panic caught. The Dart side is `apps/flutter/lib/core/miracast.dart`.

| Function | Purpose |
| --- | --- |
| `cc_miracast_abi_version() -> u32` | Currently 1. Dart refuses another value |
| `cc_miracast_alloc(len) -> *u8`, `cc_miracast_free(ptr, len)` | Memory for the buffers below (so Dart needs no `ffi` package) |
| `cc_miracast_probe(buf, cap) -> len` | JSON: feature, adapter, receiver available, issue. Starts nothing, shows nothing |
| `cc_miracast_start(json, len)` | `{"name":"...","pairing":"confirm\|pin_if_requested\|pin_required"}`. Returns at once; the receiver is created on a thread of its own (so the UI thread never waits for Windows and never joins a COM apartment) |
| `cc_miracast_state(buf, cap) -> len` | JSON: status, issue, generation, pending request (id, name, pin), connected name, last error, counts, picture counters |
| `cc_miracast_respond(id, accept)` | Answers the waiting request; refused if `id` is not the waiting one |
| `cc_miracast_disconnect()` | Ends the active connection, keeps listening |
| `cc_miracast_stop()` | Disconnects everything, restores the Windows settings, on a thread of its own |
| `cc_miracast_poll_frame(buf, cap, info) -> 1/0/-3` | Newest picture as BGRA, rows without padding; `-3` leaves it in the mailbox and reports the size needed |
| `cc_miracast_set_max_size(w, h)`, `cc_miracast_set_muted(m)` | Picture size limit, audio mute |

Unsafe code: only `crates/cc-miracast-ffi/src/abi.rs` (raw pointers) and one `RoInitialize` call plus the Direct3D calls in `cc-miracast` (`windows_backend.rs`, `player.rs`), each with a SAFETY comment. Every body runs in `catch_unwind`.

## Windows API used (from the Microsoft reference, from memory; all **Uncertain** until run)

| Call | Used for | Uncertain |
| --- | --- | --- |
| `MiracastReceiver::new`, `GetStatus` | Probe: `ListeningStatus`, `WiFiStatus` | That `GetStatus` works before a session exists and without the feature |
| `GetCurrentSettings` / `GetDefaultSettings`, `FriendlyName`, `AuthorizationMethod`, `RequireAuthorizationFromKnownTransmitters`, `DisconnectAllAndApplySettings` | Name and pairing | Machine-wide side effects, see below |
| `CreateSession(None)`, `AllowConnectionTakeover = false`, `MaxSimultaneousConnections = 1`, `Start` | Session | `None` for the `CoreApplicationView` in an unpackaged app (assumption A4) |
| `ConnectionCreated` (`Connection`, `Pin`, `GetDeferral`) | Consent | Deferral keeps the sender waiting; `Pin` content |
| `MediaSourceCreated` (`Connection`, `MediaSource`, `GetDeferral`) | Video and audio | That this event has a deferral and that the stream needs it |
| `MiracastReceiverConnection::Disconnect(ConnectionNotAccepted \| Finished)`, `Disconnected` | Ending | Completing a deferral after `Disconnect` |
| `MediaPlayer` frame server (`IsVideoFrameServerEnabled`, `VideoFrameAvailable`, `CopyFrameToVideoSurface`), `RealTimePlayback` | Pictures | Works with a Miracast `MediaSource` in an unpackaged desktop process; speed; HDCP |
| `D3D11CreateDevice`, `CreateDirect3D11SurfaceFromDXGISurface`, `SoftwareBitmap::CreateCopyFromSurfaceAsync` | Read the picture back | Stride equals width x 4; pixel format Bgra8 |

## Limits

* **Windows 10 and 11 only** (the API is documented for Windows 10 2004 and later; **Uncertain**, not checked). No receiver API exists on macOS, Linux or Android; the Flutter entry is not offered there (`miracastSupportedHere()` and the null `MiracastCore` hide it; tested).
* **Adapter support decides everything.** The Wi-Fi adapter and its driver must support the Wi-Fi Direct and Miracast receiver role. Which mini-PC chips qualify is unknown (measurement point 5 asks for two models).
* **Protected content (HDCP) is not shown.** Miracast can carry HDCP-protected streams; whether Windows hands a protected stream to `MediaPlayer`, blocks it or delivers black pictures was not checked. The screen warns that such content may appear black.
* **Latency, frame rate and picture quality are unmeasured.** No figure in this repository is a measurement. The path has several copies (see above).
* One session at a time; no keyboard, mouse or game-controller back-channel; no audio-only mode; no recording.
* MS-MICE (Miracast over the normal network) and "Miracast over infrastructure" are not configured by us; whether `MiracastReceiver` offers it is part of test plan part G.
* The picture is scaled by nearest neighbour; very small text from a 4K sender may look rough. Not looked at on real content.
* No installer and no MSIX. `WiFiDisplay.dll` / `WFDDisplaySink` is not implemented.
* The backend calls `RoInitialize` (multithreaded) on the thread that starts the receiver. The C interface starts the receiver on a thread of its own, so the Flutter UI thread (single-threaded apartment) is not involved. **Uncertain:** whether events arrive reliably in this setup.

## Build integration

* `apps/flutter/windows/CMakeLists.txt` runs two cargo commands (`cc-ffi`, `cc-docs`, `cc-miracast-ffi` together; `cc-office` alone, like Linux, to keep the Office engine out of `cc_ffi.dll`) and installs `cc_ffi.dll`, `cc_miracast_ffi.dll`, `cc-docs-worker.exe` and `cc-office-worker.exe` next to `cometcast.exe`. Debug uses the cargo dev profile, Profile and Release use `--release`. **Never run on Windows.** The generator-expression part was checked with CMake on Linux for both configurations.
* `.github/workflows/release.yml` (job `windows`) still builds the four files with `--locked` first (fast failure; `COMETCAST_CARGO_TARGET_DIR` makes CMake reuse them) and now checks that all four are in the bundle instead of copying them. `actionlint` passes.
* `.github/workflows/rust.yml` runs `cargo clippy -p cc-miracast -p cc-miracast-ffi --target x86_64-pc-windows-gnu` (no C compiler needed for these crates). `.github/workflows/flutter.yml` builds `libcc_miracast_ffi.so` for the Dart binding test.

## What was verified here

| Check | Result |
| --- | --- |
| `cargo test -p cc-miracast` on Linux (policy and name sanitising, status, config, deadline queue, frame mailbox and scaler, diagnosis, connection engine with a fake connection: consent, one session, timeout, fail closed, shutdown) | passed (45 tests) |
| `cargo test -p cc-miracast-ffi` on Linux (controller with a fake receiver: lifecycle, consent, counters, frame buffer rules, stop during start; ABI argument checks) | passed (15 tests) |
| `cargo clippy -p cc-miracast -p cc-miracast-ffi --all-targets -- -D warnings`, host and `--target x86_64-pc-windows-gnu` | passed (MinGW-w64 gcc was installed for the link step below; clippy needs no C compiler for these crates) |
| `cargo build --release -p cc-miracast-ffi --target x86_64-pc-windows-gnu` | built `cc_miracast_ffi.dll` (PE) with all 12 exports (checked with `objdump -p`); **not run** (no Windows, no Wine) |
| `flutter analyze` (whole app) | no issues |
| `flutter test` (whole app) | passed (268 tests, 18 skipped because they need native libraries); 37 of them new in `test/miracast_test.dart`: JSON parsing, library lookup, state machine, consent dialog, video view, hints in English and German, platform guard of the home screen |
| `flutter test test/miracast_ffi_test.dart` with the real Linux library (`COMETCAST_MIRACAST_LIB`) | passed (2 tests: the Dart binding, JSON and buffers work; the receiver reports "unsupported", hint "not Windows") |
| `node tools/check-i18n.mjs` | passed |
| `actionlint` on the workflows | no findings in the changed jobs (one old shellcheck note in the Android job of `flutter.yml`, line 201, was there before) |
| `cometcast-cli` for the Windows target | **not checked** (needs a C++ cross compiler for OpenH264); the CLI only uses `start`, `respond`, `stop`, whose signatures did not change |
| Run on Windows, MSVC build, CMake on Windows, Flutter Windows build | **never done** |

## Code notes

* `unsafe`: the workspace forbids `unsafe`. `cc-miracast` and `cc-miracast-ffi` lower this to `deny` and allow it in exactly the places named above.
* Event handlers run on Windows threads. Events travel through a bounded `std::sync::mpsc` channel (64; further events are dropped while the consumer is stuck; the status is still updated).
* Features of the `windows` crate: see `crates/cc-miracast/Cargo.toml` (`Media_Miracast`, `Media_Playback`, `Graphics_Imaging`, `Graphics_DirectX_Direct3D11`, `Storage_Streams`, `Win32_Graphics_Direct3D11`, `Win32_Graphics_Dxgi`, `Win32_System_WinRT_Direct3D11` and a few more).
* The `CometCastApp` constructor takes an optional `MiracastCore`; `main.dart` passes the DLL binding only when `Platform.isWindows`.

## Legal and licence notes (all **Uncertain**, not legal advice)

* The `windows` crate (Microsoft) is licensed MIT OR Apache-2.0 according to its `Cargo.toml` (not checked against the licence texts). See [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md). It is only a dependency on Windows targets.
* The `MiracastReceiver` API is documented by Microsoft; we call it as an ordinary application. Whether Microsoft's terms for the Windows SDK or Miracast (Wi-Fi Alliance) logo and name usage impose conditions on shipping a product that calls itself a Miracast receiver was not checked. Do not use the Miracast or Wi-Fi Alliance logos or certification claims.
* No code, key or certificate is taken from third-party receivers. `WiFiDisplay.dll` is not used.
* Miracast streams can carry HDCP-protected content. Whether and how the Windows API delivers or blocks protected content to a `MediaSource` was not checked.
