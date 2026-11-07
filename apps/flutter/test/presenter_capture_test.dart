// SPDX-License-Identifier: GPL-3.0-or-later
// Screen sharing on the presenter root: the Android capture feeding the one join, the desktop
// capture request, the codec page before the picker, the iOS broadcast that joins on its own, the
// scanner, links that arrive while the app runs and the first launch.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/core/android_capture.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/ios_broadcast.dart';
import 'package:cometcast/core/launch_intake.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/presenter/pages.dart' show PresenterDevice;
import 'package:cometcast/src/presenter/presenter_root.dart';

import 'android_capture_test.dart' show FakeAndroidCapture;
import 'display_support.dart';
import 'fake_core.dart';
import 'fake_miracast.dart';

import 'package:cometcast/main.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'ios_broadcast_test.dart' show FakeIosBroadcast;

Future<void> settle(WidgetTester t) async {
  for (var i = 0; i < 3; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
}

class Rig {
  Rig(this.core, this.settings, this.links);
  final FakeCoreApi core;
  final AppSettings settings;
  final LinkHandler links;
}

Future<Rig> pump(
  WidgetTester t, {
  String? code = 'K7QM2W',
  AndroidCapture? android,
  IosBroadcast? ios,
  bool desktop = false,
  PresenterDevice? device,
  ScanQr? scanQr,
  CodecController? codec,
  Stream<String>? linkStream,
  bool firstLaunch = false,
  FakeCoreApi? core,
}) async {
  await t.runAsync(loadAppFonts);
  final c = core ?? FakeCoreApi();
  final settings = AppSettings.fromDefaults(
    c.serverDefaults(),
    serverAddress: 'relay.cometcast.de',
    store: MemorySettingsStore(),
  );
  final links = LinkHandler(core: c, settings: settings);
  t.view.physicalSize = const Size(400, 800);
  t.view.devicePixelRatio = 1;
  addTearDown(t.view.reset);
  await t.pumpWidget(
    MaterialApp(
      locale: const Locale('en'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: PresenterRoot(
        core: c,
        settings: settings,
        pickFile: (_) async => null,
        androidCapture: android,
        iosBroadcast: ios,
        desktop: desktop,
        device: device,
        scanQr: scanQr,
        codec: codec,
        links: links,
        linkStream: linkStream,
        firstLaunch: firstLaunch,
        initialCode: code,
      ),
    ),
  );
  await settle(t);
  return Rig(c, settings, links);
}

Future<void> toWhat(WidgetTester t, Rig r) async {
  await t.tap(find.text('Connect'));
  await settle(t);
  r.core.presenterEvents.add(
    const PresenterUiEvent.joined(roomName: 'Meeting room Elbe'),
  );
  await settle(t);
}

void main() {
  group('Android capture inside the one join', () {
    testWidgets('consent, then the encoded video is presented', (t) async {
      final cap = FakeAndroidCapture();
      final r = await pump(t, android: cap);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      expect(cap.startCalls, 1);
      // The notification texts come from the language of the app.
      expect(cap.notification!.title, 'Sharing your screen');
      // Nothing is presented before the person agrees in the system dialog.
      expect(r.core.presentEncodedCalls, 0);
      cap.consent.complete(CaptureStartResult.started);
      await settle(t);
      expect(r.core.presentEncodedCalls, 1);
      expect(r.core.startPresenterCalls, 1);
      r.core.presenterEvents.add(const PresenterUiEvent.presenting());
      await settle(t);
      expect(find.text('You are sharing your screen'), findsOneWidget);
      // Stop ends the presentation and the capture service, the join stays.
      await t.tap(find.text('Stop showing'));
      await settle(t);
      expect(r.core.stopPresentingCalls, 1);
      expect(cap.stopCalls, greaterThanOrEqualTo(1));
      expect(r.core.stopPresenterCalls, 0);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('a refused consent dialog: back to What, no error, no frames', (
      t,
    ) async {
      final cap = FakeAndroidCapture();
      final r = await pump(t, android: cap);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      cap.consent.complete(CaptureStartResult.denied);
      await settle(t);
      expect(r.core.presentEncodedCalls, 0);
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(find.text('Something went wrong'), findsNothing);
    });

    testWidgets('a native failure while starting is the other page', (t) async {
      final cap = FakeAndroidCapture()..startError = StateError('busy');
      final r = await pump(t, android: cap);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      expect(find.text('Something went wrong'), findsOneWidget);
      // The join was not given up; Try again goes back to What to show.
      expect(r.core.stopPresenterCalls, 0);
      await t.tap(find.text('Try again'));
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('the system ending the projection is the same as Stop', (
      t,
    ) async {
      final cap = FakeAndroidCapture();
      final r = await pump(t, android: cap);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      cap.consent.complete(CaptureStartResult.started);
      await settle(t);
      r.core.presenterEvents.add(const PresenterUiEvent.presenting());
      await settle(t);
      cap.controller.add(const CaptureStopped('user'));
      await settle(t);
      expect(r.core.stopPresentingCalls, 1);
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(find.text('Something went wrong'), findsNothing);
    });

    testWidgets('Disconnect and the display leaving stop the capture service', (
      t,
    ) async {
      final cap = FakeAndroidCapture();
      final r = await pump(t, android: cap);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      cap.consent.complete(CaptureStartResult.started);
      await settle(t);
      r.core.presenterEvents.add(const PresenterUiEvent.presenting());
      await settle(t);
      r.core.presenterEvents.add(
        const PresenterUiEvent.ended(code: 'display_left'),
      );
      await settle(t);
      expect(cap.stopCalls, greaterThanOrEqualTo(1));
      expect(find.text('Presentation ended'), findsOneWidget);
    });
  });

  group('desktop capture', () {
    testWidgets('Screen asks the core for the capture inside the join', (
      t,
    ) async {
      final r = await pump(t, desktop: true);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      expect(r.core.presentedScreens.length, 1);
      expect(r.core.presentedScreens.single, isNotNull);
      expect(r.core.startPresenterCalls, 1);
    });

    testWidgets('without the H.264 encoder the codec page comes first', (
      t,
    ) async {
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.missing()
        ..codecInfoResult = const UiCodecInfo(
          fileName: 'libopenh264.so',
          version: '2.6.0',
          host: 'ciscobinary.openh264.org',
          downloadBytes: 1000,
          autoDownload: false,
          adminDir: null,
        );
      final ctl = CodecController(
        core: core,
        settings: AppSettings.fromDefaults(core.serverDefaults()),
      );
      addTearDown(ctl.dispose);
      final r = await pump(t, desktop: true, codec: ctl, core: core);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      expect(find.text('Video codec needed'), findsOneWidget);
      expect(r.core.presentedScreens, isEmpty);
      // Back goes to What to show (system Back too).
      await t.tap(find.byKey(const Key('codec-back')));
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('capture_unavailable is the other page and keeps the join', (
      t,
    ) async {
      final r = await pump(t, desktop: true);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      r.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'capture_unavailable', message: 'x'),
      );
      await settle(t);
      expect(find.text('Something went wrong'), findsOneWidget);
      expect(r.core.stopPresenterCalls, 0);
    });
  });

  group('iOS broadcast', () {
    testWidgets(
      'the extension joins on its own: the approval is handed over first',
      (t) async {
        final ios = FakeIosBroadcast();
        final r = await pump(t, ios: ios);
        // The person has a name: the display binds the token to it.
        await t.enterText(find.byKey(const Key('name-field')), 'Lena');
        await toWhat(t, r);
        await t.tap(find.byKey(const Key('what-screen')));
        await settle(t);
        // 1. The display is asked to hold a single-use token (128 bits, base64url).
        expect(r.core.handoverRequests, hasLength(1));
        final token = r.core.handoverRequests.single;
        expect(token, matches(RegExp(r'^[A-Za-z0-9_-]{22}$')));
        // 2. The extension gets the room, the name and the same token.
        expect(r.core.stopPresenterCalls, greaterThanOrEqualTo(1));
        expect(ios.prepared!.code, 'K7QM2W');
        expect(ios.prepared!.relay, 'relay.cometcast.de');
        expect(ios.prepared!.name, 'Lena');
        expect(ios.prepared!.handover, token);
        expect(ios.showPickerCalls, 1);
        ios.controller.add(
          const IosBroadcastStatus(
            state: IosBroadcastState.streaming,
            roomName: 'Room 204',
            sas: '171 852',
          ),
        );
        await settle(t);
        // 3. The Presenting page shows the extension's room and security code.
        expect(find.text('You are sharing your screen'), findsOneWidget);
        expect(find.textContaining('Room 204'), findsWidgets);
        expect(find.textContaining('171 852'), findsWidgets);
        // 4. Stop writes the stop file; the extension's last report carries the way back.
        await t.tap(find.text('Stop showing'));
        await settle(t);
        expect(ios.stopCalls, greaterThanOrEqualTo(1));
        ios.controller.add(
          const IosBroadcastStatus(
            state: IosBroadcastState.ended,
            nextHandover: 'wayBackToken0123456789',
          ),
        );
        await settle(t);
        // 5. The app joins again with that token and lands on "What to show".
        expect(r.core.startPresenterCalls, 2);
        expect(r.core.presenterHandover, 'wayBackToken0123456789');
        expect(r.core.presenterName, 'Lena');
        r.core.presenterEvents.add(
          const PresenterUiEvent.joined(roomName: 'Room 204'),
        );
        await settle(t);
        expect(find.text('What would you like to show?'), findsOneWidget);
      },
    );

    testWidgets(
      'a display that does not answer: no token, ordinary join, plain rejoin',
      (t) async {
        final ios = FakeIosBroadcast();
        final core = FakeCoreApi()..handoverAnswer = false;
        final r = await pump(t, ios: ios, core: core);
        await toWhat(t, r);
        await t.tap(find.byKey(const Key('what-screen')));
        await settle(t);
        expect(ios.prepared!.handover, isNull);
        ios.controller.add(
          const IosBroadcastStatus(state: IosBroadcastState.streaming),
        );
        await settle(t);
        await t.tap(find.text('Stop showing'));
        await settle(t);
        // No token of the way back (an old display): Connect again with the code.
        ios.controller.add(
          const IosBroadcastStatus(state: IosBroadcastState.ended),
        );
        await settle(t);
        expect(r.core.startPresenterCalls, 2);
        expect(r.core.presenterHandover, isNull);
        expect(r.core.presenterCode, 'K7QM2W');
      },
    );

    testWidgets(
      'a join the display did not accept: the waiting page with the extension\'s number',
      (t) async {
        final ios = FakeIosBroadcast();
        final r = await pump(t, ios: ios);
        await toWhat(t, r);
        await t.tap(find.byKey(const Key('what-screen')));
        await settle(t);
        ios.controller.add(
          const IosBroadcastStatus(
            state: IosBroadcastState.pending,
            check: 42,
            timeoutSecs: 60,
          ),
        );
        await settle(t);
        expect(find.text('Waiting for approval'), findsOneWidget);
        expect(find.text('42'), findsOneWidget);
        // The host denies: the normal page of a denied join.
        ios.controller.add(
          const IosBroadcastStatus(
            state: IosBroadcastState.failed,
            error: IosBroadcastError.joinDenied,
          ),
        );
        await settle(t);
        expect(find.text('Waiting for approval'), findsNothing);
        expect(find.text('Welcome to Comet Cast'), findsNothing);
      },
    );

    testWidgets('a room that is gone is the code page, by code', (t) async {
      final ios = FakeIosBroadcast();
      final r = await pump(t, ios: ios);
      await toWhat(t, r);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      ios.controller.add(
        const IosBroadcastStatus(
          state: IosBroadcastState.failed,
          error: IosBroadcastError.roomNotFound,
        ),
      );
      await settle(t);
      expect(find.text('This code does not exist right now'), findsOneWidget);
    });

    test('the extension errors map to the structured codes', () {
      expect(iosErrorCode(IosBroadcastError.peerLeft), 'display_left');
      expect(
        iosErrorCode(IosBroadcastError.serverUnreachable),
        'connection_lost',
      );
      expect(iosErrorCode(IosBroadcastError.memory), 'capture_failed');
      expect(iosErrorCode(null), 'capture_failed');
    });
  });

  group('the scanner (5.3)', () {
    Future<Rig> withScan(WidgetTester t, String? Function() result) => pump(
      t,
      code: null,
      scanQr: () async => result(),
      device: PresenterDevice.phone,
    );

    testWidgets('a code of ours becomes the scanned chip', (t) async {
      final r = await withScan(t, () => 'https://relay.cometcast.de/K7QM2W');
      await t.tap(find.byKey(const Key('scan-button')));
      await settle(t);
      expect(find.text('K7Q M2W'), findsOneWidget);
      expect(find.text('scanned'), findsOneWidget);
      await t.tap(find.text('Connect'));
      await settle(t);
      expect(r.core.presenterCode, 'K7QM2W');
    });

    testWidgets('a foreign QR code: the line, nothing else changes', (t) async {
      await withScan(t, () => 'https://example.org/nothing');
      await t.tap(find.byKey(const Key('scan-button')));
      await settle(t);
      expect(
        find.text('This QR code is not from a Comet Cast screen.'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('code-chip')), findsNothing);
      expect(find.byKey(const Key('code-field')), findsOneWidget);
    });

    testWidgets('the line goes with the next scan', (t) async {
      var next = 'garbage';
      await withScan(t, () => next);
      await t.tap(find.byKey(const Key('scan-button')));
      await settle(t);
      expect(find.byKey(const Key('scan-wrong')), findsOneWidget);
      next = 'https://relay.cometcast.de/K7QM2W';
      await t.tap(find.byKey(const Key('scan-button')));
      await settle(t);
      expect(find.byKey(const Key('scan-wrong')), findsNothing);
    });

    testWidgets('cancelling the scanner changes nothing', (t) async {
      await withScan(t, () => null);
      await t.tap(find.byKey(const Key('scan-button')));
      await settle(t);
      expect(find.byKey(const Key('scan-wrong')), findsNothing);
      expect(find.byKey(const Key('code-chip')), findsNothing);
    });

    testWidgets('an unknown server: the relay page first, nothing connects', (
      t,
    ) async {
      final r = await withScan(t, () => 'https://cast.example.org/K7QM2W');
      await t.tap(find.byKey(const Key('scan-button')));
      await settle(t);
      expect(find.byKey(const Key('relay-host')), findsOneWidget);
      expect(r.core.startPresenterCalls, 0);
      await t.tap(find.byKey(const Key('relay-use')));
      await settle(t);
      expect(r.settings.confirmedServers, {'cast.example.org'});
      expect(find.text('K7Q M2W'), findsOneWidget);
    });

    testWidgets('computers have no scan button', (t) async {
      await pump(t, code: null, scanQr: () async => null, desktop: true);
      expect(find.byKey(const Key('scan-button')), findsNothing);
    });

    testWidgets('Scan again on the code error page only with a scanner', (
      t,
    ) async {
      await pump(
        t,
        code: null,
        scanQr: () async => null,
        device: PresenterDevice.phone,
      );
      await t.enterText(find.byKey(const Key('code-field')), 'OOO000');
      await t.tap(find.text('Connect'));
      await settle(t);
      expect(find.byKey(const Key('error-rescan')), findsOneWidget);
    });
  });

  group('links while the app runs', () {
    testWidgets('a link with a known server becomes the chip', (t) async {
      final c = StreamController<String>();
      addTearDown(c.close);
      await pump(t, code: null, linkStream: c.stream);
      c.add('https://relay.cometcast.de/K7QM2W');
      await settle(t);
      expect(find.text('K7Q M2W'), findsOneWidget);
      expect(find.text('taken from your link'), findsOneWidget);
    });

    testWidgets(
      'an unknown server opens the relay page; Cancel keeps the chip',
      (t) async {
        final c = StreamController<String>();
        addTearDown(c.close);
        final r = await pump(t, code: null, linkStream: c.stream);
        c.add('https://relay.cometcast.de/K7QM2W?server=cast.example.org');
        await settle(t);
        expect(find.byKey(const Key('relay-host')), findsOneWidget);
        await t.tap(find.byKey(const Key('relay-cancel')));
        await settle(t);
        expect(find.text('K7Q M2W'), findsOneWidget);
        expect(r.settings.serverAddress, 'relay.cometcast.de');
        expect(r.settings.confirmedServers, isEmpty);
      },
    );

    testWidgets('something that is no link of ours changes nothing', (t) async {
      final c = StreamController<String>();
      addTearDown(c.close);
      await pump(t, code: null, linkStream: c.stream);
      c.add('https://example.org/abc');
      await settle(t);
      expect(find.byKey(const Key('code-chip')), findsNothing);
    });

    testWidgets('a link while connected is ignored', (t) async {
      final c = StreamController<String>();
      addTearDown(c.close);
      final r = await pump(t, linkStream: c.stream);
      await toWhat(t, r);
      c.add('https://relay.cometcast.de/ABCDEF');
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });
  });

  group('the first launch', () {
    testWidgets('scan again is said once, without a code', (t) async {
      await pump(
        t,
        code: null,
        scanQr: () async => null,
        firstLaunch: true,
        device: PresenterDevice.phone,
      );
      expect(find.byKey(const Key('scan-again-body')), findsOneWidget);
    });

    testWidgets('not when the app brought a code', (t) async {
      await pump(
        t,
        scanQr: () async => null,
        firstLaunch: true,
        device: PresenterDevice.phone,
      );
      expect(find.byKey(const Key('scan-again-body')), findsNothing);
    });
  });

  group('Windows', () {
    testWidgets('the link opens the Miracast page and Back returns', (t) async {
      await t.runAsync(loadAppFonts);
      final core = FakeCoreApi();
      t.view.physicalSize = const Size(1024, 900);
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.reset);
      await t.pumpWidget(
        CometCastApp(
          locale: const Locale('en'),
          core: core,
          settings: AppSettings.fromDefaults(core.serverDefaults()),
          displayStore: MemoryDisplayStore(),
          forceDisplayCapable: false,
          miracast: FakeMiracastCore(),
          pickFile: (_) async => null,
          openUrl: (_) async => true,
        ),
      );
      await settle(t);
      // The old small "Miracast" button at the bottom right is gone.
      expect(find.byKey(const Key('miracastLink')), findsNothing);
      await t.tap(find.byKey(const Key('miracast-link')));
      await settle(t);
      expect(find.byKey(const Key('miracastStartButton')), findsOneWidget);
      await t.tap(find.byKey(const Key('miracastBack')));
      await t.pumpAndSettle();
      expect(find.byKey(const Key('miracastStartButton')), findsNothing);
      expect(find.byKey(const Key('miracast-link')), findsOneWidget);
    });

    testWidgets('the Miracast text link is the last item on the first page', (
      t,
    ) async {
      await pumpWindows(t);
      expect(find.byKey(const Key('miracast-link')), findsOneWidget);
    });
  });
}

Future<void> pumpWindows(WidgetTester t) async {
  await t.runAsync(loadAppFonts);
  final core = FakeCoreApi();
  final settings = AppSettings.fromDefaults(core.serverDefaults());
  t.view.physicalSize = const Size(1024, 800);
  t.view.devicePixelRatio = 1;
  addTearDown(t.view.reset);
  await t.pumpWidget(
    MaterialApp(
      locale: const Locale('en'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: PresenterRoot(
        core: core,
        settings: settings,
        pickFile: (_) async => null,
        desktop: true,
        onBeDisplay: () {},
        onMiracast: () {},
      ),
    ),
  );
  await settle(t);
}
