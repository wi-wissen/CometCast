<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Pause: freeze the picture from either side

Status: specification, to be built. Replaces the display-only "Freeze" of the web display
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
