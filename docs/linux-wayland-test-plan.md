<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Linux Wayland screen capture test plan

For a person with **a Linux desktop session on Wayland** (ideally one machine each with GNOME, KDE Plasma and a
wlroots compositor such as sway) and a second device or browser as the receiver. It tests the Wayland backend of
the desktop screen picker (fix list item B7, decision N26) that was **built to specification without any compositor**:
see [capture.md](capture.md), "Desktop capture with the system picker". Nothing here has been run.

Measurement points served in [measurement-points.md](measurement-points.md): **A10** (efficient image transfer:
unchanged frames are not encoded, static refresh, pointer), **A11** (hardware encoders: only the software encoder is
used so far), **A1** (the stream reaches the receiver), and spike 1 "Desktop capture" (1080p30 to the receiver,
under 200 ms delay, under 25 % CPU on a mid-range laptop, "Wayland dialog once per session").

Time: about 90 minutes per compositor.

## 0. What you need

| Item | Notes |
| --- | --- |
| A Wayland session | Record `echo $XDG_SESSION_TYPE $WAYLAND_DISPLAY $XDG_CURRENT_DESKTOP`, the compositor version, `pipewire --version`, `xdg-desktop-portal --version` and which backend is installed (`xdg-desktop-portal-gnome`, `-kde`, `-wlr`, `-gtk`, `-hyprland`). `-wlr` and `-gtk` alone do **not** offer ScreenCast pickers for every case; `-gtk` has no ScreenCast at all |
| Build | `sudo apt install libpipewire-0.3-dev libclang-dev`; then the Flutter Linux app as usual: its CMake step also builds `libcc_capture_pw.so` (`crates/cc-capture-pw`) and installs it into `bundle/lib/`. The app itself does not link libpipewire (decision N57): it loads the library with dlopen when Screen is tapped. Without the library, or on a machine without `libpipewire-0.3.so.0`, the app starts and Screen on Wayland shows the page `error.capture` with `errors.capture_no_portal` (no fallback to XWayland, by design). `cargo build -p cc-ffi --features wayland` still links the backend into the program (development) |
| A receiver | The web app in a browser or a second Comet Cast, on the same network, joined to the same room |
| A stopwatch or phone camera filming both screens | For latency |
| `top`/`htop`, `pw-top`, `journalctl --user -f` | CPU and PipeWire/portal messages. Start `journalctl --user -f -u xdg-desktop-portal -u 'xdg-desktop-portal-*' > portal.log` before each run |

Keep one log file per test (`<compositor>-<test id>.log`). Write down for every test: compositor, scale factor,
monitor layout, what the picker showed, what the receiver showed.

## A. Before the picker (portal detection)

| # | Do | Expect |
| --- | --- | --- |
| A1 | Start the app on the Wayland session, join a room as presenter, tap Screen | The compositor's own picker opens at once. No Comet Cast page before it (except the codec page when the encoder is missing). The picker names **Comet Cast** (the app id `de.cometcast.cometcast` is registered with the portal and `de.cometcast.cometcast.desktop` is installed, e.g. to `/usr/share/applications`); record what it shows when the desktop entry is not installed. Note the picker's look and which choices it offers (monitors, windows, both) |
| A2 | Run `busctl --user introspect org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.ScreenCast` | `AvailableSourceTypes` and `AvailableCursorModes` exist (record the values: 1 monitor, 2 window; 1 hidden, 2 embedded, 4 metadata). If this fails the backend order must give `capture_no_portal` on Wayland: check that the app shows the screen-sharing page `error.capture` (title "Screen sharing does not work here", the `capture_no_portal` text, buttons Show photo or file, Show a link, Back; still connected) and no crash |
| A3 | Stop the portal (`systemctl --user stop xdg-desktop-portal`) and tap Screen | Either the portal is restarted by D-Bus activation and A1 works, or the app shows `error.capture`. Never a hang. Record which |
| A4 | Run on Wayland **without** `libcc_capture_pw.so` (delete it from `bundle/lib/`) and, separately, on a machine without `libpipewire-0.3.so.0` | The app starts (no loader error). Screen shows `error.capture` with the `capture_no_portal` text. On an X11 session (log in with "GNOME on Xorg" or similar) the X11 whole desktop is shared without a picker (the fallback) |
| A5 | X11 session **with** the feature | The portal picker is used if a portal answers (the order is portal first), otherwise the X11 fallback. Record which |

## B. Picking and cancelling

| # | Do | Expect |
| --- | --- | --- |
| B1 | Pick a whole monitor, confirm | Presenting starts; the receiver shows the monitor within a few seconds (note the time from confirm to first picture; `FIRST_FRAME_TIMEOUT` is 10 s, a placeholder) |
| B2 | Stop presenting, tap Screen again, pick a **window** | Only that window is shown on the receiver (acceptance of B7). Record whether the window contents of other windows ever leak in |
| B3 | Tap Screen, then cancel the picker (Esc and the Cancel button, separately) | Back at "What to show" with **no error page** (`capture_denied`). The session stays joined. Tap Screen again: the picker opens again. Record in the log what the portal answered (`Response` 1 or 2: the code treats both as denied, which is UNCERTAIN) |
| B4 | Tap Screen, confirm with nothing selected (if the picker allows it) | Same as B3 |
| B5 | Does the picker open **every time** Screen is tapped? | Yes (decision N55: portal restore tokens stay off, no "remember" option). Record if not |
| B6 | With a restore token enabled by hand (set `PickerOptions::use_restore_token`, rebuild) pick a monitor, stop, share again | The portal skips its picker; the token file exists with mode 0600 under `~/.local/share/CometCast/screencast/`; revoke the permission in the system settings and share again: the picker appears and the stale token is deleted |

## C. Picture quality and behaviour

| # | Do | Expect |
| --- | --- | --- |
| C1 | Share a monitor at scale 100 %, then 200 %, then 150 % (fractional) | The receiver's picture is the monitor's **pixel** size (check the receiver's reported size: 1920x1080 on a 1920x1080 panel at any scale; with the logical size 1280x720 at 150 % the picture must still be 1920x1080, not 1280x720). Text sharp, not stretched. If the picture is blurry or the wrong size, record the compositor's behaviour (some give logical size) |
| C2 | Two monitors with different scales | Pick each in turn; each gives its own size; no black or doubled areas |
| C3 | Move the mouse over the shared monitor and over the shared window | Pointer visible in the picture where the portal embeds it (GNOME, KDE). On compositors that only offer hidden or metadata mode the pointer is missing (A10: separate pointer message not built); record |
| C4 | Leave the screen static for 30 s | Receiver keeps the last picture; CLI statistics or logs show polls with "unchanged" and no encoded frames after the 3 refreshes (A10). CPU of the app near idle (record %) |
| C5 | Play a 1080p30 video full screen in the shared monitor for 10 min | Smooth on the receiver; record CPU of the app (target of the spike: under 25 % on a mid-range laptop), end-to-end latency with the stopwatch (target under 200 ms), frozen pictures, dropped frames (`pw-top` errors column) |
| C6 | Resize the shared window, maximise it, minimise it, restore it | Picture follows the size (the encoder restarts at the new size). While minimised the display keeps the last picture, no timeout, no message on either side (N55); after restoring it updates. Record what each compositor does while minimised (stream paused?) |
| C7 | Close the shared window | The presentation stops like Stop (N55, `PresentingStopped`): the presenter is back at "What to show" and still connected, the display goes to Idle with the same code, no toast, no error page (`Capture::Ended`). If instead the picture freezes forever, record the compositor and what `pw-cli ls Node` shows: the code reacts to the removal of the node and to the portal's `Closed` signal |
| C8 | Unplug (or disable in settings) the shared monitor | Same as C7 |
| C9 | Lock the screen and unlock; suspend and resume | Record: does the stream continue, end or freeze? Expected by design: ends or continues, never a crash |
| C10 | End the share from the compositor's own indicator (GNOME top bar "stop sharing", KDE tray, sway notification) | The portal session is closed; the app returns to "What to show" without an error page (same as the presenter's stop) |
| C11 | Run `pw-dump` while sharing | One node of ours (`cometcast-screen`); after stopping, no leftover node or link |
| C12 | Look at the memory of the app over 10 min | No growth (frames are replaced in a one-slot mailbox) |

## D. Failure cases

| # | Do | Expect |
| --- | --- | --- |
| D1 | Compositor sends DMA-BUF only (some drivers/compositors with modifiers): check the log of the app | The stream fails with the message "the compositor sent DMA-BUF buffers although only shared memory was offered" and the app shows the page `error.capture` or ends the share like Stop (record which). Record the compositor, GPU and driver; this is the case that DMA-BUF import would fix |
| D2 | Kill PipeWire (`systemctl --user restart pipewire`) while sharing | Share ends (error or ended), no hang, no crash |
| D3 | Start two shares quickly (tap Screen twice) | One picker only; no second portal session left over (`busctl --user tree org.freedesktop.portal.Desktop`) |
| D4 | Run `tools/pipewire-loopback-test.sh` on this machine | Passes. This only checks the PipeWire half with a fake producer |

## Report

Per compositor: a table of the test ids with pass / fail / note, the first-picture time (B1), CPU (C4, C5), latency
(C5), the answers recorded in A2, B3, B5, C1, C3, C6, C7, C9, and the portal log. Send what contradicts
[capture.md](capture.md) (marked UNCERTAIN there) so the text can be corrected.
