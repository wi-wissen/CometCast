// SPDX-License-Identifier: GPL-3.0-or-later
// Screenshot capture for the review lists: writes a PNG of the current frame (no golden files).
// The test binding switches shadows off (decision N21); the screenshots are taken with them on, as
// the product draws them.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

/// Where the screenshots go (outside the repository). Override with COMETCAST_SHOTS.
String shotsDir() =>
    Platform.environment['COMETCAST_SHOTS'] ?? '/tmp/shots-flutter2';

/// A test with soft shadows (the binding's invariant check wants the value restored inside the
/// test body, so the reset cannot be a tearDown).
void shotTest(String name, Future<void> Function(WidgetTester t) body) {
  testWidgets(name, (t) async {
    debugDisableShadows = false;
    try {
      await body(t);
    } finally {
      debugDisableShadows = true;
    }
  });
}

/// One more frame first (focus and fades settle), then the PNG.
Future<void> shot(WidgetTester tester, String name) async {
  await tester.pump(const Duration(milliseconds: 16));
  final dir = Directory(shotsDir())..createSync(recursive: true);
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer as OffsetLayer;
  await tester.runAsync(() async {
    final image = await layer.toImage(Offset.zero & view.size, pixelRatio: 1);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    File('${dir.path}/$name.png').writeAsBytesSync(bytes!.buffer.asUint8List());
  });
}
