// SPDX-License-Identifier: GPL-3.0-or-later
// End-to-end test of the content channel with the REAL Rust core, the real UI, a real
// signalling server, the real PDF render worker (bundled next to the app) and the real
// `cometcast-cli`. Run it with `integration_test/run_content_e2e.sh` (see apps/flutter/README.md).
//
// A1 (app = receiver, PDF): `cometcast-cli send-file CODE classroom.pdf --then next,go:3,prev,stop`.
//    The page indicator must walk 1, 2, 3, 2 of 3; the pixels the app drew differ per page; the
//    page is drawn at the width of the view; after `stop` the page is gone.
// A2 (app = receiver, image): `send-file CODE picture.png`; the image is shown; closing it
//    goes back to waiting.
// A3 (app = receiver, link): `send-link CODE https://...`; a card with the host is shown and
//    nothing is opened until "Open" is clicked.
// B  (app = sender, PDF): the app sends classroom.pdf to `cometcast-cli receive`; its `Content:`
//    lines show the document and the pages the presenter controls asked for.
//
// Needed --dart-define values:
//   COMETCAST_TEST_SERVER   host:port of cometcast-server
//   COMETCAST_CLI           path of the cometcast-cli binary
//   COMETCAST_TEST_PDF      path of a PDF with 3 pages (crates/cc-docs/tests/data/classroom.pdf)

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/rust_loader.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';

import '../test/content_support.dart' show makePng;

const _server = String.fromEnvironment('COMETCAST_TEST_SERVER');
const _cli = String.fromEnvironment('COMETCAST_CLI');
const _pdf = String.fromEnvironment('COMETCAST_TEST_PDF');

void _log(String s) {
  // ignore: avoid_print
  print('CONTENT-E2E $s');
}

Future<bool> _waitFor(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 40),
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 100));
    if (done()) return true;
  }
  return done();
}

Widget _app({List<Uri>? opened, String? pickPath}) => CometCastApp(
  locale: const Locale('en'),
  core: const RustCoreApi(),
  settings: AppSettings(serverAddress: _server, udpBind: '127.0.0.1:0'),
  pickFile: (_) async => pickPath,
  openUrl: (u) async {
    opened?.add(u);
    return true;
  },
);

/// Opens the receive screen, starts it and returns the room code.
Future<String> _startReceiver(WidgetTester tester) async {
  await tester.tap(find.text('Receive on this device'));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const Key('startButton')));
  final got = await _waitFor(
    tester,
    () =>
        (tester.widget(
          find.byKey(const Key('roomCode')),
        ) as SelectableText).data !=
        '······',
  );
  expect(got, isTrue, reason: 'no room code');
  return (tester.widget(
    find.byKey(const Key('roomCode')),
  ) as SelectableText).data!;
}

/// A running `cometcast-cli`, its output is in a log file.
class _Cli {
  _Cli(this.pid, this.log);

  final int pid;
  final File log;

  List<String> get lines {
    try {
      return log.readAsLinesSync();
    } on FileSystemException {
      return [];
    }
  }

  void kill() => Process.killPid(pid);
}

/// Starts `cometcast-cli` with [args], detached, with its output in a log file.
///
/// Why not a plain `Process.start`: while the Dart VM tracks a child process, its exit handler
/// can collect (`waitpid`) children that it did not start, including the PDF render worker
/// that the Rust core starts; the core then reports "cannot watch the worker" (observed:
/// after `Process.start` the page count of a PDF came back unknown, before and after a
/// detached start it was 3). A detached start is not tracked. The app itself never starts a
/// process from Dart, so this only concerns the test.
Future<_Cli> _cliProcess(List<String> args) async {
  final dir = Directory.systemTemp.createTempSync('cometcast_cli_');
  addTearDown(() => dir.deleteSync(recursive: true));
  final log = File('${dir.path}/out.log');
  final p = await Process.start(
    '/bin/sh',
    ['-c', 'exec "\$0" "\$@" > "\$CC_LOG" 2>&1', _cli, ...args],
    mode: ProcessStartMode.detached,
    environment: {'CC_LOG': log.path},
  );
  return _Cli(p.pid, log);
}

/// A stable fingerprint of the pixels the app drew in the page view.
Future<int> _pageHash(WidgetTester tester) async {
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(const Key('pageView')),
  );
  return (await tester.runAsync(() async {
    final img = await boundary.toImage();
    final data = await img.toByteData(format: ui.ImageByteFormat.rawRgba);
    img.dispose();
    var h = 0xcbf29ce484222325;
    for (final b in data!.buffer.asUint8List()) {
      h = ((h ^ b) * 0x100000001b3) & 0x7fffffffffffffff;
    }
    return h;
  }))!;
}

String? _indicator(WidgetTester tester) {
  final f = find.byKey(const Key('pageIndicator'));
  if (f.evaluate().isEmpty) return null;
  return (tester.widget(f) as Text).data;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await initRustCore();
  });

  void needs() {
    expect(
      _server,
      isNotEmpty,
      reason: 'pass --dart-define=COMETCAST_TEST_SERVER',
    );
    expect(_cli, isNotEmpty, reason: 'pass --dart-define=COMETCAST_CLI');
    expect(_pdf, isNotEmpty, reason: 'pass --dart-define=COMETCAST_TEST_PDF');
  }

  testWidgets('the PDF worker is bundled next to the app', (tester) async {
    final exe = File(Platform.resolvedExecutable);
    final worker = File('${exe.parent.path}/cc-docs-worker');
    _log('executable ${exe.path}');
    expect(
      worker.existsSync(),
      isTrue,
      reason: 'flutter build linux must install cc-docs-worker next to the executable',
    );
    // The Rust core finds it from there (no environment variable is set by the script).
    expect(
      Platform.environment['COMETCAST_DOCS_WORKER'],
      anyOf(isNull, isEmpty),
    );
    expect(const RustCoreApi().docsWorkerPath(), worker.path);
  });

  testWidgets('A1: receiver shows PDF pages that the sender turns', (
    tester,
  ) async {
    needs();
    await tester.pumpWidget(_app());
    await tester.pumpAndSettle();
    final code = await _startReceiver(tester);
    _log('room code $code');

    final cli = await _cliProcess([
      'send-file',
      code,
      _pdf,
      '--then',
      'next,go:3,prev,stop',
      '--server',
      _server,
      '--bind',
      '127.0.0.1:0',
    ]);
    addTearDown(cli.kill);

    // Watch the page indicator and the drawn pixels until the sender's `stop` removes the page.
    final seen = <String>[];
    final hashes = <String, int>{};
    int? pageWidth;
    var sawPage = false;
    final end = DateTime.now().add(const Duration(seconds: 60));
    while (DateTime.now().isBefore(end)) {
      await tester.pump(const Duration(milliseconds: 40));
      final ind = _indicator(tester);
      if (ind != null) {
        sawPage = true;
        if (seen.isEmpty || seen.last != ind) {
          seen.add(ind);
          hashes[ind] = await _pageHash(tester);
          pageWidth ??= tester
              .widget<RawImage>(
                find.descendant(
                  of: find.byKey(const Key('pageView')),
                  matching: find.byType(RawImage),
                ),
              )
              .image!
              .width;
          _log('indicator "$ind" hash ${hashes[ind]}');
        }
      } else if (sawPage) {
        break; // `stop` came: the page is gone
      }
    }
    expect(seen, [
      'Page 1 of 3',
      'Page 2 of 3',
      'Page 3 of 3',
      'Page 2 of 3',
    ], reason: 'CLI output: ${cli.lines}');
    // Different pages look different.
    expect({
      hashes['Page 1 of 3'],
      hashes['Page 2 of 3'],
      hashes['Page 3 of 3'],
    }, hasLength(3));
    // The page was drawn at the width of the view (physical pixels), within rounding.
    final viewWidth = tester.view.physicalSize.width;
    _log('page width $pageWidth, view width $viewWidth');
    expect((pageWidth! - viewWidth).abs(), lessThanOrEqualTo(2));
    // After `stop` the receiver is waiting again (still running, code visible).
    expect(find.byKey(const Key('pageView')), findsNothing);
    expect(find.text(code), findsOneWidget);
    await tester.runAsync(() => const RustCoreApi().stopReceiver());
  });

  testWidgets('A2: receiver shows an image; closing it goes back to waiting', (
    tester,
  ) async {
    needs();
    final dir = Directory.systemTemp.createTempSync('cometcast_e2e_');
    addTearDown(() => dir.deleteSync(recursive: true));
    final png = File('${dir.path}/picture.png')
      ..writeAsBytesSync(makePng(64, 32, [200, 30, 30]));
    await tester.pumpWidget(_app());
    await tester.pumpAndSettle();
    final code = await _startReceiver(tester);
    final cli = await _cliProcess([
      'send-file',
      code,
      png.path,
      '--server',
      _server,
      '--bind',
      '127.0.0.1:0',
    ]);
    addTearDown(cli.kill);
    final shown = await _waitFor(
      tester,
      () => find.byKey(const Key('contentImage')).evaluate().isNotEmpty,
    );
    expect(shown, isTrue, reason: 'CLI output: ${cli.lines}');
    await tester.tap(find.byKey(const Key('contentCloseButton')));
    await tester.pump();
    expect(find.byKey(const Key('contentImage')), findsNothing);
    expect(find.text(code), findsOneWidget);
    await tester.runAsync(() => const RustCoreApi().stopReceiver());
  });

  testWidgets('A3: a link is a card; nothing opens before the click', (
    tester,
  ) async {
    needs();
    final opened = <Uri>[];
    await tester.pumpWidget(_app(opened: opened));
    await tester.pumpAndSettle();
    final code = await _startReceiver(tester);
    final cli = await _cliProcess([
      'send-link',
      code,
      'https://bücher.example/path?q=1',
      '--server',
      _server,
      '--bind',
      '127.0.0.1:0',
    ]);
    addTearDown(cli.kill);
    final shown = await _waitFor(
      tester,
      () => find.byKey(const Key('linkCard')).evaluate().isNotEmpty,
    );
    expect(shown, isTrue, reason: 'CLI output: ${cli.lines}');
    // The host is shown in its punycode form.
    expect(
      find.text('The sender wants to open xn--bcher-kva.example'),
      findsOneWidget,
    );
    await tester.pump(const Duration(seconds: 2));
    expect(opened, isEmpty, reason: 'must never open without a click');
    await tester.tap(find.byKey(const Key('linkOpenButton')));
    await tester.pump();
    await tester.pump();
    expect(opened, hasLength(1));
    expect(opened.single.host, 'xn--bcher-kva.example');
    expect(opened.single.scheme, 'https');
    await tester.runAsync(() => const RustCoreApi().stopReceiver());
  });

  testWidgets(
    'B: the app sends a PDF to cometcast-cli receive and turns its pages',
    (tester) async {
      needs();
      final cli = await _cliProcess([
        'receive',
        '--server',
        _server,
        '--bind',
        '127.0.0.1:0',
      ]);
      addTearDown(cli.kill);
      String? code;
      final until = DateTime.now().add(const Duration(seconds: 30));
      while (code == null && DateTime.now().isBefore(until)) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 200)),
        );
        for (final l in cli.lines) {
          final m = RegExp(r'Room code: ([A-Z0-9]{6})').firstMatch(l);
          if (m != null) code = m.group(1);
        }
      }
      expect(
        code,
        isNotNull,
        reason: 'CLI receiver printed no room code: ${cli.lines}',
      );

      await tester.pumpWidget(_app(pickPath: _pdf));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Send file or link'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pickFileButton')));
      await tester.pump();
      await tester.enterText(find.byKey(const Key('codeField')), code!);
      await tester.tap(find.byKey(const Key('startButton')));

      final delivered = await _waitFor(
        tester,
        () => find.byKey(const Key('presenter')).evaluate().isNotEmpty,
      );
      final status =
          (tester.widget(find.byKey(const Key('statusText'))) as Text).data;
      expect(
        delivered,
        isTrue,
        reason: 'status: $status; CLI output: ${cli.lines}',
      );
      expect(
        find.text('Page 1 of 3'),
        findsOneWidget,
        reason: 'the worker counts the pages',
      );

      bool printed(String needle) => cli.lines.any((l) => l.contains(needle));
      Future<void> expectLine(String needle) async {
        final ok = await _waitFor(
          tester,
          () => printed(needle),
          timeout: const Duration(seconds: 20),
        );
        expect(ok, isTrue, reason: 'missing "$needle" in ${cli.lines}');
      }

      await expectLine('Content: document name="classroom.pdf" pages=3');
      await expectLine('Content: page=1 ');

      // Presenter keys: next, then go to page 3, then previous.
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await expectLine('Content: page=2 ');
      await tester.enterText(find.byKey(const Key('gotoField')), '3');
      await tester.ensureVisible(find.byKey(const Key('gotoButton')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('gotoButton')));
      await expectLine('Content: page=3 ');
      await tester.ensureVisible(find.byKey(const Key('prevPageButton')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('prevPageButton')));
      final pages = cli.lines
          .where((l) => l.startsWith('Content: page='))
          .map(
            (l) =>
                RegExp(r'page=(\d+) width=(\d+) height=(\d+)').firstMatch(l)!,
          )
          .toList();
      _log(
        'pages rendered by the receiver: ${pages.map((m) => m.group(0)).toList()}',
      );
      final ok = await _waitFor(
        tester,
        () =>
            cli.lines.where((l) => l.startsWith('Content: page=')).length >= 4,
        timeout: const Duration(seconds: 20),
      );
      expect(ok, isTrue, reason: '${cli.lines}');
      final order = cli.lines
          .where((l) => l.startsWith('Content: page='))
          .map((l) => int.parse(RegExp(r'page=(\d+)').firstMatch(l)!.group(1)!))
          .toList();
      expect(order, [1, 2, 3, 2]);

      // Stop showing: the receiver prints "stopped" and the sender ends.
      await tester.ensureVisible(find.byKey(const Key('stopShowingButton')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('stopShowingButton')));
      await expectLine('Content: stopped');
      final stopped = await _waitFor(
        tester,
        () => find
            .text('The content was closed on the receiver.')
            .evaluate()
            .isNotEmpty,
      );
      expect(stopped, isTrue);
      await tester.runAsync(() => const RustCoreApi().stopSender());
    },
  );
}
