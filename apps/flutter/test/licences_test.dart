// SPDX-License-Identifier: GPL-3.0-or-later
// The licence parser (spec 15.9 "Content sources", fix list F3) on the real bundled files.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/licences.dart';

void main() {
  final licences = File('assets/licenses/THIRD_PARTY_LICENSES.md')
      .readAsStringSync();
  final notices = File('assets/licenses/THIRD_PARTY_NOTICES.md')
      .readAsStringSync();
  final set = parseLicences(LicenceInput(licences: licences, notices: notices));

  test(
    'the featured group: OpenH264, Atkinson, the Office fonts, then the rest',
    () {
      expect(set.featured.map((e) => e.name).toList(), [
        'Cisco OpenH264',
        'Atkinson Hyperlegible',
        'Liberation, Carlito, Caladea',
        'Open Screen Library',
        'PDFium Foxit font',
        'Adobe CMap',
      ]);
      expect(set.featured.every((e) => e.featured), isTrue);
    },
  );

  test(
    'without the Office worker and on non-desktop builds parts are left out',
    () {
      final s = parseLicences(
        LicenceInput(
          licences: licences,
          notices: notices,
          desktop: false,
          officeFonts: false,
        ),
      );
      final names = s.featured.map((e) => e.name);
      expect(names, isNot(contains('Liberation, Carlito, Caladea')));
      expect(names, isNot(contains('Open Screen Library')));
      expect(names, contains('Cisco OpenH264'));
    },
  );

  test('UxPlay comes second where its licence file is shipped', () {
    final s = parseLicences(
      LicenceInput(
        licences: licences,
        notices: notices,
        extra: const {'UxPlay': 'GNU GENERAL PUBLIC LICENSE\nVersion 3'},
      ),
    );
    expect(s.featured.map((e) => e.name).take(3).toList(), [
      'Cisco OpenH264',
      'UxPlay',
      'Atkinson Hyperlegible',
    ]);
  });

  test('every #### heading of the Rust and Flutter texts is an entry, texts are fenced blocks', () {
    final headings = RegExp(r'^#### ', multiLine: true).allMatches(licences);
    final fontHeadings = RegExp(
      r'^#### (Liberation|Carlito|Caladea)',
      multiLine: true,
    ).allMatches(licences).length;
    final npmStart = licences.indexOf('## npm packages');
    final npmEnd = licences.indexOf('\n## ', npmStart + 5);
    final npmRows =
        RegExp(
          r'^\| [^|]+ \| [^|]+ \| [^|]+ \|$',
          multiLine: true,
        ).allMatches(licences.substring(npmStart, npmEnd)).length -
        2; // the header and the separator row
    expect(set.all.length, headings.length - fontHeadings + npmRows);
    final tokio = set.all.where((e) => e.name.startsWith('tokio ')).toList();
    expect(tokio, isNotEmpty);
    expect(tokio.first.text, isNot(contains('```')));
    expect(tokio.first.licence, isNotEmpty);
  });

  test(
    'the licence line is the first non-empty line, at most 60 characters',
    () {
      for (final e in [...set.featured, ...set.all]) {
        expect(e.licence.runes.length, lessThanOrEqualTo(60), reason: e.name);
      }
      expect(licenceLine('\n\n  MIT License\nrest'), 'MIT License');
      expect(licenceLine('x' * 70).runes.length, 60);
    },
  );

  test('npm rows are name plus SPDX, the text being the SPDX line', () {
    final vue = set.all.firstWhere((e) => e.name == 'vue');
    expect(vue.text, 'MIT');
    expect(vue.licence, 'MIT');
  });

  test('search is case-insensitive on the name: tokio yes, xyz no', () {
    expect(
      filterLicences(set.all, 'TokIO').any((e) => e.name.startsWith('tokio')),
      isTrue,
    );
    expect(filterLicences(set.all, 'xyzxyz'), isEmpty);
    expect(
      filterLicences(set.featured, 'openh264').single.name,
      'Cisco OpenH264',
    );
    expect(
      filterLicences(set.featured, 'atkinson').single.name,
      'Atkinson Hyperlegible',
    );
    expect(filterLicences(set.all, '  '), set.all);
  });

  test('a licence text of a ## line inside a fence is no new section', () {
    // The OpenEXR texts contain `## 1. Provision of the Data` lines inside fences.
    expect(set.all.any((e) => e.name.startsWith('1. Provision')), isFalse);
  });
}
