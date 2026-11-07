<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Windows screen capture test plan

For a person with **a Windows 10 (1803 or newer) or Windows 11 PC**, ideally with two monitors (one at 100 % and one
at 150 % or 200 % scale), and a receiver (browser or second Comet Cast). It tests the Windows backend of the desktop
screen picker (fix list item B7, decision N26): `GraphicsCapturePicker` and Windows.Graphics.Capture, **built to
specification and only type-checked** (`cargo check`/`clippy` for `x86_64-pc-windows-gnu`). See
[capture.md](capture.md), "Desktop capture with the system picker". Nothing here has been run.

Measurement points served in [measurement-points.md](measurement-points.md): **A10**, **A11** (software encoder only so
far), **A1**, and spike 1 "Desktop capture" (1080p30, under 200 ms, under 25 % CPU on a mid-range laptop).

Time: about 75 minutes.

## 0. What you need

| Item | Notes |
| --- | --- |
| The PC | Record `winver` (build number), CPU, GPU and driver, monitors and scale factors, and whether the app is run elevated (it should not be) |
| A build | Windows build of the Flutter app with the Rust core (`cc-ffi`); the `windows` crate code is compiled in on Windows targets by default (feature set in `crates/cc-capture/Cargo.toml`). If the build fails here, that is a result: record the first error |
| A receiver and a stopwatch | As for the Wayland plan |
| Task manager, PerfMon or `typeperf` | CPU of the app process |

## A. Picker

| # | Do | Expect |
| --- | --- | --- |
| A1 | Join a room as presenter, tap Screen | The Windows picker ("Choose what to share" with Screens and Windows) opens as a dialog **owned by the Comet Cast window** (not behind it, not on another monitor). If it opens behind the app or fails with an error about the window handle, record it: the owner window is the foreground window unless `PickerOptions::owner_window` is passed (UNCERTAIN, open question H3) |
| A2 | Pick a screen, confirm | Presenting starts; first picture on the receiver within a few seconds (record time). The system's yellow capture border is visible around the screen or window (expected; the code does not hide it) |
| A3 | Stop, tap Screen, pick one window | Only that window is on the receiver (acceptance of B7); windows that cover it are **not** in the picture (Graphics Capture reads the window itself) — record whether that is so |
| A4 | Tap Screen, then cancel (X, Esc, Cancel) | Back at "What to show", **no error page**, session still joined. Record the error code in the log: the code treats `E_POINTER`, `E_ABORT`, `E_ACCESSDENIED`, `ERROR_CANCELLED` and `S_OK` as "denied" (UNCERTAIN); any other code would show as unavailable, which is a bug to report with the hex code |
| A5 | Windows 10 older than 1803, or a Windows Server without the feature (if available) | The unavailable page; no crash |
| A6 | Tap Screen from a remote desktop (RDP) session | Record: works, picker fails, or black pictures |

## B. Picture and behaviour

| # | Do | Expect |
| --- | --- | --- |
| B1 | Share the 100 % monitor, then the 150 %/200 % monitor | Picture size is the monitor's **physical** pixels (record the receiver's size); text sharp; no offset or cropping |
| B2 | Mouse pointer over the shared screen and window | Pointer visible in the picture (cursor capture is requested; Windows 10 2004 or newer, otherwise the call is ignored). Record |
| B3 | Static screen for 30 s | Receiver keeps the picture; the sender's counters show "unchanged" polls and no encoded frames after 3 refreshes (A10); CPU near idle (record) |
| B4 | 1080p30 video full screen for 10 min | Smooth; CPU of the app (target under 25 %), latency with the stopwatch (target under 200 ms); frozen pictures |
| B5 | Resize the shared window larger and smaller, maximise, restore | Picture follows the new size within about a second (frame pool is recreated); no stretched or cropped picture; no crash. Record the receiver's size after each step |
| B6 | Minimise the shared window | Receiver keeps the last picture, no error; after restore it updates (Windows may deliver nothing while minimised: UNCERTAIN) |
| B7 | Close the shared window | The share ends: back at "What to show", no error page (`GraphicsCaptureItem.Closed`). If the picture freezes forever, record it |
| B8 | Unplug the shared monitor, or change its resolution/scale while sharing | Ends as in B7, or follows the new size (record which; the size path is `SizeTracker`) |
| B9 | Lock the PC (Win+L) and unlock; sleep and resume | Record: continues, ends or black. Never a crash |
| B10 | Click "Stop sharing" in the system's own capture border control (Windows 11 builds that have one) | Ends like the presenter's stop |
| B11 | UAC prompt (secure desktop) while sharing | Picture goes black or freezes during the prompt (known behaviour of Graphics Capture); record, then check it recovers |
| B12 | Protected content (a DRM video player) | Black rectangle expected; record |
| B13 | HDR enabled on the monitor | Record: colours washed out or correct (the pool is 8-bit BGRA) |
| B14 | Watch memory and handles of the app for 10 min | No growth (frames are replaced in a one-slot mailbox; the staging texture is reused) |
| B15 | Tap Screen twice quickly | One picker only |
| B16 | GPU without hardware D3D11 (a VM) | Falls back to WARP and still captures, slower; record |

## Report

Test ids with pass / fail / note, the Windows build, first-picture time (A2), CPU (B3, B4), latency (B4), the codes
recorded in A4, and everything that contradicts [capture.md](capture.md) (marked UNCERTAIN there).
