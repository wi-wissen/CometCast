// SPDX-License-Identifier: GPL-3.0-or-later
// Screenshot capture for the review lists: writes a PNG of the current frame (no golden files).

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

/// Where the screenshots go (outside the repository). Override with COMETCAST_SHOTS.
String shotsDir() =>
    Platform.environment['COMETCAST_SHOTS'] ?? '/tmp/shots-flutter';

Future<void> shot(WidgetTester tester, String name) async {
  final dir = Directory(shotsDir())..createSync(recursive: true);
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer as OffsetLayer;
  await tester.runAsync(() async {
    final image = await layer.toImage(Offset.zero & view.size, pixelRatio: 1);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    File('${dir.path}/$name.png').writeAsBytesSync(bytes!.buffer.asUint8List());
  });
}
