# Miracast receiving (Windows 11)

Milestone M5. Crate: `crates/cc-miracast`. CLI: `cometcast-cli miracast [--name NAME]`.

**State in one sentence: compile-checked only, never run.** The Windows backend was written against the `windows` crate 0.62.2 sources and compiles for `x86_64-pc-windows-gnu`. It was never started on a Windows machine, nobody has seen a Miracast sender connect to it, and every statement below about runtime behaviour is **Uncertain** until the checklist in this page is done. This page belongs to measurement assumption A4 and measurement point 5 in [measurement-points.md](measurement-points.md).

## How it is meant to work

A Windows 11 mini PC runs Comet Cast. A Windows laptop presses Win+K (or Win+P, "Connect to a wireless display"), sees the mini PC in its list, connects, and the mini PC receives the screen. No sender app is needed on the laptop.

On the mini PC the app uses the documented Windows Runtime API `Windows.Media.Miracast.MiracastReceiver`:

1. Activate `MiracastReceiver`. Read `GetCurrentSettings` (fallback `GetDefaultSettings`), set `FriendlyName` (the name shown to senders), set `AuthorizationMethod` (`ConfirmConnection`, or `PinDisplayRequired` when a PIN is wanted) and `RequireAuthorizationFromKnownTransmitters`. Apply with `DisconnectAllAndApplySettings`.
2. Subscribe to `StatusChanged`.
3. `CreateSession(None)` (no `CoreApplicationView`, because the app is not packaged; the plan names this as the thing to measure). Set `AllowConnectionTakeover` to off and `MaxSimultaneousConnections` to 1. Subscribe to `ConnectionCreated`, `MediaSourceCreated`, `Disconnected`. Call `Start`.
4. On `ConnectionCreated` the backend reads the sender name (`Connection.Transmitter.Name`) and the PIN (`ConnectionCreatedEventArgs.Pin`), asks `ConnectionPolicy`, and, if allowed, holds the connection with a `Deferral` and emits `ConnectionRequested`. The person at the mini PC answers; `respond(id, true|false)` completes the deferral or disconnects with `ConnectionNotAccepted`. No answer within 30 seconds counts as "no".
5. On `MediaSourceCreated` the backend emits `StreamReady` with an opaque `StreamHandle` (the Windows `MediaSource`).
6. Dropping the receiver removes all handlers, disconnects everything and closes the session.

### Rules of our own (`ConnectionPolicy`, tested on Linux)

* Every foreign device needs a confirmation by a person. There is no "accept all" setting and no input that makes the policy accept without asking.
* At most one session at a time (active or waiting). A second request is rejected.
* A sender without a name is rejected (nothing to show to the person).
* Constants (name length 32, timeout 30 s, one session) are guesses or product decisions. **Uncertain:** the real limits of Windows were not measured.

## What is NOT done

* **No video on screen.** The crate hands out the `MediaSource`; nothing plays it. The Flutter app would need a Windows video surface (a native texture or platform view that plays a `MediaSource`). That does not exist. The CLI therefore only prints events.
* **No `Deferral` is taken in `MediaSourceCreated`.** If Windows needs the app to attach the source to a player before the handler returns, the stream may not start. **Uncertain.**
* No input back-channel (keyboard, game controller), no cursor channel, no stream settings. The API has them (`InputDevices`, `CursorImageChannel`, `StreamControl`); they are left out on purpose.
* No Flutter or FFI binding yet. No installer. No MSIX.
* **`WiFiDisplay.dll` / `WFDDisplaySink` is not implemented.** It is named in the plan only as a reserve if `MiracastReceiver` fails unpackaged; it is undocumented and its behaviour on Windows 11 is unknown. No code touches it.

## Requirements

All from the plan or the API documentation; none verified on hardware.

* Windows 11 (the API exists on Windows 10 2004 and later according to the Microsoft documentation; **Uncertain**, not checked here).
* A Wi-Fi adapter whose driver supports Wi-Fi Direct and Miracast receiving. `MiracastReceiverWiFiStatus` and `MiracastReceiverApplySettingsStatus.MiracastNotSupported` exist to report a missing support; the backend maps them to `MiracastStatus::Unsupported`. Which mini-PC Wi-Fi chips qualify is unknown (measurement point 5 asks for two models).
* The optional Windows feature "Wireless Display" installed. **Uncertain:** the plan lists this as a question; it may be required for the receiver role.
* Network: Miracast uses Wi-Fi Direct and an RTSP connection (port 7236 is typical). The Windows firewall may block it for an unpackaged program. **Uncertain.**
* MS-MICE (Miracast over infrastructure) would let a sender connect over the normal network instead of Wi-Fi Direct. Whether `MiracastReceiver` offers it, and what must be enabled, is **Uncertain** and part of point 5.

## Test checklist for a real Windows machine

Build: `cargo build -p cometcast-cli --target x86_64-pc-windows-msvc` on Windows (the gnu target was only used to compile-check). Run `cometcast-cli miracast --name TestPC` as a normal, unpackaged `.exe` without MSIX. Write down for each item: result, Windows build number, Wi-Fi adapter and driver version, exact error text.

1. [ ] `MiracastReceiver::new()` works in an unpackaged console program. (A4: "an unpackaged app runs stably".) If activation fails, note the HRESULT.
2. [ ] `DisconnectAllAndApplySettings` returns `Success`. Note the status if not (`AccessDenied` would point to packaging or capability problems).
3. [ ] `CreateSession(None)` works unpackaged. If it fails: is a `CoreApplicationView` required? Does a packaged (MSIX) build fix it?
4. [ ] `Session.Start` returns `Success`; the CLI prints `Listening`.
5. [ ] The mini PC appears in the list of a second Windows machine (Win+K) under the chosen name. Also try an Android phone and a Miracast TV stick if available.
6. [ ] A connection attempt produces `connection requested` with the right device name.
7. [ ] **Who shows the PIN?** With `PairingPolicy::RequirePin` (CLI currently uses the default; temporarily change `MiracastConfig` in `main.rs`): does Windows draw a PIN dialog itself, or does only our app know the PIN? Is `Pin` empty in `ConnectionCreated`? What does the sender see? (Plan: point 5 "who shows the PIN".)
8. [ ] With the default `ConfirmConnection`: does Windows show its own prompt in addition to ours? Does the sender wait for our answer? Does the deferral keep the connection waiting for 30 seconds?
9. [ ] Answer `n`: the sender is told the connection was not accepted; `Disconnected` is printed.
10. [ ] Answer `y`: `connected` and then `StreamReady` are printed. Does the sender stay connected although nothing plays the `MediaSource`? (Expected: unknown. If Windows drops it, a player must be attached inside the handler.)
11. [ ] A second sender while one is connected is rejected, and the first one stays up.
12. [ ] `AllowConnectionTakeover` off: confirm that a second sender cannot take over.
13. [ ] The sender stops mirroring: `Disconnected` arrives. Unplug the Wi-Fi: what events appear?
14. [ ] Ctrl-C: the program exits, the sender sees the receiver disappear, the receiver name is gone from the list after a few seconds. Run it again right away: does it start cleanly (no leftover session)?
15. [ ] Windows Firewall: was a prompt shown at first start? Does it work after "allow"? Does it work without any rule? Which ports and which profile (private/public)?
16. [ ] MS-MICE: with both machines on the same LAN and no Wi-Fi Direct, does the sender find the receiver? Which setting enables it?
17. [ ] Run for an hour with a stream; watch memory and handle counts (point 5 asks for stability).
18. [ ] Repeat on a second mini-PC model with a different Wi-Fi chip. List which chips work.
19. [ ] Repeat the unpackaged result with the app run as a different user or as a service (not expected to work; note it).

If items 1 to 4 fail only unpackaged, the fallback is MSIX packaging (a package identity and the capabilities Windows requires; **Uncertain** which). If that also fails, the plan names the reserve `WiFiDisplay.dll`; it is not implemented here and its use would need its own legal and technical review.

## What was verified here

| Check | Result |
| --- | --- |
| `cargo test -p cc-miracast` on Linux (policy, status, config, "Unsupported" on non-Windows) | passed (10 tests) |
| `cargo check` for `x86_64-pc-windows-gnu` (`cc-miracast`, `cometcast-cli`) | passed. The `ring` C code could not be compiled in this Linux sandbox (no MinGW); a stub compiler was used only so the check could pass the build script. Nothing was linked |
| `cargo clippy -p cc-miracast -p cometcast-cli --target x86_64-pc-windows-gnu -- -D warnings` | passed |
| Run on Windows | **never done** |

## Code notes

* `unsafe`: the workspace forbids `unsafe`. `cc-miracast` lowers this to `deny` (like `cc-ffi`) because `RoInitialize` is an `unsafe fn` in the `windows` crate. There is exactly one `#[allow(unsafe_code)]` with a SAFETY comment. Everything else in the backend is safe code.
* The backend calls `RoInitialize` for the multithreaded apartment on the calling thread and never calls `RoUninitialize`. **Uncertain:** the effect when the host app already uses a single-threaded apartment (Flutter UI thread) is not tested; a host app should start the receiver from a thread of its own.
* Event handlers run on Windows threads. Events travel through a `std::sync::mpsc` channel.
* Features of the `windows` crate: `ApplicationModel_Core` (the `CreateSession` signature), `Foundation`, `Foundation_Collections`, `Media_Core`, `Media_Miracast`, `Media_Playback` (gates the `MediaSource` type), `Win32_System_WinRT`.

## Legal and licence notes (all **Uncertain**, not legal advice)

* The `windows` crate (Microsoft) is licensed MIT OR Apache-2.0 according to its `Cargo.toml` (not checked against the licence texts). See [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md). It is only a dependency on Windows targets.
* The `MiracastReceiver` API is documented by Microsoft; we call it as an ordinary application. Whether Microsoft's terms for the Windows SDK or Miracast (Wi-Fi Alliance) logo and name usage impose conditions on shipping a product that calls itself a Miracast receiver was not checked. Do not use the Miracast or Wi-Fi Alliance logos or certification claims.
* No code, key or certificate is taken from third-party receivers. `WiFiDisplay.dll` is not used.
* Miracast streams can carry HDCP-protected content. Whether and how the Windows API delivers or blocks protected content to a `MediaSource` was not checked.
