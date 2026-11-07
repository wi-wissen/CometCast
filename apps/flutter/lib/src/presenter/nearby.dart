// SPDX-License-Identifier: GPL-3.0-or-later
// The section "screens on this network" of the app start page: rows, states and their texts
// (spec 14 of docs/design/first-run-and-display.md, native-apps.md 4.1). The state lives in
// NearbyController; what a tap does lives in PresenterRoot. Only tokens of spec 3.3 are used.

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../core/core_api.dart';
import '../../nearby_controller.dart';
import '../design/icons.dart';
import 'host_text.dart';
import 'kit.dart';
import '../display/display_logic.dart' show formatCode;

/// The host that `nearby.via` names for [screen], or null when the row says nothing about the
/// server (the default relay, a policy server or a DNS-SRV server, spec 14.3).
/// Row text column minimum (decision N62): below it the code moves under the subtitle.
const double _minTitleColumn = 160;

typedef NearbyVia = String? Function(NearbyScreenInfo screen);

/// A secondary button of the section (spec 14.2, "Compact button"): `p-surface`, 1 px
/// `p-border`, radius 10, min height 44, padding 0 16, 15 / 700, icon 18, content width on every
/// device.
class PCompactButton extends StatelessWidget {
  const PCompactButton({
    super.key,
    required this.label,
    this.icon,
    required this.onPressed,
    this.centered = false,
  });

  final String label;
  final CcIconName? icon;

  /// Null: disabled (`p-disabled`, as the primary button).
  final VoidCallback? onPressed;

  /// Not stretched to the left edge of its parent (a row of two buttons).
  final bool centered;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final enabled = onPressed != null;
    final button = PTap(
      onPressed: onPressed,
      radius: 10,
      builder: (_) => Container(
        constraints: const BoxConstraints(minHeight: 44),
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          color: enabled ? p.surface : p.disabled,
          border: Border.all(
            width: 1,
            color: enabled ? p.borderInput : Colors.transparent,
          ),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              CcIcon(icon!, color: enabled ? p.fg : p.onDisabled, size: 18),
              const SizedBox(width: 8),
            ],
            Flexible(
              child: Text(
                label,
                style: s.type.secondaryButton.copyWith(
                  fontSize: 15,
                  fontWeight: FontWeight.w700,
                  color: enabled ? p.fg : p.onDisabled,
                ),
              ),
            ),
          ],
        ),
      ),
    );
    return centered
        ? button
        : Align(alignment: Alignment.centerLeft, child: button);
  }
}

/// The section. [onTap] is called for a row that may be tapped (the guard of spec 14.3 is
/// applied here); [via] names the server of rows whose server is not a known one.
class NearbySection extends StatelessWidget {
  const NearbySection({
    super.key,
    required this.controller,
    required this.onTap,
    required this.via,
    this.tvs,
  });

  /// The TVs without Comet Cast (spec 15.5), below the Comet Cast states and rows; draws its
  /// own gap and nothing when there are none.
  final Widget? tvs;

  final NearbyController controller;
  final void Function(NearbyRow row) onTap;
  final NearbyVia via;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final status = controller.status;
        if (status == NearbyStatus.hidden) return const SizedBox.shrink();
        final s = PresenterScope.of(context);
        final p = s.tokens;
        final t = AppLocalizations.of(context);
        final body = <Widget>[];
        switch (status) {
          case NearbyStatus.hidden || NearbyStatus.resolving:
            break;
          case NearbyStatus.notAsked:
            body.add(
              PCompactButton(
                key: const Key('nearby-search'),
                label: t.nearbySearch,
                icon: CcIconName.search,
                onPressed: controller.searchNow,
              ),
            );
          case NearbyStatus.searching:
            body.add(
              Semantics(
                liveRegion: true,
                child: _Placeholder(
                  key: const Key('nearby-searching'),
                  child: Row(
                    children: [
                      const PSpinner(),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          t.nearbySearching,
                          style: s.type.body.copyWith(color: p.fg3),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            );
          case NearbyStatus.found:
            for (var i = 0; i < controller.rows.length; i++) {
              if (i > 0) body.add(const SizedBox(height: 8));
              final row = controller.rows[i];
              body.add(
                NearbyRowTile(
                  key: Key('nearby-row-${row.id}'),
                  row: row,
                  connecting: controller.connectingId == row.id,
                  othersBusy: controller.connectingId != null,
                  via: via(row.screen),
                  onPressed: () {
                    if (controller.canTap(row)) onTap(row);
                  },
                ),
              );
            }
          case NearbyStatus.none:
            body.add(
              Semantics(
                liveRegion: true,
                child: Column(
                  key: const Key('nearby-none'),
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      t.nearbyNone,
                      style: s.type.body.copyWith(color: p.fg2),
                    ),
                    const SizedBox(height: 4),
                    Text(t.nearbyNoneWhy, style: s.type.small),
                  ],
                ),
              ),
            );
          case NearbyStatus.paused:
            body
              ..add(
                Text(
                  t.nearbyPaused,
                  key: const Key('nearby-paused'),
                  style: s.type.small,
                ),
              )
              ..add(const SizedBox(height: 8))
              ..add(
                PCompactButton(
                  key: const Key('nearby-search'),
                  label: t.nearbySearch,
                  icon: CcIconName.search,
                  onPressed: controller.searchNow,
                ),
              );
          case NearbyStatus.denied:
            body
              ..add(
                _WarnLine(
                  key: const Key('nearby-denied'),
                  icon: CcIconName.info,
                  text: t.nearbyDenied,
                ),
              )
              ..add(
                PTap(
                  onPressed: () => controller.openSettings(),
                  radius: 4,
                  builder: (_) => Padding(
                    padding: const EdgeInsets.symmetric(vertical: 12),
                    child: Text(
                      t.nearbyAllow,
                      key: const Key('nearby-allow'),
                      style: s.type.body.copyWith(
                        color: p.primary,
                        decoration: TextDecoration.underline,
                      ),
                    ),
                  ),
                ),
              );
          case NearbyStatus.noNetwork:
            body.add(
              _WarnLine(
                key: const Key('nearby-no-wifi'),
                icon: CcIconName.wifiOff,
                text: t.nearbyNoWifi,
              ),
            );
        }
        if (controller.goneLine) {
          body
            ..add(const SizedBox(height: 8))
            ..add(
              Semantics(
                liveRegion: true,
                child: Text(
                  t.nearbyGone,
                  key: const Key('nearby-gone'),
                  style: s.type.errorText,
                ),
              ),
            );
        }
        return Column(
          key: const Key('nearby-section'),
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Semantics(
              header: true,
              child: Text(t.nearbyTitle, style: s.type.label),
            ),
            if (body.isNotEmpty) const SizedBox(height: 8),
            ...body,
            ?tvs,
          ],
        );
      },
    );
  }
}

/// The card of the laptop layout (spec 14.1): `p-surface`, 1 px card edge, radius 14,
/// padding 16.
class NearbyCard extends StatelessWidget {
  const NearbyCard({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: p.surface,
        border: Border.all(width: 1, color: p.borderCard),
        borderRadius: BorderRadius.circular(14),
      ),
      child: child,
    );
  }
}

/// The divider `start.or`: a rule, the word in small `p-fg3`, a rule.
class PDividerOr extends StatelessWidget {
  const PDividerOr({super.key});

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final t = AppLocalizations.of(context);
    Widget rule() => Expanded(child: Container(height: 1, color: p.divider));
    return Row(
      key: const Key('nearby-or'),
      children: [
        rule(),
        const SizedBox(width: 10),
        Text(t.startOr, style: s.type.small),
        const SizedBox(width: 10),
        rule(),
      ],
    );
  }
}

class _WarnLine extends StatelessWidget {
  const _WarnLine({super.key, required this.icon, required this.text});
  final CcIconName icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    return Semantics(
      liveRegion: true,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 1),
            child: CcIcon(icon, color: s.tokens.warn, size: 18),
          ),
          const SizedBox(width: 8),
          Expanded(child: Text(text, style: s.type.small)),
        ],
      ),
    );
  }
}

/// The searching placeholder: option-row size, 1 px dashed `p-border`.
class _Placeholder extends StatelessWidget {
  const _Placeholder({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    return CustomPaint(
      foregroundPainter: _DashedBorder(s.tokens.borderInput, 12),
      child: Container(
        constraints: BoxConstraints(minHeight: s.laptop ? 50 : 58),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        alignment: Alignment.centerLeft,
        child: child,
      ),
    );
  }
}

/// One screen (spec 14.3).
class NearbyRowTile extends StatefulWidget {
  const NearbyRowTile({
    super.key,
    required this.row,
    required this.connecting,
    required this.othersBusy,
    required this.via,
    required this.onPressed,
  });

  final NearbyRow row;

  /// This row is being connected: spinner instead of the code, `nearby.connecting` as subtitle.
  final bool connecting;

  /// Another row is being connected: this one is disabled (55 % opacity).
  final bool othersBusy;
  final String? via;
  final VoidCallback onPressed;

  @override
  State<NearbyRowTile> createState() => _NearbyRowTileState();
}

class _NearbyRowTileState extends State<NearbyRowTile> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final t = AppLocalizations.of(context);
    final row = widget.row;
    final screen = row.screen;
    final locked = row.locked;
    final empty = screen.name.isEmpty;
    final title = empty ? t.commonNoName : t.commonQuoted(screen.name);
    final mode = switch (screen.mode) {
      NearbyMode.open => t.nearbyOpen,
      NearbyMode.approval => t.nearbyApproval,
      NearbyMode.locked => t.nearbyLocked,
    };
    final subtitle = widget.connecting
        ? t.nearbyConnecting
        : (!locked && widget.via != null)
        ? '$mode · ${t.nearbyVia(widget.via!)}'
        : mode;
    final code = screen.code;
    final label = locked || code == null
        ? '$title, $mode'
        : t.nearbyAria(title, formatCode(code), mode);
    final titleStyle = empty
        ? s.type.optionTitle.copyWith(
            fontWeight: FontWeight.w500,
            color: p.fg2,
            height: 1.25,
          )
        : s.type.optionTitle.copyWith(
            height: 1.25,
            color: locked ? p.fg3 : p.fg,
          );
    final codeText = !locked && code != null ? formatCode(code) : null;
    final codeStyle = s.type.smallDigits.copyWith(
      fontSize: 15,
      color: p.fg2,
      letterSpacing: .06 * 15,
    );
    // The slot is as wide and high as the code in every state (N64); the spinner (20) sits in it.
    final codePainter = codeText == null
        ? null
        : (TextPainter(
            text: TextSpan(text: codeText, style: codeStyle),
            textDirection: Directionality.of(context),
            textScaler: MediaQuery.textScalerOf(context),
          )..layout());
    final slotWidth = codePainter?.width ?? 0.0;
    final slotHeight = math.max(codePainter?.height ?? 0.0, 20.0);
    final content = Container(
      constraints: BoxConstraints(minHeight: s.laptop ? 50 : 58),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
      decoration: BoxDecoration(
        // In the laptop card the row fill is `p-bg` (the card is `p-surface`).
        color: s.laptop ? p.bg : p.surface,
        border: locked
            ? null
            : Border.all(
                width: 1,
                color: _hover && !widget.othersBusy ? p.primary : p.borderInput,
              ),
        borderRadius: BorderRadius.circular(12),
      ),
      child: _RowLayout(
        slotWidth: slotWidth,
        children: [
          CcIcon(
            locked ? CcIconName.locked : CcIconName.screen,
            color: locked ? p.fg3 : p.primary,
            size: 22,
          ),
          Text(
            title,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: titleStyle,
          ),
          (!widget.connecting && !locked && widget.via != null)
              ? HostText(subtitle, host: widget.via!, style: s.type.optionSub)
              : Text(subtitle, style: s.type.optionSub),
          if (codeText != null)
            SizedBox(
              height: slotHeight,
              child: Align(
                alignment: Alignment.centerLeft,
                widthFactor: 1,
                child: widget.connecting
                    ? const PSpinner()
                    : Text(
                        codeText,
                        key: Key('nearby-code-${row.id}'),
                        style: codeStyle,
                      ),
              ),
            ),
        ],
      ),
    );
    if (locked) {
      // Stays in the list so the person knows the screen is there: dashed, not focusable, not
      // tappable.
      return Semantics(
        label: label,
        enabled: false,
        container: true,
        child: ExcludeSemantics(
          child: CustomPaint(
            foregroundPainter: _DashedBorder(p.borderInput, 12),
            child: content,
          ),
        ),
      );
    }
    final enabled = !widget.othersBusy && !widget.connecting;
    return Opacity(
      opacity: widget.othersBusy && !widget.connecting ? .55 : 1,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hover = true),
        onExit: (_) => setState(() => _hover = false),
        child: PTap(
          // While a connection starts all rows are disabled (the spinner row included).
          onPressed: enabled ? widget.onPressed : null,
          radius: 12,
          label: label,
          builder: (_) => content,
        ),
      ),
    );
  }
}

/// A 1 px dashed rounded border.
class _DashedBorder extends CustomPainter {
  _DashedBorder(this.color, this.radius);
  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    final path = Path()
      ..addRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(.5, .5, size.width - 1, size.height - 1),
          Radius.circular(radius),
        ),
      );
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1
      ..color = color;
    for (final m in path.computeMetrics()) {
      var d = 0.0;
      while (d < m.length) {
        canvas.drawPath(m.extractPath(d, d + 4), paint);
        d += 7;
      }
    }
  }

  @override
  bool shouldRepaint(_DashedBorder old) =>
      old.color != color || old.radius != radius;
}

/// The inside of a row (decisions N62, N64): icon, title and subtitle, and the code in a slot on
/// the right. When the text column (width minus icon, gaps and slot) would be under
/// [_minTitleColumn] the code moves under the subtitle, left-aligned with the title. The
/// children are, in order: icon, title, subtitle and optionally the code (or the spinner, which
/// then sits at the slot's right edge). Written as a render object because the page sizes itself
/// with `IntrinsicHeight`, which a `LayoutBuilder` cannot answer.
class _RowLayout extends MultiChildRenderObjectWidget {
  const _RowLayout({required this.slotWidth, required super.children});

  /// Width of the code (0: no code or spinner).
  final double slotWidth;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderRowLayout(slotWidth);

  @override
  void updateRenderObject(BuildContext context, _RenderRowLayout r) {
    r.slotWidth = slotWidth;
  }
}

class _RowParentData extends ContainerBoxParentData<RenderBox> {}

class _RenderRowLayout extends RenderBox
    with
        ContainerRenderObjectMixin<RenderBox, _RowParentData>,
        RenderBoxContainerDefaultsMixin<RenderBox, _RowParentData> {
  _RenderRowLayout(this._slotWidth);

  static const double _icon = 22;
  static const double _gap = 12;

  double _slotWidth;
  set slotWidth(double v) {
    if (v == _slotWidth) return;
    _slotWidth = v;
    markNeedsLayout();
  }

  @override
  void setupParentData(RenderBox child) {
    if (child.parentData is! _RowParentData) {
      child.parentData = _RowParentData();
    }
  }

  List<RenderBox> get _kids {
    final out = <RenderBox>[];
    for (var c = firstChild; c != null; c = childAfter(c)) {
      out.add(c);
    }
    return out;
  }

  bool _below(double width, List<RenderBox> k) =>
      k.length > 3 &&
      width - _icon - _gap - _gap - _slotWidth < _minTitleColumn;

  /// Sizes of the title, subtitle and code and the total height for [width].
  ({double textW, double h, bool below, Size title, Size sub, Size code}) _plan(
    double width,
    Size Function(RenderBox, BoxConstraints) measure,
  ) {
    final k = _kids;
    final below = _below(width, k);
    final hasCode = k.length > 3;
    final textW = math.max(
      0.0,
      width - _icon - _gap - (hasCode && !below ? _gap + _slotWidth : 0),
    );
    final loose = BoxConstraints(maxWidth: textW);
    final title = measure(k[1], loose);
    final sub = measure(k[2], loose);
    final code = hasCode ? measure(k[3], const BoxConstraints()) : Size.zero;
    final col = title.height + sub.height + (below ? code.height : 0);
    final h = math.max<double>(
      math.max<double>(_icon, col),
      hasCode && !below ? code.height : 0,
    );
    return (
      textW: textW,
      h: h,
      below: below,
      title: title,
      sub: sub,
      code: code,
    );
  }

  @override
  Size computeDryLayout(BoxConstraints constraints) {
    final w = constraints.maxWidth;
    final p = _plan(w, (c, bc) => c.getDryLayout(bc));
    return constraints.constrain(Size(w, p.h));
  }

  @override
  void performLayout() {
    final w = constraints.maxWidth;
    final k = _kids;
    k[0].layout(
      BoxConstraints.tight(const Size(_icon, _icon)),
      parentUsesSize: true,
    );
    final p = _plan(w, (c, bc) {
      c.layout(bc, parentUsesSize: true);
      return c.size;
    });
    size = constraints.constrain(Size(w, p.h));
    final h = size.height;
    double centre(double ch) => (h - ch) / 2;
    (k[0].parentData! as _RowParentData).offset = Offset(0, centre(_icon));
    const textX = _icon + _gap;
    final col = p.title.height + p.sub.height + (p.below ? p.code.height : 0);
    final top = centre(col);
    (k[1].parentData! as _RowParentData).offset = Offset(textX, top);
    (k[2].parentData! as _RowParentData).offset = Offset(
      textX,
      top + p.title.height,
    );
    if (k.length > 3) {
      (k[3].parentData! as _RowParentData).offset = p.below
          ? Offset(textX, top + p.title.height + p.sub.height)
          : Offset(w - p.code.width, centre(p.code.height));
    }
  }

  double _heightFor(double width) => _plan(
    width,
    (c, bc) => Size(bc.maxWidth, c.getMinIntrinsicHeight(bc.maxWidth)),
  ).h;

  @override
  double computeMinIntrinsicHeight(double width) => _heightFor(width);

  @override
  double computeMaxIntrinsicHeight(double width) => _heightFor(width);

  @override
  double computeMinIntrinsicWidth(double height) {
    final k = _kids;
    return _icon +
        _gap +
        math.max(
          k[1].getMinIntrinsicWidth(double.infinity),
          k[2].getMinIntrinsicWidth(double.infinity),
        );
  }

  @override
  double computeMaxIntrinsicWidth(double height) {
    final k = _kids;
    return _icon +
        _gap +
        math.max(
          k[1].getMaxIntrinsicWidth(double.infinity),
          k[2].getMaxIntrinsicWidth(double.infinity),
        ) +
        (k.length > 3 ? _gap + _slotWidth : 0);
  }

  @override
  void paint(PaintingContext context, Offset offset) =>
      defaultPaint(context, offset);

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) =>
      defaultHitTestChildren(result, position: position);
}
