<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Pause: freeze the picture from either side

Status: **built** (see "As built" at the end). Replaces the display-only "Freeze" of the web display
(`toolbarState.ts`, `PresentingStage.vue`) with one feature that both sides see.

## Goal

While a screen is shown, the audience should be able to keep seeing the last picture while the
presenter prepares something (opens another file, types a password, rearranges windows). Either
side can pause:

* **The presenter** presses Pause on its device. The display keeps the last picture; the
  presenter's screen is no longer sent until Resume.
* **The display** (host) presses Pause (the existing "Freeze" button of the web display toolbar,
  and the same control on the native displays). The display keeps the last picture; the presenter
  is told, so it knows the audience does not see its live screen.

## Rules

1. There are two independent flags: `presenter` (set only by the presenter) and `display` (set
   only by the display). The picture on the display is frozen while **either** flag is set.
   Each side can only clear its own flag. Both sides always see both flags.
2. Pause applies to the **live screen picture** only. Images, PDF pages and links are shown
   until replaced anyway; the Pause control is not offered while no screen is shown (web: the
   existing `canFreeze`). If the screen stops (`stopScreen`, `PresentationStopped`, peer left,
   new session), both flags are cleared on both sides.
3. While the **presenter** flag is set, the presenter **stops sending video** (privacy: what it
   prepares must not reach the display device at all). On Resume it sends again and the
   receiver gets a key frame (web: `RTCRtpSender.replaceTrack(null)` / `replaceTrack(track)`,
   no renegotiation; native: `cc-sender` stops feeding the encoder and forces a key frame on
   resume). The receiver must not treat the missing frames as a failure (no stall error, no
   "connection lost", no key-frame request storm). Audio, if any, is paused with it.
4. While only the **display** flag is set, the presenter keeps sending; the display just does
   not update the picture (web: the existing freeze canvas).
5. On the display, a frozen picture carries the existing badge (`toolbar.frozenHint`), with a
   text that says who paused: "Paused by the presenter" / "Frozen on this display" (both: the
   presenter text). On the presenter, a banner: "Paused: the audience sees the last picture"
   with a Resume button, or, when only the display froze: "The display has frozen the picture".
6. Annotations (pen, marker, laser) keep working on a frozen picture, as today.
7. Black screen stays independent of pause, as today.

## Wire protocol (control data channel `control`, docs/web-m2.md, crates/cc-files wire)

* Presenter → display: `{"kind":"cmd","cmd":"pause"}` and `{"kind":"cmd","cmd":"resume"}`
  (new values of `ControlCmd`). Idempotent.
* Display → presenter: `{"kind":"pause_state","presenter":<bool>,"display":<bool>}`, sent
  whenever either flag changes and once when the control channel opens (both `false`). It is
  the source of truth for the presenter's UI: the presenter shows its own flag as confirmed only
  when the display echoes it (it may show a pending state meanwhile). Like `page`, it is a
  display-to-presenter message only; a display ignores a received `pause_state`, a presenter
  ignores received `pause`/`resume` commands.
* An older peer that does not know the messages ignores them (unknown `cmd` values and kinds
  are already ignored or counted as a bad message? Check both parsers: an unknown `cmd` must
  not end the transfer or count towards the violation limit; if it does today, make the new
  values known on every receiver in the same change).
* Add cases to the shared fixtures (`tests/fixtures/control-parse.json`,
  `tests/fixtures/content-scenarios.json`) so the Rust, web and Swift parsers stay equal.

## Where it is built

| Part | Work |
| --- | --- |
| `crates/cc-files` wire + receiver | parse `pause`/`resume` and `pause_state`; a receiver-side `PauseState` |
| `crates/cc-receiver` | emit `ReceiverEvent::PauseChanged { presenter, display }`; command `set_display_paused(bool)`; send `pause_state`; clear on presentation end; no stall handling while paused |
| `crates/cc-sender` | `pause()`/`resume()`: stop/restart sending video, key frame on resume; receive `pause_state` and emit an event |
| `crates/cc-ffi` | expose the above to Flutter; regenerate bindings (flutter_rust_bridge_codegen 2.13.0, Flutter 3.47.9 in `/home/user/tools/flutter/bin`) |
| `apps/web` | sender: Pause/Resume button on the presenting page while the screen is shared; display: the existing Freeze becomes the `display` flag and sends `pause_state`; badge texts per rule 5 |
| `apps/flutter` | presenter: Pause/Resume button and banner; display: a Pause control next to the existing display controls (and on Android TV reachable with the D-pad), badge |
| `apps/apple-tv` | display: Play/Pause button of the Siri Remote toggles the display flag; badge; react to `pause`/`resume` and send `pause_state` (cannot be compiled here: keep the change small and mirror the Rust logic, add Swift tests next to the existing ones) |
| iOS broadcast extension | presenter Pause is **not** offered (the extension has no UI; the app is in the background). The display can still pause. Say so in `docs/ios.md` |
| `cometcast-cli` | print the pause state it receives; no new command needed |
| Docs | `docs/web-m2.md` (messages), `docs/protocol.md` if it lists control messages, `docs/design/first-run-and-display.md` and `native-apps.md` (UI), copy tables for new strings (the web locale test requires every key whose du and Sie forms differ to be in the spec tables) |

## Strings (en / de du / de Sie)

* `pause.presenter.button` Pause / Pausieren / Pausieren
* `pause.presenter.resume` Resume / Fortsetzen / Fortsetzen
* `pause.presenter.banner` Paused: the audience sees the last picture. / Pausiert: Das Publikum sieht das letzte Bild. / (same)
* `pause.presenter.displayFrozen` The display has frozen the picture. / Die Anzeige hat das Bild angehalten. / (same)
* `pause.display.byPresenter` Paused by the presenter / Vom Vortragenden pausiert / (same)
* existing `toolbar.frozenHint` stays for the display's own freeze.

Keep the du and Sie texts identical where possible, so no new spec rows are needed; if one must
differ, add it to the copy tables.

## Acceptance

* Web e2e-level unit tests: presenter pause → display frozen + presenter banner; display freeze
  → presenter banner; both → resume by one side keeps the other frozen; stop screen clears.
* Rust tests in cc-receiver/cc-sender with the loopback harness: pause stops frames reaching the
  receiver, resume brings a key frame, no error events while paused for 30 s.
* Flutter widget tests for both buttons and banners. Fixtures updated and passing in Rust, web
  and (by reading) Swift.

## As built

What was built per row of the table above, the decisions taken where the spec said "check", and what is still unsure.

| Part | Built |
| --- | --- |
| `cc-files` wire + receiver | `ControlCmd::Pause` / `Resume` (in `cc-proto`), `Message::PauseState` (both fields required booleans), `PauseState` (pure: `set_presenter`, `set_display`, `clear`, `message`), `ContentEvent::PauseRequested`, `pause_state_message`. Pause and resume are never violations, are not queued while a document opens and do not touch a running transfer |
| `cc-receiver` | `ReceiverEvent::PauseChanged { presenter, display }`, `ReceiverCore::set_display_paused(bool)`, `pause_state` to the presenter on every change and once when the control channel arrives, both flags cleared (one event, before `PresentationStopped` / `PeerLeft`) when the presentation ends. Pause only applies while a video track is shown. No frame watchdog exists, so a pause of 30 s is no error (loopback test `crates/cc-receiver/tests/pause.rs`) |
| `cc-sender` | `SenderCore::pause` / `resume` (screen: the pipeline thread neither captures nor encodes, key frame forced on resume; external H.264: units refused while paused, `begin_streaming` on resume asks the host for a key frame and drops delta frames until it comes; test pattern: silent), `PresenterSession::pause` / `resume`, `PresenterEvent::PauseState`, a flag the display does not hold is asked for again. Order: pause stops the video first and then tells the display; resume tells the display first, then sends again |
| `cc-ffi` | `set_display_paused`, `ReceiverUiEvent::PauseChanged`, `set_presenter_paused`, `PresenterUiEvent::PauseState`; bindings regenerated. A TV session answers `Unsupported` to a pause command (the apps do not offer it there) |
| `apps/web` | `ReceiverSession.setDisplayPaused` and `state.pause`; `SenderSession.pausePresenter` / `resumePresenter` and `state.pause` (`requested`, `presenter`, `display`); `Peer.setStreamPaused` (`replaceTrack`); `PresentingStage` badge and freeze canvas; `PresentingPage` button and banner |
| `apps/flutter` | `ReceiverController` (`pausedByPresenter`, `pausedByDisplay`, `frozen`, `canPause`), `PresenterSessionController` (`pauseRequested`, `pauseConfirmed`, `displayFrozen`, `pauseBanner`), Pause control left of End, badge, button and banners |
| `apps/apple-tv` | `PauseState`, parser and `pause_state` message in CometCastCore, `AppModel` sends and applies the flags, Pause control, badge, `intentForPlayPause` for the remote |
| iOS broadcast extension | presenter pause not offered; stated in `docs/ios.md` |
| `cometcast-cli` | `receive` prints `Pause: presenter ..., display ...` for every change |
| Docs | `docs/web-m2.md` (messages and the "Pause" section), `docs/protocol.md`, `docs/transport.md`, `docs/appletv.md`, `docs/ios.md`, `docs/design/first-run-and-display.md` (4.5 and copy table 11.11), `docs/design/native-apps.md`, `docs/measurement-points.md` (A22 to A24) |

### Decisions

* **Older peers (the spec's "check").** Checked: before this change `cc-files` answered an unknown `cmd` value with `InvalidFields`, which is one protocol violation **and** drops a running transfer (every refused text message does); the web and Swift parsers answered with a bad-message toast. So the new values are known on every receiver in the same commit series (`cc-files`, web, Swift) and the shared fixtures pin it. An older display cannot be fixed; see the "Pause" section of `docs/web-m2.md` for what happens with one.
* **Pending state.** The presenter shows its own flag as confirmed (banner) only after the display's echo. Meanwhile the button already reads Resume and the video is already stopped. With a display that never echoes the presenter stays pending and shows no banner (UNCERTAIN: acceptable for an old display, no timeout is built).
* **Banner precedence.** The presenter text wins when both flags are set (as on the display). The presenter's own request alone, not yet echoed, shows no banner.
* **Where the flags live.** On the display side in the receiver core (native) or the receiver session (web, Apple TV core): the single owner that sees the video track, the control channel and the end of the presentation. The presenter side keeps only its request and the last `pause_state`.
* **Order on the wire.** Pause: video off, then `pause`. Resume: `resume`, then video on. The display therefore never unfreezes before the presenter's frames can arrive, and never receives frames of what the presenter prepares.
* **Frozen picture on the displays.** Web: a canvas painted from the video element (also for the presenter's flag, because a `<video>` without frames can go black). Flutter software path: new pictures are held (the newest one is shown when live again). Flutter hardware path: units are dropped while frozen and a key frame is requested when live again. Apple TV: the video view is taken off the track.
* **Display's own control words.** The display's Pause control uses `pause.presenter.button` / `pause.presenter.resume` ("Pause" / "Resume"); no extra strings. Apple TV has no pause icon in the asset catalog, so its control is text only.
* **Strings.** The five keys of the spec, du and Sie identical, so no "differs" rows; the copy table 11.11 was added to `first-run-and-display.md` anyway so that the locale test checks the words.
* **Stopping the screen.** A web presenter sends `resume` before it removes the track, so a native display clears the presenter flag. Both flags are cleared by the display with the stream (web) or the end of the presentation (native).

### UNCERTAIN

* The key frame after `replaceTrack(track)` in browsers (A22 in [../measurement-points.md](../measurement-points.md)).
* That the Apple TV Metal view, the Android hardware texture and a `<video>` keep their last frame when nothing updates them (A23).
* A native display does not learn that a web presenter stopped its screen while the display flag was set (A24).
* Apple TV: written against the headers, never compiled; the Swift tests (`PauseTests`, `testPlayPauseTogglesThePauseFlagWhileALiveScreenIsShown`) were not run.
