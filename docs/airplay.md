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
| Is SIGTERM handled? | Yes: it prints `Stopping...` and exits with code 0. |

The "iPhone" in this test was one hand-written request (`POST /pair-pin-start`) sent to port 7000. A real device will send much more; the lines for a real connection, a started mirroring, a stall or a disconnect were **not observed**.

## Conflicts between the plan and the installed version (UNCERTAIN)

- The plan names UxPlay 1.73.6 and `-pw` (random PIN per connection), `-h265`, `-vrtp`. The installed 1.68 has `-pin` and none of the others. `AirplayConfig` therefore has `pin_flag` (`-pin` by default, `-pw` as option) and `h265` (off by default). `unsupported_flags()` compares our arguments with `uxplay -h`; the CLI prints a warning for unknown ones. Which names the newer versions really use: **not verified**.
- The plan example uses `-nohold`. We do **not** pass it: its help text says it drops the current connection when a new client connects, which contradicts "a running mirroring continues". What UxPlay does for a second device without `-nohold` was not tested.
- We never pass `-reg`: it keeps a register of paired devices, so returning devices could skip the PIN (taken from the help text; behaviour not tested). The plan says the same for `-pin` with `-reg`.
- We do not use `-vrtp` (stream into our own renderer). The plan says it stops sending after 5 to 60 seconds (UxPlay issue #465) and that UxPlay draws into its own window until an endurance test passes. This crate only supervises the window mode.

## Events and status

Events (`AirplayEvent`): `Starting`, `ServerStarted`, `PairingRequested`, `PinShown{pin}`, `ClientConnected`, `ClientDisconnected`, `Error`, `StreamStalled`, `Denied`, `ProcessExited`, `Restarting`, `Failed`, `Stopped`.

Status for the UI (`AirplayStatus`): `Off`, `Starting`, `Waiting`, `PinShown`, `Connected`, `Restarting`, `Failed`. `status_after()` folds events into the status.

Honest limits of the events:

- `ClientConnected` and `ClientDisconnected` come from UxPlay's line `Open connections: N`. That counts every TCP connection, including short probes (the real log shows `1` then `0` for one probe). It does **not** prove that a screen is being mirrored. **UNCERTAIN:** a line that marks the start of real mirroring was not observed.
- `StreamStalled` is matched by words ("client silence", "timed out", "timeout"). **UNCERTAIN:** UxPlay's real text is unknown. Treat the event as a hint.
- The parser ignores unknown lines. A PIN is accepted only as exactly 4 ASCII digits behind the UxPlay marker text. Newer versions may differ (**UNCERTAIN**).
- **Output is never trusted for a security decision.** The PIN is shown to people; whether a device may connect is decided by UxPlay's own PIN check. Our policy uses events only for display and for stopping the service.

## Comet Cast side policy (`policy.rs`)

| Rule | Value | Status |
| --- | --- | --- |
| Foreign devices may start a connection for | 10 minutes after "allow foreign devices" is switched on (`FOREIGN_WINDOW`) | UNCERTAIN: planning decision, not user-tested |
| Active AirPlay sessions at the same time | 1 (`MAX_ACTIVE_SESSIONS`) | UNCERTAIN: planning decision |
| After the window | New devices are rejected, a running mirroring continues, the service stops when it ends (`service_action`) | decision in the plan |
| Switch turned off during a running mirroring | The picture is kept (our choice, the plan does not say) | UNCERTAIN |
| Confirmation dialog | `ConfirmHook` trait, the app implements it (shows the PIN, answers `Accept` or `Deny`). `Deny` restarts UxPlay, which drops the pairing | UNCERTAIN: the output does not name the device, so one device cannot be blocked individually |

The policy functions only decide. Connecting them to the real switch, timer and UxPlay (counting active sessions, stopping the service) is left to the app. The window start time and the session count are inputs of `PolicyState`.

## Process handling

- `Supervisor::start(config, options)` returns the supervisor and a `std::sync::mpsc::Receiver<AirplayEvent>`.
- Restart: wait 1 s, then 2 s, 4 s ... up to 30 s; a run longer than 30 s resets the wait; after 10 failures in a row it gives up (`Failed`). All values UNCERTAIN (placeholders, not tuned on real hardware).
- If the program cannot be started at all (not installed), it reports `Failed` once and does not retry.
- Stop: SIGTERM, wait up to `stop_timeout` (5 s, UNCERTAIN), then SIGKILL to the process group, then the process is collected (`wait`). The tests check with `waitpid` and `/proc` that no zombie is left.
- Windows: `portable-pty` supports ConPTY, and the code has a non-Unix path (`kill`), but **nothing was compiled or run on Windows or macOS**. UxPlay on Windows is a separate topic in the plan (MSYS2 build).

## Tests

`cargo test -p cc-airplay` runs unit tests (parser with real fixtures, config, policy, backoff) and supervisor tests with a **fake** `uxplay` shell script (`tests/fixtures/fake-uxplay.sh`; it prints lines copied in style from the real output, it is not UxPlay). One more test runs the real program and is off by default:

```
DISPLAY=:99 COMETCAST_REAL_UXPLAY=1 cargo test -p cc-airplay real_uxplay -- --nocapture
```

It needs `uxplay`, the GStreamer plugin packages, Avahi and an X display (for example `Xvfb :99`). Fixtures from the real program are the files named `uxplay-1.68-real-*.txt`.

## Limits

- Not tested with any Apple device. iOS 26.6 and 27 behaviour is taken from the plan (sources there), not from our tests.
- No audio-only AirPlay (music) handling beyond what UxPlay does itself.
- No integration yet with `cc-receiver`, the apps or the render path.
- A PIN printed in the process output can also be seen by other local users who can read the process's terminal output (the supervisor does not log it, the CLI prints it). **UNCERTAIN:** whether that matters for a shared display device.
