// SPDX-License-Identifier: GPL-3.0-or-later
// Running text with a host name inside (`nearby.via`, the relay page body), decision N62: text
// breaks only at spaces and after a hyphen; a host that alone is wider than the line may also
// break after each dot. The app adds a zero-width break opportunity (U+200B) after every dot of
// the host only when that is needed, so a host that fits on a line moves down whole.

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';

/// [host] is inside [data]. A [Text] for finders and semantics; it draws itself.
class HostText extends Text {
  const HostText(
    // ignore: type_init_formals
    String super.data, {
    super.key,
    required this.host,
    super.style,
  });

  final String host;

  /// [host] with U+200B after every dot.
  static String breakAfterDots(String host) => host.replaceAll('.', '.​');

  /// The run around [host] that has no ordinary space (the no-break space of N53 keeps "via"
  /// with the host): the least that must fit on a line.
  static String unitOf(String text, String host) {
    final i = text.indexOf(host);
    if (i < 0) return text;
    final start = text.lastIndexOf(' ', i) + 1;
    final next = text.indexOf(' ', i + host.length);
    return text.substring(start, next < 0 ? text.length : next);
  }

  @override
  Widget build(BuildContext context) {
    final text = data!;
    var effective = DefaultTextStyle.of(context).style;
    if (style == null || style!.inherit) {
      effective = effective.merge(style);
    } else {
      effective = style!;
    }
    if (MediaQuery.boldTextOf(context)) {
      effective = effective.merge(const TextStyle(fontWeight: FontWeight.bold));
    }
    return _HostParagraph(
      text: text,
      dotted: text.replaceFirst(host, breakAfterDots(host)),
      unit: unitOf(text, host),
      style: effective,
      scaler: MediaQuery.textScalerOf(context),
      direction: Directionality.of(context),
    );
  }
}

class _HostParagraph extends LeafRenderObjectWidget {
  const _HostParagraph({
    required this.text,
    required this.dotted,
    required this.unit,
    required this.style,
    required this.scaler,
    required this.direction,
  });

  final String text;
  final String dotted;
  final String unit;
  final TextStyle style;
  final TextScaler scaler;
  final TextDirection direction;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      RenderHostText(text, dotted, unit, style, scaler, direction);

  @override
  void updateRenderObject(BuildContext context, RenderHostText r) {
    r.update(text, dotted, unit, style, scaler, direction);
  }
}

class RenderHostText extends RenderBox {
  RenderHostText(
    String text,
    String dotted,
    String unit,
    TextStyle style,
    TextScaler scaler,
    TextDirection direction,
  ) : _text = text,
      _dotted = dotted,
      _unit = unit,
      _style = style,
      _scaler = scaler,
      _direction = direction;

  String _text;
  String _dotted;
  String _unit;
  TextStyle _style;
  TextScaler _scaler;
  TextDirection _direction;
  bool _usesDotted = false;

  void update(
    String text,
    String dotted,
    String unit,
    TextStyle style,
    TextScaler scaler,
    TextDirection direction,
  ) {
    if (text == _text &&
        dotted == _dotted &&
        unit == _unit &&
        style == _style &&
        scaler == _scaler &&
        direction == _direction) {
      return;
    }
    _text = text;
    _dotted = dotted;
    _unit = unit;
    _style = style;
    _scaler = scaler;
    _direction = direction;
    markNeedsLayout();
    markNeedsSemanticsUpdate();
  }

  /// True when the dotted variant is drawn at the last layout (for tests).
  bool get usesDotted => _usesDotted;

  /// The laid-out text, for tests.
  TextPainter? get painter => _drawn;

  TextPainter _painter(String text) => TextPainter(
    text: TextSpan(text: text, style: _style),
    textDirection: _direction,
    textScaler: _scaler,
  );

  bool _needsDots(double maxWidth) {
    if (!maxWidth.isFinite) return false;
    final u = _painter(_unit)..layout();
    final w = u.width;
    u.dispose();
    return w > maxWidth;
  }

  TextPainter _laid(BoxConstraints c) {
    final dots = _needsDots(c.maxWidth);
    final p = _painter(dots ? _dotted : _text)
      ..layout(minWidth: c.minWidth, maxWidth: c.maxWidth);
    return p;
  }

  TextPainter? _drawn;

  @override
  Size computeDryLayout(BoxConstraints constraints) {
    final p = _laid(constraints);
    final size = constraints.constrain(Size(p.width, p.height));
    p.dispose();
    return size;
  }

  @override
  void performLayout() {
    _drawn?.dispose();
    _usesDotted = _needsDots(constraints.maxWidth);
    _drawn = _painter(_usesDotted ? _dotted : _text)
      ..layout(minWidth: constraints.minWidth, maxWidth: constraints.maxWidth);
    size = constraints.constrain(Size(_drawn!.width, _drawn!.height));
  }

  double _heightAt(double width) {
    final p = _laid(BoxConstraints(maxWidth: width));
    final h = p.height;
    p.dispose();
    return h;
  }

  @override
  double computeMinIntrinsicHeight(double width) => _heightAt(width);

  @override
  double computeMaxIntrinsicHeight(double width) => _heightAt(width);

  @override
  double computeMinIntrinsicWidth(double height) {
    final p = _painter(_dotted)..layout();
    final w = p.minIntrinsicWidth;
    p.dispose();
    return w;
  }

  @override
  double computeMaxIntrinsicWidth(double height) {
    final p = _painter(_text)..layout();
    final w = p.maxIntrinsicWidth;
    p.dispose();
    return math.max(w, 0);
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    _drawn?.paint(context.canvas, offset);
  }

  @override
  void describeSemanticsConfiguration(SemanticsConfiguration config) {
    super.describeSemanticsConfiguration(config);
    config
      ..label = _text
      ..textDirection = _direction;
  }

  @override
  void dispose() {
    _drawn?.dispose();
    _drawn = null;
    super.dispose();
  }
}
