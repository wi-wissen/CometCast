# First run and display: implementation spec

Status: **final, approved for implementation.** The owner approved round 4 with two changes, which are applied here and in the final mockup. Build from this document; do not wait for further design review.

- Final clickable mockup: https://claude.ai/artifact/6gJouE3T4cWCdiWuG3CWc1 (German and English, uses "du")
- Earlier rounds, for history only: round 4 https://claude.ai/artifact/3pJjsoF2aTEjCkstWngUoW, round 3 https://claude.ai/artifact/1yevs8Z4UukdUF83R31Y7K, round 2 https://claude.ai/artifact/MAGbm5M2sgYXhy1PjLxhgt

Where this document and the mockup disagree, this document wins. The mockup's grey frame (chapter tabs, the remote control, the scenario buttons, the yellow notes, the device switcher) is a presentation aid and is not part of the product.

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
| Code | Join code shown on the display. Default server: 8 characters, shown as two groups of 4 (`K7QM 2W4P`). Own server or in-house box: 6 digits, two groups of 3 (`482 913`). |
| Check number | Two-digit number shown on the presenter's waiting page and in the join request on the display, so the host can match a request to a person. |
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

Derived values: selected tile fill = `color-mix(in srgb, var(--acc) 14%, var(--panel2))`; name tag and End fill = `color-mix(in srgb, var(--panel) 92%, transparent)`; focus glow = `0 0 0 9px rgba(255,201,77,.18)`. Text on an accent fill uses `--bg`.

Rules:

- No other hue appears on the display. There are no green, blue or red status colours. Join mode is told apart by icon and word, not colour.
- OLED black: every surface is `#000000`; surfaces are separated only by `--line` borders. No gradients, no shadows that lighten black, no drift, no burn-in shifting.
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
| `p-disabled` | `#B9C0CD` | `#3B404B` (text `#9EA6B3`) | Disabled primary button |
| `p-error` | `#B5483C` on `#F6E1DE` | `#F08C80` | Error text, error icon disc, invalid input border |
| `p-ok` | `#2C7A51` on `#DDF0E5` | `#6CC597` | "from the QR code" tick, connected tick |
| `p-warn` | `#93650C` on `#F8EFDC` | `#E3B45A` | Clock and offline error icons, "app only" hint icon |

Error/ok/warn are semantic and appear only as icon discs and short status text, never as button fills. Every button is either primary (`p-primary`) or secondary (`p-surface` with `p-border`).

### 3.4 Type

| Role | Font | Notes |
| --- | --- | --- |
| UI text | **Atkinson Hyperlegible Next**, 400 / 500 / 700, fallback `system-ui, sans-serif` | All text on display and presenter pages |
| Codes and numbers | **Atkinson Hyperlegible Mono**, 500 / 700, fallback `ui-monospace, monospace` | Join code, check number, code input |

Bundle both fonts with the apps (SIL OFL); do not load them from the network on the display.

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
| "No and block" | ghost button, min height 34, radius 10, border 2, 14 px text |
| Name tag and End | both min height 46, radius 10, border 2 `--line`, 12 from the bottom and side edges |
| Toast | top 14, centred, padding 8 20, radius 999, fill `--fg`, text `--bg`, 17/700, shadow `0 10px 30px rgba(0,0,0,.4)` |
| Focus ring | outline 4 `--focus`, offset 3, glow 9, scale 1.03 |

Presenter (CSS px): page padding 18 22 22 (phone), 20 40 (laptop); vertical gap 14; inputs 46 high, radius 8; primary button min 50, radius 10; secondary min 48, radius 10; option rows min 58 (laptop 50), radius 12; cards radius 14, padding 14; icon disc 56 (icon 28). Touch targets are never below 44 × 44.

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
| Idle | No one presenting |
| Idle, offline | Idle, and the display cannot reach `{dom}` |
| Idle, dimmed | Idle and no activity for 7 minutes |
| Presenting | A presenter is sending a picture |
| Overlays | Settings (only over Idle), join request (over Idle or Presenting), toast (over anything) |

### 4.1 Setup wizard (first run)

A card (780 ref px wide) centred on `--bg`. Top row: logo (40) left; step dots (4 dots, 10 px, done = `--acc`, todo = `--line`) and "Step n of 4" right, hidden on the welcome screen.

| Step | Content | Buttons (D-pad order, left to right) | Initial focus |
| --- | --- | --- | --- |
| 0 Welcome | `setup.welcome.title`, `setup.welcome.body` | `setup.welcome.start` (primary) | Start |
| 1 Room name | `setup.name.title`, `setup.name.body`; suggestion chips (row 1); text field `setup.name.custom` (max 32 characters) | Back, Next (row 2) | Next |
| 2 Who may connect | `setup.mode.title`; two choice cards: Open (eyebrow `setup.mode.fewPeople`, text `setup.mode.openText`) and Approval on (eyebrow `setup.mode.manyPeople`, text `setup.mode.approvalText`); `setup.mode.later` | Back, Next (row 2) | The selected card (Open by default, D1) |
| 3 Address | `setup.address.title`; card with globe icon, `{dom}` (30/700) and where it came from (`setup.address.source*`); `setup.address.body` | Back, Other address, Looks right (primary) | Looks right |
| 4 Done | `setup.done.title`, `setup.done.body` | `setup.done.finish` (primary) | Finish |

- Name suggestions (DE / EN): Besprechungsraum Elbe / Meeting room Elbe, Wohnzimmer / Living room, Vereinsheim / Clubhouse, Werkstatt / Workshop, Raum 204 / Room 204. The first is preselected. A custom name opens the system on-screen keyboard.
- Locked is not offered as a default; it only exists during a session.
- The address is detected, not typed: DNS entry of the company network → own server; in-house box found on the network → box; otherwise the default server. "Other address" cycles through the found options (if only one exists, it opens a text field for an address).
- Back on the remote goes one step back; on step 0 it does nothing.
- Finish stores room name, default join mode and server, then shows Idle with the chosen mode.

### 4.2 Idle screen

Layout top to bottom (ref px, 960 × 540):

1. **Top row**: logo 46 × 46, room name (30/700), mode pill at the right (icon in `--acc` + word: Open / Approval on / Locked).
2. **Main row** (flex, fills height, centred vertically):
   - Left column: step 1 — number disc "1", label `idle.step1` ("Open in your browser"), address `{dom}` (58/700). For the in-house box also a line `idle.altAddress` with the IP address (16, `--mut`). Step 2 — disc "2", label `idle.step2`, the code (80 mono). If Locked: code at 35 % opacity and the line `idle.locked` under it (indented 54).
   - Right column (236 wide): QR code (212 box) and caption `idle.qrCaption`. If Locked: QR at 25 % opacity, caption `idle.qrLocked`.
3. **Footer row**: `idle.appLine` left (15, `--mut`); **Settings button** right: gear icon + word `settings.open`.

The QR code encodes `https://{dom}/{CODE}` (code without spaces; for the in-house box the IP address is used instead of `{dom}`). Use error correction level M.

Open and Approval on look the same apart from the pill. The step 3 "guide" cards from round 2 are gone.

**Offline:** steps 1 and 2 are replaced by a box (`--panel`, dashed 2 px `--line` border, radius 16, wifi-off icon 34 in `--acc`) with `idle.offline.title` (22/700) and `idle.offline.body`. QR at 25 % opacity. When the network returns, code and QR come back by themselves.

### 4.3 Settings dialog

Opens only over the idle screen (not during setup, not while presenting, not while a join request is open).

Content (card 600 wide, scrim behind):

1. Header row: `settings.title` (section-label style, `--mut`) left; Close button right: × icon + `settings.close` (remote: `settings.closeRemote`, i.e. "Close (Back key)").
2. Section `settings.join`: three tiles Open / Approval on / Locked (icon + word), then one line describing the current mode (`settings.join.*Text`).
3. Section `settings.appearance`: four tiles Dark / Light / OLED black / Automatic.

Selected tile: border `--acc`, fill = selected tile fill, bold. Changes apply at once; there is no Save button. The dialog stays open after a change.

Selecting **Locked** while join requests are waiting rejects all of them, shows toast `toast.lockedRejected`, and their presenters see the "Locked right now" error (variant "while waiting"). Locked lasts until the session ends or the display restarts; after a restart the default mode from setup applies again.

Initial focus: the tile of the current join mode.

### 4.4 Join request dialog (mode Approval on)

Appears by itself when someone asks to join, over the idle screen **or over the running presentation** (the picture stays visible behind the 55 % scrim). It is an `alertdialog`.

Content (card 600 wide):

1. Eyebrow `request.title` (section-label style). If more requests wait: `request.queue` at the right ("+2 waiting").
2. Row: left the name in quotes (32/700, wraps anywhere) and below it `request.subtitle` ("wants to connect · Laptop"); right the check-number box (`--bg` fill, radius 10, label `request.check` 11 caps, number 24 mono).
3. Buttons: **Yes** (primary, `--acc`), **No** (secondary), gap, then **No and block** (ghost) at the far right.

Behaviour:

- One press of Yes allows. No hold.
- **Input is ignored for 1 s after the dialog appears** (and after it switches to the next request), so a stray key press approves nothing.
- Initial focus: Yes.
- Requests are handled first come, first served; after a decision the next request takes the dialog and focus returns to Yes (with the 1 s guard again). With no more requests the dialog closes; focus returns to the Settings button (idle) or nothing (presenting; the overlay stays hidden).
- A request expires after **60 s** without an answer: it leaves the queue, toast `toast.expired`, the presenter sees "No answer".
- No and block: this device cannot ask again until the session ends; its later attempts are not shown on the display at all and the device sees "Not allowed". A new code (End) lifts the block.
- After No, the same device may ask again after 15 s (enforced on the presenter page with a countdown).
- Names: strip control and invisible characters, collapse whitespace, cut to 18 characters + "…", show in quotes. Empty name → `common.noName`. Show the full name only in the accessible label.
- Back on the remote does nothing while the dialog is open (an answer is required). Menu does nothing.

### 4.5 Presenting

The picture fills the screen edge to edge on black (`#000000`), letterboxed in its own aspect ratio (a portrait phone screen sits centred). **Nothing else is shown at rest**: no code, no room name, no mode.

On input (see 8) two controls fade in for 5 s:

- **Name tag**, bottom left: `present.byline` ("“Lena” is showing · Laptop"), 15/700, `--fg` on the panel fill, border `--line`.
- **End**, bottom right: square stop icon + `present.end`. Same fill, border and text colour as the name tag. Not pink, not amber.

Both use the current appearance's `--panel`, `--line` and `--fg` (dark blue in Dark, light grey in Light, black with grey border in OLED black). With D-pad input, End shows the focus ring.

**End** closes the session: all presenters are disconnected, all waiting requests are dropped, blocks are cleared, a new code is made, the display returns to Idle, toast `toast.sessionEnded`. Presenters see "Presentation ended". There is no confirmation step.

While presenting, the screen is kept awake (D5). When the presenter stops, the display returns to Idle with the same code; presenters stay connected.

### 4.6 Toasts

One line, centred at the top, `--fg` background with `--bg` text, 3.2 s. A new toast replaces the old one. Toasts never take focus. Keys: see the `toast.*` table.

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
2. Heading: `start.titleRoom` ("Connect to “{room}”") when the code came from the QR code, else `start.titleGeneric`.
3. Code: from QR → code chip (code in mono + green tick + `start.fromQr`); typed → field `start.codeLabel` with placeholder `XXXX XXXX` (default server) or `000 000` (own server/box), `inputmode` text, uppercase, autocomplete off, max 11 characters.
4. Field `start.nameLabel` (max 40 characters, remembered on the device).
5. Primary button with globe icon `start.browser`, small print below: `start.browserPhone`.
6. Divider with `start.or`.
7. Secondary button with download icon: `start.appAndroid` / `start.appIos`; small print `start.appPhone`.
8. Info line with (i) icon: `start.detected*` ("Detected: Android phone, Chrome").

Laptop (≥ 720 px wide): two columns. Left: brand, heading, code, name. Right, in a card: browser button + small print `start.browserLaptop`, "or", `start.appDesktop` + `start.appLaptop`, detected line, and `start.beDisplay` ("Should this device be the screen itself? Open {dom}/display."). Clicking the desktop download shows `start.downloadStarted` under it; the person can continue in the browser meanwhile.

Code handling: uppercase; remove spaces and dashes; read O as 0 and I or L as 1. Validate on submit only. Empty → inline error `start.codeEmpty` (field border `p-error`). A code of the other server's format (6 digits on the default server, 8 characters on an own server) → error page "This code belongs to another address". Unknown code → error page "This code does not exist right now".

"Continue in the browser" → connect: Open mode → "What to show" page; Approval on → waiting page; Locked → "Locked right now"; blocked device → "Not allowed".

### 5.3 App path

- Store: the system store page (not designed by us). Android passes code and address through the Google Play install referrer (assumption, not yet verified). iOS cannot, so the app asks to scan again.
- App first-run page: heading `app.welcome`; code chip with `app.codeFromLink` (Android) or `app.codeScanned` (after scanning); if there is no code: `app.scanAgainBody` and a secondary button `app.scanQr`; name field; primary `app.connect` (disabled while there is no code).

### 5.4 Waiting for approval

Spinner + room name (label style); heading `wait.title`; card with `wait.checkLabel`, the check number (58 mono, `p-primary`) and `wait.checkHint`; progress bar (6 px) counting down 60 s with `wait.expires`; at the bottom secondary `wait.withdraw`. Withdrawing removes the request from the display (toast `toast.withdrawn`) and returns to the start page.

### 5.5 What to show

Header row: green tick disc (40) + `what.connectedTo`. Heading `what.title`. Three option rows (icon in `p-primary`, title + subtitle):

| Row | Title | Subtitle |
| --- | --- | --- |
| Screen | `what.screen` | Laptop: `what.screenLaptop`; app: `what.screenApp`; phone browser: `what.screenPhoneBrowser` (icon in `p-warn`) |
| File | `what.file` | `what.fileSub` |
| Link | `what.link` | `what.linkSub` |

Bottom: secondary `what.disconnect` → "Disconnected" page; toast `toast.left` on the display.

- Screen on a laptop → the browser's own share dialog opens directly, with "Window" preselected where the browser allows it. There is **no** explainer page before it.
- Screen in a phone browser → error "Screen sharing does not work in this browser" (it is known before; the page says so in the subtitle).
- Screen in the app → the system prompt (Android "Start recording or casting", iOS "Start Broadcast").
- Cancelling the system dialog returns to this page without an error.
- File → file page: heading `file.title`, `file.body`, recent files as a 2-column grid of thumbnails, secondary `common.back`. Picking a file starts presenting.
- Link → link page: heading `what.link`, field `link.label` (type url), `link.hint`, primary `link.go`, secondary `common.back`.

### 5.6 Presenting (presenter side)

Icon disc + heading (`presenting.screen` / `presenting.file` / `presenting.link`), body `presenting.where`, primary button at the bottom `presenting.stop`. Stop returns to "What to show". The browser's own "sharing" bar or the phone's red indicator stay as the system draws them; when Chrome's bar is shown at the bottom of the page, keep 76 px free under the button.

### 5.7 Disconnected

Grey tick disc, `bye.title`, `bye.body`, primary `bye.again` (back to the start page; phone: start route).

## 6. Error states (presenter pages)

Every error page has the same layout: icon disc (56), heading, a card with two labelled parts (`error.why`, `error.now`), and buttons at the bottom (primary first; laptop: in a row, phone: stacked).

| Key prefix | When | Icon | Buttons |
| --- | --- | --- | --- |
| `error.code` | Unknown code | × in `p-error` | Retype code (primary); phone: Scan QR code again |
| `error.expired` | Code was valid, no longer | clock in `p-warn` | Retype code; phone: Scan again |
| `error.otherServer` | Code format belongs to another server | (i) in `p-primary` | Open {dom} (primary); phone: Scan again |
| `error.noInternet` | Presenter device offline | × in `p-warn` | Try again (also retries by itself) |
| `error.noScreen` | Phone browser, Screen chosen | (i) in `p-primary` | Show photo or file (primary), Show a link, Get the app |
| `error.displayOff` | Display does not answer | × in `p-warn` | Try again |
| `error.denied` | Host chose No, or device is blocked | × in `p-error` | Ask again (disabled for 15 s, label counts down) |
| `error.locked` | Mode is Locked (two "why" variants: at once / while waiting) | lock in grey | Try again |
| `error.timeout` | No answer within 60 s | clock in `p-warn` | Ask again |
| `error.ended` | Host pressed End | (i) in grey | Retype code; phone: Scan again |

For the in-house box, `error.noInternet` uses the `nowBox` text (must be on the same Wi-Fi).

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
| Presenting (controls shown) | [End] |
| Setup 0 | [Start] |
| Setup 1 | [5 name chips] · [Back, Next] |
| Setup 2 | [Open, Approval on] · [Back, Next] |
| Setup 3 | [Back, Other address, Looks right] |
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
- No focus ring with mouse or touch; with keyboard use, the same ring as on TV (`:focus-visible`).
- Mouse movement over the screen during a presentation shows the controls (throttled to one trigger per 400 ms); a click or tap anywhere shows them; clicking End ends the session. The pointer hides together with the controls.
- Keyboard: arrows = D-pad, Enter or Space = OK, Escape = Back, `M` = Menu. Tab follows the focus order above.
- Settings button opens Settings by click or tap.

### 7.3 Presenter: phone

Width 360–430 px, single column, primary action at the bottom of the page where the page has one main action. Inputs use the platform keyboard; the code field shows the alphanumeric keyboard in uppercase.

### 7.4 Presenter: laptop

Pages wider than 720 px use the two-column layouts given in 5.2 and the error pages; buttons sit in a row. Max content width 744 px, centred.

## 8. Interaction rules (summary)

1. **Presenting is quiet.** Only the picture until someone touches an input. Then name tag + End for 5 s after the last input, 400 ms fade.
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
- Icon-only items do not exist on the display. The QR code has the label "QR: {url}".

## 10. Not included (do not build)

- Accounts, sign-in, room lists, scheduling.
- A list of connected people, removing a single presenter, "deny all", the round 2 hold-to-allow request cards and the request banner during presentation.
- Any control during a presentation other than End (no disconnect-person, no mode, no appearance, no code overlay).
- The step-by-step guide cards on the idle screen.
- The explainer page before the browser's share dialog.
- Showing several presenters at once (split screen).
- Renaming the room, changing the server address or re-running setup from Settings. To change these, reset the app's data (TV) or clear site data (`/display`). A later round may add this.
- OLED black on presenter pages.
- AirPlay, Miracast and Google Cast receiving (covered elsewhere; not part of this flow).
- The mockup's own frame: chapter tabs, simulated remote, scenario buttons, notes, device switcher, the "view from 4 m" preview, the host instructions panel.
- Store pages, system share dialogs and camera chips: they are drawn by the operating system.

## 11. Copy

### 11.1 Rules for German

- **Shipped default: "Sie"** (column "DE Sie"). The column "DE du" is the wording of the mockup. The owner may switch to "du" later.
- Implement this as two German value sets with identical keys (for example `de.json` = Sie and a `de-du.json` overlay, or two ARB files), so switching is one file swap, not a code change.
- Every German string is a short main clause. Only the pronoun and the verb ending change between the two forms; sentence structure stays the same. Do not add strings in only one form.
- English has one form.
- Placeholders: `{room}`, `{dom}`, `{code}`, `{name}`, `{device}`, `{n}`, `{ip}`, `{url}`, `{os}` (Windows, macOS, Linux), `{browser}` (Chrome, Safari, …), `{here}` (the address the presenter page is on). Quotes are part of the string where shown (German „…“, English “…”). Names are inserted already quoted via `common.quoted`.

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
| `settings.join` | Joining | Beitritt | Beitritt |
| `settings.join.openText` | Anyone with the code connects at once. | Jeder mit dem Code ist sofort verbunden. | Jeder mit dem Code ist sofort verbunden. |
| `settings.join.approvalText` | You let each person in one by one. | Du lässt jede Person einzeln zu. | Sie lassen jede Person einzeln zu. |
| `settings.join.lockedText` | Nobody new can join. Connected stay. | Niemand kommt neu hinzu. Verbundene bleiben. | Niemand kommt neu hinzu. Verbundene bleiben. |
| `settings.appearance` | Appearance | Aussehen | Aussehen |
| `request.title` | Request | Anfrage | Anfrage |
| `request.queue` | +{n} waiting | +{n} warten | +{n} warten |
| `request.subtitle` | wants to connect · {device} | möchte sich verbinden · {device} | möchte sich verbinden · {device} |
| `request.check` | Check | Prüfzahl | Prüfzahl |
| `request.yes` | Yes | Ja | Ja |
| `request.no` | No | Nein | Nein |
| `request.noBlock` | No and block | Nein und sperren | Nein und sperren |
| `request.aria` | Request: {name} | Anfrage: {name} | Anfrage: {name} |
| `present.byline` | {name} is showing · {device} | {name} zeigt · {device} | {name} zeigt · {device} |
| `present.end` | End | Beenden | Beenden |
| `toast.joined` | {name} connected | {name} ist verbunden | {name} ist verbunden |
| `toast.allowed` | {name} allowed | {name} zugelassen | {name} zugelassen |
| `toast.denied` | {name} denied | {name} abgelehnt | {name} abgelehnt |
| `toast.blocked` | {name} denied and blocked | {name} abgelehnt und gesperrt | {name} abgelehnt und gesperrt |
| `toast.expired` | Request from {name} expired | Anfrage von {name} verfallen | Anfrage von {name} verfallen |
| `toast.withdrawn` | {name} withdrew the request | {name} hat die Anfrage zurückgezogen | {name} hat die Anfrage zurückgezogen |
| `toast.left` | {name} disconnected | {name} hat sich getrennt | {name} hat sich getrennt |
| `toast.lockedRejected` | Locked · {n} open requests denied | Gesperrt · {n} offene Anfragen abgelehnt | Gesperrt · {n} offene Anfragen abgelehnt |
| `toast.sessionEnded` | Session ended · new code | Sitzung beendet · neuer Code | Sitzung beendet · neuer Code |

### 11.5 Presenter: start, app, waiting, what to show, presenting

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `start.titleRoom` | Connect to “{room}” | Mit „{room}“ verbinden | Mit „{room}“ verbinden |
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
| `what.title` | What would you like to show? | Was möchtest du zeigen? | Was möchten Sie zeigen? |
| `what.screen` | Share screen | Bildschirm teilen | Bildschirm teilen |
| `what.screenLaptop` | Whole screen or one window, live | Ganzer Bildschirm oder ein Fenster, live | Ganzer Bildschirm oder ein Fenster, live |
| `what.screenApp` | Your whole phone display, live | Dein ganzes Handy-Display, live | Ihr ganzes Handy-Display, live |
| `what.screenPhoneBrowser` | On a phone only with the app | Geht auf dem Handy nur mit der App | Geht auf dem Handy nur mit der App |
| `what.file` | Show a photo or file | Foto oder Datei zeigen | Foto oder Datei zeigen |
| `what.fileSub` | PDF or image. You turn pages, everything else stays with you | PDF oder Bild. Du blätterst, der Rest bleibt bei dir | PDF oder Bild. Sie blättern, der Rest bleibt bei Ihnen |
| `what.link` | Show a link | Link zeigen | Link zeigen |
| `what.linkSub` | The screen opens the page itself | Der Bildschirm öffnet die Seite selbst | Der Bildschirm öffnet die Seite selbst |
| `what.disconnect` | Disconnect | Trennen | Trennen |
| `file.title` | What would you like to show? | Was möchtest du zeigen? | Was möchten Sie zeigen? |
| `file.body` | Recent on this device. Or “Other file” in the system dialog. | Zuletzt auf diesem Gerät. Oder „Andere Datei“ im Systemdialog. | Zuletzt auf diesem Gerät. Oder „Andere Datei“ im Systemdialog. |
| `link.label` | Page address | Adresse der Seite | Adresse der Seite |
| `link.hint` | The screen needs its own internet for this. | Der Bildschirm braucht dafür selbst Internet. | Der Bildschirm braucht dafür selbst Internet. |
| `link.go` | Open on the screen | Auf dem Bildschirm öffnen | Auf dem Bildschirm öffnen |
| `presenting.screen` | You are sharing your screen | Du zeigst deinen Bildschirm | Sie zeigen Ihren Bildschirm |
| `presenting.file` | You are showing a file | Du zeigst eine Datei | Sie zeigen eine Datei |
| `presenting.link` | The screen shows your link | Der Bildschirm zeigt deinen Link | Der Bildschirm zeigt Ihren Link |
| `presenting.where` | On “{room}”. | Auf „{room}“. | Auf „{room}“. |
| `presenting.stop` | Stop showing | Zeigen beenden | Zeigen beenden |
| `bye.title` | Disconnected | Getrennt | Getrennt |
| `bye.body` | You are no longer connected. You can close this tab. | Du bist nicht mehr verbunden. Du kannst diesen Tab schließen. | Sie sind nicht mehr verbunden. Sie können diesen Tab schließen. |
| `bye.again` | Connect again | Neu verbinden | Neu verbinden |

### 11.6 Presenter: errors

| Key | EN | DE du | DE Sie (shipped) |
| --- | --- | --- | --- |
| `error.why` | Why | Warum | Warum |
| `error.now` | What now | Was jetzt | Was jetzt |
| `error.code.title` | This code does not exist right now | Diesen Code gibt es gerade nicht | Diesen Code gibt es gerade nicht |
| `error.code.why` | Usually a typo. Comet Cast reads an O in a code as zero and an I as one, so you need not tell them apart. | Meist ein Tippfehler. Ein O im Code liest Comet Cast als Null, ein I als Eins, das musst du nicht unterscheiden. | Meist ein Tippfehler. Ein O im Code liest Comet Cast als Null, ein I als Eins, das müssen Sie nicht unterscheiden. |
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
| `error.noScreen.now` | Show a photo, a file or a link instead. Or get the app, then the screen works too. | Zeig stattdessen ein Foto, eine Datei oder einen Link. Oder hol dir die App, dann geht auch der Bildschirm. | Zeigen Sie stattdessen ein Foto, eine Datei oder einen Link. Oder holen Sie sich die App, dann geht auch der Bildschirm. |
| `error.displayOff.title` | The screen does not answer | Der Bildschirm antwortet nicht | Der Bildschirm antwortet nicht |
| `error.displayOff.why` | The display device is off, asleep, or Comet Cast is closed there. Your request did not arrive. | Das Anzeigegerät ist aus, schläft oder Comet Cast ist dort geschlossen. Deine Anfrage ist nicht angekommen. | Das Anzeigegerät ist aus, schläft oder Comet Cast ist dort geschlossen. Ihre Anfrage ist nicht angekommen. |
| `error.displayOff.now` | Is the code still on the screen? If not, ask the host to open Comet Cast there. | Steht der Code noch auf dem Bildschirm? Wenn nicht, den Gastgeber bitten, Comet Cast dort zu öffnen. | same as du |
| `error.denied.title` | Not allowed | Nicht zugelassen | Nicht zugelassen |
| `error.denied.why` | “No” was chosen on the screen. | Am Bildschirm wurde „Nein“ gewählt. | Am Bildschirm wurde „Nein“ gewählt. |
| `error.denied.now` | Ask in person if you need to. | Frag bei Bedarf kurz persönlich nach. | Fragen Sie bei Bedarf kurz persönlich nach. |
| `error.locked.title` | Locked right now | Gerade gesperrt | Gerade gesperrt |
| `error.locked.why` | The code is right, but this screen is not taking anyone new right now. | Der Code stimmt, aber dieser Bildschirm nimmt gerade niemanden Neues an. | same as du |
| `error.locked.whyWhile` | The host locked the room while you were waiting. Your request was dropped. | Der Gastgeber hat gesperrt, während du gewartet hast. Deine Anfrage wurde verworfen. | Der Gastgeber hat gesperrt, während Sie gewartet haben. Ihre Anfrage wurde verworfen. |
| `error.locked.now` | Try again later or talk to the host. | Später noch einmal versuchen oder den Gastgeber ansprechen. | same as du |
| `error.timeout.title` | No answer | Keine Antwort | Keine Antwort |
| `error.timeout.why` | Nobody answered your request within 60 seconds. | Niemand hat deine Anfrage innerhalb von 60 Sekunden beantwortet. | Niemand hat Ihre Anfrage innerhalb von 60 Sekunden beantwortet. |
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

"same as du" means the Sie value is identical to the du value; it still exists as its own entry in the Sie file.

Note on `error.otherServer.now`: the mockup said "…the right address, here {dom}". This spec drops "here {dom}", because the presenter page cannot know the display's address when the code belongs to another server. Show the "Open {dom}" button only when the code's server is known (for example from a QR link); otherwise show only "Retype code".

## 12. Open points that do not block implementation

- The owner's question "du" or "Sie" (D6). Ship Sie; the du set is ready.
- The Android install referrer passing code and address to the app is an assumption; if it does not work, Android uses the iOS behaviour (scan again).
- Exact idle-dim delay: 7 minutes is inside the owner's 5–10 minute range; adjust in one constant if testing suggests otherwise.
