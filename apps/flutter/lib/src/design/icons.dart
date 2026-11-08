// SPDX-License-Identifier: GPL-3.0-or-later
// The stroke icons of the design: the path data of apps/web/src/components/Icon.vue, 24 x 24
// view box, stroke 2, round caps and joins (check 2.6, stop and x 2.4). No Material or Cupertino
// icons (docs/design/native-apps.md 2 and 13).

import 'package:flutter/widgets.dart';
import 'package:path_drawing/path_drawing.dart';

enum CcIconName {
  open,
  approval,
  locked,
  gear,
  close,
  stop,
  wifiOff,
  globe,
  download,
  info,
  check,
  x,
  clock,
  screen,
  file,
  link,
  scan,
  search,
  pause,
  play,
}

sealed class _Shape {
  const _Shape();
}

class _P extends _Shape {
  const _P(this.d);
  final String d;
}

class _C extends _Shape {
  const _C(this.cx, this.cy, this.r);
  final double cx, cy, r;
}

class _R extends _Shape {
  const _R(this.x, this.y, this.w, this.h, this.rx);
  final double x, y, w, h, rx;
}

const Map<CcIconName, List<_Shape>> _shapes = {
  CcIconName.open: [_R(4, 11, 16, 10, 2), _P('M8 11V7a4 4 0 0 1 7.5-2')],
  CcIconName.approval: [
    _C(9, 8, 4),
    _P('M2 21c0-4 3-6 7-6s7 2 7 6'),
    _P('M16 11l2 2 4-4'),
  ],
  CcIconName.locked: [_R(4, 11, 16, 10, 2), _P('M8 11V7a4 4 0 0 1 8 0v4')],
  CcIconName.gear: [
    _P(
      'M10.3 2.6h3.4l.5 2.6a7.5 7.5 0 0 1 1.9 1.1l2.5-.9 1.7 2.9-2 1.7a7.6 7.6 0 0 1 0 2.2l2 1.7-1.7 2.9-2.5-.9a7.5 7.5 0 0 1-1.9 1.1l-.5 2.6h-3.4l-.5-2.6a7.5 7.5 0 0 1-1.9-1.1l-2.5.9-1.7-2.9 2-1.7a7.6 7.6 0 0 1 0-2.2l-2-1.7 1.7-2.9 2.5.9a7.5 7.5 0 0 1 1.9-1.1z',
    ),
    _C(12, 12, 3),
  ],
  CcIconName.close: [_P('M6 6l12 12M18 6L6 18')],
  CcIconName.stop: [_R(5, 5, 14, 14, 2)],
  // Pause and resume (docs/design/pause.md): the same two bars as the web toolbar, and a triangle.
  CcIconName.pause: [_P('M9 5v14M15 5v14')],
  CcIconName.play: [_P('M8 5l11 7-11 7z')],
  CcIconName.wifiOff: [
    _P('M2 2l20 20'),
    _P('M8.5 16.5a5 5 0 0 1 7 0'),
    _P('M5 12.9a10 10 0 0 1 5.2-2.8'),
    _P('M19 12.9a10 10 0 0 0-2.3-1.6'),
    _P('M12 20h.01'),
  ],
  CcIconName.globe: [
    _C(12, 12, 9),
    _P(
      'M3 12h18M12 3c2.5 2.7 3.7 5.7 3.7 9s-1.2 6.3-3.7 9c-2.5-2.7-3.7-5.7-3.7-9S9.5 5.7 12 3z',
    ),
  ],
  CcIconName.download: [_P('M12 3v12M7 10l5 5 5-5M5 21h14')],
  CcIconName.info: [_C(12, 12, 9), _P('M12 11v6M12 7.5h.01')],
  CcIconName.check: [_P('M5 12l5 5 9-10')],
  CcIconName.x: [_P('M6 6l12 12M18 6L6 18')],
  CcIconName.clock: [_C(12, 12, 9), _P('M12 7v5l3 2')],
  CcIconName.screen: [_R(3, 4, 18, 12, 2), _P('M8 20h8M12 16v4')],
  CcIconName.file: [
    _P('M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z'),
    _P('M14 3v6h6'),
  ],
  CcIconName.link: [
    _P('M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1'),
    _P('M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1'),
  ],
  CcIconName.scan: [
    _P(
      'M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3',
    ),
  ],
  CcIconName.search: [_C(11, 11, 7), _P('M20 20l-4-4')],
};

/// Stroke weights: 2 by default; the tick is heavier.
double iconStrokeWidth(CcIconName n) => switch (n) {
  CcIconName.check => 2.6,
  CcIconName.stop || CcIconName.x => 2.4,
  _ => 2.0,
};

/// A stroke icon of the design in [color] at [size] logical px (the 24 x 24 view box is scaled).
class CcIcon extends StatelessWidget {
  const CcIcon(this.name, {super.key, required this.color, this.size = 20});

  final CcIconName name;
  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) {
    return ExcludeSemantics(
      child: SizedBox(
        width: size,
        height: size,
        child: CustomPaint(painter: _IconPainter(name, color)),
      ),
    );
  }
}

class _IconPainter extends CustomPainter {
  _IconPainter(this.name, this.color);
  final CcIconName name;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final s = size.width / 24;
    canvas.save();
    canvas.scale(s);
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..color = color
      ..strokeWidth = iconStrokeWidth(name)
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    for (final shape in _shapes[name]!) {
      switch (shape) {
        case _P(:final d):
          canvas.drawPath(parseSvgPathData(d), stroke);
        case _C(:final cx, :final cy, :final r):
          canvas.drawCircle(Offset(cx, cy), r, stroke);
        case _R(:final x, :final y, :final w, :final h, :final rx):
          canvas.drawRRect(
            RRect.fromRectXY(Rect.fromLTWH(x, y, w, h), rx, rx),
            stroke,
          );
      }
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(_IconPainter old) =>
      old.name != name || old.color != color;
}
