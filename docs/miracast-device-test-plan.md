<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Miracast device test plan (Windows 11 PC, Android phone, Windows sender)

For a person with **one Windows 11 PC that is to be the receiver** (a mini PC or laptop with a Wi-Fi adapter), **one Android phone** and, if possible, **a second Windows PC as a sender**. It tests milestone M5 (Miracast receiving on Windows), which was built to specification **without any Windows machine**: nothing in this plan has ever been run. Every expected result below is therefore an *expectation*, not a known fact; where the code or the Microsoft documentation leaves something open it says `UNCERTAIN`. The aim is to find out what is true and to write it down so the docs can change.

Measurement points served: **A4** (Miracast receiving through `MiracastReceiver` without MSIX: "an unpackaged app runs stably", "who shows the PIN") and **measurement point 5** (the "Wireless Display" feature, MS-MICE, firewall, Wi-Fi chips) in [measurement-points.md](measurement-points.md). What was built and why: [miracast.md](miracast.md).

Time: about 3 hours for everything with one receiver and two senders; parts A to C and F alone about 45 minutes.

## 0. What you need

| Item | Notes |
| --- | --- |
| Receiver PC with Windows 11 (22H2 or newer preferred; also try Windows 10 2004 or newer if you have one), a Wi-Fi adapter, an HDMI or normal display | Record: Windows build (`winver`), edition, the Wi-Fi adapter name and driver version and date (`Get-NetAdapter` and `Get-NetAdapter -Name Wi-Fi \| Format-List DriverVersion,DriverDate` in PowerShell), the graphics driver. Two different PCs (two Wi-Fi chips) answer point 5; one is enough to start |
| Android phone with Cast / Smart View (a Samsung "Smart View" and a Pixel "Cast" if you have both) | Record model, Android version. Newer Pixel phones removed Miracast in some versions: if the phone's cast list never shows Miracast devices, note that and try another phone |
| A second Windows PC (sender) | Windows key + K ("Cast") or Win + P, "Connect to a wireless display" |
| Both senders and the receiver near each other, ideally in the same room | Miracast uses Wi-Fi Direct: the PC's Wi-Fi must be on. Whether it works while the receiver is also connected to a normal Wi-Fi is part of the test (G2) |
| The repository and the Rust and Flutter toolchains on the receiver, or a release zip from the Windows job | `flutter build windows --release` in `apps/flutter` (the CMake step builds `cc_ffi.dll`, `cc_miracast_ffi.dll` and both workers with cargo and installs them next to `cometcast.exe`). The CLI needs `cargo build -p cometcast-cli` |
| Administrator rights on the receiver | For the optional feature and firewall checks |
| A stopwatch web page and a phone camera at 60 fps or more | For latency (E6) |
| Something copy-protected to play on the sender (a streaming video app that shows HDCP content), optional | E9 |

Commands that give useful facts before you start (write the outputs down):

```text
netsh wlan show drivers          # look for "Wireless Display Supported" (UNCERTAIN: exact wording)
dxdiag /t dxdiag.txt             # look for "Miracast:" in the Display section (UNCERTAIN: exact wording)
DISM /Online /Get-CapabilityInfo /CapabilityName:App.WirelessDisplay.Connect~~~~0.0.1.0   # as administrator; UNCERTAIN: name
DISM /Online /Get-Capabilities | findstr /i wireless                                       # as administrator
```

## 1. What to record (for every test)

One table row per run in a file `results/<date>-<receiver>.md` (not committed; attach it to the issue):

`test id | receiver (PC, Windows build, Wi-Fi adapter + driver) | sender (model, OS) | build (commit, debug/release, exe or CLI) | result (pass / fail / partly) | numbers | screenshot or photo | what surprised you`

Always keep: the exact text of every error, the HRESULT if Windows gives one, and a screenshot of the Comet Cast window **and** of any Windows prompt.

Where the logs are: run `cometcast.exe` from a terminal (or `flutter run -d windows` in `apps/flutter`) to see Dart output; the CLI (`cometcast-cli miracast --name TestPC`) prints every receiver event. The state that the screen polls (status, hint, counters) is visible on the screen itself; the picture counters are in the statistics line.

Numbers that matter and how to get them:

| Number | How |
| --- | --- |
| Time to first picture | Stopwatch from pressing "Allow" to the first picture |
| Glass-to-glass latency | A running millisecond clock on the sender, film sender and receiver screens together at 60 fps, subtract the two readings on a still frame. Five times, record the median |
| Picture rate | The statistics line under the picture ("Picture: W x H, N skipped") and, for exact counters, the state JSON (`frames.published`, `taken`, `dropped`, `failed`). Compare `published` over 30 seconds |
| CPU, memory, handles | Task Manager (Details): `cometcast.exe` CPU, memory (private working set) and handle count at the start and after 10 minutes of streaming; GPU engine usage in the Performance tab |
| Which side shows which dialog | A screenshot of each dialog with the title bar visible (ours, Windows' own, the sender's) |

## A. The machine (before Comet Cast)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| A1 | Run the commands of section 0 and record the outputs | `netsh` says wireless display is supported if the adapter and graphics driver support Miracast; `dxdiag` says "Miracast: Available" (with or without HDCP) | The wording of both lines was written from memory. Record them verbatim |
| A2 | On the receiver open Settings, System, **Projecting to this PC** (Windows 10: the same name; Windows 11 24H2 moved some settings) and note what it offers | Either a page with "Available everywhere / on secure networks", "Ask to project to this PC", "Require PIN for pairing", or a message that the optional feature must be installed | This is the **system's own** receiver UI. Record whether it exists on this PC and whether it works with the Windows sender (Win + K) **without Comet Cast**: that is the baseline for everything else. If the baseline fails, the adapter or driver is the problem, not our code |
| A3 | Baseline with the phone: start the system receiver of A2, cast from the phone | The phone sees the PC and the screen shows | Record which senders work with the system receiver. A sender that does not work there will not work with ours |
| A4 | Close the system receiver. Check that no other app uses Miracast (Task Manager: `WirelessDisplay.exe`, "Connect") | Nothing running | Our start disconnects all Miracast sessions machine-wide (see [miracast.md](miracast.md), "Side effects") |

## B. Build, bundle, start (A4: unpackaged app runs)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| B1 | `flutter build windows --release` | Succeeds. CMake runs cargo (`cc_rust_build` target); no error about `cargo not found` | Never run on Windows. Record every error text. If the cargo step fails on `COMETCAST_CARGO_TARGET_DIR` or the generator expressions, that is a CMake bug to fix in `apps/flutter/windows/CMakeLists.txt` |
| B2 | List `build\windows\x64\runner\Release` | `cometcast.exe`, `cc_ffi.dll`, `cc_miracast_ffi.dll`, `cc-docs-worker.exe`, `cc-office-worker.exe` next to each other | A missing file is a bug in the install step |
| B3 | Start `cometcast.exe` from a terminal | The home screen shows. The list has "Send screen", "Send file or link", "Receive on this device" **and "Receive via Miracast (Windows)"** | The entry must not appear on any other platform (a Linux or Android build must not show it: already tested with fakes) |
| B4 | Open "Receive via Miracast (Windows)" | The screen shows the intro, a name field with the host name, the pairing menu, the status "Not started.", the Start button, and three notes. **No hint card** if the machine is fine | A red hint at this point comes from the system probe: copy its text (it tells which check said so) |
| B5 | Temporarily rename `cc_miracast_ffi.dll` and open the screen again | The status card says receiving is not possible and shows the "component not found" hint; the rest of the app works | Restore the file afterwards |
| B6 | Windows Defender SmartScreen or antivirus | Record whether the unsigned exe and DLL were blocked or warned about | |

## C. Start and visibility (A4: a session can be created)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| C1 | Press Start (default pairing, name `TestPC`) | Status "Starting …", within a few seconds "Waiting for a sender. On the sending device, choose "TestPC" …". No Windows dialog | **A4 itself.** If it fails, record the hint and the technical "Last message" line. `Access is denied` points to packaging; `Class not registered` or similar to the missing feature (D2) |
| C2 | Windows sender: Win + K | `TestPC` is in the list within about 10 to 30 seconds | Discovery time; whether the list shows our name or the PC's host name |
| C3 | Android phone: Cast / Smart View | `TestPC` is in the list | Some phones only show Miracast devices after a while or after Wi-Fi is on |
| C4 | Change the name on the screen before starting (a name with an umlaut, a 32-character name, a 33-character attempt) | The field accepts 32 characters; the name shows on the senders | Windows may refuse names it dislikes: the error text is the finding |
| C5 | While listening, press "Stop receiving" | Status "Not started."; after a few seconds `TestPC` disappears from the senders' lists; start again works at once | Leftover session: if the second start fails, note the text |
| C6 | Close the window while listening | The process exits (check Task Manager); the receiver disappears from the senders | Dart `dispose` calls stop; a hung process means the stop thread is stuck |
| C7 | Run the CLI instead: `cometcast-cli miracast --name TestPC` | Prints `Listening`; a sender triggers `connection requested` (answer `y` or `n`) | The CLI has no picture; it does play the audio after `y` |

## D. The optional feature and unsupported adapters (hints)

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| D1 | Check the feature as administrator: `DISM /Online /Get-CapabilityInfo /CapabilityName:App.WirelessDisplay.Connect~~~~0.0.1.0` and look in Settings, System, **Optional features**, "View features", search "Wireless" | Record whether the **capability name is right**, its state (Installed / NotPresent), and the **exact name and menu path in English and, if you can, German Windows** | The name and path in the hint text are from memory. If wrong, fix `WIRELESS_DISPLAY_CAPABILITY` in `crates/cc-miracast/src/diagnosis.rs`, the `miracastHintFeatureMissing` strings in both `.arb` files, and [miracast.md](miracast.md) |
| D2 | With the feature **not installed** (uninstall it, or use a PC that never had it): open the screen and press Start | The status card shows the "Windows could not provide the Miracast receiver …" hint with the Settings path and the DISM line | **The most important unknown of this part:** which error does Windows give when the feature is missing (activation error, `MiracastNotSupported`, access denied)? Record the "Last message" text exactly. The hint logic in `diagnose_start_error` assumes "activation fails" |
| D3 | Install the feature as the hint says (Settings path, or the DISM command), restart the app, Start | Starts normally (C1) | Whether a restart of the PC is needed |
| D4 | Check whether the feature is needed at all: is a receiver possible with the feature **not** installed? (D2 tells) | Record yes or no | The plan lists it as a question |
| D5 | Adapter without Miracast support: a PC whose `netsh wlan show drivers` says wireless display is **not** supported, or a USB Wi-Fi stick | Start shows the "Wi-Fi adapter or its driver cannot receive Miracast" hint | Which status path fired: `Unsupported` from `Start`, from `DisconnectAllAndApplySettings`, or only the probe? Record the "Last message" |
| D6 | Adapter that reports support but "not optimised" | A warning line appears while listening; receiving may work | Rare; record if you see it |
| D7 | Switch Wi-Fi off in Windows (airplane mode) while listening, then on again | Status becomes "Receiving is not possible right now" with the "temporarily" hint, and returns to waiting when Wi-Fi is back, or needs a restart | Which `ListeningStatus` values appear; whether we recover by ourselves. Record the sequence |
| D8 | Group policy or MDM that disables receiving, if you can (unlikely) | The "policy" hint | Optional |

## E. Video and audio (the new part, never run)

Sender for E: the Windows laptop first (the best-supported path), then the phone. Receiver window at 1080p if possible.

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| E1 | Sender connects, you press **Allow** (see F1). Watch the receiver | Within a few seconds the sender's screen is in the Comet Cast window. Audio plays from the PC's speakers. The status says "Receiving the screen of …" | **This is the first time the `MediaPlayer` frame-server path runs.** If no picture: look at "Last message" ("playback failed: …" or "could not play the stream: …"), the counters (`published` zero means `VideoFrameAvailable` never fired or `CopyFrameToVideoSurface` failed: `failed` rising), and whether the sender stays connected at all. Record all three |
| E2 | Same with the CLI | Audio plays; no picture (by design) | If audio plays in the CLI but E1 has no picture, the problem is the frame path, not the stream |
| E3 | Picture size and sharpness: put 12 pt text on the sender screen | The text is readable; the picture keeps the aspect ratio with black bars | Nearest-neighbour scaling quality: record whether text looks rough when the sender is above 1080p |
| E4 | Picture rate: scroll or play a video on the sender; record the counters over 30 seconds | A smooth picture. Record `published` per second and `dropped` | If `published` is far below the sender's rate and CPU is high, the copy chain (GPU read-back and scaling) is the limit: this is the number that decides about the GPU-texture upgrade in [miracast.md](miracast.md) |
| E5 | CPU, GPU and memory while streaming 1080p for 10 minutes | Record the numbers at the start and the end. Memory should stay flat | A leak in the per-picture buffers or in the D3D texture recreation would show here |
| E6 | Glass-to-glass latency with the stopwatch page, five times, for the Windows sender and for the phone | Record the median. No target is promised | Miracast itself usually adds a few hundred milliseconds; ours adds an unknown amount |
| E7 | **Rotation**: rotate the phone portrait and back while casting | The picture follows (new size), a short glitch is acceptable, no permanent black or stretched picture, no crash | The texture is recreated when the natural size changes; never run |
| E8 | Change the sender's resolution or scaling (Windows display settings), or move the sender window between displays | The picture re-fits | |
| E9 | **Protected content**: play something copy-protected on the sender | Expect black or a refusal; no crash. Record exactly what happens, and what the sender shows (many senders refuse to send protected video over Miracast without HDCP 2.x) | HDCP behaviour is unchecked. The screen already warns that such content may be black |
| E10 | Audio: mute and unmute with the button; change the Windows volume; play a sound with a known beat on the sender | The button works; record lip sync and audio delay | UNCERTAIN: audio latency |
| E11 | Cast for one hour | No freeze, no memory growth, no handle growth, the sender stays connected | The point 5 stability check (A4) |
| E12 | Press **Disconnect** while casting | The sender is told, the picture disappears at once, the status returns to waiting, the sender can connect again | |
| E13 | Window handling: minimise, maximise, full screen via the window, move the window to another display | No crash; the picture keeps updating after restoring | |

## F. Consent, pairing and safety

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| F1 | Sender connects (default pairing) | A dialog appears **in the Comet Cast window** with the sender's name, the text that nothing is shown before you allow, the 30-second note, and the buttons **Decline** (focused) and **Allow**. **No picture appears while the dialog is open** | **Privacy criterion: nothing may be visible or audible before "Allow".** Listen for audio as well |
| F2 | Pairing "Ask me, and show a PIN if the sender asks for one", then "Ask me, and always require a PIN". Connect from the Windows sender and from the phone | Record per sender and mode: does the **sender** ask for a PIN? Who shows the PIN (**our dialog** line "PIN: …", **Windows' own** dialog, or nobody)? Is the PIN field of the event empty? What does the sender show if the PIN is wrong? | **The point 5 question "who shows the PIN".** If Windows shows its own PIN dialog and ours shows nothing, the hint "Enter it on the sending device" is misleading: note the finding |
| F3 | With default pairing, wait 25 seconds before answering on a Windows sender, then answer | The sender waits for the whole time and connects after Allow | **The deferral assumption.** If the sender gives up early, or Windows shows its own prompt as well, or the connection is accepted without our dialog, record it exactly. The last case is a **safety bug**: stop and report |
| F4 | Do not answer for 35 seconds | The dialog closes by itself after about 30 seconds; the sender is told "not accepted"; status back to waiting | Timeout value 30 s is a guess |
| F5 | Press **Decline** | The sender is told the connection was not accepted; no picture; status waiting | What the sender shows to the user (record the text) |
| F6 | While one sender is connected, a **second** sender tries to connect | The second is turned away without a dialog; the first stays connected; a small note "Another device tried to connect and was turned away" appears | `AllowConnectionTakeover = false` honoured? Windows' own takeover prompt on the second sender? |
| F7 | While a dialog is open for sender A, sender B tries | B is turned away; A's dialog stays | |
| F8 | **Hostile name**: rename the phone (Settings, About phone, Device name) to `Bob<newline>Accept?` (if the phone allows a newline) or to a very long name (100 characters) or with right-to-left override characters, then connect | The dialog shows one clean line (no newline, no reversed text, cut at 64 characters) | The sanitiser is unit-tested; whether Windows already cleans the name first is open |
| F9 | **No input back-channel**: with the sender connected, type on the sender and move the mouse; also try the Windows sender's "Allow input from a keyboard or mouse connected to this display" prompt if it appears | Nothing on the receiver PC reacts to the sender's input; no cursor from the sender controls the receiver | UIBC is not enabled by the code. If the **sender** offers input redirection, record what it says |
| F10 | Connect, allow, then disconnect from the **sender** side | `Disconnected` is seen: the picture disappears, status waiting | |
| F11 | Run Comet Cast twice (two windows or the exe plus the CLI) | The second start reports a problem or busy; the first keeps working; no crash | One receiver per Windows user is probably all Windows allows. Record the text |

## G. Network, firewall, MS-MICE

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| G1 | First start on a clean Windows (or after deleting the firewall rule for `cometcast.exe` in Windows Defender Firewall, Advanced) | Record whether a **Windows Security Alert** ("Allow Comet Cast to communicate on …") appears, **when** (at Start, or at the first connection), and which networks are pre-ticked (private / public) | The firewall note on the screen assumes an alert for private networks |
| G2 | Answer the alert with "Allow" for private only, then with nothing allowed | Record which combination lets a sender connect. Run `netstat -ano \| findstr 7236` while listening and note which ports `cometcast.exe` listens on (TCP 7236 is typical for Miracast) and on which addresses | Whether Wi-Fi Direct needs a firewall rule at all (the connection is on a separate virtual adapter, often "Microsoft Wi-Fi Direct Virtual Adapter") |
| G3 | Is the receiver also on a normal Wi-Fi or Ethernet while casting? Test with Ethernet only, Wi-Fi only, both | Record which combinations work | The Wi-Fi adapter may need to be on for the Wi-Fi Direct role |
| G4 | MS-MICE: both PCs on the same LAN, **Wi-Fi Direct impossible** (Wi-Fi off on the sender, wired both) | Record whether the Windows sender finds the receiver in the Win + K list, and which Windows setting enables it | Whether `MiracastReceiver` offers infrastructure mode at all and whether our app must enable something |
| G5 | Public network profile instead of private | Record whether discovery or connecting is blocked | |

## H. Senders matrix

Run C2, E1 and F1 once per sender and fill one table:

| Sender | Found in the list | Connects after Allow | Picture | Audio | Rotation | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Windows 11 laptop (Win + K) | | | | | n/a | |
| Windows 10 laptop | | | | | n/a | |
| Android phone with Smart View (Samsung) | | | | | | |
| Android phone with Cast (Pixel or other) | | | | | | |
| Miracast TV stick or dongle (if you have one) | | | | | n/a | |

## I. Lifecycle and side effects

| Id | Steps | Expected | UNCERTAIN / look for |
| --- | --- | --- | --- |
| I1 | Before the first run note the receiver name and authorization in Settings, System, Projecting to this PC (if it exists). Run Comet Cast, connect, stop. Look again | The settings are the same as before | Our start changes `FriendlyName`, authorization and "known transmitters" machine-wide and tries to restore them on stop. Whether they persist and whether the restore works is open ([miracast.md](miracast.md), "Side effects") |
| I2 | Start another Miracast receiver app (the system receiver of A2) and cast to it; then start Comet Cast | Comet Cast's start **disconnects the other session** | Documented side effect; record that it happens and how the other sender reacts |
| I3 | Kill `cometcast.exe` in Task Manager while a sender is connected, then start again | The sender loses the connection; the new start works; no leftover session | Whether Windows cleans up a dead session by itself |
| I4 | Sleep and resume the receiver while listening | After resume the receiver is listening again, or shows a problem and a restart fixes it | |
| I5 | Pull the Wi-Fi (disable the adapter) while casting | The sender is disconnected; the status changes; no crash | Which events appear |
| I6 | Log off and on, or run as a different user | Not expected to work for a second user at the same time; record what happens | |
| I7 | Handles and threads: after 20 connect and disconnect cycles check Task Manager | No growth | |

## J. Pass criteria

| Question | Pass | Fail means |
| --- | --- | --- |
| A4: an **unpackaged** app can create a session and stays stable (C1, E11, I3) | Receiver starts, is found, one hour without a crash or freeze | Try MSIX packaging; reserve `WiFiDisplay.dll` (needs its own review) |
| Consent is always asked and nothing is shown first (F1, F3, F4, F5) | No picture and no audio before Allow, in every test; a dialog for every foreign sender | **Stop and report as a safety bug** |
| Who shows the PIN (F2) | Answered and documented for two senders | Fix the dialog text (`miracastConsentPin`) |
| Video and audio (E1, E4, E6) | A picture and audio. Numbers recorded. Usable means: no freezes, and median latency recorded (the plan wants "good enough to present"; no figure promised) | If the frame rate is too low: GPU texture upgrade (see [miracast.md](miracast.md)) |
| Feature and adapter hints (D1, D2, D5) | The right hint appears; the capability name and the menu path are correct in English | Fix the texts and `diagnosis.rs` |
| Firewall and MS-MICE (G1, G2, G4) | Prompt, ports and the working combination documented | Fix the firewall note |
| One session, no input back-channel (F6, F9) | Second sender turned away; no sender input arrives | Safety bug |
| Side effects (I1, I2) | Documented | Consider not using `DisconnectAllAndApplySettings` |

After the run: edit [miracast.md](miracast.md) (replace each **Uncertain** that you resolved with what you saw, with the Windows build), the M5 row in [architecture.md](architecture.md), and the rows A4 and point 5 in [measurement-points.md](measurement-points.md). Report every finding as an issue with the result file attached.
