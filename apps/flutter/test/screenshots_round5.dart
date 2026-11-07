// SPDX-License-Identifier: GPL-3.0-or-later
// Screenshots of round 5 (spec 15.11): share page, display Settings with the receive rows, idle
// line, request and PIN dialogs, Miracast presenting, TV rows, pair page, TV pages and errors,
// the converted-file hint, address line, app settings, About, licence viewer. Registered by
// screenshots_test.dart (only when COMETCAST_SHOTS is set). DE du and EN; presenter pages Light
// and Dark, display Dark, Light and OLED black.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/miracast.dart';
import 'package:cometcast/core/share_intake.dart';
import 'package:cometcast/src/branding_info.dart';
import 'package:cometcast/src/design/tokens.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'fake_miracast.dart';
import 'nearby_support.dart' show screen, settle;
import 'round5_support.dart';
import 'shot.dart';
import 'ways_support.dart';

class _SlowProbe extends FakeCoreApi {
  final gate = Completer<ProbeResult>();
  @override
  Future<ProbeResult> probeServer(String server) {
    probed.add(server);
    return gate.future;
  }
}

TvUiEvent _cast(String id, String name, List<String> addr) => TvUiEvent.seen(
  id: id,
  name: name,
  kind: TvKind.cast,
  addresses: addr,
  canScreen: true,
  canPhoto: false,
  canVideo: false,
);

TvUiEvent _air(String id, String name, List<String> addr) => TvUiEvent.seen(
  id: id,
  name: name,
  kind: TvKind.airPlay,
  addresses: addr,
  canScreen: true,
  canPhoto: true,
  canVideo: true,
);

const _pdf = SharedItem(
  kind: SharedKind.file,
  path: '/cache/share/a/Quarterly report 2026 final version.pdf',
  name: 'Quarterly report 2026 final version.pdf',
  size: 1887437,
  mime: 'application/pdf',
);
const _docx = SharedItem(
  kind: SharedKind.file,
  path: '/cache/share/b/plan.docx',
  name: 'plan.docx',
  size: 400000,
  mime:
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
);

void registerRound5Shots() {
  const locales = {'en': Locale('en'), 'de': Locale('de')};
  const bright = {'light': Brightness.light, 'dark': Brightness.dark};
  const looks = {
    'dark': Appearance.dark,
    'light': Appearance.light,
    'oled': Appearance.oled,
  };

  // --- presenter pages -----------------------------------------------------------------

  for (final l in locales.entries) {
    for (final b in bright.entries) {
      final p = '${b.key}-${l.key}';
      Future<R5> phone(
        WidgetTester t, {
        SharedItem? item,
        bool office = true,
        bool locked = false,
        FakeCoreApi? fake,
        String? code,
      }) async {
        final intake = FakeShareIntake()..item = item;
        final r = await pumpR5(
          t,
          locale: l.value,
          brightness: b.value,
          office: office,
          locked: locked,
          fake: fake,
          code: code,
          shareIntake: item == null ? null : intake,
        );
        await settle(t);
        return r;
      }

      shotTest('presenter $p: share page', (t) async {
        final r = await phone(t, item: _pdf);
        await shot(t, 'r5-share-notconnected-$p');
        await finish(t, r);
      });

      shotTest('presenter $p: share connected, converting, refusal', (t) async {
        var r = await phone(t, code: 'K7QM2W');
        await _connectChip(t, r);
        r.intake.arrive(_pdf);
        await settle(t);
        await shot(t, 'r5-share-connected-$p');
        r.intake.arrive(_docx);
        await settle(t);
        await tapKey(t, 'share-show');
        r.core.presenterEvents.add(
          const PresenterUiEvent.converting(name: 'plan.docx'),
        );
        await settle(t);
        await shot(t, 'r5-share-converting-$p');
        await finish(t, r);
        r = await phone(
          t,
          item: const SharedItem(
            kind: SharedKind.tooLarge,
            name: 'big.pdf',
            mime: 'application/pdf',
          ),
        );
        await shot(t, 'r5-share-refusal-$p');
        await finish(t, r);
      });

      shotTest('presenter $p: address line', (t) async {
        var r = await phone(t);
        await shot(t, 'r5-address-default-$p');
        await tapKey(t, 'address-change');
        await shot(t, 'r5-address-field-$p');
        await finish(t, r);
        r = await phone(t, locked: true);
        await shot(t, 'r5-address-policy-$p');
        await finish(t, r);
        final slow = _SlowProbe();
        r = await phone(t, fake: slow);
        await tapKey(t, 'address-change');
        await t.enterText(
          find.byKey(const Key('address-field')),
          'cast.example.org',
        );
        await t.tap(find.byKey(const Key('address-use')));
        await t.pump();
        await shot(t, 'r5-address-checking-$p');
        slow.gate.complete(ProbeResult.notCometCast);
        await settle(t);
        await shot(t, 'r5-address-error-$p');
        await finish(t, r);
      });

      shotTest('presenter $p: app settings and About', (t) async {
        var r = await pumpR5(
          t,
          locale: l.value,
          brightness: b.value,
          desktop: true,
          office: true,
          tvs: true,
          size: const Size(1100, 800),
        );
        await shot(t, 'r5-start-desktop-brand-$p');
        await tapKey(t, 'app-settings-open');
        await shot(t, 'r5-appsettings-desktop-$p');
        await tapKey(t, 'about-open');
        await shot(t, 'r5-about-desktop-$p');
        await finish(t, r);
        r = await pumpR5(t, locale: l.value, brightness: b.value, office: true);
        await shot(t, 'r5-start-android-brand-$p');
        await tapKey(t, 'app-settings-open');
        await shot(t, 'r5-appsettings-android-$p');
        await tapKey(t, 'about-open');
        await shot(t, 'r5-about-android-$p');
        await finish(t, r);
        // With the official branding file: publisher, Privacy, Imprint (U11).
        BrandingInfo.current = BrandingInfo.official;
        r = await pumpR5(t, locale: l.value, brightness: b.value, office: true);
        await tapKey(t, 'app-settings-open');
        await tapKey(t, 'about-open');
        await shot(t, 'r5-about-android-branded-$p');
        BrandingInfo.current = const BrandingInfo.generated();
        await finish(t, r);
        r = await pumpR5(t, locale: l.value, brightness: b.value, office: true);
        await tapKey(t, 'app-settings-open');
        await tapKey(t, 'about-open');
        await t.enterText(find.byKey(const Key('licence-search')), 'xyz');
        await settle(t);
        await shot(t, 'r5-about-none-$p');
        await t.enterText(find.byKey(const Key('licence-search')), '');
        await settle(t);
        await t.tap(find.text('tokio 1.40.0'));
        await settle(t);
        await shot(t, 'r5-licence-text-$p');
        await finish(t, r);
        r = await pumpR5(t, locale: l.value, brightness: b.value);
        await shot(t, 'r5-start-ios-about-$p');
        await finish(t, r);
      });

      shotTest('presenter $p: converted file', (t) async {
        final r = await pumpR5(
          t,
          locale: l.value,
          brightness: b.value,
          office: true,
          code: 'K7QM2W',
          pickResults: ['/tmp/report.docx'],
        );
        await _connectChip(t, r);
        await tapKey(t, 'what-file');
        await tapText(t, _filePick(l.key));
        r.core.presenterEvents
          ..add(const PresenterUiEvent.converting(name: 'report.docx'))
          ..add(
            PresenterUiEvent.contentPrepared(
              name: 'report.docx',
              kind: UiContentKind.pdf,
              size: BigInt.from(1000),
              pages: 3,
            ),
          )
          ..add(const PresenterUiEvent.presenting())
          ..add(const PresenterUiEvent.connectionCode(sas: '123 456'));
        await settle(t);
        await shot(t, 'r5-presenting-converted-$p');
        await finish(t, r);
      });

      shotTest('presenter $p: TVs without Comet Cast', (t) async {
        final r = await pumpR5(
          t,
          locale: l.value,
          brightness: b.value,
          desktop: true,
          withNearby: true,
          tvs: true,
          size: const Size(1100, 800),
        );
        await settle(t);
        r.core.search.add(const NearbyUiEvent.searchStarted());
        r.core.search.add(
          NearbyUiEvent.added(screen: screen('a', name: 'Room 204')),
        );
        r.core.search.add(
          NearbyUiEvent.added(
            screen: screen(
              'b',
              name: 'Studio',
              server: 'wss://192.168.1.77:8443',
            ),
          ),
        );
        for (final e in [
          _cast('cast:1', 'Living room TV', ['192.168.1.20']),
          _air('airplay:1', 'Apple TV', ['192.168.1.30']),
          _air('airplay:2', 'Studio box', ['192.168.1.77']),
        ]) {
          r.core.tvWatchers.last.add(e);
        }
        await settle(t);
        await t.pump(const Duration(milliseconds: 600));
        await shot(t, 'r5-tv-rows-$p');
        // Connecting.
        await t.tap(find.byKey(const Key('tv-row-cast:1')));
        await settle(t);
        await shot(t, 'r5-tv-connecting-$p');
        r.core.tvSessions.last.add(
          const PresenterUiEvent.joined(roomName: 'Living room TV'),
        );
        await settle(t);
        await shot(t, 'r5-tv-what-cast-$p');
        await t.tap(find.text(_disconnect(l.key)));
        await settle(t);
        await finish(t, r);
      });

      shotTest('presenter $p: TV AirPlay pages and errors', (t) async {
        var r = await pumpR5(
          t,
          locale: l.value,
          brightness: b.value,
          desktop: true,
          withNearby: true,
          tvs: true,
          size: const Size(1100, 800),
        );
        await settle(t);
        r.core.search.add(const NearbyUiEvent.searchStarted());
        r.core.tvWatchers.last.add(
          _air('airplay:1', 'Apple TV', ['192.168.1.30']),
        );
        await settle(t);
        await t.pump(const Duration(milliseconds: 600));
        await t.tap(find.byKey(const Key('tv-row-airplay:1')));
        await settle(t);
        r.core.tvSessions.last.add(const PresenterUiEvent.pairingRequired());
        await settle(t);
        await shot(t, 'r5-pair-$p');
        r.core.tvPairResult = false;
        await t.enterText(find.byKey(const Key('pair-field')), '1234');
        await tapKey(t, 'pair-go');
        await shot(t, 'r5-pair-wrong-$p');
        r.core.tvPairResult = true;
        r.core.tvSessions.last.add(
          const PresenterUiEvent.joined(roomName: 'Apple TV'),
        );
        await settle(t);
        await shot(t, 'r5-tv-what-airplay-$p');
        await finish(t, r);
        for (final v in [
          (
            'cast',
            _cast('cast:1', 'Living room TV', ['192.168.1.20']),
            'tv_refused',
          ),
          (
            'airplay',
            _air('airplay:1', 'Apple TV', ['192.168.1.30']),
            'tv_refused',
          ),
          (
            'lost',
            _cast('cast:1', 'Living room TV', ['192.168.1.20']),
            'tv_lost',
          ),
        ]) {
          r = await pumpR5(
            t,
            locale: l.value,
            brightness: b.value,
            desktop: true,
            withNearby: true,
            tvs: true,
            size: const Size(1100, 800),
          );
          await settle(t);
          r.core.search.add(const NearbyUiEvent.searchStarted());
          r.core.tvWatchers.last.add(v.$2);
          await settle(t);
          await t.pump(const Duration(milliseconds: 600));
          await t.tap(find.byKey(Key('tv-row-${(v.$2 as TvUiEvent_Seen).id}')));
          await settle(t);
          r.core.tvSessions.last.add(
            PresenterUiEvent.error(code: v.$3, message: ''),
          );
          await settle(t);
          await shot(t, 'r5-error-tv-${v.$1}-$p');
          await finish(t, r);
        }
      });
    }
  }

  // --- display ---------------------------------------------------------------------------

  for (final l in locales.entries) {
    for (final look in looks.entries) {
      final p = '${look.key}-${l.key}';
      Future<DisplayHarness> win(
        WidgetTester t, {
        FakeMiracastCore? m,
        bool airplay = true,
      }) => ways(
        t,
        miracast: m,
        airplay: airplay,
        appearance: look.value,
        locale: l.value,
        server: 'relay.cometcast.de:443',
      );

      shotTest('display $p: Settings, Windows both rows', (t) async {
        final m = FakeMiracastCore();
        final h = await win(t, m: m);
        await openSettings(t);
        await shot(t, 'r5-display-settings-off-$p');
        await toggle(t, 'airplay');
        await toggle(t, 'miracast');
        await rxEvent(t, h, const AirplayUiEvent.advertising());
        await pump(t, 200); // the tracks have reached their colour (N97)
        await shot(t, 'r5-display-settings-on-$p');
        await rxEvent(t, h, const AirplayUiEvent.failed(code: 'no_dnssd'));
        await shot(t, 'r5-display-settings-failed-$p');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
        );
        await pump(t, 100);
        await shot(t, 'r5-display-settings-locked-$p');
      });

      shotTest('display $p: Settings, noHw and Linux', (t) async {
        final m = FakeMiracastCore()
          ..probeResult = const MiracastProbe(
            issue: MiracastIssue.adapterUnsupported,
          );
        await win(t, m: m);
        await openSettings(t);
        await shot(t, 'r5-display-settings-nohw-$p');
        await t.pumpWidget(const SizedBox());
        await win(t);
        await openSettings(t);
        await shot(t, 'r5-display-settings-linux-$p');
      });

      shotTest('display $p: idle line, requests, PIN', (t) async {
        final m = FakeMiracastCore();
        final h = await win(t, m: m);
        await openSettings(t);
        await toggle(t, 'airplay');
        await toggle(t, 'miracast');
        await rxEvent(t, h, const AirplayUiEvent.advertising());
        await pump(t, 300);
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t);
        await shot(t, 'r5-display-idle-both-$p');
        await rxEvent(t, h, const AirplayUiEvent.pin(pin: '4821'));
        await shot(t, 'r5-display-pin-airplay-$p');
        await rxEvent(t, h, const AirplayUiEvent.pinDone());
        m.request(7, 'Lena’s PC', pin: '12345678');
        await pump(t, 300);
        await shot(t, 'r5-display-pin-miracast-$p');
        m.connect('Lena’s PC');
        // A picture with something in it (BGRA).
        final px = Uint8List(32 * 18 * 4);
        for (var i = 0; i < 32 * 18; i++) {
          px.setRange(i * 4, i * 4 + 4, [
            200 - (i % 32) * 4,
            120,
            40 + (i ~/ 32) * 8,
            255,
          ]);
        }
        m.frames.add(MiracastFrame(width: 32, height: 18, seq: 1, bgra: px));
        await pump(t, 300);
        await t.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 100)),
        );
        await pump(t, 300);
        await t.tapAt(const Offset(500, 500));
        // The name tag fades in 400 ms: the shot is taken at least 500 ms after the input
        // (N101). One frame first, so that the fade has started (a single long pump would draw
        // only its first frame).
        await pump(t, 16);
        await pump(t, 700);
        await shot(t, 'r5-display-miracast-presenting-$p');
      });

      shotTest('display $p: request variants and idle variants', (t) async {
        var h = await win(t, airplay: true);
        await openSettings(t);
        await toggle(t, 'airplay');
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t);
        await rxEvent(t, h, const AirplayUiEvent.advertising());
        await shot(t, 'r5-display-idle-airplay-$p');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
        );
        await pump(t, 100);
        await rxEvent(t, h, const AirplayUiEvent.request());
        await shot(t, 'r5-display-request-airplay-$p');
        await t.pumpWidget(const SizedBox());
        final m = FakeMiracastCore();
        h = await win(t, m: m, airplay: false);
        await openSettings(t);
        await toggle(t, 'miracast');
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t, 300);
        await shot(t, 'r5-display-idle-miracast-$p');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
        );
        await pump(t, 300);
        m.request(8, 'Lena’s PC', pin: '4455');
        await pump(t, 300);
        await shot(t, 'r5-display-request-miracast-$p');
      });

      shotTest('display $p: idle line offline', (t) async {
        final core = FakeCoreApi()..airplayAvailable = true;
        final h = await pumpDisplay(
          t,
          tv: false,
          core: core,
          offerWays: true,
          appearance: look.value,
          locale: l.value,
          start: false,
          server: 'relay.cometcast.de:443',
        );
        await t.pump(const Duration(milliseconds: 300));
        await t.tap(find.byKey(const Key('settings-open')));
        await pump(t);
        await toggle(t, 'airplay');
        await rxEvent(t, h, const AirplayUiEvent.advertising());
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t);
        await t.pump(const Duration(seconds: 9));
        await shot(t, 'r5-display-idle-offline-$p');
      });

      shotTest('display $p: About and licence viewer', (t) async {
        await win(t);
        await openSettings(t);
        await t.tap(find.byKey(const Key('settings-about')));
        await pump(t);
        await shot(t, 'r5-display-about-$p');
        await t.tap(find.byKey(const Key('about-licences')));
        await pump(t, 100);
        await pump(t, 100);
        await shot(t, 'r5-display-licences-$p');
        await t.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await pump(t, 100);
        await shot(t, 'r5-display-licences-pane-$p');
      });

      shotTest('display $p: About with the official branding', (t) async {
        BrandingInfo.current = BrandingInfo.official;
        try {
          await win(t);
          await openSettings(t);
          await t.tap(find.byKey(const Key('settings-about')));
          await pump(t);
          await shot(t, 'r5-display-about-branded-$p');
        } finally {
          BrandingInfo.current = const BrandingInfo.generated();
        }
      });

      shotTest('display $p: TV About and licence viewer', (t) async {
        await ways(
          t,
          tv: true,
          airplay: false,
          appearance: look.value,
          locale: l.value,
        );
        await openSettings(t);
        await shot(t, 'r5-display-tv-settings-$p');
        await t.tap(find.byKey(const Key('settings-about')));
        await pump(t);
        await shot(t, 'r5-display-tv-about-$p');
        await t.tap(find.byKey(const Key('about-licences')));
        await pump(t, 100);
        await pump(t, 100);
        await t.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await pump(t, 100);
        await shot(t, 'r5-display-tv-licences-pane-$p');
      });
    }
  }
}

Future<void> _connectChip(WidgetTester t, R5 r) async {
  final connect = find.byWidgetPredicate(
    (w) => w is Text && (w.data == 'Connect' || w.data == 'Verbinden'),
  );
  await t.tap(connect.first);
  await settle(t);
  r.core.presenterEvents.add(
    const PresenterUiEvent.joined(roomName: 'Meeting room Elbe'),
  );
  await settle(t);
}

String _filePick(String lang) =>
    lang == 'de' ? 'Datei auswählen' : 'Choose file';
String _disconnect(String lang) => lang == 'de' ? 'Trennen' : 'Disconnect';
