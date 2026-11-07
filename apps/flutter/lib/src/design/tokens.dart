// SPDX-License-Identifier: GPL-3.0-or-later
// The design tokens of the product: colours, type and sizes. Source of every value:
// docs/design/first-run-and-display.md (3.2 to 3.5) and docs/design/native-apps.md (10, 11).
// Nothing here is a design decision of the app; a value that is not in those documents does not
// belong in this file.

import 'package:flutter/material.dart';

/// The family names of the bundled fonts (pubspec.yaml, assets/fonts).
const fontUi = 'AtkinsonHyperlegibleNext';
const fontUiExt = 'AtkinsonHyperlegibleNextExt';
const fontMono = 'AtkinsonHyperlegibleMono';

/// What the person chose in Settings (spec 4.3). Automatic follows the system and never picks
/// OLED black (spec 3.2).
enum Appearance { dark, light, oled, auto }

/// Colours of the display (TV app and display role of tablets and computers), spec 3.2.
@immutable
class DisplayTokens extends ThemeExtension<DisplayTokens> {
  const DisplayTokens({
    required this.bg,
    required this.panel,
    required this.panel2,
    required this.line,
    required this.fg,
    required this.mut,
    required this.acc,
    required this.focus,
    required this.btn,
    required this.qrbg,
    required this.oled,
    required this.light,
  });

  final Color bg, panel, panel2, line, fg, mut, acc, focus, btn, qrbg;

  /// OLED black: the selected tile stays black, the check-number box gets a line border.
  final bool oled;

  /// The Light appearance: the QR box gets a 1 px line border (spec 3.2, decision 13.33).
  final bool light;

  /// QR code dots: always dark on light.
  static const qrModule = Color(0xFF0E1630);

  /// Behind the join request and Settings: rgba(0,0,0,.55).
  static const scrim = Color(0x8C000000);

  /// Focus glow: rgba(255,201,77,.18).
  static const focusGlow = Color(0x2EFFC94D);

  /// The picture's own background while presenting.
  static const stage = Color(0xFF000000);

  static const dark = DisplayTokens(
    bg: Color(0xFF0E1630),
    panel: Color(0xFF17213F),
    panel2: Color(0xFF1F2B50),
    line: Color(0xFF2E3C68),
    fg: Color(0xFFF3F5FA),
    mut: Color(0xFFB9C2D8),
    acc: Color(0xFFFFC266),
    focus: Color(0xFFFFC94D),
    btn: Color(0xFF24305A),
    qrbg: Color(0xFFFFFFFF),
    oled: false,
    light: false,
  );

  static const lightTheme = DisplayTokens(
    bg: Color(0xFFFFFFFF),
    panel: Color(0xFFF2F4F8),
    panel2: Color(0xFFE6EAF1),
    line: Color(0xFFB4BCCB),
    fg: Color(0xFF0A0F1E),
    mut: Color(0xFF343C51),
    acc: Color(0xFF7A4300),
    focus: Color(0xFFB35C00),
    btn: Color(0xFFE3E7EF),
    qrbg: Color(0xFFFFFFFF),
    oled: false,
    light: true,
  );

  static const oledBlack = DisplayTokens(
    bg: Color(0xFF000000),
    panel: Color(0xFF000000),
    panel2: Color(0xFF000000),
    line: Color(0xFF2C313C),
    fg: Color(0xFFE2E6EE),
    mut: Color(0xFFA1A9BA),
    acc: Color(0xFFD29E48),
    focus: Color(0xFFDDAE48),
    btn: Color(0xFF000000),
    qrbg: Color(0xFFE6E6E6),
    oled: true,
    light: false,
  );

  /// The tokens for [a]; [platform] is the system brightness (used by Automatic only).
  static DisplayTokens of(Appearance a, Brightness platform) => switch (a) {
    Appearance.dark => dark,
    Appearance.light => lightTheme,
    Appearance.oled => oledBlack,
    Appearance.auto => platform == Brightness.dark ? dark : lightTheme,
  };

  /// Selected tile fill: `color-mix(in srgb, acc 14%, panel2)`; OLED black: no fill (spec 3.2).
  Color get selectedTileFill =>
      oled ? bg : Color.alphaBlend(acc.withValues(alpha: .14), panel2);

  /// Name tag, End, badges, pills: `color-mix(in srgb, panel 92%, transparent)`.
  Color get overlayFill => panel.withValues(alpha: .92);

  /// The logo file for this appearance (spec 3.2): dark on Dark and OLED black.
  String get logoAsset => light
      ? 'assets/logo/cometcast-logo-light.svg'
      : 'assets/logo/cometcast-logo-dark.svg';

  @override
  DisplayTokens copyWith({
    Color? bg,
    Color? panel,
    Color? panel2,
    Color? line,
    Color? fg,
    Color? mut,
    Color? acc,
    Color? focus,
    Color? btn,
    Color? qrbg,
    bool? oled,
    bool? light,
  }) => DisplayTokens(
    bg: bg ?? this.bg,
    panel: panel ?? this.panel,
    panel2: panel2 ?? this.panel2,
    line: line ?? this.line,
    fg: fg ?? this.fg,
    mut: mut ?? this.mut,
    acc: acc ?? this.acc,
    focus: focus ?? this.focus,
    btn: btn ?? this.btn,
    qrbg: qrbg ?? this.qrbg,
    oled: oled ?? this.oled,
    light: light ?? this.light,
  );

  /// Themes switch at once (native-apps.md 10): no animated colour change.
  @override
  DisplayTokens lerp(ThemeExtension<DisplayTokens>? other, double t) =>
      other is DisplayTokens ? other : this;
}

/// Display type (spec 3.4), in reference px of the 960 x 540 canvas. Line height is the spec's
/// multiplier; letter spacing is em times size.
class DisplayType {
  const DisplayType(this.t);
  final DisplayTokens t;

  static const _fallback = [fontUiExt];

  TextStyle _ui(
    double size,
    FontWeight w,
    Color c, {
    double? height,
    double em = 0,
  }) => TextStyle(
    fontFamily: fontUi,
    fontFamilyFallback: _fallback,
    fontSize: size,
    fontWeight: w,
    color: c,
    height: height,
    letterSpacing: em * size,
    decoration: TextDecoration.none,
  );

  TextStyle _mono(
    double size,
    FontWeight w,
    Color c, {
    double? height,
    double em = 0,
  }) => TextStyle(
    fontFamily: fontMono,
    fontSize: size,
    fontWeight: w,
    color: c,
    height: height,
    letterSpacing: em * size,
    decoration: TextDecoration.none,
  );

  static const bold = FontWeight.w700;
  static const medium = FontWeight.w500;
  static const regular = FontWeight.w400;

  /// Join code: 80 / 700 mono, 1.0, .06em.
  TextStyle get code => _mono(80, bold, t.fg, height: 1.0, em: .06);

  /// Address: 58 / 700, 1.05, -.01em.
  TextStyle get address => _ui(58, bold, t.fg, height: 1.05, em: -.01);

  /// Wizard heading and request name: 32 / 700, 1.15.
  TextStyle get heading => _ui(32, bold, t.fg, height: 1.15);

  /// Room name: 30 / 700, 1.05.
  TextStyle get roomName => _ui(30, bold, t.fg, height: 1.05);

  /// Address on the wizard's address card: 30 / 700.
  TextStyle get cardAddress => _ui(30, bold, t.fg);

  /// Step number disc: 21 / 700 in `--bg`.
  TextStyle get stepNumber => _ui(21, bold, t.bg);

  /// Wizard choice title: 21 / 700.
  TextStyle get choiceTitle => _ui(21, bold, t.fg);

  /// Mode pill: 17 / 700.
  TextStyle get pill => _ui(17, bold, t.fg);

  /// Step labels (17 / 400 `--mut`).
  TextStyle get stepLabel => _ui(17, regular, t.mut);

  /// Settings button, toast: 17 / 700.
  TextStyle get label17Bold => _ui(17, bold, t.fg);

  /// Wizard body: 18 / 400 / 1.4 `--mut`.
  TextStyle get body => _ui(18, regular, t.mut, height: 1.4);

  /// Wizard and dialog buttons: 19 / 700.
  TextStyle get button => _ui(19, bold, t.fg);

  /// Chips: 18.
  TextStyle get chip => _ui(18, regular, t.fg);

  /// Mode tile: 16 / 500, selected 700.
  TextStyle modeTile(bool selected) => _ui(16, selected ? bold : medium, t.fg);

  /// Appearance tile: 15 / 500, selected 700.
  TextStyle lookTile(bool selected) => _ui(15, selected ? bold : medium, t.fg);

  /// Settings description: 15 `--mut`.
  TextStyle get description => _ui(15, regular, t.mut);

  /// Footer line, QR caption: 15 / 1.3 `--mut`.
  TextStyle get footer => _ui(15, regular, t.mut, height: 1.3);

  /// Name tag line 1: 15 / 700 `--fg`.
  TextStyle get nameTag => _ui(15, bold, t.fg, height: 1.3);

  /// Name tag line 2 (security code): 13 / 400 `--mut`.
  TextStyle get nameTagSub => _ui(13, regular, t.mut, height: 1.3);

  /// Section label in dialogs: 14 / 700, uppercase, .08em.
  TextStyle get sectionLabel => _ui(14, bold, t.mut, height: 1.3, em: .08);

  /// Small caps label (check): 11 / 400, uppercase, .06em.
  TextStyle get smallCaps => _ui(11, regular, t.mut, em: .06);

  /// Check number in the request: 24 / 700 mono, 1.1.
  TextStyle get checkNumber => _mono(24, bold, t.fg, height: 1.1);

  /// Eyebrow of a wizard choice card: 13 / 700 uppercase `--acc`, .05em.
  TextStyle get eyebrow => _ui(13, bold, t.acc, em: .05);

  /// Wizard choice text: 16 / 1.35 `--mut`.
  TextStyle get choiceText => _ui(16, regular, t.mut, height: 1.35);

  /// Text field: 22.
  TextStyle get field => _ui(22, regular, t.fg);

  /// Offline box title: 22 / 700. Body 18 / 1.35.
  TextStyle get offlineTitle => _ui(22, bold, t.fg, height: 1.35);
  TextStyle get offlineBody => _ui(18, regular, t.fg, height: 1.35);

  /// Link address in the link card: 18 mono `--mut`.
  TextStyle get linkAddress => _mono(18, medium, t.mut, height: 1.3);

  /// Card text of the link card: 16 / 1.4 `--mut`.
  TextStyle get cardText => _ui(16, regular, t.mut, height: 1.4);

  /// Ghost button: 14.
  TextStyle get ghost => _ui(14, regular, t.fg);

  /// Close button in Settings: 14 `--mut`.
  TextStyle get close => _ui(14, regular, t.mut);

  /// Step n of 4: 15 `--mut`.
  TextStyle get dots => _ui(15, regular, t.mut);
}

/// Colours of the presenter pages, spec 3.3. Light and dark follow the device; OLED black does
/// not exist here.
@immutable
class PresenterTokens extends ThemeExtension<PresenterTokens> {
  const PresenterTokens({
    required this.bg,
    required this.surface,
    required this.borderInput,
    required this.borderCard,
    required this.divider,
    required this.dividerSoft,
    required this.fg,
    required this.heading,
    required this.fg2,
    required this.fg3,
    required this.primary,
    required this.onPrimary,
    required this.primarySoft,
    required this.disabled,
    required this.onDisabled,
    required this.error,
    required this.errorDisc,
    required this.ok,
    required this.okDisc,
    required this.warn,
    required this.warnDisc,
    required this.mutDisc,
    required this.onMutDisc,
  });

  final Color bg, surface;

  /// `p-border` of inputs, buttons and option rows.
  final Color borderInput;

  /// `p-border` of cards (dark: #343944, light: same as input border in the CSS: #E2E4E9 card edge).
  final Color borderCard;

  /// `p-divider`: the "or" rule (first value of the pair).
  final Color divider;

  /// `p-divider` second value: card edges, progress track.
  final Color dividerSoft;
  final Color fg, heading, fg2, fg3;
  final Color primary, onPrimary, primarySoft, disabled, onDisabled;
  final Color error, errorDisc, ok, okDisc, warn, warnDisc;

  /// Grey icon discs (locked, ended, disconnected): the web's `p-mut-bg` / `p-mut-fg`.
  final Color mutDisc, onMutDisc;

  static const light = PresenterTokens(
    bg: Color(0xFFF4F5F8),
    surface: Color(0xFFFCFCFD),
    borderInput: Color(0xFFCDD1D9),
    borderCard: Color(0xFFE2E4E9),
    divider: Color(0xFFD4D8E0),
    dividerSoft: Color(0xFFE2E4E9),
    fg: Color(0xFF1C2027),
    heading: Color(0xFF1C2027),
    fg2: Color(0xFF3F4652),
    fg3: Color(0xFF646C7B),
    primary: Color(0xFF2D4A8A),
    onPrimary: Color(0xFFFFFFFF),
    primarySoft: Color(0xFFE7ECF6),
    disabled: Color(0xFFDDE1E8),
    onDisabled: Color(0xFF4A5160),
    error: Color(0xFFB5483C),
    errorDisc: Color(0xFFF6E1DE),
    ok: Color(0xFF2C7A51),
    okDisc: Color(0xFFDDF0E5),
    warn: Color(0xFF93650C),
    warnDisc: Color(0xFFF8EFDC),
    mutDisc: Color(0xFFE2E4E9),
    onMutDisc: Color(0xFF3F4652),
  );

  static const dark = PresenterTokens(
    bg: Color(0xFF14171D),
    surface: Color(0xFF1D2128),
    borderInput: Color(0xFF3B404B),
    borderCard: Color(0xFF343944),
    divider: Color(0xFF343944),
    dividerSoft: Color(0xFF2B2F38),
    fg: Color(0xFFE8EAEF),
    heading: Color(0xFFF1F3F7),
    fg2: Color(0xFFBCC3CF),
    fg3: Color(0xFF9EA6B3),
    primary: Color(0xFF93AAE6),
    onPrimary: Color(0xFF0F1420),
    primarySoft: Color(0xFF222A3D),
    disabled: Color(0xFF2A2F38),
    onDisabled: Color(0xFF9EA6B3),
    error: Color(0xFFF08C80),
    errorDisc: Color(0xFF342A2C),
    ok: Color(0xFF6CC597),
    okDisc: Color(0xFF21312F),
    warn: Color(0xFFE3B45A),
    warnDisc: Color(0xFF332F26),
    mutDisc: Color(0xFF2B2F38),
    onMutDisc: Color(0xFFC9CED8),
  );

  static PresenterTokens of(Brightness b) =>
      b == Brightness.dark ? dark : light;

  @override
  PresenterTokens copyWith({Color? bg}) => PresenterTokens(
    bg: bg ?? this.bg,
    surface: surface,
    borderInput: borderInput,
    borderCard: borderCard,
    divider: divider,
    dividerSoft: dividerSoft,
    fg: fg,
    heading: heading,
    fg2: fg2,
    fg3: fg3,
    primary: primary,
    onPrimary: onPrimary,
    primarySoft: primarySoft,
    disabled: disabled,
    onDisabled: onDisabled,
    error: error,
    errorDisc: errorDisc,
    ok: ok,
    okDisc: okDisc,
    warn: warn,
    warnDisc: warnDisc,
    mutDisc: mutDisc,
    onMutDisc: onMutDisc,
  );

  @override
  PresenterTokens lerp(ThemeExtension<PresenterTokens>? other, double t) =>
      other is PresenterTokens ? other : this;
}

/// Presenter type (spec 3.4), in logical px. [laptop] is the laptop layout (>= 720 px wide).
class PresenterType {
  const PresenterType(this.p, {this.laptop = false});
  final PresenterTokens p;
  final bool laptop;

  static const _fallback = [fontUiExt];

  TextStyle _ui(double size, FontWeight w, Color c, {double? height}) =>
      TextStyle(
        fontFamily: fontUi,
        fontFamilyFallback: _fallback,
        fontSize: size,
        fontWeight: w,
        color: c,
        height: height,
      );

  TextStyle _mono(double size, FontWeight w, Color c, {double em = 0}) =>
      TextStyle(
        fontFamily: fontMono,
        fontSize: size,
        fontWeight: w,
        color: c,
        letterSpacing: em * size,
      );

  /// Page heading: 25 (laptop 24) / 700 / 1.15.
  TextStyle get heading =>
      _ui(laptop ? 24 : 25, FontWeight.w700, p.heading, height: 1.15);

  /// Body: 15 / 400 / 1.45.
  TextStyle get body => _ui(15, FontWeight.w400, p.fg2, height: 1.45);

  /// Label: 13 / 700.
  TextStyle get label => _ui(13, FontWeight.w700, p.fg2);

  /// Small print: 13 / 400 / 1.4.
  TextStyle get small => _ui(13, FontWeight.w400, p.fg3, height: 1.4);

  /// Error text: 13 / 700 `p-error`.
  TextStyle get errorText => _ui(13, FontWeight.w700, p.error);

  /// Input: 17.
  TextStyle get input => _ui(17, FontWeight.w400, p.fg);

  /// Code input: 22 mono, .12em.
  TextStyle get codeInput => _mono(22, FontWeight.w500, p.fg, em: .12);

  /// Code chip: 24 mono 700, .08em.
  TextStyle get codeChip => _mono(24, FontWeight.w700, p.fg, em: .08);

  /// Check number: 58 mono 700, .08em, `p-primary`.
  TextStyle get checkNumber => _mono(58, FontWeight.w700, p.primary, em: .08);

  /// Option row title 17 / 700, subtitle 13 / 400.
  TextStyle get optionTitle => _ui(17, FontWeight.w700, p.fg);
  TextStyle get optionSub => _ui(13, FontWeight.w400, p.fg3);

  /// Primary button: 17 / 700.
  TextStyle get primaryButton => _ui(17, FontWeight.w700, p.onPrimary);

  /// Secondary button: 16 / 500 (the web's `.cp-sbtn`).
  TextStyle get secondaryButton => _ui(16, FontWeight.w500, p.fg);

  /// Brand row: 16 / 700.
  TextStyle get brand => _ui(16, FontWeight.w700, p.fg);

  /// Error card labels (`error.why`, `error.now`): 12 / 700 uppercase, .06em, `p-fg3`.
  TextStyle get cardLabel => TextStyle(
    fontFamily: fontUi,
    fontFamilyFallback: _fallback,
    fontSize: 12,
    fontWeight: FontWeight.w700,
    color: p.fg3,
    letterSpacing: .06 * 12,
  );

  /// Security code digits in small print: mono 700 `p-fg`.
  TextStyle get smallDigits => _mono(13, FontWeight.w700, p.fg);

  /// Host on the relay page: 17 mono 700 `p-fg`.
  TextStyle get host => _mono(17, FontWeight.w700, p.fg);
}

/// The presenter ThemeData of native-apps.md 10. Material is used only as a carrier for text
/// fields, scrolling and system integration; every page reads [PresenterTokens].
ThemeData presenterTheme(Brightness b) {
  final p = PresenterTokens.of(b);
  final scheme = ColorScheme(
    brightness: b,
    primary: p.primary,
    onPrimary: p.onPrimary,
    primaryContainer: p.primarySoft,
    onPrimaryContainer: p.primary,
    secondary: p.primary,
    onSecondary: p.onPrimary,
    surface: p.surface,
    onSurface: p.fg,
    onSurfaceVariant: p.fg2,
    outline: p.borderInput,
    outlineVariant: p.divider,
    error: p.error,
    onError: p.surface,
    errorContainer: p.errorDisc,
    onErrorContainer: p.error,
    surfaceTint: Colors.transparent,
  );
  return ThemeData(
    useMaterial3: true,
    brightness: b,
    fontFamily: fontUi,
    fontFamilyFallback: const [fontUiExt],
    scaffoldBackgroundColor: p.bg,
    colorScheme: scheme,
    extensions: [p],
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    hoverColor: p.fg.withValues(alpha: .04),
    focusColor: Colors.transparent,
    textTheme: const TextTheme().apply(
      bodyColor: p.fg,
      displayColor: p.fg,
      fontFamily: fontUi,
    ),
    // No page transitions: pages replace each other (native-apps.md 14).
    pageTransitionsTheme: const PageTransitionsTheme(
      builders: {
        TargetPlatform.android: _NoTransitions(),
        TargetPlatform.iOS: _NoTransitions(),
        TargetPlatform.linux: _NoTransitions(),
        TargetPlatform.macOS: _NoTransitions(),
        TargetPlatform.windows: _NoTransitions(),
      },
    ),
  );
}

class _NoTransitions extends PageTransitionsBuilder {
  const _NoTransitions();
  @override
  Widget buildTransitions<T>(
    PageRoute<T> route,
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) => child;
}

/// ThemeData behind the display: only the carrier for the [DisplayTokens]; no Material colours
/// are used by display widgets (native-apps.md 10).
ThemeData displayTheme(DisplayTokens t) => ThemeData(
  useMaterial3: true,
  brightness: t.light ? Brightness.light : Brightness.dark,
  fontFamily: fontUi,
  fontFamilyFallback: const [fontUiExt],
  scaffoldBackgroundColor: t.bg,
  colorScheme: ColorScheme(
    brightness: t.light ? Brightness.light : Brightness.dark,
    primary: t.acc,
    onPrimary: t.bg,
    secondary: t.acc,
    onSecondary: t.bg,
    surface: t.panel,
    onSurface: t.fg,
    error: t.fg,
    onError: t.bg,
    surfaceTint: Colors.transparent,
  ),
  extensions: [t],
  splashFactory: NoSplash.splashFactory,
  highlightColor: Colors.transparent,
  hoverColor: Colors.transparent,
  focusColor: Colors.transparent,
);
