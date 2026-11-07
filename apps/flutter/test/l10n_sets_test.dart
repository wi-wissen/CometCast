// SPDX-License-Identifier: GPL-3.0-or-later
// The two German sets (docs/design/first-run-and-display.md 11.1): app_de.arb is "du" and is the
// loaded one; sie/app_de.arb is the formal alternate. Same keys and the same placeholders.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

Map<String, dynamic> load(String p) =>
    jsonDecode(File(p).readAsStringSync()) as Map<String, dynamic>;

Set<String> holders(String s) =>
    RegExp(r'\{(\w+)').allMatches(s).map((m) => m.group(1)!).toSet();

void main() {
  final du = load('lib/l10n/app_de.arb');
  final sie = load('lib/l10n/sie/app_de.arb');
  final en = load('lib/l10n/app_en.arb');

  test('both German sets have identical keys', () {
    expect(sie.keys.toSet(), du.keys.toSet());
  });

  test('both German sets have identical placeholders', () {
    for (final k in du.keys.where((k) => !k.startsWith('@'))) {
      expect(holders(sie[k] as String), holders(du[k] as String), reason: k);
    }
  });

  test('German and English have the same keys', () {
    final e = en.keys.where((k) => !k.startsWith('@')).toSet();
    final d = du.keys.where((k) => !k.startsWith('@')).toSet();
    expect(d, e);
  });

  test('the loaded German set addresses nobody with Sie', () {
    final formal = RegExp(r'\b(Sie|Ihr\w*|Ihnen)\b');
    // "Sie" means "it" in this one string (the same known false alarm as in tools/check-i18n.mjs).
    const falseAlarms = {'sendRejectOfficeFailed'};
    for (final k in du.keys.where(
      (k) => !k.startsWith('@') && !falseAlarms.contains(k),
    )) {
      expect(formal.hasMatch(du[k] as String), isFalse, reason: '$k: ${du[k]}');
    }
  });

  test('the Sie set differs from du in the addressing strings of the spec', () {
    expect(du['setupModeApprovalText'], contains('du lässt'));
    expect(sie['setupModeApprovalText'], contains('Sie lassen'));
  });

  test('switching to Sie changes only addressing strings', () {
    final formal = RegExp(r'\b(Sie|Ihr\w*|Ihnen)\b');
    const falseAlarms = {'sendRejectOfficeFailed'};
    // The greeting is formal in the one set ("Willkommen!") and informal in the other ("Hallo!"):
    // the form of address changes, no pronoun is involved.
    const greetings = {'setupWelcomeTitle'};
    for (final k in du.keys.where((k) => !k.startsWith('@'))) {
      if (du[k] == sie[k] || greetings.contains(k)) continue;
      expect(
        formal.hasMatch(sie[k] as String),
        isTrue,
        reason: '$k differs between the sets but is not addressed with Sie',
      );
      expect(falseAlarms.contains(k), isFalse, reason: k);
    }
  });

  test('the strings of the Flutter review (11.9) are in all three sets', () {
    const keys = {
      'receiveLinkHintApp': 'It opens in the browser, and only if you choose',
      'setupAddressSourcePolicy': 'Set by the administrator of this device.',
      'idleOfflineBodyOutdatedApp':
          'This app is out of date. Update it to get a code.',
      'errorOutdatedTitleApp': 'This app is out of date',
      'errorOutdatedNowApp': 'Update the app, then try again.',
      'whatScreenTablet': 'Your whole tablet display, live',
      'fileBodyApp': 'A PDF, an image or an Office file from this device.',
      'fileConverting': 'Converting {name} …',
      'relayConfirmBodyApp': 'A link asks this app to use the relay {host}',
      'appScanWrong': 'This QR code is not from a Comet Cast screen.',
      'appMiracast': 'Receive Miracast on this PC',
      'iosCameraUsage': 'Comet Cast uses the camera only to scan the QR code',
      'codecLater': 'Not now',
      'codecDownloading': 'Downloading the video codec …',
    };
    for (final e in keys.entries) {
      expect(en[e.key], contains(e.value), reason: e.key);
      expect(du[e.key], isA<String>(), reason: e.key);
      expect(sie[e.key], isA<String>(), reason: e.key);
    }
    // Typographic ellipsis, not three dots.
    expect(en['codecDownloading'], endsWith(' …'));
    expect(du['codecDownloading'], endsWith(' …'));
    expect(sie['codecDownloading'], endsWith(' …'));
  });

  test('the formal set addresses the strings that the spec lists', () {
    expect(sie['receiveLinkHintApp'], contains('Öffnen Sie ihn nur'));
    expect(du['receiveLinkHintApp'], contains('Öffne ihn nur'));
    expect(sie['errorOutdatedNowApp'], contains('Aktualisieren Sie die App'));
    expect(du['errorOutdatedNowApp'], contains('Aktualisiere die App'));
    expect(
      sie['idleOfflineBodyOutdatedApp'],
      contains('Aktualisieren Sie sie'),
    );
    expect(sie['relayConfirmBodyApp'], contains('Fahren Sie nur fort'));
    expect(du['relayConfirmBodyApp'], contains('Mach nur weiter'));
    // The ones the spec gives as "same as du" are the same.
    for (final k in [
      'setupAddressSourcePolicy',
      'errorOutdatedTitleApp',
      'fileConverting',
      'appScanWrong',
      'appMiracast',
      'iosCameraUsage',
      'codecLater',
      'codecDownloading',
    ]) {
      expect(sie[k], du[k], reason: k);
    }
  });

  test('strings of the web page that the apps never show are gone', () {
    for (final k in [
      'errorActionReload',
      'errorOutdatedTitle',
      'errorOutdatedNow',
      'idleOfflineBodyOutdated',
      'relayConfirmBody',
      'receiveLinkHint',
      'byeBody',
    ]) {
      expect(en.containsKey(k), isFalse, reason: k);
      expect(du.containsKey(k), isFalse, reason: k);
      expect(sie.containsKey(k), isFalse, reason: k);
    }
  });
}
