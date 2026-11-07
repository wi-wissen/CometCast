<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Android and Android TV (Flutter app)

Status: milestone M1 on Android and milestone M6 on Android are **complete in code and built to specification, and unverified on any device**. Everything that can be written, built and unit-tested without a device exists: the MediaProjection / MediaCodec sender path into the Rust sender, the receiver (software decoding, and a MediaCodec hardware path into a Flutter texture), the Android TV / Google TV receiver-only mode, the PDF and Office workers as native libraries, the Gradle build with environment-provided signing, Dart widget tests with fake platform channels and Kotlin JVM unit tests. **Nothing was run on a device or an emulator** (none was available), so every statement about runtime behaviour on Android, Android TV or Google TV below is untested; `UNCERTAIN` marks where the Android documentation as remembered, or an API's behaviour on real hardware, decides. What a person with a phone and a Google TV must do to check it: [android-device-test-plan.md](android-device-test-plan.md). Measurement points: A2 and A7 in [measurement-points.md](measurement-points.md).

## What exists

| Part | Where | State |
| --- | --- | --- |
| Rust core for Android through cargo-ndk, wired into Gradle | `apps/flutter/android/app/build.gradle.kts` | Built: `arm64-v8a` and `armeabi-v7a` (debug). `x86_64` is mapped but was not built |
| Android TV manifest bits (leanback launcher, optional touchscreen / leanback, banner) | `apps/flutter/android/app/src/main/AndroidManifest.xml` | In the built APK (checked with `aapt2 dump badging`); not tried on a TV |
| TV banner (320x180 dp: 320, 480 and 640 px wide) and launcher icons | `res/drawable-{mdpi,hdpi,xhdpi}/tv_banner.png`, `res/mipmap-*`; script `android/tools/make_android_icons.py` | Generated from `assets/logo` (originals untouched). The earlier version put 320x180 px into `drawable-xhdpi`, which is only 160x90 dp; fixed |
| TV receiver-only mode, big room code, auto start, keep-screen-on | `lib/core/android_platform.dart`, `lib/home_page.dart`, `lib/receive_page.dart`, `platform/PlatformBridge.kt` | Widget tests with fakes; never on a TV (next sections) |
| Receiver video: software and MediaCodec hardware path | `crates/cc-ffi/src/video.rs`, `lib/receiver_controller.dart`, `lib/core/android_video.dart`, `video/*.kt` | Rust unit tests, Dart tests with fakes, Kotlin JVM tests of the parsing helpers; MediaCodec never run |
| PDF / Office workers as native libraries | `app/build.gradle.kts` (`registerWorkers`), `lib/core/android_platform.dart` (`configureAndroidWorkers`) | Built into the APK (see the commands); never started on a device |
| D-pad focus in the Flutter screens | `lib/widgets.dart`, `lib/home_page.dart`, `lib/main.dart`, `lib/send_page.dart` | Widget tests with simulated key events; no real remote tried |
| Screen capture (Kotlin) | `android/app/src/main/kotlin/de/cometcast/cometcast/capture/` | **Compiles, never run.** Wired in code to the Rust sender through Dart (next sections) |
| Dart capture wrapper and feeder | `lib/core/android_capture.dart`, `lib/capture_feeder.dart`, `lib/sender_controller.dart`, `lib/send_page.dart` | Widget and unit tests with fakes; never run on a device |
| Rust `ExternalH264` input | `crates/cc-sender/src/external.rs`, API in `crates/cc-ffi/src/api.rs` | Tested on Linux with real H.264 end to end; the Android build of the library compiles |
| Kotlin unit tests | `android/app/src/test/kotlin/...` | `./gradlew :app:testDebugUnitTest` (see the commands) |
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

### Release build and signing

`flutter build apk --release` (and `flutter build appbundle`) use `--release` with the workspace's LTO profile for the core and the two workers. The release key is read from the **environment** (or from Gradle properties of the person building, for example `~/.gradle/gradle.properties`); nothing secret is in the repository:

| Variable | Meaning |
| --- | --- |
| `COMETCAST_KEYSTORE` | path of the keystore file |
| `COMETCAST_KEYSTORE_PASSWORD` | store password |
| `COMETCAST_KEY_ALIAS` | key alias |
| `COMETCAST_KEY_PASSWORD` | key password (optional, defaults to the store password) |

With the first three set the release build type is signed with that key. Without them it falls back to the **debug key** and the Gradle task `checkReleaseSigning` (hooked into `preReleaseBuild`) **fails on purpose** unless `-PcometcastAllowDebugSigning=true` is given (for `flutter build`, which does not forward `-P`, set `ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true` in the environment). Such an APK is for local tests only and must never be distributed. The signing path was exercised with a throwaway key: guard failing and passing, `signingReport`, and a real `flutter build apk --release --target-platform android-arm64` whose APK `apksigner` verified ("Commands that were run in the finishing session" below). A release signed with a real key, Play App Signing, and the 32-bit release were not tried. Play Store packaging (app bundle upload, Play App Signing, store listing with the 1280x720 banner Google asks for) is not part of this work.

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

After the M6 capture wiring the same `flutter build apk --debug --target-platform android-arm64` was run again: `Built build/app/outputs/flutter-apk/app-debug.apk` (about 97 MiB, 242 s with a cold Gradle cache). This only proves that the Kotlin, Dart and Rust changes compile and package. The mirror had to be extended to also stand in for `plugins.gradle.org/m2` (the plugin portal answered with Maven Central's 429 for the Flutter Gradle plugin's own dependencies).

### Gradle and Maven Central (HTTP 429) on this machine

dl.google.com, maven.google.com, services.gradle.org, plugins.gradle.org and storage.googleapis.com answered normally. `repo.maven.apache.org` (Maven Central) answered **HTTP 429 for roughly every other request**, also for single sequential `curl` GETs (observed 200, 429, 200). Gradle's built-in three retries are not enough: the first builds failed with `Plugin [id: 'org.jetbrains.kotlin.android', version: '2.4.0'] was not found` although the POM exists (a direct `curl` returned 200). Raising `systemProp.org.gradle.internal.repository.max.tentatives` to 8 with a 3 s backoff in `~/.gradle/gradle.properties` did not help within 6 minutes (UNCERTAIN whether the property was honoured). What worked: a **local pass-through mirror** (about 40 lines of Python, retrying on 429 with backoff and caching) on `127.0.0.1:18090` plus a Gradle init script in `~/.gradle/init.d/` that points `https://repo.maven.apache.org/maven2` repositories to it. Both live outside the repository because they are specific to this sandbox. With it the first complete `flutter build apk --debug` took about 3 minutes. On a normal network none of this should be needed. UNCERTAIN: whether the 429 is permanent policy or load; it may differ on another day.

### Stale ABIs and task wiring

The task `cargoNdk<Type>` deletes `build/app/rust-jniLibs/<type>` before it runs, so an ABI from an earlier build cannot end up in a later APK. Checked with two builds: `--target-platform android-arm,android-arm64` gave `lib/arm64-v8a/libcc_ffi.so` and `lib/armeabi-v7a/libcc_ffi.so` in the APK (`unzip -l`); a following `--target-platform android-arm64` build gave only the arm64-v8a one. AGP refuses Provider instances in `jniLibs.srcDir(...)` and a `files(dir).builtBy(task)` collection did not trigger the task either (tried), so the `merge*JniLibFolders` / `merge*NativeLibs` tasks depend on `cargoNdk<Type>` explicitly.

## Android TV / Google TV

Manifest (`AndroidManifest.xml`):

- `<uses-feature android.hardware.touchscreen required="false">` and `<uses-feature android.software.leanback required="false">`: installable on TVs and still on phones.
- A second intent filter with `android.intent.category.LEANBACK_LAUNCHER` on `MainActivity`, and `android:banner="@drawable/tv_banner"` on the application.
- `INTERNET`, `ACCESS_NETWORK_STATE` (no `usesCleartextTraffic`: `ws://` is opened by the Rust core with its own raw TCP sockets, which Android's cleartext policy does not govern; UNCERTAIN, not run on a device. TLS (`wss://`) verification in Rust uses `rustls-native-certs`; whether it finds Android's system CA store (it is not a plain PEM file) is UNVERIFIED and may need `rustls-platform-verifier`); for capture `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MEDIA_PROJECTION`, `POST_NOTIFICATIONS`.

Banner and icons: `python3 apps/flutter/android/tools/make_android_icons.py` (Pillow) reads `assets/logo/cometcast-app-icon-1024.png` and `cometcast-logo-dark-1024.png` and writes the launcher icons (48 to 192 px) and the TV banner. The banner must be **320x180 dp**, so it is written as 320x180 px in `drawable-mdpi`, 480x270 in `drawable-hdpi` and 640x360 in `drawable-xhdpi` (navy background taken from the icon, the logo and the words "Comet Cast"; the name is part of the picture because the launcher shows only the banner). The originals are not modified. UNCERTAIN: the banner was only looked at as a PNG, not on a TV launcher; the text uses DejaVu Sans Bold on this machine (the script falls back to Pillow's default font). Google Play wants a separate 1280x720 banner for the store listing; not made.

D-pad focus: **Flutter's built-in focus system, not the `dpad` package** (named in A2). Reason: the built-in system already binds the arrow keys to directional focus and select / Enter to activate, and the three screens are simple lists and forms; the `dpad` package was not tried, and its resolvability was not checked. What was added:

- one `FocusTraversalGroup` (reading order) per screen (`EscapeToGoBack` in `lib/widgets.dart`, and the home screen);
- `FocusRing`: a 3 px frame around the focused home row, and a thick focus border for filled, outlined and icon buttons (theme in `lib/main.dart`; Material's own overlay is faint);
- the first home row has autofocus;
- `textFieldFocusNode()`: **a finding from the test**: with the default setup the up and down arrow keys stay inside a single-line text field, so a D-pad user could not leave it. The helper maps up / down to focus movement; left / right still move the caret.

`test/dpad_focus_test.dart` (3 widget tests): arrow keys and select on the home screen; arrow keys between server field and start button on the receive screen; arrow keys through all fields of the send screen. These simulate key events only. UNCERTAIN / not tested: a real remote (Back and Home keys, long press), the on-screen keyboard that Android TV may open when a text field gets focus (entering the room code with a remote is likely awkward), overscan and 10-foot layout, focus after `Navigator.pop`, the `Material` text sizes on a TV.

## Screen capture, wired to the Rust sender (not run)

Files in `.../kotlin/de/cometcast/cometcast/capture/`:

| File | Job |
| --- | --- |
| `ScreenCaptureService.kt` | Foreground service, `foregroundServiceType="mediaProjection"` |
| `ScreenEncoder.kt` | `MediaCodec` H.264 (`video/avc`), `COLOR_FormatSurface` input, drain thread |
| `CaptureBridge.kt` | MethodChannel `de.cometcast/capture`, consent dialog, `onActivityResult` |
| `CaptureEvents.kt` | EventChannel `de.cometcast/capture/frames` |

Dart contract (implemented by `ChannelAndroidCapture` in `lib/core/android_capture.dart`):

- `invokeMethod('start', {maxDimension, fps, bitrate, notificationTitle, notificationText, notificationStop})` returns `true` (started) or `false` (the user refused). The notification texts are meant to come from the ARB keys `captureNotificationTitle`, `captureNotificationText`, `captureNotificationStop` (added in English and German); the native side contains no user-visible strings except the notification channel's technical name.
- `invokeMethod('stop')`, `invokeMethod('requestSyncFrame')` (alias `requestKeyFrame`): sets `MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME` through `setParameters`.
- The event stream emits maps: `config` (SPS/PPS, Annex B), `frame` (`ptsUs`, `keyFrame`, `data`), `stopped`, `error`. See `CaptureEvents.kt`.

Android 14 rules as implemented (from the Android documentation as understood; UNCERTAIN until run): `startForeground(..., FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)` before `getMediaProjection`; a `MediaProjection.Callback` registered before `createVirtualDisplay`; the consent result is used once and never stored, so every session asks again; `onStop` of the callback ends the session.

Robustness as implemented (UNCERTAIN until run on a device): every start command reaches `startForeground()` first (also a second start while a session runs and a missing consent), and a failing `startForeground` (for example `SecurityException`) is caught and ends the service instead of crashing; `ScreenEncoder.start()` releases the codec and surface itself when it throws and `stop()` is safe on a half-started encoder. Backpressure in `CaptureEvents`: at most 8 encoded frames wait for the main looper, frames are dropped and counted (`dropped` in each frame event) while nobody listens or the queue is full, delta frames are skipped until the next key frame after a drop, the codec config is cached and replayed on `onListen` together with a key frame request, and `stopped` is sent once per session.

### What is wired (milestone M6, Android side)

```
MediaProjection -> VirtualDisplay -> ScreenEncoder (MediaCodec, surface input)
  -> CaptureEvents -> EventChannel de.cometcast/capture/frames
  -> CaptureFeeder (Dart): config -> push_codec_config, frame -> push_encoded_frame
  -> cc-ffi (copies the bytes) -> cc-sender ExternalInput (normalise, SPS/PPS cache, bounded queue)
  -> RTP packetiser of cc-transport (one frame in flight) -> WebRTC
Rust key frame request (new peer, PLI/FIR, drop policy) -> keyframe_requested (polled every 100 ms)
  -> MethodChannel requestSyncFrame -> MediaCodec PARAMETER_KEY_REQUEST_SYNC_FRAME
```

- **Rust (`cc-sender`, `external.rs`):** input mode `ExternalH264` (`SenderOptions::external`). Accepts Annex B (3 or 4 byte start codes) and AVCC (4 byte lengths; 1 to 3 byte lengths only after an `avcC` config record named them) and converts to Annex B. Codec config as Annex B or `avcC`. SPS/PPS are cached (also from the stream) and put in front of key frames that lack them (after a leading access unit delimiter); a key frame that cannot be completed is dropped and a new one is requested. Frames are refused until a peer is connected. The queue holds at most 3 units (placeholder), the sender task writes one unit at a time; a full queue drops the new unit, counts it, and then drops delta frames until the next key frame is pushed (and requests one); a new IDR replaces everything still queued. Key frame requests are rate limited to one per second like in the X11 path. RTP time steps come from the difference of the presentation times (clamped to 1 ms to 1 s), else from the wall clock. Counters: `ExternalStats`.
- **FFI (`cc-ffi`):** `start_encoded_sender(code, server, udp_bind)` (stream of `SenderUiEvent`, with `EncodedStats` once per second), `push_encoded_frame(data, is_key, pts_us)` (sync, never blocks, **copies** `data`, returns `PushResult`), `push_codec_config(data)`, `keyframe_requested()` (poll; clears the flag). One encoded sender at a time (process-wide slot, `encoded.rs`); no `unsafe` was added outside the generated module. `capture_info` answers on Android with `Supported` and one monitor with the id `android-projection` (the size is not known to Rust).
- **Dart:** the Send screen shows "Share this screen" when `captureInfo` says supported; when the monitor list holds `android-projection` and the app has an `AndroidCapture`, the switch starts the Android flow: system consent dialog first (nothing connects if the person refuses: translated message), then `start_encoded_sender`. The notification texts come from the ARB keys `captureNotification*`. A `stopped` event (person or system ended the projection), an `error` event, the receiver leaving or a start failure stop the capture service again and show a translated message (`sendCaptureDenied`, `sendCaptureStopped`, or the technical text behind the translated "Error:" prefix). The pairing is by the monitor id, not by `Platform.isAndroid`, so the flow is testable on Linux with fakes.
- **Kotlin:** `requestSyncFrame` method added; the encoder asks for the Baseline profile (the offer says Constrained Baseline, `42e01f`) and retries without a profile if configuring fails; on API 29+ it sets `KEY_PREPEND_HEADER_TO_SYNC_FRAMES`.

### What was verified, and what was not

Verified without a device:

- Rust tests with real H.264 (`crates/cc-receiver/tests/external_h264.rs`): OpenH264 encodes synthetic pictures, the test pushes them as Annex B (SPS/PPS in band) and as AVCC (SPS/PPS only in an `avcC` record), they go through the real sender, a real `cc-server` in the process and WebRTC over loopback to the native receiver, are reassembled and decoded; the colour and a colour change are checked, and a receiver PLI reaches the host as a key frame request which the host answers. Unit tests in `external.rs` cover both formats, the 4-byte-length-looks-like-a-start-code case, garbage, SPS/PPS insertion, the bounded queue and drop counting, and the rate limit.
- Dart: widget tests of the Send screen with a fake capture and a fake core (consent pending, refused, started, stopped, error, receiver left, German texts), feeder tests, tests of the platform-channel wrapper against mocked `MethodChannel` / `EventChannel`, and one test against the real library.
- `flutter build apk --debug --target-platform android-arm64` compiles the Kotlin, Dart and Rust changes (see "Commands that were run").

**Not verified (nothing ran on a device or an emulator):**

- UNCERTAIN: what `MediaCodec` really emits on real devices: whether the codec config arrives as a separate `CODEC_CONFIG` buffer (the Kotlin side forwards it as the `config` event) or inside the first frame, whether key frames carry SPS/PPS, whether the data is Annex B (assumed, as the Kotlin comments say) or something else. Both Annex B and AVCC are handled and tested in Rust, but only with OpenH264's output, not with a hardware encoder's.
- UNCERTAIN: that the profile the encoder produces fits the offer (`42e01f`); a browser may refuse or mis-decode a High profile stream without a visible error. Level and resolution limits were not checked either.
- UNCERTAIN: Android 14 and 15 behaviour of the consent dialog, the foreground service type, the "share one app" mode, a revoked projection, the notification permission flow (all in the Kotlin code from the earlier skeleton, never run).
- UNCERTAIN: throughput of the Dart path (one platform channel message and one FFI copy per frame; the settings 1280 px, 30 fps and 4 Mbit/s are placeholders, not tuned or measured), latency, and the 100 ms polling for key frame requests.
- UNCERTAIN: behaviour on a static screen (the encoder repeats the last frame after 1 s; whether devices honour `KEY_REPEAT_PREVIOUS_FRAME_AFTER` was not checked), rotation and resize (the encoder size is fixed at the start), and Android TV devices that have no screen capture.
- UNCERTAIN: Stop while a dialog is open. `CaptureBridge` answers the pending `start` with `false`, closes the consent dialog (`finishActivity`) and ignores its result; `CaptureFeeder` stops the service again if `start` still returns `started` after `stop`, and a stale feeder (a newer session owns the singleton service) does not stop it. Tested in Dart with fakes; the Kotlin part and the dialog closing were never run.
- UNCERTAIN: RTP timestamp step on a static screen is clamped to 1 ms .. 500 ms (the pts difference when it increases, else the wall clock). 500 ms is a placeholder, not measured. SPS/PPS inside access units only update the cache from key-frame units and when at least 4 bytes long.
- Not done: audio, a native path past Dart (a possible later optimisation), the release build, a run of the GitHub workflow.

Also UNCERTAIN, all untested: encoder acceptance on weak TV boxes, behaviour when the screen is static (the format sets `KEY_REPEAT_PREVIOUS_FRAME_AFTER` = 1 s), rotation and resize (`onCapturedContentResize` and `onCapturedContentVisibilityChanged` are overridden but do nothing: the encoder size is fixed at the start), the Android 15 "share one app" mode, the `POST_NOTIFICATIONS` request (implemented in `CaptureBridge`: on Android 13+ the system dialog is shown before the consent dialog while the permission is missing; the capture continues if it is denied and only the notification is hidden), and whether Android TV devices offer MediaProjection at all (many TV boxes may not; to be checked on the Google TV Streamer).

## Receiver video: software or MediaCodec (decision)

Two paths exist; the UI picks one per device:

| Path | How | Costs | Used by default |
| --- | --- | --- | --- |
| Software | Rust decoder thread: H264Assembler -> OpenH264 -> RGB -> RGBA -> `take_frame` -> Dart `ui.Image` | One CPU decode plus two copies of an RGBA picture (Rust to Dart heap, then to the engine) per frame; OpenH264 built from source for Android (`h264-source`); software decode of 1080p30 on a weak TV box is probably too slow (UNCERTAIN, unmeasured) | Phones and tablets |
| Hardware | The decoder thread stops after depacketising: `set_video_passthrough(true)`, `take_encoded_video` -> Dart -> `MethodChannel de.cometcast/video` `queue` -> `MediaCodec` (Annex B, SPS/PPS as `csd-0/1`, size from the SPS) -> `Surface` of a Flutter `SurfaceProducer` -> `Texture(textureId)` | Only the encoded bytes (about 4 Mbit/s) cross Rust, Dart and the platform channel (two copies of a few KB per frame); no OpenH264 needed at all | Television (`UiModeManager` television or the leanback feature) |

Override for measuring: `flutter build apk --dart-define=COMETCAST_VIDEO=hardware|software|auto` (default `auto`; `main.dart`, `_androidSetup`).

Why this choice and not a different one:

- **A pure Rust path with an `AMediaCodec` NDK binding** straight into an `ANativeWindow` would avoid Dart. It needs `unsafe` NDK FFI in a crate that forbids it, the Flutter `Texture` surface is created on the Kotlin side anyway, and nothing here can be tested without a device. The Dart-mediated path is a few KB per frame; a native path is the first optimisation if the device test shows the platform channel to be the limit (UNCERTAIN: not measured).
- **Hardware on phones by default** was not chosen: the software path also supports freeze and the annotation toolbar over live video (they need the picture), and phones are strong enough for 1080p30 software decoding in all likelihood (UNCERTAIN). The device test (C1 to C4 in the plan) compares both on the same phone; the default may then change.
- **Texture vs SurfaceView / PlatformView:** `SurfaceProducer` is Flutter's supported way to give Android a surface (it handles surface recreation, and with it the callbacks `onSurfaceAvailable` / `onSurfaceCleanup`, as declared in this Flutter version's engine jar). It does not need a platform view and composes with the Flutter widgets (the stop button, the statistics).

Behaviour as built (UNCERTAIN until run):

- The decoder is configured at the first key frame with the size parsed from its SPS (`H264Util.spsSize`, unit-tested with synthetic SPS bytes) and `csd-0` / `csd-1`; a key frame with another SPS (the sender rotated) releases the codec and configures a new one. The reported size is sent to Dart (aspect ratio of the texture). The decoder is asked for low latency (`KEY_LOW_LATENCY`, API 30; `KEY_PRIORITY` 0) and every output buffer is rendered at once (no pacing).
- Backpressure: 8 units wait in Kotlin; a full queue or no free input buffer for 20 ms drops the unit, drops delta frames until a key frame, and sends `needKeyFrame` (Dart calls `request_video_keyframe`). The Rust queue of 8 units behind it works the same way.
- Backgrounding: when the texture surface is destroyed, the codec is released; when a new one exists, a key frame is requested.
- Failure: any codec error, a missing decoder or a texture that cannot be created makes the controller switch the core back to the software decoder (`hardware video decoder failed` in the log, `ReceiverController.hardwareError`). In that case the OpenH264 decoder must exist (with the `h264-libloading` release variant: Cisco's library on Android, downloaded by the codec manager, see below).
- Codec download and the hardware path: the receive screen shows the "Video codec needed" card (docs/capture.md, "H.264 licensing") only when OpenH264 is really used, that is with the software decoder (`CodecController(needed: true)`, a phone or tablet). With the MediaCodec hardware decoder active (`DeviceProfile.videoDecoder != null`, the default on a television) the card never shows and nothing is downloaded (`needed: false`; test `hardware decoder: no codec card and no download` in `test/android_tv_test.dart`). The Android sender uses MediaCodec as well and never asks for the codec (`SendPage._androidMode`). If the hardware decoder fails at run time and the controller falls back to the software decoder, OpenH264 is needed after all: the card does not appear in that case (UNCERTAIN: not run on a device; the picture then stays empty). Start-up order in `main()`: `initRustCore`, `configureCodecDir(core)` (`lib/codec_setup.dart`: on Android `getApplicationSupportDirectory()` goes to `set_codec_dir`, before any screen can ask the core for the codec state), `AppSettings.load`, then `_androidSetup` (television, device info, workers).
- Not available over the texture: the annotation toolbar and freeze (there is no picture in Dart to freeze), and the picture is not scaled to the view size by the core (the texture is scaled by the engine). The statistics overlay marks hardware decoding.

## Android TV / Google TV (as built)

Everything below is untested on a real TV or remote.

- **Launcher:** `LEANBACK_LAUNCHER` intent filter, `android:banner` on the application and on the activity, 320x180 dp banner in mdpi, hdpi and xhdpi (see above), `uses-feature` for touchscreen, leanback and Wi-Fi all `required="false"` (the Wi-Fi one because `ACCESS_WIFI_STATE` would otherwise make Wi-Fi required and exclude Ethernet-only boxes). Checked in the APK with `aapt2 dump badging` (see the commands).
- **Receiver only:** on a television (`UiModeManager` television or the leanback feature, queried through `de.cometcast/platform`) the home screen shows one large "Receive on this device" entry, focused at start, and a line saying that the TV only receives. "Send screen" and "Send file or link" are not shown: a TV has no touch screen, usually no screen capture, and the file picker may not exist. The phone and tablet layout is unchanged.
- **Receive screen:** starts by itself when opened (no "Start" to hunt for with a remote); Start/Stop has the autofocus; the room code is shown at 160 sp (scaled down to fit the width) with wide letter spacing and without the copy button; the drawing toolbar (needs a pointer) is not shown; the video toolbar (statistics, full screen, stop) stays and is reachable with the D-pad. The Back key pops the navigator.
- **Keep screen on:** while the receive view is open, `FLAG_KEEP_SCREEN_ON` is set (all Android devices, not only TVs) and cleared when it closes. UNCERTAIN: some TV firmware ignores the flag.
- **Name in the network:** the device announces its model name (`Build.MODEL`) instead of the host name, which on Android is not meaningful.
- **Focus:** Flutter's built-in focus traversal (see "D-pad focus" above), tested with key events including `select`. Not tested: the system on-screen keyboard for the server field, overscan, a real remote's long press.

## PDF and Office on Android

The workers `cc-docs-worker` (render a PDF page) and `cc-office-worker` (convert Office to PDF) are separate programs started as child processes by the Rust core. On Linux they sit next to the executable. On Android that does not work, for two reasons, found by reading the code and the Android rules (not by running anything):

1. `std::env::current_exe()` on Android is `/system/bin/app_process64` (the zygote's executable), so "next to the executable" is the system's bin directory.
2. An app cannot execute a file it wrote into its own data directory: since Android 10 (targetSdk 29 and newer) SELinux blocks `execve` there. The one place an app may execute from is the **native library directory** (`ApplicationInfo.nativeLibraryDir`), for files the package manager extracted from `lib/<abi>/lib*.so`.

Implemented (the first of the two options in the task):

- `app/build.gradle.kts`, function `registerWorkers`: per build type and ABI, `cargo ndk ... build -p cc-docs --bin cc-docs-worker` and `-p cc-office --bin cc-office-worker`, each in its own cargo invocation (features must not unify, so the Typst engine does not end up in the PDF worker), then the executables are copied to `build/app/rust-workers/<type>/<abi>/libcc_docs_worker.so` and `libcc_office_worker.so` and registered as a `jniLibs` source set. The merge tasks depend on them.
- `packaging.jniLibs.useLegacyPackaging = true` so the libraries are **extracted** to disk at install time (the equivalent of `android:extractNativeLibs="true"`); without extraction there is no file to execute. Cost: the libraries exist twice (in the APK and on disk) and the install is larger.
- The `de.cometcast/platform` channel returns `nativeLibraryDir`; at start `configureAndroidWorkers` calls `set_docs_worker` and `set_office_worker` with `<dir>/libcc_docs_worker.so` and `<dir>/libcc_office_worker.so` when those files exist.
- Trust check: `cc_docs::check_worker_file` (used for the Office worker; the PDF override path is only checked for existence) requires the file to be owned by the current user or root. Files in `nativeLibraryDir` are installed by the package manager as `system` (uid 1000), so on Android that owner is accepted too (`ANDROID_AID_SYSTEM`, `crates/cc-docs/src/isolation.rs`); the "not writable by group or others" and "directory not writable by others" rules still apply. UNCERTAIN: the owner and modes on real devices and across Android versions.
- If a worker file is missing or cannot be started the behaviour is the existing one: PDFs show the translated "the PDF component was not found" note, Office files are refused with the translated "conversion component was not found" message; nothing falls back to the in-process engine.

Honest limits: the workers are **resource-limited** (rlimits: address space, CPU, file size, open files, no core dumps) but have **no Landlock and no seccomp filter of their own on Android** (those exist for Linux x86_64 and aarch64 `target_os = "linux"` only; Android's own per-app seccomp filter and SELinux domain apply to the child as they do to the app). That is weaker than on Linux; `docs/documents.md` describes the Linux sandbox. The Typst engine in `cc-office-worker` adds several MB per ABI to the APK. Whether the process can really be started from `nativeLibraryDir` on current Android versions (including the SELinux domain of the child), and whether the rlimits can be set, was not run (C5 and C6 in the test plan). If it turns out not to work, the fallback is the second option: refuse PDF and Office on Android with the existing translated messages (remove the worker task and the Dart set-up call).

## Toolbar, content channel, Office send, links on Android

Checked by reading, not by running:

- **Nothing in the Dart code is Linux-only** except the full-screen channel (`de.cometcast/window`, which answers `MissingPluginException` on Android and is handled as "not done") and `Platform.localHostname` (replaced by the model name on Android).
- **Content channel** (images, PDFs, links, page commands): the Rust receiver and sender code is not platform specific; the PDF worker needs the set-up above.
- **Toolbar:** the annotation code is Dart over pointer events, so touch and stylus input is what it was written for. It is shown over the software-decoded live picture, images and PDF pages on phones and tablets; not over the hardware texture and not on a television (no pointer).
- **Send file or link:** the file picker is `file_selector` (Android Storage Access Framework). UNCERTAIN: the Android implementation returns a path of a copy in the app cache (not the original), which the Rust core can read; large files are copied first.
- **Links:** `url_launcher` with an external application; the plugin declares the package-visibility `<queries>` it needs. The Rust core never opens links; the person presses "Open". Media links are not played by the Flutter receiver (not built on any platform).
- **Office on a phone:** the conversion runs in `libcc_office_worker.so`; weak phones may be slow and the 1 GiB address-space limit of the worker may be too much or too little there (UNCERTAIN).

## Screen capture: lifecycle, rotation, errors (additions of the finishing session)

On top of the earlier skeleton (all UNCERTAIN until run, see the test plan B1 to B15):

- **Size changes:** a `DisplayListener` on the default display and `onCapturedContentResize` (Android 14 single-app sharing) call `scheduleResize`; after 300 ms of quiet the service builds a new `ScreenEncoder` for the new size, calls `VirtualDisplay.resize` and `setSurface` with the new encoder's input surface and stops the old encoder. The new encoder emits a new config event and starts with a key frame; the Rust sender takes the new SPS/PPS and the receiver decodes at the next IDR (the hardware decoder reconfigures itself). A failure keeps the old encoder and sends an `error` event. Pure size arithmetic in `CaptureGeometry` (JVM unit tests).
- **Encoder robustness:** configuration attempts Baseline + CBR, no profile + CBR, no profile + default rate control; the size is rounded down to the encoder's alignment and refused early if unsupported (`VideoCapabilities`); `KEY_LATENCY` and `KEY_PRIORITY` hints.
- **Background:** the service is a foreground service of type `mediaProjection` (Android 14 requirement), holds a partial wake lock (4 h safety timeout) and a Wi-Fi lock (`WIFI_MODE_FULL_LOW_LATENCY` on API 29+) while it runs, and the notification is shown at once (`FOREGROUND_SERVICE_IMMEDIATE` on API 31+), has a monochrome small icon (`ic_stat_capture`), opens the app when tapped and has a Stop action. UNCERTAIN: that the Flutter engine keeps running Dart timers and platform channels while the activity is in the background (expected, the process stays alive through the foreground service), that Wi-Fi locks are honoured, and what each manufacturer's battery saver does.
- **Lifecycle:** `stopWithTask="true"` plus `onTaskRemoved` stop the service when the task is removed; `MainActivity.onDestroy` stops it when the activity (and with it the Dart consumer) is destroyed for real (not for a configuration change, which the manifest handles with `configChanges`). The service is `START_NOT_STICKY`: after a process kill it does not come back with no consent.
- **Permission flow:** `POST_NOTIFICATIONS` (API 33+) first, then the system consent dialog every time; consent used once. `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_MEDIA_PROJECTION` declared.
- **Android 14/15 notes, from the documentation as remembered:** the foreground service type must be declared and started before `getMediaProjection`; a `MediaProjection.Callback` must be registered before `createVirtualDisplay`; a consent token may be used once; the user can pick one app (`onCapturedContentResize`, `onCapturedContentVisibilityChanged` arrive); Android 15 adds status-bar chip and lock-screen behaviours that can end a projection (handled as `onStop`).

## Commands that were run in the finishing session

On the same machine as above (no device). Rust target directories and the Gradle `build/` folder were deleted afterwards.

```sh
# Rust (host): the new pass-through tests and the cc-docs worker tests
cargo test -p cc-ffi -p cc-docs        # all passed (video.rs: 12 tests incl. 4 pass-through; cc-docs: all suites)
cargo clippy -p cc-ffi -p cc-docs --all-targets   # no warnings;  cargo fmt --check: clean
flutter_rust_bridge_codegen generate   # in apps/flutter; then cargo fmt / dart format / build_runner for the .freezed file
# Dart
flutter gen-l10n && flutter analyze    # No issues found
flutter test                           # 202 passed, 18 skipped (the real-library tests need the host libcc_ffi.so, not built to save disk)
# Android
flutter build apk --debug --target-platform android-arm64     # Built app-debug.apk (about 105 MiB: debug workers, Typst unstripped)
cd android && ./gradlew :app:testDebugUnitTest                # 16 JVM tests passed (CaptureGeometryTest 7, H264UtilTest 9)
COMETCAST_KEYSTORE=... COMETCAST_KEYSTORE_PASSWORD=... COMETCAST_KEY_ALIAS=... \
  flutter build apk --release --target-platform android-arm64 # Built app-release.apk (38.5 MB), signed with a throwaway test key
apksigner verify --print-certs app-release.apk                # verified, signer = the test key
```

Facts from those runs:

- Debug APK (arm64 only): `lib/arm64-v8a/libcc_ffi.so` 37 MB (the earlier 13.5 MB figure above was a different state of the code; not investigated), `libcc_docs_worker.so` 14.7 MB, `libcc_office_worker.so` 109 MB (unoptimised Typst), `libflutter.so` 38.8 MB. Release APK: `libcc_ffi.so` 13.7 MB, `libcc_docs_worker.so` 4.4 MB, `libcc_office_worker.so` 44.4 MB, `libflutter.so` 11.8 MB, total 38.5 MB. The Office worker is by far the biggest part; with legacy packaging (extracted libraries) the installed size is about twice the APK.
- `readelf` on the packaged `libcc_docs_worker.so`: ELF `DYN` (PIE), AArch64, interpreter `/system/bin/linker64`: a normal Android executable that merely carries a `lib*.so` name.
- `aapt2 dump badging` of the debug APK: `leanback-launchable-activity` MainActivity with `banner=res/drawable-mdpi-v4/tv_banner.png`, `uses-feature-not-required` for touchscreen, Wi-Fi and leanback, the permissions `WAKE_LOCK` and `ACCESS_WIFI_STATE` in addition to the earlier ones; banner PNGs in mdpi, hdpi and xhdpi are in the APK.
- Signing guard: without the keystore variables `:app:checkReleaseSigning` fails with the message naming them; with `-PcometcastAllowDebugSigning=true` it passes with a warning; with the three variables set `:app:signingReport` lists the release variant with the keystore and alias from the environment and the guard stays silent.
- The `:app:testDebugUnitTest` task is named with `:app:` because `./gradlew testDebugUnitTest` also runs the plugins' own unit tests, and one of them (`url_launcher_android`, `UrlLauncherTest > classMethod`) failed here; it is not part of this repository and was not investigated.
- The 429 problem of Maven Central (above) happened again in this session; the same kind of local pass-through mirror and Gradle init script was used outside the repository (and removed afterwards).

Not run: `flutter build apk` for `armeabi-v7a` or `x86_64` (the Rust core was built for 32-bit in the earlier session only, the new Kotlin and workers were not built for it), `flutter build appbundle`, the GitHub workflow, `integration_test`, anything with a device or emulator, the Rust tests with the real library in Dart (`rust_core_test.dart` skipped).

## What a real-device test must check

The full checklist is [android-device-test-plan.md](android-device-test-plan.md): install and ABI (A), phone as sender (B, including rotation, background, lock, task removal, backpressure), phone as receiver and content (C, software against hardware decoding, PDF and Office workers, toolbar), Google TV as receiver with the remote only (D), network cases (E), edge cases (F), what to record and the criteria for the A2 and A7 fallbacks (G).
