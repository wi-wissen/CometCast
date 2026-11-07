// SPDX-License-Identifier: GPL-3.0-or-later
// The building blocks of the display screens: the 960 x 540 canvas, the focus ring and the
// controls (docs/design/first-run-and-display.md 3.1, 3.5, 7.1, 7.2; native-apps.md 6, 10).
// Every number is in reference px of the canvas.

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../design/icons.dart';
import '../design/tokens.dart';

/// Reference canvas of the display (spec 3.1).
const canvasWidth = 960.0;
const canvasHeight = 540.0;

/// What every display widget reads: tokens, type and the focus ring rule.
class DisplayScope extends InheritedWidget {
  const DisplayScope({
    super.key,
    required this.tokens,
    required this.ringVisible,
    required this.reduceMotion,
    required super.child,
    this.keyGate,
  });

  final DisplayTokens tokens;

  /// Called first for every key press that reaches a control. Shows the focus ring and returns
  /// true when the key must only restore the brightness after idle dimming (spec 8.6).
  final bool Function(KeyEvent event)? keyGate;

  /// The ring rule of spec 7.2: shown on start, hidden by a pointer press, touch or real mouse
  /// movement, shown again by a key. Always true on a TV.
  final ValueListenable<bool> ringVisible;

  /// System "remove animations": fades are instant, the ring does not scale (spec 3.6).
  final bool reduceMotion;

  DisplayType get type => DisplayType(tokens);

  static DisplayScope of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DisplayScope>()!;

  static DisplayScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DisplayScope>();

  @override
  bool updateShouldNotify(DisplayScope old) =>
      tokens != old.tokens ||
      ringVisible != old.ringVisible ||
      keyGate != old.keyGate ||
      reduceMotion != old.reduceMotion;
}

/// Lays [child] out on the 960 x 540 canvas, scaled uniformly by `min(w/960, h/540)` and
/// centred; the rest of the viewport is `--bg` (spec 3.1). Text inside never scales with the
/// system text size (the canvas scales as a whole).
class DisplayCanvas extends StatelessWidget {
  const DisplayCanvas({super.key, required this.child, this.background = true});

  final Widget child;

  /// Paint `--bg` behind (false for layers over the picture).
  final bool background;

  @override
  Widget build(BuildContext context) {
    final scope = DisplayScope.of(context);
    final canvas = FittedBox(
      fit: BoxFit.contain,
      child: SizedBox(
        width: canvasWidth,
        height: canvasHeight,
        child: ClipRect(
          child: MediaQuery(
            data: MediaQuery.of(
              context,
            ).copyWith(textScaler: TextScaler.noScaling),
            child: DefaultTextStyle(
              style: scope.type.body.copyWith(color: scope.tokens.fg),
              child: child,
            ),
          ),
        ),
      ),
    );
    if (!background) return canvas;
    return ColoredBox(color: scope.tokens.bg, child: canvas);
  }
}

/// A focus node that knows its place in the rows of a screen (spec 7.1: left/right move within
/// a row, up/down to the same column or the last item of the next row, no wrapping).
class RowNode extends FocusNode {
  RowNode({required this.row, required this.col, super.debugLabel, super.onKeyEvent});

  final int row;
  final int col;
}

/// Traversal by rows. Only [RowNode]s take part; everything else falls back to the order of
/// the widget tree.
class RowsPolicy extends WidgetOrderTraversalPolicy {
  RowsPolicy();

  @override
  bool inDirection(FocusNode currentNode, TraversalDirection direction) {
    final scope = currentNode.nearestScope;
    if (currentNode is! RowNode || scope == null) {
      return super.inDirection(currentNode, direction);
    }
    final nodes = scope.traversalDescendants
        .whereType<RowNode>()
        .where((n) => n.canRequestFocus)
        .toList();
    final rows = <int, List<RowNode>>{};
    for (final n in nodes) {
      rows.putIfAbsent(n.row, () => []).add(n);
    }
    for (final r in rows.values) {
      r.sort((a, b) => a.col.compareTo(b.col));
    }
    final rowKeys = rows.keys.toList()..sort();
    final row = rows[currentNode.row];
    if (row == null) return false;
    final i = row.indexOf(currentNode);
    RowNode? target;
    switch (direction) {
      case TraversalDirection.left:
        if (i > 0) target = row[i - 1];
      case TraversalDirection.right:
        if (i >= 0 && i < row.length - 1) target = row[i + 1];
      case TraversalDirection.up || TraversalDirection.down:
        final at = rowKeys.indexOf(currentNode.row);
        final next = direction == TraversalDirection.up ? at - 1 : at + 1;
        if (next >= 0 && next < rowKeys.length) {
          final r = rows[rowKeys[next]]!;
          target = r[i.clamp(0, r.length - 1)];
        }
    }
    if (target == null) return false;
    target.requestFocus();
    return true;
  }
}

/// The keys that activate a focused control: OK / Select (DPAD_CENTER), Enter, Space.
bool isActivateKey(LogicalKeyboardKey k) =>
    k == LogicalKeyboardKey.enter ||
    k == LogicalKeyboardKey.numpadEnter ||
    k == LogicalKeyboardKey.select ||
    k == LogicalKeyboardKey.space;

/// One focusable control of the display: ring (outline 4 `--focus`, offset 3, glow 9 at 18 %,
/// scale 1.03 in 120 ms; text fields without scale), tap and key activation, semantics.
class DisplayControl extends StatefulWidget {
  const DisplayControl({
    super.key,
    required this.row,
    required this.col,
    required this.radius,
    required this.child,
    required this.onPressed,
    this.autofocus = false,
    this.focusNode,
    this.label,
    this.selected,
    this.scale = true,
    this.externalFocus = false,
  });

  final int row;
  final int col;
  final double radius;
  final Widget child;

  /// Null = inert (not focusable).
  final VoidCallback? onPressed;
  final bool autofocus;

  /// Optional node (a [RowNode] if the screen wants to move focus by hand).
  final FocusNode? focusNode;

  /// Accessible name; defaults to the text inside.
  final String? label;

  /// Radio semantics: `null` = a plain button.
  final bool? selected;
  final bool scale;

  /// The child owns [focusNode] (a text field): this control only draws the ring.
  final bool externalFocus;

  @override
  State<DisplayControl> createState() => _DisplayControlState();
}

class _DisplayControlState extends State<DisplayControl> {
  late final RowNode _own = RowNode(
    row: widget.row,
    col: widget.col,
    debugLabel: 'display:${widget.row}:${widget.col}',
  );
  FocusNode get _node => widget.focusNode ?? _own;
  bool _focused = false;

  @override
  void initState() {
    super.initState();
    // The initial focus of a screen or dialog is taken explicitly: Flutter's own autofocus
    // does nothing while another control (the one under the dialog) still has the focus.
    if (widget.autofocus) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && widget.onPressed != null) _node.requestFocus();
      });
    }
  }

  @override
  void dispose() {
    _own.dispose();
    super.dispose();
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent e) {
    if (e is KeyDownEvent &&
        (DisplayScope.maybeOf(context)?.keyGate?.call(e) ?? false)) {
      return KeyEventResult.handled;
    }
    if (widget.onPressed == null) return KeyEventResult.ignored;
    if (isActivateKey(e.logicalKey)) {
      if (e is KeyDownEvent) widget.onPressed!();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final scope = DisplayScope.of(context);
    final t = scope.tokens;
    final enabled = widget.onPressed != null;
    return Semantics(
      container: true,
      button: widget.selected == null,
      inMutuallyExclusiveGroup: widget.selected == null ? null : true,
      checked: widget.selected,
      enabled: enabled,
      label: widget.label,
      onTap: widget.onPressed,
      child: ExcludeSemantics(
        excluding: widget.label != null,
        child: _focusHost(
          GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: widget.onPressed,
            child: ValueListenableBuilder<bool>(
              valueListenable: scope.ringVisible,
              builder: (context, visible, child) {
                final ring = _focused && visible;
                final grow = ring && widget.scale && !scope.reduceMotion;
                return AnimatedScale(
                  scale: grow ? 1.03 : 1.0,
                  duration: scope.reduceMotion
                      ? Duration.zero
                      : const Duration(milliseconds: 120),
                  curve: Curves.ease,
                  child: Stack(
                    clipBehavior: Clip.none,
                    children: [
                      child!,
                      if (ring) ...[
                        // Glow: 9 px at 18 %, a ring from -9 to 0.
                        Positioned(
                          left: -9,
                          top: -9,
                          right: -9,
                          bottom: -9,
                          child: IgnorePointer(
                            child: DecoratedBox(
                              decoration: BoxDecoration(
                                borderRadius: BorderRadius.circular(
                                  widget.radius + 9,
                                ),
                                border: Border.all(
                                  width: 9,
                                  color: DisplayTokens.focusGlow,
                                ),
                              ),
                            ),
                          ),
                        ),
                        // Outline: 4 px `--focus`, offset 3: a ring from -7 to -3.
                        Positioned(
                          left: -7,
                          top: -7,
                          right: -7,
                          bottom: -7,
                          child: IgnorePointer(
                            child: DecoratedBox(
                              decoration: BoxDecoration(
                                borderRadius: BorderRadius.circular(
                                  widget.radius + 7,
                                ),
                                border: Border.all(width: 4, color: t.focus),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ],
                  ),
                );
              },
              child: widget.child,
            ),
          ),
        ),
      ),
    );
  }

  Widget _focusHost(Widget child) {
    if (widget.externalFocus) {
      return ListenableBuilder(
        listenable: _node,
        builder: (context, _) {
          _focused = _node.hasFocus;
          return child;
        },
      );
    }
    return Focus(
      focusNode: _node,
      autofocus: widget.autofocus,
      canRequestFocus: widget.onPressed != null,
      onKeyEvent: _onKey,
      onFocusChange: (v) => setState(() => _focused = v),
      child: child,
    );
  }
}

enum DButtonKind { primary, secondary }

/// Wizard and dialog button: min height 50, padding 0 22, radius 12, border 2, 19 / 700.
class DButton extends StatelessWidget {
  const DButton({
    super.key,
    required this.label,
    required this.onPressed,
    required this.row,
    required this.col,
    this.kind = DButtonKind.secondary,
    this.autofocus = false,
    this.focusNode,
  });

  final String label;
  final VoidCallback? onPressed;
  final int row;
  final int col;
  final DButtonKind kind;
  final bool autofocus;
  final FocusNode? focusNode;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    final primary = kind == DButtonKind.primary;
    return DisplayControl(
      row: row,
      col: col,
      radius: 12,
      autofocus: autofocus,
      focusNode: focusNode,
      onPressed: onPressed,
      child: Container(
        constraints: const BoxConstraints(minHeight: 50),
        padding: const EdgeInsets.symmetric(horizontal: 22),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: primary ? t.acc : t.btn,
          border: Border.all(width: 2, color: primary ? t.acc : t.line),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          label,
          style: s.type.button.copyWith(color: primary ? t.bg : t.fg),
          textAlign: TextAlign.center,
        ),
      ),
    );
  }
}

/// "No and block": ghost, 34 high, radius 10, border 2, 14 px (spec 3.5).
class DGhostButton extends StatelessWidget {
  const DGhostButton({
    super.key,
    required this.label,
    required this.onPressed,
    required this.row,
    required this.col,
  });
  final String label;
  final VoidCallback? onPressed;
  final int row;
  final int col;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    return DisplayControl(
      row: row,
      col: col,
      radius: 10,
      onPressed: onPressed,
      child: Container(
        height: 34,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        alignment: Alignment.center,
        decoration: BoxDecoration(
          border: Border.all(width: 2, color: s.tokens.line),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Text(label, style: s.type.ghost),
      ),
    );
  }
}

/// Close button of Settings: x icon 16 + text 14 `--mut`, no border, min height 34.
class DCloseButton extends StatelessWidget {
  const DCloseButton({
    super.key,
    required this.label,
    required this.onPressed,
    required this.row,
    required this.col,
  });
  final String label;
  final VoidCallback onPressed;
  final int row;
  final int col;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    return DisplayControl(
      row: row,
      col: col,
      radius: 8,
      onPressed: onPressed,
      child: Container(
        constraints: const BoxConstraints(minHeight: 34),
        padding: const EdgeInsets.symmetric(horizontal: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            CcIcon(CcIconName.close, color: s.tokens.mut, size: 16),
            const SizedBox(width: 6),
            Text(label, style: s.type.close),
          ],
        ),
      ),
    );
  }
}

/// Name-tag look: fill `panel` at 92 %, border 2 `--line`, radius 10, min height 46.
BoxDecoration overlayDecoration(DisplayTokens t) => BoxDecoration(
  color: t.overlayFill,
  border: Border.all(width: 2, color: t.line),
  borderRadius: BorderRadius.circular(10),
);

/// The dialog card (spec 3.5): width 600 (wizard 780), padding 28 x 34, radius 22, border 1
/// `--line`, gap 16. [shadow] for dialogs over a picture or a scrim.
class DCard extends StatelessWidget {
  const DCard({
    super.key,
    required this.width,
    required this.children,
    this.shadow = false,
    this.crossAxisAlignment = CrossAxisAlignment.stretch,
  });

  final double width;
  final List<Widget> children;
  final bool shadow;
  final CrossAxisAlignment crossAxisAlignment;

  @override
  Widget build(BuildContext context) {
    final t = DisplayScope.of(context).tokens;
    final spaced = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      if (i > 0) spaced.add(const SizedBox(height: 16));
      spaced.add(children[i]);
    }
    return Container(
      width: width,
      padding: const EdgeInsets.symmetric(horizontal: 34, vertical: 28),
      decoration: BoxDecoration(
        color: t.panel,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(width: 1, color: t.line),
        boxShadow: shadow
            ? const [
                BoxShadow(
                  color: Color(0x8C000000),
                  blurRadius: 60,
                  offset: Offset(0, 24),
                ),
              ]
            : null,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: crossAxisAlignment,
        children: spaced,
      ),
    );
  }
}

/// Section label in dialogs: 14 / 700 uppercase, .08em, `--mut`.
class DSectionLabel extends StatelessWidget {
  const DSectionLabel(this.text, {super.key});
  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text.toUpperCase(),
    style: DisplayScope.of(context).type.sectionLabel,
  );
}

/// Scrim over the canvas behind a dialog (spec 3.2).
class DScrim extends StatelessWidget {
  const DScrim({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) => Container(
    color: DisplayTokens.scrim,
    alignment: Alignment.center,
    child: child,
  );
}

/// The logo (spec 3.2): dark file on Dark and OLED black, light file on Light.
class DLogo extends StatelessWidget {
  const DLogo({super.key, required this.size});
  final double size;

  @override
  Widget build(BuildContext context) {
    final t = DisplayScope.of(context).tokens;
    return ExcludeSemantics(
      child: SvgPicture.asset(t.logoAsset, width: size, height: size),
    );
  }
}
