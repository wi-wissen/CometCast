# AirPlay receiving through UxPlay (M3)

Status: the supervisor library and a CLI command exist. **No iPhone, iPad or Mac was available**, so nothing here was tested with a real Apple device. Measurement point **A3** (see [measurement-points.md](measurement-points.md)) stays a beta measurement.

## What it does

A mini PC with Comet Cast can show the screen of an iPhone, iPad or Mac that uses the normal AirPlay "Screen Mirroring" button. The receiving part is done by [UxPlay](https://github.com/FDH2/UxPlay), an existing open-source program. Comet Cast does not receive AirPlay itself. It only:

1. builds a safe command line for `uxplay` from a typed config (`AirplayConfig`),
2. starts the program and reads its text output,
3. turns interesting lines into typed events (`AirplayEvent`) on a channel,
4. restarts the program with a growing wait if it dies by itself,
5. stops it cleanly (SIGTERM, kill after a timeout, no zombie process),
6. applies Comet Cast's own rules (window, one session, confirmation dialog hook).

Code: `crates/cc-airplay`. Dev command: `cometcast-cli airplay [--name NAME] [--uxplay-path PATH]`.

## The licence boundary

UxPlay is GPL-3.0 (the Ubuntu package lists parts as LGPL-2.1-or-later and MIT, checked in `/usr/share/doc/uxplay/copyright` of package 1.68.2). It runs as a **separate process**: we start the binary, give it arguments and read its output. We do not link it, include its source or port its code. The project plan records this decision; it treats the FSF "mere aggregation" argument as the basis, with its application to our case as an **assumption**.

What must be shipped when a Comet Cast release contains a UxPlay binary (**UNCERTAIN: this is a technical reading of the GPL-3.0 duties, not legal advice; have it reviewed before the first release that ships UxPlay**):

- the GPL-3.0 licence text,
- a written offer of the matching UxPlay source code, or the source itself (for the exact version that is shipped),
- the notices of UxPlay's own bundled parts (LGPL-2.1-or-later and MIT parts according to the Debian copyright file),
- UxPlay's GStreamer and other dependencies with their own licences, when they are shipped too.

If UxPlay is only installed by the user (for example `apt-get install uxplay`) and not shipped by us, the duties of shipping fall to whoever distributes it. **UNCERTAIN:** whether that changes our own duties.

Not allowed (decision in the plan): linking UxPlay, copying its code, or porting it to Rust. This crate contains none of it. The fixtures in `crates/cc-airplay/tests/fixtures/` hold only short output lines and the help text of the program as plain test data. **UNCERTAIN:** whether quoting the program's help text in a test file needs a notice; it is kept because it is the evidence for the flag check.

This crate also contains nothing about decrypting the FairPlay handshake, no keys and no key extraction. UxPlay does all of that inside its own process. The plan notes that the legal classification of that handshake is not finally settled; that is why the module is optional and separate.

## What was checked against the real program

Checked on 2026-11-06 in a Linux container with the Ubuntu package `uxplay 1.68.2-1build2` (the program reports "UxPlay 1.68"), GStreamer 1.24, Xvfb and Avahi:

| Question | Result |
| --- | --- |
| Does it start without a window/sound card? | Yes with an X display (Xvfb) and `-vs fakesink -as 0`. It needs the GStreamer plugin packages `plugins-base`, `-good`, `-bad` and a running DNS-SD service (Avahi). Without them it prints errors and exits: `Required gstreamer plugin 'app' not found` and `*** ERROR: No DNS-SD Server found ...`. These lines are real fixtures. |
| Does the PIN line appear through a normal pipe? | **Not in time.** With a pipe, UxPlay's output stayed in a buffer and arrived only when it exited (about 2 seconds later in our test, at the end). |
| Does a pseudo-terminal (PTY) fix it? | Yes. With the PTY the supervisor saw the PIN 4 ms after the (simulated) pairing request. `stdbuf -oL` in front of the program also works (tested through a pipe), but needs a Unix `stdbuf`. |
| How does the PIN line look? | `*** CLIENT MUST NOW ENTER PIN = "8227" AS AIRPLAY PASSWORD`, followed by a big ASCII-art picture of the digits (without the PIN in plain text on those lines). |
| Which flags does 1.68 know? | `-n`, `-nh`, `-pin[xxxx]`, `-reg`, `-fs`, `-p`, `-vs`, `-as`, `-nohold`, `-restrict`, `-reset`. **Not** `-h265`, `-vrtp`, `-artp`, `-pw` (the full help text is a fixture). |
| Is SIGTERM handled? | Results differed between two runs: with SIGTERM to the process only, the supervisor saw exit code 0; with SIGTERM to the process group (current code), it saw death by the signal ("Terminated"). **UNCERTAIN** why. Either way UxPlay ended within milliseconds and was reaped. The supervisor reports a signal death as a signal name, never as an exit code. |

The "iPhone" in this test was one hand-written request (`POST /pair-pin-start`) sent to port 7000. A real device will send much more; the lines for a real connection, a started mirroring, a stall or a disconnect were **not observed**.

## Conflicts between the plan and the installed version (UNCERTAIN)

- The plan names UxPlay 1.73.6 and `-pw` (random PIN per connection), `-h265`, `-vrtp`. The installed 1.68 has `-pin` and none of the others. `AirplayConfig` therefore has `pin_flag` (`-pin` by default, `-pw` as option) and `h265` (off by default). `unsupported_flags()` compares our arguments with `uxplay -h`; the CLI prints a warning for unknown ones. Which names the newer versions really use: **not verified**.
- The plan example uses `-nohold`. We do **not** pass it: its help text says it drops the current connection when a new client connects, which contradicts "a running mirroring continues". What UxPlay does for a second device without `-nohold` was not tested.
- We never pass `-reg`: it keeps a register of paired devices, so returning devices could skip the PIN (taken from the help text; behaviour not tested). The plan says the same for `-pin` with `-reg`.
- We do not use `-vrtp` (stream into our own renderer). The plan says it stops sending after 5 to 60 seconds (UxPlay issue #465) and that UxPlay draws into its own window until an endurance test passes. This crate only supervises the window mode.

## Events and status

Events (`AirplayEvent`): `Starting`, `ServerStarted`, `PairingRequested`, `PinShown{pin}`, `ClientConnected`, `ClientDisconnected`, `Error`, `StreamStalled`, `Denied`, `DenyIgnored`, `SessionChanged`, `ProcessExited{code, signal, pid, expected}`, `Restarting`, `Failed`, `Stopped`.

Status for the UI (`AirplayStatus`): `Off`, `Starting`, `Waiting`, `PinShown`, `Connected`, `Restarting`, `Failed`. `status_after()` folds events into the status. `Connected` is set only by `SessionChanged{active: true}`, which the **app** sends with `Supervisor::set_session_active(true)` (UxPlay's output has no line we trust for "a screen is mirrored"). A `PinShown` event while the status is `Connected` does not change it.

Honest limits of the events:

- `ClientConnected` and `ClientDisconnected` come from UxPlay's line `Open connections: N`. That counts every TCP connection, including short probes (the real log shows `1` then `0` for one probe). It does **not** prove that a screen is being mirrored. **UNCERTAIN:** a line that marks the start of real mirroring was not observed.
- `StreamStalled` is matched by words ("client silence", "timed out", "timeout"). **UNCERTAIN:** UxPlay's real text is unknown. Treat the event as a hint.
- The parser ignores unknown lines. Every pattern is anchored to the start of the line. The PIN line must be exactly `*** CLIENT MUST NOW ENTER PIN = "dddd" AS AIRPLAY PASSWORD` (4 ASCII digits, nothing before or after); the pairing line must be exactly `client sent PAIR-PIN-START request`. A marker in the middle of a line (for example inside a device name) is ignored; tests cover spoofed lines. Newer UxPlay versions may use other texts (**UNCERTAIN**).
- The supervisor emits `PinShown` only when a pairing line came right before it. A device that can put a line break into text that UxPlay logs could still fake both lines (**UNCERTAIN**: which logged texts a device controls was not checked). **The UI must never show or trust a PIN that did not come from a `PinShown` event, and no access decision may rest on it.** The PIN is shown to people; whether a device may connect is decided by UxPlay's own PIN check. Our policy uses events only for display and for stopping the service.
- `ClientConnected` needs the line `Open connections: N`. In the real non-debug run (the PIN fixture) that line did **not** appear; it appeared only with UxPlay's `-d` (debug) option, which `AirplayConfig` does not pass. So by default `ClientConnected` and `ClientDisconnected` never fire; they do not drive the status.
- `StreamStalled` is a hint only (see above).

## Comet Cast side policy (`policy.rs`)

| Rule | Value | Status |
| --- | --- | --- |
| Foreign devices may start a connection for | 10 minutes after "allow foreign devices" is switched on (`FOREIGN_WINDOW`) | UNCERTAIN: planning decision, not user-tested |
| Active AirPlay sessions at the same time | 1 (`MAX_ACTIVE_SESSIONS`) | UNCERTAIN: planning decision |
| After the window | New devices are rejected, a running mirroring continues, the service stops when it ends (`service_action`) | decision in the plan |
| Switch turned off during a running mirroring | The picture is kept (our choice, the plan does not say) | UNCERTAIN |
| Confirmation dialog | `ConfirmHook` trait, the app implements it (shows the PIN, answers `Accept` or `Deny`). `Deny` restarts UxPlay, which drops the pairing. Only one dialog is open at a time; a PIN that appears while a dialog is open gets no dialog (dropped, not queued). While the app says a session is active (`set_session_active(true)`), no dialog is opened and a late `Deny` is ignored (`DenyIgnored`), so a running picture is never cut | UNCERTAIN: the output does not name the device, so one device cannot be blocked individually |

The supervisor enforces none of these rules by itself. With the default `confirm: None` there is no dialog, and every device that knows the PIN is accepted by UxPlay. The window, the session count and the call to `set_session_active` are the **app's job**.

The policy functions only decide. Connecting them to the real switch, timer and UxPlay (counting active sessions, stopping the service) is left to the app. The window start time and the session count are inputs of `PolicyState`.

## Process handling

- `Supervisor::start(config, options)` returns the supervisor and a `std::sync::mpsc::Receiver<AirplayEvent>`.
- Restart: wait 1 s, then 2 s, 4 s ... up to 30 s; a run longer than 30 s resets the wait; the 10th failure in a row gives up (`Failed`). `Restarting{attempt}` is the number of failures in a row (1 = first); planned restarts (`restart()`, denial) do not count and reset the wait. All values UNCERTAIN (placeholders, not tuned on real hardware).
- If the program cannot be started at all (not installed), it reports `Failed` once and does not retry.
- Stop: SIGTERM to the whole process group, wait up to `stop_timeout` (5 s, UNCERTAIN), then SIGKILL to the group, then the process is collected (`wait`). After the leader has ended (also when it died by itself), the rest of its group gets SIGKILL, so helper processes do not survive. The tests check with `waitpid` and `/proc` that no zombie is left, and that a backgrounded helper of the fake is dead after a stop and after a crash. A helper that leaves the group (its own `setsid`) is not caught (**UNCERTAIN**: not known whether UxPlay or GStreamer start such helpers); its output thread then stays blocked, but its events are dropped by a run-number check.
- Commands that pile up during a stop are dropped (except a stop).
- Platforms: everything is written and tested on **Linux only**. The signal and process-group code is `#[cfg(unix)]` (macOS should compile it, **unverified**). On other systems the code falls back to `portable-pty`'s `kill()`, which means no graceful SIGTERM, no group cleanup, and the supervisor tests are Unix-only. Nothing was compiled or run on Windows or macOS; `portable-pty` claims ConPTY support there (**unverified**). The `nix` dependency is Unix-only. UxPlay on Windows is a separate topic in the plan (MSYS2 build).

## Tests

`cargo test -p cc-airplay` runs unit tests (parser with real fixtures, config, policy, backoff) and supervisor tests with a **fake** `uxplay` shell script (`tests/fixtures/fake-uxplay.sh`; it prints lines copied in style from the real output, it is not UxPlay). One more test runs the real program and is off by default:

```
DISPLAY=:99 COMETCAST_REAL_UXPLAY=1 cargo test -p cc-airplay real_uxplay -- --nocapture
```

It needs `uxplay`, the GStreamer plugin packages, Avahi and an X display (for example `Xvfb :99`). Fixtures from the real program are the files named `uxplay-1.68-real-*.txt`.

Timing in the supervisor tests: they wait for events (`collect_until`) with a 60 s deadline instead of fixed sleeps, and the tests use a long `stop_timeout` / `reader_grace` where a short one would only make them depend on machine speed. The waits end as soon as the event arrives, so the generous deadline costs nothing when the machine is idle. Only the timeouts that are the subject of a test stay short (the kill of a process that ignores SIGTERM, the tiny backoff). `SupervisorOptions::reader_grace` (default 1 s, unchanged) is how long the manager waits for the output thread to deliver the last lines of an ended run before it sends the exit event; on a heavily loaded machine a 1 s limit could drop a late `Error` line, so the tests set it to 30 s. Under `cargo test --workspace` on a loaded machine these tests were reported to fail with the old fixed limits; this was not reproduced here (see the commit message), so the cause above is the likeliest one, not a proven one.

## Limits

- Not tested with any Apple device. iOS 26.6 and 27 behaviour is taken from the plan (sources there), not from our tests.
- No audio-only AirPlay (music) handling beyond what UxPlay does itself.
- No integration yet with `cc-receiver`, the apps or the render path.
- A PIN printed in the process output can also be seen by other local users who can read the process's terminal output (the supervisor does not log it, the CLI prints it). **UNCERTAIN:** whether that matters for a shared display device.

> The table of rules above (10-minute window, foreign devices switch) is the first version; round 5 replaced it by the join mode, see the next section.

## Round 5: receiving as a display feature (native-apps.md 21.4, spec 15.4)

The display app of a mini-PC (Linux, macOS, Windows) runs UxPlay itself, through `cc-airplay`'s `AirplayReceiver` (`crates/cc-airplay/src/receive.rs`) and the `cc-ffi` calls `airplay_receive_available`, `start_airplay_receive`, `airplay_set_ask_first`, `airplay_set_locked`, `airplay_set_refusing`, `airplay_answer`, `airplay_end_session`, `stop_airplay_receive`. Status: **built and tested against the fake UxPlay script and the installed UxPlay 1.68 text; never tried with a real iPhone.**

- **Admission is the join mode, not a time window.** The 10-minute `FOREIGN_WINDOW` and `since_enabled` are gone. `policy::admit_new_session` gives Allow (Open: the PIN is shown at once), Ask (Approval on: `Request` first, then `Pin`), or Reject (Locked, one session already, or a Comet Cast presenter presents). A refusal restarts UxPlay, which drops the pairing; the device shows its own failure. `service_action` says Stop for Locked once no mirroring runs; a running mirror stays until it ends. A request that nobody answers is refused after 60 s (`REQUEST_TIMEOUT`, UNCERTAIN, the app answers earlier when its own dialog ends).
- **A PIN is always required.** `-pin` without a value for every connection; `-reg` is never passed (tested: `pin_is_always_requested_and_never_remembered`). `-d` is always passed for receiving (see the signal).
- **The mirroring signal** (`Mirroring { active }`, blocking for release): the engineers chose UxPlay's **own output lines**, not the window. *Start*: the line `raop_rtp_mirror starting mirroring` (`parser::MIRRORING_STARTED_LINE`, a format string of `raop_rtp_mirror.c`; found in the text of the installed 1.68 binary), believed only after a PIN was shown in the same run (a device that never paired cannot start it, whatever text it makes UxPlay print; anchored match, tested). *End*: `Open connections: 0` (UxPlay debug output, hence `-d`; the connection count of its HTTP server) or the end of the UxPlay process (the supervisor reports the end before the exit). **Not verified:** the log level of the start line (with `-d` every level is printed, so it does not matter), and that a real iPhone produces exactly one start line per mirroring and the count reaches 0 when it stops (device test plan item to add). `has_mirroring_signal(path)` looks for both texts in the program file, and `airplay_receive_available()` is false for a UxPlay without them, so release builds with such a UxPlay do not list AirPlay.
- **UxPlay found:** next to the running executable (where packages put it), then `PATH` (`locate_uxplay`). The Linux `.deb` has `Recommends: uxplay, gstreamer1.0-plugins-{base,good,bad}, avahi-daemon` (`release.yml`); macOS bundle and Windows packaging (MSYS2 build) are not done (item C10, UNCERTAIN). GPL-3.0 licence files of UxPlay and its bundled parts must be copied to `apps/flutter/assets/licenses/` where it is shipped (not done: nothing is shipped yet).
- **Window.** UxPlay runs with `-fs`; `AirplayConfig::x11_display` sets its `DISPLAY` so the window opens on the screen of the Comet Cast window. UxPlay has no option to choose a monitor of one X screen; on Wayland and on macOS/Windows the window lands where the sink puts it. For a dialog during a mirror the Dart side calls `raiseWindow(true/false)` (`lib/window_control.dart`): the Linux runner implements `raise` (`gtk_window_set_keep_above`), Windows and macOS do not yet (the call answers false, the request then waits for the server's expiry).
- **Limits (Beta, not shown on screen):** the host cannot press End over the UxPlay window; the mirror ends on the sending device, by switching AirPlay off in Settings, or by a Comet Cast presentation (`airplay_end_session`).

## Device test plan: receiving with a real iPhone (release gate, spec 15.14 N115, native-apps.md 21.10 item 2)

**Status: not run** (no iPhone, iPad or Mac was available). AirPlay receiving stays out of release builds until this plan passed on a real device with the packaged UxPlay. Machine: a Linux, macOS or Windows computer with the display app and the UxPlay that the package ships or recommends; one iPhone or iPad on the same network.

| # | Do | Expect | Record |
| --- | --- | --- | --- |
| A1 | Display Settings, AirPlay on (Open). On the iPhone: Control Center, Screen Mirroring, pick the room name | The PIN dialog shows four digits; the iPhone asks for them; after entering them the iPhone screen is shown in UxPlay's window | Time until the room name is listed; the exact text of the PIN prompt |
| A2 | Read the UxPlay output of A1 (`-d` is always passed) | **Exactly one** line `raop_rtp_mirror starting mirroring` per mirroring, after the PIN was shown in the same run | The whole log of one mirroring, with the log level of that line |
| A3 | Stop mirroring on the iPhone | `Open connections: 0` appears and the app reports `Mirroring { active: false }` (the display is Idle again, the wake lock is released) | Whether the count reaches 0 after every stop; whether a second line `Open connections: 0` or a stuck count of 1 happens |
| A4 | Mirror, stop, mirror again, five times; then mirror and switch the row off in Settings | One start line and one end per round; the end of the process ends the mirror | Any round with two start lines or no end |
| A5 | Approval on: the request dialog (Yes, No) first, then the PIN; No refuses | The iPhone shows a failure; no PIN appears | The iPhone's wording |
| A6 | Locked while a mirror runs; then Locked while idle | The running mirror stays; idle: the room name is no longer listed within 10 s | The time |
| A7 | A Comet Cast presenter starts while the iPhone mirrors | The mirror ends with `toast.rxEnded`; while the presenter presents a pairing is refused without a dialog (N88) | How the iPhone reacts |
| A8 | `airplay_receive_available()` on the packaged build | True only with a UxPlay that has both texts; the licence list has the entry "UxPlay" (GPL-3.0) second | The UxPlay version |

After the run: replace the **UNCERTAIN** lines about the mirroring signal in this file by what was seen (device, iOS version, UxPlay version) and report each deviation as an issue.

