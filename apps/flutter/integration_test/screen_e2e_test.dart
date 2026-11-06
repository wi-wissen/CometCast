// SPDX-License-Identifier: GPL-3.0-or-later
// End-to-end test of the screen path with the REAL Rust core, the real UI, a real
// signalling server and a real X11 screen (Xvfb). Run it with
// `integration_test/run_screen_e2e.sh` (see apps/flutter/README.md): the script starts the
// server and an Xvfb display for the screen that is shared, and runs this file under a second
// Xvfb display.
//
// Scenario A: the app is the RECEIVER. `cometcast-cli send <CODE> --screen --display :99`
//   sends a red xterm window; the test reads the pixels of the drawn picture
//   (RenderRepaintBoundary.toImage) and checks that red dominates, then that a blue window
//   turns the picture blue, then measures display rate and CPU while the screen flips between
//   red and blue.
// Scenario B: the app is the SENDER. "Share this screen" with the display :99, and
//   `cometcast-cli receive --decode` prints the mean colour of what it decoded; the test checks
//   that it is red.
//
// Needed --dart-define values:
//   COMETCAST_TEST_SERVER    host:port of cometcast-server
//   COMETCAST_CLI            path of the cometcast-cli binary
//   COMETCAST_CAPTURE_DISPLAY X11 display with the screen to share, for example :99

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/rust_loader.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';

const _server = String.fromEnvironment('COMETCAST_TEST_SERVER');
const _cli = String.fromEnvironment('COMETCAST_CLI');
const _display = String.fromEnvironment('COMETCAST_CAPTURE_DISPLAY');

void _log(String s) {
  // ignore: avoid_print
  print('SCREEN-E2E $s');
}

Future<bool> _waitFor(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 40),
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 200));
    if (done()) return true;
  }
  return done();
}

/// A window that fills the Xvfb screen: xterm with the given background (and by default the
/// same foreground, so no text is visible).
Future<Process> _xterm(
  String color, {
  String? fg,
  List<String> command = const [],
}) async {
  final p = await Process.start('xterm', [
    '-display',
    _display,
    '-bg',
    color,
    '-fg',
    fg ?? color,
    '-geometry',
    '200x80+0+0',
    '+sb',
    ...command,
  ]);
  unawaited(p.stdout.drain<void>());
  unawaited(p.stderr.drain<void>());
  return p;
}

/// Share of pixels (0..1) near [rgb] in the RGBA bytes, with [tol] per channel.
double _share(List<int> rgba, List<int> rgb, {int tol = 60}) {
  var hits = 0;
  final n = rgba.length ~/ 4;
  for (var i = 0; i < n; i++) {
    final o = i * 4;
    if ((rgba[o] - rgb[0]).abs() <= tol &&
        (rgba[o + 1] - rgb[1]).abs() <= tol &&
        (rgba[o + 2] - rgb[2]).abs() <= tol) {
      hits++;
    }
  }
  return n == 0 ? 0 : hits / n;
}

/// Reads back the pixels the app has drawn in the picture view.
Future<({int w, int h, List<int> rgba})> _readPicture(
  WidgetTester tester,
) async {
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(const Key('videoView')),
  );
  return (await tester.runAsync(() async {
    final img = await boundary.toImage();
    final data = await img.toByteData(format: ui.ImageByteFormat.rawRgba);
    final out = (w: img.width, h: img.height, rgba: data!.buffer.asUint8List());
    img.dispose();
    return out;
  }))!;
}

/// CPU seconds used by this process so far (utime + stime from /proc/self/stat).
double _cpuSeconds() {
  final stat = File('/proc/self/stat').readAsStringSync();
  // The command name in parentheses may contain spaces: split after the last ')'.
  final rest = stat.substring(stat.lastIndexOf(')') + 2).split(' ');
  // After ')' the fields start at number 3, so utime (14) is rest[11] and stime (15) rest[12].
  return (int.parse(rest[11]) + int.parse(rest[12])) / 100.0;
}

/// CPU seconds per thread name (from /proc/self/task/*/{comm,stat}); same-named threads add up.
Map<String, double> _threadCpu() {
  final out = <String, double>{};
  for (final t in Directory('/proc/self/task').listSync()) {
    try {
      final name = File('${t.path}/comm').readAsStringSync().trim();
      final stat = File('${t.path}/stat').readAsStringSync();
      final rest = stat.substring(stat.lastIndexOf(')') + 2).split(' ');
      out[name] =
          (out[name] ?? 0) +
          (int.parse(rest[11]) + int.parse(rest[12])) / 100.0;
    } on FileSystemException {
      // The thread ended meanwhile.
    }
  }
  return out;
}

String _statsText(WidgetTester tester) {
  final text = tester.widget<Text>(
    find.descendant(
      of: find.byKey(const Key('videoStats')),
      matching: find.byType(Text),
    ),
  );
  return (text.data ?? '').replaceAll('\n', ' | ');
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await initRustCore();
  });

  testWidgets('A: receiver shows the shared screen, in its colour', (
    tester,
  ) async {
    expect(
      _server,
      isNotEmpty,
      reason: 'pass --dart-define=COMETCAST_TEST_SERVER',
    );
    expect(_cli, isNotEmpty, reason: 'pass --dart-define=COMETCAST_CLI');
    expect(
      _display,
      isNotEmpty,
      reason: 'pass --dart-define=COMETCAST_CAPTURE_DISPLAY',
    );

    final red = await _xterm('red');
    addTearDown(red.kill);

    await tester.pumpWidget(
      CometCastApp(
        locale: const Locale('en'),
        core: const RustCoreApi(),
        settings: AppSettings(serverAddress: _server, udpBind: '127.0.0.1:0'),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Receive on this device'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('startButton')));
    final gotCode = await _waitFor(
      tester,
      () =>
          (tester.widget(
            find.byKey(const Key('roomCode')),
          ) as SelectableText).data !=
          '······',
    );
    expect(gotCode, isTrue, reason: 'no room code');
    final code = (tester.widget(
      find.byKey(const Key('roomCode')),
    ) as SelectableText).data!;
    _log('room code $code');

    final sender = await Process.start(_cli, [
      'send',
      code,
      '--server',
      _server,
      '--bind',
      '127.0.0.1:0',
      '--screen',
      '--display',
      _display,
      '--seconds',
      '120',
    ]);
    addTearDown(sender.kill);
    final senderLines = <String>[];
    sender.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen(senderLines.add);
    unawaited(sender.stderr.drain<void>());

    // The picture view replaces the form once the first decoded picture arrives.
    final started = DateTime.now();
    final shown = await _waitFor(
      tester,
      () => find.byKey(const Key('videoView')).evaluate().isNotEmpty,
    );
    expect(shown, isTrue, reason: 'no picture appeared');
    _log(
      'first picture after ${DateTime.now().difference(started).inMilliseconds} ms '
      '(includes starting the CLI)',
    );

    // Red dominates the drawn picture.
    var pic = await _readPicture(tester);
    var red100 = _share(pic.rgba, [255, 0, 0]);
    final redOk = await _waitFor(
      tester,
      () => false,
      timeout: const Duration(seconds: 2),
    );
    expect(redOk, isFalse); // just lets a few more pictures arrive
    pic = await _readPicture(tester);
    red100 = _share(pic.rgba, [255, 0, 0]);
    _log(
      'picture ${pic.w}x${pic.h}, share of red pixels: ${red100.toStringAsFixed(3)}',
    );
    expect(red100, greaterThan(0.8), reason: 'the drawn picture is not red');

    // Baseline: CPU of the app while the screen is static (nothing new to show).
    final staticCpu0 = _cpuSeconds();
    await _waitFor(tester, () => false, timeout: const Duration(seconds: 5));
    _log(
      'static screen: app process CPU '
      '${((_cpuSeconds() - staticCpu0) / 5 * 100).toStringAsFixed(0)} % of one core',
    );

    // A blue window on top turns the picture blue.
    final blue = await _xterm('blue');
    addTearDown(blue.kill);
    var blueShare = 0.0;
    final turnedBlue = await _waitFor(
      tester,
      () => false,
      timeout: const Duration(seconds: 1),
    );
    expect(turnedBlue, isFalse);
    final endBlue = DateTime.now().add(const Duration(seconds: 20));
    while (DateTime.now().isBefore(endBlue) && blueShare < 0.8) {
      await tester.pump(const Duration(milliseconds: 300));
      pic = await _readPicture(tester);
      blueShare = _share(pic.rgba, [0, 0, 255]);
    }
    _log(
      'after the blue window: share of blue pixels ${blueShare.toStringAsFixed(3)}',
    );
    expect(
      blueShare,
      greaterThan(0.8),
      reason: 'the picture did not turn blue',
    );
    blue.kill();

    // Measurement: the screen flips between two colours (xterm reverse video, about 20 times
    // a second), the sender looks at it 15 times a second.
    final flip = await _xterm(
      'red',
      fg: 'blue',
      command: [
        '-e',
        'sh',
        '-c',
        r'while :; do printf "\033[?5h"; sleep 0.05; printf "\033[?5l"; sleep 0.05; done',
      ],
    );
    addTearDown(flip.kill);
    await tester.tap(find.byKey(const Key('statsButton')));
    await tester.pump();
    await _waitFor(tester, () => false, timeout: const Duration(seconds: 3));
    final cpu0 = _cpuSeconds();
    final threads0 = _threadCpu();
    final wall0 = DateTime.now();
    await _waitFor(tester, () => false, timeout: const Duration(seconds: 10));
    final cpu1 = _cpuSeconds();
    final threads1 = _threadCpu();
    final wallS = DateTime.now().difference(wall0).inMilliseconds / 1000;
    final stats = _statsText(tester);
    _log(
      'measurement over ${wallS.toStringAsFixed(1)} s: '
      'app process CPU ${((cpu1 - cpu0) / wallS * 100).toStringAsFixed(0)} % of one core',
    );
    final perThread = [
      for (final e in threads1.entries)
        (e.key, e.value - (threads0[e.key] ?? 0)),
    ]..sort((a, b) => b.$2.compareTo(a.$2));
    _log(
      'busiest threads (% of one core): ${perThread.take(6).map((e) => '${e.$1}=${(e.$2 / wallS * 100).toStringAsFixed(0)}').join(', ')}',
    );
    _log('overlay: $stats');
    _log(
      'sender says: ${senderLines.reversed.take(2).toList().reversed.join(' // ')}',
    );
    final fps = RegExp(r'Display rate: ([0-9.]+) fps').firstMatch(stats);
    expect(fps, isNotNull);
    expect(
      double.parse(fps!.group(1)!),
      greaterThan(3),
      reason: 'display rate too low',
    );
    flip.kill();

    // The sender leaves: the picture goes and the status says so.
    sender.kill();
    final left = await _waitFor(
      tester,
      () => find.byKey(const Key('videoView')).evaluate().isEmpty,
      timeout: const Duration(seconds: 60),
    );
    _log('sender killed; picture removed from the view: $left');
    await tester.runAsync(() => const RustCoreApi().stopReceiver());
  });

  testWidgets('B: sender shares the screen to cometcast-cli receive --decode', (
    tester,
  ) async {
    expect(_server, isNotEmpty);
    expect(_cli, isNotEmpty);
    expect(_display, isNotEmpty);

    final red = await _xterm('red');
    addTearDown(red.kill);

    final receiver = await Process.start(_cli, [
      'receive',
      '--server',
      _server,
      '--bind',
      '127.0.0.1:0',
      '--decode',
    ]);
    addTearDown(receiver.kill);
    final lines = <String>[];
    receiver.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen(lines.add);
    unawaited(receiver.stderr.drain<void>());
    String? code;
    final end = DateTime.now().add(const Duration(seconds: 30));
    while (code == null && DateTime.now().isBefore(end)) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 200)),
      );
      for (final l in lines) {
        final m = RegExp(r'Room code: ([A-Z0-9]{6})').firstMatch(l);
        if (m != null) code = m.group(1);
      }
    }
    expect(
      code,
      isNotNull,
      reason: 'CLI receiver printed no room code: $lines',
    );

    await tester.pumpWidget(
      CometCastApp(
        locale: const Locale('en'),
        core: const RustCoreApi(),
        settings: AppSettings(
          serverAddress: _server,
          udpBind: '127.0.0.1:0',
          captureDisplay: _display,
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Send screen'));
    await tester.pumpAndSettle();
    // The Rust capability check answers; the switch becomes usable.
    final usable = await _waitFor(
      tester,
      () =>
          tester
              .widget<SwitchListTile>(
                find.byKey(const Key('shareScreenSwitch')),
              )
              .onChanged !=
          null,
      timeout: const Duration(seconds: 10),
    );
    expect(
      usable,
      isTrue,
      reason: 'capture check did not report X11 as supported',
    );
    await tester.tap(find.byKey(const Key('shareScreenSwitch')));
    await tester.pump();
    final dropdown = find.byKey(const Key('monitorDropdown'));
    expect(dropdown, findsOneWidget);
    _log(
      'monitor list shows: ${find.textContaining('Whole desktop').evaluate().length} '
      'entry "Whole desktop (...)"',
    );
    await tester.enterText(find.byKey(const Key('codeField')), code!);
    await tester.tap(find.byKey(const Key('startButton')));

    final streaming = await _waitFor(
      tester,
      () => (tester.widget(find.byKey(const Key('statusText'))) as Text).data!
          .startsWith('Sharing the screen'),
    );
    expect(
      streaming,
      isTrue,
      reason:
          'status: ${(tester.widget(find.byKey(const Key('statusText'))) as Text).data}',
    );

    // The CLI decodes what arrives and prints the mean colour.
    List<int>? mean;
    final until = DateTime.now().add(const Duration(seconds: 30));
    while (mean == null && DateTime.now().isBefore(until)) {
      await tester.pump(const Duration(milliseconds: 300));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 100)),
      );
      for (final l in lines.reversed) {
        final m = RegExp(
          r'decoded units=\d+ pictures=(\d+) last=(\d+)x(\d+) mean_rgb=\((\d+),(\d+),(\d+)\)',
        ).firstMatch(l);
        if (m != null && int.parse(m.group(1)!) > 0) {
          mean = [
            int.parse(m.group(4)!),
            int.parse(m.group(5)!),
            int.parse(m.group(6)!),
          ];
          _log('CLI decoded: $l');
          break;
        }
      }
    }
    expect(
      mean,
      isNotNull,
      reason: 'the CLI decoded no picture: ${lines.take(20)}',
    );
    expect(mean![0], greaterThan(200));
    expect(mean[1], lessThan(60));
    expect(mean[2], lessThan(60));

    await tester.tap(find.byKey(const Key('stopButton')));
    await tester.pump();
    await tester.runAsync(() => const RustCoreApi().stopSender());
  });
}
