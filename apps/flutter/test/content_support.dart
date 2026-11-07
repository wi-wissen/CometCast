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

/// A minimal real `.docx` (a ZIP with stored entries: content types, the package relationship
/// and one paragraph), as the Office conversion of the Rust core accepts it.
Uint8List makeDocx(String text) {
  const head = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>';
  final parts = <(String, String)>[
    (
      '[Content_Types].xml',
      '$head<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
          '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
          '<Default Extension="xml" ContentType="application/xml"/>'
          '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>',
    ),
    (
      '_rels/.rels',
      '$head<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
          '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
    ),
    (
      'word/document.xml',
      '$head<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>'
          '<w:p><w:r><w:t>$text</w:t></w:r></w:p>'
          '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr>'
          '</w:body></w:document>',
    ),
  ];
  ByteData le(int bytes, int v) {
    final d = ByteData(bytes);
    if (bytes == 2) {
      d.setUint16(0, v, Endian.little);
    } else {
      d.setUint32(0, v, Endian.little);
    }
    return d;
  }

  final out = BytesBuilder();
  final central = BytesBuilder();
  for (final (name, content) in parts) {
    final n = name.codeUnits;
    final data = content.codeUnits;
    final crc = _crc32(data);
    final offset = out.length;
    void h(BytesBuilder b, int bytes, int v) =>
        b.add(le(bytes, v).buffer.asUint8List());
    // Local header: stored, no flags.
    h(out, 4, 0x04034b50);
    h(out, 2, 20);
    h(out, 2, 0);
    h(out, 2, 0);
    h(out, 4, 0); // time and date
    h(out, 4, crc);
    h(out, 4, data.length);
    h(out, 4, data.length);
    h(out, 2, n.length);
    h(out, 2, 0);
    out.add(n);
    out.add(data);
    // Central directory entry.
    h(central, 4, 0x02014b50);
    h(central, 2, 20);
    h(central, 2, 20);
    h(central, 2, 0);
    h(central, 2, 0);
    h(central, 4, 0);
    h(central, 4, crc);
    h(central, 4, data.length);
    h(central, 4, data.length);
    h(central, 2, n.length);
    h(central, 2, 0);
    h(central, 2, 0);
    h(central, 2, 0);
    h(central, 2, 0);
    h(central, 4, 0);
    h(central, 4, offset);
    central.add(n);
  }
  final centralStart = out.length;
  final centralBytes = central.toBytes();
  out.add(centralBytes);
  void e(int bytes, int v) => out.add(le(bytes, v).buffer.asUint8List());
  e(4, 0x06054b50);
  e(2, 0);
  e(2, 0);
  e(2, parts.length);
  e(2, parts.length);
  e(4, centralBytes.length);
  e(4, centralStart);
  e(2, 0);
  return out.toBytes();
}
