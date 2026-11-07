// SPDX-License-Identifier: GPL-3.0-or-later
// The toolbar over the received screen: pen, highlighter, eraser, laser pointer, undo, clear and
// freeze, the same set and the same behaviour as the web receiver (apps/web, docs/web-m2.md).
//
// Annotations are LOCAL: they are drawn over the picture on this device and are never sent to
// the sender or anyone else (there is no annotation message in the control channel, see
// docs/flutter-bridge.md, "Toolbar"). Points are normalised to the picture (0..1) so a resize
// or full screen keeps them in place. Nothing is stored; they are dropped when the picture or
// page underneath changes.

import 'dart:ui' as ui;

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

enum AnnotationTool { pen, marker, eraser, laser }

/// The five pen colours of the web toolbar (same values).
enum AnnotationColor {
  red(Color(0xFFE5484D)),
  yellow(Color(0xFFF5C451)),
  green(Color(0xFF30A46C)),
  blue(Color(0xFF3E8BFF)),
  white(Color(0xFFFFFFFF));

  const AnnotationColor(this.color);
  final Color color;
}

/// Line widths in logical pixels for the three sizes (web: CSS pixels).
const penWidths = [2.0, 4.0, 8.0];
const markerWidths = [12.0, 20.0, 32.0];

/// Eraser radius in logical pixels.
const eraserRadius = 16.0;

/// Highlighter strokes are drawn with this opacity (web: 0.35).
const markerOpacity = 0.35;

/// Undo steps that are kept.
const maxHistory = 100;

/// One stroke. [points] are normalised (0..1) to the picture.
class Stroke {
  Stroke({
    required this.marker,
    required this.color,
    required this.width,
    required Offset first,
  }) : points = [first];

  final bool marker;
  final Color color;

  /// Line width in logical pixels.
  final double width;
  final List<Offset> points;
}

double _distToSegment(Offset p, Offset a, Offset b) {
  final d = b - a;
  final len2 = d.dx * d.dx + d.dy * d.dy;
  final t = len2 == 0
      ? 0.0
      : (((p.dx - a.dx) * d.dx + (p.dy - a.dy) * d.dy) / len2).clamp(0.0, 1.0);
  return (p - (a + d * t)).distance;
}

/// State of the toolbar and the strokes. No widgets in here, so it can be tested alone.
class AnnotationController extends ChangeNotifier {
  AnnotationTool? tool;
  AnnotationColor color = AnnotationColor.red;

  /// 0 thin, 1 medium, 2 thick.
  int size = 1;

  /// The toolbar is open (false: only a small button is shown).
  bool expanded = true;

  List<Stroke> _strokes = const [];
  final List<List<Stroke>> _history = [];
  Stroke? _current;
  bool _erasePushed = false;
  bool _disposed = false;

  /// The laser pointer position (normalised), or null.
  Offset? laser;

  /// The picture that is shown while frozen (a clone owned by this controller), or null.
  ui.Image? frozenImage;

  List<Stroke> get strokes => _strokes;
  bool get canUndo => _history.isNotEmpty;
  bool get isEmpty => _strokes.isEmpty;
  bool get frozen => frozenImage != null;
  bool get drawing =>
      tool == AnnotationTool.pen || tool == AnnotationTool.marker;

  /// Selecting the active tool again deselects it (plain viewing).
  void toggleTool(AnnotationTool t) {
    tool = tool == t ? null : t;
    if (tool != AnnotationTool.laser) laser = null;
    notifyListeners();
  }

  /// Picking a colour while no drawing tool is active switches to the pen.
  void setColor(AnnotationColor c) {
    color = c;
    if (!drawing) tool = AnnotationTool.pen;
    laser = null;
    notifyListeners();
  }

  void setSize(int s) {
    size = s.clamp(0, 2);
    notifyListeners();
  }

  void setExpanded(bool v) {
    expanded = v;
    if (!v) laser = null;
    notifyListeners();
  }

  double get lineWidth =>
      (tool == AnnotationTool.marker ? markerWidths : penWidths)[size];

  void _push() {
    _history.add(_strokes);
    if (_history.length > maxHistory) _history.removeAt(0);
  }

  /// Start a stroke with the current tool at [p] (normalised). Only for pen and marker.
  void beginStroke(Offset p) {
    if (!drawing) return;
    _push();
    final s = Stroke(
      marker: tool == AnnotationTool.marker,
      color: color.color,
      width: lineWidth,
      first: p,
    );
    _current = s;
    _strokes = [..._strokes, s];
    notifyListeners();
  }

  void extend(Offset p) {
    final s = _current;
    if (s == null) return;
    s.points.add(p);
    notifyListeners();
  }

  void endStroke() => _current = null;

  void beginErase() => _erasePushed = false;

  /// Remove every stroke that passes within [eraserRadius] of [p]. [w] and [h] are the size
  /// of the picture in logical pixels. Returns true if something was removed.
  bool eraseAt(Offset p, double w, double h) {
    final px = Offset(p.dx * w, p.dy * h);
    final keep = _strokes.where((s) {
      final r = eraserRadius + s.width / 2;
      Offset abs(Offset o) => Offset(o.dx * w, o.dy * h);
      final pts = s.points;
      if (pts.length == 1) return (abs(pts[0]) - px).distance > r;
      for (var i = 1; i < pts.length; i++) {
        if (_distToSegment(px, abs(pts[i - 1]), abs(pts[i])) <= r) return false;
      }
      return true;
    }).toList();
    if (keep.length == _strokes.length) return false;
    if (!_erasePushed) {
      _push();
      _erasePushed = true;
    }
    _strokes = keep;
    notifyListeners();
    return true;
  }

  bool undo() {
    if (_history.isEmpty) return false;
    _current = null;
    _strokes = _history.removeLast();
    notifyListeners();
    return true;
  }

  void clear() {
    if (_strokes.isEmpty) return;
    _push();
    _strokes = const [];
    notifyListeners();
  }

  void setLaser(Offset? p) {
    if (laser == p) return;
    laser = p;
    notifyListeners();
  }

  /// Freeze on [current] (cloned; the controller disposes its clone). A second call
  /// unfreezes. Does nothing without a picture.
  void toggleFreeze(ui.Image? current) {
    if (frozenImage != null) {
      _dropFrozen();
    } else if (current != null) {
      frozenImage = current.clone();
    }
    notifyListeners();
  }

  void _dropFrozen() {
    final img = frozenImage;
    frozenImage = null;
    // Disposed after the frame: the frame that is being built may still draw it.
    if (img != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => img.dispose());
    }
  }

  /// Drop strokes, undo history, the laser and the frozen picture (the picture or page
  /// underneath changed). The tool, colour and size stay. With [notify] false it is safe to
  /// call while a widget tree is being built.
  void reset({bool notify = true}) {
    _strokes = const [];
    _history.clear();
    _current = null;
    laser = null;
    _dropFrozen();
    if (notify) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    frozenImage?.dispose();
    frozenImage = null;
    super.dispose();
  }

  @override
  void notifyListeners() {
    if (_disposed) return;
    super.notifyListeners();
  }
}

/// Draws the strokes (normalised points) over a box of the given size.
class StrokePainter extends CustomPainter {
  StrokePainter(this.controller) : super(repaint: controller);

  final AnnotationController controller;

  @override
  void paint(Canvas canvas, Size size) {
    for (final s in controller.strokes) {
      final paint = Paint()
        ..color = s.marker ? s.color.withValues(alpha: markerOpacity) : s.color
        ..strokeWidth = s.width
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round
        ..style = PaintingStyle.stroke;
      Offset abs(Offset o) => Offset(o.dx * size.width, o.dy * size.height);
      final first = abs(s.points.first);
      if (s.points.length == 1) {
        canvas.drawCircle(
          first,
          s.width / 2,
          paint..style = PaintingStyle.fill,
        );
        continue;
      }
      final path = Path()..moveTo(first.dx, first.dy);
      for (final p in s.points.skip(1)) {
        final a = abs(p);
        path.lineTo(a.dx, a.dy);
      }
      canvas.drawPath(path, paint);
    }
  }

  @override
  bool shouldRepaint(StrokePainter old) => old.controller != controller;
}

/// The drawing layer. Put it exactly over the picture (same box): points are normalised to
/// it. Without a tool it lets pointer events through.
class AnnotationLayer extends StatelessWidget {
  const AnnotationLayer({super.key, required this.controller});

  final AnnotationController controller;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return LayoutBuilder(
      builder: (context, box) {
        final w = box.maxWidth;
        final h = box.maxHeight;
        Offset norm(Offset local) => Offset(
          w <= 0 ? 0 : (local.dx / w).clamp(0.0, 1.0),
          h <= 0 ? 0 : (local.dy / h).clamp(0.0, 1.0),
        );
        return ListenableBuilder(
          listenable: controller,
          builder: (context, _) {
            final tool = controller.tool;
            final laser = controller.laser;
            return Semantics(
              container: true,
              label: t.toolbarCanvasLabel,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  IgnorePointer(
                    child: CustomPaint(
                      key: const Key('annotationCanvas'),
                      painter: StrokePainter(controller),
                    ),
                  ),
                  IgnorePointer(
                    ignoring: tool == null,
                    child: MouseRegion(
                      cursor: tool == null
                          ? MouseCursor.defer
                          : SystemMouseCursors.precise,
                      // A mouse shows the laser while it hovers; touch and pen only while
                      // pressed (see onPointerDown).
                      onHover: (e) {
                        if (tool == AnnotationTool.laser) {
                          controller.setLaser(norm(e.localPosition));
                        }
                      },
                      onExit: (_) {
                        if (tool == AnnotationTool.laser) {
                          controller.setLaser(null);
                        }
                      },
                      child: Listener(
                        key: const Key('annotationSurface'),
                        behavior: HitTestBehavior.opaque,
                        onPointerDown: (e) {
                          final p = norm(e.localPosition);
                          switch (tool) {
                            case AnnotationTool.pen || AnnotationTool.marker:
                              controller.beginStroke(p);
                            case AnnotationTool.eraser:
                              controller.beginErase();
                              controller.eraseAt(p, w, h);
                            case AnnotationTool.laser:
                              controller.setLaser(p);
                            case null:
                              break;
                          }
                        },
                        onPointerMove: (e) {
                          final p = norm(e.localPosition);
                          switch (tool) {
                            case AnnotationTool.pen || AnnotationTool.marker:
                              controller.extend(p);
                            case AnnotationTool.eraser:
                              controller.eraseAt(p, w, h);
                            case AnnotationTool.laser:
                              controller.setLaser(p);
                            case null:
                              break;
                          }
                        },
                        onPointerUp: (e) {
                          controller.endStroke();
                          if (tool == AnnotationTool.laser &&
                              e.kind != PointerDeviceKind.mouse) {
                            controller.setLaser(null);
                          }
                        },
                        onPointerCancel: (_) {
                          controller.endStroke();
                          if (tool == AnnotationTool.laser) {
                            controller.setLaser(null);
                          }
                        },
                      ),
                    ),
                  ),
                  if (laser != null)
                    Positioned(
                      left: laser.dx * w - _laserSize / 2,
                      top: laser.dy * h - _laserSize / 2,
                      width: _laserSize,
                      height: _laserSize,
                      child: const IgnorePointer(child: _LaserDot()),
                    ),
                ],
              ),
            );
          },
        );
      },
    );
  }
}

const _laserSize = 22.0;

/// The red laser dot with a soft glow.
class _LaserDot extends StatelessWidget {
  const _LaserDot();

  @override
  Widget build(BuildContext context) {
    return Container(
      key: const Key('laserDot'),
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: const Color(0xFFFF2D2D),
        border: Border.all(color: Colors.white70, width: 1.5),
        boxShadow: const [
          BoxShadow(color: Color(0x99FF2D2D), blurRadius: 12, spreadRadius: 3),
        ],
      ),
    );
  }
}

/// The floating toolbar. [freezeSource] returns the picture to freeze (the live video image),
/// or null where freezing makes no sense (a received image or page): then the button is off.
class AnnotationToolbar extends StatelessWidget {
  const AnnotationToolbar({
    super.key,
    required this.controller,
    this.freezeSource,
  });

  final AnnotationController controller;
  final ui.Image? Function()? freezeSource;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final c = controller;
        if (!c.expanded) {
          return Material(
            color: Colors.black54,
            shape: const CircleBorder(),
            child: IconButton(
              key: const Key('toolbarExpand'),
              tooltip: t.toolbarExpand,
              color: Colors.white,
              icon: const Icon(Icons.edit),
              onPressed: () => c.setExpanded(true),
            ),
          );
        }
        final canFreeze = freezeSource != null;
        Widget tool(
          String key,
          AnnotationTool tl,
          IconData icon,
          String label,
        ) => _ToolButton(
          buttonKey: Key(key),
          icon: icon,
          label: label,
          selected: c.tool == tl,
          onPressed: () => c.toggleTool(tl),
        );
        return Semantics(
          container: true,
          label: t.toolbarLabel,
          child: Material(
            key: const Key('annotationToolbar'),
            color: Colors.black54,
            borderRadius: BorderRadius.circular(28),
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    tool(
                      'toolPen',
                      AnnotationTool.pen,
                      Icons.edit,
                      t.toolbarPen,
                    ),
                    tool(
                      'toolMarker',
                      AnnotationTool.marker,
                      Icons.border_color,
                      t.toolbarMarker,
                    ),
                    tool(
                      'toolEraser',
                      AnnotationTool.eraser,
                      Icons.auto_fix_normal,
                      t.toolbarEraser,
                    ),
                    tool(
                      'toolLaser',
                      AnnotationTool.laser,
                      Icons.adjust,
                      t.toolbarLaser,
                    ),
                    const _Divider(),
                    for (final col in AnnotationColor.values)
                      _ColorButton(
                        color: col,
                        label: _colorName(t, col),
                        selected: c.color == col && c.drawing,
                        onPressed: () => c.setColor(col),
                      ),
                    const _Divider(),
                    for (var i = 0; i < 3; i++)
                      _SizeButton(
                        index: i,
                        label: _sizeName(t, i),
                        selected: c.size == i,
                        onPressed: () => c.setSize(i),
                      ),
                    const _Divider(),
                    _ToolButton(
                      buttonKey: const Key('toolUndo'),
                      icon: Icons.undo,
                      label: t.toolbarUndo,
                      onPressed: c.canUndo ? c.undo : null,
                    ),
                    _ToolButton(
                      buttonKey: const Key('toolClear'),
                      icon: Icons.delete_outline,
                      label: t.toolbarClear,
                      onPressed: c.isEmpty ? null : c.clear,
                    ),
                    _ToolButton(
                      buttonKey: const Key('toolFreeze'),
                      icon: Icons.pause_circle_outline,
                      label: c.frozen ? t.toolbarUnfreeze : t.toolbarFreeze,
                      selected: c.frozen,
                      onPressed: (canFreeze || c.frozen)
                          ? () => c.toggleFreeze(freezeSource?.call())
                          : null,
                    ),
                    _ToolButton(
                      buttonKey: const Key('toolbarCollapse'),
                      icon: Icons.chevron_left,
                      label: t.toolbarCollapse,
                      onPressed: () => c.setExpanded(false),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

String _colorName(AppLocalizations t, AnnotationColor c) => switch (c) {
  AnnotationColor.red => t.toolbarColorRed,
  AnnotationColor.yellow => t.toolbarColorYellow,
  AnnotationColor.green => t.toolbarColorGreen,
  AnnotationColor.blue => t.toolbarColorBlue,
  AnnotationColor.white => t.toolbarColorWhite,
};

String _sizeName(AppLocalizations t, int i) => switch (i) {
  0 => t.toolbarSizeSmall,
  1 => t.toolbarSizeMedium,
  _ => t.toolbarSizeLarge,
};

class _Divider extends StatelessWidget {
  const _Divider();

  @override
  Widget build(BuildContext context) => const SizedBox(
    height: 24,
    child: VerticalDivider(width: 12, color: Colors.white38),
  );
}

class _ToolButton extends StatelessWidget {
  const _ToolButton({
    required this.buttonKey,
    required this.icon,
    required this.label,
    required this.onPressed,
    this.selected,
  });

  final Key buttonKey;
  final IconData icon;
  final String label;
  final VoidCallback? onPressed;

  /// null: not a toggle.
  final bool? selected;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      toggled: selected,
      child: IconButton(
        key: buttonKey,
        tooltip: label,
        color: Colors.white,
        disabledColor: Colors.white30,
        style: IconButton.styleFrom(
          backgroundColor: selected == true ? Colors.white24 : null,
        ),
        icon: Icon(icon),
        onPressed: onPressed,
      ),
    );
  }
}

class _ColorButton extends StatelessWidget {
  const _ColorButton({
    required this.color,
    required this.label,
    required this.selected,
    required this.onPressed,
  });

  final AnnotationColor color;
  final String label;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      selected: selected,
      label: label,
      excludeSemantics: true,
      child: Tooltip(
        message: label,
        child: InkResponse(
          key: Key('color_${color.name}'),
          onTap: onPressed,
          radius: 20,
          child: Padding(
            padding: const EdgeInsets.all(10),
            child: Container(
              width: 20,
              height: 20,
              decoration: BoxDecoration(
                color: color.color,
                shape: BoxShape.circle,
                border: Border.all(
                  color: selected ? Colors.white : Colors.white38,
                  width: selected ? 3 : 1,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _SizeButton extends StatelessWidget {
  const _SizeButton({
    required this.index,
    required this.label,
    required this.selected,
    required this.onPressed,
  });

  final int index;
  final String label;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final d = 6.0 + index * 4;
    return Semantics(
      button: true,
      selected: selected,
      label: label,
      excludeSemantics: true,
      child: Tooltip(
        message: label,
        child: InkResponse(
          key: Key('size_$index'),
          onTap: onPressed,
          radius: 20,
          child: Padding(
            padding: const EdgeInsets.all(10),
            child: SizedBox(
              width: 20,
              height: 20,
              child: Center(
                child: Container(
                  width: d,
                  height: d,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: selected ? Colors.white : Colors.white54,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The hint shown while the image is frozen.
class FrozenHint extends StatelessWidget {
  const FrozenHint({super.key});

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return Semantics(
      liveRegion: true,
      child: Container(
        key: const Key('frozenHint'),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(
          color: Colors.black54,
          borderRadius: BorderRadius.circular(16),
        ),
        child: Text(
          t.toolbarFrozenHint,
          style: const TextStyle(color: Colors.white),
        ),
      ),
    );
  }
}
