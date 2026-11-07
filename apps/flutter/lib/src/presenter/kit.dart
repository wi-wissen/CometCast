// SPDX-License-Identifier: GPL-3.0-or-later
// The building blocks of the presenter (sender) pages: page frame, buttons, fields, icon discs,
// cards (docs/design/first-run-and-display.md 3.3 to 3.5, 5, 6, 7.3, 7.4; native-apps.md 4, 6.3,
// 7, 10). Sizes in logical px.

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../design/icons.dart';
import '../design/tokens.dart';

/// Window width from which the laptop layout applies (spec 7.4).
const laptopFrom = 720.0;

class PresenterScope extends InheritedWidget {
  const PresenterScope({
    super.key,
    required this.tokens,
    required this.laptop,
    required super.child,
  });

  final PresenterTokens tokens;
  final bool laptop;

  PresenterType get type => PresenterType(tokens, laptop: laptop);

  static PresenterScope of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<PresenterScope>()!;

  @override
  bool updateShouldNotify(PresenterScope old) =>
      tokens != old.tokens || laptop != old.laptop;
}

/// Gives the pages their tokens (from the device's light or dark setting) and layout class.
class PresenterFrame extends StatelessWidget {
  const PresenterFrame({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final b = MediaQuery.platformBrightnessOf(context);
    final p = PresenterTokens.of(b);
    final w = MediaQuery.sizeOf(context).width;
    return Theme(
      data: presenterTheme(b),
      child: PresenterScope(
        tokens: p,
        laptop: w >= laptopFrom,
        child: ColoredBox(
          color: p.bg,
          child: DefaultTextStyle(
            style: PresenterType(p, laptop: w >= laptopFrom).body,
            child: child,
          ),
        ),
      ),
    );
  }
}

/// One page: padding 18 22 22 (phone), laptop top `max(40, 10 % of the height)`, sides 40,
/// bottom 40; vertical gap 14; max content width 744. On the phone [actions] sit at the bottom
/// (a spacer grows); on the laptop they follow the content, 24 below it (spec 7.4). The page
/// scrolls when the text size makes it taller than the window.
class PPage extends StatelessWidget {
  const PPage({
    super.key,
    required this.children,
    this.actions = const [],
    this.actionsRow = false,
    this.bottomReserve = 0,
  });

  final List<Widget> children;

  /// Buttons at the bottom of the page.
  final List<Widget> actions;

  /// Laptop: the actions are a row (error pages); else stacked.
  final bool actionsRow;

  /// Extra space below the actions (reserved for a system bar).
  final double bottomReserve;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final mq = MediaQuery.of(context);
    final laptop = s.laptop;
    final height = mq.size.height;
    final pad = laptop
        ? EdgeInsets.fromLTRB(40, math.max(40, height * .1), 40, 40)
        : const EdgeInsets.fromLTRB(22, 18, 22, 22);
    final spaced = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      if (i > 0) spaced.add(const SizedBox(height: 14));
      spaced.add(children[i]);
    }
    Widget actionArea() {
      if (actions.isEmpty) return const SizedBox.shrink();
      if (laptop && actionsRow) {
        return Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [for (final a in actions) a],
        );
      }
      final stacked = <Widget>[];
      for (var i = 0; i < actions.length; i++) {
        if (i > 0) stacked.add(const SizedBox(height: 8));
        stacked.add(actions[i]);
      }
      return Column(
        crossAxisAlignment: laptop
            ? CrossAxisAlignment.start
            : CrossAxisAlignment.stretch,
        children: stacked,
      );
    }

    return SafeArea(
      child: LayoutBuilder(
        builder: (context, box) {
          return SingleChildScrollView(
            child: ConstrainedBox(
              constraints: BoxConstraints(minHeight: box.maxHeight),
              child: IntrinsicHeight(
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 744),
                    child: Padding(
                      padding: pad,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          ...spaced,
                          if (laptop)
                            // The action row follows the content, 24 below it (14 gap + 10).
                            const SizedBox(height: 24)
                          else
                            const Spacer(),
                          actionArea(),
                          if (bottomReserve > 0) SizedBox(height: bottomReserve),
                          if (laptop) const Spacer(),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// Tap target with the presenter's keyboard focus outline (2 px `p-primary`, offset 2), drawn
/// only for keyboard focus; no ripple, no Material focus look.
class PTap extends StatefulWidget {
  const PTap({
    super.key,
    required this.onPressed,
    required this.radius,
    required this.builder,
    this.autofocus = false,
    this.label,
    this.focusNode,
  });

  final VoidCallback? onPressed;
  final double radius;
  final Widget Function(BuildContext context) builder;
  final bool autofocus;
  final String? label;
  final FocusNode? focusNode;

  @override
  State<PTap> createState() => _PTapState();
}

class _PTapState extends State<PTap> {
  bool _focus = false;

  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    final enabled = widget.onPressed != null;
    return Semantics(
      button: true,
      enabled: enabled,
      label: widget.label,
      container: true,
      child: ExcludeSemantics(
        excluding: widget.label != null,
        child: FocusableActionDetector(
          enabled: enabled,
          autofocus: widget.autofocus,
          focusNode: widget.focusNode,
          mouseCursor: enabled ? SystemMouseCursors.click : MouseCursor.defer,
          onShowFocusHighlight: (v) => setState(() => _focus = v),
          actions: {
            ActivateIntent: CallbackAction<ActivateIntent>(
              onInvoke: (_) {
                widget.onPressed?.call();
                return null;
              },
            ),
          },
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: widget.onPressed,
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                widget.builder(context),
                if (_focus)
                  Positioned(
                    left: -4,
                    top: -4,
                    right: -4,
                    bottom: -4,
                    child: IgnorePointer(
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(
                            widget.radius + 4,
                          ),
                          border: Border.all(width: 2, color: p.primary),
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Primary button: min height 50, radius 10, 17 / 700, `p-primary`; disabled `p-disabled`
/// with readable text. Phone: full width; laptop: content width, min 160, padding 0 24.
class PPrimaryButton extends StatelessWidget {
  const PPrimaryButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.icon,
    this.autofocus = false,
  });

  final String label;
  final VoidCallback? onPressed;
  final CcIconName? icon;
  final bool autofocus;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final enabled = onPressed != null;
    return PTap(
      onPressed: onPressed,
      radius: 10,
      autofocus: autofocus,
      builder: (_) => Container(
        constraints: BoxConstraints(
          minHeight: 50,
          minWidth: s.laptop ? 160 : 0,
        ),
        padding: EdgeInsets.symmetric(horizontal: s.laptop ? 24 : 14),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: enabled ? p.primary : p.disabled,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              CcIcon(icon!, color: enabled ? p.onPrimary : p.onDisabled),
              const SizedBox(width: 8),
            ],
            Flexible(
              child: Text(
                label,
                textAlign: TextAlign.center,
                style: s.type.primaryButton.copyWith(
                  color: enabled ? p.onPrimary : p.onDisabled,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Secondary button: min height 48, radius 10, `p-surface` with a 1 px `p-border`, 16 / 500.
class PSecondaryButton extends StatelessWidget {
  const PSecondaryButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.icon,
    this.fullWidth = false,
  });

  final String label;
  final VoidCallback? onPressed;
  final CcIconName? icon;

  /// Fill the width even on the laptop (the two page buttons of a PDF).
  final bool fullWidth;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final enabled = onPressed != null;
    return PTap(
      onPressed: onPressed,
      radius: 10,
      builder: (_) => Container(
        constraints: BoxConstraints(
          minHeight: 48,
          minWidth: s.laptop && !fullWidth ? 160 : 0,
        ),
        padding: EdgeInsets.symmetric(
          horizontal: s.laptop && !fullWidth ? 24 : 14,
        ),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: p.surface,
          border: Border.all(width: 1, color: p.borderInput),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              CcIcon(icon!, color: enabled ? p.fg : p.fg3),
              const SizedBox(width: 8),
            ],
            Flexible(
              child: Text(
                label,
                textAlign: TextAlign.center,
                style: s.type.secondaryButton.copyWith(
                  color: enabled ? p.fg : p.fg3,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A round icon disc (spec 3.5: 56, icon 28): icon colour on its disc colour.
enum PTone { ok, err, warn, mut, acc }

class PIconDisc extends StatelessWidget {
  const PIconDisc({
    super.key,
    required this.icon,
    required this.tone,
    this.size = 56,
  });
  final CcIconName icon;
  final PTone tone;
  final double size;

  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    final (bg, fg) = switch (tone) {
      PTone.ok => (p.okDisc, p.ok),
      PTone.err => (p.errorDisc, p.error),
      PTone.warn => (p.warnDisc, p.warn),
      PTone.mut => (p.mutDisc, p.onMutDisc),
      PTone.acc => (p.primarySoft, p.primary),
    };
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(color: bg, shape: BoxShape.circle),
      child: CcIcon(icon, color: fg, size: size * .5),
    );
  }
}

/// A card: `p-surface`, 1 px card border, radius 14, padding 14.
class PCard extends StatelessWidget {
  const PCard({super.key, required this.child, this.center = false});
  final Widget child;
  final bool center;

  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: p.surface,
        border: Border.all(width: 1, color: p.borderCard),
        borderRadius: BorderRadius.circular(14),
      ),
      child: child,
    );
  }
}

/// The 6 px progress bar.
class PProgressBar extends StatelessWidget {
  const PProgressBar({super.key, required this.fraction, this.label});
  final double fraction;
  final String? label;

  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    return Semantics(
      label: label,
      value: '${(fraction * 100).round()} %',
      child: ClipRRect(
        borderRadius: BorderRadius.circular(3),
        child: Container(
          height: 6,
          color: p.dividerSoft,
          alignment: Alignment.centerLeft,
          child: FractionallySizedBox(
            widthFactor: fraction.clamp(0, 1),
            child: Container(height: 6, color: p.primary),
          ),
        ),
      ),
    );
  }
}

/// Spinner: 20 px, 3 px ring, one turn per second, linear (spec 3.6).
class PSpinner extends StatefulWidget {
  const PSpinner({super.key});

  @override
  State<PSpinner> createState() => _PSpinnerState();
}

class _PSpinnerState extends State<PSpinner>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 1),
  );

  @override
  void initState() {
    super.initState();
    _c.repeat();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Reduced motion: a slower turn (the web page does the same, 4 s).
    _c.duration = MediaQuery.disableAnimationsOf(context)
        ? const Duration(seconds: 4)
        : const Duration(seconds: 1);
    if (_c.isAnimating) _c.repeat();
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    return ExcludeSemantics(
      child: RotationTransition(
        turns: _c,
        child: Container(
          width: 20,
          height: 20,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            border: Border(
              top: BorderSide(width: 3, color: p.primary),
              left: BorderSide(width: 3, color: p.primarySoft),
              right: BorderSide(width: 3, color: p.primarySoft),
              bottom: BorderSide(width: 3, color: p.primarySoft),
            ),
          ),
        ),
      ),
    );
  }
}

/// Text field: 46 high, radius 8, `p-surface`, 1 px input border; focus 2 px `p-primary`;
/// invalid border `p-error` (spec 3.5).
class PField extends StatefulWidget {
  const PField({
    super.key,
    required this.label,
    required this.controller,
    this.code = false,
    this.error = false,
    this.maxLength,
    this.keyboardType,
    this.hint,
    this.onSubmitted,
    this.onChanged,
    this.textInputAction,
    this.helper,
    this.errorText,
    this.fieldKey,
    this.autofocus = false,
  });

  final String label;
  final TextEditingController controller;
  final bool code;
  final bool error;
  final int? maxLength;
  final TextInputType? keyboardType;
  final String? hint;
  final String? helper;
  final String? errorText;
  final ValueChanged<String>? onSubmitted;
  final ValueChanged<String>? onChanged;
  final TextInputAction? textInputAction;
  final Key? fieldKey;
  final bool autofocus;

  @override
  State<PField> createState() => _PFieldState();
}

class _PFieldState extends State<PField> {
  final FocusNode _node = FocusNode();
  bool _focus = false;

  @override
  void initState() {
    super.initState();
    _node.addListener(() => setState(() => _focus = _node.hasFocus));
  }

  @override
  void dispose() {
    _node.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final style = widget.code ? s.type.codeInput : s.type.input;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(widget.label, style: s.type.label),
        const SizedBox(height: 5),
        Container(
          height: 46,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          alignment: Alignment.centerLeft,
          decoration: BoxDecoration(
            color: p.surface,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(
              width: _focus || widget.error ? 2 : 1,
              color: widget.error
                  ? p.error
                  : _focus
                  ? p.primary
                  : p.borderInput,
            ),
          ),
          child: EditableText(
            key: widget.fieldKey,
            controller: widget.controller,
            focusNode: _node,
            autofocus: widget.autofocus,
            style: style,
            cursorColor: p.primary,
            backgroundCursorColor: p.borderInput,
            maxLines: 1,
            keyboardType: widget.keyboardType ?? TextInputType.text,
            textInputAction: widget.textInputAction,
            textCapitalization: widget.code
                ? TextCapitalization.characters
                : TextCapitalization.none,
            autocorrect: false,
            enableSuggestions: !widget.code,
            inputFormatters: [
              if (widget.maxLength != null)
                LengthLimitingTextInputFormatter(widget.maxLength),
              if (widget.code) _UpperCase(),
            ],
            onChanged: widget.onChanged,
            onSubmitted: widget.onSubmitted,
          ),
        ),
        if (widget.helper != null) ...[
          const SizedBox(height: 5),
          Text(widget.helper!, style: s.type.small),
        ],
        if (widget.errorText != null) ...[
          const SizedBox(height: 5),
          Semantics(
            liveRegion: true,
            child: Text(widget.errorText!, style: s.type.errorText),
          ),
        ],
      ],
    );
  }
}

class _UpperCase extends TextInputFormatter {
  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) => newValue.copyWith(text: newValue.text.toUpperCase());
}

/// An option row of "What to show": min height 58 (laptop 50), radius 12, `p-surface`, 1 px
/// border; icon 22 in `p-primary` (`p-warn` when limited), title 17 / 700, subtitle 13 / 400.
class POption extends StatelessWidget {
  const POption({
    super.key,
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onPressed,
    this.limited = false,
  });
  final CcIconName icon;
  final String title;
  final String subtitle;
  final VoidCallback onPressed;
  final bool limited;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    return PTap(
      onPressed: onPressed,
      radius: 12,
      builder: (_) => Container(
        constraints: BoxConstraints(minHeight: s.laptop ? 50 : 58),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: p.surface,
          border: Border.all(width: 1, color: p.borderInput),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            CcIcon(icon, color: limited ? p.warn : p.primary, size: 22),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: s.type.optionTitle),
                  Text(subtitle, style: s.type.optionSub),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
