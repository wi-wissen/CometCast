// SPDX-License-Identifier: GPL-3.0-or-later
// AirPlay and Miracast on a desktop display, About on the display, the join address (spec 15.4,
// 15.9, N85; fix list F4, F8, F11).

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/miracast.dart';
import 'package:cometcast/src/design/tokens.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'fake_miracast.dart';

import 'package:cometcast/licences.dart';

import 'round5_support.dart' show fakeLicences;
import 'solid_frame.dart';

Future<void> pump(WidgetTester t, [int ms = 50]) =>
    t.pump(Duration(milliseconds: ms));

Future<DisplayHarness> ways(
  WidgetTester t, {
  bool airplay = true,
  FakeMiracastCore? miracast,
  Appearance appearance = Appearance.dark,
  Locale locale = const Locale('en'),
  bool tv = false,
  String server = 'cast.example.org',
}) async {
  final core = FakeCoreApi()..airplayAvailable = airplay;
  final h = await pumpDisplay(
    t,
    tv: tv,
    core: core,
    miracast: miracast,
    offerWays: true,
    appearance: appearance,
    locale: locale,
    server: server,
    licenceLoader: () async => fakeLicences(),
  );
  await pump(t, 100);
  return h;
}

Future<void> openSettings(WidgetTester t) async {
  await t.tap(find.byKey(const Key('settings-open')));
  await pump(t);
}

Future<void> toggle(WidgetTester t, String way) async {
  await t.tap(find.byKey(Key('settings-way-$way')));
  await pump(t, 100);
}

String? focusLabel() => FocusManager.instance.primaryFocus?.debugLabel;

Future<void> key(WidgetTester t, LogicalKeyboardKey k) async {
  await t.sendKeyEvent(k);
  await pump(t);
}

Future<void> rxEvent(WidgetTester t, DisplayHarness h, AirplayUiEvent e) async {
  h.core.airplayControllers.last.add(e);
  await pump(t);
}

void main() {
  group('Settings: Without the app (15.4)', () {
    testWidgets(
      'Linux with UxPlay: the header has About and Close, one AirPlay row',
      (t) async {
        await ways(t);
        await openSettings(t);
        expect(find.byKey(const Key('settings-about')), findsOneWidget);
        expect(find.text('About'), findsOneWidget);
        expect(find.byKey(const Key('settings-close')), findsOneWidget);
        expect(find.text('WITHOUT THE APP'), findsOneWidget);
        expect(find.byKey(const Key('settings-way-airplay')), findsOneWidget);
        expect(find.byKey(const Key('settings-way-miracast')), findsNothing);
        expect(find.text('BETA'), findsOneWidget);
        expect(
          find.text('iPhone, iPad and Mac show their screen here.'),
          findsOneWidget,
        );
      },
    );

    testWidgets('no way at all: the section is not shown', (t) async {
      await ways(t, airplay: false);
      await openSettings(t);
      expect(find.byKey(const Key('settings-receive')), findsNothing);
      expect(find.text('WITHOUT THE APP'), findsNothing);
    });

    testWidgets('Windows: AirPlay first, Miracast second', (t) async {
      await ways(t, miracast: FakeMiracastCore());
      await openSettings(t);
      final a = t.getTopLeft(find.byKey(const Key('settings-way-airplay'))).dy;
      final m = t.getTopLeft(find.byKey(const Key('settings-way-miracast'))).dy;
      expect(a, lessThan(m));
      expect(find.text('BETA'), findsNWidgets(2));
    });

    testWidgets(
      'default off; on starts the receiver with the room name, off stops it',
      (t) async {
        final h = await ways(t);
        expect(h.core.airplayStarts, isEmpty);
        await openSettings(t);
        await toggle(t, 'airplay');
        expect(h.settings.airplayReceive, isTrue);
        expect(h.core.airplayStarts.single.name, 'Meeting room Elbe');
        expect(h.core.airplayStarts.single.askFirst, isFalse);
        await toggle(t, 'airplay');
        expect(h.settings.airplayReceive, isFalse);
        expect(h.core.airplayCalls, contains('stop'));
      },
    );

    testWidgets('a switch row is one semantics node: title, Beta, toggled', (
      t,
    ) async {
      final handle = t.ensureSemantics();
      await ways(t);
      await openSettings(t);
      final node = t.getSemantics(
        find.byKey(const Key('settings-way-airplay')),
      );
      expect(node.label, 'AirPlay, Beta');
      expect(node.hint, 'iPhone, iPad and Mac show their screen here.');
      handle.dispose();
    });

    testWidgets(
      'the rows are their own focus rows: [About, Close] · modes · looks · AirPlay · Miracast',
      (t) async {
        await ways(t, miracast: FakeMiracastCore());
        await openSettings(t);
        // Initial focus: the selected join mode (Open).
        await key(t, LogicalKeyboardKey.arrowDown); // looks
        await key(t, LogicalKeyboardKey.arrowDown); // AirPlay
        expect(focusLabel(), 'display:3:0');
        await key(t, LogicalKeyboardKey.arrowDown); // Miracast
        expect(focusLabel(), 'display:4:0');
        await key(t, LogicalKeyboardKey.arrowDown); // no wrapping
        expect(focusLabel(), 'display:4:0');
        // OK toggles the focused row.
        await key(t, LogicalKeyboardKey.enter);
        await pump(t, 100);
        for (var i = 0; i < 4; i++) {
          await key(t, LogicalKeyboardKey.arrowUp);
        }
        expect(focusLabel(), 'about'); // the header row: About first
        await key(t, LogicalKeyboardKey.arrowRight);
        expect(focusLabel(), 'display:0:1');
      },
    );

    testWidgets(
      'no hardware: the line, the switch off and disabled, not focusable',
      (t) async {
        final m = FakeMiracastCore()
          ..probeResult = const MiracastProbe(
            issue: MiracastIssue.adapterUnsupported,
          );
        final h = await ways(t, miracast: m, airplay: false);
        await openSettings(t);
        expect(
          find.text('This Wi-Fi adapter cannot receive Miracast.'),
          findsOneWidget,
        );
        await toggle(t, 'miracast');
        expect(h.settings.miracastReceive, isFalse);
        expect(m.calls, isNot(contains('start')));
      },
    );

    testWidgets('missing feature and policy have their lines', (t) async {
      for (final (issue, line) in [
        (
          MiracastIssue.featureMissing,
          'Windows needs the optional feature “Wireless Display” for this.',
        ),
        (
          MiracastIssue.disabledByPolicy,
          'Switched off by the administrator of this device.',
        ),
      ]) {
        final m = FakeMiracastCore()..probeResult = MiracastProbe(issue: issue);
        await ways(t, miracast: m, airplay: false);
        await openSettings(t);
        expect(find.text(line), findsOneWidget);
        await t.pumpWidget(const SizedBox());
      }
    });

    testWidgets('failed: the switch stays on; off and on again retries', (
      t,
    ) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      await rxEvent(t, h, const AirplayUiEvent.failed(code: 'no_dnssd'));
      expect(
        find.text(
          'Could not start. Switch it off and on again, or restart the app.',
        ),
        findsOneWidget,
      );
      expect(h.settings.airplayReceive, isTrue);
      await toggle(t, 'airplay'); // off
      await toggle(t, 'airplay'); // on: a new attempt
      expect(h.core.airplayStarts.length, 2);
    });

    testWidgets(
      'Locked: the line "Paused", the switch as set; the core is told',
      (t) async {
        final h = await ways(t);
        await openSettings(t);
        await toggle(t, 'airplay');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
        );
        await pump(t, 100);
        expect(find.text('Paused while Locked is on.'), findsOneWidget);
        expect(h.settings.airplayReceive, isTrue);
        expect(h.core.airplayCalls, contains('locked:true'));
      },
    );

    testWidgets('the mode follows: Approval on asks first', (t) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      h.core.receiverEvents.add(
        const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
      );
      await pump(t, 100);
      expect(h.core.airplayCalls, contains('askFirst:true'));
    });
  });

  group('the idle line (15.4)', () {
    testWidgets('AirPlay only, Miracast only, both; hidden in Locked', (
      t,
    ) async {
      final m = FakeMiracastCore();
      final h = await ways(t, miracast: m);
      h.settings.airplayReceive = true;
      h.settings.miracastReceive = true;
      await openSettings(t);
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      // Re-enter the room so that the receivers start from the settings.
      await t.pumpWidget(const SizedBox());
    });

    testWidgets('AirPlay running: the first footer line', (t) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      await rxEvent(t, h, const AirplayUiEvent.advertising());
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      expect(find.byKey(const Key('idle-also-via')), findsOneWidget);
      expect(find.text('Also via AirPlay'), findsOneWidget);
      // idle.appLine stays below it.
      expect(
        t.getTopLeft(find.byKey(const Key('idle-also-via'))).dy,
        lessThan(t.getTopLeft(find.byKey(const Key('idle-foot'))).dy),
      );
      // Locked: hidden.
      h.core.receiverEvents.add(
        const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
      );
      await pump(t, 100);
      expect(find.byKey(const Key('idle-also-via')), findsNothing);
    });

    testWidgets('both ways: "Also via AirPlay and Miracast"', (t) async {
      final m = FakeMiracastCore();
      final h = await ways(t, miracast: m);
      await openSettings(t);
      await toggle(t, 'airplay');
      await toggle(t, 'miracast');
      await rxEvent(t, h, const AirplayUiEvent.advertising());
      await pump(t, 300);
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      expect(find.text('Also via AirPlay and Miracast'), findsOneWidget);
      expect(m.startedPairing, MiracastPairing.pinRequired);
      expect(m.startedName, 'Meeting room Elbe');
    });

    testWidgets('Miracast only', (t) async {
      final m = FakeMiracastCore();
      final h = await ways(t, miracast: m, airplay: false);
      await openSettings(t);
      await toggle(t, 'miracast');
      await pump(t, 300);
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      expect(find.text('Also via Miracast'), findsOneWidget);
      expect(h.settings.miracastReceive, isTrue);
    });

    testWidgets('not when no receiver runs', (t) async {
      await ways(t);
      expect(find.byKey(const Key('idle-also-via')), findsNothing);
    });
  });

  group('joining by AirPlay (15.4)', () {
    Future<DisplayHarness> running(
      WidgetTester t, {
      UiJoinMode mode = UiJoinMode.open,
    }) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      await rxEvent(t, h, const AirplayUiEvent.advertising());
      if (mode != UiJoinMode.open) {
        h.core.receiverEvents.add(ReceiverUiEvent.joinModeChanged(mode: mode));
        await pump(t, 100);
      }
      return h;
    }

    testWidgets('Open: the PIN dialog at once, no question; Cancel refuses', (
      t,
    ) async {
      final h = await running(t);
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '4821'));
      expect(find.text('AIRPLAY CODE'), findsOneWidget);
      expect(
        find.text('Enter this code on the device that wants to mirror.'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('pin-value')), findsOneWidget);
      expect(find.text('4821'), findsOneWidget);
      expect(find.byKey(const Key('request-yes')), findsNothing);
      // Cancel is guarded for 1 s.
      await t.tap(find.byKey(const Key('pin-cancel')));
      await pump(t);
      expect(h.core.airplayCalls, isNot(contains('answer:false')));
      await pump(t, 1100);
      await t.tap(find.byKey(const Key('pin-cancel')));
      await pump(t);
      expect(h.core.airplayCalls, contains('answer:false'));
      expect(find.byKey(const Key('pin-value')), findsNothing);
    });

    testWidgets('Back is Cancel', (t) async {
      final h = await running(t);
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '4821'));
      await t.binding.handlePopRoute();
      await pump(t);
      expect(h.core.airplayCalls, contains('answer:false'));
      expect(find.byKey(const Key('pin-value')), findsNothing);
    });

    testWidgets('the dialog closes by itself when the mirror starts', (
      t,
    ) async {
      final h = await running(t);
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '4821'));
      await rxEvent(t, h, const AirplayUiEvent.mirroring(active: true));
      expect(find.byKey(const Key('pin-value')), findsNothing);
      // The display counts as Presenting for the wake lock; Comet Cast draws nothing over it.
      expect(h.wake.calls.last, isTrue);
    });

    testWidgets('PinDone and 30 s close it', (t) async {
      final h = await running(t);
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '4821'));
      await rxEvent(t, h, const AirplayUiEvent.pinDone());
      expect(find.byKey(const Key('pin-value')), findsNothing);
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '1111'));
      await pump(t, 31000);
      expect(find.byKey(const Key('pin-value')), findsNothing);
    });

    testWidgets('Approval on: the request first (Yes, No), then the PIN', (
      t,
    ) async {
      final h = await running(t, mode: UiJoinMode.confirm);
      await rxEvent(t, h, const AirplayUiEvent.request());
      expect(find.text('An AirPlay device'), findsOneWidget);
      expect(find.text('wants to mirror via AirPlay'), findsOneWidget);
      // No check-number box, no "No and block".
      expect(find.byKey(const Key('request-check')), findsNothing);
      expect(find.byKey(const Key('request-block')), findsNothing);
      await pump(t, 1100);
      await t.tap(find.byKey(const Key('request-yes')));
      await pump(t);
      expect(h.core.airplayCalls, contains('answer:true'));
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '9090'));
      expect(find.text('9090'), findsOneWidget);
    });

    testWidgets('Approval on: No refuses; Back does nothing', (t) async {
      final h = await running(t, mode: UiJoinMode.confirm);
      await rxEvent(t, h, const AirplayUiEvent.request());
      await t.binding.handlePopRoute();
      await pump(t);
      expect(find.text('An AirPlay device'), findsOneWidget);
      await pump(t, 1100);
      await t.tap(find.byKey(const Key('request-no')));
      await pump(t);
      expect(h.core.airplayCalls, contains('answer:false'));
      expect(find.text('An AirPlay device'), findsNothing);
    });

    testWidgets('the request closes Settings like a join request', (t) async {
      final h = await running(t, mode: UiJoinMode.confirm);
      await openSettings(t);
      await rxEvent(t, h, const AirplayUiEvent.request());
      expect(find.byKey(const Key('settings-way-airplay')), findsNothing);
      expect(find.text('An AirPlay device'), findsOneWidget);
    });
  });

  group('Comet Cast first (N88)', () {
    testWidgets('while a presenter presents a pairing is refused without UI', (
      t,
    ) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
      );
      await deliverFrame(t, h.core, solidFrame(8, 4, [200, 30, 30]));
      expect(h.core.airplayCalls, contains('refusing:true'));
      await rxEvent(t, h, const AirplayUiEvent.request());
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '1234'));
      expect(find.byKey(const Key('pin-value')), findsNothing);
      expect(find.text('An AirPlay device'), findsNothing);
      expect(h.core.airplayCalls, contains('answer:false'));
    });

    testWidgets(
      'a presentation that starts ends the AirPlay mirror with the toast',
      (t) async {
        final h = await ways(t);
        await openSettings(t);
        await toggle(t, 'airplay');
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t);
        await rxEvent(t, h, const AirplayUiEvent.mirroring(active: true));
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
        );
        await deliverFrame(t, h.core, solidFrame(8, 4, [200, 30, 30]));
        expect(h.core.airplayCalls, contains('end'));
        expect(find.text('AirPlay ended'), findsOneWidget);
      },
    );
  });

  group('Miracast (15.4)', () {
    Future<(DisplayHarness, FakeMiracastCore)> running(
      WidgetTester t, {
      UiJoinMode mode = UiJoinMode.open,
    }) async {
      final m = FakeMiracastCore();
      final h = await ways(t, miracast: m, airplay: false);
      await openSettings(t);
      await toggle(t, 'miracast');
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t, 300);
      if (mode != UiJoinMode.open) {
        h.core.receiverEvents.add(ReceiverUiEvent.joinModeChanged(mode: mode));
        await pump(t, 300);
      }
      return (h, m);
    }

    testWidgets(
      'Open: consent answered at once, the PIN dialog with groups of 4',
      (t) async {
        final (_, m) = await running(t);
        m.request(7, 'Lena’s PC', pin: '12345678');
        await pump(t, 300);
        expect(m.responses, [(7, true)]);
        expect(find.text('MIRACAST CODE'), findsOneWidget);
        expect(find.text('Enter this code on “Lena’s PC”.'), findsOneWidget);
        expect(find.text('1234 5678'), findsOneWidget);
      },
    );

    testWidgets('Approval on: the request with the quoted name, Yes then PIN', (
      t,
    ) async {
      final (_, m) = await running(t, mode: UiJoinMode.confirm);
      m.request(8, 'Lena’s PC', pin: '4455');
      await pump(t, 300);
      expect(find.text('“Lena’s PC”'), findsOneWidget);
      expect(find.text('wants to mirror via Miracast'), findsOneWidget);
      expect(m.responses, isEmpty);
      await pump(t, 1100);
      await t.tap(find.byKey(const Key('request-yes')));
      await pump(t, 300);
      expect(m.responses, [(8, true)]);
      expect(find.text('4455'), findsOneWidget);
    });

    testWidgets('Approval on: No refuses', (t) async {
      final (_, m) = await running(t, mode: UiJoinMode.confirm);
      m.request(9, 'PC', pin: '4455');
      await pump(t, 300);
      await pump(t, 1100);
      await t.tap(find.byKey(const Key('request-no')));
      await pump(t, 300);
      expect(m.responses, [(9, false)]);
    });

    testWidgets('an unanswered request leaves with toast.expired', (t) async {
      final (_, m) = await running(t, mode: UiJoinMode.confirm);
      m.request(9, 'Lena’s PC', pin: '4455');
      await pump(t, 300);
      m.requestGone();
      await pump(t, 300);
      expect(find.text('Lena’s PC'), findsNothing);
      expect(find.textContaining('Request from'), findsOneWidget);
    });

    testWidgets('Locked: the receiver stops', (t) async {
      final (_, m) = await running(t, mode: UiJoinMode.locked);
      expect(m.calls, contains('stop'));
    });

    testWidgets(
      'connected: frames in the stage, name tag with the way, End ends both',
      (t) async {
        final (h, m) = await running(t);
        m.request(7, 'Lena’s PC', pin: '1234');
        await pump(t, 300);
        m.connect('Lena’s PC');
        m.addFrame(1);
        await pump(t, 300);
        await t.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 100)),
        );
        await pump(t, 300);
        expect(find.byKey(const Key('pin-value')), findsNothing);
        // The controls fade after the first input; show them.
        await t.tapAt(const Offset(500, 500));
        await pump(t, 300);
        expect(find.byKey(const Key('byline')), findsOneWidget);
        expect(find.text('“Lena’s PC” is showing'), findsOneWidget);
        expect(find.text('via Miracast'), findsOneWidget);
        expect(find.byKey(const Key('sas-line')), findsNothing);
        await t.tap(find.byKey(const Key('end')));
        await pump(t, 300);
        expect(m.calls, contains('disconnect'));
        expect(h.core.endRoomCalls, 1);
      },
    );

    testWidgets('the device stops: Idle and the toast with its name', (
      t,
    ) async {
      final (_, m) = await running(t);
      m.request(7, 'Lena’s PC', pin: '1234');
      await pump(t, 300);
      m.connect('Lena’s PC');
      await pump(t, 300);
      m.listening();
      await pump(t, 300);
      expect(find.byKey(const Key('room-name')), findsOneWidget);
      expect(find.textContaining('Lena’s PC'), findsOneWidget);
    });

    testWidgets('a Comet Cast presentation ends the mirror with the toast', (
      t,
    ) async {
      final (h, m) = await running(t);
      m.request(7, 'Lena’s PC', pin: '1234');
      await pump(t, 300);
      m.connect('Lena’s PC');
      await pump(t, 300);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Max', handover: false),
      );
      await deliverFrame(t, h.core, solidFrame(8, 4, [200, 30, 30]));
      expect(m.calls, contains('disconnect'));
      expect(find.text('Miracast ended'), findsOneWidget);
    });
  });

  group('the join address (N85, F11)', () {
    testWidgets('the built-in relay: cometcast.de/app and the QR url', (
      t,
    ) async {
      await ways(t, airplay: false, server: 'relay.cometcast.de:443');
      expect(find.text('cometcast.de/app'), findsWidgets);
      expect(_qr('https://cometcast.de/app/K7QM2W'), findsOneWidget);
    });

    testWidgets('a self-hosted server is unchanged', (t) async {
      await ways(t, airplay: false, server: 'cast.example.org');
      expect(find.text('cast.example.org'), findsWidgets);
      expect(_qr('https://cast.example.org/K7QM2W'), findsOneWidget);
    });
  });

  group('About on the display (15.9)', () {
    testWidgets('the dialog: lines and buttons [Licences, Close]', (t) async {
      await ways(t);
      await openSettings(t);
      await t.tap(find.byKey(const Key('settings-about')));
      await pump(t);
      expect(find.text('ABOUT COMET CAST'), findsOneWidget);
      expect(find.byKey(const Key('about-version')), findsOneWidget);
      expect(find.text('Free software under the GPL-3.0'), findsOneWidget);
      expect(find.byKey(const Key('about-source')), findsOneWidget);
      expect(find.byKey(const Key('about-licences')), findsOneWidget);
      expect(find.byKey(const Key('about-close')), findsOneWidget);
      // Back = Close, back to Settings with the focus on About.
      await t.binding.handlePopRoute();
      await pump(t, 100);
      await pump(t, 100);
      expect(find.byKey(const Key('settings-about')), findsOneWidget);
      expect(find.byKey(const Key('about-close')), findsNothing);
      expect(focusLabel(), 'about');
    });

    testWidgets(
      'the viewer: list and pane, D-pad moves, Right to the pane, Left back',
      (t) async {
        await ways(t, tv: false);
        await openSettings(t);
        await t.tap(find.byKey(const Key('settings-about')));
        await pump(t);
        await t.tap(find.byKey(const Key('about-licences')));
        await pump(t, 100);
        await pump(t, 100);
        expect(find.text('LICENCES'), findsWidgets);
        // The pane shows the first entry.
        expect(find.text('OpenH264 text'), findsOneWidget);
        // Search exists on computers.
        expect(find.byKey(const Key('licence-search')), findsOneWidget);
        // Down in the list: the pane shows the focused entry.
        await t.tap(find.text('Atkinson Hyperlegible'));
        await pump(t, 100);
        expect(find.text('Atkinson text'), findsOneWidget);
        await key(t, LogicalKeyboardKey.arrowRight);
        expect(focusLabel(), 'licence:pane');
        await key(t, LogicalKeyboardKey.arrowLeft);
        expect(focusLabel(), startsWith('f:'));
        // Back closes the viewer, to the About dialog.
        await t.binding.handlePopRoute();
        await pump(t, 100);
        expect(find.byKey(const Key('about-licences')), findsOneWidget);
      },
    );

    testWidgets('search filters the list: tokio found, nothing found', (
      t,
    ) async {
      await ways(t);
      await openSettings(t);
      await t.tap(find.byKey(const Key('settings-about')));
      await pump(t);
      await t.tap(find.byKey(const Key('about-licences')));
      await pump(t, 100);
      await t.enterText(find.byKey(const Key('licence-search')), 'tokio');
      await pump(t, 100);
      expect(find.text('tokio 1.40.0'), findsOneWidget);
      expect(find.text('serde 1.0.0'), findsNothing);
      await t.enterText(find.byKey(const Key('licence-search')), 'xyz');
      await pump(t, 100);
      expect(find.text('Nothing found.'), findsOneWidget);
    });

    testWidgets(
      'a TV has no search; Up and Down in the pane scroll by half a pane',
      (t) async {
        // A TV never offers the ways, but it has About in its Settings.
        final core = FakeCoreApi();
        final long = List.generate(300, (i) => 'line $i').join('\n\n');
        await pumpDisplay(
          t,
          core: core,
          offerWays: false,
          licenceLoader: () async => LicenceSetOf(long),
        );
        await pump(t, 100);
        await openSettings(t);
        await t.tap(find.byKey(const Key('settings-about')));
        await pump(t);
        await t.tap(find.byKey(const Key('about-licences')));
        await pump(t, 100);
        expect(find.byKey(const Key('licence-search')), findsNothing);
        await key(t, LogicalKeyboardKey.arrowRight);
        expect(focusLabel(), 'licence:pane');
        final scroll = t.widget<ListView>(
          find.descendant(
            of: find.byKey(const Key('licence-pane')),
            matching: find.byType(ListView),
          ),
        );
        final before = scroll.controller!.offset;
        await key(t, LogicalKeyboardKey.arrowDown);
        final half = scroll.controller!.position.viewportDimension / 2;
        expect(scroll.controller!.offset - before, closeTo(half, 1));
        await key(t, LogicalKeyboardKey.arrowUp);
        expect(scroll.controller!.offset, closeTo(before, 1));
      },
    );
  });
}

LicenceSet LicenceSetOf(String text) => LicenceSet(
  featured: [
    LicenceEntry(name: 'Long', licence: 'x', text: text, featured: true),
  ],
  all: const [],
);

Finder _qr(String url) => find.byWidgetPredicate(
  (w) => w is Semantics && w.properties.label == 'QR: $url',
);
