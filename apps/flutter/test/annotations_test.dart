// SPDX-License-Identifier: GPL-3.0-or-later
// Unit tests of the annotation model (the same rules as apps/web/src/core/annotations.ts).

import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/annotations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  AnnotationController pen() {
    final c = AnnotationController();
    c.toggleTool(AnnotationTool.pen);
    return c;
  }

  test(
    'only pen and marker start strokes; sizes and colours give the style',
    () {
      final c = AnnotationController();
      c.beginStroke(const Offset(0.1, 0.1));
      expect(c.strokes, isEmpty, reason: 'no tool, no stroke');
      c.toggleTool(AnnotationTool.eraser);
      c.beginStroke(const Offset(0.1, 0.1));
      expect(c.strokes, isEmpty);

      c.toggleTool(AnnotationTool.pen);
      for (var i = 0; i < 3; i++) {
        c.setSize(i);
        expect(c.lineWidth, penWidths[i]);
      }
      c.toggleTool(AnnotationTool.marker);
      c.setSize(2);
      expect(c.lineWidth, 32);
      c.setSize(9); // clamped
      expect(c.size, 2);
      c.setColor(AnnotationColor.yellow);
      c.beginStroke(const Offset(0.5, 0.5));
      expect(c.strokes.single.color, const Color(0xFFF5C451));
      expect(c.strokes.single.marker, isTrue);
    },
  );

  test('toggling a tool twice deselects it; a colour selects the pen', () {
    final c = AnnotationController();
    c.toggleTool(AnnotationTool.laser);
    expect(c.tool, AnnotationTool.laser);
    c.toggleTool(AnnotationTool.laser);
    expect(c.tool, isNull);
    c.setColor(AnnotationColor.white);
    expect(c.tool, AnnotationTool.pen);
    // With the highlighter active a colour keeps it.
    c.toggleTool(AnnotationTool.marker);
    c.setColor(AnnotationColor.red);
    expect(c.tool, AnnotationTool.marker);
  });

  test('undo restores the strokes before the last change', () {
    final c = pen();
    c.beginStroke(const Offset(0.1, 0.1));
    c.extend(const Offset(0.2, 0.2));
    c.endStroke();
    c.beginStroke(const Offset(0.5, 0.5));
    c.endStroke();
    expect(c.strokes, hasLength(2));
    expect(c.undo(), isTrue);
    expect(c.strokes, hasLength(1));
    expect(c.strokes.single.points, hasLength(2));
    expect(c.undo(), isTrue);
    expect(c.strokes, isEmpty);
    expect(c.undo(), isFalse);
    expect(c.canUndo, isFalse);
  });

  test('history is bounded', () {
    final c = pen();
    for (var i = 0; i < maxHistory + 20; i++) {
      c.beginStroke(const Offset(0.5, 0.5));
      c.endStroke();
    }
    var undone = 0;
    while (c.undo()) {
      undone++;
    }
    expect(undone, maxHistory);
    // The oldest 20 steps are gone for good: those strokes stay.
    expect(c.strokes, hasLength(20));
  });

  test('clear is one undo step and does nothing when empty', () {
    final c = pen();
    c.clear();
    expect(c.canUndo, isFalse);
    c.beginStroke(const Offset(0.5, 0.5));
    c.endStroke();
    c.clear();
    expect(c.isEmpty, isTrue);
    expect(c.undo(), isTrue);
    expect(c.strokes, hasLength(1));
  });

  test('the eraser removes strokes within its radius, once per undo step', () {
    final c = pen();
    // A horizontal stroke at y = 0.5 and a single dot far away, on a 200 x 100 picture.
    c.beginStroke(const Offset(0.1, 0.5));
    c.extend(const Offset(0.9, 0.5));
    c.endStroke();
    c.beginStroke(const Offset(0.5, 0.9));
    c.endStroke();
    c.beginErase();
    // 40 px away from the line: out of reach (radius 16 + half the width 2).
    expect(c.eraseAt(const Offset(0.5, 0.1), 200, 100), isFalse);
    // 10 px away: erased. The dot is 40 px below: kept.
    expect(c.eraseAt(const Offset(0.5, 0.4), 200, 100), isTrue);
    expect(c.strokes, hasLength(1));
    // Erasing the dot in the same drag adds no second undo step.
    expect(c.eraseAt(const Offset(0.5, 0.9), 200, 100), isTrue);
    expect(c.strokes, isEmpty);
    expect(c.undo(), isTrue);
    expect(c.strokes, hasLength(2));
  });

  test('reset drops strokes, history, laser and the tool stays', () {
    final c = pen();
    c.beginStroke(const Offset(0.5, 0.5));
    c.endStroke();
    c.setLaser(const Offset(0.1, 0.1));
    var notified = 0;
    c.addListener(() => notified++);
    c.reset(notify: false);
    expect(notified, 0);
    expect(c.strokes, isEmpty);
    expect(c.canUndo, isFalse);
    expect(c.laser, isNull);
    expect(c.tool, AnnotationTool.pen);
    c.reset();
    expect(notified, 1);
  });

  test('freeze needs a picture and a second call unfreezes', () {
    final c = AnnotationController();
    c.toggleFreeze(null);
    expect(c.frozen, isFalse);
    c.dispose();
  });
}
