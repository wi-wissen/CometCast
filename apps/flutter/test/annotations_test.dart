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

  test('close points are dropped and a stroke has a point cap', () {
    final c = pen();
    c.beginStroke(const Offset(0.5, 0.5));
    // Closer than minPointDistance: nothing visible, nothing stored.
    c.extend(const Offset(0.5 + minPointDistance / 4, 0.5));
    expect(c.strokes.single.points, hasLength(1));
    c.extend(const Offset(0.6, 0.5));
    expect(c.strokes.single.points, hasLength(2));
    // A long scribble stops at the cap; the stroke is still valid and the next one starts.
    for (var i = 0; i < maxPointsPerStroke * 2; i++) {
      c.extend(Offset(0.5 + (i % 2 == 0 ? 0.2 : 0.3), (i % 400) / 400));
    }
    expect(c.strokes.single.points, hasLength(maxPointsPerStroke));
    c.endStroke();
    c.beginStroke(const Offset(0.1, 0.1));
    expect(c.strokes, hasLength(2));
  });

  test('the number of strokes is capped until something makes room', () {
    final c = pen();
    for (var i = 0; i < maxStrokes + 10; i++) {
      c.beginStroke(Offset((i % 100) / 100, (i ~/ 100) / 10));
      c.endStroke();
    }
    expect(c.strokes, hasLength(maxStrokes));
    // Undo makes room for one more.
    expect(c.undo(), isTrue);
    expect(c.strokes, hasLength(maxStrokes - 1));
    c.beginStroke(const Offset(0.9, 0.9));
    expect(c.strokes, hasLength(maxStrokes));
    c.endStroke();
    c.clear();
    c.beginStroke(const Offset(0.9, 0.9));
    expect(c.strokes, hasLength(1));
  });

  test('only the first pointer owns the gesture', () {
    final c = pen();
    expect(c.acquirePointer(1), isTrue);
    expect(c.acquirePointer(2), isFalse);
    expect(c.activePointer, 1);
    c.releasePointer(2); // not the owner: nothing happens
    expect(c.activePointer, 1);
    c.beginStroke(const Offset(0.5, 0.5));
    c.releasePointer(1);
    expect(c.activePointer, isNull);
    expect(
      c.currentStroke,
      isNull,
      reason: 'the stroke ended with its pointer',
    );
    expect(c.acquirePointer(2), isTrue);
  });

  test('clear, undo, reset and a tool change end the stroke being drawn', () {
    void drawing(AnnotationController c) {
      c.acquirePointer(7);
      c.beginStroke(const Offset(0.5, 0.5));
      c.extend(const Offset(0.6, 0.6));
      expect(c.currentStroke, isNotNull);
    }

    void ended(AnnotationController c, String why) {
      expect(c.currentStroke, isNull, reason: why);
      expect(c.activePointer, isNull, reason: why);
      // The moves of the pointer that is still down add nothing.
      final n = c.strokes.fold<int>(0, (a, s) => a + s.points.length);
      c.extend(const Offset(0.9, 0.9));
      expect(
        c.strokes.fold<int>(0, (a, s) => a + s.points.length),
        n,
        reason: why,
      );
    }

    var c = pen();
    drawing(c);
    c.clear();
    ended(c, 'clear');
    c = pen();
    drawing(c);
    expect(c.undo(), isTrue);
    ended(c, 'undo');
    c = pen();
    drawing(c);
    c.toggleTool(AnnotationTool.eraser);
    ended(c, 'tool change');
    c = pen();
    drawing(c);
    c.setColor(AnnotationColor.blue);
    ended(c, 'colour change');
    c = pen();
    drawing(c);
    c.reset();
    ended(c, 'reset');
  });

  test('the painter records finished strokes once and grows the live path', () {
    final c = pen();
    final painter = StrokePainter(c);
    const size = Size(200, 100);
    void paint() {
      final rec = PictureRecorder();
      painter.paint(Canvas(rec), size);
      rec.endRecording().dispose();
    }

    for (var i = 0; i < 3; i++) {
      c.beginStroke(Offset(0.1 * (i + 1), 0.1));
      paint();
      c.endStroke();
      paint();
    }
    expect(c.strokes, hasLength(3));
    final builds = painter.pictureBuilds;
    // One long live stroke: many moves, no new picture and one path build.
    c.beginStroke(const Offset(0.05, 0.5));
    paint();
    final before = painter.pictureBuilds;
    for (var i = 1; i <= 100; i++) {
      c.extend(Offset(0.05 + i * 0.008, 0.5 + (i % 2) * 0.1));
      paint();
    }
    expect(painter.pictureBuilds, before);
    expect(painter.livePathRebuilds, 1);
    expect(before - builds, lessThanOrEqualTo(1));
    // A new size records the picture again; ending the stroke does too.
    final rec = PictureRecorder();
    painter.paint(Canvas(rec), const Size(300, 150));
    rec.endRecording().dispose();
    expect(painter.pictureBuilds, before + 1);
    c.endStroke();
    paint();
    expect(painter.pictureBuilds, before + 2);
    painter.dispose();
  });
}
