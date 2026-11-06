# Comet Cast - Flutter app (phone, tablet, desktop, Android TV in progress)

> **Verified with Flutter 3.47.9 (stable, Dart 3.13.7) and Rust 1.97 on Linux: `flutter analyze` clean, `flutter test` passes (widget tests with a fake core and tests against the real Rust library), `flutter build linux --debug` succeeds with the Rust core built and bundled, and a headless smoke test (xvfb) shows a room code and rising packet counters in the UI. `flutter build apk --debug` also succeeds (arm64-v8a and armeabi-v7a, Rust core built with cargo-ndk). Not verified: anything on an Android device or emulator or an Android TV, the Android release build, Windows, macOS, iOS builds, `flutter build linux --release`.**

Status: milestone M1 on the desktop. The app talks to the Rust core through
[flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) (see
[docs/flutter-bridge.md](../../docs/flutter-bridge.md)). Screens:

- Home: a quiet list. "Send file or link" is still a placeholder.
- **Receive on this device**: server field, big room code, status, live counters
  (packets, bytes, frames, key frames, codec).
- **Send screen**: server field, room code input (validated by the Rust core with the same
  alphabet as `cc-proto`), duration, status.

**Not built yet:** screen capture and video decoding/rendering. The receiver only counts
incoming video; the sender sends a built-in VP8 test stream. Both screens say so (translated).
English + German, light/dark following the system, keyboard friendly (Tab, Enter starts,
Escape goes back). License: GPL-3.0-or-later.

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
flutter test                     # 26 passed + 3 real-library tests skipped (run `cargo build -p cc-ffi` first to run them); includes the D-pad tests
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

## Structure

| Path | What |
| --- | --- |
| `lib/main.dart` | Entry point, theme, languages |
| `lib/home_page.dart`, `receive_page.dart`, `send_page.dart` | Screens |
| `lib/receiver_controller.dart`, `sender_controller.dart` | Screen state (like Vue composables) |
| `lib/core/core_api.dart` | Interface to the Rust core; real and fake implementations |
| `lib/core/rust_loader.dart` | Finds and loads `libcc_ffi` |
| `lib/src/rust/` | Generated by flutter_rust_bridge (committed, do not edit) |
| `lib/l10n/*.arb` | Texts (English and German, keys must match: `node tools/check-i18n.mjs`) |
| `linux/CMakeLists.txt` | Builds and bundles the Rust core (Linux) |
| `android/app/build.gradle.kts` | Builds the Rust core with cargo-ndk (Android) |
| `android/app/src/main/kotlin/.../capture/` | Screen capture skeleton (MediaProjection + MediaCodec), not run |
| `android/tools/make_android_icons.py` | Derives launcher icons and the TV banner from `assets/logo` |
| `test/`, `integration_test/` | Tests |

Notes for a Vue developer: `pubspec.yaml` is like `package.json`;
`lib/main.dart` like `main.js`; widgets are like components; a `ChangeNotifier` plus
`ListenableBuilder` is like a store plus a computed view; the `.arb` files are like
`locales/en.json`.

## Known uncertainties

- Server address: a plain text field, default `127.0.0.1:8443`, kept in memory only
  (UNCERTAIN: the real default list comes from `cc-discovery` later; no persistence package chosen).
- Loading and bundling the Rust library is built for Linux (CMake) and Android (cargo-ndk, build only).
  Windows, macOS and iOS need their own packaging (cargokit is the likely route); not tried.
- Android: no device or emulator was available; see docs/android.md for everything that is untested.
- Whether the default WebRTC bind address (`0.0.0.0:0`) connects two peers on one machine was not tested.
- The CI workflow (`.github/workflows/flutter.yml`) has not run on GitHub.
- German texts are written by the AI agent in formal "Sie"; a German speaker should review them.
- `l10n.yaml` writes generated code to `lib/l10n` (`synthetic-package: false`); the code imports
  `package:cometcast/l10n/app_localizations.dart`.

## Later

- Screen capture (native) and video decoding/rendering; the file/link row on the home screen.
- Android TV / Google TV: test on real devices (focus with a real remote, the 32-bit build, a
  MediaCodec video surface for decoding; the package `dpad` was not tried, Flutter's built-in focus
  system is used). H.264 from the capture skeleton into the Rust core. This is measurement point 1 /
  assumption A2 (and A7) in `docs/measurement-points.md`.
- Apple TV is a separate native SwiftUI app: see `apps/apple-tv`.

## Android release signing

The Android release build type is signed with the debug key because no release keystore is configured. The build therefore fails unless `-PcometcastAllowDebugSigning=true` (Gradle) or `ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true` (environment, for `flutter build apk --release`) is set, and then only prints a warning. Such an APK is for local tests, never for distribution. See [docs/android.md](../../docs/android.md).
