# First run and display: implementation spec

Status: **final, approved for implementation; revised after the first and the second implementation review of the web app (2026-11-07).** The owner approved round 4 with two changes, which are applied here and in the final mockup. The review decisions are listed in [section 13](#13-decisions-after-implementation-review) (1–29 first review, 30–53 second review) and are already worked into the sections below; the work items for the web app are in [fix-list-web.md](fix-list-web.md) (done) and [fix-list-web-2.md](fix-list-web-2.md) (open), the answered questions in [implementation-questions.md](implementation-questions.md). How the native apps (Flutter, SwiftUI on Apple TV) build the same product is in [native-apps.md](native-apps.md). Build from this document; do not wait for further design review.

This document describes the **whole product**, including parts that the mockup never drew (annotation toolbar, link card, relay confirmation, security code, connecting and offline states, the later error pages, the PDF page row, the web file page, the wizard address field). Where an item has no mockup counterpart, the section named here is its design.

- Final clickable mockup: https://claude.ai/artifact/6gJouE3T4cWCdiWuG3CWc1 (German and English, uses "du")
- Earlier rounds, for history only: round 4 https://claude.ai/artifact/3pJjsoF2aTEjCkstWngUoW, round 3 https://claude.ai/artifact/1yevs8Z4UukdUF83R31Y7K, round 2 https://claude.ai/artifact/MAGbm5M2sgYXhy1PjLxhgt

Where this document and the mockup disagree, this document wins. Known places where the mockup is outdated: the 8-character code, the device type ("· Laptop") in the request and the name tag, the file page's thumbnail grid in a browser, and the link that "opens by itself" (see section 13). The mockup's grey frame (chapter tabs, the remote control, the scenario buttons, the yellow notes, the device switcher) is a presentation aid and is not part of the product.

## 0. Changes since round 4

1. The settings button on the idle screen shows the word **"Einstellungen" / "Settings"** next to a gear icon. Round 4 had an icon-only button whose icon looked like a light/dark switch.
2. **One accent colour only.** The "End" button during a presentation no longer uses the pink colour. It uses the dark blue surface of the background, the same as the name tag next to it. The mode pill, the selected tiles in Settings and the offline box also lost their green/blue/pink colours. The amber accent is now the only colour on the display; everything else uses the background's blues (or the light/OLED equivalents).
3. On the presenter's side, "Stop showing" is now the normal primary button (blue), not red, for the same reason.
4. Fixed: on a laptop presenter page, Chrome's sharing bar covered the "Stop showing" button. The page now keeps 76 px free at the bottom while that bar is shown.

## 1. Decisions already made by the owner (apply as stated)

| # | Decision |
| --- | --- |
| D1 | The default join mode is **Open**. The setup wizard preselects Open. |
| D2 | Join mode and appearance live in **Settings** on the display. Nothing else is in Settings. |
| D3 | **Dark** is the default appearance. |
| D4 | **OLED black** means true black (`#000000`) for the background and all surfaces, with **no pixel drift** or position shifting. |
| D5 | In the idle state the display **dims after about 5 to 10 minutes** without activity (this spec: 7 minutes, see 8.6). The system screensaver and sleep are blocked **only while someone is presenting**. The screen is kept awake during an active presentation and released when the display returns to idle or the session ends. |
| D6 | German UI wording is formal **"Sie"** until the owner says otherwise. The mockup uses "du". Every German string below is given in both forms. **The shipped default is "Sie"** (as in the product today). The owner has an open question about this, so the switch must be possible in a single translation pass (see 11.1). |

## 2. Glossary

| Term | Meaning |
| --- | --- |
| Display | The device that shows the picture: TV app (Google TV / Android TV, Apple TV), or the web page `{dom}/display` on a computer, board or tablet. |
| Presenter | The person whose phone or laptop sends a picture. |
| Host | Whoever operates the display (remote, mouse or touch). Has no account. |
| Session | Everything between a fresh code and "End". Ending the session disconnects everyone and makes a new code. |
| Code | Join code shown on the display. Every server (default, own server, in-house box): **6 characters** from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (no O, 0, I, 1), shown as two groups of 3 (`K7Q M2W`). This is the protocol's only format (`docs/protocol.md`). |
| Check number | Two-digit number shown on the presenter's waiting page and in the join request on the display, so the host can match a request to a person. |
| Security code | Six digits (`171 852`) computed on both sides from the connection's keys. Shown in the name tag on the display and on the presenter's "Presenting" page; if they differ, the connection may be intercepted. Not the check number, not the join code. |
| Room name | The name chosen in setup step 1. Shown on the idle screen and, through the protocol field `room_name`, on the presenter's waiting, "What to show" and "Presenting" pages. |
| Join mode | Open, Approval on (`Mit Freigabe`), Locked. |
| `{dom}` | The address of the server the display uses, e.g. `cometcast.de`, `cast.musterfirma.de`, `cast-box.local`. |

## 3. Design tokens

### 3.1 Reference canvas and scaling (display)

All display measurements are in **reference px on a 960 × 540 canvas**. Scale uniformly: `1 ref px = viewport height / 540` (Full HD: × 2, 4K: × 4). If the viewport is not 16:9, scale by `min(width / 960, height / 540)` and fill the rest with `--bg`. The display never scrolls.

### 3.2 Colour: display (TV app and `/display`)

| Token | Dark (default) | Light | OLED black | Used for |
| --- | --- | --- | --- | --- |
| `--bg` | `#0E1630` | `#FFFFFF` | `#000000` | Screen background |
| `--panel` | `#17213F` | `#F2F4F8` | `#000000` | Dialogs, settings button, name tag, End button, wizard card |
| `--panel2` | `#1F2B50` | `#E6EAF1` | `#000000` | Tiles, chips, mode pill |
| `--line` | `#2E3C68` | `#B4BCCB` | `#2C313C` | Borders of every surface and button |
| `--fg` | `#F3F5FA` | `#0A0F1E` | `#E2E6EE` | Main text |
| `--mut` | `#B9C2D8` | `#343C51` | `#A1A9BA` | Secondary text, labels |
| `--acc` | `#FFC266` | `#7A4300` | `#D29E48` | **The one accent:** step numbers, primary buttons, selected tile border, mode-pill icon, offline icon |
| `--focus` | `#FFC94D` | `#B35C00` | `#DDAE48` | D-pad focus ring |
| `--btn` | `#24305A` | `#E3E7EF` | `#000000` | Secondary button fill (wizard "Back", request "No") |
| `--qrbg` | `#FFFFFF` | `#FFFFFF` | `#E6E6E6` | Quiet zone behind the QR code |
| QR modules | `#0E1630` | `#0E1630` | `#0E1630` | QR code dots (always dark on light) |
| Dialog scrim | `rgba(0,0,0,.55)` | same | same | Behind join request and Settings |

Derived values: selected tile fill = `color-mix(in srgb, var(--acc) 14%, var(--panel2))` in Dark and Light; **in OLED black the selected tile fill stays `#000000`** (selection = `--acc` border + bold text only); name tag, End, annotation toolbar, frozen badge and transfer progress pill fill = `color-mix(in srgb, var(--panel) 92%, transparent)`; focus glow = `0 0 0 9px rgba(255,201,77,.18)`. Text on an accent fill uses `--bg`.

QR box (decision 13.33): with a code it has the `--qrbg` fill; in **Light** it also has a 1 px `--line` border (white on white has no edge otherwise). **Empty QR box** (connecting, offline): no modules, no caption, fill `--panel2`, 1 px `--line` border, **full opacity** (in OLED black: `#000000` fill + border). The Locked QR (modules present) keeps the 25 % opacity.

Rules:

- No other hue appears on the display. There are no green, blue or red status colours. Join mode is told apart by icon and word, not colour.
- OLED black: every surface is `#000000`; surfaces are separated only by `--line` borders. No gradients, no tints, no shadows that lighten black, no drift, no burn-in shifting. Consequence: any element that is told apart by a `--bg` fill in Dark/Light (the check-number box in the join request) gets a 1 px `--line` border in OLED black.
- The only other colours on the display are content: the picture itself and the ink swatches of the annotation pens (they show the ink colour they draw with).
- Automatic follows the system setting: dark system → Dark, light system → Light. It never picks OLED black.
- Logo: `assets/logo/cometcast-logo-dark.svg` on Dark and OLED black, `assets/logo/cometcast-logo-light.svg` on Light.

### 3.3 Colour: presenter pages (phone and laptop, browser and app)

Presenter pages follow the device's own setting (light or dark). OLED black does not exist there.

| Token | Light | Dark | Used for |
| --- | --- | --- | --- |
| `p-bg` | `#F4F5F8` | `#14171D` | Page |
| `p-surface` | `#FCFCFD` | `#1D2128` | Inputs, cards, secondary buttons, option rows |
| `p-border` | `#CDD1D9` | `#3B404B` (inputs), `#343944` (cards) | Borders |
| `p-divider` | `#D4D8E0` / `#E2E4E9` | `#343944` / `#2B2F38` | "or" rule, card edges, progress track |
| `p-fg` | `#1C2027` | `#E8EAEF` (headings `#F1F3F7`) | Text |
| `p-fg2` | `#3F4652` | `#BCC3CF` | Body text, labels |
| `p-fg3` | `#646C7B` | `#9EA6B3` | Small print, hints |
| `p-primary` | `#2D4A8A` (text `#FFFFFF`) | `#93AAE6` (text `#0F1420`) | **The one accent:** primary button, focus outline, progress, links, check number |
| `p-primary-soft` | `#E7ECF6` | `#222A3D` | Icon discs |
| `p-disabled` | `#DDE1E8` (text `#4A5160`) | `#2A2F38` (text `#9EA6B3`) | Disabled primary button. The text must stay readable (≥ 4.5:1): "Ask again in 15 s" carries information. |
| `p-error` | `#B5483C` on `#F6E1DE` | `#F08C80` on `#342A2C` | Error text, error icon disc, invalid input border |
| `p-ok` | `#2C7A51` on `#DDF0E5` | `#6CC597` on `#21312F` | "from the QR code" tick, connected tick |
| `p-warn` | `#93650C` on `#F8EFDC` | `#E3B45A` on `#332F26` | Clock and offline error icons, "app only" hint icon |

The dark disc colours are the icon colour at 15 % over `p-bg` (as built); use the hex values. Error/ok/warn are semantic and appear only as icon discs and short status text, never as button fills or as filled boxes. Every button is either primary (`p-primary`) or secondary (`p-surface` with `p-border`).

### 3.4 Type

| Role | Font | Notes |
| --- | --- | --- |
| UI text | **Atkinson Hyperlegible Next**, 400 / 500 / 700, fallback `system-ui, sans-serif` | All text on display and presenter pages |
| Codes and numbers | **Atkinson Hyperlegible Mono**, 500 / 700, fallback `ui-monospace, monospace` | Join code, check number, code input |

Bundle both fonts with the apps (SIL OFL); do not load them from the network on the display. Subsets: the UI font with Latin and Latin Extended (names of people), the mono font with Latin only (codes and digits). Other scripts fall back to the system font; that is accepted.

Display type scale (ref px, see 3.1):

| Use | Size | Weight | Line height | Extra |
| --- | --- | --- | --- | --- |
| Join code | 80 | 700 mono | 1.0 | letter-spacing .06em, groups separated by one space |
| Address (`{dom}`) | 58 | 700 | 1.05 | letter-spacing −.01em |
| Wizard heading, request name | 32 | 700 | 1.15 | long names wrap anywhere |
| Room name | 30 | 700 | 1.05 | |
| Step number disc | 21 | 700 | | 38 × 38 disc in `--acc`, text `--bg` |
| Wizard choice title | 21 | 700 | | |
| Mode pill | 17 | 700 | | |
| Step labels, toast, settings button | 17 | 400 / 700 | | |
| Wizard body, buttons | 18–19 | 400 / 700 | 1.4 | |
| Mode tile, settings description | 15–16 | 500 / 700 | | |
| Footer line, QR caption, name tag | 15 | 400 / 700 | 1.3 | |
| Section label in dialogs | 14 | 700 | | uppercase, letter-spacing .08em |
| Small caps label (`Prüfzahl`) | 11 | 400 | | uppercase, letter-spacing .06em |

Presenter type scale (CSS px):

| Use | Phone | Laptop |
| --- | --- | --- |
| Page heading | 25 / 700 / 1.15 | 24 / 700 |
| Body | 15 / 400 / 1.45 | 15 |
| Label | 13 / 700 | 13 |
| Input | 17; code input 22 mono, letter-spacing .12em, uppercase | same |
| Code chip | 24 mono 700, letter-spacing .08em | same |
| Check number | 58 mono 700, letter-spacing .08em | same |
| Option row title / subtitle | 17 / 700 and 13 / 400 | same |
| Small print | 13 / 400 / 1.4 | same |
| Primary button | 17 / 700 | same |

### 3.5 Spacing, sizes, radii

Display (ref px):

| Item | Value |
| --- | --- |
| Idle screen padding | top 26, sides 40, bottom 18 (≥ 48 device px at 1080p, inside TV title-safe area) |
| Idle vertical gap | 14 |
| Address/code column to QR gap | 36 |
| Steps gap | 12; number to text 16 |
| QR box | 212 × 212, padding 10, radius 14; caption below, gap 8 |
| Settings button | min height 46, padding 0 16 0 12, radius 10, border 2 `--line`, gear 20, gap 8 |
| Mode pill | padding 7 14 7 10, radius 999, border 1 `--line`, icon 20 |
| Dialog card | width 600 (wizard 780), padding 28 × 34, radius 22, border 1 `--line`, gap 16, shadow `0 24px 60px rgba(0,0,0,.55)` |
| Settings sections | gap 14 inside a section, 16 between sections |
| Mode tiles | 3 columns, gap 8, height 60, radius 12, border 2; appearance tiles 4 columns, gap 6, height 46 |
| Wizard / dialog buttons | min height 50, padding 0 22, radius 12, border 2 |
| "No and block" | ghost button, height 34 (not stretched to the row; vertically centred against Yes/No), radius 10, border 2, 14 px text |
| Name tag and End | both min height 46, radius 10, border 2 `--line`, 12 from the bottom and side edges. Name tag: line 1 15/700 `--fg`; optional line 2 (security code) 13/400 `--mut`, digits mono 700 `--fg`; padding 6 14 |
| Annotation toolbar (web display only) | top 12, centred; height 46, radius 10, border 2 `--line`, fill as name tag; icons 20 in `--fg`, buttons 36 × 36; selected tool, selected swatch and selected size: 2 px `--acc` border; disabled button (undo/clear with nothing to undo): icon at 40 % opacity; separators 1 px `--line`; ink swatches 20 discs in their ink colour with a 1 px `--line` ring; keyboard focus (Tab, not in the D-pad rows): outline 2 `--focus`, offset 2, no glow, no scale. Every button has a tooltip and an accessible name (`toolbar.*`, 11.7). Hidden = not focusable (`inert`) |
| Frozen badge | top 12, left 12; same look as the name tag, 15/700 |
| Transfer progress pill | bottom 12, centred; name-tag look (fill, 2 px `--line`, radius 10, min height 46, padding 6 14), text 15/700 `--fg` (`receive.transfer`); shown only while presenting and while a transfer runs, at rest too (it is a state) |
| Toast | top 14, centred, padding 8 20, radius 999, fill `--fg`, text `--bg`, 17/700, shadow `0 10px 30px rgba(0,0,0,.4)`. While the annotation toolbar is visible: top = toolbar bottom + 8 = 66 (never on top of it) |
| Focus ring | outline 4 `--focus`, offset 3, glow 9, scale 1.03. **Text fields** (wizard room name and server address): the same outline, offset and glow, **no scale**; the field's own 2 px border stays `--line` |

Everything on the display, **including toasts, dialogs and the presenting controls**, is placed on the 960 × 540 canvas (3.1), never on the browser viewport. Only the picture and its black background fill the whole viewport while presenting (decision 13.36).

Presenter (CSS px): page padding 18 22 22 (phone), laptop top `max(40px, 10vh)`, sides 40, bottom 40 (decision 13.43); vertical gap 14; error card labels (`error.why`, `error.now`) 12/700 uppercase, letter-spacing .06em, `p-fg3`, 4 above their text, 10 between the two parts; inputs 46 high, radius 8; primary button min 50, radius 10; secondary min 48, radius 10 (phone: full width; laptop: content width, min 160, padding 0 24, see 7.4); option rows min 58 (laptop 50), radius 12; cards radius 14, padding 14; icon disc 56 (icon 28). Touch targets are never below 44 × 44.

### 3.6 Motion

| What | Duration | Easing |
| --- | --- | --- |
| Name tag and End fade in/out | 400 ms | ease |
| Name tag and End auto-hide after last input | 5 s | |
| Mouse movement re-trigger throttle | 400 ms | |
| Focus ring move / scale | 120 ms | ease |
| Toast visible | 3.2 s | appears without animation |
| Idle dim to 40 % brightness | 2 s | ease-out; any activity restores in 200 ms |
| Spinner (presenter waiting) | 1 s per turn | linear |
| Text caret (presenter mock only) | n/a | |

With `prefers-reduced-motion` (presenter pages) or the TV's reduce-motion setting: fades become instant, the focus ring does not scale.

## 4. Display states

The display is in exactly one of these states. Dialogs (Settings, join request) are overlays on top of a state.

| State | Shown when |
| --- | --- |
| Setup | First launch, until "Go to idle screen" |
| Idle, connecting | Idle, no code yet (first 8 s after start or after the connection came back): room name, pill, step 1 with `{dom}` as usual; the code line keeps its height but is empty; empty QR box (3.2), no caption. No spinner, no text. After 8 s without a code → Idle, offline |
| Idle | No one presenting |
| Idle, offline | Idle, and the display cannot reach `{dom}` (or the server refuses it; see 4.2) |
| Idle, dimmed | Idle and no activity for 7 minutes |
| Presenting | A presenter is sending a picture |
| Overlays | Settings (only over Idle), join request (over Idle or Presenting), toast (over anything) |

### 4.1 Setup wizard (first run)

A card (780 ref px wide) centred horizontally on `--bg`. The welcome card (step 0) is centred vertically; for steps 1–4 the card's **top edge is fixed at 64** so the logo and the step dots do not jump between steps (decision 13.46). Top row: logo (40) left; step dots (4 dots, 10 px, done = `--acc`, todo = `--line`) and "Step n of 4" right, hidden on the welcome screen.

| Step | Content | Buttons (D-pad order, left to right) | Initial focus |
| --- | --- | --- | --- |
| 0 Welcome | `setup.welcome.title`, `setup.welcome.body` | `setup.welcome.start` (primary) | Start |
| 1 Room name | `setup.name.title`, `setup.name.body`; suggestion chips (row 1); text field `setup.name.custom` (max 32 characters) | Back, Next (row 2) | Next |
| 2 Who may connect | `setup.mode.title`; two choice cards: Open (eyebrow `setup.mode.fewPeople`, text `setup.mode.openText`) and Approval on (eyebrow `setup.mode.manyPeople`, text `setup.mode.approvalText`); `setup.mode.later` | Back, Next (row 2) | The selected card (Open by default, D1) |
| 3 Address | `setup.address.title`; card with globe icon, `{dom}` (30/700) and where it came from (`setup.address.source*`); `setup.address.body` | Back, Other address, Looks right (primary) | Looks right |
| 4 Done | `setup.done.title`, `setup.done.body` | `setup.done.finish` (primary) | Finish |

- Name suggestions (DE / EN): Besprechungsraum Elbe / Meeting room Elbe, Wohnzimmer / Living room, Vereinsheim / Clubhouse, Werkstatt / Workshop, Raum 204 / Room 204. The first is preselected. A custom name opens the system on-screen keyboard.
- The custom name field starts **empty** (placeholder none). Typing a non-empty name deselects all chips; choosing a chip clears the field. The room name is whichever of the two is set. The chips are one D-pad row even when they wrap onto two lines. **When the field is emptied again, the chip that was selected before typing is selected again** (at first: the first suggestion), so exactly one name is always visible as chosen (decision 13.39). A name of only spaces counts as empty.
- The custom name field and the address field open the on-screen keyboard only on OK / Enter / tap, not when they get focus. Up/Down leave a text field (they never move the caret); Left/Right move the caret.
- Locked is not offered as a default; it only exists during a session.
- The address is detected, not typed: DNS entry of the company network → own server; in-house box found on the network → box; otherwise the default server. "Other address" cycles through the found options (if only one exists, it opens a text field for an address).
- **Web (`/display`)**: a web page cannot see DNS entries or boxes. The card shows the relay host the page uses. Source line: `setup.address.sourceDefault` for the default relay, `setup.address.sourcePage` for any relay that comes from the page's own server or its `config.json`, `setup.address.sourceTyped` for an address typed into the field. `setup.address.sourceOwn` and `setup.address.sourceBox` are used by the native apps only. "Other address" cycles through the relays of `config.json`, then opens the field; the field replaces the address card content in place: label `setup.address.fieldLabel`, the same input style as the room name field, OK/Enter confirms (back to the card with `sourceTyped`), Back cancels. With the field open the button row is [Back, Looks right]: **Looks right** confirms like Enter, **Back** (and Escape) closes the field and stays on step 3 with focus on "Other address", "Other address" is not drawn. After the field was used or cancelled, "Other address" opens the field again; the relays of `config.json` are reached again by going one step back and forward (decision 13.38). An invalid address shows the existing `relay.errors.*` line under the field (16, `--fg`, led by the `info` icon 20 in `--acc`, as the warning lines of 4.5; no red).
- Which source line (web, decision 13.40): `sourceDefault` only for the relay built into the page (`VITE_DEFAULT_RELAY`); the page's own server, a `config.json` relay (also when reached with "Other address") and a confirmed `?server=` relay say `sourcePage`; an address typed into the field says `sourceTyped`.
- Back on the remote goes one step back; on step 0 it does nothing.
- Finish stores room name, default join mode and server, then shows Idle with the chosen mode.

### 4.2 Idle screen

Layout top to bottom (ref px, 960 × 540):

1. **Top row**: logo 46 × 46, room name (30/700), mode pill at the right (icon in `--acc` + word: Open / Approval on / Locked).
2. **Main row** (flex, fills height, centred vertically):
   - Left column: step 1 — number disc "1", label `idle.step1` ("Open in your browser"), address `{dom}` (58/700). For the in-house box (native apps only; the web page never shows it) also a line `idle.altAddress` with the IP address (16, `--mut`). Step 2 — disc "2", label `idle.step2`, the code (80 mono, 6 characters as `XXX XXX`). If Locked: code at 35 % opacity and the line `idle.locked` under it (indented 54).
   - Right column (236 wide): QR code (212 box) and caption `idle.qrCaption`. If Locked: QR at 25 % opacity, caption `idle.qrLocked`.
3. **Footer row**: left (15, `--mut`) `idle.appLine` when at least one store or download link exists, otherwise `idle.appLineWeb` ("No download needed."); never name a store that has no listing. **Settings button** right: gear icon + word `settings.open`.

The QR code encodes `https://{dom}/{CODE}` (code without spaces; for the in-house box the IP address is used instead of `{dom}`). Use error correction level M; the QR version is whatever that URL needs (version 3 or 4 for normal addresses), the box stays 212.

`{dom}/app` is the presenter start page (the same as `{dom}`), which offers the app for the visitor's device.

Open and Approval on look the same apart from the pill. The step 3 "guide" cards from round 2 are gone.

**Offline:** steps 1 and 2 are replaced by a box (`--panel`, dashed 2 px `--line` border, radius 16, wifi-off icon 34 in `--acc`) with `idle.offline.title` (22/700) and a body. The QR box stays in place but is **empty** (3.2: no modules, so the old code cannot be scanned; `--panel2` fill, `--line` border, no caption). When the network returns, code and QR come back by themselves.

Reconnect schedule (decision 13.30): after a loss, try again **at 1 s, 2 s and 3 s**, then every 4 s. The offline box appears **when the attempt at 3 s has failed** (not on a separate timer), so a drop shorter than 3 s never shows the box, and it never flickers away while a later attempt runs. Until then the idle screen stays unchanged (no flicker). When the connection comes back with a new code, code and QR change without animation.

| Cause | Body | Retry |
| --- | --- | --- |
| Network or server not reachable, connection lost | `idle.offline.body` | 1 s, 2 s, 3 s, then every 4 s |
| `rate_limited`, `banned`, `server_full`, `forbidden` | `idle.offline.bodyBusy` (box at once, no 3 s wait) | after 30 s. A connection that closes within 60 s after one of these refusals counts as the same refusal (decision 13.44) |
| `proto_mismatch`, `bad_hello` (page older than server) | `idle.offline.bodyOutdated` | none; reload needed |

If the server refuses Open (`open_rooms_disabled`), the room becomes Approval on, the pill changes and toast `toast.openDisabled` appears once. A failed transfer of content (damaged PDF …) is a toast with the existing wording.

### 4.3 Settings dialog

Opens only over the idle screen (not during setup, not while presenting, not while a join request is open).

Content (card 600 wide, scrim behind):

1. Header row: `settings.title` (section-label style, `--mut`) left; Close button right: × icon + `settings.close` (remote: `settings.closeRemote`, i.e. "Close (Back key)").
2. Section `settings.join.label` ("Joining"): three tiles Open / Approval on / Locked (icon + word), then one line describing the current mode (`settings.join.*Text`).
3. Section `settings.appearance`: four tiles Dark / Light / OLED black / Automatic.

Selected tile: border `--acc`, fill = selected tile fill (OLED black: no fill, see 3.2), bold. Changes apply at once; there is no Save button. The dialog stays open after a change.

What is remembered: the appearance, and Open or Approval on (they become the new default mode, replacing the one from setup). **Locked is never stored**: it lasts until End (the next session starts in the default mode) or until the display restarts.

**A join request closes Settings.** Only one dialog is on screen at a time: when a request arrives while Settings is open, Settings closes (changes already made stay) and the request dialog opens with focus on Yes. After the last request, focus returns to the Settings button. Consequence: Locked can never be chosen while requests wait, so there is no "Locked · n requests denied" toast (`toast.lockedRejected` is removed).

Initial focus: the tile of the current join mode.

### 4.4 Join request dialog (mode Approval on)

Appears by itself when someone asks to join, over the idle screen **or over the running presentation** (the picture stays visible behind the 55 % scrim). It is an `alertdialog`.

Content (card 600 wide):

1. Eyebrow `request.title` (section-label style). If more requests wait: `request.queue` at the right ("+2 waiting").
2. Row: left the name in quotes (32/700, wraps anywhere) and below it `request.subtitle` ("wants to connect"; there is no device type, the protocol does not carry one); right the check-number box (`--bg` fill, radius 10, label `request.check` 11 caps, number 24 mono 700; OLED black: plus 1 px `--line` border).
3. Buttons: **Yes** (primary, `--acc`), **No** (secondary), gap, then **No and block** (ghost, 34 high, centred in the row) at the far right.

Behaviour:

- One press of Yes allows. No hold.
- **Input is ignored for 1 s after the dialog appears** (and after it switches to the next request), so a stray key press approves nothing.
- Initial focus: Yes.
- Requests are handled first come, first served; after a decision the next request takes the dialog and focus returns to Yes (with the 1 s guard again). With no more requests the dialog closes; focus returns to the Settings button (idle) or nothing (presenting; the overlay stays hidden).
- A request expires when the **server** ends it (`join_approval_secs`, 60 s by default; operators may change it). The display has no timer of its own: on the server's cancel message the request leaves the queue, toast `toast.expired`, the presenter sees "No answer". The presenter's countdown uses `join_pending.timeout_secs`.
- No and block: this device cannot ask again until the session ends; its later attempts are not shown on the display at all and the device sees "Not allowed". A new code (End) lifts the block.
- After No, the same device may ask again after 15 s (enforced on the presenter page with a countdown).
- Names: strip control and invisible characters, collapse whitespace, cut to 18 characters + "…", show in quotes. Empty name → `common.noName`. Show the full name only in the accessible label.
- Back on the remote does nothing while the dialog is open (an answer is required). Menu does nothing.

### 4.5 Presenting

The picture fills the screen edge to edge on black (`#000000`), letterboxed in its own aspect ratio (a portrait phone screen sits centred). **Nothing else is shown at rest**: no code, no room name, no mode.

On input (see 8) the controls fade in together for 5 s and fade out together:

- **Name tag**, bottom left: `present.byline` ("“Lena” is showing"), 15/700, `--fg` on the panel fill, border `--line`. Second line, when the security code is known: `present.sas` ("Security code 171 852"), 13/400 `--mut`, digits mono 700 `--fg`.
- **End**, bottom right: square stop icon + `present.end`. Same fill, border and text colour as the name tag. Not pink, not amber.
- **Annotation toolbar** (web display only, where a mouse, pen or touch exists; the TV apps have none): top centre, see 3.5. Pen, highlighter, eraser, ink swatches, undo, clear, laser, freeze, black screen, full screen. No grip (it does not move) and no collapse button (it hides by itself). It is not in the D-pad focus rows; Tab reaches it. Drawing counts as input, so the controls stay while someone draws; strokes stay visible after the controls fade.

All use the current appearance's `--panel`, `--line` and `--fg` (dark blue in Dark, light grey in Light, black with grey border in OLED black). With D-pad input, End shows the focus ring.

Shown only while it applies, at rest too (they are states, not controls): the **frozen badge** top left (`toolbar.frozenHint`), and the black screen with `toolbar.blackHint` (15, `--mut` on `#000000`).

Not on the display: "Close image", "Close document" and PDF page buttons. Pages are turned on the presenter's page (5.6); content goes away when the presenter stops, disconnects or the host presses End.

**Link sent by a presenter:** the display never opens a page by itself (safety rule). The link card is a dialog in the style of the join request (4.4: 600 card, scrim, section label, `receive.link.title`, the address in 18 mono `--mut` wrapping anywhere, `receive.link.hint`), buttons **Open** (primary) and **Dismiss** (secondary), initial focus Open, the same 1 s input guard, Back = Dismiss. On the web display Open opens a new tab. The section label is `receive.link.title` ("{host} wants to open a link"); there is no separate heading.

- **Media link** (decision 13.34): a secondary button **Play** (`receive.media.play`) comes before Open: focus row [Play, Open, Dismiss], initial focus still Open. Warning and error lines (`receive.media.localWarning`, `.mixed`, `.hlsUnsupported`, `.error.*`) are 16/700 `--fg`, led by the `info` icon 20 in `--acc`; all other card text is `--mut`. No red. While playing, the card shows the section label, the address and the player: a header row (`receive.media.from` 15/700 `--fg` left, secondary **Stop** right) and the media element, full card width, at most 300 high, on `#000000`. Stop, Back/Escape and Dismiss close the whole card.
- **Platforms that cannot open a web page** (Apple TV; an Android TV without a browser): no Open button; the hint is `receive.link.hintTv`; focus row [Dismiss] or [Play, Dismiss] for media; initial focus Dismiss (Play for media).

**Content states while presenting** (decision 13.41): a PDF page is drawn on `#000000` like every picture (no grey page background). A running transfer shows the transfer progress pill (3.5). A failed transfer or a PDF that cannot be shown is a **toast** (`receive.transferError.*`); a single PDF page that cannot be drawn shows its `receive.document.pageError.*` text centred in a name-tag-look box (15/700 `--fg`). No red, no black pills. The **black screen, frozen picture and all strokes end with the presentation**: when the presenter stops, disconnects or the host presses End, the next presentation starts with a visible, live picture and no strokes.

**Annotation toolbar contents** (web display only, left to right, separators between groups): [Pen, Highlighter, Eraser] · [5 ink swatches: red, yellow, green, blue, white; 3 sizes: thin, medium, thick] · [Undo, Clear all, Laser pointer] · [Freeze, Black screen, Full screen]. Accessible names and tooltips: `toolbar.*` (11.7). No tool is selected at first (the pointer only shows the controls); choosing a tool marks it with the `--acc` border. The toolbar is the only place on the display with icon-only buttons (decision 13.35); it is a pointer tool, every button has a tooltip and an accessible name.

**End** closes the session: all presenters are disconnected, all waiting requests are dropped, blocks are cleared, a new code is made, the display returns to Idle, toast `toast.sessionEnded`. Presenters see "Presentation ended". There is no confirmation step.

While presenting, the screen is kept awake (D5). When the presenter stops, the display returns to Idle with the same code; presenters stay connected.

### 4.6 Toasts

One line, centred at the top, `--fg` background with `--bg` text, 3.2 s. A new toast replaces the old one. Toasts never take focus and never cover the annotation toolbar (3.5). Keys: see the `toast.*` table.

## 5. Presenter flow

### 5.1 Entry routes

| Route | Start | Lands on |
| --- | --- | --- |
| Phone camera | System camera recognises the QR code; the system chip opens `https://{dom}/{CODE}` | Start page with code already filled in |
| Typed address | Person types `{dom}` into the browser | Start page with an empty code field |
| App | App opened from the store link or home screen | App first-run page |

### 5.2 Start page (browser or app choice)

This is the page at `{dom}` and `{dom}/{CODE}`.

Phone (single column):

1. Brand row: logo 28 + "Comet Cast".
2. Heading: always `start.titleGeneric` (the room name is not known before joining; `start.titleRoom` is not used).
3. Code: from QR → code chip (code in mono as `XXX XXX` + green tick + `start.fromQr`); typed → field `start.codeLabel` with placeholder `XXX XXX`, `inputmode` text, uppercase, autocomplete off, max 9 characters.
4. Field `start.nameLabel` (max 40 characters, remembered on the device).
5. Primary button with globe icon `start.browser`, small print below: `start.browserPhone`.
6. Divider with `start.or`.
7. Secondary button with download icon: `start.appAndroid` / `start.appIos`; small print `start.appPhone`.
8. Info line with (i) icon: `start.detected*` ("Detected: Android phone, Chrome"). Plain text, not a link.

Items 6 and 7 (and the laptop's "or" + `start.appDesktop` + `start.appLaptop` + `start.downloadStarted`) are shown **only when the store or download link for that platform exists**. Without one the page ends with the browser button, its small print and the detected line. A button never does nothing and the page never claims a download it did not start.

Laptop (≥ 720 px wide): two columns. Left: brand, heading, code, name. Right, in a card: browser button + small print `start.browserLaptop`, "or", `start.appDesktop` + `start.appLaptop`, detected line, and `start.beDisplay` ("Should this device be the screen itself? Open {dom}/display."), where `{dom}/display` is a **link** (`p-primary`, underlined) to the display page in the same tab. Clicking the desktop download starts the download and shows `start.downloadStarted` under it as small print (`p-fg2`, no coloured box); the person can continue in the browser meanwhile.

Code handling: the field shows what is typed, in uppercase; it never drops characters silently. On submit: remove spaces and dashes. Validate on submit only. Empty → inline error `start.codeEmpty` (field border `p-error`). Not exactly 6 characters from the code alphabet (for example an O, 0, I or 1, or a wrong length) → error page `error.code` without asking the server. Unknown code → the same error page. There is only one code format, so the error page "This code belongs to another address" (`error.otherServer`) is not used.

"Continue in the browser" → connect: Open mode → "What to show" page; Approval on → waiting page; Locked → "Locked right now"; blocked device → "Not allowed".

### 5.3 App path

- Store: the system store page (not designed by us). Android passes code and address through the Google Play install referrer (assumption, not yet verified). iOS cannot, so the app asks to scan again.
- App first-run page: heading `app.welcome`; code chip with `app.codeFromLink` (Android) or `app.codeScanned` (after scanning); if there is no code: `app.scanAgainBody` and a secondary button `app.scanQr`; name field; primary `app.connect` (disabled while there is no code).
- Platform variants (details in [native-apps.md](native-apps.md)): devices without a camera (computers) show the code field of 5.2 instead of `app.scanQr`; on tablets and computers the page ends with the text link `app.beDisplay`, which starts the display setup (4.1). Scanning uses the system scanner (not designed by us). In the apps the disconnected page (5.7) omits `bye.body` (it speaks of a browser tab).

### 5.4 Waiting for approval

Spinner + room name (label style, from `join_pending.room_name`; without a room name the row shows the spinner only, never the code); heading `wait.title`; card with `wait.checkLabel`, the check number (58 mono, `p-primary`) and `wait.checkHint`; progress bar (6 px) counting down `join_pending.timeout_secs` (60 s by default) with `wait.expires`; at the bottom secondary `wait.withdraw`. Withdrawing removes the request from the display (toast `toast.withdrawn`) and returns to the start page.

### 5.5 What to show

Header row: green tick disc (40) + `what.connectedTo` (room name from `joined.room_name`; without one `what.connected`, never the code). Heading `what.title`. Three option rows (icon in `p-primary`, title + subtitle):

| Row | Title | Subtitle |
| --- | --- | --- |
| Screen | `what.screen` | Laptop: `what.screenLaptop`; app: `what.screenApp`; phone browser: `what.screenPhoneBrowser` when an app can be offered for this device, otherwise `what.screenPhoneBrowserNoApp` (icon in `p-warn` in both cases) |
| File | `what.file` | `what.fileSub` |
| Link | `what.link` | `what.linkSub` |

Bottom: secondary `what.disconnect` → "Disconnected" page; toast `toast.left` on the display. No security code on this page (the display shows it only while presenting).

- Screen on a laptop → the browser's own share dialog opens directly, with "Window" preselected where the browser allows it. There is **no** explainer page before it.
- Screen in a phone browser → error "Screen sharing does not work in this browser" (it is known before; the page says so in the subtitle). Without an app offer for the device (13.6) the page has no "Get the app" button and its "What now" text is `error.noScreen.nowNoApp` (decision 13.42).
- Screen in the app → the system prompt (Android "Start recording or casting", iOS "Start Broadcast").
- Cancelling the system dialog returns to this page without an error (`capture_denied` never shows an error page).
- File → file page. **App:** heading `file.title`, `file.body`, recent files as a 2-column grid of thumbnails, secondary `common.back`. **Browser** (a browser cannot list recent files): heading `what.file`, body `file.bodyWeb`, primary `file.pick` (opens the system file dialog: PNG, JPEG, WebP, GIF, PDF), then while sending a progress line (small print "{name} · {n} %" + 6 px bar), an inline error in `p-error` if it fails, secondary `common.back` at the bottom. Picking a file starts presenting. Cancelling the dialog changes nothing.
- Link → link page: heading `what.link`, field `link.label` (type url), `link.hint`, primary `link.go`, secondary `common.back`. The display asks its host before opening (4.5).
- Replacing what is shown: "Stop showing", then pick again (the display shows Idle in between). No "replace" action.

### 5.6 Presenting (presenter side)

Icon disc + heading (`presenting.screen` / `presenting.file` / `presenting.link`), body `presenting.where` (omitted when there is no room name), small print `presenting.sas` when the security code is known (`p-fg3`, digits mono 700 `p-fg`), primary button `presenting.stop`. Stop returns to "What to show". The browser's own "sharing" bar or the phone's red indicator stay as the system draws them; when Chrome's bar is shown at the bottom of the page, keep 76 px free at the bottom of the page.

While a PDF is shown: a row of two secondary buttons `presenting.prevPage` and `presenting.nextPage` (phone: side by side, each half width, min 48 high) **directly above Stop** (8 between them; on the phone both rows are part of the bottom action area, so the thumb reaches all three), with `presenting.pageOf` ("Page 3 of 12") as small print above the row when the page count is known. When the page is known, Previous is disabled (`p-disabled`) on page 1 and Next on the last page; while it is unknown both stay enabled. **Protocol requirement (decision 13.31):** the display reports the shown page and the page count to the presenter over the content control channel whenever they change (`page`, `pages`); until this message exists the line is not shown and both buttons stay enabled.

### 5.7 Disconnected

Grey tick disc, `bye.title`, `bye.body`, primary `bye.again` (back to the start page; phone: start route).

## 6. Error states (presenter pages)

Every error page has the same layout: icon disc (56), heading, a card with two labelled parts (`error.why`, `error.now`), and buttons at the bottom (primary first; laptop: in a row, phone: stacked).

| Key prefix | When | Icon | Buttons |
| --- | --- | --- | --- |
| `error.code` | Unknown code, or not a valid code (5.2) | × in `p-error` | Retype code (primary); app on a phone: Scan QR code again |
| `error.expired` | Code was valid, no longer (`room_not_found` after waiting or after being connected) | clock in `p-warn` | Retype code; app on a phone: Scan again |
| `error.otherServer` | **Not used** (one code format; kept for a later multi-server design) | (i) in `p-primary` | Open {dom} (primary) |
| `error.noInternet` | Presenter device offline | × in `p-warn` | Try again (also retries by itself) |
| `error.noScreen` | Phone browser, Screen chosen | (i) in `p-primary` | Show photo or file (primary), Show a link, Get the app |
| `error.displayOff` | Display does not answer | × in `p-warn` | Try again |
| `error.denied` | Host chose No, or device is blocked | × in `p-error` | Ask again (disabled for 15 s, label counts down) |
| `error.locked` | Mode is Locked (two "why" variants: at once / while waiting) | lock in grey | Try again |
| `error.timeout` | No answer within 60 s | clock in `p-warn` | Ask again |
| `error.ended` | Host pressed End, or the host removed this presenter (`removed_by_host`) | (i) in grey | Retype code; app on a phone: Scan again |
| `error.occupied` | Someone else is already connected (`room_full`; one presenter per room) | (i) in grey | Try again |
| `error.busy` | `rate_limited`, `banned`, `server_full` | clock in `p-warn` | Try again |
| `error.outdated` | `proto_mismatch`, `bad_hello`, `bad_message` (page and server do not match) | (i) in grey | `error.action.reload` (reloads the page) |
| `error.other` | Every other failure: `forbidden`, `altcha_*`, `wrong_role`, `not_in_room`, `unknown` and any code not listed (decision 13.32) | (i) in grey | Try again |

For the in-house box, `error.noInternet` uses the `nowBox` text (must be on the same Wi-Fi). Mapping of other failures:

- `connection_lost` → `error.noInternet`, **except** within 60 s after a `rate_limited`, `banned` or `server_full` refusal, where it is `error.busy` again (the server closes the connection after a ban; decision 13.44).
- `peer_failed` → `error.displayOff`; **but** when the list of STUN/TURN servers (`/ice-servers`) was refused for this connection (`rate_limited` or `refused`) → `error.busy`, and when it failed otherwise (`http_error`, `bad_answer`, `unreachable`) → `error.displayOff` with the "why" text `error.displayOff.whyRelay`. A relay that offers no list (404, `none`) changes nothing. Nothing is shown before the connection fails (decision 13.45).
- `capture_denied` → no error page (back to "What to show").
- Anything not listed → `error.other`: heading `error.other.title`, "why" = the existing text `errors.<code>`, "what now" = `error.other.now`. `error.code` is used only for codes that are wrong or unknown.

"Scan QR code again" exists only in the apps (a browser page cannot open the camera); web error pages never show it.

## 7. Layout per device class

### 7.1 TV (Google TV / Android TV, Apple TV), D-pad

- Full screen, landscape, scaled as in 3.1. Everything inside the title-safe area (the paddings in 3.5 already are).
- Focus is always visible on exactly one control (ring per 3.5) whenever a focusable control is on screen. At rest during a presentation nothing is focusable and nothing is drawn.
- Focus moves in rows; Left/Right move within a row, Up/Down move to the same column (or the last item) of the next row. Focus does not wrap.

Focus order (rows top to bottom):

| Screen | Rows |
| --- | --- |
| Idle | [Settings] |
| Settings | [Close] · [Open, Approval on, Locked] · [Dark, Light, OLED black, Automatic] |
| Join request | [Yes, No, No and block] |
| Link card | [Open, Dismiss]; media link [Play, Open, Dismiss]; platform without a browser [Dismiss] or [Play, Dismiss] (Back = Dismiss) |
| Media playing in the link card | [Stop] (Back = close the card) |
| Relay confirmation (`/display`) | [Cancel, Use this relay] |
| Presenting (controls shown) | [End] |
| Setup 0 | [Start] |
| Setup 1 | [5 name chips] · [field] · [Back, Next] |
| Setup 2 | [Open, Approval on] · [Back, Next] |
| Setup 3 | [Back, Other address, Looks right]; field open: [field] · [Back, Looks right] |
| Setup 4 | [Finish] |

Keys:

| Key | Idle | Settings open | Join request | Presenting, controls hidden | Presenting, controls shown | Setup |
| --- | --- | --- | --- | --- | --- | --- |
| Arrows | move focus | move focus | move focus | show controls, focus End (the key does nothing else) | move focus, restart 5 s | move focus |
| OK / Select | activate | activate | activate (after 1 s guard) | show controls (does not activate) | activate End | activate |
| Back | nothing (system may leave the app) | close Settings | nothing | show controls | hide controls | previous step |
| Menu (where the remote has one) | open Settings | close Settings | nothing | show controls | nothing | nothing |

- Google TV remotes and the Siri Remote usually have no Menu key. Settings must always be reachable by focus: on Idle, focus starts on the Settings button. On Apple TV, map Play/Pause to Menu as a shortcut.
- The Back key on Android TV during Idle exits the app only on a second press within 2 s; the first press is consumed silently. This prevents leaving the display by accident.

### 7.2 Computer, board or tablet (`{dom}/display`, mouse, keyboard, touch)

- Same screens and sizes, scaled to the window. The page asks for fullscreen on first click (optional; ignore rejection) and uses the Screen Wake Lock API while presenting (D5).
- A portrait or very narrow window (a phone) gets the 16:9 canvas centred and letterboxed, as in 3.1. There is no portrait layout; a phone is not a target display.
- Focus ring: on page load the initial focus shows the ring (the page cannot know yet whether a keyboard or remote drives it). The first pointer press, touch start or real mouse movement (non-zero distance) hides it; the next key press shows it again (decision 13.50). Same ring as on TV.
- The server (or any static host) must answer `/display`, `/{CODE}` and `/app` with `index.html`; otherwise the QR code leads to a 404 (GitHub Pages: a copy of `index.html` as `404.html`).
- A relay that only a link asked for (`?server=`) is confirmed **before anything else is shown or connects** (decision 13.37). Nothing of the page that would use the relay is drawn behind it, and the relay host appears nowhere else (not as `{dom}`, not in the `start.beDisplay` link) until it is confirmed.
  - **On `/display`:** on the scaled canvas (3.1, all sizes in reference px), over plain `--bg` with only the logo (46) at the idle screen's top-left position. Dialog card 600 wide as 4.4 (no scrim needed: there is nothing behind it), centred: section label `relay.confirm.title`; body `relay.confirm.body` 16 `--mut`; the host on its own line, 18 mono `--fg`, wrapping anywhere; `relay.confirm.risk` 16 `--mut`; buttons [**Cancel** (`relay.confirm.cancel`, secondary, initial focus), **Use this relay** (primary)], left-aligned, 50 high. Back/Escape = Cancel. Cancel continues with the default relay (setup or idle); Use continues with the confirmed relay.
  - **On presenter pages:** its own **page** in the error-page layout (6), replacing the start page: icon disc `info` in `p-warn`; heading `relay.confirm.title`; body `relay.confirm.body` (15, `p-fg2`); the host on its own line, 17 mono 700 `p-fg`; `relay.confirm.risk` as small print; actions [**Cancel** secondary, **Use this relay** primary] in this order (phone: stacked at the bottom, Cancel on top; laptop: in a row below the content, Cancel left). Cancel has the initial focus; Enter on load cancels. After the decision the start page appears (with the code chip if the link carried a code).
  - Copy unchanged (11.7).
- Mouse movement over the screen during a presentation shows the controls (throttled to one trigger per 400 ms); a click or tap anywhere shows them; clicking End ends the session. The pointer hides together with the controls.
- Keyboard: arrows = D-pad, Enter or Space = OK, Escape = Back, `M` = Menu. Tab follows the focus order above.
- Settings button opens Settings by click or tap.

### 7.3 Presenter: phone

Width 360–430 px, single column, primary action at the bottom of the page where the page has one main action. Inputs use the platform keyboard; the code field shows the alphanumeric keyboard in uppercase.

### 7.4 Presenter: laptop

Pages wider than 720 px use the two-column layouts given in 5.2 and the error pages; buttons sit in a row. Max content width 744 px, centred. The action row **follows the content** (24 px below it), buttons content width (min 160, padding 0 24), primary first (the relay confirmation page is the one exception: Cancel first, 7.2), left-aligned; it is not pinned to the bottom of the window (that is the phone layout). Presenter pages are real pages in the browser; the mockup's device frames are not drawn.

## 8. Interaction rules (summary)

1. **Presenting is quiet.** Only the picture until someone touches an input. Then name tag + End (on the web display also the annotation toolbar) for 5 s after the last input, 400 ms fade. Nothing is drawn at rest except the frozen badge or black-screen hint while they apply.
2. **First input only reveals.** On TV, the key press that reveals the controls never activates anything.
3. **Dialogs over pictures.** A join request appears over the presentation without stopping it.
4. **1 s guard** on every new join request.
5. **No hold gestures** anywhere.
6. **Idle dimming (D5).** After 7 minutes on Idle without activity (no input, no join, no request, no mode change), fade the whole idle screen to 40 % brightness (overlay of `--bg` at 60 %) over 2 s. Code and QR stay readable up close. Any input, join request or connection restores full brightness in 200 ms; the first key press only restores (does not activate). The app does **not** hold a wake lock on Idle, so the system screensaver or sleep may follow its own timer.
7. **Wake lock (D5).** Acquire when the first frame of a presentation is shown; release when the presentation stops, the session ends, or the app goes to the background. Android TV: `FLAG_KEEP_SCREEN_ON` on the window only while presenting. Apple TV: `isIdleTimerDisabled = true` only while presenting. Web: `navigator.wakeLock.request('screen')` only while presenting; re-request on `visibilitychange`.
8. **OLED black (D4)** never moves pixels and never adds a screensaver of its own.

## 9. Accessibility

- Contrast: all text pairs in 3.2 and 3.3 meet 4.5:1 (large display text meets it with margin).
- Join request is announced as an alert dialog with the label "Request: “{name}”".
- Mode tiles and appearance tiles are radio groups; the settings button has the visible word as its label.
- Toasts are polite live regions.
- Icon-only items do not exist on the display, except the annotation toolbar of the web display (pointer tool; every button has a tooltip and an accessible name, 4.5). The QR code has the label "QR: {url}".

## 10. Not included (do not build)

- Accounts, sign-in, room lists, scheduling.
- A list of connected people, removing a single presenter, "deny all", the round 2 hold-to-allow request cards and the request banner during presentation.
- Any control during a presentation other than End (no disconnect-person, no mode, no appearance, no code overlay, no close-image, no page buttons). Exceptions: the annotation toolbar of the web display (shown only together with End) and the link card (appears by itself, like a join request).
- The step-by-step guide cards on the idle screen.
- The explainer page before the browser's share dialog.
- Showing several presenters at once (split screen).
- Renaming the room, changing the server address or re-running setup from Settings. To change these, reset the app's data (TV) or clear site data (`/display`). A later round may add this.
- OLED black on presenter pages.
- AirPlay, Miracast and Google Cast receiving (covered elsewhere; not part of this flow).
- The mockup's own frame: chapter tabs, simulated remote, scenario buttons, notes, device switcher, the "view from 4 m" preview, the host instructions panel.
- Store pages, system share dialogs and camera chips: they are drawn by the operating system.
- Share sheet entry points ("Share to Comet Cast" from other apps) on any platform: not in this round (native-apps.md, section 9).

## 11. Copy

### 11.1 Rules for German

- **Shipped default: "Sie"** (column "DE Sie"). The column "DE du" is the wording of the mockup. The owner may switch to "du" later.
- Implement this as two German value sets with identical keys (for example `de.json` = Sie and a `de-du.json` overlay, or two ARB files), so switching is one file swap, not a code change.
- Every German string is a short main clause. Only the pronoun and the verb ending change between the two forms; sentence structure stays the same. Do not add strings in only one form.
- English has one form.
- Placeholders: `{room}`, `{dom}`, `{code}`, `{name}`, `{device}` (only in `start.detected`), `{n}`, `{total}`, `{sas}` (security code, `123 456`), `{ip}`, `{url}`, `{os}` (Windows, macOS, Linux), `{browser}` (Chrome, Safari, …), `{here}` (the address the presenter page is on). Quotes are part of the string where shown (German „…“, English “…”). Names are inserted already quoted via `common.quoted`.

Keys use the web app's dotted form. Flutter ARB uses the same keys in lower camel case (`settings.join.openText` → `settingsJoinOpenText`). Apple TV uses the dotted keys as they are.

### 11.2 Common

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `common.back` | Back | Zurück | Zurück |
| `common.next` | Next | Weiter | Weiter |
| `common.cancel` | Cancel | Abbrechen | Abbrechen |
| `common.or` | or | oder | oder |
| `common.noName` | No name | Ohne Namen | Ohne Namen |
| `common.quoted` | “{name}” | „{name}“ | „{name}“ |
| `common.stepOf` | Step {n} of 4 | Schritt {n} von 4 | Schritt {n} von 4 |
| `device.phone` | Phone | Handy | Handy |
| `device.android` | Android phone | Android-Handy | Android-Handy |
| `device.iphone` | iPhone | iPhone | iPhone |
| `device.ipad` | iPad | iPad | iPad |
| `device.laptop` | Laptop | Laptop | Laptop |
| `device.chromebook` | Chromebook | Chromebook | Chromebook |
| `mode.open` | Open | Offen | Offen |
| `mode.approval` | Approval on | Mit Freigabe | Mit Freigabe |
| `mode.locked` | Locked | Gesperrt | Gesperrt |
| `appearance.dark` | Dark | Dunkel | Dunkel |
| `appearance.light` | Light | Hell | Hell |
| `appearance.oled` | OLED black | OLED-Schwarz | OLED-Schwarz |
| `appearance.auto` | Automatic | Automatisch | Automatisch |

### 11.3 Display: setup

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `setup.welcome.title` | Hi! This screen is about to become a display device. | Hallo! Dieser Bildschirm wird zum Anzeigegerät. | Willkommen! Dieser Bildschirm wird zum Anzeigegerät. |
| `setup.welcome.body` | Afterwards anyone in the room can show here what is on their phone or laptop. With a code, no account. | Danach kann jeder im Raum hier zeigen, was auf seinem Handy oder Laptop ist. Mit einem Code, ohne Konto. | Danach kann jeder im Raum hier zeigen, was auf seinem Handy oder Laptop ist. Mit einem Code, ohne Konto. |
| `setup.welcome.start` | Set up | Einrichten | Einrichten |
| `setup.name.title` | What is this room called? | Wie heißt dieser Raum? | Wie heißt dieser Raum? |
| `setup.name.body` | The name is shown large on the screen so people know they are in the right place. | Der Name steht groß auf dem Bildschirm, damit alle wissen, dass sie richtig sind. | Der Name steht groß auf dem Bildschirm, damit alle wissen, dass sie richtig sind. |
| `setup.name.custom` | Custom name | Eigener Name | Eigener Name |
| `setup.name.s1`…`s5` | Meeting room Elbe · Living room · Clubhouse · Workshop · Room 204 | Besprechungsraum Elbe · Wohnzimmer · Vereinsheim · Werkstatt · Raum 204 | same as du |
| `setup.mode.title` | Who may connect? | Wer darf sich verbinden? | Wer darf sich verbinden? |
| `setup.mode.fewPeople` | Few people | Wenige Leute | Wenige Leute |
| `setup.mode.manyPeople` | Many people | Viele Leute | Viele Leute |
| `setup.mode.openText` | Anyone who sees the code connects at once. For small groups, teams and at home. | Wer den Code sieht, ist sofort verbunden. Für kleine Runden, Teams und zu Hause. | Wer den Code sieht, ist sofort verbunden. Für kleine Runden, Teams und zu Hause. |
| `setup.mode.approvalText` | Each person shows up here as a request and you let them in. For rooms many people use. | Jede Person erscheint hier als Anfrage, du lässt sie rein. Für Räume, die viele nutzen. | Jede Person erscheint hier als Anfrage, Sie lassen sie herein. Für Räume, die viele nutzen. |
| `setup.mode.later` | You can switch or lock any time later in the settings. | Umschalten und Sperren geht später jederzeit in den Einstellungen. | Umschalten und Sperren geht später jederzeit in den Einstellungen. |
| `setup.address.title` | Which address will others use? | Über welche Adresse kommen die anderen? | Über welche Adresse kommen die anderen? |
| `setup.address.body` | This address is shown large on the screen and is inside the QR code. | Diese Adresse steht später groß auf dem Bildschirm und steckt im QR-Code. | Diese Adresse steht später groß auf dem Bildschirm und steckt im QR-Code. |
| `setup.address.sourceDefault` | Default server. Nothing to set up. | Standardserver. Nichts einzustellen. | Standardserver. Nichts einzustellen. |
| `setup.address.sourceOwn` | Found via the company network (DNS entry). | Gefunden über das Firmennetz (DNS-Eintrag). | Gefunden über das Firmennetz (DNS-Eintrag). |
| `setup.address.sourceBox` | In-house box found on this network. | Box im Haus im selben Netz gefunden. | Box im Haus im selben Netz gefunden. |
| `setup.address.ok` | Looks right | Passt | Passt |
| `setup.address.change` | Other address | Andere Adresse | Andere Adresse |
| `setup.done.title` | Done. | Fertig. | Fertig. |
| `setup.done.bodyRemote` | Open the settings any time with the Menu key or the Settings button: joining and appearance. | Die Einstellungen öffnest du jederzeit mit der Menü-Taste oder „Einstellungen“: Beitritt und Aussehen. | Die Einstellungen öffnen Sie jederzeit mit der Menü-Taste oder „Einstellungen“: Beitritt und Aussehen. |
| `setup.done.bodyPointer` | Open the settings any time with “Settings” at the bottom right: joining and appearance. | Die Einstellungen öffnest du jederzeit mit „Einstellungen“ unten rechts: Beitritt und Aussehen. | Die Einstellungen öffnen Sie jederzeit mit „Einstellungen“ unten rechts: Beitritt und Aussehen. |
| `setup.done.finish` | Go to idle screen | Zum Wartebildschirm | Zum Wartebildschirm |

Note: `setup.done.bodyRemote` adds "or the Settings button" to the mockup text, because many remotes have no Menu key (7.1).

### 11.4 Display: idle, settings, request, presenting, toasts

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `idle.step1` | Open in your browser | Im Browser öffnen | Im Browser öffnen |
| `idle.step2` | Enter this code | Diesen Code eingeben | Diesen Code eingeben |
| `idle.altAddress` | or {ip} | oder {ip} | oder {ip} |
| `idle.locked` | Locked right now. | Gerade gesperrt. | Gerade gesperrt. |
| `idle.qrCaption` | Or scan | Oder scannen | Oder scannen |
| `idle.qrLocked` | Locked | Gesperrt | Gesperrt |
| `idle.appLine` | No download needed. Prefer an app: App Store · Google Play · {dom}/app | Kein Download nötig. Lieber als App: App Store · Google Play · {dom}/app | Kein Download nötig. Lieber als App: App Store · Google Play · {dom}/app |
| `idle.offline.title` | No connection to {dom} | Keine Verbindung zu {dom} | Keine Verbindung zu {dom} |
| `idle.offline.body` | This screen is offline right now. Code and QR code come back by themselves once the network is back. | Dieser Bildschirm ist gerade offline. Sobald das Netz wieder da ist, erscheinen Code und QR-Code von selbst. | Dieser Bildschirm ist gerade offline. Sobald das Netz wieder da ist, erscheinen Code und QR-Code von selbst. |
| `settings.open` | Settings | Einstellungen | Einstellungen |
| `settings.title` | Settings | Einstellungen | Einstellungen |
| `settings.close` | Close | Schließen | Schließen |
| `settings.closeRemote` | Close (Back key) | Schließen (Zurück-Taste) | Schließen (Zurück-Taste) |
| `settings.join.label` | Joining | Beitritt | Beitritt |
| `settings.join.openText` | Anyone with the code connects at once. | Jeder mit dem Code ist sofort verbunden. | Jeder mit dem Code ist sofort verbunden. |
| `settings.join.approvalText` | You let each person in one by one. | Du lässt jede Person einzeln zu. | Sie lassen jede Person einzeln zu. |
| `settings.join.lockedText` | Nobody new can join. Connected stay. | Niemand kommt neu hinzu. Verbundene bleiben. | Niemand kommt neu hinzu. Verbundene bleiben. |
| `settings.appearance` | Appearance | Aussehen | Aussehen |
| `request.title` | Request | Anfrage | Anfrage |
| `request.queue` | +{n} waiting | +{n} warten | +{n} warten |
| `request.subtitle` | wants to connect | möchte sich verbinden | möchte sich verbinden |
| `request.check` | Check | Prüfzahl | Prüfzahl |
| `request.yes` | Yes | Ja | Ja |
| `request.no` | No | Nein | Nein |
| `request.noBlock` | No and block | Nein und sperren | Nein und sperren |
| `request.aria` | Request: {name} | Anfrage: {name} | Anfrage: {name} |
| `present.byline` | {name} is showing | {name} zeigt | {name} zeigt |
| `present.sas` | Security code {sas} | Sicherheitscode {sas} | Sicherheitscode {sas} |
| `present.end` | End | Beenden | Beenden |
| `toast.joined` | {name} connected | {name} ist verbunden | {name} ist verbunden |
| `toast.allowed` | {name} allowed | {name} zugelassen | {name} zugelassen |
| `toast.denied` | {name} denied | {name} abgelehnt | {name} abgelehnt |
| `toast.blocked` | {name} denied and blocked | {name} abgelehnt und gesperrt | {name} abgelehnt und gesperrt |
| `toast.expired` | Request from {name} expired | Anfrage von {name} verfallen | Anfrage von {name} verfallen |
| `toast.withdrawn` | {name} withdrew the request | {name} hat die Anfrage zurückgezogen | {name} hat die Anfrage zurückgezogen |
| `toast.left` | {name} disconnected | {name} hat sich getrennt | {name} hat sich getrennt |
| `toast.sessionEnded` | Session ended · new code | Sitzung beendet · neuer Code | Sitzung beendet · neuer Code |
| `toast.openDisabled` | This server only allows Approval on | Dieser Server erlaubt nur „Mit Freigabe“ | Dieser Server erlaubt nur „Mit Freigabe“ |
| `idle.appLineWeb` | No download needed. | Kein Download nötig. | Kein Download nötig. |
| `idle.offline.bodyBusy` | The server is busy right now. This screen tries again by itself. | Der Server ist gerade ausgelastet. Dieser Bildschirm versucht es von selbst noch einmal. | same as du |
| `idle.offline.bodyOutdated` | This page is out of date. Reload it to get a code. | Diese Seite ist veraltet. Lade sie neu, um einen Code zu bekommen. | Diese Seite ist veraltet. Laden Sie sie neu, um einen Code zu bekommen. |

`toast.lockedRejected` is removed (decision 13.9).

Additional setup keys (web, decision 13.5):

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `setup.address.sourcePage` | Set by the server of this page. | Vom Server dieser Seite vorgegeben. | Vom Server dieser Seite vorgegeben. |
| `setup.address.sourceTyped` | Entered by hand. | Von Hand eingegeben. | Von Hand eingegeben. |
| `setup.address.fieldLabel` | Server address | Serveradresse | Serveradresse |

### 11.5 Presenter: start, app, waiting, what to show, presenting

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `start.titleRoom` | Connect to “{room}” (not used, decision 13.2) | Mit „{room}“ verbinden | Mit „{room}“ verbinden |
| `start.titleGeneric` | Connect to a screen | Mit einem Bildschirm verbinden | Mit einem Bildschirm verbinden |
| `start.fromQr` | from the QR code | aus dem QR-Code | aus dem QR-Code |
| `start.codeLabel` | Code from the screen | Code vom Bildschirm | Code vom Bildschirm |
| `start.codeEmpty` | Please enter the code shown large on the screen. | Bitte den Code eingeben, der groß auf dem Bildschirm steht. | Bitte den Code eingeben, der groß auf dem Bildschirm steht. |
| `start.nameLabel` | Your name | Dein Name | Ihr Name |
| `start.browser` | Continue in the browser | Im Browser weiter | Im Browser weiter |
| `start.browserPhone` | No install. Show photos, files and links. | Ohne Installation. Fotos, Dateien und Links zeigen. | Ohne Installation. Fotos, Dateien und Links zeigen. |
| `start.browserLaptop` | No install. Show your screen, a window, files and links. | Ohne Installation. Bildschirm, Fenster, Dateien und Links zeigen. | Ohne Installation. Bildschirm, Fenster, Dateien und Links zeigen. |
| `start.or` | or | oder | oder |
| `start.appAndroid` | Get the app on Google Play | App bei Google Play holen | App bei Google Play holen |
| `start.appIos` | Get the app on the App Store | App im App Store holen | App im App Store holen |
| `start.appDesktop` | Download the app for {os} | App für {os} herunterladen | App für {os} herunterladen |
| `start.appPhone` | With the app you can also share your whole phone screen. | Mit der App teilst du auch deinen ganzen Handy-Bildschirm. | Mit der App teilen Sie auch Ihren ganzen Handy-Bildschirm. |
| `start.appLaptop` | The app finds screens on the same network without a code. For now the browser is enough. | Die App findet Bildschirme im selben Netz auch ohne Code. Für jetzt reicht der Browser. | Die App findet Bildschirme im selben Netz auch ohne Code. Für jetzt reicht der Browser. |
| `start.detected` | Detected: {device}, {browser} | Erkannt: {device}, {browser} | Erkannt: {device}, {browser} |
| `start.beDisplay` | Should this device be the screen itself? Open {dom}/display. | Soll dieses Gerät selbst der Bildschirm sein? {dom}/display öffnen. | Soll dieses Gerät selbst der Bildschirm sein? {dom}/display öffnen. |
| `start.downloadStarted` | Download started. Until the app is installed, everything works here in the browser. | Download gestartet. Bis die App installiert ist, geht alles hier im Browser. | Download gestartet. Bis die App installiert ist, geht alles hier im Browser. |
| `app.welcome` | Welcome to Comet Cast | Willkommen bei Comet Cast | Willkommen bei Comet Cast |
| `app.codeFromLink` | taken from your link | aus dem Link übernommen | aus dem Link übernommen |
| `app.codeScanned` | scanned | gescannt | gescannt |
| `app.scanAgainBody` | Scan the QR code on the screen once more to continue. | Scanne den QR-Code auf dem Bildschirm noch einmal, dann geht es weiter. | Scannen Sie den QR-Code auf dem Bildschirm noch einmal, dann geht es weiter. |
| `app.scanQr` | Scan QR code | QR-Code scannen | QR-Code scannen |
| `app.connect` | Connect | Verbinden | Verbinden |
| `wait.title` | Waiting for approval | Warten auf Freigabe | Warten auf Freigabe |
| `wait.checkLabel` | Your check number | Deine Prüfzahl | Ihre Prüfzahl |
| `wait.checkHint` | Also shown on the screen. | Steht auch auf dem Bildschirm. | Steht auch auf dem Bildschirm. |
| `wait.expires` | The request expires in {n} s. | Die Anfrage verfällt in {n} s. | Die Anfrage verfällt in {n} s. |
| `wait.withdraw` | Withdraw request | Anfrage zurückziehen | Anfrage zurückziehen |
| `what.connectedTo` | Connected to “{room}” | Verbunden mit „{room}“ | Verbunden mit „{room}“ |
| `what.connected` | Connected | Verbunden | Verbunden |
| `what.title` | What would you like to show? | Was möchtest du zeigen? | Was möchten Sie zeigen? |
| `what.screen` | Share screen | Bildschirm teilen | Bildschirm teilen |
| `what.screenLaptop` | Whole screen or one window, live | Ganzer Bildschirm oder ein Fenster, live | Ganzer Bildschirm oder ein Fenster, live |
| `what.screenApp` | Your whole phone display, live | Dein ganzes Handy-Display, live | Ihr ganzes Handy-Display, live |
| `what.screenPhoneBrowser` | On a phone only with the app | Geht auf dem Handy nur mit der App | Geht auf dem Handy nur mit der App |
| `what.screenPhoneBrowserNoApp` | Not possible in a phone browser | Geht im Handy-Browser nicht | Geht im Handy-Browser nicht |
| `what.file` | Show a photo or file | Foto oder Datei zeigen | Foto oder Datei zeigen |
| `what.fileSub` | PDF or image. You turn pages, everything else stays with you | PDF oder Bild. Du blätterst, der Rest bleibt bei dir | PDF oder Bild. Sie blättern, der Rest bleibt bei Ihnen |
| `what.link` | Show a link | Link zeigen | Link zeigen |
| `what.linkSub` | The screen asks before it opens the page | Der Bildschirm fragt, bevor er die Seite öffnet | Der Bildschirm fragt, bevor er die Seite öffnet |
| `what.disconnect` | Disconnect | Trennen | Trennen |
| `file.title` | What would you like to show? | Was möchtest du zeigen? | Was möchten Sie zeigen? |
| `file.body` | Recent on this device. Or “Other file” in the system dialog. (apps only) | Zuletzt auf diesem Gerät. Oder „Andere Datei“ im Systemdialog. | Zuletzt auf diesem Gerät. Oder „Andere Datei“ im Systemdialog. |
| `file.bodyWeb` | A PDF or an image from this device. It appears on the screen as soon as you pick it. | Ein PDF oder ein Bild von diesem Gerät. Es erscheint auf dem Bildschirm, sobald du es auswählst. | Ein PDF oder ein Bild von diesem Gerät. Es erscheint auf dem Bildschirm, sobald Sie es auswählen. |
| `file.pick` | Choose file | Datei auswählen | Datei auswählen |
| `link.label` | Page address | Adresse der Seite | Adresse der Seite |
| `link.hint` | The screen needs its own internet for this. | Der Bildschirm braucht dafür selbst Internet. | Der Bildschirm braucht dafür selbst Internet. |
| `link.go` | Open on the screen | Auf dem Bildschirm öffnen | Auf dem Bildschirm öffnen |
| `presenting.screen` | You are sharing your screen | Du zeigst deinen Bildschirm | Sie zeigen Ihren Bildschirm |
| `presenting.file` | You are showing a file | Du zeigst eine Datei | Sie zeigen eine Datei |
| `presenting.link` | The screen shows your link | Der Bildschirm zeigt deinen Link | Der Bildschirm zeigt Ihren Link |
| `presenting.where` | On “{room}”. | Auf „{room}“. | Auf „{room}“. |
| `presenting.stop` | Stop showing | Zeigen beenden | Zeigen beenden |
| `presenting.sas` | Security code {sas}. The screen shows the same code under your name. | Sicherheitscode {sas}. Der Bildschirm zeigt denselben Code unter deinem Namen. | Sicherheitscode {sas}. Der Bildschirm zeigt denselben Code unter Ihrem Namen. |
| `presenting.prevPage` | Previous page | Vorherige Seite | Vorherige Seite |
| `presenting.nextPage` | Next page | Nächste Seite | Nächste Seite |
| `presenting.pageOf` | Page {n} of {total} | Seite {n} von {total} | Seite {n} von {total} |
| `bye.title` | Disconnected | Getrennt | Getrennt |
| `bye.body` | You are no longer connected. You can close this tab. | Du bist nicht mehr verbunden. Du kannst diesen Tab schließen. | Sie sind nicht mehr verbunden. Sie können diesen Tab schließen. |
| `bye.again` | Connect again | Neu verbinden | Neu verbinden |

### 11.6 Presenter: errors

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `error.why` | Why | Warum | Warum |
| `error.now` | What now | Was jetzt | Was jetzt |
| `error.code.title` | This code does not exist right now | Diesen Code gibt es gerade nicht | Diesen Code gibt es gerade nicht |
| `error.code.why` | Usually a typo. A code has 6 characters and never contains O, 0, I or 1. | Meist ein Tippfehler. Ein Code hat 6 Zeichen und enthält nie O, 0, I oder 1. | Meist ein Tippfehler. Ein Code hat 6 Zeichen und enthält nie O, 0, I oder 1. |
| `error.code.now` | Look at the screen again and retype the code. | Schau noch einmal auf den Bildschirm und tippe den Code neu. | Schauen Sie noch einmal auf den Bildschirm und tippen Sie den Code neu. |
| `error.expired.title` | This code is no longer valid | Dieser Code gilt nicht mehr | Dieser Code gilt nicht mehr |
| `error.expired.why` | The screen has a new code by now. Codes change when the display device restarts or a session ends. | Der Bildschirm hat inzwischen einen neuen Code. Codes wechseln, wenn das Anzeigegerät neu startet oder eine Sitzung endet. | same as du |
| `error.expired.now` | Type the code the screen shows now, or scan the QR code again. | Den Code abtippen, der jetzt auf dem Bildschirm steht, oder den QR-Code neu scannen. | same as du |
| `error.otherServer.title` | This code belongs to another address | Dieser Code gehört zu einer anderen Adresse | Dieser Code gehört zu einer anderen Adresse |
| `error.otherServer.why` | You are on {here}. The code fits another server, for example a company’s or a club’s. | Du bist auf {here}. Der Code passt aber zu einem anderen Server, zum Beispiel dem einer Firma oder eines Vereins. | Sie sind auf {here}. Der Code passt aber zu einem anderen Server, zum Beispiel dem einer Firma oder eines Vereins. |
| `error.otherServer.now` | Look at the screen: it shows the right address. Or scan the QR code and both match. | Schau auf den Bildschirm: Dort steht die richtige Adresse. Oder scanne den QR-Code, dann stimmt beides. | Schauen Sie auf den Bildschirm: Dort steht die richtige Adresse. Oder scannen Sie den QR-Code, dann stimmt beides. |
| `error.noInternet.title` | No internet on this device | Kein Internet auf diesem Gerät | Kein Internet auf diesem Gerät |
| `error.noInternet.why` | Your device is offline, or the Wi-Fi only allows certain sites. | Dein Gerät ist offline, oder das WLAN lässt nur bestimmte Seiten zu. | Ihr Gerät ist offline, oder das WLAN lässt nur bestimmte Seiten zu. |
| `error.noInternet.now` | Turn on mobile data or join the room’s Wi-Fi. The page then retries by itself. | Mobile Daten einschalten oder das WLAN des Raums wählen. Die Seite versucht es dann von selbst noch einmal. | same as du |
| `error.noInternet.nowBox` | For {dom} you must be on the same Wi-Fi as the screen. Ask for the room’s Wi-Fi. | Für {dom} musst du im selben WLAN sein wie der Bildschirm. Frag nach dem WLAN des Raums. | Für {dom} müssen Sie im selben WLAN sein wie der Bildschirm. Fragen Sie nach dem WLAN des Raums. |
| `error.noScreen.title` | Screen sharing does not work in this browser | Bildschirm teilen geht in diesem Browser nicht | Bildschirm teilen geht in diesem Browser nicht |
| `error.noScreen.why` | Phone browsers may not pass on your screen. That is the browser, not you. | Handy-Browser dürfen deinen Bildschirm nicht weitergeben. Das liegt am Browser, nicht an dir. | Handy-Browser dürfen Ihren Bildschirm nicht weitergeben. Das liegt am Browser, nicht an Ihnen. |
| `error.noScreen.nowNoApp` | Show a photo, a file or a link instead. | Zeig stattdessen ein Foto, eine Datei oder einen Link. | Zeigen Sie stattdessen ein Foto, eine Datei oder einen Link. |
| `error.noScreen.now` | Show a photo, a file or a link instead. Or get the app, then the screen works too. | Zeig stattdessen ein Foto, eine Datei oder einen Link. Oder hol dir die App, dann geht auch der Bildschirm. | Zeigen Sie stattdessen ein Foto, eine Datei oder einen Link. Oder holen Sie sich die App, dann geht auch der Bildschirm. |
| `error.displayOff.title` | The screen does not answer | Der Bildschirm antwortet nicht | Der Bildschirm antwortet nicht |
| `error.displayOff.why` | The display device is off, asleep, or Comet Cast is closed there. Your request did not arrive. | Das Anzeigegerät ist aus, schläft oder Comet Cast ist dort geschlossen. Deine Anfrage ist nicht angekommen. | Das Anzeigegerät ist aus, schläft oder Comet Cast ist dort geschlossen. Ihre Anfrage ist nicht angekommen. |
| `error.displayOff.whyRelay` | The two devices could not reach each other, and the server could not help this time. | Die beiden Geräte haben sich nicht erreicht, und der Server konnte diesmal nicht helfen. | same as du |
| `error.displayOff.now` | Is the code still on the screen? If not, ask the host to open Comet Cast there. | Steht der Code noch auf dem Bildschirm? Wenn nicht, den Gastgeber bitten, Comet Cast dort zu öffnen. | same as du |
| `error.denied.title` | Not allowed | Nicht zugelassen | Nicht zugelassen |
| `error.denied.why` | “No” was chosen on the screen. | Am Bildschirm wurde „Nein“ gewählt. | Am Bildschirm wurde „Nein“ gewählt. |
| `error.denied.now` | Ask in person if you need to. | Frag bei Bedarf kurz persönlich nach. | Fragen Sie bei Bedarf kurz persönlich nach. |
| `error.locked.title` | Locked right now | Gerade gesperrt | Gerade gesperrt |
| `error.locked.why` | The code is right, but this screen is not taking anyone new right now. | Der Code stimmt, aber dieser Bildschirm nimmt gerade niemanden Neues an. | same as du |
| `error.locked.whyWhile` | The host locked the room while you were waiting. Your request was dropped. | Der Gastgeber hat gesperrt, während du gewartet hast. Deine Anfrage wurde verworfen. | Der Gastgeber hat gesperrt, während Sie gewartet haben. Ihre Anfrage wurde verworfen. |
| `error.locked.now` | Try again later or talk to the host. | Später noch einmal versuchen oder den Gastgeber ansprechen. | same as du |
| `error.timeout.title` | No answer | Keine Antwort | Keine Antwort |
| `error.timeout.why` | Nobody answered your request in time. | Niemand hat deine Anfrage rechtzeitig beantwortet. | Niemand hat Ihre Anfrage rechtzeitig beantwortet. |
| `error.timeout.now` | Ask again and let the host know. | Noch einmal anfragen und dem Gastgeber Bescheid sagen. | same as du |
| `error.ended.title` | Presentation ended | Präsentation beendet | Präsentation beendet |
| `error.ended.why` | The session was ended on the screen. | Am Bildschirm wurde die Sitzung beendet. | Am Bildschirm wurde die Sitzung beendet. |
| `error.ended.now` | To continue, enter the new code or scan the QR code again. | Zum Weitermachen den neuen Code eingeben oder den QR-Code neu scannen. | same as du |
| `error.action.retype` | Retype code | Code neu eingeben | Code neu eingeben |
| `error.action.rescan` | Scan QR code again | QR-Code neu scannen | QR-Code neu scannen |
| `error.action.retry` | Try again | Erneut versuchen | Erneut versuchen |
| `error.action.showFile` | Show photo or file | Foto oder Datei zeigen | Foto oder Datei zeigen |
| `error.action.showLink` | Show a link | Link zeigen | Link zeigen |
| `error.action.getApp` | Get the app | App holen | App holen |
| `error.action.ask` | Ask again | Erneut anfragen | Erneut anfragen |
| `error.action.askIn` | Ask again in {n} s | Erneut anfragen in {n} s | Erneut anfragen in {n} s |
| `error.action.openDom` | Open {dom} | {dom} öffnen | {dom} öffnen |
| `error.action.reload` | Reload page | Seite neu laden | Seite neu laden |
| `error.occupied.title` | Someone else is connected | Jemand anderes ist verbunden | Jemand anderes ist verbunden |
| `error.occupied.why` | This screen takes one person at a time. | Dieser Bildschirm nimmt immer nur eine Person an. | same as du |
| `error.occupied.now` | Ask that person to disconnect, or ask the host to press End. Then try again. | Die Person bitten, sich zu trennen, oder den Gastgeber, „Beenden“ zu drücken. Dann noch einmal versuchen. | same as du |
| `error.busy.title` | The server is busy | Der Server ist ausgelastet | Der Server ist ausgelastet |
| `error.busy.why` | Too many attempts from this network, or too many people at once. | Zu viele Versuche aus diesem Netz oder zu viele Leute gleichzeitig. | same as du |
| `error.busy.now` | Wait a minute, then try again. | Eine Minute warten, dann noch einmal versuchen. | same as du |
| `error.outdated.title` | This page is out of date | Diese Seite ist veraltet | Diese Seite ist veraltet |
| `error.outdated.why` | The server was updated in the meantime. | Der Server wurde inzwischen aktualisiert. | same as du |
| `error.outdated.now` | Reload the page. | Die Seite neu laden. | same as du |
| `error.other.title` | Something went wrong | Etwas hat nicht geklappt | Etwas hat nicht geklappt |
| `error.other.why` | (the existing text `errors.<code>`, 11.8) | | |
| `error.other.now` | Try again. If it happens again, tell the host. | Versuch es noch einmal. Wenn es wieder passiert, sag dem Gastgeber Bescheid. | Versuchen Sie es noch einmal. Wenn es wieder passiert, sagen Sie dem Gastgeber Bescheid. |

"same as du" means the Sie value is identical to the du value; it still exists as its own entry in the Sie file.

Note on `error.otherServer.now`: the mockup said "…the right address, here {dom}". This spec drops "here {dom}", because the presenter page cannot know the display's address when the code belongs to another server. Show the "Open {dom}" button only when the code's server is known (for example from a QR link); otherwise show only "Retype code".

### 11.7 Copy of items the mockup did not draw

These keys already exist in the web app with these values (EN and DE Sie); they are now part of the spec. The DE du column is required (see 11.8 for the rest).

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `relay.confirm.title` | Use this relay? | Dieses Relay verwenden? | Dieses Relay verwenden? |
| `relay.confirm.body` | A link asks this page to use the relay {host} instead of the default. Only continue if you know and trust its operator. | Ein Link fordert diese Seite auf, das Relay {host} statt des Standards zu verwenden. Mach nur weiter, wenn du den Betreiber kennst und ihm vertraust. | Ein Link fordert diese Seite auf, das Relay {host} statt des Standards zu verwenden. Fahren Sie nur fort, wenn Sie den Betreiber kennen und ihm vertrauen. |
| `relay.confirm.risk` | The operator of this relay can see who connects, your room codes and network addresses, and can redirect the connection. Nothing is connected and no screen is captured before you decide. | Der Betreiber dieses Relays kann sehen, wer sich verbindet, deine Raumcodes und Netzwerkadressen, und kann die Verbindung umleiten. Es wird nichts verbunden und kein Bildschirm erfasst, bevor du entschieden hast. | Der Betreiber dieses Relays kann sehen, wer sich verbindet, Ihre Raumcodes und Netzwerkadressen, und kann die Verbindung umleiten. Es wird nichts verbunden und kein Bildschirm erfasst, bevor Sie entschieden haben. |
| `relay.confirm.use` | Use this relay | Dieses Relay verwenden | Dieses Relay verwenden |
| `relay.confirm.cancel` | Cancel (use the default relay) | Abbrechen (Standard-Relay verwenden) | Abbrechen (Standard-Relay verwenden) |
| `receive.link.title` | {host} wants to open a link | {host} möchte einen Link öffnen | {host} möchte einen Link öffnen |
| `receive.link.hint` | The sender asks you to open this link. Open it only if you expect it. It opens in a new tab, and only if you click “Open”. | Der Absender bittet dich, diesen Link zu öffnen. Öffne ihn nur, wenn du ihn erwartest. Er öffnet sich in einem neuen Tab, und nur wenn du auf „Öffnen“ klickst. | Der Absender bittet Sie, diesen Link zu öffnen. Öffnen Sie ihn nur, wenn Sie ihn erwarten. Er öffnet sich in einem neuen Tab, und nur wenn Sie auf „Öffnen“ klicken. |
| `receive.link.hintTv` (new) | This screen cannot open web pages. Open the address on your own device if you need it. | Dieser Bildschirm kann keine Webseiten öffnen. Öffne die Adresse bei Bedarf auf deinem eigenen Gerät. | Dieser Bildschirm kann keine Webseiten öffnen. Öffnen Sie die Adresse bei Bedarf auf Ihrem eigenen Gerät. |
| `receive.link.open` | Open | Öffnen | Öffnen |
| `receive.link.dismiss` | Dismiss | Verwerfen | Verwerfen |
| `receive.media.play` | Play from {host} | Von {host} abspielen | Von {host} abspielen |
| `receive.media.stop` | Stop | Stopp | Stopp |
| `receive.media.from` | Playing from {host} | Wiedergabe von {host} | Wiedergabe von {host} |
| `receive.transfer` | Receiving {name}… {percent} % | {name} wird empfangen … {percent} % | {name} wird empfangen … {percent} % |
| `toolbar.label` | Screen tools | Bildschirm-Werkzeuge | Bildschirm-Werkzeuge |
| `toolbar.pen` · `.marker` · `.eraser` | Pen · Highlighter · Eraser | Stift · Textmarker · Radierer | same as du |
| `toolbar.color.*` | Red · Yellow · Green · Blue · White | Rot · Gelb · Grün · Blau · Weiß | same as du |
| `toolbar.size.*` | Thin · Medium · Thick | Dünn · Mittel · Dick | same as du |
| `toolbar.undo` · `.clear` · `.laser` | Undo · Clear all annotations · Laser pointer | Rückgängig · Alle Markierungen löschen · Laserpointer | same as du |
| `toolbar.freeze` | Freeze image (the live stream keeps running) | Bild einfrieren (der Live-Stream läuft weiter) | same as du |
| `toolbar.black` · `.fullscreen` · `.exitFullscreen` | Black screen · Full screen · Exit full screen | Schwarzer Bildschirm · Vollbild · Vollbild beenden | same as du |
| `toolbar.frozenHint` | Frozen. The live stream keeps running in the background. | Eingefroren. Der Live-Stream läuft im Hintergrund weiter. | same as du |
| `toolbar.blackHint` | Screen hidden | Bildschirm ausgeblendet | same as du |
| `app.beDisplay` (new, native apps on tablets and computers, see native-apps.md) | Use this device as the screen | Dieses Gerät als Bildschirm verwenden | Dieses Gerät als Bildschirm verwenden |

The other `receive.media.*`, `receive.transferError.*`, `receive.document.pageError.*` and `relay.errors.*` values stay as they are in `apps/web/src/locales/en.json` and `de.json`; their du forms are in 11.8 where the Sie form addresses the reader.

### 11.8 Missing "du" values (decision 13.47)

The du file must contain **every** key whose Sie value addresses the reader (Sie, Ihr, Ihnen …); otherwise the du set silently shows Sie text. These 42 keys were missing in `de-du.json`. Values:

| Key | DE du |
| --- | --- |
| `relay.errors.empty` | Bitte gib eine Adresse ein. |
| `relay.errors.insecure` | Unverschlüsseltes ws:// ist nur für localhost und Adressen im lokalen Netzwerk erlaubt. Bitte verwende wss://. |
| `relay.errors.mixed` | Eine über https geöffnete Seite kann kein unverschlüsseltes ws://-Relay verwenden (der Browser blockiert das). Bitte verwende wss://. |
| `relay.confirm.body`, `relay.confirm.risk`, `receive.link.hint` | see 11.7 |
| `receive.status.checking` | Dein Browser wird geprüft … |
| `receive.status.peer_left` | Der Absender hat die Verbindung verlassen. Du kannst diesen Code behalten und auf einen neuen Absender warten. |
| `receive.media.hint` | Dieser Link verweist auf eine Mediendatei. Sie wird von {host} nur geladen, wenn du auf „Abspielen“ klickst. |
| `receive.media.hlsUnsupported` | Dies sieht nach einem HLS-Stream aus, den dieser Browser nicht abspielen kann. Du kannst den Link trotzdem öffnen. |
| `receive.media.error.unsupported` | Diese Datei kann hier nicht abgespielt werden: Das Format wird nicht unterstützt, die Datei fehlt oder der Medienserver erlaubt die Wiedergabe von anderen Websites nicht. Mit „Öffnen“ siehst du sie in einem neuen Tab. ({host}) |
| `receive.media.error.blocked` | Der Browser hat die automatische Wiedergabe blockiert. Klick auf „Erneut versuchen“, um sie zu starten. |
| `receive.media.mixed` | Diese Adresse verwendet http://, diese Seite aber https://, daher würde der Browser die Wiedergabe hier blockieren. Du kannst den Link trotzdem öffnen. |
| `receive.media.localWarning` | Achtung: Diese Adresse zeigt auf ein Gerät oder einen Namen innerhalb eines Netzwerks (eine IP-Adresse oder ein lokaler Name). Spiel sie nur ab, wenn du das erwartest. |
| `send.status.idle`, `sendContent.status.idle` | Gib den Code des Empfängers ein, um zu beginnen. |
| `send.status.capturing` | Wähle im Browser-Dialog aus, was geteilt werden soll … |
| `send.status.sharing` | Dein Bildschirm wird geteilt. |
| `sendContent.intro` | Gib den Raumcode des Empfängers ein und sende dann ein Bild, ein PDF oder einen Link. Die Inhalte gehen direkt zum Empfänger; der Server hilft nur beim Verbinden. |
| `sendContent.status.sharing` | Verbunden. Du kannst jetzt senden. |
| `sendContent.image.drop` | Zieh ein Bild hierher, oder |
| `sendContent.document.drop` | Zieh ein PDF hierher, oder |
| `sendContent.link.invalid` | Bitte gib eine vollständige Webadresse ein, die mit http:// oder https:// beginnt. |
| `errors.bad_hello` | Der Server hat den Verbindungsaufbau nicht akzeptiert. Bitte lade die Seite neu. |
| `errors.proto_mismatch` | Diese Seite und der Server verwenden unterschiedliche Protokollversionen. Bitte lade die Seite neu. |
| `errors.bad_message` | Der Server konnte eine Nachricht nicht verstehen. Bitte lade die Seite neu. |
| `errors.room_not_found` | Es gibt keinen Raum mit diesem Code. Prüf den Code auf dem Empfänger. |
| `errors.altcha_required` | Der Server verlangt eine kurze Browserprüfung, bevor er einen Raum anlegt. Bitte versuch es erneut. |
| `errors.altcha_invalid` | Die Browserprüfung wurde nicht akzeptiert (möglicherweise ist sie abgelaufen). Bitte versuch es erneut. |
| `errors.altcha_failed` | Die Browserprüfung konnte nicht abgeschlossen werden. Bitte versuch es erneut. |
| `errors.altcha_too_hard` | Der Server verlangt eine Browserprüfung, die für dieses Gerät zu aufwendig ist, und sie wurde nicht gestartet. Bitte versuch es später erneut oder wende dich an den Betreiber des Servers. |
| `errors.altcha_expired` | Die Browserprüfung des Servers war bereits abgelaufen. Bitte versuch es erneut; falls das wiederholt passiert, prüf Datum und Uhrzeit dieses Geräts. |
| `errors.rate_limited` | Zu viele Anfragen aus diesem Netzwerk. Bitte warte einen Moment und versuch es erneut. |
| `errors.banned` | Dieses Netzwerk ist wegen zu vieler fehlgeschlagener Versuche für kurze Zeit gesperrt. Bitte versuch es später erneut. |
| `errors.server_full` | Der Server ist gerade ausgelastet. Bitte versuch es später erneut. |
| `errors.join_pending` | Bitte warte, bis der Gastgeber deinen Beitritt erlaubt hat. |
| `errors.join_denied` | Der Gastgeber hat deinen Beitritt nicht erlaubt. |
| `errors.room_locked` | Der Raumcode ist richtig, aber der Raum ist gerade gesperrt. Bitte den Gastgeber, ihn zu entsperren. |
| `errors.room_locked_waiting` | Der Raum wurde gesperrt, während du gewartet hast. Der Code war richtig; bitte den Gastgeber, den Raum zu entsperren. |
| `errors.approval_timeout` | Niemand hat deine Anfrage rechtzeitig beantwortet. Du kannst erneut anfragen. |
| `errors.removed_by_host` | Der Gastgeber hat dich aus dem Raum entfernt. |
| `errors.block_list_full` | Die Blockliste ist voll. Gib zuerst jemanden frei. |

`tools/check-i18n.mjs` gets a rule: a key whose `de` value contains the word Sie, Ihr/Ihre/Ihren/Ihrem/Ihrer/Ihres or Ihnen must exist in `de-du` (the "Sie wird" of `receive.media.hint` means "it" and is the one known false alarm; list it as an exception).

## 12. Open points that do not block implementation

- The owner's question "du" or "Sie" (D6). Ship Sie; the du set is ready.
- The Android install referrer passing code and address to the app is an assumption; if it does not work, Android uses the iOS behaviour (scan again).
- Exact idle-dim delay: 7 minutes is inside the owner's 5–10 minute range; adjust in one constant if testing suggests otherwise.
- Owner: should the web display keep its drawing tools (annotation toolbar) at all? This spec keeps them, hidden at rest (13.7); removing them would make the display match "only End" exactly.
- Owner: store listings and the desktop download. Until they exist the app offers are hidden (13.6).
- Owner: `start.appLaptop` promises that the desktop app "finds screens on the same network without a code". The list of nearby screens is **not designed** yet (native-apps.md, section 9). Before `DOWNLOAD_URL_DESKTOP` is filled, either that list is designed and built, or the line changes. The line is hidden today (no desktop link).
- Owner: whether native display apps on tablets and computers get the annotation toolbar (they do not in this round; same question as the web toolbar above).

## 13. Decisions after implementation review

Made on 2026-11-07 after reviewing the built web app (`apps/web`, merge fd03220) against the mockup and its screenshots. The questions are in [implementation-questions.md](implementation-questions.md) (A1 … C), the work items in [fix-list-web.md](fix-list-web.md). The sections above already contain these decisions.

1. **Code format (A1).** The protocol's code is the only format: 6 characters from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, shown `XXX XXX`, on every server. The field never drops typed characters; a code with other characters or another length goes to `error.code` without a server round trip; `error.code.why` says codes contain no O, 0, I or 1. `error.otherServer` is not used. No protocol change.
2. **Room name reaches the presenter (A2). Protocol requirement:** `create_room` gets an optional `room_name` (string); the server cleans it like `name` (removes control, bidi, zero-width, invisible and private-use characters, collapses white space), cuts it at **32 characters**, and drops it when empty. `join_pending` and `joined` carry the cleaned value as optional `room_name`; it is absent when the display set none. Additive and optional: every client must work without it. It is not put into the QR URL. Presenter pages use it in the waiting label, `what.connectedTo` and `presenting.where`; without it they show no room (`what.connected`, no `presenting.where` line, spinner only) and **never the code** in its place. `start.titleRoom` is not used (the start page is before the join).
3. **No device type (A3).** The texts drop it: `request.subtitle` = "wants to connect", `present.byline` = "{name} is showing". No protocol change. `device.*` stays for `start.detected`.
4. **In-house box line (A4).** `idle.altAddress` is native-only; the web display never shows it.
5. **Wizard address on the web (A4).** Source lines `setup.address.sourceDefault` / new `sourcePage` / new `sourceTyped`; `sourceOwn` and `sourceBox` are native-only; the typed field has the label `setup.address.fieldLabel` (4.1).
6. **`{dom}/app` and store links (A5, B10).** `/app` is the start page. App offers (store buttons, desktop download, the "or" divider, their small print, `start.downloadStarted`) are shown only for platforms whose link exists; the idle footer then says `idle.appLineWeb` only. `start.downloadStarted` is small print, not a green box.
7. **Presenting stays quiet (B1).** The annotation toolbar is hidden at rest and fades in and out together with name tag and End; it moves to display tokens (3.5), loses grip and collapse, and the toast never covers it. "Close image", "Close document" and the display's PDF page buttons are removed; the frozen badge and black-screen hint stay as states.
8. **Links (B1).** The display never opens a page by itself; the link card is a themed dialog with Open / Dismiss, 1 s guard. `what.linkSub` = "The screen asks before it opens the page".
9. **Settings and requests (B2).** A join request closes Settings; one dialog at a time. `toast.lockedRejected` is removed; `error.locked.whyWhile` stays for a server that drops waiters with reason `locked`.
10. **What is remembered (B3).** As built: appearance and Open / Approval on are stored (new default); Locked never, and it ends with End.
11. **Request expiry (B4).** As built: the server's timer is the only one; `error.timeout.why` no longer names 60 seconds.
12. **Undesigned display states (B5).** "Idle, connecting" (4), offline box only after 3 s, empty QR box while offline or connecting, body variants `bodyBusy` / `bodyOutdated`, toast `toast.openDisabled`. Content errors stay toasts; transfer progress shows only while presenting; dimming covers only Idle (all as built).
13. **Security code (B6).** It is called "Security code" / "Sicherheitscode" (not "check", which is the two-digit number). Display: second line of the name tag (visible only with the controls). Presenter: one line of small print on the "Presenting" page only (`presenting.sas`); the long explanation (`sas.explain`) and the line on "What to show" are removed.
14. **PDF pages (B7).** Turned on the presenter's "Presenting" page: Previous / Next in a row above Stop, optional "Page n of m".
15. **File page in a browser (B8).** One primary `file.pick` with `file.bodyWeb`; the thumbnail grid is for the apps.
16. **Replacing content (B9).** As built: Stop, then pick again.
17. **Rescan (B11).** As built: only in the apps.
18. **Other failures (B12).** New error pages `error.occupied` (`room_full`), `error.busy` (rate limit, ban, full server), `error.outdated` (protocol mismatch, with "Reload page"); mapping in 6.
19. **Presenter colour tokens (B13).** Dark icon discs = 15 % tints, now as hex tokens; `p-disabled` changed so its text is readable (the light disabled button had white text at 1.8:1).
20. **Other existing behaviour (B14).** Screen-reader status line, no language switch, removal of the relay field and fullscreen on first click: accepted. The relay confirmation dialog must be themed (7.2).
21. **Key name (B15).** `settings.join.label` (ARB `settingsJoinLabel`).
22. **Static hosting (C).** `/display`, `/{CODE}`, `/app` must serve `index.html` on every host (7.2).
23. **OLED black.** No amber tint on selected tiles; the check-number box gets a `--line` border.
24. **Focus ring on load.** Accepted: shown on load, hidden by the first pointer input (7.2).
25. **Laptop presenter pages.** Real pages without device frames (accepted); the action row follows the content instead of sitting at the window bottom (7.4).
26. **Portrait display.** The centred 16:9 canvas on a phone is accepted (3.1); no portrait layout.
27. **Fonts.** Bundled subsets accepted (3.4).
28. **QR version.** Any version at level M is fine; the box size does not change.
29. **Wizard room name.** The custom name field starts empty; a chip and the field are never both set.

### Decisions after the second implementation review

Made on 2026-11-07 after reviewing the fix round (`apps/web`, main 4a13254) and its 183 screenshots. Questions D1 and E1–E14 are answered in [implementation-questions.md](implementation-questions.md); the work items are in [fix-list-web-2.md](fix-list-web-2.md). The sections above already contain these decisions.

30. **Reconnect schedule (E1).** After a loss the display tries again at 1 s, 2 s and 3 s, then every 4 s; the offline box appears when the attempt at 3 s fails, so a drop under 3 s never shows it (4.2). Busy refusals: box at once, retry after 30 s.
31. **PDF page on the presenter (E2). Protocol requirement:** the display reports `page` and `pages` to the presenter over the content control channel. With it: "Page n of m" and Previous/Next disabled at the ends; without it (today): no line, both enabled (5.6).
32. **Other failures (E3).** New page `error.other` (heading "Something went wrong", why = the existing `errors.<code>` text, now = `error.other.now`, Try again) for every code without its own page. `error.code` only for wrong or unknown codes; `capture_denied` never shows a page (6).
33. **QR box.** Light: 1 px `--line` border. Empty box (connecting, offline): `--panel2` fill, `--line` border, full opacity, no modules, no caption; OLED black `#000000` (3.2). The light-grey 25 % slab is gone.
34. **Media link card (E5).** As built (Play secondary before Open, initial focus Open, player up to 300 high), plus: warning and error lines 16/700 `--fg` led by the `info` icon in `--acc` (4.5).
35. **Toolbar is the one icon-only control set (E12, section 9).** Tooltips and accessible names on every button; its own Tab focus outline (2 `--focus`, offset 2); swatches with a 1 px `--line` ring; selected tool, swatch and size with a 2 px `--acc` border; hidden = `inert`; toast at 66 while it shows (3.5).
36. **Everything on the canvas.** Toasts, dialogs, toolbar, name tag, End, badges and pills are placed on the scaled 16:9 canvas, never on the viewport; only the picture fills the viewport (3.5).
37. **Relay confirmation, redesigned (E4).** Shown before anything else: on `/display` a 600 dialog card on the canvas over plain `--bg` with the logo only; on presenter pages its own page in the error-page layout. The unconfirmed host appears nowhere else. Order [Cancel, Use this relay], Cancel focused (7.2).
38. **Wizard address field (E6).** As built: [Back, Looks right] while the field is open; Back closes the field (focus to "Other address"); after that "Other address" opens the field again (4.1).
39. **Wizard room name (E7).** An emptied field reselects the chip that was selected before typing (4.1).
40. **Default relay on the web (E8).** As built: `sourceDefault` only for the relay built into the page; everything the page's server provides says `sourcePage` (4.1).
41. **Content states (E13).** PDF pages on black; transfer progress as a name-tag-look pill; content errors as toasts; page errors in a name-tag-look box; black screen, frozen picture and strokes end with the presentation (4.5).
42. **No app, no promise (E9).** "Get the app" only when an app can be offered (as built); then the texts change too: `error.noScreen.nowNoApp`, `what.screenPhoneBrowserNoApp` (5.5).
43. **Laptop presenter pages.** Top padding `max(40px, 10vh)`; the content no longer hugs the window's top edge (3.5).
44. **Banned, then closed (E14).** For 60 s after a `rate_limited`, `banned` or `server_full` refusal, a closed connection is treated as the same refusal: presenter `error.busy`, display `bodyBusy` (4.2, 6).
45. **STUN/TURN list refused (D1).** Show nothing at once; only when the connection then fails: refused list (`rate_limited`, `refused`) → `error.busy`; failed list (`http_error`, `bad_answer`, `unreachable`) → `error.displayOff` with `error.displayOff.whyRelay`. The display shows nothing (the presenter tells the person). Sessions expose the fetch state next to `iceServers` (6).
46. **Wizard card position.** Steps 1–4: the card's top edge is fixed at 64; only the welcome card is centred (4.1).
47. **"du" set complete.** 42 missing du values are given in 11.8; the i18n check enforces it.
48. **Presenter relay page order (E4).** Cancel (secondary) before Use this relay (primary), on the phone too; the only exception to "primary first".
49. **`room_name` of the wrong type (E10).** The lenient rule is right: a missing, null, empty, blank or non-string `room_name` from the server means "no room name"; the rest of the message stays valid. The field is display text, not a security property (unlike `check`).
50. **Which input hides the focus ring (E11).** As built: a pointer press, a touch start or a real mouse movement hides it; any key shows it.
51. **Text fields get the focus ring** without the scale (3.5).
52. **Native apps.** [native-apps.md](native-apps.md) maps every screen and state of this spec to Android (phone, tablet, TV), iOS/iPadOS, Linux/Windows/macOS (Flutter) and Apple TV (SwiftUI), with token, font, icon, motion, dimming and wake-lock rules. It adds no screens; where a platform needs a variant (link card without a browser, the `app.beDisplay` entry on tablets and computers, system scanners and pickers), it is named there and here.
53. **Spec over mockup, now for everything.** The mockup is not redrawn for the items it never had (toolbar, link card, relay confirmation, security code, the later error pages, the PDF page row, the web file page, connecting and offline states, wizard address field, `start.beDisplay` link, OLED check outline). Their design is the section of this document named in the status note; the mockup's step cards, old headline, mode instruction lines, room menu, hold-to-allow, 8-character code and device frames stay superseded.
