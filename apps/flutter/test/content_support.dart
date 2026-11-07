// SPDX-License-Identifier: GPL-3.0-or-later
// Helpers for the content tests: small real pictures and waiting for the real event loop.

import 'dart:io' show ZLibCodec;
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// CRC-32 of the PNG chunks.
int _crc32(List<int> bytes) {
  var c = 0xFFFFFFFF;
  for (final b in bytes) {
    c ^= b;
    for (var k = 0; k < 8; k++) {
      c = (c & 1) != 0 ? (0xEDB88320 ^ (c >> 1)) : (c >> 1);
    }
  }
  return c ^ 0xFFFFFFFF;
}

Uint8List _chunk(String type, List<int> data) {
  final body = <int>[...type.codeUnits, ...data];
  final out = BytesBuilder();
  out.add((ByteData(4)..setUint32(0, data.length)).buffer.asUint8List());
  out.add(body);
  out.add((ByteData(4)..setUint32(0, _crc32(body))).buffer.asUint8List());
  return out.toBytes();
}

/// A real PNG of one colour, [w] x [h] pixels.
Uint8List makePng(int w, int h, List<int> rgb) {
  final raw = BytesBuilder();
  for (var y = 0; y < h; y++) {
    raw.addByte(0); // filter: none
    for (var x = 0; x < w; x++) {
      raw.add(rgb);
    }
  }
  final ihdr = ByteData(13)
    ..setUint32(0, w)
    ..setUint32(4, h)
    ..setUint8(8, 8) // bit depth
    ..setUint8(9, 2); // colour type: RGB
  final out = BytesBuilder();
  out.add([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
  out.add(_chunk('IHDR', ihdr.buffer.asUint8List()));
  out.add(_chunk('IDAT', ZLibCodec().encode(raw.toBytes())));
  out.add(_chunk('IEND', const []));
  return out.toBytes();
}

/// RGBA pixels of one colour, as the core hands out a rendered page.
Uint8List solidRgba(int w, int h, List<int> rgb) {
  final px = Uint8List(w * h * 4);
  for (var i = 0; i < w * h; i++) {
    px.setRange(i * 4, i * 4 + 4, [...rgb, 255]);
  }
  return px;
}

/// Pump (in real time, up to [seconds]) until [finder] matches. Decoding images needs the
/// real event loop, hence runAsync.
Future<bool> pumpUntilFound(
  WidgetTester tester,
  Finder finder, {
  int seconds = 5,
}) async {
  for (var i = 0; i < seconds * 10; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 100)),
    );
    await tester.pump();
    if (finder.evaluate().isNotEmpty) return true;
  }
  return false;
}

/// Pump until the predicate holds.
Future<bool> pumpUntil(
  WidgetTester tester,
  bool Function() done, {
  int seconds = 5,
}) async {
  for (var i = 0; i < seconds * 10; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 100)),
    );
    await tester.pump();
    if (done()) return true;
  }
  return false;
}

/// The first pixel (r, g, b) of the [RawImage] inside the widget with [key].
Future<List<int>> firstPixel(WidgetTester tester, Key key) async {
  final raw = tester.widget<RawImage>(
    find.descendant(of: find.byKey(key), matching: find.byType(RawImage)),
  );
  final data = await tester.runAsync(() => raw.image!.toByteData());
  return [data!.getUint8(0), data.getUint8(1), data.getUint8(2)];
}
