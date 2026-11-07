// SPDX-License-Identifier: GPL-3.0-or-later
// Row layout of "screens on this network" (decisions N61 to N64, fix list 4 A1 to A3): the code
// moves under the subtitle in a narrow row, host names break only after a dot, a connecting row
// keeps its size.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/presenter/host_text.dart';
import 'package:cometcast/src/presenter/pages.dart' show PresenterDevice;

import 'nearby_support.dart';

/// The lines of the drawn text of a [HostText] as strings.
List<String> _lines(WidgetTester t, Finder f) {
  final r = t.renderObject<RenderHostText>(f);
  final tp = r.painter!;
  final text = tp.text!.toPlainText();
  final lines = <String>[];
  var start = 0;
  while (start < text.length) {
    final b = tp.getLineBoundary(TextPosition(offset: start));
    final end = b.end <= start ? text.length : b.end;
    lines.add(text.substring(start, end));
    start = end;
  }
  return lines;
}

void main() {
  testWidgets('breakAfterDots adds U+200B after every dot only', (t) async {
    expect(
      HostText.breakAfterDots('cast.musterfirma.de'),
      'cast.\u200Bmusterfirma.\u200Bde',
    );
    expect(HostText.breakAfterDots('relay'), 'relay');
  });

  testWidgets('a host that fits moves down whole, a wider one breaks at dots', (
    t,
  ) async {
    Widget at(double w, String host) => Directionality(
      textDirection: TextDirection.ltr,
      child: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(
          width: w,
          child: HostText(
            'Mit Freigabe · über\u00A0$host',
            host: host,
            style: const TextStyle(fontSize: 13),
          ),
        ),
      ),
    );
    await t.pumpWidget(at(320, 'cast.musterfirma.de'));
    var r = t.renderObject<RenderHostText>(find.byType(HostText));
    expect(r.usesDotted, isFalse);
    expect(
      _lines(t, find.byType(HostText)).last,
      contains('cast.musterfirma.de'),
    );
    await t.pumpWidget(at(150, 'cast.musterfirma.de'));
    r = t.renderObject<RenderHostText>(find.byType(HostText));
    expect(r.usesDotted, isTrue);
  });

  testWidgets('a narrow row puts the code under the subtitle', (t) async {
    final n = await pumpNearby(t, size: const Size(300, 800));
    await found(t, n, [screen('a', name: 'Raum', code: 'K7QM2W')]);
    final title = t.getTopLeft(find.text('“Raum”'));
    final sub = t.getTopLeft(find.text('Connects at once'));
    final code = t.getTopLeft(
      find.byKey(Key('nearby-code-${n.controller.rows.first.id}')),
    );
    expect(code.dx, title.dx, reason: 'left-aligned with the title');
    expect(code.dy, greaterThan(sub.dy));
    await done(t, n);
  });

  testWidgets('a row of normal width keeps the code on the right', (t) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen('a', name: 'Raum', code: 'K7QM2W')]);
    final title = t.getTopLeft(find.text('“Raum”'));
    final code = t.getTopLeft(
      find.byKey(Key('nearby-code-${n.controller.rows.first.id}')),
    );
    expect(code.dx, greaterThan(title.dx + 100));
    expect(code.dy, lessThan(title.dy + 30));
    await done(t, n);
  });

  testWidgets('a long host breaks only after dots or the hyphen', (t) async {
    const host = 'cast.abteilung-nord.musterfirma-gruppe.de';
    final n = await pumpNearby(t, size: const Size(300, 800));
    await found(t, n, [
      screen(
        'a',
        name: 'Raum',
        mode: NearbyMode.approval,
        server: 'wss://$host:443',
      ),
    ]);
    // The address line under the code field (spec 15.7) is a HostText too: take the row's.
    final f = find.byWidgetPredicate(
      (w) => w is HostText && w.data!.contains('Approval'),
    );
    expect(f, findsOneWidget);
    final lines = _lines(t, f);
    expect(lines.length, greaterThan(1));
    for (final l in lines.take(lines.length - 1)) {
      expect(
        RegExp(r'[  ​\-.]$').hasMatch(l),
        isTrue,
        reason: 'line "$l" ends inside a word',
      );
    }
    expect(lines.first.contains('über') || lines.first.contains('via'), isTrue);
    await done(t, n);
  });

  for (final w in [360.0, 1100.0]) {
    testWidgets('a connecting row keeps its height at $w wide', (t) async {
      final n = await pumpNearby(
        t,
        size: Size(w, 800),
        device: w > 600 ? PresenterDevice.computer : PresenterDevice.phone,
        desktop: w > 600,
        platform: FakeNearbyPlatform(limited: w <= 600),
      );
      await found(t, n, [
        screen('a', name: 'Besprechungsraum Elbe', code: 'K7QM2W'),
        screen('b', name: 'Raum 204', code: 'P4X9TR'),
      ]);
      final row = find.byKey(const Key('nearby-row-a'));
      final below = find.byKey(const Key('nearby-row-b'));
      final h = t.getSize(row).height;
      final y = t.getTopLeft(below).dy;
      await t.tap(row);
      await t.pump(const Duration(milliseconds: 100));
      expect(t.getSize(row).height, h);
      expect(t.getTopLeft(below).dy, y);
      await done(t, n);
    });
  }
}
