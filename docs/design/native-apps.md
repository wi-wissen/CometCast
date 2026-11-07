# Native apps: platform guidance

Status: **final for this round (2026-11-07); revised after the Flutter review (spec decisions N1–N28, section 20 added) and after the second Flutter review (N29–N47: sections 3, 4, 20.5, 20.6).** Companion to [first-run-and-display.md](first-run-and-display.md) (the spec). This file adds **no new design** (the list of screens on the same network is designed in spec 14; section 4.1 below says how each platform builds it). It says, for each native platform, how the screens and states of the spec are built, what is shared, and where a platform needs a variant because of what the platform can or cannot do. Where this file and the spec disagree, the spec wins; tell the designer.

Platforms of this round:

| Platform | Toolkit | App folder |
| --- | --- | --- |
| Android phone and tablet | Flutter | `apps/flutter` |
| Android TV / Google TV | Flutter | `apps/flutter` (leanback mode, `lib/core/android_platform.dart`) |
| iOS (iPhone) and iPadOS | Flutter (+ broadcast extension for screen sharing) | `apps/flutter`, `apps/flutter/ios` |
| Linux, Windows, macOS | Flutter desktop | `apps/flutter/linux`, `windows`, `macos` |
| Apple TV (tvOS 17+) | SwiftUI | `apps/apple-tv` |

Spec section numbers are written as "spec 4.2".

## 1. Roles per platform

A device is either a **display** (spec 4, shows the picture) or a **presenter** (spec 5, sends one). The web has both as pages; the apps have them as follows.

| Platform | Display | Presenter | First launch |
| --- | --- | --- | --- |
| Android TV / Google TV | yes, only role | no | Setup wizard (spec 4.1) |
| Apple TV | yes, only role | no | Setup wizard (spec 4.1) |
| Android phone, iPhone | no (a phone is not a target display, spec 7.2) | yes | App first-run page (spec 5.3) |
| Android tablet, iPad | yes | yes | App first-run page (spec 5.3) |
| Linux, Windows, macOS | yes | yes | App first-run page (spec 5.3) |

Choosing the display role on tablets and computers: the app first-run page (spec 5.3) ends with one text link, `app.beDisplay` ("Use this device as the screen", spec 11.7), styled exactly like the `{dom}/display` link of `start.beDisplay` (spec 5.2: `p-primary`, underlined, 15). It opens the setup wizard. Once setup is finished, the app starts in the display role on every launch. Going back to the presenter role is done like changing the room name: reset the app's data (spec 10). There is no role switch in Settings (spec D2: Settings holds join mode and appearance only).

## 2. What must be identical on every platform

Engineers must not change these per platform. A difference is a bug.

1. **Tokens:** every colour of spec 3.2 and 3.3, as exact hex (sections 10 and 11 below). No Material dynamic colour, no `ColorScheme.fromSeed`, no system accent colour, no SwiftUI semantic colours (`.primary`, `.accentColor`, materials) on our surfaces.
2. **The one-accent rule** (spec 3.2 rules, 3.3): no other hue on the display; on presenter pages error/ok/warn only as icon discs and short status text.
3. **Geometry:** every display size in reference px on the 960 × 540 canvas (spec 3.1, 3.5); every presenter size in logical px (Flutter logical pixels, Apple points) as given in spec 3.4/3.5 in CSS px.
4. **Type:** Atkinson Hyperlegible Next and Mono, sizes, weights, line heights and letter spacing of spec 3.4.
5. **Icons:** the stroke icons of `apps/web/src/components/Icon.vue` and the toolbar icons of `apps/web/src/components/ReceiverToolbar.vue` (24 × 24 view box, stroke 2, round caps and joins; `check` 2.6, `stop` and `x` 2.4). No Material Icons, no Cupertino icons, no SF Symbols.
6. **States and timings:** the state table (spec 4), the 1 s input guard, controls shown 5 s with 400 ms fades, mouse throttle 400 ms, toast 3.2 s, idle dim after 7 min to 40 % in 2 s (restore 200 ms), the reconnect schedule (1 s, 2 s, 3 s, then 4 s; offline box when the 3 s attempt fails; busy 30 s; the 60 s busy window), connecting → offline after 8 s.
7. **Behaviour:** focus rows and key meanings (spec 7.1) with the platform's key names, one dialog at a time, first input only reveals, no hold gestures, what is remembered (spec 4.3), the error mapping (spec 6), the code format and the code field rules (spec 5.2), the QR content `https://{dom}/{CODE}` at level M.
8. **Copy:** the keys and values of spec 11 and 14.10 (EN, DE du shipped, DE Sie ready), including the `.Remote` / `.Pointer` variants chosen by input type, not by platform name.
9. **What is not built** (spec 10), plus section 9 below.

## 3. Display screens per platform

"Same" means: as the spec says, with the tokens and sizes of the spec. Only the differences are written out.

| Spec | Android TV / Google TV | Apple TV | Android tablet, iPad | Linux / Windows / macOS |
| --- | --- | --- | --- | --- |
| 3.1 canvas | Full screen, landscape. Scale `min(w/960, h/540)`, rest `--bg` | Same | Same; the app is locked to landscape while in the display role; in portrait (iPad in split view) the 16:9 canvas is letterboxed (spec 7.2) | Same, in a window or full screen |
| 4.1 wizard | Same. Text fields open the system keyboard on OK only (section 8) | Same; text fields use the full-screen tvOS keyboard on Select | Same; touch keyboard; `setup.done.bodyPointer` | Same; `setup.done.bodyPointer` |
| 4.1 address step | Detects own server (DNS) and in-house box: `sourceOwn`, `sourceBox`; administrator policy: `sourcePolicy` (N13); `{dom}` without `:443` (N12); "Other address" cycles the found ones, then opens the field | Same | Same | Same |
| 4.2 idle | Same. `idle.altAddress` with the box IP when the box is used (N40: the server came from the box announcement, `sourceBox`; the IPv4 of its resolution; only when `{dom}` is a host name) | Same | Same | Same |
| 4.2 connecting / offline | Same (empty QR box of spec 3.2) | Same | Same | Same |
| 4.3 settings | Opened by focus on the Settings button, or Menu key; close label `settings.closeRemote` | Play/Pause = Menu; `settings.closeRemote` | Tap; `settings.close` | Click, `M` key; `settings.close` |
| 4.4 join request | Same; Back does nothing while open | Menu (Back) does nothing while open | Same | Same; Escape does nothing while open |
| 4.5 presenting | Same; no toolbar | Same; no toolbar | Same; no toolbar this round (section 9) | Same; no toolbar this round (section 9) |
| 4.5 content states | Same: transfer pill one line, max 340, name shortened in the middle, then `receive.document.loading` in the same pill (spec 3.5); the display sends the page message `{"kind":"page","page":n,"pages":m}` (spec 5.6, decision 13.58) | Same | Same | Same |
| 4.5 link card | Open only if the system can open web links (Flutter `canLaunchUrl`), hint `receive.link.hintApp`; otherwise the no-browser variant (`receive.link.hintTv`, [Dismiss]). Address 18 mono `--fg` | Always the no-browser variant (tvOS has no browser) | Open opens the system browser; `receive.link.hintApp` | Open opens the default browser; `receive.link.hintApp` |
| 4.5 media in link card | Not this round (spec N11): plain link card, no Play | AVPlayer; same rules (see `docs/appletv.md`) | Not this round: plain link card | Not this round: plain link card |
| 4.7 video codec dialog | n/a (system decoders) | n/a | n/a | Linux, Windows: spec 4.7 (780 wide, over Presenting without a picture, once per presentation, `codec.stageHint` after Not now; N30, N31); macOS: n/a |
| 4.6 toasts | Same | Same | Same | Same |
| 7.2 relay confirmation | Not used: a display app gets no server from a link (spec N19) | Not used | Not used | Not used |

## 4. Presenter screens per platform

Presenter pages use the phone layout below 720 logical px of window width and the laptop layout (spec 7.4) at 720 and above, whatever the device. A tablet in landscape and a desktop window are laptop layout; an iPad in narrow split view is phone layout.

| Spec | Android phone / tablet | iPhone / iPad | Linux / Windows / macOS |
| --- | --- | --- | --- |
| 5.2 start page | Not in the app (web only) | Not in the app | Not in the app |
| 5.3 app first-run page | Order of spec 5.3 (N22): brand row, `app.welcome`, name field, then the code chip (`app.codeFromLink` from an app link or the install referrer, `app.codeScanned` after a scan) or `app.scanQr` (system scanner, section 8) with the code field of spec 5.2 below it, `app.connect`; tablets: `app.beDisplay` after Connect. `app.scanAgainBody` only on the first launch without a code | Same (iPhone, iPad) | No scanning: brand row, `app.welcome`, name field, code field (`start.codeLabel`, placeholder `XXX XXX`), `app.connect`, then `app.beDisplay`; Windows: `app.miracast` below it (spec N18) |
| 5.4 waiting | Same | Same | Same |
| 5.5 what to show: Screen | Phone `what.screenApp`, tablet `what.screenTablet` (by device kind, not width); opens the system capture consent (section 8) | iPhone `what.screenApp`, iPad `what.screenTablet`; opens the system broadcast picker; the join is handed to the broadcast extension and back (section 20.5, N32) | `what.screenLaptop`; opens the system window/screen picker (Linux, Windows: the codec page of spec 5.8 first when the encoder is missing) |
| 5.5 file page | The browser variant of spec 5.5 (N16): `what.file`, `file.bodyWeb`, primary `file.pick` (system document picker), progress line, inline error, `common.back`. No recent-files grid | Same | Same; `file.bodyApp` where Office conversion is available, progress line `file.converting` while converting |
| 5.5 link page | Same | Same | Same |
| 5.6 presenting | Same; no 76 px reserve (no Chrome bar). The system's own capture indicator (status bar chip) is drawn by the system | Same; the red status pill is drawn by the system | Same; no 76 px reserve. The OS draws its own indicator (Windows yellow border, macOS menu bar icon) |
| 5.7 disconnected | `bye.title` and `bye.again` only (note below) | Same | Same |
| 6 error pages | Same; "Scan QR code again" only where the system scanner exists; `error.outdated` with `titleApp` / `nowApp` / Try again; `error.noScreen` never | Same | No rescan button; otherwise as Android |

Windows Miracast page (N39): presenter kit; pairing as three single-choice option rows; phone: content scrolls above the pinned [Start receiving, Back] with ≥ 24 free above the action area; laptop: [Start receiving, Back] in one row under the content; consent page in the relay-page layout with the PIN digits (only) in mono, [Decline, Allow], initial focus Decline.

Note on 5.7: `bye.body` says "You can close this tab", which is wrong in an app. In the apps, the disconnected page shows `bye.title` and the button only; the body line is omitted. (No new copy.)

### 4.1 Screens on the same network (spec 14)

The app start page of every presenter app (Android phone and tablet, iPhone, iPad, Linux, Windows, macOS) shows the section `nearby.title` of spec 14 and the display apps announce themselves (spec 14.8). Everything visible is identical; only the system parts differ:

| | Android phone / tablet | iPhone / iPad | Linux / Windows / macOS | Displays (Android TV, Apple TV, tablet or computer in the display role) |
| --- | --- | --- | --- | --- |
| Browse | `NsdManager.discoverServices` (`_cometcast._tcp`), resolve with `registerServiceInfoCallback` (API 34+) or `resolveService` | `NWBrowser` with `.bonjourWithTXTRecord`; TXT via `NWTXTRecord` | the existing `mdns-sd` backend of `cc-discovery` | n/a (no list on a display) |
| Announce | n/a on phones; `NsdManager.registerService` on tablets in the display role and Android TV | n/a on iPhone; `NWListener` service on iPad in the display role | `cc-discovery` `advertise_receiver` in the display role | Android TV: `NsdManager.registerService`; Apple TV: `NWListener.service` with TXT (replaces today's announcement fields, spec 14.8) |
| Permission | none for browsing; a future Android local-network permission is handled like iOS | **Local Network** prompt, shown when the person taps `nearby.search` the first time (spec 14.2 "Not asked yet"); `NSLocalNetworkUsageDescription` = `ios.localNetworkUsage`; `NSBonjourServices` lists `_cometcast._tcp`. Denied (`PolicyDenied`) → state "Not allowed", link to `UIApplication.openSettingsURLString` | none (macOS 15+ may ask for Local Network for the desktop app the same way: handle it as iOS) | Apple TV and iPad already ask when announcing starts (`docs/appletv-device-test-plan.md` B1) |
| Search time | 60 s, then Paused (spec 14.5) | 60 s, then Paused | while the window is visible | announce while Idle or Presenting |
| "No local network" | `ConnectivityManager`: no `TRANSPORT_WIFI` / `TRANSPORT_ETHERNET` | `NWPathMonitor`: no `.wifi` / `.wiredEthernet` | no non-loopback interface with a private address | n/a |
| Open the system settings (`nearby.allow`) | n/a (no permission) | `openSettingsURLString` | macOS: the Privacy & Security pane | n/a |

Nothing about the list may run on the web; the web start page keeps `start.appLaptop` as the reason to get the app.

## 5. Navigation and Back

There is no navigation bar, no app bar and no tab bar on any page. Pages replace each other; there is no visible back arrow. The system Back (Android back button or gesture, Escape on desktop, Menu on the Siri Remote) does what the page's own secondary action does:

| Page | System Back |
| --- | --- |
| Display: idle | Android TV: first press consumed, second within 2 s leaves the app (spec 7.1). Apple TV: Menu leaves the app (tvOS rule). Tablet/desktop: nothing |
| Display: settings, link card, media | Close Settings / Dismiss / close the card |
| Display: join request | Nothing |
| Display: presenting | Controls hidden: show them. Controls shown: hide them (spec 7.1) |
| Display: setup | Previous step; on step 0 nothing (Android TV: leaves the app on a second press within 2 s) |
| Presenter: first-run page | Leaves the app (system default) |
| Presenter: waiting | Withdraw request (`wait.withdraw`) |
| Presenter: what to show | Disconnect (`what.disconnect`) |
| Presenter: file, link | `common.back` |
| Presenter: presenting | Nothing. Stop is the only way out (an accidental back gesture must not end a presentation) |
| Presenter: error, disconnected | Back to the first-run page |

iOS: there is no edge-swipe back (pages are not pushed with a back stack the user can swipe). Android predictive back: only where the table says Back goes somewhere; elsewhere disable the predictive animation.

## 6. Focus and input

### 6.1 TV (Android TV, Apple TV)

- Focus rows exactly as spec 7.1. Focus never wraps. Exactly one control has the ring whenever a control is on screen; nothing is focusable at rest during a presentation.
- Ring: spec 3.5 (outline 4 `--focus`, offset 3, glow 9 at 18 %, scale 1.03; text fields without scale). Do **not** use the platform's focus look (Material focus overlay, tvOS lift/parallax/highlight).
- Key mapping:

| Spec key | Android TV | Apple TV (Siri Remote) |
| --- | --- | --- |
| Arrows | DPAD_UP/DOWN/LEFT/RIGHT | Swipe or click on the touch surface / ring |
| OK | DPAD_CENTER, ENTER, NUMPAD_ENTER | Select (click) |
| Back | BACK | Menu / Back (`onExitCommand`) |
| Menu | MENU (where present) | Play/Pause (`onPlayPauseCommand`) |

  Do not bind the remote's Settings, Home, voice or app keys.
- Flutter: one `FocusTraversalGroup` per row with `OrderedTraversalPolicy`; up/down move between rows to the same column or the last item (custom `DirectionalFocusAction`, see `RowsPolicy` in `lib/src/display/kit.dart` and `DField` in `lib/src/display/fields.dart`, which map up/down out of text fields). Initial focus per spec (Yes, Open, Cancel, Settings button, the selected tile …) with `autofocus` on that control only.
- SwiftUI (tvOS 17): rows are `HStack`s with `.focusSection()`; initial focus with `.defaultFocus($focus, …)` and `@FocusState`; buttons use a custom `ButtonStyle` that draws the spec ring from `@Environment(\.isFocused)` and `.focusEffectDisabled()` to remove the system effect. The 1 s guard ignores Select in `Button` actions (keep the button focusable, ignore the action).
- First input only reveals (spec 8.2): while controls are hidden, the key that shows them is consumed (Flutter: return `KeyEventResult.handled`; SwiftUI: handle in `onMoveCommand` / a hidden focusable catcher view and do not forward).

### 6.2 Tablets and computers in the display role

Same rows. Keyboard: arrows, Enter/Space = OK, Escape = Back, `M` = Menu, Tab follows the rows (spec 7.2). Ring rule of spec 7.2 (shown on start; hidden by the first pointer press, touch start or real mouse movement; shown again by a key). Mouse movement during a presentation shows the controls (400 ms throttle); a tap or click shows them; the pointer hides with them (desktop).

### 6.3 Presenter pages

Touch first. Keyboard (desktop, tablets with a keyboard): Tab order = reading order, Enter activates the focused button and submits a form from its last field. Focus outline: 2 `p-primary`, offset 2 (the web presenter's outline). Platform focus highlights off.

## 7. Safe areas, overscan, windows, system bars

- **Android TV:** the canvas fills the screen; the spec paddings (top 26, sides 40, bottom 18 ref) are the margins. Do not add Flutter `SafeArea` on TV (TVs report zero insets; overscan is handled by the spec paddings).
- **Apple TV:** the canvas fills the screen (`.ignoresSafeArea()` for the background and the picture). The **controls and text** keep at least the tvOS safe area: each edge's padding is `max(spec padding, safe-area inset ÷ scale)`; at 1920 × 1080 points this gives top 30, bottom 30, sides 40 ref. Only the idle screen's top row and footer, the name tag, End and toasts move by this; dialogs are centred and unaffected.
- **Phones and tablets, presenter role:** edge to edge with `p-bg` behind the status and navigation bars; content inside `SafeArea`. The bottom action area sits above the home indicator / gesture bar: its 22 px bottom padding is added to the inset. When the keyboard is open, the action area moves above it (Flutter `resizeToAvoidBottomInset: true`). Status bar icons dark on light pages, light on dark pages.
- **Tablets, display role:** immersive full screen (system bars hidden; Android `SystemUiMode.immersiveSticky`, iPadOS `prefersStatusBarHidden` and `prefersHomeIndicatorAutoHidden`), landscape only, `--bg` everywhere.
- **Desktop, display role:** the window opens full screen after setup and on every start in the display role. Leaving full screen uses the system shortcut (F11 on Linux/Windows, Ctrl+Cmd+F on macOS); Escape is Back (spec 7.2), not "leave full screen". In a window, the canvas is letterboxed in `--bg`. Minimum window 640 × 360.
- **Desktop, presenter role:** a normal window, default 480 × 800 (phone layout), resizable; at ≥ 720 wide the laptop layout applies. Window title "Comet Cast". Window background `p-bg`.

## 8. System UI used by the apps (drawn by the system, not designed by us)

| Need | Android | iOS / iPadOS | Linux | Windows | macOS | Apple TV |
| --- | --- | --- | --- | --- | --- | --- |
| Share screen | MediaProjection consent ("Start recording or casting") | `RPSystemBroadcastPickerView` ("Start Broadcast") | xdg-desktop-portal ScreenCast picker | Graphics Capture picker | ScreenCaptureKit picker (`SCContentSharingPicker`, macOS 14+); Screen Recording permission on first use | n/a |
| Choose a file | System document picker (`file_selector`) | `UIDocumentPickerViewController` | Portal / GTK file chooser | Windows file dialog | `NSOpenPanel` | n/a |
| Scan the QR code (`app.scanQr`) | Google code scanner (`GmsBarcodeScanning`, system UI, no camera permission), phones and tablets | VisionKit `DataScannerViewController` (system UI; `NSCameraUsageDescription` = `ios.cameraUsage`), iPhone and iPad | n/a | n/a | n/a | n/a |
| Text input on TV | System IME (Gboard for TV), opened on OK | Full-screen tvOS keyboard, opened on Select | | | | |
| Open a link (link card Open) | `url_launcher`, external browser | n/a (presenter only) | default browser | default browser | default browser | n/a |
| Permissions asked before use | Notifications (Android 13+, for the capture foreground service) when Share screen is first chosen | Local network (when the person taps `nearby.search` the first time, spec 14.2), camera (on Scan) | | | Screen Recording | |

Rules: open a system dialog only as the direct result of the person's choice (spec 5.5: no explainer page before it). Cancelling it returns to the page it came from without an error. A refused permission is handled exactly like a cancelled dialog (back to the page, no error page; spec 5.5: `capture_denied` never shows a page). The system itself tells the person how to allow it later.

TV text input: a text field gets focus like a button (with the ring) and opens the keyboard only on OK. When the keyboard closes with Done/Enter, focus moves to the row's primary button (wizard step 1: Next; step 3 field: Looks right). Up/Down leave the field.

## 9. Not in this round

- **Share sheet entry points** ("Share to Comet Cast" from other apps, Android `ACTION_SEND`, iOS share extension, macOS share menu): not designed, not built.
- The annotation toolbar on native display apps (tablets, computers): not built until the owner decides on drawing tools (spec 12). Existing annotation code in the Flutter receiver stays behind a build flag that is off.
- ~~The list of nearby screens (discovery without a code)~~: now designed for all presenter apps (spec 14, section 4.1 above).
- The Miracast receiver page (Windows) is reachable only through the text link `app.miracast` (spec N18) and is rebuilt with the presenter kit, not redesigned.
- The annotation toolbar stays unreachable in every Flutter build of this round (confirmed in the Flutter review, N20).
- AirPlay, Google Cast and Miracast receiving screens (beyond the existing Windows page above), widgets, notifications other than the system's capture notification, home-screen shortcuts, Picture in Picture, multiple windows.
- Any OLED black for presenter pages; any platform-specific appearance (Material You, Liquid Glass, Mica, vibrancy).

## 10. Flutter: tokens and theme

Put the tokens in one file, `apps/flutter/lib/src/design/tokens.dart`, as `ThemeExtension`s. Values are spec 3.2 / 3.3 exactly.

```dart
@immutable
class DisplayTokens extends ThemeExtension<DisplayTokens> {
  const DisplayTokens({required this.bg, required this.panel, required this.panel2, required this.line,
      required this.fg, required this.mut, required this.acc, required this.focus, required this.btn,
      required this.qrbg, required this.oled});
  final Color bg, panel, panel2, line, fg, mut, acc, focus, btn, qrbg;
  final bool oled; // OLED black: selected tile stays black; check box gets a line border
  static const qrModule = Color(0xFF0E1630);
  static const scrim = Color(0x8C000000); // rgba(0,0,0,.55)
  static const focusGlow = Color(0x2EFFC94D); // rgba(255,201,77,.18)
  static const dark = DisplayTokens(bg: Color(0xFF0E1630), panel: Color(0xFF17213F), panel2: Color(0xFF1F2B50),
      line: Color(0xFF2E3C68), fg: Color(0xFFF3F5FA), mut: Color(0xFFB9C2D8), acc: Color(0xFFFFC266),
      focus: Color(0xFFFFC94D), btn: Color(0xFF24305A), qrbg: Color(0xFFFFFFFF), oled: false);
  static const light = DisplayTokens(bg: Color(0xFFFFFFFF), panel: Color(0xFFF2F4F8), panel2: Color(0xFFE6EAF1),
      line: Color(0xFFB4BCCB), fg: Color(0xFF0A0F1E), mut: Color(0xFF343C51), acc: Color(0xFF7A4300),
      focus: Color(0xFFB35C00), btn: Color(0xFFE3E7EF), qrbg: Color(0xFFFFFFFF), oled: false);
  static const oledBlack = DisplayTokens(bg: Color(0xFF000000), panel: Color(0xFF000000), panel2: Color(0xFF000000),
      line: Color(0xFF2C313C), fg: Color(0xFFE2E6EE), mut: Color(0xFFA1A9BA), acc: Color(0xFFD29E48),
      focus: Color(0xFFDDAE48), btn: Color(0xFF000000), qrbg: Color(0xFFE6E6E6), oled: true);
  Color get selectedTileFill => oled ? bg : Color.alphaBlend(acc.withValues(alpha: .14), panel2);
  Color get overlayFill => panel.withValues(alpha: .92); // name tag, End, badges, pills
  // copyWith / lerp: lerp returns `other` (themes switch at once; no animated colour change)
}
```

`color-mix(in srgb, acc 14%, panel2)` equals `alphaBlend(acc @ 14 %, panel2)`: Dark `#3E4053`, Light `#D7D3CF`.

Presenter tokens (`PresenterTokens`, light and dark) hold every row of spec 3.3, including both border and divider values (`borderInput`, `borderCard`, `divider`, `dividerSoft`), `primarySoft`, `disabled` / `onDisabled`, and `error`, `errorDisc`, `ok`, `okDisc`, `warn`, `warnDisc`.

Presenter `ThemeData` (light shown; dark uses the dark column the same way):

```dart
ThemeData presenterTheme(PresenterTokens p, Brightness b) => ThemeData(
  useMaterial3: true,
  brightness: b,
  fontFamily: 'AtkinsonHyperlegibleNext',
  scaffoldBackgroundColor: p.bg,                          // p-bg
  colorScheme: ColorScheme(
    brightness: b,
    primary: p.primary, onPrimary: p.onPrimary,           // #2D4A8A / #FFFFFF
    primaryContainer: p.primarySoft, onPrimaryContainer: p.primary,
    secondary: p.primary, onSecondary: p.onPrimary,       // no second accent
    surface: p.surface, onSurface: p.fg,                  // p-surface / p-fg
    onSurfaceVariant: p.fg2,                              // body text, labels
    outline: p.borderInput, outlineVariant: p.divider,
    error: p.error, onError: p.surface,
    errorContainer: p.errorDisc, onErrorContainer: p.error,
    surfaceTint: Colors.transparent,                      // no Material tint
  ),
  extensions: [p],
  splashFactory: NoSplash.splashFactory,                  // no ripple (the web has none)
  highlightColor: Colors.transparent,
  hoverColor: p.fg.withValues(alpha: .04),
  focusColor: Colors.transparent,                         // focus = 2 px p-primary outline, drawn by our button style
  textTheme: presenterText(p),                            // section 11
  filledButtonTheme: /* primary: min 50 high, radius 10, 17/700, p-primary; disabled p-disabled / onDisabled */,
  outlinedButtonTheme: /* secondary: min 48, radius 10, p-surface fill, 1 px p-border, 16/500 p-fg (as the web, N23); disabled: p-disabled fill, no border, p-ondisabled text */,
  inputDecorationTheme: /* 46 high, radius 8, p-surface, 1 px border-input; focus 2 px p-primary; error border p-error */,
);
```

The display does not use `ColorScheme` at all: every display widget reads `DisplayTokens` (`Theme.of(context).extension<DisplayTokens>()!`). Automatic appearance: `MediaQuery.platformBrightnessOf(context)` → `dark` or `light`, never `oledBlack` (spec 3.2).

Scaling the display: build every display screen as a `SizedBox(width: 960, height: 540)` with all numbers in reference px and wrap it in `FittedBox(fit: BoxFit.contain)` inside a `ColoredBox(color: tokens.bg)`. Inside the canvas, set `MediaQuery(textScaler: TextScaler.noScaling)` (the canvas scales as a whole). The presenting picture is drawn outside the canvas, full screen on `#000000`; the controls are drawn on the canvas (spec 3.5).

Presenter pages honour the system text size (`MediaQuery.textScalerOf`), clamped to 2.0; every layout must wrap rather than clip at 2.0. Touch targets ≥ 44 × 44 at any scale.

## 11. Type and fonts

Bundle the fonts (SIL OFL 1.1; licence text in `THIRD_PARTY_LICENSES.md`):

| Family name (Flutter `fontFamily`) | Files | Weights | Apple PostScript names |
| --- | --- | --- | --- |
| `AtkinsonHyperlegibleNext` | Latin + Latin Extended subset | 400, 500, 700 | `AtkinsonHyperlegibleNext-Regular`, `-Medium`, `-Bold` |
| `AtkinsonHyperlegibleMono` | Latin subset | 500, 700 | `AtkinsonHyperlegibleMono-Medium`, `-Bold` |

Use the upstream TTF/OTF files of the same version as the web app's `@fontsource` packages (5.3.0 → the font release they package). Never load fonts from the network. Other scripts fall back to the system font (spec 3.4).

Flutter `pubspec.yaml`: two `fonts:` families with the weights above. Apple TV: add the files to the target and list them under `UIAppFonts` in `Info.plist`; use `Font.custom(name, fixedSize:)` on the display canvas (no Dynamic Type on the canvas, it scales as a whole).

Presenter `TextTheme` mapping (spec 3.4 presenter scale; laptop values in brackets):

| Spec role | `TextTheme` slot | Size / weight / height |
| --- | --- | --- |
| Page heading | `headlineSmall` | 25 (24) / 700 / 1.15 |
| Body | `bodyLarge` | 15 / 400 / 1.45 |
| Label | `labelLarge` | 13 / 700 |
| Input | `bodyMedium` (field text) | 17; code field 22 mono, letter spacing .12em, uppercase |
| Option row title / subtitle | `titleMedium` / `bodySmall` | 17 / 700 and 13 / 400 |
| Small print | `labelSmall` (renamed use) | 13 / 400 / 1.4 |
| Primary button | button style text | 17 / 700 |
| Code chip, check number | mono styles (not in `TextTheme`) | 24 mono 700 .08em; 58 mono 700 .08em |

Letter spacing in em: Flutter `letterSpacing` is in logical px, so `em × fontSize` (for example .08em at 58 = 4.64). Line height: Flutter `height` is the multiplier as given. Display type: the spec 3.4 display table, in reference px, as named `TextStyle` constants in `tokens.dart`; do not route display text through `TextTheme`.

SwiftUI: a `DisplayType` enum with one case per row of the display type table (`code`, `address`, `heading`, `roomName`, …) returning `Font.custom(…, fixedSize: ref * scale)` plus `.tracking(em * size * scale)` and `.lineSpacing` for the given line height.

## 12. SwiftUI (Apple TV): tokens

```swift
struct DisplayPalette {
    let bg, panel, panel2, line, fg, mut, acc, focus, btn, qrbg: Color
    let oled: Bool
    static let qrModule = Color(hex: 0x0E1630)
    static let scrim = Color.black.opacity(0.55)
    static let focusGlow = Color(hex: 0xFFC94D).opacity(0.18)
    static let dark = DisplayPalette(bg: .init(hex: 0x0E1630), panel: .init(hex: 0x17213F), panel2: .init(hex: 0x1F2B50),
        line: .init(hex: 0x2E3C68), fg: .init(hex: 0xF3F5FA), mut: .init(hex: 0xB9C2D8), acc: .init(hex: 0xFFC266),
        focus: .init(hex: 0xFFC94D), btn: .init(hex: 0x24305A), qrbg: .init(hex: 0xFFFFFF), oled: false)
    static let light = DisplayPalette(bg: .init(hex: 0xFFFFFF), panel: .init(hex: 0xF2F4F8), panel2: .init(hex: 0xE6EAF1),
        line: .init(hex: 0xB4BCCB), fg: .init(hex: 0x0A0F1E), mut: .init(hex: 0x343C51), acc: .init(hex: 0x7A4300),
        focus: .init(hex: 0xB35C00), btn: .init(hex: 0xE3E7EF), qrbg: .init(hex: 0xFFFFFF), oled: false)
    static let oledBlack = DisplayPalette(bg: .init(hex: 0x000000), panel: .init(hex: 0x000000), panel2: .init(hex: 0x000000),
        line: .init(hex: 0x2C313C), fg: .init(hex: 0xE2E6EE), mut: .init(hex: 0xA1A9BA), acc: .init(hex: 0xD29E48),
        focus: .init(hex: 0xDDAE48), btn: .init(hex: 0x000000), qrbg: .init(hex: 0xE6E6E6), oled: true)
}
```

`Color(hex:)` builds the colour in the **sRGB** colour space (`Color(.sRGB, red:green:blue:)`), never Display P3. The selected-tile fill is the sRGB blend of `acc` at 14 % over `panel2` (Dark `#3E4053`, Light `#D7D3CF`; OLED black: no fill). Pass the palette through an `EnvironmentValues` key; `preferredColorScheme` is not used for our surfaces. Automatic: `@Environment(\.colorScheme)` → `.dark` or `.light`.

Scaling: one `GeometryReader` at the root computes `scale = min(w/960, h/540)` and puts it in the environment; every length is written `r(26)` = `26 * scale`. Do not use `.scaleEffect` for the canvas (it rasterises text and blurs it on a TV).

## 13. Icons

Source of truth: the path data in `apps/web/src/components/Icon.vue` (spec icons) and `apps/web/src/components/ReceiverToolbar.vue` (toolbar, web only). Export each as an SVG (24 × 24, `fill="none"`, stroke `currentColor`, the given stroke width, round caps/joins), checked into `assets/icons/` once, and generate from there:

- Flutter: draw them with one `CustomPainter` that parses the same path strings (`path_drawing` `parseSvgPathData`) or with `flutter_svg` from the SVG files; colour from the token, size from the spec (20 for buttons, 34 offline icon, 28 in presenter discs …).
- Apple TV: asset catalog images from the SVGs with "Preserve Vector Data" and "Render As: Template Image"; tint with the token.

The logo: `assets/logo/cometcast-logo-dark.svg` on Dark and OLED black, `-light.svg` on Light (spec 3.2), as vectors. App icons and the TV banner are unchanged (`docs/android.md`).

## 14. Motion

Durations and easings of spec 3.6, everywhere. Reduced motion (fades instant, focus ring without scale) follows: Android `MediaQuery.disableAnimationsOf(context)` (system "Remove animations"); iOS, iPadOS and tvOS `accessibilityReduceMotion`; macOS "Reduce motion" and Windows "Animation effects" through `MediaQuery.disableAnimationsOf` (Flutter reads both); Linux: the GTK `gtk-enable-animations` setting where Flutter reports it, otherwise motion on. No page transitions: pages replace each other without slide or fade (Flutter `PageRouteBuilder` with zero duration, or no routes at all). No hero animations, no Material ripple, no tvOS focus parallax.

## 15. Idle dimming, wake lock and lifecycle

Dimming (spec 8.6) is drawn by the app: an overlay of `--bg` at 60 % over the idle screen after 7 minutes without activity, identical on every platform. The app never changes the system brightness.

Wake lock (spec D5, 8.7): **only while presenting** (from the first frame until the presentation stops, the session ends or the app goes to the background). Never on Idle; the system screensaver or sleep may follow its own timer.

| Platform | Acquire | Release |
| --- | --- | --- |
| Android TV, Android tablet (display) | `FLAG_KEEP_SCREEN_ON` on the window | clear the flag |
| Apple TV | `UIApplication.shared.isIdleTimerDisabled = true` | `false` |
| iPad (display) | `isIdleTimerDisabled = true` | `false` |
| macOS (display) | `IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep, …)` | `IOPMAssertionRelease` |
| Windows (display) | `SetThreadExecutionState(ES_CONTINUOUS \| ES_DISPLAY_REQUIRED)` | `SetThreadExecutionState(ES_CONTINUOUS)` |
| Linux (display) | xdg-desktop-portal `Inhibit` (flag idle), fallback `org.freedesktop.ScreenSaver.Inhibit` | the matching uninhibit |
| Presenter role, all platforms | none (the system decides; screen capture keeps running under the system's own rules) | |

(Flutter: `wakelock_plus` covers Android, iOS, macOS, Windows and Linux with these calls; it may be used if it does exactly this.)

Lifecycle: when a display app goes to the background (Android `onStop`, tvOS/iPadOS background, desktop window minimised does **not** count), the session ends like End (spec 4.5) without a toast; on return it starts a new session with a new code (as `docs/appletv.md` already does on tvOS). A presenter app in the background keeps a running screen share (system rules) and drops a waiting request only if the system ends the app.

## 16. Copy and languages

- Keys as spec 11.1: web dotted keys; Flutter ARB lower camel case (`settings.join.openText` → `settingsJoinOpenText`); Apple TV string catalog with the dotted keys.
- German "du" is shipped (spec D6, decision 54); "Sie" stays switchable in one file swap: Flutter keeps two German ARB sets (`lib/l10n/app_de.arb` = du, `lib/l10n/sie/app_de.arb` = Sie) and `l10n.yaml` points `arb-dir` to the chosen set at build time; Apple TV keeps `Localizable.xcstrings` (du) and `Localizable-sie.xcstrings` (Sie) and reads the table named by one build setting. The iOS and tvOS `InfoPlist.strings` (local-network text) follow the same set.
- `.Remote` vs `.Pointer` variants (`setup.done.body*`, `settings.close*`): Remote on TVs; Pointer on tablets and computers.
- Language: system language, German or English, else English (`docs/i18n.md`). No language switch in the app.

## 17. Accessibility mapping

| Spec 9 item | Flutter | SwiftUI (tvOS) |
| --- | --- | --- |
| Join request is an alert dialog labelled "Request: “{name}”" | `Semantics(scopesRoute: true, namesRoute: true, label: request.aria)`, announce on open | `.accessibilityAddTraits(.isModal)`, `.accessibilityLabel(request.aria)`, post `.screenChanged` |
| Tiles are radio groups | `Semantics(inMutuallyExclusiveGroup: true, checked: selected)` | `.accessibilityAddTraits(selected ? .isSelected : [])`, grouped container |
| Toasts are polite live regions | `SemanticsService.announce(text, dir)` (no focus) | `AccessibilityNotification.Announcement(text).post()` |
| Settings button labelled by its word | visible text is the label | same |
| QR labelled "QR: {url}" | `Semantics(label: …, image: true)` | `.accessibilityLabel` |
| Status line for screen readers | `Semantics(liveRegion: true)` on an invisible text | announcement on change |

Contrast is guaranteed by the tokens; do not add platform contrast modes on top (Increase Contrast may only thicken borders from 1 to 2 px).

## 18. Known differences in the current native builds that this round changes

These are as built today (`docs/android.md`, `docs/appletv.md`) and differ from the spec. They are not design questions; build the spec.

- Flutter `lib/main.dart` uses `ColorScheme.fromSeed` and Material focus borders; `fontFamily: 'monospace'` for codes → sections 10 and 11.
- Apple TV idle screen: "three steps, server in use and its source, Show room code, Change server", no QR code → spec 4.2 idle screen with QR, Settings button, no server change outside setup (spec 10).
- Apple TV: "Menu on the receive screen asks Stop receiving?" → spec 7.1 (Back shows/hides controls; End without confirmation).
- Apple TV: the idle timer is disabled while a room is open → only while presenting (section 15).
- Apple TV link card: "Close button that has the default focus", content shown with a Close button → spec 4.5 link card (no-browser variant) and no close buttons on content (spec 10).
- Apple TV: the connection check code as a badge on the video → second line of the name tag (spec 4.5), visible only with the controls.
- Flutter receiver: statistics overlay, codec card and annotation toolbar on the display → statistics only in debug builds; the codec consent as the dialog of spec 4.7 (display) and the page of spec 5.8 (presenter), not as toasts; toolbar off (section 9).

## 19. Acceptance per platform

A platform build is accepted for this round when, in all three display appearances (TV, tablet and desktop display) and both presenter appearances:

1. Screenshots of every state in spec 4 and every page in spec 5–6 match the web screenshots of the same state in layout, tokens, type and copy (pixel differences from font rendering are fine; no different colour, size or position).
2. Every focus row of spec 7.1 works with the platform's keys (section 6), with the spec ring and no system focus effect.
3. The wake lock is held only while presenting (checked by letting the device reach its screensaver timeout on Idle).
4. Reduced motion removes the fades and the ring scale.
5. Switching the German file set from du to Sie changes every addressing string and nothing else.
6. Presenter apps: the states of spec 14.2 (not asked, searching, found, nothing found, paused, not allowed, no local network) and the tap outcomes of 14.4 (open, approval, other relay, gone) match the mockup and spec 14 on a real network with two displays of the same name; a display that is switched off leaves the list within 30 s.

## 20. Core interface and platform integration the apps need (Flutter review)

Added after the Flutter review (spec decisions N1–N9, N26). These are engineering requirements derived from the design; names are proposals, the behaviour is binding. Keep them minimal: nothing here adds a screen.

### 20.1 Presenter session (`cc-ffi`, `cc-sender`)

One session per join, replacing "join when sending" (N5):

- `start_presenter(server, code, name, events: StreamSink<PresenterUiEvent>)` joins the room at once (on Connect). `stop_presenter()` leaves (Disconnect, withdraw while waiting, app ends).
- Inside a joined session: `present_screen(…)` (desktop: the system picker, 20.3; mobile: the existing capture paths), `present_content(source)` (file or link), `send_content_cmd(cmd)` (pages), `stop_presenting()`. Stopping ends only the presentation: the display returns to Idle with the same code and the presenter stays joined. A new presentation in the same session needs no new approval. Video and content use the same peer connection (renegotiation or a pre-negotiated track; the engineers choose).
- `PresenterUiEvent`: `WaitingForApproval { timeout_secs, check, room_name: Option<String> }`, `Joined { room_name: Option<String> }`, `ConnectionCode { sas: String }` (N2), `Page { page: u32, pages: u32 }` (N3), `Progress { sent, total }`, `Presenting`, `PresentingStopped` (also when the system ended the capture), `Ended { code }` (`removed_by_host`, the room ended, the display left), `Error { code: String, message: String }` (N9; codes in implementation-questions-flutter.md C9). `room_name` follows decision 49 (lenient).

### 20.2 Receiver (`cc-ffi`, `cc-receiver`)

- `start_receiver(…, room_name: Option<String>, …)` → `create_room.room_name` (N1).
- Keep the room after `PeerLeft` and `remove_peer`: fresh WebRTC session and content task for the next presenter, same signalling connection, code, join mode, blocks and queue; the event stream stays open (N8). A presenter's `stop_presenting` is reported as `PresentationStopped` (display → Idle, no toast), a leave as `PeerLeft` (display → Idle, `toast.left`).
- `ConnectionCode { sas }` once per presenter connection (N2).
- `ContentTransferProgress { received, total }`, at most one per percent and four per second (N4).
- Send `{"kind":"page","page":n,"pages":m}` on the control channel when the page count is known and on every page change (N3).
- `Error { code, message }` with the same code rule as 20.1; the display picks the offline body by code (spec 4.2).

### 20.3 Desktop capture (`cc-capture`)

Screen sharing on Linux, Windows and macOS opens the system picker directly (section 8): xdg-desktop-portal ScreenCast, `GraphicsCapturePicker`, `SCContentSharingPicker`. Cancel or refusal = `capture_denied` (no error page). No monitor list, duration or test stream in the UI (N26).

### 20.4 Links, install referrer and scanner (Flutter + platform)

- **Accepted URL forms** (link, referrer, scanned QR): `https://{host}/{CODE}` and `https://{host}/{CODE}?server={server}`, `{CODE}` = 6 characters of the code alphabet (spaces and dashes removed; anything else = no code). The server is `{server}` if given, else `{host}`.
- **App Links / Universal Links:** only for the built-in relay's host (`assetlinks.json`, `apple-app-site-association` served there, path `/*` with a 6-character code). Other hosts stay browser links.
- **Android install referrer:** `code` and `server` parameters, same rules (spec 12: still an assumption; without it Android behaves like iOS).
- **Trust:** a server that is the built-in relay, the policy server, the DNS server, the in-house box or confirmed on this device before is used at once (chip). Any other server opens the relay confirmation page (spec 7.2, `relay.confirm.bodyApp`) before anything connects; Use stores it as confirmed. A link never changes the stored "last used" server until a join succeeded.
- **Scanner:** section 8; result handling spec 5.3 (`app.scanWrong` for anything else).

### 20.5 iOS screen sharing: hand-over of the join (N32)

The ReplayKit extension must make its own connection (the app is suspended while another app is in front; docs/ios.md, design A). The host must still approve a person once per connection (N5). Behaviour (binding; carrier names are proposals):

1. **Before the broadcast** (person taps Screen, the system picker is confirmed): the app's presenter session asks the display for a hand-over over the content control channel, `{"kind":"handover","token":T}`; T is 128 random bits (base64url), valid 60 s and once. The display stores T with the presenter's name and answers `{"kind":"handover_ok"}`. The app writes T, the name, the code and the server into `config.json` (App Group, as today), then leaves the room.
2. **Leaving with a pending hand-over** is not a leave for the display: no `toast.left`; the display stays in Presenting on black (or Idle if nothing was shown) for at most 60 s.
3. **The extension joins** with the person's name and T in an optional join field (`handover`, at most 64 characters, relayed unchanged by the server in `join_request` to the host). A display that holds T admits the join at once without the request dialog (in Open and Approval on), with no `toast.connected`, and drops T. A display without T (expired, End, another display, an old server that drops the field) handles it as a normal join request; the presenter then sees the normal waiting flow (step 6).
4. **While the extension runs**, the app shows the Presenting page (5.6, `presenting.screen`) from the extension's status (`streaming`), with `presenting.where` and `presenting.sas` from the status file (the extension writes `room_name` and `sas`). Stop in the app writes `stop`; the red status pill or Control Center stop the same way.
5. **After the broadcast** the extension asks the display for a new token the same way before it leaves and writes it to `status.json`; the app, when active again, joins with it and lands on "What to show" (connected, header with the room name), with no dialog on the display.
6. **Fallbacks, no new copy:** if a hand-over join is not admitted at once, the app shows the waiting page (5.4) with the check number and countdown the extension writes to the status file, and the normal error pages for `join_denied`, `approval_timeout`, `room_locked` (Locked refuses hand-over joins; accepted). If the rejoin after the broadcast fails, the app joins with the code like Connect (waiting page in Approval on).
7. **Until 1–5 ship** (test builds only): option (a) of F2 with the fixes of N32 (name sent, waiting page with the extension's check number, rejoin with the same code to "What to show" after the broadcast).

The web display must accept the same `handover` messages to keep the promise against web displays; until it does, iOS against a web display uses step 6.

### 20.6 Rules from the second Flutter review

- Error and failure lines show the key's text only; technical reasons (`message`, "network error", "no library") go to the log (N46).
- `capture_unavailable` on a desktop without a capture backend: `error.other` with `errors.capture_unavailable`, action `common.back` (N33).

