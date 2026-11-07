# Comet Cast - Flutter app (phone, tablet, desktop, Android TV in progress)

> **Verified with Flutter 3.47.9 (stable, Dart 3.13.7) and Rust 1.97 on Linux: `flutter analyze` clean, `flutter test` passes (widget tests with a fake core and tests against the real Rust library), `flutter build linux --debug` succeeds with the Rust core built and bundled, and two headless tests (xvfb) with a real server: the old smoke test (room code, rising packet counters) and a screen test in which a real X11 screen (Xvfb with a red window) travels as H.264 through `cometcast-cli send --screen` into the app, whose drawn picture is read back pixel by pixel (red, then blue), and the app as sender of that screen into `cometcast-cli receive --decode`. `flutter build apk --debug` also succeeded earlier (arm64-v8a and armeabi-v7a, Rust core built with cargo-ndk; not re-run for this change). Not verified: anything on an Android device or emulator or an Android TV, the Android release build, Windows, macOS, iOS builds, `flutter build linux --release`, a real Wayland or GNOME/KDE desktop, a real window manager (full screen was only tested with a mocked channel and compiled into the GTK runner, never seen on a real display).**

Status: milestone M1 on the desktop. The app talks to the Rust core through
[flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) (see
[docs/flutter-bridge.md](../../docs/flutter-bridge.md)). Screens:

- Home: a quiet list: send screen, send file or link, receive.
- **Send file or link**: choose an image (PNG, JPEG, WebP, GIF) or a PDF with the file dialog, or type an
  http(s) link; enter the receiver's room code; progress bar; for a PDF the presenter controls (previous, next,
  go to page, stop; arrow keys, Page Up/Down). All checks are done by Rust. Details: docs/documents.md,
  "In the Flutter app".
- **Receive on this device**: server field (pre-filled, see below), big room code, status,
  live counters. Images, PDF pages (drawn by Rust at the width of the view, page indicator, pages follow the
  sender) and link cards ("The sender wants to open <host>", Open / Dismiss, never opened without a click) that a
  sender pushes take the window; a close button goes back. When an H.264 screen arrives, the **picture fills the window**: buttons for
  statistics (key `I`), full screen (`F11`; `Esc` leaves full screen first, then goes back) and
  stop. A translated "waiting for the sender's screen" line shows until the first picture. A
  track that is not H.264 (the built-in VP8 test video) is only counted and the screen says so.
- **Send screen**: server field, **"Share this screen"** switch with the monitor list (Linux/X11;
  on Wayland and other systems a translated "not supported on this system yet" message, decided
  by a Rust-side check), room code input (validated by the Rust core with the same alphabet as
  `cc-proto`), duration (only for the test video), status. Without the switch it sends the
  built-in VP8 test stream.
- **Server address**: pre-filled from the Rust core: at once from `server_defaults()`, then from
  `discover_servers()` (policy, DNS-SRV, display announcements, built-in list, then
  `127.0.0.1:8443` for development). Suggestions are chips that say where each came from
  (translated). The last server you started with is saved (`shared_preferences`) and wins at the
  next start unless an administrator policy names servers; `COMETCAST_SERVER=host:port`
  overrides both (development). If the policy forbids other servers the field is read-only
  ("Your administrator has set the server."). Problems while searching are shown as calm,
  translated notes (by code, never raw text); a small hint shows while the search runs and
  closing the screen cancels it.
- **Receive screen** announces itself and its server in the local network while it is open
  (`start_advertising`); the **send screen** lists "Nearby receivers" (name and announced
  server, marked "not verified"); a tap takes the server, shows its source next to the field
  and focuses the room code field (the code is never announced). Announced servers are never
  pre-filled or saved as "last used". Details and open questions: docs/discovery.md, "In the Flutter app".

**Not built yet:** screen capture on Windows, macOS, Wayland and mobile; other video codecs than
H.264 in the picture view; audio; drag and drop of files; the PDF worker for platforms other than Linux (PDFs
there fail with a translated note); a thumbnail of the page on the presenter's side. English + German, light/dark following the
system, keyboard friendly (Tab, Enter starts, Escape goes back). License: GPL-3.0-or-later.

## Needs

Flutter SDK, a Rust toolchain (`cargo`), and for Linux desktop: `clang cmake ninja-build pkg-config libgtk-3-dev`.
The Linux build runs `cargo build -p cc-ffi` itself (see the bridge doc).

## Android

`flutter build apk --debug` builds the Rust core with **cargo-ndk** through a Gradle task and puts
`libcc_ffi.so` into the APK (checked with `unzip -l`). Extra needs: JDK, Android SDK (platform 36,
build-tools 36.0.0, NDK 28.2.13676358), `rustup target add aarch64-linux-android armv7-linux-androideabi`,
`cargo install cargo-ndk --locked`, `ANDROID_HOME`. Exact commands, versions, the Android TV
bits, the capture skeleton and a real-device checklist: [docs/android.md](../../docs/android.md).

```sh
flutter build apk --debug --target-platform android-arm64                # about 94 MB
flutter build apk --debug --target-platform android-arm,android-arm64    # adds the 32-bit library
```

Nothing was run on a device. The D-pad focus handling is covered by widget tests with simulated
keys only (`test/dpad_focus_test.dart`); the screen capture code (Kotlin) only compiles and is not
connected to the Rust core.

## Commands that were run

From the repository root, once, to have the library for the tests:

```sh
cargo build -p cc-ffi                        # target/debug/libcc_ffi.so, used by test/rust_core_test.dart
```

Inside `apps/flutter`:

```sh
export PATH=/opt/flutter/bin:$PATH FLUTTER_ALLOW_ROOT=1 CI=true   # how this machine is set up
flutter pub get
flutter gen-l10n                 # generates AppLocalizations into lib/l10n (git-ignored)
flutter analyze                  # No issues found
flutter test                     # 47 passed with the library built (`cargo build -p cc-ffi` first); without it the 6 real-library tests are skipped; includes the D-pad tests
flutter build linux --debug      # builds the Rust core via CMake, bundle in build/linux/x64/debug/bundle
flutter run -d linux             # run it (needs a display)
```

### Smoke test (real server, real CLI, headless)

```sh
# from the repository root
cargo build -p cometcast-server -p cometcast-cli
target/debug/cometcast-server --listen 127.0.0.1:18443 &

# inside apps/flutter (xvfb = a virtual display; install with: apt-get install -y xvfb)
xvfb-run -a -s "-screen 0 1280x1024x24" \
  flutter test integration_test/receiver_smoke_test.dart -d linux \
  --dart-define=COMETCAST_TEST_SERVER=127.0.0.1:18443 \
  --dart-define=COMETCAST_CLI=$PWD/../../target/debug/cometcast-cli
```

Test 1 opens "Receive on this device", reads the room code from the screen, starts
`cometcast-cli send <CODE> --server ... --bind 127.0.0.1:0 --seconds 8` and waits until the
packet counter rises. Test 2 starts a receiver through the bridge, types its code into the
"Send screen" view and checks that the UI reports the test video and the receiver counts
packets. Output of the run on this machine:

```
SMOKE room code on screen: VTNDJ6
SMOKE packets after first rise: 5
SMOKE final: packets=35 bytes=15121 frames=35 keyframes=35 codec=vp8
SMOKE send: Finished: 75 frames sent, receiver packets=72
All tests passed!
```

(The integration test binds WebRTC to `127.0.0.1:0` because both peers run on one machine.
The app reads `COMETCAST_UDP_BIND` for the same purpose; normally leave it unset.)

### Content end-to-end test (real server, real CLI, bundled PDF worker, headless)

Results of this test were obtained locally, not on GitHub CI (UNCERTAIN there). The script waits until the server port answers (at most 20 s) before it starts the app, and stops only the server it started.

```sh
# from the repository root: the CLI, the server and the PDF worker (cc-docs-worker)
cargo build --workspace --bins
# inside apps/flutter
integration_test/run_content_e2e.sh
```

It starts a real `cometcast-server` and runs `integration_test/content_e2e_test.dart` under `xvfb-run`
(`flutter test -d linux` builds the app, so the bundle contains the worker; the script unsets
`COMETCAST_DOCS_WORKER`). Tests: the worker sits next to the executable and the core names it; **A1** the app is the
receiver and `cometcast-cli send-file <CODE> classroom.pdf --then next,go:3,prev,stop` drives it: the page
indicator must read 1, 2, 3, 2 of 3, the pixels of each page (read back from the page view) differ, the page is drawn at the
width of the view, and `stop` removes it; **A2** an image is shown and closing it goes back to waiting; **A3** `send-link`
shows a card with the punycode host and nothing is opened until Open is clicked; **B** the app is the sender of the PDF
to `cometcast-cli receive`: the `Content:` lines show the document (3 pages) and pages 1, 2, 3, 2 for the keys and buttons of the
presenter controls. The file dialog and the browser are replaced by fakes in these tests (a real dialog needs a person). The
CLI is started detached (see docs/flutter-bridge.md, "Dart's `Process.start` and the worker").

### Screen end-to-end test (real server, real X11 screen, headless)

Needs `xterm` and `xvfb` (`apt-get install -y xterm xvfb`). One script starts a real
`cometcast-server`, an Xvfb display (640x480, free display number chosen by Xvfb itself) whose
screen is shared, and runs `integration_test/screen_e2e_test.dart` under a second Xvfb display:

```sh
# from the repository root (on a small disk: CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 shrinks target/)
cargo build -p cc-ffi -p cometcast-server -p cometcast-cli
# inside apps/flutter
integration_test/run_screen_e2e.sh
# the older smoke test through the same script:
TEST_FILE=integration_test/receiver_smoke_test.dart integration_test/run_screen_e2e.sh
```

Scenario A: the app is the receiver; `cometcast-cli send <CODE> --screen --display <shared>` shares
a red `xterm` window; the test reads the pixels of the drawn picture (`RenderRepaintBoundary.toImage`)
and asserts that more than 80 % are red; a blue window on top must turn the picture blue; then the
screen flips between red and blue about 20 times a second (xterm reverse video) while the display rate,
CPU and the statistics overlay are read. Scenario B: the app is the sender ("Share this screen" with the
display from `COMETCAST_CAPTURE_DISPLAY`), `cometcast-cli receive --decode` prints the mean colour of the decoded
pictures and the test asserts it is red. Output of the last run on this machine (shortened):

```
SCREEN-E2E room code 76Q8LD
SCREEN-E2E first picture after 532 ms (includes starting the CLI)
SCREEN-E2E picture 960x720, share of red pixels: 0.993
SCREEN-E2E static screen: app process CPU 19 % of one core
SCREEN-E2E after the blue window: share of blue pixels 0.993
SCREEN-E2E measurement over 10.1 s: app process CPU 101 % of one core
SCREEN-E2E busiest threads (% of one core): DartWorker=66, cometcast=13, tokio-rt-worker=6, llvmpipe-3=3, ...
SCREEN-E2E overlay: Picture size: 640 x 480 | Display rate: 10.1 fps | Decoded: 139 | Dropped: 0 | Decode time: 1.5 ms | Key frame requests: 1 | Decoder errors: 0 | Hand-over to the picture: 8.2 ms, image: 5.2 ms
SCREEN-E2E sender says: sent frames=132 polls=310 changed=125 unchanged=184 refreshes=6 keyframes=126 ...
SCREEN-E2E sender killed; picture removed from the view: true
SCREEN-E2E monitor list shows: 1 entry "Whole desktop (...)"
SCREEN-E2E CLI decoded: decoded units=4 pictures=4 last=640x480 mean_rgb=(252,0,0)
All tests passed!
```

How to read the numbers (a debug Flutter build with software OpenGL, llvmpipe, on a shared 4-core VM; one run each,
not a benchmark): while the screen changed all the time (the sender looks 15 times a second; the flipping window changes
about 10 to 12 times a second) the window drew **10 to 12 fps** (three runs: 10.1, 11.7 and 11.9 fps, read from the
overlay); **no picture was dropped** (`Dropped: 0`) and the decoder reported no error in those three runs. Decode plus scaling in Rust took **1.5 to 1.7 ms**
per 640x480 picture (it took 33.7 ms per 1280x720 picture before `cc-ffi` was built with `opt-level = 3` in the dev
profile, see `Cargo.toml`). Handing a picture to Dart (copy over the bridge) took 8.2 ms and turning it into an image
5.2 ms on average. The **app process used 15 to 20 % of one core with a static picture and 89 to 101 % of one core
while the picture changed 10 times a second**; the busiest thread by far is the Dart VM's `DartWorker` (55 to 66 %).
UNCERTAIN: why `DartWorker` is that busy was not investigated (it may be the debug-mode engine and JIT, or the image
creation; a profile/release build was not measured), so these CPU numbers say nothing about release builds. In one earlier run the
overlay showed one `OpenH264 ... Native:16` decode error (the flipping window was not working in that run, so the
screen was mostly static); it did not come back in the next three runs and its cause is unknown. After an error the decoder waits
for the next key frame and asks for one.

## Structure

| Path | What |
| --- | --- |
| `lib/main.dart` | Entry point, theme, languages |
| `lib/home_page.dart`, `receive_page.dart`, `send_page.dart` | Screens |
| `lib/receiver_controller.dart`, `sender_controller.dart` | Screen state (like Vue composables); the receiver controller fetches pictures and measures the display rate |
| `lib/settings.dart` | Server address (default list from Rust, saved last server) |
| `lib/window_control.dart` | Full screen through a platform channel (only the Linux runner answers it) |
| `lib/core/core_api.dart` | Interface to the Rust core; real and fake implementations |
| `lib/core/rust_loader.dart` | Finds and loads `libcc_ffi` |
| `lib/src/rust/` | Generated by flutter_rust_bridge (committed, do not edit) |
| `lib/l10n/*.arb` | Texts (English and German, keys must match: `node tools/check-i18n.mjs`) |
| `linux/CMakeLists.txt` | Builds and bundles the Rust core (Linux); `COMETCAST_CARGO_TARGET_DIR` shares a cargo target folder |
| `linux/runner/my_application.cc` | Window; answers the full-screen channel `de.cometcast/window` |
| `integration_test/screen_e2e_test.dart`, `run_screen_e2e.sh` | Screen tests with a real server and X11 screen |
| `android/app/build.gradle.kts` | Builds the Rust core with cargo-ndk (Android) |
| `android/app/src/main/kotlin/.../capture/` | Screen capture skeleton (MediaProjection + MediaCodec), not run |
| `android/tools/make_android_icons.py` | Derives launcher icons and the TV banner from `assets/logo` |
| `test/`, `integration_test/` | Tests |

Notes for a Vue developer: `pubspec.yaml` is like `package.json`;
`lib/main.dart` like `main.js`; widgets are like components; a `ChangeNotifier` plus
`ListenableBuilder` is like a store plus a computed view; the `.arb` files are like
`locales/en.json`.

## Known uncertainties

- Server address: pre-filled with the first entry of `cc-discovery`'s offline list: an MDM policy file
  if present, otherwise the built-in `relay.cometcast.de:443`, **a host that does not exist yet**
  (UNCERTAIN: planned name). The DNS SRV / display-announcement steps are used through
  `discover_servers()` but were never tried on a real school network (docs/discovery.md).
  `127.0.0.1:8443` is offered as a chip for development. The last used server is saved with
  `shared_preferences`; UNCERTAIN: only its Linux behaviour (a JSON file `shared_preferences.json` in the
  application support folder, normally `~/.local/share/<application id>/`) was read about in the package, not looked at on disk.
- The picture view is not a Flutter `Texture`: every picture is copied from Rust (RGBA) to Dart and made into a
  `ui.Image` (`decodeImageFromPixels`) and drawn with `RawImage`. That is simple and safe, and the
  cost above is what it costs here; a native texture would avoid two copies. UNCERTAIN: the cost on a real GPU and
  in release builds was not measured. Scaling in Rust is nearest-neighbour (fast, jagged for fine text at large shrink factors).
- Full screen: implemented for the GTK runner only (a small platform channel). UNCERTAIN: it was compiled and the
  Dart side is tested with a mocked channel, but not seen on a real display with a window manager. Windows and macOS: no handler,
  the button then does nothing.
- Wayland check: a session counts as Wayland if `XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` is set (unless a display is
  forced with `COMETCAST_CAPTURE_DISPLAY`). Never tried on a real Wayland desktop.
- Loading and bundling the Rust library is built for Linux (CMake) and Android (cargo-ndk, build only).
  Windows: `windows/CMakeLists.txt` builds and installs the Rust DLLs and workers with cargo (written, never run on Windows), and
  the Windows-only screen "Receive via Miracast (Windows)" exists (docs/miracast.md, never run). macOS and iOS need their own
  packaging (cargokit is the likely route); not tried.
- Android: no device or emulator was available; see docs/android.md for everything that is untested.
- Whether the default WebRTC bind address (`0.0.0.0:0`) connects two peers on one machine was not tested.
- The CI workflow (`.github/workflows/flutter.yml`) has not run on GitHub. UNCERTAIN: the xvfb end-to-end tests (smoke, screen, content) were run locally on one machine only, not on GitHub CI.
- German texts are written by the AI agent in formal "Sie"; a German speaker should review them.
- `l10n.yaml` writes generated code to `lib/l10n` (`synthetic-package: false`); the code imports
  `package:cometcast/l10n/app_localizations.dart`.

## Later

- Screen capture on Wayland (PipeWire), Windows and macOS; a native texture for the picture; the file/link row on the home screen.
- Android TV / Google TV: test on real devices (focus with a real remote, the 32-bit build, a
  MediaCodec video surface for decoding; the package `dpad` was not tried, Flutter's built-in focus
  system is used). H.264 from the capture skeleton into the Rust core. This is measurement point 1 /
  assumption A2 (and A7) in `docs/measurement-points.md`.
- Apple TV is a separate native SwiftUI app: see `apps/apple-tv`.

## Android release signing

The Android release build type is signed with the debug key because no release keystore is configured. The build therefore fails unless `-PcometcastAllowDebugSigning=true` (Gradle) or `ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true` (environment, for `flutter build apk --release`) is set, and then only prints a warning. Such an APK is for local tests, never for distribution. See [docs/android.md](../../docs/android.md).
