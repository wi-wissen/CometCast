<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Android and Android TV (Flutter app)

Status: milestone M1 on Android, **build only**. The debug APK builds and contains the Rust core and the Flutter engine. **Nothing was run on a device or an emulator** (none was available), so every statement about runtime behaviour on Android, Android TV or Google TV below is untested. Measurement points: A2 and A7 in [measurement-points.md](measurement-points.md).

## What exists

| Part | Where | State |
| --- | --- | --- |
| Rust core for Android through cargo-ndk, wired into Gradle | `apps/flutter/android/app/build.gradle.kts` | Built: `arm64-v8a` and `armeabi-v7a` (debug). `x86_64` is mapped but was not built |
| Android TV manifest bits (leanback launcher, optional touchscreen / leanback, banner) | `apps/flutter/android/app/src/main/AndroidManifest.xml` | In the built APK (checked with `aapt2 dump badging`); not tried on a TV |
| TV banner 320x180 and launcher icons | `res/drawable-xhdpi/tv_banner.png`, `res/mipmap-*`; script `android/tools/make_android_icons.py` | Generated from `assets/logo` (originals untouched) |
| D-pad focus in the Flutter screens | `lib/widgets.dart`, `lib/home_page.dart`, `lib/main.dart`, `lib/send_page.dart` | Widget tests with simulated key events; no real remote tried |
| Screen capture skeleton (Kotlin) | `android/app/src/main/kotlin/de/cometcast/cometcast/capture/` | **Compiles, never run. Not wired to the Rust core** |
| CI job `android` | `.github/workflows/flutter.yml` | **Not run on GitHub** |

## How the Rust core gets into the APK

Chosen: an explicit Gradle task that runs **cargo-ndk**, not cargokit.

Why not cargokit: cargokit (the usual flutter_rust_bridge integration) expects the Rust crate inside a Flutter plugin folder. Here the crate is `crates/cc-ffi` in the repository's Cargo workspace, the Linux build already uses a small explicit CMake step for the same reason (see [flutter-bridge.md](flutter-bridge.md)), and one readable Gradle task is easier to debug. Cargokit was not tried, so this is a judgement, not a measured comparison.

How it works (`apps/flutter/android/app/build.gradle.kts`, function `registerCargoNdk`):

1. Flutter passes the wanted ABIs as the Gradle property `target-platform` (`android-arm`, `android-arm64`, `android-x64`). The script maps them to cargo-ndk names (`armeabi-v7a`, `arm64-v8a`, `x86_64`). Without the property it builds all three. `-PcometcastRustAbis=arm64-v8a,armeabi-v7a` overrides it.
2. One `Exec` task per build type (`cargoNdkDebug`, `cargoNdkProfile`, `cargoNdkRelease`) runs `cargo ndk --platform <minSdk> -t <abi> ... -o build/app/rust-jniLibs/<type> build -p cc-ffi --target-dir build/app/cargo-target-android` in the repository root. Debug uses the cargo dev profile (with `CARGO_PROFILE_DEV_DEBUG=0`, which keeps the `.so` at about 13 MB for arm64 instead of about 140 MB with debug info); profile and release use `--release`.
3. The output folder is registered as the `jniLibs` source set of that build type, and the `merge*JniLibFolders` tasks depend on the cargo task.
4. `ANDROID_NDK_HOME` is set from the NDK that Gradle uses (Flutter 3.47.9 asks for NDK `28.2.13676358`). `~/.cargo/bin` is added to `PATH`.

The library is loaded by its plain name (`libcc_ffi.so`) by `lib/core/rust_loader.dart`, which falls back to the file name when no `lib/` folder exists next to the executable. UNCERTAIN: that this resolves through Android's linker namespace was not run on a device.

The release build type (`flutter build apk --release`, `flutter build appbundle`) uses `--release` with the workspace's LTO profile. It was **not run** (slow). It is still signed with the **debug key**; a release key is not set up. Therefore the release build **fails on purpose** (Gradle task `checkReleaseSigning`, hooked into `preReleaseBuild`) unless `-PcometcastAllowDebugSigning=true` is given (for `flutter build`, which does not forward `-P`, set `ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true` in the environment). With it, the build prints a warning; such an APK is for local tests only and must never be distributed. CI builds the debug APK only. The failing/warning behaviour of this guard was not run (no release build was made).

### Build-time tools (nothing is bundled in the repository)

Android SDK, NDK, Gradle, cmdline-tools and cargo-ndk are build-time tools downloaded by whoever builds. Nothing of them is committed, so there is no entry for them in `THIRD_PARTY_NOTICES.md`. What ends up inside the APK are the Flutter engine, the Flutter and AndroidX libraries and the Rust core; the notices for those are still to be generated from the lock files (see `THIRD_PARTY_NOTICES.md`). The Android SDK licence has to be accepted by the person building (`sdkmanager --licenses`).

## Versions used

| Tool | Version |
| --- | --- |
| Flutter / Dart | 3.47.9 / 3.13.7 |
| Rust | 1.97.0 (stable); targets `aarch64-linux-android`, `armv7-linux-androideabi` |
| cargo-ndk | 4.1.2 |
| JDK | OpenJDK 21.0.12.1 |
| Gradle (wrapper) | 9.3.1 |
| Android Gradle Plugin / Kotlin plugin | 9.1.0 / 2.4.0 (from the Flutter template, unchanged) |
| Android SDK | commandlinetools build 11076708, platform-tools 37.0.1, platform `android-36`, build-tools 36.0.0 |
| NDK | 28.2.13676358 (r28c) |
| minSdk / targetSdk / compileSdk | 24 / 36 / 36 (Flutter defaults of 3.47.9) |

## Commands that were run

Setup (once, as root on this machine; SDK in `/opt/android-sdk`):

```sh
curl -o clt.zip https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
unzip clt.zip -d x && mkdir -p /opt/android-sdk/cmdline-tools && mv x/cmdline-tools /opt/android-sdk/cmdline-tools/latest
yes | sdkmanager --licenses
sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "ndk;28.2.13676358"
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-ndk --locked          # 4.1.2
export ANDROID_HOME=/opt/android-sdk
flutter config --android-sdk /opt/android-sdk
```

Rust crate alone, with cargo-ndk (from the repository root):

```sh
cargo ndk -t arm64-v8a   --platform 24 -o /tmp/jni build -p cc-ffi --target-dir /tmp/cargo-target   # Finished dev, 36 s
cargo ndk -t armeabi-v7a --platform 24 -o /tmp/jni build -p cc-ffi --target-dir /tmp/cargo-target   # Finished dev, 32 s
```

Result: **both compile**, including `webrtc 0.21.1` and its dependencies (ring was not a problem in this configuration). The 32-bit build needed no change. (`--target-dir` goes after `build`; before it cargo-ndk rejects it.)

App (inside `apps/flutter`):

```sh
flutter pub get && flutter gen-l10n
flutter analyze                                              # No issues found
flutter test                                                 # 26 passed, 3 skipped (see below)
flutter build apk --debug --target-platform android-arm64    # Built build/app/outputs/flutter-apk/app-debug.apk (94 MB)
flutter build apk --debug --target-platform android-arm,android-arm64   # 145 MB
```

`flutter test`: the 3 skipped tests are the real-library tests in `test/rust_core_test.dart`, which need the host `libcc_ffi.so` (`cargo build -p cc-ffi`); it was not built in this worktree to save disk. They are unchanged by this work.

APK contents (`unzip -l build/app/outputs/flutter-apk/app-debug.apk`), dual-ABI build:

```
13507640  lib/arm64-v8a/libcc_ffi.so
38846424  lib/arm64-v8a/libflutter.so
 7448720  lib/armeabi-v7a/libcc_ffi.so
33159780  lib/armeabi-v7a/libflutter.so
```

`aapt2 dump badging` shows: `leanback-launchable-activity` MainActivity, `uses-feature-not-required` for `android.hardware.touchscreen` and `android.software.leanback`, `banner='res/drawable-xhdpi-v4/tv_banner.png'`, `native-code: 'arm64-v8a'` (arm64-only build), and the service entry with `foregroundServiceType=0x20` (mediaProjection).

### Gradle and Maven Central (HTTP 429) on this machine

dl.google.com, maven.google.com, services.gradle.org, plugins.gradle.org and storage.googleapis.com answered normally. `repo.maven.apache.org` (Maven Central) answered **HTTP 429 for roughly every other request**, also for single sequential `curl` GETs (observed 200, 429, 200). Gradle's built-in three retries are not enough: the first builds failed with `Plugin [id: 'org.jetbrains.kotlin.android', version: '2.4.0'] was not found` although the POM exists (a direct `curl` returned 200). Raising `systemProp.org.gradle.internal.repository.max.tentatives` to 8 with a 3 s backoff in `~/.gradle/gradle.properties` did not help within 6 minutes (UNCERTAIN whether the property was honoured). What worked: a **local pass-through mirror** (about 40 lines of Python, retrying on 429 with backoff and caching) on `127.0.0.1:18090` plus a Gradle init script in `~/.gradle/init.d/` that points `https://repo.maven.apache.org/maven2` repositories to it. Both live outside the repository because they are specific to this sandbox. With it the first complete `flutter build apk --debug` took about 3 minutes. On a normal network none of this should be needed. UNCERTAIN: whether the 429 is permanent policy or load; it may differ on another day.

### Stale ABIs and task wiring

The task `cargoNdk<Type>` deletes `build/app/rust-jniLibs/<type>` before it runs, so an ABI from an earlier build cannot end up in a later APK. Checked with two builds: `--target-platform android-arm,android-arm64` gave `lib/arm64-v8a/libcc_ffi.so` and `lib/armeabi-v7a/libcc_ffi.so` in the APK (`unzip -l`); a following `--target-platform android-arm64` build gave only the arm64-v8a one. AGP refuses Provider instances in `jniLibs.srcDir(...)` and a `files(dir).builtBy(task)` collection did not trigger the task either (tried), so the `merge*JniLibFolders` / `merge*NativeLibs` tasks depend on `cargoNdk<Type>` explicitly.

## Android TV / Google TV

Manifest (`AndroidManifest.xml`):

- `<uses-feature android.hardware.touchscreen required="false">` and `<uses-feature android.software.leanback required="false">`: installable on TVs and still on phones.
- A second intent filter with `android.intent.category.LEANBACK_LAUNCHER` on `MainActivity`, and `android:banner="@drawable/tv_banner"` on the application.
- `INTERNET`, `ACCESS_NETWORK_STATE` (no `usesCleartextTraffic`: `ws://` is opened by the Rust core with its own raw TCP sockets, which Android's cleartext policy does not govern; UNCERTAIN, not run on a device. TLS (`wss://`) verification in Rust uses `rustls-native-certs`; whether it finds Android's system CA store (it is not a plain PEM file) is UNVERIFIED and may need `rustls-platform-verifier`); for capture `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MEDIA_PROJECTION`, `POST_NOTIFICATIONS`.

Banner and icons: `python3 apps/flutter/android/tools/make_android_icons.py` (Pillow) reads `assets/logo/cometcast-app-icon-1024.png` and `cometcast-logo-dark-1024.png` and writes the launcher icons (48 to 192 px) and a 320x180 banner (navy background taken from the icon, the logo and the words "Comet Cast"). The originals are not modified. UNCERTAIN: the banner was only looked at as a PNG, not on a TV launcher; the text uses DejaVu Sans Bold on this machine (the script falls back to Pillow's default font).

D-pad focus: **Flutter's built-in focus system, not the `dpad` package** (named in A2). Reason: the built-in system already binds the arrow keys to directional focus and select / Enter to activate, and the three screens are simple lists and forms; the `dpad` package was not tried, and its resolvability was not checked. What was added:

- one `FocusTraversalGroup` (reading order) per screen (`EscapeToGoBack` in `lib/widgets.dart`, and the home screen);
- `FocusRing`: a 3 px frame around the focused home row, and a thick focus border for filled, outlined and icon buttons (theme in `lib/main.dart`; Material's own overlay is faint);
- the first home row has autofocus;
- `textFieldFocusNode()`: **a finding from the test**: with the default setup the up and down arrow keys stay inside a single-line text field, so a D-pad user could not leave it. The helper maps up / down to focus movement; left / right still move the caret.

`test/dpad_focus_test.dart` (3 widget tests): arrow keys and select on the home screen; arrow keys between server field and start button on the receive screen; arrow keys through all fields of the send screen. These simulate key events only. UNCERTAIN / not tested: a real remote (Back and Home keys, long press), the on-screen keyboard that Android TV may open when a text field gets focus (entering the room code with a remote is likely awkward), overscan and 10-foot layout, focus after `Navigator.pop`, the `Material` text sizes on a TV.

## Screen capture skeleton

Files in `.../kotlin/de/cometcast/cometcast/capture/`:

| File | Job |
| --- | --- |
| `ScreenCaptureService.kt` | Foreground service, `foregroundServiceType="mediaProjection"` |
| `ScreenEncoder.kt` | `MediaCodec` H.264 (`video/avc`), `COLOR_FormatSurface` input, drain thread |
| `CaptureBridge.kt` | MethodChannel `de.cometcast/capture`, consent dialog, `onActivityResult` |
| `CaptureEvents.kt` | EventChannel `de.cometcast/capture/frames` |

Dart contract (no Dart wrapper exists yet; `lib` was left alone apart from the focus work):

- `invokeMethod('start', {maxDimension, fps, bitrate, notificationTitle, notificationText, notificationStop})` returns `true` (started) or `false` (the user refused). The notification texts are meant to come from the ARB keys `captureNotificationTitle`, `captureNotificationText`, `captureNotificationStop` (added in English and German); the native side contains no user-visible strings except the notification channel's technical name.
- `invokeMethod('stop')`, `invokeMethod('requestKeyFrame')`.
- The event stream emits maps: `config` (SPS/PPS, Annex B), `frame` (`ptsUs`, `keyFrame`, `data`), `stopped`, `error`. See `CaptureEvents.kt`.

Android 14 rules as implemented (from the Android documentation as understood; UNCERTAIN until run): `startForeground(..., FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)` before `getMediaProjection`; a `MediaProjection.Callback` registered before `createVirtualDisplay`; the consent result is used once and never stored, so every session asks again; `onStop` of the callback ends the session.

Robustness as implemented (UNCERTAIN until run on a device): every start command reaches `startForeground()` first (also a second start while a session runs and a missing consent), and a failing `startForeground` (for example `SecurityException`) is caught and ends the service instead of crashing; `ScreenEncoder.start()` releases the codec and surface itself when it throws and `stop()` is safe on a half-started encoder. Backpressure in `CaptureEvents`: at most 8 encoded frames wait for the main looper, frames are dropped and counted (`dropped` in each frame event) while nobody listens or the queue is full, delta frames are skipped until the next key frame after a drop, the codec config is cached and replayed on `onListen` together with a key frame request, and `stopped` is sent once per session.

**Not wired:** `cc-sender` sends only a synthetic VP8 test stream (see [flutter-bridge.md](flutter-bridge.md)). There is no H.264 path into the Rust core: no RTP packetisation for H.264, no FFI function that accepts external frames. Until that exists, the captured frames only reach Dart and go nowhere. Likely next step (not done, not decided): an FFI call that takes Annex B access units and a sender track that packetises them (RFC 6184), or a native path that bypasses the Dart event channel.

Also UNCERTAIN, all untested: encoder acceptance on weak TV boxes, behaviour when the screen is static (the format sets `KEY_REPEAT_PREVIOUS_FRAME_AFTER` = 1 s), rotation and resize (`onCapturedContentResize` and `onCapturedContentVisibilityChanged` are overridden but do nothing: the encoder size is fixed at the start), the Android 15 "share one app" mode, the `POST_NOTIFICATIONS` request (implemented in `CaptureBridge`: on Android 13+ the system dialog is shown before the consent dialog while the permission is missing; the capture continues if it is denied and only the notification is hidden), and whether Android TV devices offer MediaProjection at all (many TV boxes may not; to be checked on the Google TV Streamer).

## What a real-device test must check

Measurement point A2 (Flutter UI with Rust core, Google TV operation) and A7 (Android TV boxes as receivers, 32-bit build):

1. Install the APK (`adb install`) on: a phone (arm64), the Google TV Streamer, and one cheap 32-bit TV box. Check which ABI each installs (`adb shell getprop ro.product.cpu.abi`). A 32-bit userland on a 64-bit CPU needs the `armeabi-v7a` library.
2. App starts and `libcc_ffi.so` loads (look for the exception in `adb logcat`); receive screen shows a room code and counters rise against a real server and a sender (the smoke test that exists for Linux, repeated by hand).
3. D-pad: every control reachable and visible with a real remote; the room code can be entered; Back works; no focus trap; banner shown in the launcher row.
4. Weak boxes: UI smoothness, memory, crash on start, WebRTC (ICE) over Wi-Fi and Ethernet. Record device, Android version, ABI, RAM. Video decoding is not built yet, so "video stutters" (A2 risk) cannot be judged until a decoder path exists.
5. MediaProjection (capture skeleton): consent dialog appears on every start (Android 14+); notification shown and the Stop action works; the projection stops when the user stops sharing or locks the screen; behaviour when the app is backgrounded; what happens on Android 15; what the encoder produces (frame rate, key frame interval, bitrate, static screen); check `adb logcat` for `SecurityException` ("must start foreground service first", "must register a callback"). Check whether a TV device even offers the consent dialog.
6. Fallback if A2 fails (from the measurement plan): build only the TV receiver natively in Kotlin.
