<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# macOS screen capture test plan

For a person with **a Mac running macOS 14 (Sonoma) or newer** (better: one on 14 and one on 15 or later, one with an
external monitor, one on an older macOS 13 to check the unavailable path) and a receiver. It tests the macOS backend
of the desktop screen picker (fix list item B7, decision N26): `SCContentSharingPicker` and ScreenCaptureKit, **built
to specification and only type-checked** (`cargo check`/`clippy` for `aarch64-apple-darwin`; never linked or run). See
[capture.md](capture.md), "Desktop capture with the system picker". Nothing here has been run.

Measurement points served in [measurement-points.md](measurement-points.md): **A10**, **A11** (software encoder only so
far; VideoToolbox is not used), **A1**, spike 1 "Desktop capture" (1080p30, under 200 ms, under 25 % CPU).

Time: about 75 minutes.

## 0. What you need

| Item | Notes |
| --- | --- |
| The Mac | Record `sw_vers`, the chip, the displays and their scaling ("Default" or "More space"), whether Stage Manager is on |
| A build | macOS build of the Flutter app with the Rust core. **The first thing to find out is whether the app links and starts at all** (never built on a Mac): record the first error. The picker runs on the main queue, so it only works inside the Flutter app, not in a command line program |
| Entitlements and bundle | Record whether the sandboxed or the non-sandboxed build is used and whether `NSScreenCaptureUsageDescription` or the screen capture entitlement is in the bundle. The code assumes the system picker needs no Screen Recording permission up front; if the stream start fails with `SCStreamErrorMissingEntitlements` (-3803) the app shows the unavailable page: record it |
| Console.app | Filter on the process name and on `ScreenCaptureKit`, `replayd`, `tccd`; save the log per test |
| A receiver, a stopwatch, Activity Monitor | As for the other plans |

## A. Permission and picker

| # | Do | Expect |
| --- | --- | --- |
| A1 | Fresh user or `tccutil reset ScreenCapture <bundle id>`; join as presenter, tap Screen | The system picker appears (windows and displays) without a prior Screen Recording prompt. If macOS asks for Screen Recording permission first, record the exact dialog and what happens after granting (UNCERTAIN in the code) |
| A2 | Pick a display, confirm | Presenting starts; the menu bar shows the system's sharing indicator; first picture on the receiver (record the time) |
| A3 | Stop; tap Screen; pick one window | Only that window on the receiver; other windows over it are not in the picture |
| A4 | Tap Screen, then cancel the picker | Back at "What to show", **no error page** (`capture_denied`), session still joined; Screen again opens the picker |
| A5 | Check System Settings > Privacy & Security > Screen & System Audio Recording | Record whether the app is listed after a share, and what happens if it is switched off and Screen is tapped again (expected: `capture_denied` or the unavailable page, never a crash) |
| A6 | macOS 13 or older | The screen-sharing page `error.capture` with the `capture_os_old` text ("Sharing the screen needs macOS 14 or later. This Mac has an older version."); buttons Show photo or file, Show a link, Back; still connected; no crash |
| A7 | Tap Screen twice quickly | One picker only |
| A8 | The picker while the app is full screen / in Stage Manager / on another Space | The picker is visible and usable; record |

## B. Picture and behaviour

| # | Do | Expect |
| --- | --- | --- |
| B1 | Share a Retina display | Receiver's size = points times scale (for example 3024x1964 on a 14-inch MacBook Pro at "Default"; the code makes the size even); text sharp. Record the receiver's size and the CPU; a 5K display gives a very large picture (UNCERTAIN: no downscale is applied) |
| B2 | Mouse pointer | Visible in the picture (cursor is requested) |
| B3 | Static screen 30 s | Receiver keeps the picture; "unchanged" polls, no encoded frames after the 3 refreshes (A10); CPU near idle (record) |
| B4 | 1080p30 video full screen 10 min | Smooth; CPU (target under 25 %), latency (target under 200 ms), frozen pictures |
| B5 | Resize the shared window, move it to another display, change the display's scaling while sharing | Record: ScreenCaptureKit scales to the configured size, so the picture size should stay the one at pick time (UNCERTAIN: no resize handling in the code; a window that becomes much larger may look blurry) |
| B6 | Minimise the shared window | Display keeps the last picture, no timeout, no message on either side (N55); after restore it updates (`Blank`/`Suspended` frame status) |
| B7 | Close the shared window, and end the share with the system's own "stop sharing" control | The presentation stops like Stop (N55): the presenter is back at "What to show" and still connected, the display goes to Idle with the same code, no toast, no error page. If the picture freezes, record it |
| B8 | Click the system's "stop sharing" in the menu bar indicator | Ends like the presenter's stop (`SCStreamErrorUserStopped` is a normal end), no error page |
| B9 | Disconnect the shared external display | Ends like B7 |
| B10 | Lock the Mac and unlock; sleep and wake | Record: continues, ends or black. Never a crash |
| B11 | Protected content (DRM video) | Black expected; record |
| B12 | Memory and CPU of the app over 10 min | No growth |
| B13 | Hardened runtime: the Developer ID build of the app | The picker and capture work; also test that Cisco's OpenH264 library still loads (open item in [releasing.md](releasing.md), "Hardened runtime and library validation") |

## Report

Test ids with pass / fail / note, `sw_vers`, first-picture time (A2), CPU (B3, B4), latency (B4), the dialogs seen in A1
and A5, and everything that contradicts [capture.md](capture.md) (marked UNCERTAIN there).
