// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter's app settings and About (spec 15.8, 15.9; fix list F1 to F3).

import 'dart:ui' show Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/settings.dart';
import 'package:cometcast/src/beta.dart';
import 'package:cometcast/src/presenter/kit.dart';

import 'nearby_support.dart' show settle;
import 'round5_support.dart';

void main() {
  test('the Beta table lists exactly the three Beta ways', () {
    expect(betaFeatures, {
      BetaFeature.airplayReceive,
      BetaFeature.miracastReceive,
      BetaFeature.otherTvs,
    });
  });

  test(
    'the on/off settings have their defaults and survive a restart',
    () async {
      final store = MemorySettingsStore();
      final a = AppSettings(store: store);
      expect(a.officeConvert, isTrue);
      expect(a.otherTvs, isFalse);
      expect(a.airplayReceive, isFalse);
      expect(a.miracastReceive, isFalse);
      a.officeConvert = false;
      a.otherTvs = true;
      a.airplayReceive = true;
      a.miracastReceive = true;
      final b = AppSettings(store: store);
      await b.loadFlags();
      expect(b.officeConvert, isFalse);
      expect(b.otherTvs, isTrue);
      expect(b.airplayReceive, isTrue);
      expect(b.miracastReceive, isTrue);
    },
  );

  group('entry in the brand row', () {
    testWidgets('desktop: the button reads Settings, the row is 44 high', (
      t,
    ) async {
      final r = await pumpR5(t, desktop: true, size: const Size(1100, 800));
      expect(find.byKey(const Key('app-settings-open')), findsOneWidget);
      expect(find.text('Settings'), findsOneWidget);
      expect(t.getSize(find.byKey(const Key('brand'))).height, 44);
      await finish(t, r);
    });

    testWidgets('no switch anywhere (iOS): the button reads About', (t) async {
      final r = await pumpR5(t); // phone, no worker: no switch
      expect(find.text('About'), findsOneWidget);
      await tapKey(t, 'app-settings-open');
      // About directly.
      expect(find.text('About Comet Cast'), findsWidgets);
      expect(find.byKey(const Key('switch-office')), findsNothing);
      await finish(t, r);
    });
  });

  group('the settings page', () {
    testWidgets('desktop: Office, TV (Beta) and About rows', (t) async {
      final r = await pumpR5(
        t,
        desktop: true,
        office: true,
        size: const Size(1100, 800),
      );
      await tapKey(t, 'app-settings-open');
      expect(find.text('Settings'), findsOneWidget);
      expect(find.text('Convert Office files to PDF'), findsOneWidget);
      expect(find.text('TVs without Comet Cast'), findsOneWidget);
      expect(find.text('BETA'), findsOneWidget);
      expect(find.text('About Comet Cast'), findsOneWidget);
      expect(find.text('Version 0.1.0'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('Android: Office and About, no TV row', (t) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      expect(find.byKey(const Key('switch-office')), findsOneWidget);
      expect(find.byKey(const Key('switch-tvs')), findsNothing);
      await finish(t, r);
    });

    testWidgets('the whole row toggles; the value is stored at once', (
      t,
    ) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      expect(r.settings.officeConvert, isTrue);
      await t.tap(find.text('Convert Office files to PDF'));
      await settle(t);
      expect(r.settings.officeConvert, isFalse);
      expect(r.store.flags['officeConvert'], isFalse);
      await finish(t, r);
    });

    testWidgets('a switch row is one semantics node with the toggled state', (
      t,
    ) async {
      final handle = t.ensureSemantics();
      final r = await pumpR5(
        t,
        desktop: true,
        office: true,
        tvs: true,
        size: const Size(1100, 800),
      );
      await tapKey(t, 'app-settings-open');
      final node = t.getSemantics(find.byKey(const Key('switch-tvs')));
      expect(node.label, 'TVs without Comet Cast, Beta');
      expect(node.flagsCollection.isToggled, Tristate.isTrue);
      expect(node.hint, contains('Not tested with every TV'));
      handle.dispose();
      await finish(t, r);
    });

    testWidgets('the knob jumps with reduced motion', (t) async {
      final r = await pumpR5(t, office: true);
      t.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      addTearDown(t.platformDispatcher.clearAccessibilityFeaturesTestValue);
      await tapKey(t, 'app-settings-open');
      final knob = find
          .descendant(
            of: find.byKey(const Key('switch-knob')).first,
            matching: find.byType(DecoratedBox),
          )
          .first;
      final before = t.getTopLeft(knob);
      await t.tap(find.text('Convert Office files to PDF'));
      await t.pump();
      expect(t.getTopLeft(knob).dx, lessThan(before.dx));
      await finish(t, r);
    });

    testWidgets('Back and system Back lead to the start page', (t) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'settings-back');
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      await tapKey(t, 'app-settings-open');
      await t.binding.handlePopRoute();
      await settle(t);
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      await finish(t, r);
    });
  });

  group('About', () {
    testWidgets('version, licence, links in the UI language, list', (t) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      expect(find.text('About Comet Cast'), findsOneWidget);
      expect(find.byKey(const Key('about-version')), findsOneWidget);
      expect(find.text('Free software under the GPL-3.0'), findsOneWidget);
      // The default branding has no publisher, privacy or imprint (empty = no links).
      expect(find.byKey(const Key('about-publisher')), findsNothing);
      expect(find.byKey(const Key('about-privacy')), findsNothing);
      await t.tap(find.byKey(const Key('about-source')));
      await settle(t);
      expect(
        r.opened.single.toString(),
        'https://github.com/wi-wissen/CometCast/tree/v0.1.0',
      );
      // Featured group first.
      final featured = t.getTopLeft(find.text('With their own notice')).dy;
      final all = t.getTopLeft(find.text('All parts')).dy;
      expect(featured, lessThan(all));
      expect(
        t.getTopLeft(find.text('Cisco OpenH264')).dy,
        lessThan(t.getTopLeft(find.text('Atkinson Hyperlegible')).dy),
      );
      await finish(t, r);
    });

    testWidgets('search finds tokio, xyz shows Nothing found', (t) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      await t.enterText(find.byKey(const Key('licence-search')), 'TOKIO');
      await settle(t);
      expect(find.text('tokio 1.40.0'), findsOneWidget);
      expect(find.text('serde 1.0.0'), findsNothing);
      expect(find.text('Cisco OpenH264'), findsNothing);
      await t.enterText(find.byKey(const Key('licence-search')), 'xyz');
      await settle(t);
      expect(find.text('Nothing found.'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('a row opens the text page; Back returns to the list', (
      t,
    ) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      await tapText(t, 'tokio 1.40.0');
      expect(find.byKey(const Key('licence-title')), findsOneWidget);
      expect(find.text('tokio licence text'), findsOneWidget);
      await tapKey(t, 'licence-back');
      expect(find.text('About Comet Cast'), findsOneWidget);
      // System Back from the text page.
      await tapText(t, 'tokio 1.40.0');
      await t.binding.handlePopRoute();
      await settle(t);
      expect(find.text('About Comet Cast'), findsOneWidget);
      await finish(t, r);
    });
  });

  testWidgets('the presenter switch is not the Material Switch', (t) async {
    final r = await pumpR5(t, office: true);
    await tapKey(t, 'app-settings-open');
    expect(find.byType(Switch), findsNothing);
    expect(find.byType(PSwitch), findsOneWidget);
    // Track 44 x 26 (spec 15.10).
    expect(t.getSize(find.byType(PSwitch)), const Size(44, 26));
    await finish(t, r);
  });
}

class FakeAccessibilityFeatures implements AccessibilityFeatures {
  const FakeAccessibilityFeatures({this.disableAnimations = false});
  @override
  final bool disableAnimations;
  @override
  bool get accessibleNavigation => false;
  @override
  bool get boldText => false;
  @override
  bool get highContrast => false;
  @override
  bool get invertColors => false;
  @override
  bool get onOffSwitchLabels => false;
  @override
  bool get reduceMotion => false;
  @override
  bool get supportsAnnounce => true;
  @override
  bool get autoPlayAnimatedImages => true;
  @override
  bool get autoPlayVideos => true;
  @override
  bool get deterministicCursor => false;
}
