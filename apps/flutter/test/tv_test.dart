// SPDX-License-Identifier: GPL-3.0-or-later
// TVs without Comet Cast in the desktop presenter app (spec 15.5, fix list F10).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/presenter/file_picker.dart';

import 'nearby_support.dart' show screen, settle;
import 'round5_support.dart';

TvUiEvent cast(
  String id, {
  String name = 'Living room TV',
  bool screen = true,
  List<String> addresses = const ['192.168.1.20'],
}) => TvUiEvent.seen(
  id: id,
  name: name,
  kind: TvKind.cast,
  addresses: addresses,
  canScreen: screen,
  canPhoto: false,
  canVideo: false,
);

TvUiEvent airplay(
  String id, {
  String name = 'Apple TV',
  bool photo = true,
  bool video = true,
  bool screen = true,
  List<String> addresses = const ['192.168.1.30'],
}) => TvUiEvent.seen(
  id: id,
  name: name,
  kind: TvKind.airPlay,
  addresses: addresses,
  canScreen: screen,
  canPhoto: photo,
  canVideo: video,
);

Future<R5> desktop(
  WidgetTester t, {
  bool tvs = true,
  List<String>? picks,
}) async {
  final r = await pumpR5(
    t,
    desktop: true,
    withNearby: true,
    tvs: tvs,
    size: const Size(1100, 800),
    pickResults: picks,
  );
  await settle(t);
  return r;
}

Future<void> see(WidgetTester t, R5 r, List<TvUiEvent> events) async {
  for (final e in events) {
    r.core.tvWatchers.last.add(e);
  }
  await settle(t);
  await t.pump(const Duration(milliseconds: 600)); // past the tap guard
}

Future<void> connectTv(WidgetTester t, R5 r, String id, String name) async {
  await t.tap(find.byKey(Key('tv-row-$id')));
  await settle(t);
  r.core.tvSessions.last.add(PresenterUiEvent.joined(roomName: name));
  await settle(t);
}

void main() {
  testWidgets('switch off: nothing below exists and no search runs', (t) async {
    final r = await desktop(t, tvs: false);
    expect(r.core.tvWatchers, isEmpty);
    expect(find.text('Without Comet Cast'), findsNothing);
    await finish(t, r);
  });

  testWidgets('switch on and none found: silent, no label, no placeholder', (
    t,
  ) async {
    final r = await desktop(t);
    expect(r.core.tvWatchers, hasLength(1));
    expect(find.text('Without Comet Cast'), findsNothing);
    expect(find.byKey(const Key('tv-section')), findsNothing);
    await finish(t, r);
  });

  testWidgets(
    'rows under their own label, empty right slot, Beta in the subtitle',
    (t) async {
      final r = await desktop(t);
      await see(t, r, [cast('cast:1'), airplay('airplay:1')]);
      expect(find.text('Without Comet Cast'), findsOneWidget);
      expect(find.text('“Living room TV”'), findsOneWidget);
      expect(find.text('Google Cast · Beta'), findsOneWidget);
      expect(find.text('AirPlay · Beta'), findsOneWidget);
      expect(find.byKey(const Key('tv-row-cast:1')), findsOneWidget);
      await finish(t, r);
    },
  );

  testWidgets(
    'hidden: audio-only Cast, AirPlay with neither video nor photos, shared addresses',
    (t) async {
      final r = await desktop(t);
      // A Comet Cast row in the nearby list that names a host of a TV (the hook of N84).
      r.core.search.add(const NearbyUiEvent.searchStarted());
      r.core.search.add(
        NearbyUiEvent.added(
          screen: screen('a', server: 'wss://192.168.1.77:8443'),
        ),
      );
      await see(t, r, [
        cast('cast:1', screen: false),
        airplay('airplay:1', photo: false, video: false),
        airplay('airplay:2', name: 'Same box', addresses: ['192.168.1.77']),
        cast('cast:2', name: 'Visible'),
      ]);
      expect(find.byKey(const Key('tv-row-cast:1')), findsNothing);
      expect(find.byKey(const Key('tv-row-airplay:1')), findsNothing);
      expect(find.byKey(const Key('tv-row-airplay:2')), findsNothing);
      expect(find.byKey(const Key('tv-row-cast:2')), findsOneWidget);
      await finish(t, r);
    },
  );

  testWidgets('a row that is gone leaves at once', (t) async {
    final r = await desktop(t);
    await see(t, r, [cast('cast:1')]);
    r.core.tvWatchers.last.add(const TvUiEvent.gone(id: 'cast:1'));
    await settle(t);
    expect(find.byKey(const Key('tv-row-cast:1')), findsNothing);
    await finish(t, r);
  });

  testWidgets('at most 32 rows', (t) async {
    final r = await desktop(t);
    await see(t, r, [
      for (var i = 0; i < 40; i++) cast('cast:$i', name: 'TV $i'),
    ]);
    expect(r.tv!.rows.length, 32);
    await finish(t, r);
  });

  group('tapping a Google Cast row', () {
    testWidgets(
      'spinner, Connecting, the others disabled, then What to show: Screen only',
      (t) async {
        final r = await desktop(t);
        await see(t, r, [cast('cast:1'), airplay('airplay:1')]);
        await t.tap(find.byKey(const Key('tv-row-cast:1')));
        await settle(t);
        expect(r.core.tvStarts, ['cast:1']);
        expect(find.text('Connecting …'), findsOneWidget);
        // Another row is disabled.
        await t.tap(
          find.byKey(const Key('tv-row-airplay:1')),
          warnIfMissed: false,
        );
        await settle(t);
        expect(r.core.tvStarts, ['cast:1']);
        r.core.tvSessions.last.add(
          const PresenterUiEvent.joined(roomName: 'Living room TV'),
        );
        await settle(t);
        expect(find.text('Connected to “Living room TV”'), findsOneWidget);
        expect(find.text('Whole screen, live'), findsOneWidget);
        expect(find.text('Show a photo'), findsNothing);
        expect(find.text('Show a video link'), findsNothing);
        expect(find.text('Disconnect'), findsOneWidget);
        await finish(t, r);
      },
    );

    testWidgets('Screen presents; Presenting has no security code', (t) async {
      final r = await desktop(t);
      await see(t, r, [cast('cast:1')]);
      await connectTv(t, r, 'cast:1', 'Living room TV');
      // The codec page comes first on a desktop without the encoder; the fake core has it.
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      r.core.tvSessions.last.add(const PresenterUiEvent.presenting());
      await settle(t);
      expect(find.byKey(const Key('presenting-where')), findsOneWidget);
      expect(find.byKey(const Key('presenting-sas')), findsNothing);
      await finish(t, r);
    });
  });

  group('an AirPlay row', () {
    testWidgets(
      'pairing: the pair page, wrong code inline, then What to show',
      (t) async {
        final r = await desktop(t);
        await see(t, r, [airplay('airplay:1')]);
        await t.tap(find.byKey(const Key('tv-row-airplay:1')));
        await settle(t);
        r.core.tvSessions.last.add(const PresenterUiEvent.pairingRequired());
        await settle(t);
        expect(find.text('Pair with “Apple TV”'), findsOneWidget);
        expect(find.text('Code from the TV'), findsOneWidget);
        expect(find.text('Pair'), findsOneWidget);
        // Only digits, at most 4.
        await t.enterText(find.byKey(const Key('pair-field')), 'a1b2c3d4e5');
        await t.pump();
        expect(
          (t.widget(
            find.byKey(const Key('pair-field')),
          ) as EditableText).controller.text,
          '1234',
        );
        r.core.tvPairResult = false;
        await tapKey(t, 'pair-go');
        expect(r.core.tvPins, ['1234']);
        expect(
          find.text('That code was not right. The TV now shows a new one.'),
          findsOneWidget,
        );
        expect(
          (t.widget(
            find.byKey(const Key('pair-field')),
          ) as EditableText).controller.text,
          '',
        );
        r.core.tvPairResult = true;
        await t.enterText(find.byKey(const Key('pair-field')), '5678');
        await tapKey(t, 'pair-go');
        r.core.tvSessions.last.add(
          const PresenterUiEvent.joined(roomName: 'Apple TV'),
        );
        await settle(t);
        expect(find.text('Whole screen, a few seconds behind'), findsOneWidget);
        expect(find.text('Show a photo'), findsOneWidget);
        expect(find.text('Show a video link'), findsOneWidget);
        await finish(t, r);
      },
    );

    testWidgets('Back on the pair page returns to the start page', (t) async {
      final r = await desktop(t);
      await see(t, r, [airplay('airplay:1')]);
      await t.tap(find.byKey(const Key('tv-row-airplay:1')));
      await settle(t);
      r.core.tvSessions.last.add(const PresenterUiEvent.pairingRequired());
      await settle(t);
      await tapKey(t, 'pair-back');
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      expect(r.core.stopPresenterCalls, 1);
      await finish(t, r);
    });

    testWidgets('Photo: the TV body and a picker for PNG, JPEG, WebP', (
      t,
    ) async {
      final r = await desktop(t, picks: ['/tmp/pic.jpg']);
      await see(t, r, [airplay('airplay:1')]);
      await connectTv(t, r, 'airplay:1', 'Apple TV');
      await tapText(t, 'Show a photo');
      expect(
        find.text(
          'A photo from this device. It appears on the TV as soon as you pick it.',
        ),
        findsOneWidget,
      );
      await tapText(t, 'Choose file');
      expect(r.picks, [PickKind.photo]);
      expect(r.core.presentedContent.single, isA<ContentSource_File>());
      await finish(t, r);
    });

    testWidgets(
      'Video link: the TV hint; an unsupported link is inline and keeps the join',
      (t) async {
        final r = await desktop(t);
        await see(t, r, [airplay('airplay:1')]);
        await connectTv(t, r, 'airplay:1', 'Apple TV');
        await tapText(t, 'Show a video link');
        expect(
          find.text(
            'The TV loads the video itself. Only direct links to a video file or an HLS stream work.',
          ),
          findsOneWidget,
        );
        await t.enterText(
          find.byKey(const Key('link-url')),
          'https://example.org/page',
        );
        await tapText(t, 'Open on the screen');
        r.core.tvSessions.last.add(
          const PresenterUiEvent.error(
            code: 'tv_unsupported_content',
            message: '',
          ),
        );
        await settle(t);
        expect(
          find.text(
            'The TV cannot play this. Use a direct link to a video file, or a JPEG, PNG or WebP photo.',
          ),
          findsOneWidget,
        );
        expect(find.byKey(const Key('link-url')), findsOneWidget);
        await finish(t, r);
      },
    );
  });

  group('errors (error.tv)', () {
    testWidgets('refused by a Google Cast TV', (t) async {
      final r = await desktop(t);
      await see(t, r, [cast('cast:1')]);
      await t.tap(find.byKey(const Key('tv-row-cast:1')));
      await settle(t);
      r.core.tvSessions.last.add(
        const PresenterUiEvent.error(code: 'tv_refused', message: ''),
      );
      await settle(t);
      expect(
        find.text('“Living room TV” does not accept the connection'),
        findsOneWidget,
      );
      expect(
        find.text(
          'Google Cast is still Beta in Comet Cast and does not work with every TV.',
        ),
        findsOneWidget,
      );
      expect(
        find.text(
          'If you can, install Comet Cast on the TV. Then the code on the screen works.',
        ),
        findsOneWidget,
      );
      await tapKey(t, 'error-back');
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('refused by an AirPlay TV', (t) async {
      final r = await desktop(t);
      await see(t, r, [airplay('airplay:1')]);
      await t.tap(find.byKey(const Key('tv-row-airplay:1')));
      await settle(t);
      r.core.tvSessions.last.add(
        const PresenterUiEvent.error(code: 'tv_refused', message: ''),
      );
      await settle(t);
      expect(
        find.text(
          'AirPlay is still Beta in Comet Cast and does not work with every TV.',
        ),
        findsOneWidget,
      );
      await finish(t, r);
    });

    testWidgets('lost later', (t) async {
      final r = await desktop(t);
      await see(t, r, [cast('cast:1')]);
      await connectTv(t, r, 'cast:1', 'Living room TV');
      r.core.tvSessions.last.add(
        const PresenterUiEvent.error(code: 'tv_lost', message: ''),
      );
      await settle(t);
      expect(find.text('The connection to the TV was lost.'), findsOneWidget);
      await finish(t, r);
    });
  });
}
