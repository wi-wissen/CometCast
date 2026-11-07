# First run and display: implementation spec

Status: **final, approved for implementation; revised after the first, second and third implementation review of the web app (2026-11-07).** The owner approved round 4 with two changes, which are applied here and in the final mockup. The review decisions are listed in [section 13](#13-decisions-after-implementation-review) (1–29 first review, 30–53 second review, 57–69 third review) and are already worked into the sections below; the work items for the web app are in [fix-list-web.md](fix-list-web.md) (done) and [fix-list-web-2.md](fix-list-web-2.md) (done) and [fix-list-web-3.md](fix-list-web-3.md) (open, the last design work items for the web display, decision 69), the answered questions in [implementation-questions.md](implementation-questions.md). How the native apps (Flutter, SwiftUI on Apple TV) build the same product is in [native-apps.md](native-apps.md). Build from this document; do not wait for further design review. **Addendum (2026-11-07):** [section 14](#14-screens-on-the-same-network-apps-only) adds the list of screens on the same network to the app start page, and German now ships in the "du" form (decision 54). **Flutter review (2026-11-07):** decisions N1–N28 in section 13, new sections 4.7 and 5.8 (video codec consent), copy 11.9–11.10; work items in [fix-list-flutter.md](fix-list-flutter.md). **Second Flutter review (2026-11-07):** decisions N29–N47 in section 13 ("Decisions after the second Flutter review"); work items in [fix-list-flutter-2.md](fix-list-flutter-2.md).

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
| D2 | Join mode and appearance live in **Settings** on the display. Nothing else is in Settings. **Amended (decision N91):** on desktop display apps also the "Without the app" section (AirPlay, Miracast), and on every display an About button in the header (15.4, 15.9). |
| D3 | **Dark** is the default appearance. |
| D4 | **OLED black** means true black (`#000000`) for the background and all surfaces, with **no pixel drift** or position shifting. |
| D5 | In the idle state the display **dims after about 5 to 10 minutes** without activity (this spec: 7 minutes, see 8.6). The system screensaver and sleep are blocked **only while someone is presenting**. The screen is kept awake during an active presentation and released when the display returns to idle or the session ends. |
| D6 | German UI wording is **"du"** (owner decision, 2026-11-07; decision 54). Every German string below is given in both forms; the "Sie" set stays complete, so a switch back is still one file swap (see 11.1). |

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
| `{dom}` | The address of the server the display uses, e.g. `cometcast.de/app`, `cast.musterfirma.de`, `cast-box.local`. It is the **join address**, where a browser finds the web app: for the built-in relay `relay.cometcast.de` it is `cometcast.de/app` (decision N85, 15.1). Host name only (plus that `/app`); `:port` is added only when the port is not 443 (decision N12), on the idle screen, in the wizard, in `idle.offline.title` and in the QR URL alike. |

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
| `--onblack` | `#A1A9BA` | `#A1A9BA` | `#A1A9BA` | Text drawn directly on the black picture area (`toolbar.blackHint`); the picture area is black in every theme, so this value does not follow the theme (decision 13.57) |

Derived values: selected tile fill = `color-mix(in srgb, var(--acc) 14%, var(--panel2))` in Dark and Light; **in OLED black the selected tile fill stays `#000000`** (selection = `--acc` border + bold text only); name tag, End, annotation toolbar, frozen badge and transfer progress pill fill = `color-mix(in srgb, var(--panel) 92%, transparent)`; focus glow = `0 0 0 9px rgba(255,201,77,.18)`. Text on an accent fill uses `--bg`.

QR box (decision 13.33): with a code it has the `--qrbg` fill; in **Light** it also has a 1 px `--line` border (white on white has no edge otherwise). **Empty QR box** (connecting, offline): no modules, no caption, fill `--panel2`, 1 px `--line` border, **full opacity** (in OLED black: `#000000` fill + border). The Locked QR (modules present) keeps the 25 % opacity; the 25 % applies to the code (quiet zone and modules) only, the Light theme's 1 px `--line` border stays at full opacity (decision 13.67).

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
| Frozen badge | top 12, left 12; same look as the name tag, 15/700. **Hidden while the annotation toolbar is visible** (the toolbar would cover it); then the Freeze button's selected border shows the state (decision 13.65) |
| Transfer progress pill | bottom 12, centred; name-tag look (fill, 2 px `--line`, radius 10, min height 46, padding 6 14), text 15/700 `--fg` (`receive.transfer`, then `receive.document.loading` while a received PDF is opened, in the same place until its first page is drawn); shown only while presenting and while a transfer or opening runs, at rest too (it is a state). **One line, max width 340** (it then clears the widest name tag and End); a long file name is shortened in the middle with `…`, keeping its start and its extension; the percentage is never cut (decision 13.63) |
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

A card (780 ref px wide) centred horizontally on `--bg`. The welcome card (step 0) is centred vertically; for steps 1–4 the card's **top edge is fixed at 40** so the logo and the step dots do not jump between steps (decisions 13.46, 13.64); every step keeps at least 24 below the card in EN and DE, also when the name chips wrap onto two lines. Top row: logo (40) left; step dots (4 dots, 10 px, done = `--acc`, todo = `--line`) and "Step n of 4" right, hidden on the welcome screen.

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
- The two choice cards of step 2 use the selected-tile look of Settings: selected = 2 px `--acc` border + selected tile fill (3.2; OLED black: no fill); not selected = 2 px `--line` border, `--panel2` fill. Focus adds the focus ring on top, so a focused unselected card never looks selected (decision 13.66).
- Locked is not offered as a default; it only exists during a session.
- The address is detected, not typed: DNS entry of the company network → own server; in-house box found on the network → box; otherwise the default server. "Other address" cycles through the found options (if only one exists, it opens a text field for an address).
- **Web (`/display`)**: a web page cannot see DNS entries or boxes. The card shows the relay host the page uses. Source line: `setup.address.sourceDefault` for the default relay, `setup.address.sourcePage` for any relay that comes from the page's own server or its `config.json`, `setup.address.sourceTyped` for an address typed into the field. `setup.address.sourceOwn` and `setup.address.sourceBox` are used by the native apps only. "Other address" cycles through the relays of `config.json`, then opens the field; the field replaces the address card content in place: label `setup.address.fieldLabel`, the same input style as the room name field, OK/Enter confirms (back to the card with `sourceTyped`), Back cancels. With the field open the button row is [Back, Looks right]: **Looks right** confirms like Enter, **Back** (and Escape) closes the field and stays on step 3 with focus on "Other address", "Other address" is not drawn. After the field was used or cancelled, "Other address" opens the field again; the relays of `config.json` are reached again by going one step back and forward (decision 13.38). An invalid address shows the existing `relay.errors.*` line under the field (16, `--fg`, led by the `info` icon 20 in `--acc`, as the warning lines of 4.5; no red).
- Which source line (native apps, decision N13): DNS entry → `sourceOwn`, in-house box → `sourceBox`, an administrator's policy → `sourcePolicy` ("Other address" is hidden when the policy locks the server), an address typed into the field → `sourceTyped`, otherwise `sourceDefault`.
- Which source line (web, decision 13.40): `sourceDefault` only for the relay built into the page (`VITE_DEFAULT_RELAY`); the page's own server, a `config.json` relay (also when reached with "Other address") and a confirmed `?server=` relay say `sourcePage`; an address typed into the field says `sourceTyped`.
- Back on the remote goes one step back; on step 0 it does nothing.
- Finish stores room name, default join mode and server, then shows Idle with the chosen mode.

### 4.2 Idle screen

Layout top to bottom (ref px, 960 × 540):

1. **Top row**: logo 46 × 46, room name (30/700), mode pill at the right (icon in `--acc` + word: Open / Approval on / Locked).
2. **Main row** (flex, fills height, centred vertically):
   - Left column: step 1 — number disc "1", label `idle.step1` ("Open in your browser"), address `{dom}` (58/700). For the in-house box (native apps only; the web page never shows it) also a line `idle.altAddress` with the IP address (16, `--mut`; the IPv4 address the box announcement resolved to, shown only when `{dom}` is a host name, decision N40). Step 2 — disc "2", label `idle.step2`, the code (80 mono, 6 characters as `XXX XXX`). If Locked: code at 35 % opacity and the line `idle.locked` under it (indented 54).
   - Right column (236 wide): QR code (212 box) and caption `idle.qrCaption`. If Locked: QR at 25 % opacity, caption `idle.qrLocked`.
3. **Footer row**: left (15, `--mut`) `idle.appLine` when at least one store or download link exists, otherwise `idle.appLineWeb` ("No download needed."); never name a store that has no listing. **Settings button** right: gear icon + word `settings.open`.

The QR code encodes `https://{dom}/{CODE}` (code without spaces; for the in-house box the IP address is used instead of `{dom}`). Use error correction level M; the QR version is whatever that URL needs (version 3 or 4 for normal addresses), the box stays 212.

`{dom}/app` is the presenter start page (the same as `{dom}`), which offers the app for the visitor's device.

Open and Approval on look the same apart from the pill. The step 3 "guide" cards from round 2 are gone.

**Offline:** steps 1 and 2 are replaced by a box (`--panel`, dashed 2 px `--line` border, radius 16, wifi-off icon 34 in `--acc`) with `idle.offline.title` (22/700) and a body. The box is as high as its content (padding 16 20, body 18 / 1.35, which wraps to two lines in EN and DE) and vertically centred in the main row; the QR column keeps its caption line, so the box centre sits about 14 ref below the QR box centre. That offset is intended (the web has the same geometry; decision N29). The QR box stays in place but is **empty** (3.2: no modules, so the old code cannot be scanned; `--panel2` fill, `--line` border, no caption). When the network returns, code and QR come back by themselves.

Reconnect schedule (decision 13.30): after a loss (at start, the first failed attempt counts as the loss; without any failure the 8 s rule of "Idle, connecting" applies; decision 13.59), try again **at 1 s, 2 s and 3 s**, then every 4 s. The offline box appears **when the attempt at 3 s has failed** (not on a separate timer), so a drop shorter than 3 s never shows the box, and it never flickers away while a later attempt runs. Until then the idle screen stays unchanged (no flicker). When the connection comes back with a new code, code and QR change without animation.

| Cause | Body | Retry |
| --- | --- | --- |
| Network or server not reachable, connection lost | `idle.offline.body` | 1 s, 2 s, 3 s, then every 4 s |
| `rate_limited`, `banned`, `server_full`, `forbidden` | `idle.offline.bodyBusy` (box at once, no 3 s wait) | after 30 s. A connection that closes within 60 s after one of these refusals counts as the same refusal (decision 13.44) |
| `proto_mismatch`, `bad_hello` (page older than server) | `idle.offline.bodyOutdated` (box at once); native apps `idle.offline.bodyOutdatedApp` (decision N14) | none; reload (web) or update (apps) needed |

If the server refuses Open (`open_rooms_disabled`), the room becomes Approval on, the pill changes and toast `toast.openDisabled` appears once. A failed transfer of content (damaged PDF …) is a toast with the existing wording.

### 4.3 Settings dialog

Opens only over the idle screen (not during setup, not while presenting, not while a join request is open).

Content (card 600 wide, scrim behind):

1. Header row: `settings.title` (section-label style, `--mut`) left; Close button right: × icon + `settings.close` (remote: `settings.closeRemote`, i.e. "Close (Back key)").
2. Section `settings.join.label` ("Joining"): three tiles Open / Approval on / Locked (icon + word), then one line describing the current mode (`settings.join.*Text`).
3. Section `settings.appearance`: four tiles Dark / Light / OLED black / Automatic.
4. Desktop display apps only: section `settings.receive.label` with the AirPlay and Miracast switch rows (15.4). The header row also has the About button left of Close (15.9).

Selected tile: border `--acc`, fill = selected tile fill (OLED black: no fill, see 3.2), bold. Changes apply at once; there is no Save button. The dialog stays open after a change.

What is remembered: the appearance, and Open or Approval on (they become the new default mode, replacing the one from setup). **Locked is never stored**: it lasts until End (the next session starts in the default mode) or until the display restarts.

**A join request closes Settings.** Only one dialog is on screen at a time: when a request arrives while Settings is open, Settings closes (changes already made stay) and the request dialog opens with focus on Yes. After the last request, focus returns to the Settings button. Consequence: Locked can never be chosen while requests wait, so there is no "Locked · n requests denied" toast (`toast.lockedRejected` is removed).

Initial focus: the tile of the current join mode.

### 4.4 Join request dialog (mode Approval on)

Appears by itself when someone asks to join, over the idle screen **or over the running presentation** (the picture stays visible behind the 55 % scrim). Over a presentation, name tag, End, toolbar, badge and pill are **under** the scrim and inert while any dialog (join request, link card) is open (decision 13.68). It is an `alertdialog`.

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

- **Name tag**, bottom left: `present.byline` ("“Lena” is showing"), 15/700, `--fg` on the panel fill, border `--line`. Second line, when the security code is known: `present.sas` ("Security code 171 852"), 13/400 `--mut`, digits mono 700 `--fg`. The two groups are separated by one ordinary space set in the mono font (one character cell wide, as in the join code); no extra and no narrower space (decision N34).
- **End**, bottom right: square stop icon + `present.end`. Same fill, border and text colour as the name tag. Not pink, not amber.
- **Annotation toolbar** (web display only, where a mouse, pen or touch exists; the TV apps have none): top centre, see 3.5. Pen, highlighter, eraser, ink swatches, undo, clear, laser, freeze, black screen, full screen. No grip (it does not move) and no collapse button (it hides by itself). It is not in the D-pad focus rows; Tab reaches it. Drawing counts as input, so the controls stay while someone draws; strokes stay visible after the controls fade.

All use the current appearance's `--panel`, `--line` and `--fg` (dark blue in Dark, light grey in Light, black with grey border in OLED black). With D-pad input, End shows the focus ring.

Shown only while it applies, at rest too (they are states, not controls): the **frozen badge** top left (`toolbar.frozenHint`), and the black screen with `toolbar.blackHint` (15/400 `--onblack` on `#000000`, centred, no box; decision 13.57). The frozen badge is hidden while the toolbar is visible (3.5).

Not on the display: "Close image", "Close document" and PDF page buttons. Pages are turned on the presenter's page (5.6); content goes away when the presenter stops, disconnects or the host presses End.

**Link sent by a presenter:** the display never opens a page by itself (safety rule). The link card is a dialog in the style of the join request (4.4: 600 card, scrim, section label, `receive.link.title`, the address in 18 mono `--fg` wrapping anywhere (decision 13.61), `receive.link.hint`), buttons **Open** (primary) and **Dismiss** (secondary), initial focus Open, the same 1 s input guard, Back = Dismiss. On the web display Open opens a new tab. The section label is `receive.link.title` ("{host} wants to open a link"); there is no separate heading.

- **Media link** (decision 13.34): a secondary button **Play** (`receive.media.play`, the word only, no host; decision 13.60) comes before Open: focus row [Play, Open, Dismiss], initial focus still Open. Warning and error lines (`receive.media.localWarning`, `.mixed`, `.hlsUnsupported`, `.error.*`) are 16/700 `--fg`, led by the `info` icon 20 in `--acc`; all other card text is `--mut`. No red. While playing, the card shows the section label, the address and the player: a header row (`receive.media.from` 15/700 `--fg` left, secondary **Stop** right) and the media element, full card width, at most 300 high, on `#000000`. Stop, Back/Escape and Dismiss close the whole card.
- **Platforms that cannot open a web page** (Apple TV; an Android TV without a browser): no Open button; the hint is `receive.link.hintTv`; focus row [Dismiss] or [Play, Dismiss] for media; initial focus Dismiss (Play for media).
- **Native display apps that can open a web page** (Android TV with a browser, tablets, computers): the hint is `receive.link.hintApp` (there is no tab; Open opens the system browser). `receive.link.hint` is web-only (decision N10).
- **Flutter display apps, this round:** no media player; a media link gets the plain link card (no Play, no media warning lines). Apple TV keeps its AVPlayer rules (`docs/appletv.md`). Decision N11.

**Content states while presenting** (decision 13.41): a PDF page is drawn on `#000000` like every picture (no grey page background). A running transfer, and then the opening of a received PDF, shows the transfer progress pill (3.5); there is no separate loading line. A failed transfer or a PDF that cannot be shown is a **toast** (`receive.transferError.*`); a single PDF page that cannot be drawn shows its `receive.document.pageError.*` text centred in a name-tag-look box (15/700 `--fg`). No red, no black pills. The **black screen, frozen picture and all strokes end with the presentation**: when the presenter stops, disconnects or the host presses End, the next presentation starts with a visible, live picture and no strokes.

**Annotation toolbar contents** (web display only, left to right, separators between groups): [Pen, Highlighter, Eraser] · [5 ink swatches: red, yellow, green, blue, white; 3 sizes: thin, medium, thick] · [Undo, Clear all, Laser pointer] · [Freeze, Black screen, Full screen]. Accessible names and tooltips: `toolbar.*` (11.7). No tool is selected at first (the pointer only shows the controls); choosing a tool marks it with the `--acc` border. The toolbar is the only place on the display with icon-only buttons (decision 13.35); it is a pointer tool, every button has a tooltip and an accessible name.

**End** closes the session: all presenters are disconnected, all waiting requests are dropped, blocks are cleared, a new code is made, the display returns to Idle, toast `toast.sessionEnded`. Presenters see "Presentation ended". There is no confirmation step.

While presenting, the screen is kept awake (D5). When the presenter stops, the display returns to Idle with the same code; presenters stay connected. When the presenter **leaves** (Disconnect, closed app, lost connection), the display also keeps its code: Idle, toast `toast.left`; join mode, blocks and waiting requests stay (decision N8). Only End, a restart, a lost and re-made room and the background rule of native-apps.md 15 make a new code.

### 4.6 Toasts

One line, centred at the top, `--fg` background with `--bg` text, 3.2 s. A new toast replaces the old one. Toasts never take focus and never cover the annotation toolbar (3.5). Keys: see the `toast.*` table.

### 4.7 Video codec dialog (desktop display apps only)

Added after the Flutter review (decision N17). Linux and Windows display apps may lack an H.264 decoder; the free one (Cisco OpenH264) may only be downloaded with the host's consent. The web display and TV, tablet and macOS display apps never show this.

- **When:** only over **Presenting**, when the presenter's picture is H.264 and no decoder is ready. It appears by itself, like the link card, with the same rules: scrim, the presenting controls under the scrim and inert (decision 68), 1 s input guard, one dialog at a time (a join request or link card waits until it is closed). At most **once per presentation**: after "Not now" it does not come back until the next presentation. Never over Idle, Settings or setup.
- **Presenting without a picture (decision N31):** the display is in Presenting from the moment the presenter's H.264 track arrives and no decoder is ready: black stage (`#000000`), wake lock held, nothing of Idle shown. The dialog's appearance does **not** reveal the name tag and End: if they were visible (an input within the last 5 s) they stay under the scrim and fade on their normal timer; otherwise they stay hidden. Toasts are drawn **above** the scrim (state table: toast over anything), uncovered, at top 14. After "Not now" (and while a download runs after "Not now"), the stage shows the state line `codec.stageHint` centred on black, 15/400 `--onblack`, no box (the look of `toolbar.blackHint`, decision 57), at rest too; input shows the controls as in any presentation. The line goes when the picture appears, the presenter stops or leaves, or End.
- **Look (decision N30):** the link-card dialog family of 4.5 (padding 28 × 34, radius 22, 1 px `--line`, gap 16, display tokens, no Material card, no Material icon), but **780 wide** (the wizard width), because its text is long. Centred on the canvas; its top edge is never above 66 ref (it clears the toast; with the sizes below no state needs more than 408 ref in EN or DE, and (540 − 408) / 2 = 66). Section label `codec.title`. Body `codec.explain` (16 `--mut`, line height 1.4; 3 lines in EN). Source line `codec.source` (15 `--mut`, line height 1.3; 1 line in EN). Admin note `codec.offline` or `codec.offlineNoDir` (14 `--mut`, line height 1.4; 2 lines in EN). Buttons left-aligned, 50 high: **Download** (`codec.download`, primary, initial focus), **Not now** (`codec.later`, secondary). Focus row [Download, Not now]; Back/Escape = Not now.
- **Downloading:** the button row is replaced by one line `codec.downloading` + ` ` + `codec.progress` (16/700 `--fg`) and below it (gap 8) a 6 ref bar, full text width (track `--panel2` with 1 px `--line`, fill `--acc`, radius 3), then the row [Not now] (it only hides the card; the download goes on; initial focus Not now). When the decoder is ready the card closes by itself and the picture appears; no toast.
- **Failed:** between the body and the source line, the line `codec.failed` (16/700 `--fg`, led by the `info` icon 20 in `--acc`, as the warning lines of 4.5; no red). The text is the key's text only; the technical reason goes to the log, never on screen. Admin note, row [**Try again** (`codec.retry`, primary, initial focus), Not now].
- **Unsupported** (`codec.unsupported`): label and the body `codec.unsupported` only (no explain, source or admin note, no technical reason), row [**Close**] (`settings.close`, initial focus). Close = Not now.
- OLED black: as every dialog (black card, `--line` border); the bar track is `#000000` with the `--line` border.
- Heights at 1920 × 1080 (EN, for the screenshots): missing ≈ 313 ref (626 px), downloading ≈ 365 ref, failed ≈ 351 ref, unsupported ≈ 200 ref (today: about 377 ref in the missing state, 600 wide). DE may add one line to the body and one to the admin note.

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
- **Screens on the same network:** the app start page gets the section of [14](#14-screens-on-the-same-network-apps-only) between the name field (which moves up) and the code. One tap on a found screen connects without a code.
- Platform variants (details in [native-apps.md](native-apps.md)): devices without a camera (computers) show the code field of 5.2 instead of `app.scanQr`; on tablets and computers the page ends with the text link `app.beDisplay`, which starts the display setup (4.1). Scanning uses the system scanner (not designed by us). In the apps the disconnected page (5.7) omits `bye.body` (it speaks of a browser tab).
- **Order of the app first-run page (decision N22, replaces the two items above where they differ):** 1. brand row (logo 28 + "Comet Cast", as 5.2); 2. heading `app.welcome`; 3. name field `start.nameLabel`; 4. the code part: either the **code chip** (code from a link, the install referrer or a scan: `XXX XXX` mono + green tick + `app.codeFromLink` / `app.codeScanned`) or, without a code, [section `nearby.title` + divider `start.or`, when spec 14 is built] then the secondary button `app.scanQr` (only where a system scanner exists) and below it the **code field** of 5.2 (label `start.codeLabel`, placeholder `XXX XXX` in `p-fg3`, same rules); 5. primary `app.connect` (phone layout: at the bottom; laptop layout: follows the content); 6. tablets and computers only, after Connect (in the phone layout below it, as the last item of the bottom area): the text link `app.beDisplay`, and on Windows below it the text link `app.miracast` (same style, decision N18; retired with the Miracast page once the display switch of 15.4 ships, decision N86). The brand row carries the settings button at its right end (15.8). **Placement of the links (decision N37):** each link is one line of 15 `p-primary`, underlined, in a tap area 44 high (the text centred in it); the two areas touch (no extra gap), so the two lines are 44 apart. Laptop layout: left-aligned with Connect, the first tap area starts 12 below Connect. Phone layout: centred in the bottom area, the first tap area starts 8 below Connect; the 22 bottom padding follows the last area. With a code field, Connect is always enabled and validates on submit (5.2: `start.codeEmpty`, `error.code`); with neither chip nor field it does not exist.
- `app.scanAgainBody` (15 `p-fg2`, above the scan button) is shown only on the first launch after an install that brought no code (iOS from the store, Android without referrer); never when the app was opened before (it says "once more").
- **Scan result** (decision N7): the URL forms of a link (native-apps.md 20.4). A valid code for a known server → chip `app.codeScanned`; a server this device has not confirmed → the relay confirmation page (7.2) first; anything else (not our URL, no valid code) → nothing changes and the line `app.scanWrong` (13/700 `p-error`) appears under the scan button until the next tap. Cancelling the scanner changes nothing.

### 5.4 Waiting for approval

Spinner + room name (label style, from `join_pending.room_name`; without a room name the row shows the spinner only, never the code); heading `wait.title`; card with `wait.checkLabel`, the check number (58 mono, `p-primary`) and `wait.checkHint`; progress bar (6 px) counting down `join_pending.timeout_secs` (60 s by default) with `wait.expires`; at the bottom secondary `wait.withdraw`. Withdrawing removes the request from the display (toast `toast.withdrawn`) and returns to the start page.

### 5.5 What to show

Header row: green tick disc (40) + `what.connectedTo` (room name from `joined.room_name`; without one `what.connected`, never the code). Heading `what.title`. Three option rows (icon in `p-primary`, title + subtitle):

| Row | Title | Subtitle |
| --- | --- | --- |
| Screen | `what.screen` | Laptop: `what.screenLaptop`; app: `what.screenApp` on phones, `what.screenTablet` on tablets (decision N15; chosen by device kind, not by layout width), `what.screenLaptop` on computers; phone browser: `what.screenPhoneBrowser` when an app can be offered for this device, otherwise `what.screenPhoneBrowserNoApp` (icon in `p-warn` in both cases) |
| File | `what.file` | `what.fileSub` |
| Link | `what.link` | `what.linkSub` |

Bottom: secondary `what.disconnect` → "Disconnected" page; toast `toast.left` on the display. No security code on this page (the display shows it only while presenting).

- Screen on a laptop → the browser's own share dialog opens directly, with "Window" preselected where the browser allows it. There is **no** explainer page before it.
- Screen in a phone browser → error "Screen sharing does not work in this browser" (it is known before; the page says so in the subtitle). Without an app offer for the device (13.6) the page has no "Get the app" button and its "What now" text is `error.noScreen.nowNoApp` (decision 13.42).
- Screen in the app → the system prompt (Android "Start recording or casting", iOS "Start Broadcast").
- Cancelling the system dialog returns to this page without an error (`capture_denied` never shows an error page).
- **Desktop apps (decision N55):** the system picker opens on every tap of Screen (no remembered choice). When the system cannot share at all, the screen-sharing page `error.capture` (6) follows instead of the picker. A shared window that is minimised keeps its last picture on the display; a shared window that is closed, or a share ended in the system's own UI, ends the presentation like Stop (presenter back here, display to Idle, no toast, no message).
- File → file page. **App (decision N16):** the browser variant below on every app platform (the system picker already offers recent files; no media permission is asked). Body `file.bodyApp` where the app can convert Office files (where the conversion component is found and the Office switch of 15.8 is on: desktops and Android; decision N81), else `file.bodyWeb`; while an Office file is converted the progress line shows `file.converting` (small print, no bar). The recent-files grid, `file.title` and `file.body` are not used. **Browser** (a browser cannot list recent files): heading `what.file`, body `file.bodyWeb`, primary `file.pick` (opens the system file dialog: PNG, JPEG, WebP, GIF, PDF), then while sending a progress line (small print "{name} · {n} %" + 6 px bar), an inline error in `p-error` if it fails, secondary `common.back` at the bottom. Picking a file starts presenting. Cancelling the dialog changes nothing.
- Link → link page: heading `what.link`, field `link.label` (type url), `link.hint`, primary `link.go`, secondary `common.back`. The display asks its host before opening (4.5).
- Replacing what is shown: "Stop showing", then pick again (the display shows Idle in between). No "replace" action.

### 5.6 Presenting (presenter side)

Icon disc + heading (`presenting.screen` / `presenting.file` / `presenting.link`), body `presenting.where` (omitted when there is no room name), small print `presenting.sas` when the security code is known (`p-fg3`, digits mono 700 `p-fg`), primary button `presenting.stop`. Stop returns to "What to show". The browser's own "sharing" bar or the phone's red indicator stay as the system draws them; when Chrome's bar is shown at the bottom of the page, keep 76 px free at the bottom of the page.

While a PDF is shown: a row of two secondary buttons `presenting.prevPage` and `presenting.nextPage` (phone: side by side, each half width, min 48 high) **directly above Stop** (8 between them; on the phone both rows are part of the bottom action area, so the thumb reaches all three), with `presenting.pageOf` ("Page 3 of 12") as small print above the row when the page count is known. When the page is known, Previous is disabled (`p-disabled`) on page 1 and Next on the last page; while it is unknown both stay enabled. **Protocol requirement (decision 13.31):** the display reports the shown page and the page count to the presenter over the content control channel whenever they change, as `{"kind":"page","page":n,"pages":m}` (1-based, `1 ≤ page ≤ pages`; anything else is ignored and means "unknown"; decision 13.58); until this message exists the line is not shown and both buttons stay enabled.

### 5.7 Disconnected

Grey tick disc, `bye.title`, `bye.body`, primary `bye.again` (back to the start page; phone: start route).

### 5.8 Video codec page (desktop presenter apps only)

Added after the Flutter review (decision N17). When a Linux or Windows presenter chooses Screen and the H.264 encoder is missing, this page comes **before** the system picker (the picker would be useless without it). Layout of the relay confirmation page (7.2, presenter variant):

- Icon disc 56 `download` in `p-primary` on `p-primary-soft`; heading `codec.title`; body `codec.explain` (15 `p-fg2`); `codec.source` and the admin note (`codec.offline` / `codec.offlineNoDir`) as small print.
- Actions (laptop row, primary first): **Download** (`codec.download`, primary, initial focus), **Back** (`common.back`, secondary → "What to show"). System Back = Back.
- Downloading: the primary is disabled with the label `codec.downloading`; under the body a progress line as on the file page (small print `codec.progress` + 6 px bar). Back stays enabled (the download goes on).
- Failed: inline error `codec.failed` in `p-error` (as the file page error), the key's text only (no technical reason, decision N46); the primary reads `codec.retry`.
- Ready: the primary reads `what.screen` and opens the system picker (a system dialog only as the direct result of the person's tap, native-apps.md 8). Picking starts presenting as usual; cancelling the picker returns to "What to show" without an error.
- Unsupported: the screen-sharing page `error.capture` (6) with `codec.unsupported` as its "why" (key text only, N46; decision N56 replaces `error.other` here).
- No Material `Card`, `FilledButton`, `LinearProgressIndicator` or Material icon.

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
| `error.outdated` | `proto_mismatch`, `bad_hello`, `bad_message` (page and server do not match) | (i) in grey | `error.action.reload` (reloads the page). **Apps** (decision N14): heading `error.outdated.titleApp`, why `error.outdated.why`, now `error.outdated.nowApp`, button `error.action.retry` |
| `error.capture` | Apps only: the device cannot share its screen (`capture_unavailable`, `capture_os_old`, `capture_no_portal`, codec unsupported 5.8; decision N56) | (i) in grey | Show photo or file (primary, → file page), Show a link (→ link page), Back (→ "What to show"); the person stays connected |
| `error.other` | Every other failure: `forbidden`, `altcha_*`, `wrong_role`, `not_in_room`, `unknown` and any code not listed (decision 13.32) | (i) in grey | Try again |

For the in-house box, `error.noInternet` uses the `nowBox` text (must be on the same Wi-Fi). The apps map failures by the structured `code` the core reports (native-apps.md 20.1, decision N9), never by message text. `error.noScreen` is web-only: an app that cannot capture shows `error.capture` (decision N56: heading `error.capture.title`, "why" `errors.<code>`, "what now" `error.noScreen.nowNoApp`, initial focus on the primary, system Back = Back). Mapping of other failures:

- `connection_lost` → `error.noInternet`, **except** within 60 s after a `rate_limited`, `banned` or `server_full` refusal, where it is `error.busy` again (the server closes the connection after a ban; decision 13.44).
- `peer_failed` → `error.displayOff`; **but** when the list of STUN/TURN servers (`/ice-servers`) was refused for this connection (`rate_limited` or `refused`) → `error.busy`, and when it failed otherwise (`http_error`, `bad_answer`, `unreachable`) → `error.displayOff` with the "why" text `error.displayOff.whyRelay`. A relay that offers no list (404, `none`) changes nothing. Nothing is shown before the connection fails (decision 13.45). A refused list does **not** start the 60 s memory of the line above (decision 13.59).
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
| Settings | [About, Close] · [Open, Approval on, Locked] · [Dark, Light, OLED black, Automatic]; desktop display apps also · [AirPlay] · [Miracast] where listed (15.4) |
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
- Focus ring: on page load the initial focus shows the ring (the page cannot know yet whether a keyboard or remote drives it). The first pointer press, touch start or real mouse movement (non-zero distance) hides it; the next key press, a modifier key alone included, shows it again (decisions 13.50, 13.62). Same ring as on TV.
- The server (or any static host) must answer `/display`, `/{CODE}` and `/app` with `index.html`; otherwise the QR code leads to a 404 (GitHub Pages: a copy of `index.html` as `404.html`).
- A relay that only a link asked for (`?server=`) is confirmed **before anything else is shown or connects** (decision 13.37). Nothing of the page that would use the relay is drawn behind it, and the relay host appears nowhere else (not as `{dom}`, not in the `start.beDisplay` link) until it is confirmed.
  - **On `/display`:** on the scaled canvas (3.1, all sizes in reference px), over plain `--bg` with only the logo (46) at the idle screen's top-left position. Dialog card 600 wide as 4.4 (no scrim needed: there is nothing behind it), centred: section label `relay.confirm.title`; body `relay.confirm.body` 16 `--mut`; the host on its own line, 18 mono `--fg`, wrapping anywhere; `relay.confirm.risk` 16 `--mut`; buttons [**Cancel** (`relay.confirm.cancel`, secondary, initial focus), **Use this relay** (primary)], left-aligned, 50 high. Back/Escape = Cancel. Cancel continues with the default relay (setup or idle); Use continues with the confirmed relay.
  - **On presenter pages:** its own **page** in the error-page layout (6), replacing the start page: icon disc `info` in `p-warn`; heading `relay.confirm.title`; body `relay.confirm.body` (15, `p-fg2`); the host on its own line, 17 mono 700 `p-fg`; `relay.confirm.risk` as small print; actions [**Cancel** secondary, **Use this relay** primary] in this order (phone: stacked at the bottom, Cancel on top; laptop: in a row below the content, Cancel left). Cancel has the initial focus; Enter on load cancels. After the decision the start page appears (with the code chip if the link carried a code). **In the apps** (decision N6) the page is reached from an app link, the install referrer or a scanned QR code, and its body is `relay.confirm.bodyApp` ("this app", not "this page"); Cancel continues with the server the app would use anyway, Use remembers the host as confirmed on this device. Native **displays** never get a server from a link, so the display variant above is web-only (decision N19).
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
- Google Cast receiving; AirPlay and Miracast receiving anywhere but desktop display apps (those: section 15.4).
- The mockup's own frame: chapter tabs, simulated remote, scenario buttons, notes, device switcher, the "view from 4 m" preview, the host instructions panel.
- Store pages, system share dialogs and camera chips: they are drawn by the operating system.
- Share sheet entry points ("Share to Comet Cast" from other apps) on any platform other than Android and iOS/iPadOS (those: section 15.3).

## 11. Copy

### 11.1 Rules for German

- **Shipped default: "du"** (column "DE du (shipped)", owner decision 54). The column "DE Sie" is kept complete and is the alternative set.
- Implement this as two German value sets with identical keys (for example `de.json` = du and a `de-sie.json` set, or two ARB files), so switching is one file swap, not a code change. Where the web app today has `de.json` = Sie and `de-du.json` = du, it is enough to make the du set the one that is loaded; the i18n check of 11.8 then runs in the other direction too (every du key that addresses the reader exists in the Sie set).
- Every German string is a short main clause. Only the pronoun and the verb ending change between the two forms; sentence structure stays the same. Do not add strings in only one form.
- English has one form.
- Placeholders: `{room}`, `{dom}`, `{code}`, `{name}`, `{device}` (only in `start.detected`), `{n}`, `{total}`, `{sas}` (security code, `123 456`), `{ip}`, `{url}`, `{os}` (Windows, macOS, Linux), `{browser}` (Chrome, Safari, …), `{here}` (the address the presenter page is on). Quotes are part of the string where shown (German „…“, English “…”). Names are inserted already quoted via `common.quoted`.

Keys use the web app's dotted form. Flutter ARB uses the same keys in lower camel case (`settings.join.openText` → `settingsJoinOpenText`). Apple TV uses the dotted keys as they are.

### 11.2 Common

| Key | EN | DE du (shipped) | DE Sie |
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

| Key | EN | DE du (shipped) | DE Sie |
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

| Key | EN | DE du (shipped) | DE Sie |
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

| Key | EN | DE du (shipped) | DE Sie |
| --- | --- | --- | --- |
| `setup.address.sourcePage` | Set by the server of this page. | Vom Server dieser Seite vorgegeben. | Vom Server dieser Seite vorgegeben. |
| `setup.address.sourceTyped` | Entered by hand. | Von Hand eingegeben. | Von Hand eingegeben. |
| `setup.address.fieldLabel` | Server address | Serveradresse | Serveradresse |

### 11.5 Presenter: start, app, waiting, what to show, presenting

| Key | EN | DE du (shipped) | DE Sie |
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
| `start.appLaptop` (valid: section 14) | The app finds screens on the same network without a code. For now the browser is enough. | Die App findet Bildschirme im selben Netz auch ohne Code. Für jetzt reicht der Browser. | Die App findet Bildschirme im selben Netz auch ohne Code. Für jetzt reicht der Browser. |
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

| Key | EN | DE du (shipped) | DE Sie |
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

| Key | EN | DE du (shipped) | DE Sie |
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
| `receive.media.play` | Play | Abspielen | Abspielen |
| `receive.media.hint` | This link points to a media file. It is loaded from {host} only if you click “Play”. | Dieser Link verweist auf eine Mediendatei. Sie wird von {host} nur geladen, wenn du auf „Abspielen“ klickst. | Dieser Link verweist auf eine Mediendatei. Sie wird von {host} nur geladen, wenn Sie auf „Abspielen“ klicken. |
| `receive.document.loading` | Opening {name}… | {name} wird geöffnet … | {name} wird geöffnet … |
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

### 11.9 Native app copy (Flutter review, decisions N6–N18)

| Key | EN | DE du (shipped) | DE Sie |
| --- | --- | --- | --- |
| `receive.link.hintApp` | The sender asks you to open this link. Open it only if you expect it. It opens in the browser, and only if you choose “Open”. | Der Absender bittet dich, diesen Link zu öffnen. Öffne ihn nur, wenn du ihn erwartest. Er öffnet sich im Browser, und nur wenn du „Öffnen“ wählst. | Der Absender bittet Sie, diesen Link zu öffnen. Öffnen Sie ihn nur, wenn Sie ihn erwarten. Er öffnet sich im Browser, und nur wenn Sie „Öffnen“ wählen. |
| `setup.address.sourcePolicy` | Set by the administrator of this device. | Vom Administrator dieses Geräts vorgegeben. | same as du |
| `idle.offline.bodyOutdatedApp` | This app is out of date. Update it to get a code. | Diese App ist veraltet. Aktualisiere sie, um einen Code zu bekommen. | Diese App ist veraltet. Aktualisieren Sie sie, um einen Code zu bekommen. |
| `error.outdated.titleApp` | This app is out of date | Diese App ist veraltet | same as du |
| `error.outdated.nowApp` | Update the app, then try again. | Aktualisiere die App und versuch es dann noch einmal. | Aktualisieren Sie die App und versuchen Sie es dann noch einmal. |
| `what.screenTablet` | Your whole tablet display, live | Dein ganzes Tablet-Display, live | Ihr ganzes Tablet-Display, live |
| `file.bodyApp` | A PDF, an image or an Office file from this device. It appears on the screen as soon as you pick it. | Ein PDF, ein Bild oder eine Office-Datei von diesem Gerät. Die Datei erscheint auf dem Bildschirm, sobald du sie auswählst. | Ein PDF, ein Bild oder eine Office-Datei von diesem Gerät. Die Datei erscheint auf dem Bildschirm, sobald Sie sie auswählen. |
| `file.converting` | Converting {name} … | {name} wird umgewandelt … | same as du |
| `relay.confirm.bodyApp` | A link asks this app to use the relay {host} instead of the default. Only continue if you know and trust its operator. | Ein Link fordert diese App auf, das Relay {host} statt des Standards zu verwenden. Mach nur weiter, wenn du den Betreiber kennst und ihm vertraust. | Ein Link fordert diese App auf, das Relay {host} statt des Standards zu verwenden. Fahren Sie nur fort, wenn Sie den Betreiber kennen und ihm vertrauen. |
| `app.scanWrong` | This QR code is not from a Comet Cast screen. | Dieser QR-Code stammt nicht von einem Comet-Cast-Bildschirm. | same as du |
| `app.miracast` | Receive Miracast on this PC | Miracast auf diesem PC empfangen | same as du |
| `ios.cameraUsage` (`NSCameraUsageDescription`, shown by the system) | Comet Cast uses the camera only to scan the QR code on the screen. | Comet Cast nutzt die Kamera nur, um den QR-Code auf dem Bildschirm zu scannen. | same as du |
| `codec.later` | Not now | Nicht jetzt | same as du |
| `codec.failed` (changed, N46: no `{reason}`) | The video codec could not be downloaded. | Der Video-Codec konnte nicht heruntergeladen werden. | same as du |
| `codec.unsupported` (changed, N46: no `{reason}`) | This system cannot use the video codec, so shared screens cannot be shown or sent here. | Dieses System kann den Video-Codec nicht nutzen. Geteilte Bildschirme lassen sich hier nicht zeigen oder senden. | same as du |
| `errors.capture_unavailable` (new, N33; the "why" of `error.other`) | This device cannot share its screen with Comet Cast yet. Show a file or a link instead. | Dieses Gerät kann seinen Bildschirm noch nicht mit Comet Cast teilen. Zeig stattdessen eine Datei oder einen Link. | Dieses Gerät kann seinen Bildschirm noch nicht mit Comet Cast teilen. Zeigen Sie stattdessen eine Datei oder einen Link. |
| `codec.stageHint` (new, N31) | No picture yet: this screen needs the video codec. | Noch kein Bild: Dieser Bildschirm braucht den Video-Codec. | same as du |
| `codec.downloading` (changed: typographic ellipsis) | Downloading the video codec … | Der Video-Codec wird heruntergeladen … | same as du |
| `error.capture.title` (new, N56) | Screen sharing does not work here | Bildschirm teilen geht hier nicht | same as du |
| `errors.capture_unavailable` (changed, N56: the "what now" part moved to `error.noScreen.nowNoApp`) | This device cannot pass its screen on to Comet Cast. | Dieses Gerät kann seinen Bildschirm nicht an Comet Cast weitergeben. | same as du |
| `errors.capture_os_old` (new, N56; macOS before 14) | Sharing the screen needs macOS 14 or later. This Mac has an older version. | Bildschirm teilen braucht macOS 14 oder neuer. Auf diesem Mac läuft eine ältere Version. | same as du |
| `errors.capture_no_portal` (new, N56; Linux without the screen-sharing portal or PipeWire) | This desktop has no screen sharing for apps set up. Whoever looks after this computer can add it. | Auf diesem Desktop ist kein Bildschirm-Teilen für Apps eingerichtet. Wer diesen Computer betreut, kann es nachrüsten. | same as du |
| `error.noScreen.nowNoApp` (restored in the Flutter sets, N56; value as 11.6) | Show a photo, a file or a link instead. | Zeig stattdessen ein Foto, eine Datei oder einen Link. | Zeigen Sie stattdessen ein Foto, eine Datei oder einen Link. |

Reused without change on `error.capture`: `error.why`, `error.now`, `error.action.showFile`, `error.action.showLink`, `common.back`, `codec.unsupported` (as the "why" of 5.8). No copy for a cancelled picker, a refused permission, a minimised or a closed shared window (decision N55).

### 11.10 Built strings outside the designed flow (decision N27)

These keys exist in the Flutter ARB files and are used by surfaces this spec now places (4.7, 5.8, N18) or by system UI. Their values stay as in `apps/flutter/lib/l10n/app_en.arb`, `app_de.arb` (du, converted from Sie by pronoun and verb ending only) and `sie/app_de.arb` (Sie); they are part of the i18n check (every key in both German sets, the du/Sie rule of 11.1):

- `codec.title`, `.explain`, `.source`, `.download`, `.progress`, `.retry`, `.failed`, `.offline`, `.offlineNoDir`, `.unsupported` (ARB `codecTitle` …);
- `miracast.*` (the Miracast page, outside this design, spec 10);
- `sendReject.*` and `sendContent.*` refusal texts (inline error of the file and link pages);
- the Android capture notification texts (system notification of the foreground service).

Strings of the removed pre-design UI that nothing uses any more (the old discovery notes, the test-stream and monitor texts) are deleted from all three files.

## 12. Open points that do not block implementation

- ~~The owner's question "du" or "Sie" (D6).~~ Answered: "du" ships (decision 54).
- The Android install referrer passing code and address to the app is an assumption; if it does not work, Android uses the iOS behaviour (scan again).
- Exact idle-dim delay: 7 minutes is inside the owner's 5–10 minute range; adjust in one constant if testing suggests otherwise.
- Owner: should the web display keep its drawing tools (annotation toolbar) at all? This spec keeps them, hidden at rest (13.7); removing them would make the display match "only End" exactly.
- Owner: store listings and the desktop download. Until they exist the app offers are hidden (13.6).
- ~~Owner: `start.appLaptop` promises the list of nearby screens.~~ Answered: the line stays and is now true; the list is designed in section 14 (decision 55). Until the list is built in the desktop app, `DOWNLOAD_URL_DESKTOP` stays empty, so the line stays hidden.
- Owner: a screen in **Open** mode now lets anyone on the same network connect without seeing the screen (they only need to be in the network, not in the room). Is that acceptable as the default, or should Open screens announce themselves without the code (found in the list, but the code must still be typed), leaving one-tap joining to Approval on? This spec assumes: Open and Approval on both announce the code (14.8).
- Owner: should a host be able to switch the announcement off on a display (for example in a shared office network)? Settings holds only join mode and appearance today (D2), so this spec adds no switch; Locked already hides the code.
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

### Decisions of 2026-11-07, afternoon

54. **German ships in "du" (owner).** D6 and 11.1 changed; the tables mark "DE du" as shipped; the "Sie" set stays complete as the alternative.
55. **`start.appLaptop` stays and becomes true (owner).** The apps find screens on the same network: section 14 (start page section, rows with name, mode and code, one tap to connect, relay confirmation for announced servers, empty, blocked, paused and permission states, announcement requirements). Mockup: https://claude.ai/artifact/2o5vZaZk9zuzBkPcYeKtWF.
56. **The code is shown in every row** of that list. It is what tells equal names apart and lets the person check the row against the screen in front of them; the announcement therefore carries the code, not only a token (14.8).

### Decisions after the third implementation review

Made on 2026-11-07 after reviewing fix round 2 (`apps/web`, main 7d33eb7) and its 308 screenshots (`/tmp/shots3`, with keyboard-driven focus captures). Questions F1–F8 are answered in [implementation-questions.md](implementation-questions.md); the work items are in [fix-list-web-3.md](fix-list-web-3.md). The 16 items of fix list 2 were checked and pass (item 8 by its tests; it cannot be seen in a still). The sections above already contain these decisions.

57. **Text on black (F1).** New token `--onblack` = `#A1A9BA` in every theme for text drawn directly on the black picture area: `toolbar.blackHint` (15/400, centred, no box). `--mut` is never used on `#000000` in Light. The PDF "Opening …" line moves into the transfer pill (3.5); DE `receive.document.loading` gets the space before `…` like `receive.transfer`.
58. **Page message (F2).** `{"kind":"page","page":n,"pages":m}`, display → presenter, 1-based, sent once the count is known and on every change; invalid values mean "unknown" (5.6). Native cores send and read it with the content channel.
59. **Reconnect edges (F3, F4, F5).** At start the first failed attempt counts as the loss; the outdated box appears at once; a refused STUN/TURN list does not start the 60 s busy memory (4.2, 6). All as built.
60. **Media link copy.** `receive.media.play` = "Play" / "Abspielen" (no host): it matches the hint ("click “Play”") and keeps [Play, Open, Dismiss] on one row. EN `receive.media.hint` uses typographic quotes (11.7).
61. **Link address in `--fg`.** The address in the link card is 18 mono `--fg`, like the relay host: it is what the host has to judge (4.5).
62. **Focus captures (F6).** Product as built (Cancel first). Any key press, a modifier alone included, shows the focus ring; screenshots show the ring with Shift so the initial focus stays where it is (7.2).
63. **Transfer pill (F7).** One line, max 340 ref, file name shortened in the middle keeping the extension, percentage never cut (3.5).
64. **Wizard card top 40.** Steps 1–4: top edge fixed at 40 (was 64), at least 24 free below in EN and DE (4.1).
65. **Frozen badge vs toolbar.** Hidden while the toolbar is visible; the Freeze button's selected border carries the state (3.5, 4.5).
66. **Wizard choice cards** use the selected-tile look; focus on the unselected card no longer shows two equal amber outlines (4.1).
67. **Locked QR in Light** keeps its 1 px `--line` border at full opacity; only the code fades (3.2).
68. **Dialogs over a presentation.** Name tag, End, toolbar, badge and pill sit under the scrim and are inert while a join request or link card is open (4.4).
69. **The web display is design-complete.** With [fix-list-web-3.md](fix-list-web-3.md) done, no further design review round is needed for `/display` and the web presenter pages; the engineer checks each item against its screenshot. New design work (spec 14, native apps) does not reopen the web.

### Decisions after the Flutter review

Made on 2026-11-07 after reviewing the Flutter app (`apps/flutter`, main 66de24c) and its 198 screenshots (`/tmp/shots-flutter`: display TV 1920 × 1080 and tablet 1280 × 800 in Dark, Light and OLED black; presenter phone 400 × 800 and tablet 1280 × 800 in Light and Dark), compared with the web screenshots (`/tmp/shots3`). Questions C1–C9 and D1–D13 are answered in [implementation-questions-flutter.md](implementation-questions-flutter.md); the work items are in [fix-list-flutter.md](fix-list-flutter.md); the core interface the apps need is native-apps.md 20. Numbered N1… so they do not collide with the web review's numbers. The sections above already contain these decisions.

- **N1 Room name (C1).** `cc-ffi` passes the setup name to `create_room` and reports `room_name` from `join_pending` / `joined` to the presenter (decision 2 applies unchanged).
- **N2 Security code (C2).** Computed in the core with exactly the web algorithm (`core/sas.ts`), reported once per connection to both sides; name-tag line 2 and `presenting.sas`.
- **N3 Page message (C3).** The native display sends and the native presenter reads `{"kind":"page","page":n,"pages":m}` (decision 58).
- **N4 Receive progress (C4).** The receiver reports bytes received; the transfer pill shows the real percentage.
- **N5 One join per presenter (C5).** Connect joins; "What to show" comes after the join (with its header); Stop keeps the join; Disconnect leaves. The host approves a person once per connection, not once per thing shown.
- **N6 Server of a presenter app (C6).** No server field. Links and the install referrer on the built-in relay's domain; an unknown `server` is confirmed first on the relay confirmation page with `relay.confirm.bodyApp` (7.2).
- **N7 QR scanner (C7).** System scanner on Android and iOS/iPadOS phones and tablets; scan button and code field together; `app.scanWrong` for a foreign QR code; rescan buttons only where the scanner exists (5.3).
- **N8 Same code after a presenter leaves (C8).** The receiver keeps its room after `PeerLeft` (4.5).
- **N9 Structured errors (C9).** Error events carry a `code`; pages are chosen by code (6), never by message text.
- **N10 Link hint in apps (D1).** `receive.link.hintApp`; `receive.link.hint` is web-only (4.5).
- **N11 No media player in Flutter link cards this round.** Plain link card for media links (4.5).
- **N12 No default port in `{dom}`.** `:443` is never shown (glossary).
- **N13 Policy source line (D3).** `setup.address.sourcePolicy` (4.1).
- **N14 Outdated in apps.** `idle.offline.bodyOutdatedApp`; presenter `error.outdated.titleApp` / `nowApp` with Try again (4.2, 6).
- **N15 Tablet screen row.** `what.screenTablet` on tablets, chosen by device kind (5.5).
- **N16 File page in apps.** The browser variant on every app platform; `file.bodyApp` with Office conversion; no recent-files grid (5.5).
- **N17 Video codec consent (D6).** Desktop display: a link-card-style dialog over Presenting, once per presentation (4.7). Desktop presenter: its own page before the system picker (5.8). Replaces "codec notices as toasts".
- **N18 Miracast entry (D7).** Windows only: text link `app.miracast` below `app.beDisplay` on the app first-run page; the Miracast page is rebuilt with the presenter kit but not redesigned (outside this flow, 10).
- **N19 Relay confirmation on native displays.** Not needed: native displays get no server from a link (7.2).
- **N20 Annotation toolbar (D11).** Stays unreachable in every native build of this round.
- **N21 Shadows.** The product keeps the soft shadows of 3.5. The hard edges in the Flutter screenshots come from `flutter_test` (`debugDisableShadows`); review screenshots are taken with it off.
- **N22 App first-run page.** Brand row; order brand, heading, name, code part, Connect, then `app.beDisplay` / `app.miracast`; placeholder `XXX XXX`; `app.scanAgainBody` only on the first launch without a code (5.3). This is already the order spec 14 needs, so the nearby round only inserts its section.
- **N23 Secondary buttons.** 16/500 as built on the web (native-apps.md 10 corrected); disabled secondary = `p-disabled` fill, no border, `p-ondisabled` text; on the laptop layout the two PDF page buttons are content width (min 160), as on the web.
- **N24 Initial focus is visible in the first frame.** A screen or dialog never shows a frame with no ring or with the ring still on a control under the scrim (7.1: exactly one ring).
- **N25 Nearby list and announcement (D8).** Both come in the spec-14 round together; no interim announcement.
- **N26 Desktop capture (D9).** Through the system picker on Linux, Windows and macOS (whole screen or one window, as `what.screenLaptop` says); test stream, duration and monitor list stay removed.
- **N27 Legacy strings (D10).** Accepted as converted and listed in 11.10; unused ones are deleted.
- **N28 Flutter design-complete.** With [fix-list-flutter.md](fix-list-flutter.md) done (part A and part B), the Flutter display and presenter are design-complete for this round, except the nearby list (spec 14, next round). No further design review round is needed; the engineer checks each item against its acceptance line and screenshot.

### Decisions after the second Flutter review

Made on 2026-11-07 after reviewing main f250cc2 (the first Flutter fix round) and its 362 screenshots (`/tmp/shots-flutter2`: display TV 1920 × 1080 and tablet 1280 × 800 in Dark, Light, OLED black, desktop codec dialog 1920 × 1080, DE du; presenter phone 400 × 800 and tablet 1280 × 800 in Light and Dark), compared with the final web screenshots (`/tmp/shots4`). Questions F1–F11 and the eleven listed differences are answered in [implementation-questions-flutter.md](implementation-questions-flutter.md); work items in [fix-list-flutter-2.md](fix-list-flutter-2.md). The sections above already contain these decisions.

- **N29 Offline box position (F1).** The box is centred in the main row (as built); its centre about 14 ref below the QR box centre is intended (the QR column keeps its caption line so nothing jumps when the code returns; the web is the same). The body wraps to two lines (as on the web). The acceptance "within 10 ref of the QR centre" of fix list A1 was wrong and is withdrawn.
- **N30 Codec dialog size.** 780 wide (wizard width), the type sizes of 4.7, top edge never above 66 ref; the failed and unsupported states show the key text only (4.7).
- **N31 Presenting without a picture (F4).** Presenting starts when the H.264 track arrives without a decoder (black stage, wake lock). The dialog does not reveal the controls; controls that were visible stay under the scrim; toasts stay above the scrim. After "Not now" the stage shows `codec.stageHint` (new copy, 11.9) until the picture appears or the presentation ends (4.7).
- **N32 iOS screen sharing keeps the approval (F2).** The broadcast extension must join on its own connection (iOS suspends the app; native-apps.md 20.5), but the host approves a person once per connection on iOS too: the app hands its approval over to the extension and back with a single-use hand-over token that the display accepts without a dialog and without `toast.left` / `toast.connected` (native-apps.md 20.5). Until that ships, the iOS behaviour of F2 (a) is acceptable for test builds only, with these fixes and no new copy: the extension sends the person's name; while it waits for the host the app shows the waiting page (5.4) with the extension's check number and countdown; when the broadcast ends the app joins again with the same code (in Approval on through the waiting page) and lands on "What to show", not on the first-run page. In Locked the hand-over is refused like any join (`error.locked`); this is accepted.
- **N33 Desktop capture until the picker exists (F3).** The X11 whole-desktop path is acceptable in test builds; release needs the system picker (N26, fix list B7). Where no capture backend exists, `capture_unavailable` shows `error.other` with the new `errors.capture_unavailable` as its "why" (11.9) and Try again replaced by `common.back` to "What to show" (the person stays connected), never a technical text.
- **N34 Security-code digits.** One ordinary space in the mono font between the groups (it looks wide because a mono space is a full cell; the web and the join code do the same). Accepted as built (4.5).
- **N35 End icon.** The `stop` icon of Icon.vue (rounded square, stroke 2.4) is the specified icon; accepted as built.
- **N36 Picture on a 16:10 screen.** The picture fills the window, letterboxed in its own aspect ratio on `#000000` (4.5); the controls, toasts and dialogs stay on the 16:9 canvas (3.5), so on 1280 × 800 they sit 40 px further in than the window edge. Accepted as built.
- **N37 First-run links.** Laptop layout left-aligned, phone layout centred; 44-high tap areas stacked without a gap (5.3).
- **N38 Presenter laptop column.** Content in the 744 column centred in the window, items left-aligned, top `max(40, 10vh)`, action row content width and left-aligned under the content (7.4, as the web). The Stop button under the PDF page row and the top-left look on tablets are correct; accepted as built.
- **N39 Miracast page.** Rebuilt with the presenter kit (N18): pairing as three single-choice option rows (selected: 2 px `p-primary` border, `check` 16 in `p-primary` before the title; F10 accepted). Phone layout: the content scrolls, [Start, Back] stay in the bottom action area, at least 24 free between the last note and the action area when scrolled to the end. Laptop layout: [Start receiving, Back] in one row under the content (7.4). Consent page: the body sentence with the PIN is body text (15 `p-fg2`) with only the PIN digits in mono 700 `p-fg` (like `presenting.sas`); [Decline, Allow] in this order like the relay page (the safe choice first), initial focus Decline.
- **N40 In-house box line (F8).** Wired, not hand-made: the box address is known when the display's server came from the box announcement (`sourceBox`); `idle.altAddress` shows the IPv4 address the announcement resolved to, only when `{dom}` is a host name; the QR then uses that IP (4.2).
- **N41 Page error reasons (F5).** `receive.document.pageError.failed` for every page that cannot be drawn is accepted; the four other texts stay unused until the core reports a reason. No core work is asked for this round.
- **N42 First launch (F6).** The flag written on the first launch (`referrerRead`, on both platforms) is the signal; accepted as built. A data reset counts as a first launch.
- **N43 Links while the app runs (F7).** Accepted as built: handled on the first-run, error and disconnected pages; ignored while waiting, connected or presenting (no prompt, no toast).
- **N44 Office conversion (F9).** Accepted: `file.bodyApp` exactly when the conversion worker is found.
- **N45 Deleted strings (F11).** Accepted for the Flutter ARB files; the web keeps its own keys (spec 11 still lists them for the web).
- **N46 No technical text on screen.** Error and failure lines in the apps show the key's text only; reasons such as "network error" or "no library" go to the log (4.7, 5.8, 6).
- **N47 Flutter design status.** The Flutter display and presenter are **design-complete once [fix-list-flutter-2.md](fix-list-flutter-2.md) is done** (except the nearby list, spec 14, and the engineering blockers B7 and N32 hand-over, which change no design). No further design review round is needed; the engineer checks each item against its acceptance line and the screenshot set.

### Decisions after the nearby and capture round

Made on 2026-11-07 after reviewing main 01ea595 (branches `nearby-screens` and `capture-backends`) and the 47 screenshots of the nearby section (`/tmp/shots-nearby`: phone 400 × 800 and laptop 1100 × 800, light and dark, every state, DE du found and relay), compared with section 14 and the mockup. Questions G1–G12 (nearby), H1–H7 (capture) and E1 are answered in [implementation-questions-flutter.md](implementation-questions-flutter.md); work items in [fix-list-flutter-3.md](fix-list-flutter-3.md). The sections above already contain these decisions.

- **N48 States without a row of their own (G4, G11).** While the platform is asked, the section shows its label alone. When the search cannot run (`Unavailable`: no multicast, no interface) the section shows "Nothing found" at once, without the 8 s; no new copy (14.2).
- **N49 The 33rd screen (G3).** The first 32 found stay; further screens are ignored until a row leaves. 14.8 point 10 is changed to match 14.3; the built "drop the first found" is a defect (fix list 3, A5).
- **N50 Announcement and browse per platform (G1, G2, G6).** Receiver-side expiry (refresh every 10 s, row dropped 30 s after the last received refresh) is accepted on desktops. On Android and iOS a refresh answered from the system's cache must not keep a row alive: either the record lifetime is ≤ 30 s (patched or newer `mdns-sd`) or the bridge proves the screen answered; acceptance ≤ 35 s after a display loses power (14.8 point 5). Announcing from the core (`mdns-sd`) is accepted for Android TV, Android tablets and computers in the display role, provided Android holds a `WifiManager.MulticastLock` while announcing. iPad in the display role must announce with `NWListener` (the multicast entitlement is not counted on); the Apple TV announcement must carry the 14.8 layout, or Apple TVs never appear in the lists. macOS browses with `NWBrowser` (the iOS bridge), so its Local Network permission gives "Not asked yet" / "Not allowed" as on iOS (native-apps.md 4.1). Until then (test builds only) iPads and Apple TVs are found by code only and macOS shows "Nothing found" when refused. None of this changes what is drawn.
- **N51 Stale code and the relay page (G5, differences).** After `room_not_found` the whole list is cleared and refills (accepted); the `nearby.gone` line stays until the next tap on the page or until the page is left; a refilling list does not remove it. The rows frozen while the relay page is open, shown disabled while the join answers, are accepted.
- **N52 Accepted as built (G7–G10, G12, differences).** `start.or` and `common.cancel` as new Flutter keys. German `InfoPlist.strings` with the du text (the shipped set); a Sie build takes `iosLocalNetworkUsage` from the Sie set. A confirmed relay is never "last used". Locked row: accessible name "{name}, {mode}", not focusable. Connect stays enabled (5.3 wins over the mockup). Card edge `#E2E4E9` / `#2B2F38` is the card value of `p-divider` (3.3). The relay page in the generic layout of 7.2 (icon `info` in `p-warn`) is the specified page.
- **N53 `nearby.via` keeps "via" with the host.** No-break space inside the key in all three sets (14.3, 14.10).
- **N54 While a row connects.** Connect and `app.scanQr` are disabled, the name and code fields read-only; the screenshots showed Connect disabled and Scan still active (14.4).
- **N55 Desktop picker behaviour (H1, H3, H5, H6).** The picker opens on every share on every desktop (portal restore tokens stay off; no "remember" option; the measurement point "Wayland dialog once per session" is superseded). Windows: the picker is owned by the Comet Cast window (its own `HWND` passed through `cc-ffi`), never by whatever window is in front. Sources are what the system offers (one screen or one window on Linux and macOS, the Windows picker's own list); `what.screenLaptop` stays. A minimised shared window: the display keeps the last picture, no timeout, no message on either side (as browser sharing). A closed shared window or a share ended in the system UI: presentation stopped like Stop (`PresentingStopped`), no page, no toast. Cancel and a refused permission: back to "What to show", no page (native-apps.md 8).
- **N56 Screen-sharing page `error.capture` (H4, the missing portal; replaces N33 and the unsupported codec page).** When the device cannot share at all, one page in the error layout of 6: icon (i) in grey, `error.capture.title`, "why" by code (`errors.capture_unavailable`, `errors.capture_os_old` for macOS before 14, `errors.capture_no_portal` for Linux without the ScreenCast portal or PipeWire, `codec.unsupported` from 5.8), "what now" `error.noScreen.nowNoApp`; buttons Show photo or file (primary) → file page, Show a link → link page, Back → "What to show"; the person stays connected. The core reports the two new codes like `capture_unavailable` (non-fatal, the join stays). Copy in 11.9.
- **N57 Linux packaging and the portal (H2, H7).** The app must start on a machine without libpipewire (X11 users included): load PipeWire at run time, or declare it as a dependency in every Linux package format shipped; without it Screen on Wayland shows `error.capture` with `errors.capture_no_portal`. The app registers its application id with the portal (desktop file, host app registry), so the system picker names "Comet Cast". `capture_info` is engineering only and may go with the next bridge change.
- **N58 Screenshots.** Taken without the debug banner (`debugShowCheckedModeBanner: false`); laptop start pages with `app.beDisplay` (Windows also `app.miracast`) as the app shows them; DE du for every nearby state with text; the `error.capture` page in its four "why" variants.
- **N59 Design status.** With [fix-list-flutter-3.md](fix-list-flutter-3.md) part A done, the nearby section and the desktop capture UX are design-complete. Part B (platform announcement and browse, picker owner window, PipeWire packaging) and the manual test plans are engineering work that changes nothing that is drawn. No further design review round is needed.

### Decisions after the third Flutter round

From the review of main f99ba94 and the 479 screenshots in `/tmp/shots-flutter4`; answers to [implementation-questions-flutter.md](implementation-questions-flutter.md) J; work in [fix-list-flutter-4.md](fix-list-flutter-4.md).

- **N60 Links under Connect (J1, difference 6).** 12, as N37 and the acceptance of fix list 3 A4: the first 44-high tap area starts 12 below Connect (text about 34 below), the second touches it (lines 44 apart). The "14" in the "What" of A4 was a writing error. Accepted as built.
- **N61 Laptop columns of the app start page with the nearby card (differences 1, 2).** The two columns share the content width after the 32 gap as **4 : 5** (left : right; at the 744 maximum about 280 : 352), instead of halves. The left column is never narrower than 260; fields, Connect and the links fill or follow it as before. Without the card (chip, policy) the layout is unchanged (one column).
- **N62 Line breaking in rows, subtitles and running text (differences 1, 2).** Text breaks only at spaces and after a hyphen. A word is split inside only when that word alone is wider than the whole line (the last resort, no hyphen added). Host names inserted into running text (`{host}` in `nearby.via`, the relay page body, `{dom}` in sentences) may also break after each "." (the app adds a zero-width break opportunity U+200B after every dot of the inserted value; never in the ARB text, never in the mono host line or a field). The no-break space of N53 stays, so "über cast." stays together and the rest follows. **Row title column minimum 160:** when the row is so narrow that the text column (row width − icon 22 − gaps − code) would be under 160, the code moves to its own line under the subtitle (same style, left-aligned with the title, spinner likewise); at the widths of 7.3 and N61 this does not happen.
- **N63 Heading wraps (difference 3).** `app.welcome` (and every page heading) wraps at word boundaries, no scaling, no ellipsis; "Willkommen bei Comet / Cast" in the left column is accepted. The columns stay top-aligned; the form moves down with the heading.
- **N64 A connecting row keeps its size.** The right slot of a row is as wide as the code in every state: in `nearby.connecting` the spinner (20) sits at the slot's right edge, so the title does not re-wrap and no row below moves when the connect starts or ends (the DE shots showed the row shrinking from two lines to one).
- **N65 Buttons of `error.capture` (J8, difference 4).** Show photo or file primary (initial focus), Show a link and Back secondary, in this order; laptop one row (7.4), phone stacked. Accepted as built.
- **N66 The unsupported codec state (difference 5).** Showing `error.capture` inside the codec page's widget tree is accepted while it looks and behaves as the page of 6 (Back and system Back → "What to show", connected, focus on the primary).
- **N67 Desktop picker and `COMETCAST_CAPTURE_DISPLAY` (J2, J3).** The system picker on every tap is the behaviour (N55). The forced X11 display through `COMETCAST_CAPTURE_DISPLAY` is accepted as a development and test switch: no UI, not in user documentation, no effect when unset. The Windows owner window found in Rust is accepted; the acceptance stays "the picker is owned by and in front of the Comet Cast window" (test plan).
- **N68 Linux desktop entry and icon (J4).** Run-time loading of `libcc_capture_pw.so` is accepted (N57). Whatever Linux package format is shipped later carries the same desktop entry and icons. The entry gets: `Icon=de.cometcast.cometcast`; `GenericName=Screen sharing`, `GenericName[de]=Bildschirm teilen`; `Comment=Show your screen, files and links on the room screen`, `Comment[de]=Bildschirm, Dateien und Links auf dem Raumbildschirm zeigen` (infinitive: right for du and Sie); `Categories=Office;Presentation;`; `Keywords=cast;present;presentation;screen;display;miracast;`, `Keywords[de]=präsentieren;Präsentation;Bildschirm;teilen;Beamer;`. Icon files (unchanged copies, assets/logo README): `cometcast-app-icon.svg` → `share/icons/hicolor/scalable/apps/de.cometcast.cometcast.svg`; `cometcast-app-icon-32.png`, `-64.png`, `-512.png` → `hicolor/{32x32,64x64,512x512}/apps/de.cometcast.cometcast.png`. The GTK window uses the same icon name. Not `cometcast-logo-*` (those are wordmarks for backgrounds) and not the round icon (Android only).
- **N69 Platform items without a drawn change (J5, J7, J9, J10, difference 7).** The vendored `mdns-sd` with the TTL setters is accepted until an upstream release has one. Apple TV without a room name: accepted for test builds only; the Apple TV display uses the setup wizard of 4.1 (native-apps.md 1), and once it sets a room name the announcement carries it (14.8). Expiry, multicast lock and the picker owner are checked by the device test plans, not by screenshots; those plans must pass before release (they are not a design review). The `cc-office` clippy failure is not a design matter.
- **N70 macOS permission text in German (J6).** macOS gets `de.lproj/InfoPlist.strings` with the du text of `ios.localNetworkUsage`, as iOS (N52); a Sie build takes the Sie text.
- **N71 Screenshots.** The gallery shot `presenter-*-error-capture` (the error enum loop with the placeholder "why" "The server refused this request.") is not a state of the app: leave `ErrorKind.capture` out of that loop; the real variants are `capture-*` and `error-capture-unavailable`. Add `nearby-laptop-light-de-found` and `-de-connecting` at a 720-wide window and `nearby-phone-light-de-found` at 360 wide.
- **N72 Design status.** With [fix-list-flutter-4.md](fix-list-flutter-4.md) done, the Flutter apps (display, presenter, nearby, desktop capture UX) are design-complete for this scope. The device test plans remain engineering checks. No further design review round is needed for these surfaces.

## 14. Screens on the same network (apps only)

Added 2026-11-07. Clickable mockup: https://claude.ai/artifact/2o5vZaZk9zuzBkPcYeKtWF (DE du / EN, phone, iPhone, laptop, light and dark). Where the mockup and this section disagree, this section wins.

Goal: a person who opens the **app** on a phone, tablet or computer can connect to a screen on the same network without typing a code. This makes `start.appLaptop` true. Browsers cannot see the local network, so the web presenter pages (5.2) do not change.

### 14.1 Where it lives

On the **app start page** (5.3), as one section between the name field and the code. There is no separate screen, no sheet, no tab. The code (scan button and code field) always stays on the same page, so the way that always works is never more than one glance away.

New order of the app start page (5.3 otherwise unchanged):

| Phone layout (< 720) | Laptop layout (≥ 720, spec 7.4) |
| --- | --- |
| 1. Brand row · 2. heading `app.welcome` · 3. name field `start.nameLabel` · 4. **section `nearby.title`** · 5. divider `start.or` · 6. secondary `app.scanQr` (devices with a camera) · 7. code field `start.codeLabel` · 8. primary `app.connect` at the bottom | Left column: brand, heading, name field, code field, `app.connect` (action row follows the content, 7.4), `app.beDisplay` link. Right column: the section `nearby.title` in a card (`p-surface`, 1 px `p-divider` `#E2E4E9` / `#2B2F38`, radius 14, padding 16), as the right card of the laptop start page (5.2). Columns 4 : 5 after the 32 gap, left at least 260 (decision N61) |

- The name field moves **above** the section, because one tap on a screen connects with that name (14.4). An empty name is allowed (the display shows `common.noName`), as today.
- `app.scanAgainBody` is not shown on this page when the section is shown (it speaks of scanning "once more", which is wrong for someone who opens the app from the home screen). It stays where the code chip is missing after a store install (iOS, 5.3), that is when the section is not shown.
- The section is **not shown** (and no search runs) when: the page already has a code (code chip from a link, the install referrer or a scan: the person already has a target); the administrator's policy forbids announcements or locks the server (`docs/discovery.md`: `forbid_announcement`, `forbid_user_override`); the page is a display (TV, display role); the platform is the web.

### 14.2 The section

Section label `nearby.title` (13/700 `p-fg2`, the label style). Below it exactly one of these states:

| State | When | Content |
| --- | --- | --- |
| (Asking the platform) | Between showing the page and the platform's first answer (milliseconds; longer on iOS) | The section label alone, nothing below it (decision N48) |
| Not asked yet | The platform needs a local-network permission (iOS, iPadOS; any platform that adds one later) and it was never asked | Secondary compact button with the search icon: `nearby.search`. Tapping it starts the search, which makes the system ask (8 below; spec 8 rule: a system dialog only as the direct result of the person's choice). After "Allow" the search runs at once and starts by itself on every later visit |
| Searching | Search runs, nothing found yet, less than 8 s | One placeholder row: option-row size, 1 px **dashed** `p-border`, spinner (20, 5.4) + `nearby.searching` in `p-fg3`. No other text |
| Found | At least one screen | The list (14.3). No "searching" text: the list updates silently |
| Nothing found | Search ran 8 s and found nothing. The search continues. **Also at once** when the search cannot run at all on this network (no multicast, no usable interface: the core's `Unavailable`; decision N48); it is tried again when the network changes or the page is shown again | `nearby.none` (15, `p-fg2`), below it `nearby.noneWhy` as small print. If a screen appears later, the list replaces the text |
| Paused | Phones and tablets: the search ran 60 s (14.5) | Small print `nearby.paused` and the compact button `nearby.search`. The list is cleared when the search stops (an entry that is no longer watched could be stale) |
| Not allowed | The system permission was refused (iOS, iPadOS; macOS 15+ once it browses with `NWBrowser`, decision N50) | Info icon 18 in `p-warn` + small print `nearby.denied`; below it a text link `nearby.allow` (style of `start.beDisplay`) that opens the app's page in the system settings. iOS: `UIApplication.openSettingsURLString` |
| No local network | The device has no Wi-Fi or Ethernet connection (mobile data only) | wifi-off icon 18 in `p-warn` + small print `nearby.noWifi`. No search runs; it starts by itself when Wi-Fi comes |

Compact button (new size, only here): secondary look (`p-surface`, 1 px `p-border`, radius 10), min height 44, padding 0 16, 15/700 `p-fg`, icon 18, content width on every device.

### 14.3 One row per screen

Each found screen is one button in the option-row look of 5.5 (min height 58, laptop 50; radius 12; 1 px `p-border`; padding 8 14; gap 12). In the laptop card the row fill is `p-bg` (the card is `p-surface`). Hover (pointer): border `p-primary`. Keyboard: Tab reaches each row in list order, Enter/Space taps, focus outline 2 `p-primary`, offset 2 (6.3 of native-apps.md).

| Part | Content |
| --- | --- |
| Icon (22, `p-primary`) | `screen` |
| Title (17/700 `p-fg`, line height 1.25, at most **2 lines**, then "…") | The announced name, cleaned like `room_name` (decision 13.2: control, bidi, zero-width, invisible and private-use characters removed, white space collapsed, cut at 32 characters) and shown in quotes with `common.quoted`. Empty after cleaning → `common.noName` without quotes, 17/500 `p-fg2`. Breaks at spaces and after hyphens only, never inside a word that fits a line by itself; text column at least 160, else the code moves under the subtitle (decision N62) |
| Subtitle (13/400 `p-fg3`, wraps) | `nearby.open` or `nearby.approval`; when the screen's server is not the default relay, a policy server or a DNS-SRV server: ` · ` + `nearby.via` (`{host}` = the announced server's host name, cleaned, no port unless it is not 443). The space inside `nearby.via` is a no-break space (U+00A0), so a wrap falls after "·" and "via" / "über" stays with the host (decision N53). A host wider than the line breaks only after one of its dots (decision N62) |
| Right (15 mono 700, letter-spacing .06em, `p-fg2`) | The screen's current code as `XXX XXX`. It is the same code the screen shows large, so the person can check that the row is the screen in front of them, and two equal names stay apart. The slot keeps this width while the row connects (spinner at its right edge), so nothing re-wraps (decision N64) |

**Locked screens** stay in the list, so the person knows the screen is there: `locked` icon in `p-fg3`, title in `p-fg3`, subtitle `nearby.locked`, no code (it is not announced), **dashed** border, not focusable and not tappable (`aria-disabled`). Accessible name "{name}, {mode}" (for example "„Vereinsheim“, Gerade gesperrt"): `nearby.aria` without the code part, no new key (decision N52). It becomes a normal row as soon as the screen announces Open or Approval on again.

Rules:

- **Never merge rows by name.** Two screens called „Raum 204" are two rows; the code (and `nearby.via` when the servers differ) tells them apart. Rows are merged only when they are the same screen (same instance id, 14.8).
- **Order:** in the order they were first found; new rows are added **at the end**. Rows never re-sort while the page is shown (a row must not jump away from a finger). When the page is opened again, the list starts empty and fills in the order of discovery.
- A row that **appeared, moved or changed within the last 500 ms ignores a tap** (a row above it may just have left).
- A row leaves **at once** when its screen says goodbye or its announcement expires (14.8), without animation. No "gone" toast for rows that leave by themselves.
- At most **32 rows**: the first 32 found stay; a 33rd and later screens are not shown and not stored until a row leaves; then the next one is added at the end when its announcement next arrives (no row ever leaves because another one came; decision N49). No "and more" line. The page scrolls as a whole; the list has no scroll area of its own.
- Accessible name of a row: `nearby.aria` ("„Raum 204“, Code P4X 9TR, Mit Freigabe"), with the full cleaned name (not cut at 2 lines).
- `prefers-reduced-motion`: the spinner stands still (3.6).

### 14.4 Tapping a row

| Situation | What happens |
| --- | --- |
| Server is the default relay, a policy server, a DNS-SRV server, or a server this device was confirmed for before | The search stops. The row shows the spinner (20) instead of the code and `nearby.connecting` as subtitle; all other rows are disabled (55 % opacity). While it connects nothing else may start a join (decision N54): Connect and `app.scanQr` are disabled (disabled primary / secondary look of 3.3 and N23), the name and code fields are read-only (look unchanged, Enter does nothing); everything returns when the page comes back. The app joins with the announced code and the name from the field. Then the known flow: Open → "What to show" (5.5); Approval on → waiting page (5.4) and the join request on the screen (4.4); blocked device → `error.denied`; `room_full` → `error.occupied`; every other failure → the error pages of 6 |
| Server is anything else (announced only) | First the **relay confirmation page** (7.2, presenter variant) with the body `relay.confirm.bodyNearby` and the secondary button `common.cancel` (not `relay.confirm.cancel`: there is no default relay to fall back to here). Cancel → back to the start page, nothing was sent. "Use this relay" → the host is remembered as confirmed on this device, then as above (the start page shows the rows as they were when the relay page opened, frozen and disabled, until the join answers; decision N51). The announcement alone never makes a server trusted (`docs/discovery.md`, trust policy). A confirmed host is never saved as "last used" or put in any field (14.8 point 11) |
| The screen went Locked meanwhile (`room_locked`) | `error.locked` (6), as with a typed code |
| The code is no longer valid (`room_not_found`: the screen ended its session or went off between announcement and tap) | No error page. Back to the start page. The search starts again from an empty list (14.8 point 9), so **all** rows go and the list fills again within about a second (the Searching placeholder meanwhile; the screen may come back with its new code). Under the section one line of short status text in `p-error` (13/700): `nearby.gone`. The line stays while the list refills and goes away only with the next tap on the page or when the page is left (decision N51) |

There is no extra confirmation for Open screens: the "What to show" page already comes before anything is shared, and the system capture dialog comes before the screen is shared (5.5).

Withdrawing on the waiting page, Disconnect, the disconnected page and every error page lead back to the start page, which starts the search again.

### 14.5 When the search runs

- Starts when the start page is shown (and is not in a state of 14.1 without the section), when the app comes back to the foreground on that page, and when the device joins a Wi-Fi or Ethernet network.
- Stops when the page is left (any tap on a row, Connect, Scan), when the app goes to the background, and when the device loses the local network.
- **Phones and tablets:** stops after **60 s** on the page → state Paused (14.2). `nearby.search` starts another 60 s.
- **Computers:** runs as long as the page is visible (window not minimised); no time limit.
- No background search, no search on any other page, no notification about found screens.

### 14.6 Platforms and devices

| Device | Notes |
| --- | --- |
| Phone (Android, iPhone) | Phone layout. Rows full width, 58 high (touch target ≥ 44). On a small phone the list pushes the code part down; the page scrolls, Connect stays at the bottom of the content |
| Tablet (Android, iPad) | Phone layout below 720 wide, laptop layout above (native-apps.md 4). Search limit as phones (60 s) |
| Computer (Linux, Windows, macOS) | Laptop layout, list in the right card. Mouse hover, keyboard as 14.3. No scan button |
| TV, Apple TV, any device in the display role | Nothing new is drawn. The display only **announces** itself (14.8) while it shows Idle or Presenting; the join request (4.4) is unchanged. The presenter app does not run on TVs |
| Web (browser presenter and `/display`) | Not available. The web display cannot announce itself either; a web display is found only by its code |

### 14.7 Tokens used

Only tokens of 3.3: `p-bg`, `p-surface`, `p-border`, `p-divider`, `p-fg`, `p-fg2`, `p-fg3`, `p-primary` (icon, spinner, hover border, focus outline, text link), `p-primary-soft` (spinner track), `p-warn` (icons of Not allowed and No local network), `p-error` (the `nearby.gone` line). No new colour. Type and sizes as 3.4 / 3.5; the only new size is the compact button (14.2).

### 14.8 Requirements for the announcement (for the engineers)

What the design needs from the display's announcement and from the apps that read it. The protocol details are the engineers' (`docs/discovery.md`, section 3, extends the existing `_cometcast._tcp` record).

The display (sender of the announcement) must:

1. Announce only while it is a display with a code: in Idle and Presenting. **Not** during setup, in Idle connecting / offline (no valid code), or when its policy forbids announcements.
2. Carry: a layout version; a **stable random instance id** per installation (not the host name, not a hardware id; new after a data reset) so the same screen seen on two interfaces or addresses is one row; the **room name** as set in setup (same cleaning as `room_name`, at most 32 characters; no fallback to the device or host name: none set → empty); the **server** (`host:port` + TLS flag, as today); the **join mode** (`open`, `approval`, `locked`); and, only for Open and Approval on, the **current code** (6 characters, the same as on the screen). If a separate join token is ever used, the code still has to be carried, because the list shows it.
3. While **Locked**: keep announcing name, server and mode `locked`, and carry **no code and no token**.
4. Update the announcement **within 1 s** when the code changes (End, restart, reconnect with a new code), the join mode changes, or the room name changes.
5. Make sure a screen that dies without saying goodbye leaves every list within 30 s (acceptance: ≤ 35 s after the display loses power, on Android, iOS and desktop). The display re-announces every 10 s and the apps drop a row 30 s after its last **received** refresh. A refresh that only comes from a platform cache does not count: where the platform browse (`NsdManager`, `NWBrowser`) cannot tell, the record lifetime itself must be ≤ 30 s (decision N50). Send a **goodbye** (withdraw the record) when the display app stops, goes to the background (native-apps.md 15), loses the server, or enters a state of point 1 where it must not announce.
6. Keep the record small (one DNS-SD TXT record, well under 400 bytes) and send nothing else (no person names, no presenter information, no "busy" flag).

The apps (receivers of the announcement) must:

7. Treat every field as untrusted: clean the name (14.3), check the server like any announced server (`docs/discovery.md`: private-range IP literals only, TLS rule), drop a record whose code is not 6 characters from the code alphabet (5.2), drop records of another layout version.
8. Merge by instance id; when two different instance ids claim the same name, show both.
9. Remove a row when the record is withdrawn or its lifetime has passed without refresh; never show a cached entry from an earlier search.
10. Keep at most 32 screens: the first 32 found; ignore further ones until one leaves (14.3, decision N49).
11. Never pick a screen by itself, never pre-fill the code field from an announcement, and never save an announced server as "last used", confirmed or not; "Use this relay" only adds the host to the confirmed servers of this device (14.4, decision N52).
12. Search only as 14.5 says; use the platform's browse API (NWBrowser on Apple, NsdManager on Android, the existing mDNS backend on desktops), so iOS shows its local-network prompt and Android needs no extra permission; map a refused permission (`kDNSServiceErr_PolicyDenied`) to the state Not allowed.

### 14.9 Not included

- Discovery in the browser or on the web display (impossible), and any "nearby" list on the display itself.
- Automatic connection to the only screen found, "last used screen" shortcuts, favourites, renaming or hiding screens in the list.
- Showing screens from other networks, Bluetooth or QR-less pairing, or any cloud list of screens.
- A "busy" or "someone is presenting" state in the row (`error.occupied` covers it after the tap).
- Sorting, search field or grouping in the list; a separate "Nearby" screen, sheet or tab.
- A setting on the display to stop announcing (owner question, 12).

### 14.10 Copy

| Key | EN | DE du (shipped) | DE Sie |
| --- | --- | --- | --- |
| `nearby.title` | Screens on this network | Bildschirme in diesem Netz | Bildschirme in diesem Netz |
| `nearby.searching` | Searching … | Sucht … | Sucht … |
| `nearby.open` | Connects at once | Sofort verbunden | Sofort verbunden |
| `nearby.approval` | Approval on | Mit Freigabe | Mit Freigabe |
| `nearby.locked` | Locked right now | Gerade gesperrt | Gerade gesperrt |
| `nearby.via` | via {host} (no-break space, N53) | über {host} | über {host} |
| `nearby.connecting` | Connecting … | Verbinde … | Verbinde … |
| `nearby.none` | No screen found here. | Hier wurde kein Bildschirm gefunden. | Hier wurde kein Bildschirm gefunden. |
| `nearby.noneWhy` | Many Wi-Fi networks hide devices from each other. The code on the screen always works. | Viele WLANs verbergen Geräte voreinander. Der Code auf dem Bildschirm geht immer. | same as du |
| `nearby.paused` | Search stopped to save battery. | Suche angehalten, um Akku zu sparen. | same as du |
| `nearby.search` | Search this network | In diesem Netz suchen | In diesem Netz suchen |
| `nearby.denied` | Comet Cast may not search this network. | Comet Cast darf dieses Netz nicht durchsuchen. | same as du |
| `nearby.allow` | Allow in Settings | In den Einstellungen erlauben | In den Einstellungen erlauben |
| `nearby.noWifi` | Join the room’s Wi-Fi to find screens here. | Verbinde dich mit dem WLAN des Raums, um Bildschirme hier zu finden. | Verbinden Sie sich mit dem WLAN des Raums, um Bildschirme hier zu finden. |
| `nearby.gone` | This screen is no longer available. | Dieser Bildschirm ist nicht mehr erreichbar. | same as du |
| `nearby.aria` | {name}, code {code}, {mode} | {name}, Code {code}, {mode} | {name}, Code {code}, {mode} |
| `relay.confirm.bodyNearby` | The screen {name} uses the relay {host}. Only continue if you know and trust its operator. | Der Bildschirm {name} nutzt das Relay {host}. Mach nur weiter, wenn du den Betreiber kennst und ihm vertraust. | Der Bildschirm {name} nutzt das Relay {host}. Fahren Sie nur fort, wenn Sie den Betreiber kennen und ihm vertrauen. |
| `ios.localNetworkUsage` (`NSLocalNetworkUsageDescription`, shown by the system) | Comet Cast looks for screens on the same Wi-Fi so you do not have to type a code. | Comet Cast sucht Bildschirme im selben WLAN, damit du keinen Code tippen musst. | Comet Cast sucht Bildschirme im selben WLAN, damit Sie keinen Code tippen müssen. |

`{name}` is inserted already quoted (`common.quoted`), or `common.noName`. `{mode}` is the subtitle text of the row (`nearby.open`, `nearby.approval`). Reused without change: `start.nameLabel`, `start.or`, `start.codeLabel`, `app.scanQr`, `app.connect`, `common.quoted`, `common.noName`, `common.cancel`, `relay.confirm.title`, `relay.confirm.risk`, `relay.confirm.use`. The existing system prompt text of the Apple TV (`docs/appletv-device-test-plan.md`, B1) is for the display and stays.

## 15. Share sheet, receiving and sending without Comet Cast, Office files, the screen's address, settings and About (round 5)

Added 2026-11-07. Clickable mockup (DE du / EN): https://claude.ai/artifact/CWAeQPZwPvuZUxfvHLEgtM. It was drawn before sections 14 and N48–N69 existed; **where it and this section disagree, this section wins** (15.12 lists the places). Platform details are in [native-apps.md](native-apps.md) section 21; the work items in [fix-list-round5.md](fix-list-round5.md). The owner has not commented; the decisions below are the designer's (owner rule: decide what can be decided).

### 15.1 Decisions

The five questions the mockup left open are N81–N85.

- **N80 Scope.** This round adds: the share entry on Android and iOS/iPadOS (15.3), AirPlay and Miracast receiving on desktop display apps (15.4), sending to Google Cast and AirPlay TVs from the desktop presenter app (15.5), the Office limits line and switch (15.6), the screen's address line in the presenter apps (15.7), the presenter app settings page (15.8) and About with the licence list on every app and a footer line on the web start page (15.9). Nothing else changes. Section 10 and native-apps.md 9 are amended to match.
- **N81 Office files (question 1).** Converted on tap, on the presenting device, by the bundled `cc-office-worker` (office2pdf), for `.docx`, `.pptx`, `.xlsx` only; OpenDocument and legacy files are not offered (the picker filters them; a shared one gets the existing refusal). Platforms: where `office_worker_path()` finds the worker, that is Linux, Windows, macOS and Android; **not iOS/iPadOS** (no child processes) and **not the web**. No dialog before; the fidelity is unmeasured, so while a converted file is shown the presenter's page carries one line with the limits and the way out "Pick my own PDF instead" (15.6). It is **not** labelled Beta (it is a local conversion that the person sees the result of at once), it is **on by default**, and the app settings have a switch to turn it off; off means the picker no longer offers Office files (no error page).
- **N82 iOS share extension (question 2).** The extension cannot rely on opening the app, so it does the whole job itself, like the broadcast extension: its own connection, its own join, the content, the page turning. The sheet stays open as the remote; **"Stop and close" (and closing the sheet) stops showing and leaves**. A screen in Approval on asks the host again for the extension's join (the app is not running and cannot hand its join over; accepted). Android opens the app instead, which keeps the app's existing join (one tap when already connected).
- **N83 Publisher and legal pages (question 3).** About names the publisher, **Schulverwalter GmbH**, and links "Privacy" to `https://cometcast.de/datenschutz/` (English UI: `https://cometcast.de/en/privacy/`) and "Imprint" to `https://cometcast.de/impressum/` (`/en/imprint/`). The store listings use the same privacy address. The apps show these links always (the apps are published by Schulverwalter GmbH, whatever server they use). The web start page shows them only on the cometcast.de deployment; a self-hosted server shows its operator's pages when its `config.json` names them (`privacy_url`, `imprint_url`), otherwise none (15.9). The content of both pages is the owner's (legal text; `docs/website.md` checklist).
- **N84 AirPlay on an Apple TV (question 4).** On Apple TV, AirPlay is tvOS's own receiver and bypasses Comet Cast. The Apple TV app therefore shows **nothing** about AirPlay: no idle line, no switch, and the join mode texts stay about Comet Cast only. In the desktop app's list of TVs without Comet Cast (15.5), a Google Cast or AirPlay receiver whose IP address is also the address of a Comet Cast screen in the list is **hidden**, so people pick the Comet Cast row, which honours the join mode. On desktop display apps (15.4), where Comet Cast runs the AirPlay and Miracast receivers itself, the join mode does apply to them and Locked stops them.
- **N85 The screen's address (question 5).** `{dom}` is the **join address**: where a browser reaches the web app for this server, not the relay's host. A self-hosted server serves the web app itself, so its join address is its host (unchanged). The public relay `relay.cometcast.de` deliberately serves no web app; its join address is **`cometcast.de/app`** (the static web app). So for the built-in relay the idle screen shows `cometcast.de/app`, the QR code encodes `https://cometcast.de/app/{CODE}`, and the app links and the install referrer move to that form (native-apps.md 21.6). Where `{dom}` already ends in `/app`, the text `{dom}/app` (for example in `idle.appLine`) is written as `{dom}`. In the presenter apps, a person who types the code sees under the code field which address the app will use, "via {dom} · Change", and can type the address the screen shows (15.7); there is no `wss://` field anywhere. The web keeps its rule: the page uses the server it was loaded from (on cometcast.de: the built-in relay); a `?server=` link still gets the relay confirmation of 7.2 (decision 37; the mockup's "does nothing in the browser" is not built).
- **N86 Miracast belongs to the display role.** Receiving Miracast is a display feature: Windows display apps get a switch in Settings (15.4). The presenter-side Miracast page and the text link `app.miracast` (N18, N39) are **retired** once the display switch has passed its device test (fix list round 5, item F9); until then they stay as built.
- **N87 Joining by AirPlay and Miracast follows the join mode.** Open: the device gets in with the code the screen shows (PIN dialog, no question). Approval on: the request dialog first (variant of 4.4), then the PIN. Locked: the receivers stop announcing; a running mirror stays until it ends ("Connected stay"). A PIN is **always** required, so only someone who sees the screen can connect (Miracast reaches beyond the network). This replaces the 10-minute "allow foreign devices" window of `cc-airplay` (`FOREIGN_WINDOW`): the switch is the permission, the join mode the admission.
- **N88 One picture at a time; Comet Cast first.** While a Comet Cast presenter presents, new AirPlay and Miracast pairings are refused without any dialog (the device shows its own failure). While AirPlay or Miracast shows, a Comet Cast join request still appears, and when a Comet Cast presenter starts presenting, the AirPlay or Miracast session is ended (toast `toast.rxEnded`). Reason: only Comet Cast has the host's approval rules and End.
- **N89 Beta.** AirPlay and Miracast receiving and sending to Google Cast and AirPlay TVs are **Beta**: each is **off by default**, switched on by a switch whose title carries the Beta chip (15.10), and every row that names such a way says "· Beta". The idle line (15.4) does not say Beta (it is for the people in the room). Which features are Beta is one table in the code (`lib/src/beta.dart`); when a feature passes its device test plan, the engineers remove it from the table (the chip and the "· Beta" go; the default stays off until the designer changes it). No other feature may use the chip.
- **N90 Presenter app settings.** A small page in the presenter role (15.8): the Office switch (where conversion exists), the TV switch (desktops), and About. **No appearance choice**: presenter pages keep following the device (3.3; the mockup's "Aussehen" row is not built). Where neither switch exists (iOS/iPadOS), the entry opens About directly.
- **N91 D2 amended.** The display's Settings hold join mode and appearance, **plus**, on desktop display apps only, the "Without the app" section (AirPlay, Miracast), and in the header an About button. Nothing else: no role switch, no server, no room name (section 10 stays).
- **N92 About content.** Version (the core's version, `app_version()`), "Free software under the GPL-3.0", the publisher, links to the source of this version, privacy and imprint, and the licence list read from `THIRD_PARTY_LICENSES.md` and `THIRD_PARTY_NOTICES.md`, bundled with every app as assets (15.9, native-apps.md 21.7). Parts with their own notice come first (Cisco OpenH264, UxPlay where shipped, Atkinson Hyperlegible, the Office fonts), a search filters everything.
- **N93 Sending to TVs without Comet Cast.** Only in the desktop presenter app (Linux, Windows, macOS); phones have this in the system, the browser cannot. The TVs are rows in the nearby card of the start page (14), under their own label, shown only when the switch is on and at least one was found. Only what the way can do is offered: Google Cast the screen; AirPlay the screen (as a video, a few seconds behind), a photo and a video link. TVs that only accept mirroring are not listed (real AirPlay mirroring needs FairPlay, which the project does not use).
- **N94 Android share target is the app.** `ACTION_SEND` with one item opens the app's share page (15.3); `ACTION_SEND_MULTIPLE` is not registered. Office types are registered only in builds that ship the Office worker.
- **N95 Mockup not redrawn.** The deviations in 15.12 are listed for the engineers; the mockup stays as published.
- **N96 Design status.** With the items of [fix-list-round5.md](fix-list-round5.md) done and the device test plans of AirPlay, Miracast, Cast and the AirPlay sender passed, these surfaces are design-complete. Screenshots per 15.11; no further design review round is needed unless a device test changes what can be offered.

### 15.2 Who gets what

| Surface | Android phone / tablet | iPhone / iPad | Linux / Windows / macOS (presenter) | Linux / Windows / macOS (display role: "mini-PC") | Android TV, Apple TV, tablet in display role | Web |
| --- | --- | --- | --- | --- | --- | --- |
| Share entry (15.3) | yes (opens the app) | yes (share extension, works alone) | no | no | no | no |
| AirPlay receive (15.4) | no | no | no | yes, where UxPlay is shipped (`airplay_receive_available()`) | no (Apple TV: tvOS's own AirPlay, N84) | no |
| Miracast receive (15.4) | no | no | no (page retired, N86) | Windows only, where the adapter can | no | no |
| Send to Google Cast / AirPlay TVs (15.5) | no | no | yes (Beta switch) | no | no | no |
| Office conversion (15.6) | yes (worker in the APK) | no | yes | n/a | n/a | no |
| Address line (15.7) | yes | yes | yes | n/a | n/a | no (page uses its own server) |
| App settings (15.8) | yes | About only | yes | n/a (display Settings) | n/a (display Settings) | no |
| About (15.9) | yes | yes | yes | in display Settings | in display Settings (Apple TV: SwiftUI) | footer line on the start page |

"Mini-PC" in this section means a Flutter desktop build in the display role. Tablets, TVs and the web display get none of 15.4.

### 15.3 Share entry (Android, iOS, iPadOS)

**Where it appears.** In the system share sheet of other apps, as "Comet Cast", only for one item of these kinds: image (PNG, JPEG, WebP, GIF, at most 10 MiB), PDF (at most 64 MiB), Office (`.docx .pptx .xlsx`, at most 50 MiB; Android builds with the worker only, N81, N94), a web link (iOS: `public.url` with `http`/`https`; Android: `text/plain`, see below). For any other type (ZIP, video, several items) the entry does not exist; there is never an error afterwards for a type we did not register. Android `text/plain` cannot be filtered by content: if the shared text contains no `http://` or `https://` address, the page shows `share.noLink`.

**The share page** (presenter kit, phone layout; Android: a page of the app opened by the intent; iOS/iPadOS: the extension's sheet, the system draws its frame, iPad as a centred sheet):

1. Heading `share.title`.
2. The item card (`p-surface`, 1 px `p-divider` card value, radius 14, padding 14, gap 12): icon 28 in `p-primary` on a 56 disc `p-primary-soft` (`file` for files and images, `link` for links); title 17/700 `p-fg`, the file name shortened in the middle keeping the extension (as 3.5 transfer pill), a link shown as its host + path in 15 mono `p-fg` wrapping anywhere; meta line 13 `p-fg3`: `share.metaPdf`, `share.metaImage`, `share.metaOffice`, `share.metaLink`. While an Office file converts after "Show", the meta line is replaced by `file.converting` (small print, no bar). A refusal (size, damaged file, Office off, no link) replaces the meta line by the existing `sendReject.*` text (or `share.noLink`, `sendReject.officeOff`) in 13/700 `p-error`; then the only action is secondary `common.cancel`, which closes the page (Android: back to the app it came from).
3. **Already connected** (Android only: the app holds a join, waiting or joined, not presenting): primary `share.showOn` (`{room}` quoted; without a room name `share.showHere`), below it secondary `share.otherScreen` (leaves the room, then shows 4). One tap on `share.showOn` presents at once. While the app is presenting, the share page replaces what is shown (Stop, then present; no question).
4. **Not connected:** the app start page parts in the order of 5.3/14: name field `start.nameLabel`, the nearby section (14; iOS extension: only when the Local Network permission was already granted to the app, otherwise the section is not shown; the extension never asks), divider `start.or`, `app.scanQr` (Android only; the iOS extension has no scanner), the code field with the address line of 15.7, primary `share.show` (instead of `app.connect`; same enabling and validation rules). A tap on a nearby row presents directly, as `share.show`.
5. After the join: Open → **Presenting** (5.6) directly, there is no "What to show" (the content is chosen); Approval on → waiting page (5.4), then Presenting; every failure → the error pages of 6, whose primary or Back returns to this share page with the item kept.
6. Presenting: 5.6 with `presenting.file` / `presenting.link`, the PDF page row, `presenting.converted` for Office (15.6). Android: `presenting.stop` → "What to show" (the app continues normally). iOS/iPadOS: the primary reads `share.stopClose`; it stops, leaves and closes the sheet; closing the sheet by swiping does the same.

Name, confirmed servers and the last used server are shared with the app (Android: the same app; iOS: the App Group). The iOS extension writes nothing back except the name.

System Back (Android): on the share page = `common.cancel`; on waiting = withdraw (back to the share page); on Presenting = nothing (5 of native-apps.md). Escape on an iPad keyboard = Cancel / Stop and close.

### 15.4 AirPlay and Miracast on desktop display apps ("Without the app")

Only Flutter desktop builds in the display role (15.2). Everything below uses the display tokens (3.2) on the 960 × 540 canvas.

**Settings dialog (4.3), new parts.**

- Header row: `settings.title` left; right: **About** (secondary button style of the Close button: `info` icon 20 + `about.open`), gap 8, then Close. Focus row [About, Close].
- After the appearance section, section `settings.receive.label`, shown only when at least one way exists on this device. One **switch row** per way (AirPlay first, Miracast second; a way the device cannot do at all is not listed, for example Miracast on Linux and macOS, AirPlay without UxPlay):
  - Row: full card width, min height 50, radius 12, border 2 `--line`, fill `--panel2` (OLED black `#000000`), padding 8 14, gap 12. Left: title 16/700 `--fg` (`settings.receive.airplay` / `.miracast`) followed by the Beta chip (15.10); below it the sub line 15 `--mut` (`settings.receive.airplaySub` / `.miracastSub`, or a state line, below). Right: the display switch (15.10).
  - The whole row is the focusable control (focus ring of 3.5 on the row, no scale change of the switch); OK/Enter/click toggles. Rows are their own focus rows, so the Settings rows become [About, Close] · [Open, Approval on, Locked] · [Dark, Light, OLED black, Automatic] · [AirPlay] · [Miracast].
  - State lines (replace the sub line): `settings.receive.noHw`, `settings.receive.noFeature`, `settings.receive.policy` (switch off and disabled: 40 % opacity, not focusable, the row keeps its place); `settings.receive.failed` (switch stays on; turning it off and on retries); `settings.receive.locked` while the join mode is Locked (switch stays as set; the service is paused).
  - The dialog stays within the canvas: with both rows it is about 480 ref high in EN and DE; it scrolls never.
- Stored per device; default off (N89). Turning a switch on starts that receiver at once with the room name as its name (empty: "Comet Cast"); off stops it and ends a running mirror.

**Idle screen (4.2).** When at least one receiver runs (switch on, started, join mode not Locked), the footer's left column gets a first line above `idle.appLine`: `idle.alsoVia.both`, `.airplay` or `.miracast` (15/700 `--fg`, line height 1.3; `idle.appLine` stays 15/400 `--mut` below it, gap 2). The top row's room name is the name to pick in the device's list; nothing else is added. The line also shows in "Idle, offline" (the receivers do not need the server). It is hidden in Locked, during setup and when no receiver runs.

**Joining.**

| Join mode | AirPlay | Miracast |
| --- | --- | --- |
| Open | Pairing request → PIN dialog at once | Connection request → PIN dialog at once (consent answered by the app, pairing "PIN required") |
| Approval on | Request dialog → Yes → PIN dialog; No → refused | Request dialog with the device name → Yes → PIN dialog; No → refused |
| Locked | Receiver stopped (not announced); a running mirror stays until it ends | Same |

- **Request dialog variant** (4.4 layout, same 1 s guard, same queue as Comet Cast requests, first come first served): name line = AirPlay `request.airplayDevice` (UxPlay does not name the device; no quotes), Miracast the device name cleaned and quoted as 4.4; subtitle `request.subtitleAirplay` / `request.subtitleMiracast`; **no check-number box** and **no "No and block"** (the device cannot be identified reliably). Focus row [Yes, No]. Expiry: the device's own timeout (Miracast 30 s); then the request leaves the queue with `toast.expired`.
- **PIN dialog** (link-card family of 4.5: 600 card, scrim, padding 28 × 34): section label `rx.pin.titleAirplay` / `rx.pin.titleMiracast`; body `rx.pin.body` (Miracast with a name: `rx.pin.bodyNamed`), 16 `--mut`; the PIN in 80 mono 700 `--fg`, letter-spacing .06em (4 digits; a longer Miracast PIN in groups of 4 with one space); button row [Cancel] (`common.cancel`, secondary, initial focus, after the 1 s guard). Cancel or Back refuses the pairing. The dialog closes by itself when the mirror starts, when the device gives up, or after 30 s. Only a PIN from the core's PIN event is ever shown (`docs/airplay.md`). It can appear over Idle and over a Miracast picture; never over setup; it closes Settings like a join request does (4.3).
- While a Comet Cast presenter presents: pairings are refused, no dialog (N88).

**While AirPlay mirrors.** UxPlay draws the picture in its own full-screen window on the display's monitor; Comet Cast draws nothing over it. The display counts as Presenting for wake lock and dimming (D5); the announcement and the code are unchanged. A Comet Cast join request brings the Comet Cast window to the front for the dialog and back after it; a Comet Cast presentation that starts ends the AirPlay session first (N88, toast `toast.rxEnded` with `{way}` = AirPlay). The host cannot press End over the UxPlay window; the mirror ends on the sending device, by switching AirPlay off in Settings, or by a Comet Cast presentation. This is a Beta limitation, stated in `docs/airplay.md`, not on screen.

**While Miracast mirrors.** The picture is drawn by the app in the Presenting state of 4.5 (letterboxed on `#000000`). Name tag line 1 `present.byline` with the cleaned, quoted device name; line 2 `present.via` (`{way}` = Miracast) in the place of the security code. End ends the Miracast session and the Comet Cast session (new code, `toast.sessionEnded`), as for any presentation. When the device stops: Idle, toast `toast.left` with the device name.

**Not included:** AirPlay or Miracast receiving on TVs, tablets, phones or the web display; Google Cast receiving; blocking a single AirPlay or Miracast device; a device list or history; audio-only AirPlay; controls over the UxPlay window; a separate "allow for 10 minutes" switch.

### 15.5 Sending to TVs without Comet Cast (desktop presenter app)

**Switch.** App settings (15.8): `appSettings.otherTvs` with the Beta chip, sub `appSettings.otherTvsSub`; default off. Off: nothing below exists and no Cast or AirPlay search runs.

**In the nearby card (14).** Below the Comet Cast states and rows, when at least one TV was found: gap 14, label `tv.title` (13/700 `p-fg2`, as `nearby.title`), then one row per TV in the row look of 14.3 (min 50 on the laptop layout, `p-bg` fill in the card): icon `screen` 22 `p-primary`; title = the device name cleaned like `room_name` and quoted (`common.quoted`); subtitle `tv.cast` or `tv.airplay` + ` · ` + `beta`; **right slot empty** (there is no code). Accessible name `tv.aria`. No placeholder row, no "nothing found" text, no states of their own: the TV part is silent while searching and absent when nothing was found. The search runs exactly when the nearby search runs (14.5, desktops: while the page is visible). Order, the 500 ms tap guard, the 32-row cap (counted separately), "never merge by name" and leaving at once all as 14.3. Hidden: devices whose IP address is an address of a Comet Cast row (N84), AirPlay receivers that offer neither video playback nor photos, Cast devices without video (audio-only).

**Tapping a TV row.** As 14.4 for a known server: search stops, spinner (20) in the right slot at its right edge with `nearby.connecting` as subtitle (N64), other rows disabled, Connect and the fields locked (N54). No relay confirmation (no server is involved). Then:

- AirPlay receiver that asks for pairing (an Apple TV the first time): **pair page** in the relay-page layout (7.2, presenter variant): icon disc `screen` in `p-primary` on `p-primary-soft`; heading `pair.title`; body `pair.body`; field `pair.label` (4 digits, numeric keyboard, 22 mono, letter-spacing .12em, input rules of the code field: only digits kept, max 4); actions [**Pair** (`pair.go`, primary), **Back** (`common.back`)] (laptop row, primary first); Back and Escape → start page. A wrong code: inline `pair.wrong` (13/700 `p-error` under the field), the field is emptied. The pairing is remembered on this computer (no second time).
- Then **"What to show", TV variant** (5.5 layout): header `what.connectedTo` with the TV's quoted name; only the rows the way can do: Google Cast: Screen (`what.screen`, sub `what.screenTv`). AirPlay: Screen (sub `what.screenAirplay`), Photo (`what.photo`, sub `what.photoSub`), Video link (`what.video`, sub `what.videoSub`). Disconnect = `what.disconnect` (back to the start page). Screen → the codec page (5.8) first where the encoder is missing, then the system picker (N55). Photo → the file page (5.5) with `file.bodyTv` and a picker for PNG, JPEG, WebP only. Video link → the link page (5.5) with `link.hintTv` instead of `link.hint`.
- **Presenting** (5.6): `presenting.screen` / `presenting.file` / `presenting.link`, `presenting.where` with the TV name, **no security code** (there is none), `presenting.stop` → TV "What to show". Nothing of Comet Cast is drawn on the TV: no name tag, no End.
- **Errors:** the TV refuses or the way fails to start → error page `error.tv` (6 layout, icon (i) in grey): heading `error.tv.title`, why `error.tv.whyCast` / `error.tv.whyAirplay`, now `error.tv.now`, button `common.back` → start page. The connection is lost later → the same page with why `error.tv.whyLost`. A link or photo the TV cannot take (core code `tv_unsupported_content`) → inline error `link.tvUnsupported` (13/700 `p-error`) under the field or on the file page; the person stays connected. Capture codes → `error.capture` as for Comet Cast.

### 15.6 Office files: what the person sees

- File page (5.5, apps): `file.bodyApp` exactly when the worker is found **and** the switch is on; otherwise `file.bodyWeb` and the picker offers images and PDF only. While converting: `file.converting` (as built).
- **Presenting a converted file** (5.6; Android share page too): under the heading area, above the PDF page row, one line of small print `presenting.converted` (13 `p-fg2`), and below it the text link `presenting.ownPdf` (15 `p-primary`, underlined, 44-high tap area, as the `app.beDisplay` link). Tapping it stops presenting and opens the file picker for PDF only (the system dialog as the direct result of the tap); cancelling returns to "What to show" (connected). The line shows for the whole presentation of that file, not for PDFs or images.
- Encrypted, legacy-inside-OOXML or damaged files keep the existing inline refusals (`sendReject.officeEncrypted`, `.officeFailed`). The switch off in the share page: `sendReject.officeOff` (15.3).
- iOS/iPadOS: Office types are not in the picker and not in the share extension (N81). Web: unchanged (its picker offers images and PDF only); the mockup's web page "Office files do not work in the browser" is not built.

### 15.7 The screen's address in the presenter apps

Shown on the app start page (5.3) and the share page (15.3) whenever the **code field** is shown (not with a code chip, not on the web). Directly under the code field, one line of small print 13 `p-fg3`:

- `start.via` (`{dom}` = the join address the app will use: the last used server, else the default, as `displayHost`; no-break space inside the key as N53) + ` · ` + text link `start.viaChange` (13/700 `p-primary`, underlined, tap area 44 high).
- Policy locks the server (`forbid_user_override`): `start.via` + ` · ` + `start.viaPolicy`, no link.

**Change** replaces the line in place by: label `start.addrLabel`, a text field (46 high, as 3.5; type url, autocorrect off, placeholder = the current `{dom}` in `p-fg3`), and a row [**Use** (`start.addrUse`, compact button of 14.2), **Cancel** (`common.cancel`, compact)] (laptop: on the field's right in one row if it fits, otherwise below). Use (or Enter): the input is cleaned (`server_for_address`: scheme, path other than `/app`, default port dropped; `cometcast.de` and `cometcast.de/app` mean the built-in relay), then checked (`probe_server`, at most 5 s; meanwhile the button reads `start.addrChecking` and is disabled). OK → the line returns with the new `{dom}`; the address is used for the next join and stored as last used after that join succeeded. Not a Comet Cast server, or not reachable → inline `start.addrError` (13/700 `p-error`) under the field, the field keeps the text. Cancel or Escape → the line as before. A typed address is the person's own choice: no relay confirmation (7.2 is for addresses that come from a link or an announcement). The nearby section is not affected by this choice.

### 15.8 Presenter app settings

**Entry:** on the app first-run page only, at the right end of the brand row (5.3 item 1): the compact button of 14.2 with the `gear` icon 18 and `appSettings.open`; the brand row becomes 44 high, the logo centred in it. Where the page would hold no switch (iOS/iPadOS), the button shows `info` + `about.open` and opens About directly. Not on any other page and not in the display role.

**Page** (presenter kit; phone layout and laptop column as 7.3/7.4): heading `appSettings.title`, then:

1. Switch row `appSettings.office`, sub `appSettings.officeSub` (only where the Office worker is found).
2. Switch row `appSettings.otherTvs` + Beta chip, sub `appSettings.otherTvsSub` (desktops only).
3. Option row (5.5 look) with icon `info`, title `about.title`, subtitle `about.version` → About page.
4. Secondary `common.back` (phone: bottom action area; laptop: follows the content). System Back = Back.

Switch row: option-row look (min 58, laptop 50, radius 12, 1 px `p-border`, `p-surface`, padding 8 14, gap 12); title 17/700 `p-fg`, sub 13 `p-fg3` (wraps); the presenter switch (15.10) at the right; the whole row toggles; focus outline of 6.3. Changes apply at once and are stored on the device.

### 15.9 About

**Presenter apps (page):** heading `about.title`; card (`p-surface`, radius 14, padding 14, gap 6): logo 28 + "Comet Cast" (17/700), `about.version` (15 `p-fg2`, the version in mono 500), `about.licence` (15 `p-fg2`), `about.publisher` (13 `p-fg3`). Below the card three text links in the `app.beDisplay` style, stacked 44-high tap areas: `about.source` (opens `https://github.com/wi-wissen/CometCast/tree/v{version}` in the system browser), `about.privacy`, `about.imprint` (N83). Then section label `about.licences` with small print `about.licencesSub`, the search field (46 high, `search` icon 18 `p-fg3` inside left, placeholder `about.search`), and the list: group label `about.featured`, its rows, group label `about.all`, its rows. Row: option-row look, min 48, padding 8 14; title 15/700 `p-fg` (at most 2 lines, then "…"), subtitle 13 `p-fg3` (the licence). The search filters both groups by name, case-insensitive, as you type; no match: `about.none` (15 `p-fg2`). A row opens the **licence text page**: heading = the entry name, the text in 13 mono 500 `p-fg2`, line height 1.4, selectable, wrapping; secondary `common.back`. The page scrolls as a whole (lazy list); the bottom of About has `common.back`.

**Displays (Settings → About):** a dialog of the link-card family (600 card over the scrim, 4.5), opened from the header button, only over Idle: section label `about.title`; logo 40 + "Comet Cast" 21/700; lines 16 `--mut`: `about.version`, `about.licence`, `about.publisher`, `about.sourceTv` (`{url}` = `github.com/wi-wissen/CometCast` in 16 mono `--fg`), `about.webTv` (`{url}` = `cometcast.de`). Buttons [**Licences** (`about.licences`, secondary, initial focus), **Close** (`settings.close` / `settings.closeRemote`)]. Back = Close, back to Settings with focus on About. **Licences** opens the licence viewer: full canvas on `--bg`; top row (padding as the idle screen) section label `about.licences` left, Close right; below, two columns: the list (300 ref wide; rows 40 high, name 15/700 `--fg` one line with "…", licence 13 `--mut`; group labels `about.featured`, `about.all` in section-label style) and the text pane (rest of the width, `--panel` fill, 1 px `--line`, radius 14, padding 16, text 13 mono `--fg` line height 1.4). D-pad: Up/Down move in the list (the pane shows the focused entry); Right moves focus to the pane (focus ring on the pane), where Up/Down scroll by half a pane and Left returns to the list; Back closes the viewer. Tablets and computers: a search field (as the presenter field, display tokens, 46 ref) above the list; the wheel and touch scroll; TVs have no search. The Apple TV builds the same dialog and viewer in SwiftUI.

**Web:** on the presenter start page (5.2), after the last item, one line of small print 13 `p-fg3`: `about.footer` + ` · ` + links `about.source`, `about.licences` (opens `third-party-licenses.txt` of the same build in a new tab), and, only per N83, `about.privacy`, `about.imprint`. Links in `p-primary`, underlined. The web display gets nothing.

**Content sources** (native-apps.md 21.7): entries are built from the bundled `THIRD_PARTY_NOTICES.md` (featured: the sections "OpenH264 notice", "Atkinson Hyperlegible notice", "Open Screen Library notice" on desktop builds, "PDFium Foxit font notice", "Adobe CMap notice"; UxPlay from its shipped licence file where UxPlay is shipped) and `THIRD_PARTY_LICENSES.md` (featured: "Fonts bundled in the repository (cc-office)" as "Liberation, Carlito, Caladea" where the Office worker is shipped; all: every `####` heading under "Rust licence texts" and "Flutter licence texts", and every row of the npm table as name + SPDX, its text being the SPDX line), plus Flutter's `LicenseRegistry` as one entry `about.flutter`. Featured order: Cisco OpenH264, UxPlay, Atkinson Hyperlegible, Liberation/Carlito/Caladea, then the rest in file order. Licence (subtitle) of an `####` entry = the first non-empty line of its text, at most 60 characters.

### 15.10 Components and tokens

Only tokens of 3.2 and 3.3; no new colour.

| Component | Display (ref px) | Presenter (logical px) |
| --- | --- | --- |
| Switch | track 44 × 24, radius 12, border 2: off `--line` border, `--panel2` fill (OLED `#000000`), knob 16 `--mut`; on `--acc` border and fill, knob `--bg`. Knob moves 120 ms ease (instant with reduced motion). Disabled 40 % | track 44 × 26, radius 13: off 1 px `p-border`, `p-surface` fill, knob 20 `p-fg3`; on `p-primary` fill, knob `p-surface`. 120 ms. Disabled: `p-disabled` track, knob `p-ondisabled` |
| Beta chip | after the title, gap 8: text `beta` 13/700 `--mut`, uppercase, letter-spacing .06em, 1 px `--line` border, radius 999, padding 1 8 | text 12/700 `p-fg2`, uppercase, letter-spacing .06em, 1 px `p-border`, radius 999, padding 1 7 |
| Settings switch row | 15.4 | 15.8 |
| PIN | 80 mono 700 `--fg` (as the join code) | n/a |

Accessibility: a switch row is one control: `Semantics(toggled: on, label: title + ", Beta" where chipped, hint: sub line)`; the Beta chip is not read separately. The PIN dialog is an `alertdialog` labelled by its title; the PIN is read digit by digit. Licence text panes are reachable by screen readers as text.

### 15.11 Screenshots for acceptance

DE du and EN, Light and Dark (display: Dark, Light, OLED black): share page (Android: connected, not connected, Office converting, refusal; iOS sheet: not connected, Presenting with `share.stopClose`); display Settings on Windows with both rows (off, on, `noHw`, `failed`, Locked) and on Linux with AirPlay only; idle line (three variants, and offline); request variants AirPlay and Miracast; PIN dialog both ways; Miracast presenting with name tag; desktop start page with TV rows (Cast, AirPlay, both with Comet Cast rows); pair page with and without `pair.wrong`; TV "What to show" for Cast and AirPlay; `error.tv` three variants; Presenting a converted file; address line (default, policy, field open, checking, error); app settings (desktop, Android); About page, search with no match, licence text page; display About dialog and licence viewer with the pane focused; web start page footer on cometcast.de and on a self-hosted server.

### 15.12 Mockup deviations (this section wins)

| Mockup | Built |
| --- | --- |
| Codes like `K7QM 2W4P` (8 characters) on the default server | 6 characters everywhere (glossary) |
| "If a typed code does not fit the address, the address field opens by itself" | Not built: codes have one format on every server, so a code never tells the server (decision 1). The address line (15.7) is always there to change |
| Presenter app settings with "Aussehen" (System/Light/Dark) | No appearance setting on presenter pages (N90, 3.3) |
| Office off: Office files stay in the list and lead to a page "Bitte als PDF exportieren" | Off: the picker no longer offers Office files; no page (N81) |
| Web: page "Office-Dateien gehen im Browser nicht" | Not built: the web picker offers images and PDF only (15.6) |
| Web: a `?server=` link "has no effect" | The relay confirmation of 7.2 stays (N85) |
| "Recent on this device" file grid in the Android share flow | The browser-variant file page of 5.5 (N16); the share page shows only the shared item |
| AirPlay request names the device ("iPhone von Lena") | `request.airplayDevice`; only Miracast has a name (15.4) |
| Request dialog with "No and block" for AirPlay/Miracast | [Yes, No] only (15.4) |
| Miracast page on the Windows presenter | Retired after the display switch passes (N86) |
| Share sheet with "Recent screens" / one-tap on iOS | iOS always joins on its own connection; one tap only on Android (N82) |
| TVs "next to the code field" as a separate list | Rows under `tv.title` inside the nearby card (15.5, 14) |
| About on the web "via a line below the start page" opening a page | One footer line with links (15.9); no web About page |
| Toasts `Beendet`, `{n} hat beendet` | `toast.left`, `toast.rxEnded`, `toast.sessionEnded` (15.4) |

### 15.13 Copy

`{room}`, `{name}`, `{tv}` are inserted already quoted (`common.quoted`, cleaned) or `common.noName`. New placeholders: `{size}` (file size, one decimal, `MB` / `KB`; DE decimal comma: "1,8 MB"), `{kind}` (Word, PowerPoint, Excel; the same in all sets), `{way}` (AirPlay, Miracast; proper names, the same in all sets), `{version}` (`app_version()`), `{url}`. "same as du" as in 11.6.

| Key | EN | DE du (shipped) | DE Sie |
| --- | --- | --- | --- |
| `beta` | Beta | Beta | same as du |
| `share.title` | Show with Comet Cast | Mit Comet Cast zeigen | same as du |
| `share.metaPdf` | PDF · {size} | PDF · {size} | same as du |
| `share.metaImage` | Image · {size} | Bild · {size} | same as du |
| `share.metaOffice` | {kind} · {size} · shown as a PDF | {kind} · {size} · wird als PDF gezeigt | same as du |
| `share.metaLink` | Link | Link | same as du |
| `share.showOn` | Show on {room} | Auf {room} zeigen | same as du |
| `share.showHere` | Show on the connected screen | Auf dem verbundenen Bildschirm zeigen | same as du |
| `share.otherScreen` | Another screen | Anderer Bildschirm | same as du |
| `share.show` | Show | Zeigen | same as du |
| `share.stopClose` | Stop and close | Beenden und schließen | same as du |
| `share.noLink` | This text has no web address in it. | In diesem Text steht keine Webadresse. | same as du |
| `sendReject.officeOff` | Converting Office files is turned off in the settings. Pick a PDF instead. | Das Umwandeln von Office-Dateien ist in den Einstellungen ausgeschaltet. Wähl stattdessen ein PDF. | Das Umwandeln von Office-Dateien ist in den Einstellungen ausgeschaltet. Wählen Sie stattdessen ein PDF. |
| `idle.alsoVia.both` | Also via AirPlay and Miracast | Auch per AirPlay und Miracast | same as du |
| `idle.alsoVia.airplay` | Also via AirPlay | Auch per AirPlay | same as du |
| `idle.alsoVia.miracast` | Also via Miracast | Auch per Miracast | same as du |
| `settings.receive.label` | Without the app | Ohne App | same as du |
| `settings.receive.airplay` | AirPlay | AirPlay | same as du |
| `settings.receive.airplaySub` | iPhone, iPad and Mac show their screen here. | iPhone, iPad und Mac zeigen hier ihren Bildschirm. | same as du |
| `settings.receive.miracast` | Miracast | Miracast | same as du |
| `settings.receive.miracastSub` | Windows and many Android devices show their screen here. | Windows und viele Android-Geräte zeigen hier ihren Bildschirm. | same as du |
| `settings.receive.noHw` | This Wi-Fi adapter cannot receive Miracast. | Dieser WLAN-Adapter kann kein Miracast empfangen. | same as du |
| `settings.receive.noFeature` | Windows needs the optional feature “Wireless Display” for this. | Windows braucht dafür die optionale Funktion „Drahtlose Anzeige“. | same as du |
| `settings.receive.policy` | Switched off by the administrator of this device. | Vom Administrator dieses Geräts ausgeschaltet. | same as du |
| `settings.receive.failed` | Could not start. Switch it off and on again, or restart the app. | Konnte nicht starten. Schalte es aus und wieder ein oder starte die App neu. | Konnte nicht starten. Schalten Sie es aus und wieder ein oder starten Sie die App neu. |
| `settings.receive.locked` | Paused while Locked is on. | Pausiert, solange „Gesperrt“ gilt. | same as du |
| `request.airplayDevice` | An AirPlay device | Ein AirPlay-Gerät | same as du |
| `request.subtitleAirplay` | wants to mirror via AirPlay | möchte per AirPlay spiegeln | same as du |
| `request.subtitleMiracast` | wants to mirror via Miracast | möchte per Miracast spiegeln | same as du |
| `rx.pin.titleAirplay` | AirPlay code | AirPlay-Code | same as du |
| `rx.pin.titleMiracast` | Miracast code | Miracast-Code | same as du |
| `rx.pin.body` | Enter this code on the device that wants to mirror. | Gib diesen Code auf dem Gerät ein, das spiegeln möchte. | Geben Sie diesen Code auf dem Gerät ein, das spiegeln möchte. |
| `rx.pin.bodyNamed` | Enter this code on {name}. | Gib diesen Code auf {name} ein. | Geben Sie diesen Code auf {name} ein. |
| `present.via` | via {way} | über {way} | same as du |
| `toast.rxEnded` | {way} ended | {way} beendet | same as du |
| `tv.title` | Without Comet Cast | Ohne Comet Cast | same as du |
| `tv.cast` | Google Cast | Google Cast | same as du |
| `tv.airplay` | AirPlay | AirPlay | same as du |
| `tv.aria` | {name}, {way}, Beta | {name}, {way}, Beta | same as du |
| `what.screenTv` | Whole screen, live | Ganzer Bildschirm, live | same as du |
| `what.screenAirplay` | Whole screen, a few seconds behind | Ganzer Bildschirm, einige Sekunden verzögert | same as du |
| `what.photo` | Show a photo | Foto zeigen | same as du |
| `what.photoSub` | The TV shows it until you stop | Der Fernseher zeigt es, bis du beendest | Der Fernseher zeigt es, bis Sie beenden |
| `what.video` | Show a video link | Video-Link zeigen | same as du |
| `what.videoSub` | The TV loads and plays the video itself | Der Fernseher lädt und spielt das Video selbst | same as du |
| `file.bodyTv` | A photo from this device. It appears on the TV as soon as you pick it. | Ein Foto von diesem Gerät. Es erscheint auf dem Fernseher, sobald du es auswählst. | Ein Foto von diesem Gerät. Es erscheint auf dem Fernseher, sobald Sie es auswählen. |
| `link.hintTv` | The TV loads the video itself. Only direct links to a video file or an HLS stream work. | Der Fernseher lädt das Video selbst. Es gehen nur direkte Links auf eine Videodatei oder einen HLS-Stream. | same as du |
| `link.tvUnsupported` | The TV cannot play this. Use a direct link to a video file, or a JPEG, PNG or WebP photo. | Das kann der Fernseher nicht abspielen. Nimm einen direkten Link auf eine Videodatei oder ein Foto als JPEG, PNG oder WebP. | Das kann der Fernseher nicht abspielen. Nehmen Sie einen direkten Link auf eine Videodatei oder ein Foto als JPEG, PNG oder WebP. |
| `pair.title` | Pair with {tv} | Mit {tv} koppeln | same as du |
| `pair.body` | The TV now shows a four-digit code. Enter it here. This is needed only once. | Der Fernseher zeigt jetzt einen vierstelligen Code. Gib ihn hier ein. Das ist nur einmal nötig. | Der Fernseher zeigt jetzt einen vierstelligen Code. Geben Sie ihn hier ein. Das ist nur einmal nötig. |
| `pair.label` | Code from the TV | Code vom Fernseher | same as du |
| `pair.go` | Pair | Koppeln | same as du |
| `pair.wrong` | That code was not right. The TV now shows a new one. | Der Code stimmte nicht. Der Fernseher zeigt jetzt einen neuen. | same as du |
| `error.tv.title` | {tv} does not accept the connection | {tv} nimmt die Verbindung nicht an | same as du |
| `error.tv.whyCast` | Google Cast is still Beta in Comet Cast and does not work with every TV. | Google Cast ist in Comet Cast noch Beta und geht nicht mit jedem Fernseher. | same as du |
| `error.tv.whyAirplay` | AirPlay is still Beta in Comet Cast and does not work with every TV. | AirPlay ist in Comet Cast noch Beta und geht nicht mit jedem Fernseher. | same as du |
| `error.tv.whyLost` | The connection to the TV was lost. | Die Verbindung zum Fernseher ist abgebrochen. | same as du |
| `error.tv.now` | If you can, install Comet Cast on the TV. Then the code on the screen works. | Wenn möglich, Comet Cast auf dem Fernseher installieren. Dann geht es mit dem Code auf dem Bildschirm. | same as du |
| `presenting.converted` | Converted to PDF. Fonts, headers and footers may differ; animations are missing. | Als PDF umgewandelt. Schriften, Kopf- und Fußzeilen können abweichen, Animationen fehlen. | same as du |
| `presenting.ownPdf` | Pick my own PDF instead | Lieber eigenes PDF wählen | same as du |
| `start.via` | via {dom} (no-break space, as N53) | über {dom} | same as du |
| `start.viaChange` | Change | Ändern | same as du |
| `start.viaPolicy` | set by your IT | von deiner IT festgelegt | von Ihrer IT festgelegt |
| `start.addrLabel` | Address from the screen | Adresse vom Bildschirm | same as du |
| `start.addrUse` | Use | Übernehmen | same as du |
| `start.addrChecking` | Checking … | Wird geprüft … | same as du |
| `start.addrError` | No Comet Cast answers at this address. Compare it with the screen. | Unter dieser Adresse antwortet kein Comet Cast. Vergleich sie mit dem Bildschirm. | Unter dieser Adresse antwortet kein Comet Cast. Vergleichen Sie sie mit dem Bildschirm. |
| `appSettings.open` | Settings | Einstellungen | same as du |
| `appSettings.title` | Settings | Einstellungen | same as du |
| `appSettings.office` | Convert Office files to PDF | Office-Dateien in PDF umwandeln | same as du |
| `appSettings.officeSub` | Word, PowerPoint and Excel (.docx, .pptx, .xlsx), on this device. Fonts and layout may differ. | Word, PowerPoint und Excel (.docx, .pptx, .xlsx), auf diesem Gerät. Schriften und Layout können abweichen. | same as du |
| `appSettings.otherTvs` | TVs without Comet Cast | Fernseher ohne Comet Cast | same as du |
| `appSettings.otherTvsSub` | Shows Google Cast and AirPlay TVs on the start page. Not tested with every TV. | Zeigt Google-Cast- und AirPlay-Fernseher auf der Startseite. Nicht mit jedem Fernseher getestet. | same as du |
| `about.open` | About | Über | same as du |
| `about.title` | About Comet Cast | Über Comet Cast | same as du |
| `about.version` | Version {version} | Version {version} | same as du |
| `about.licence` | Free software under the GPL-3.0 | Freie Software unter der GPL-3.0 | same as du |
| `about.publisher` | Published by Schulverwalter GmbH | Herausgegeben von der Schulverwalter GmbH | same as du |
| `about.source` | Source code | Quelltext | same as du |
| `about.privacy` | Privacy | Datenschutz | same as du |
| `about.imprint` | Imprint | Impressum | same as du |
| `about.licences` | Licences | Lizenzen | same as du |
| `about.licencesSub` | Parts from other projects | Bausteine aus anderen Projekten | same as du |
| `about.search` | Search | Suchen | same as du |
| `about.featured` | With their own notice | Mit eigenem Hinweis | same as du |
| `about.all` | All parts | Alle Bausteine | same as du |
| `about.none` | Nothing found. | Nichts gefunden. | same as du |
| `about.flutter` | Flutter engine | Flutter-Engine | same as du |
| `about.sourceTv` | Source code: {url} | Quelltext: {url} | same as du |
| `about.webTv` | Privacy and imprint: {url} | Datenschutz und Impressum: {url} | same as du |
| `about.footer` (web) | Free software (GPL-3.0) | Freie Software (GPL-3.0) | same as du |

Reused without change: `common.cancel`, `common.back`, `common.quoted`, `common.noName`, `start.nameLabel`, `start.or`, `start.codeLabel`, `app.scanQr`, `nearby.*`, `file.converting`, `file.bodyApp`, `file.bodyWeb`, `link.label`, `link.go`, `presenting.*`, `what.connectedTo`, `what.screen`, `what.disconnect`, `request.yes`, `request.no`, `toast.expired`, `toast.left`, `toast.sessionEnded`, `present.byline`, `settings.close`, `settings.closeRemote`, `sendReject.*`. The Android share target label and the iOS extension's display name are "Comet Cast" (a name, not translated).
