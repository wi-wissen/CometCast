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
}
