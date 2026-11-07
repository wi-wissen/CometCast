<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Android device test plan (phone, tablet, Google TV)

For a person with **one Android phone (or tablet), one Google TV (or Android TV) and a computer**. It tests the Android side of milestone M6 and the Android parts of M1 and M2 that were built to specification **without any device**: nothing in this plan has ever been run. Every expected result below is therefore an *expectation*, not a known fact; where the code or the Android documentation leaves something open it says `UNCERTAIN`. The aim is to find out what is true, and to write it down in a way that can change the docs.

Measurement points served: **A2** (Flutter UI with Rust core, Google TV operation, video through MediaCodec, 32-bit build) and **A7** (Android TV boxes as receivers) in [measurement-points.md](measurement-points.md); spikes 0, 4 and 6 of the same file. Background on what was built: [android.md](android.md).

Time: about 3 hours for the full list with one phone and one TV; part A to C alone (phone sends to a Linux or web receiver) about 45 minutes.

## 0. What you need

| Item | Notes |
| --- | --- |
| Phone or tablet, Android 10 or newer; better two (one on Android 14 or newer) | Record the exact version, the model and `adb shell getprop ro.product.cpu.abi`. Android 14 and 15 are the versions where the foreground-service and consent rules changed, so one of them is the most valuable device |
| Google TV Streamer, or another Android TV / Google TV; optionally one cheap 32-bit box | Record the same. The cheap box answers A7 |
| Computer with `adb` (Android platform-tools), the Comet Cast repository and the Android SDK (see [android.md](android.md), "Commands that were run") | USB debugging on in the phone; on the TV: developer options, then "Network debugging" or "ADB debugging", then `adb connect <tv-ip>:5555` |
| A Comet Cast server reachable by all devices | Either your own (`cometcast-server`, see [server.md](server.md)) or the default host once it exists. Put the address in the server field. All devices on the same Wi-Fi for the first runs; ICE across networks is a separate test (E) |
| A Linux machine or a browser with the web receiver | The partner for sending from the phone (Part B) and receiving on the phone/TV (Part C and D) |
| A second phone or a laptop with a screen-recording tool, and a stopwatch or a phone camera filming both screens | For the latency measurement (B6, C4) |

Build and install:

```sh
# from apps/flutter; one ABI is enough for the first run
flutter build apk --debug --target-platform android-arm64
adb install -r build/app/outputs/flutter-apk/app-debug.apk
# a 32-bit box:
flutter build apk --debug --target-platform android-arm
```

Release configuration (to measure real speed, the debug build is slower): see [android.md](android.md), "Release build and signing". Short form: set `COMETCAST_KEYSTORE`, `COMETCAST_KEYSTORE_PASSWORD`, `COMETCAST_KEY_ALIAS` in the environment and run `flutter build apk --release`.

Log collection, one terminal per device, started **before** each test and saved to a file named like `<device>-<test-id>.log`:

```sh
adb -s <serial> logcat -c
adb -s <serial> logcat -v time -s flutter:V cometcast:V AndroidRuntime:E ActivityManager:I MediaCodec:W CCodec:W > <device>-<test>.log
```

At the start the app prints one line `cometcast: device {...}` (model, Android version, ABIs, TV flag) and one with the worker paths: it belongs in every report.

## 1. What to record (for every test)

A table row per test run in a file `results/<date>-<device>.md` (not committed; attach to the issue):

`test id | device | Android version | ABI | build (debug/release, commit) | result (pass / fail / partly) | numbers | log file | screenshot or photo | what surprised you`

Numbers that matter and how to get them:

| Number | How |
| --- | --- |
| Time to first picture | Stopwatch from "receiver joined" to the first picture, or from the frame timestamps in a screen recording |
| Glass-to-glass latency | Show a running millisecond clock (a stopwatch web page) on the sending screen, film sender and receiver together with a phone camera at 60 fps or more, subtract the two readings on a still frame. Repeat 5 times, record the median |
| Frame rate shown | The statistics overlay of the receive view (key `I` on a keyboard; on a TV remote: the statistics button in the toolbar) shows the display rate, decoded, dropped, key frame requests, decode errors |
| Sender side counters | The send screen shows the encoded counters once per second (pushed, sent, dropped, key frames, key frame requests, bytes) |
| CPU and memory | `adb shell top -H -p $(adb shell pidof de.cometcast.cometcast)` and `adb shell dumpsys meminfo de.cometcast.cometcast` at the start and after 10 minutes |
| Battery or heat (phone sending) | Battery percentage over 30 minutes of mirroring, screen on |
| Crash | `adb logcat -b crash` and the `AndroidRuntime` lines; always attach |

## A. Install and first start (phone and TV)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| A1 | `adb install`; launch from the launcher | The app starts, the home screen shows. Log has `cometcast: device {...}` | `libcc_ffi.so` fails to load (look for `UnsatisfiedLinkError` or a Dart FFI error in the log). That the plain library name resolves through Android's linker namespace was never run |
| A2 | Check the ABI that was installed (`adb shell pm dump de.cometcast.cometcast \| grep -i abi`) | `primaryCpuAbi` matches the device (arm64-v8a on a modern phone and on the Streamer; armeabi-v7a on a 32-bit userland) | A 32-bit userland on a 64-bit CPU needs the `armeabi-v7a` library: install the 32-bit APK there |
| A3 | `adb shell dumpsys package de.cometcast.cometcast \| grep -i -A3 "nativeLibraryDir\|extract"` and `adb shell ls -l <nativeLibraryDir>` | Files `libcc_ffi.so`, `libcc_docs_worker.so`, `libcc_office_worker.so`, and `libflutter.so`, all extracted (real files), owner `system`, mode 755 | `extractNativeLibs` / legacy packaging: if the worker files are missing on disk, PDF and Office cannot work (C5, C6) |
| A4 | Kill the app (`adb shell am force-stop ...`), start again; rotate the device; press Home and return | State is sane, no crash | |
| A5 | Phone only: open the app's page in system settings, check requested permissions | Notifications (asked later), nothing else asked at start. No microphone, no camera, no storage | Anything unexpected is a bug |

## B. Phone or tablet as a sender (screen mirroring, M6)

Receiver for all B tests: a Linux desktop receiver, or the web receiver in a browser (the more mature path). Both must show the room code.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| B1 | Phone: Send screen. On the receiver start receiving and read the room code. Enter it on the phone, switch "Share this screen" on | Android 13 and newer: the notification permission dialog first (answer Allow). Then the **system screen capture dialog** ("Start recording or casting?"). Accept. A notification "sharing" with a Stop action appears. Within a few seconds the phone's screen shows on the receiver | Android 14 and newer: the dialog may offer "A single app" or "Entire screen" (both should work; record which one you chose). Android 15: the dialog and status bar chip look different. If the app crashes at start, the log has the `SecurityException` text (missing foreground service type, "must start foreground service first" or "must register a callback") |
| B2 | Deny the notification permission once, accept the capture | Capture works; the notification is not shown (Android hides it) | Android 13+: whether the foreground service still runs without the permission (the Android documentation says yes) |
| B3 | On the capture dialog, press Cancel or Back | The app shows the translated "screen capture was refused" message; nothing connects; no notification stays | A consent dialog left open when you leave the app |
| B4 | While mirroring: press Stop in the notification | Mirroring ends on the receiver; the app returns to a clean send screen with a translated message | Notification gone; no stale foreground service (`adb shell dumpsys activity services de.cometcast.cometcast` shows none) |
| B5 | Mirror, then tap the system "stop sharing" chip (Android 14+) or the status bar privacy indicator | Same as B4 (the projection callback `onStop`) | |
| B6 | Mirror a stopwatch page. Measure latency (see section 1), five times | Record the median. No target value is promised; the plan's own wish is "smooth enough to present". Note: first picture time, whether the picture is sharp, the frame rate | The encoder settings 1280 px, 30 fps, 4 Mbit/s are placeholders |
| B7 | Mirror a static screen (nothing moving) for 30 seconds, then start scrolling | The receiver keeps the last picture; no freeze after you start moving. Join a second receiver late (open the web receiver late): it shows a picture within a few seconds | `KEY_REPEAT_PREVIOUS_FRAME_AFTER` honoured? A receiver that joins late needs a key frame: the sender counters show key frame requests rising |
| B8 | **Rotate** the phone portrait to landscape and back, three times, while mirroring | The receiver's picture follows (new size), a short glitch is acceptable, no permanent black or frozen picture, no crash. The send screen counters keep rising | The service resizes by building a new encoder and calling `VirtualDisplay.resize` + `setSurface`; never run. If the receiver shows garbage after a rotation, record the sender log lines `Could not follow the size change` |
| B9 | **Background**: mirror, press Home, open another app for 2 minutes, scroll in it, return | The receiver keeps showing the other app's screen; on return the app is intact; counters never stalled | Does Dart keep running while the activity is in the background (the key frame polling and the frame forwarding both run in Dart)? If the receiver freezes in the background, that is the most important finding of this plan |
| B10 | Mirror, **lock the screen** for 30 seconds, unlock | Android usually ends the projection when the screen locks (Android 14+: the callback `onStop`); either result is acceptable. Record which, and that the app ends in a clean state | UNCERTAIN: device specific |
| B11 | Mirror, swipe the app out of the recents list | The capture stops, the notification disappears, the receiver sees the sender leave | `stopWithTask` plus `onTaskRemoved`; a stale notification would be a bug |
| B12 | Mirror for 30 minutes (leave the phone on a table) | No freeze, memory stays flat (`dumpsys meminfo`), battery and temperature recorded | Slow leaks in the Dart-side frame copy |
| B13 | Phone on a bad connection: switch the phone to a weak 2.4 GHz Wi-Fi or walk away from the access point while mirroring | The picture stalls and recovers with a key frame; the sender counters show `dropped` rising instead of memory growing; no crash | Backpressure: Kotlin queue of 8 frames and the Rust queue of 3 units |
| B14 | Receiver leaves (close the receiver tab) while mirroring | The phone stops the capture, shows a translated message, notification disappears | |
| B15 | Use a **second phone** with Android 12 or older (if available): B1 and B4 only | Works; older Android without the new restrictions | `startForeground(..., type)` is only passed from API 29 |

## C. Phone or tablet as a receiver, and content (M1 receive, M2 on Android)

Sender: Linux desktop, web sender (a browser with `getDisplayMedia`), or the CLI. Software decoding is the default on a phone.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| C1 | Home, Receive on this device. A room code shows. Start a sender with that code | The sender's screen fills the phone (software decoder, the RGBA picture path). Statistics: decoded rises, decode time per frame noted | OpenH264 built from source for Android: does it decode at 1080p30 on this phone? Record `avgDecodeUs` and dropped |
| C2 | Same with the build flag `--dart-define=COMETCAST_VIDEO=hardware` | The picture is a Flutter texture through the device's MediaCodec. The statistics overlay shows "(hardware decoder)" after the picture size | MediaCodec failures: the app falls back to software by itself, the log says `hardware video decoder failed: ...`. Record whether it fell back and the message |
| C3 | Compare C1 and C2 on 1080p content with fast motion (a video or scrolling text): frame rate, CPU (`top`), latency | Write down which one is better on this device. That decides the default (today: hardware on TV, software on phones) | |
| C4 | Latency with the stopwatch method (section 1), C1 and C2 | Record both medians | |
| C5 | A sender shares a **PDF** | The PDF page shows (`libcc_docs_worker.so` is started as a child process from the native library directory) | One of the most doubtful design decisions: can the app start a process from a file in `nativeLibraryDir` (Android 10 and newer block executing files from the app's data directory, but not from this one, as far as the documentation says)? If it shows the translated "PDF component was not found", look at `cometcast: docs worker ...` in the log and `adb logcat` for `Permission denied` or `exec` errors. The worker runs with resource limits only (no Landlock or seccomp of its own on Android; the app's own seccomp filter and SELinux apply) |
| C6 | Office: on the phone, Send file or link, pick a `.docx`, `.pptx`, `.xlsx` | Converted to PDF on the phone and sent (`libcc_office_worker.so`), or a translated "cannot be converted" message | The file picker returns a copy of the file in the app's cache; check that it opens. Conversion time and memory on a weak phone: record |
| C7 | An image and a link from the sender | Image shows; a link shows a card with the host; "Open" opens the system browser (the Rust core never opens it) | `launchUrl` with an external application; Android 11 package visibility |
| C8 | Toolbar over the picture (pen, highlighter, eraser, laser, undo, clear, freeze) with a finger and with a stylus if you have one, on a live software-decoded picture, an image and a PDF page | Marks appear under the finger, stay local to the phone, clear on a new page | Not in hardware mode (C2): the toolbar and "freeze" are not offered over the texture; that is by design (documented) |
| C9 | Receiving, rotate the phone | The picture re-fits; the receiver continues | |
| C10 | Receiving: the screen stays on (do not touch for 3 minutes, with the system timeout set to 30 seconds) | The display does not turn off while the receive view is open, and turns off normally after you leave it | `FLAG_KEEP_SCREEN_ON` |
| C11 | Media link from the sender | The web receiver plays media links; the Flutter receiver does not yet (not built). Expected: nothing plays, no crash | Honest "not done" check |

## D. Google TV / Android TV as a receiver (M1, A2, A7)

Use the TV remote only (no touch screen, no keyboard, no mouse), except to read logs.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| D1 | `adb connect`, `adb install`. Open the launcher's apps row. Look at the home screen of the TV | The app appears in the **leanback launcher row** with the banner (a navy tile with the logo and the words "Comet Cast"), sharp, not stretched; the focus frame around it is normal | Banner scaled wrongly or the app only shows in "all apps": photograph the row. The banner is 320x180 dp in three densities |
| D2 | Start it with the remote | Home shows **one big entry** "Receive on this device" with a hint, focused, and a line that the TV only receives. No "Send screen" and no "Send file or link" | If the TV reports as a phone (`uiMode` not television) the send entries show instead: record the model; the check is `UiModeManager` or the leanback feature |
| D3 | Press OK (center) on the entry | The receive screen opens and **starts by itself**; a very large room code shows (readable from 3 m), no copy button | Server address: the default or the saved one is used. If the server is not reachable the status says so; changing the server needs the on-screen keyboard (D6) |
| D4 | Press Back | Returns to home; the screen-on flag is released | Back handling by Flutter's navigator |
| D5 | Start a sender (phone or computer) with the room code | The picture fills the TV through the **hardware decoder** (the default on a TV). The statistics (toolbar button) shows "(hardware decoder)" | If it falls back to software: log `hardware video decoder failed`. Record the message and whether software then copes. Rotated senders: change the phone from portrait to landscape while sending, the TV picture re-fits |
| D6 | Remote navigation of every screen: Home, Receive (before start, the server field), video view with the toolbar (statistics, fullscreen, stop) | Every control is reachable with arrows, OK activates, there is always a visible focus frame, no focus trap, Back always leaves. In the server field the up and down arrows leave the field | The on-screen keyboard that opens when a text field gets focus: is it usable with the remote? Entering a server address with a remote is expected to be awkward; note how awkward |
| D7 | Leave the TV receiving the sender for 30 minutes at 1080p | No freeze, no thermal throttling visible as dropped frames, memory flat | The 30 EUR box is the lower limit of the plan |
| D8 | Screen saver / sleep: let the TV idle with the app showing the room code for the system timeout | The screen-on flag keeps it awake while the receive view is open | Some TV firmware ignores the window flag. If the TV sleeps, record it |
| D9 | Stop the sender, start another one with the same room code, then a different code | The TV shows the new sender; the old picture is gone | |
| D10 | Send a PDF and an image from the phone to the TV | Image shows. PDF shows if the PDF worker started (C5); otherwise the translated note | The picture and page path on a weak TV CPU |
| D11 | Cheap 32-bit box (A7): D1, D3, D5, D7 | Record whether it installs (ABI), starts, the picture is smooth enough, the hardware decoder path works or falls back | |
| D12 | Does the TV offer screen capture at all? The app is receiver only on a TV by design (the send entries are hidden), so this is a question about the system: check `adb shell dumpsys media_projection` and whether any system screen recorder exists | Information only: many TV boxes have no MediaProjection UI | Decides whether a sender mode on TV would ever work |
| D13 | Google TV with a Bluetooth keyboard/mouse (optional) | Everything the remote does also works | |
| D14 (new) | Force the hardware decoder to fail (a TV whose decoder rejects the stream, or kill the codec with `adb shell`), then watch the screen | The receive screen shows the translated message "The video decoder of this device failed ..." and the **Video codec needed** card with the download button (consent asked first, nothing downloaded before it); after the download the next start decodes in software | UNCERTAIN: whether a running stream recovers by itself after the download (expected: start again). Restarting the receive view must go straight to software (sticky for the app run) and keep the card |
| D15 (new) | Home, wait for the screen to be turned off by the TV and on again, and background/foreground the app while a sender is active | The picture comes back within a few seconds without a manual restart; log shows `needKeyFrame` requests about once per second, not a storm | Surface teardown: no crash, no frozen last frame, no `IllegalStateException` in logcat. UNCERTAIN: the synchronous release in `onSurfaceCleanup` |
| D16 (new) | Sender sends a 1080p screen, then a very detailed full-screen image (large key frame) | No "does not fit the input buffer" error; if the decoder reports an error it falls back to software | `MAX_INPUT` 8 MiB |
| D17 (new) | Receive for more than 13 hours (or note only: the timestamp unwrap is covered by a unit test) | The picture keeps moving past the 32 bit RTP clock wrap | Optional, long |

## E. Network cases

| Id | Steps | Expected |
| --- | --- | --- |
| E1 | Phone on mobile data, receiver on Wi-Fi at another place, over the internet (own server with TURN, see [server.md](server.md)) | Connects through the relay or directly; record which and the time to the first picture. Not claimed to work: ICE across real networks is unverified |
| E2 | Phone and receiver on different VLANs or a guest Wi-Fi with client isolation | Falls back to TURN or fails with a translated message |
| E3 | Server over `wss://` with a real certificate | The Rust core must find Android's system CA store (`rustls-native-certs` and Android's non-PEM store: UNVERIFIED, may need `rustls-platform-verifier`). If `wss://` fails with a certificate error on Android while it works on Linux, that is this problem; record the log text |
| E4 | Server over `ws://` on the local network (development) | Works (the Rust core uses its own sockets, Android's cleartext policy should not apply: UNCERTAIN) |

## F. Edge and error cases worth one try each

0. (new) Sender, rotate the device four times in quick succession while a receiver watches: the receiver never shows a garbled mix of two sizes (old frames dropped by encoder generation), the picture re-fits within about a second after the last rotation. Also: `adb shell dumpsys power | grep cometcast` shows the wake lock renewed every 5 minutes during a 20 minute session and gone after stopping. Press "Share screen" twice quickly (second tap while running): no second consent dialog, no change. Make the consent dialog fail early (deny the notification permission, then Cancel): Dart always ends in the stopped state.

1. Start sharing, then quickly press Home and revoke the notification permission in settings.
2. Start the capture while another screen recorder is running (some devices refuse a second projection).
3. Accept the capture dialog and wait 5 minutes before returning to the app (does Android 14 invalidate the token? The consent is used immediately, so it should not matter).
4. Multi-window or picture-in-picture on a tablet while mirroring.
5. Airplane mode on and off while mirroring.
6. Low memory: open a heavy app while mirroring and see whether the service survives (`START_NOT_STICKY`: after a kill it does not restart, which is intended).
7. A different locale (the phone in German): the notification texts and messages are German; the TV note is German.
8. Large font size or display size setting: the room code on the TV still fits (the code scales down, it never wraps).

## G. Reporting

For each failed or surprising result open an issue with: the test id, the log, the numbers, and the **documentation sentence** that was wrong (most of [android.md](android.md) is marked `UNCERTAIN` for exactly these cases). After a full pass, update the `UNCERTAIN` marks in [android.md](android.md), the M1 and M6 rows in [architecture.md](architecture.md) and the "Check" columns of A2 and A7 in [measurement-points.md](measurement-points.md).

Pass criteria that decide the plan's own open questions (A2 fallback in [measurement-points.md](measurement-points.md)):

| Question | Looks fine if | Fallback if not |
| --- | --- | --- |
| Is the Flutter app a usable TV receiver (A2)? | D1 to D9 pass on the Streamer: launcher banner, remote navigation without traps, hardware decoded 1080p30 for 30 minutes | Build only the TV receiver natively in Kotlin (A2 fallback) |
| Do cheap boxes cope (A7)? | D11 passes or fails with a clear message | Minimum requirements in the docs and a warning about cheap boxes |
| Does phone mirroring hold (M6)? | B1, B4, B8, B9 pass; latency is recorded | Rework the service where it failed; move the frame path from Dart into native code if B6 or B12 show the platform channel to be the limit |
