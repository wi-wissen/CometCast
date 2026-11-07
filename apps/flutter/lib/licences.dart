// SPDX-License-Identifier: GPL-3.0-or-later
// The licence list of About (spec 15.9 "Content sources", native-apps.md 21.7): entries built
// from the bundled THIRD_PARTY_NOTICES.md and THIRD_PARTY_LICENSES.md. Parsed once per app run,
// off the UI thread (`compute`). No widgets and no strings: names come from the files; the one
// entry that is not in a file (Flutter's own licences) is added by the screen.

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart' show rootBundle;

/// One row of the list and its text page.
class LicenceEntry {
  const LicenceEntry({
    required this.name,
    required this.licence,
    required this.text,
    required this.featured,
    this.lazyText,
  });

  /// Reads the text when the entry is opened (Flutter's own licences: `LicenseRegistry`).
  final Future<String> Function()? lazyText;

  final String name;

  /// The first non-empty line of [text], at most 60 characters.
  final String licence;
  final String text;

  /// Parts with their own notice come first.
  final bool featured;
}

/// What the parser reads (plain strings, so that `compute` can carry it).
class LicenceInput {
  const LicenceInput({
    required this.licences,
    required this.notices,
    this.desktop = true,
    this.officeFonts = true,
    this.extra = const {},
  });

  final String licences;
  final String notices;

  /// Desktop builds also list the "Open Screen Library notice" (Google Cast sender).
  final bool desktop;

  /// The Office worker is shipped: "Liberation, Carlito, Caladea" is listed.
  final bool officeFonts;

  /// Further licence files shipped with the app (UxPlay where it is shipped): name to text.
  final Map<String, String> extra;
}

class LicenceSet {
  const LicenceSet({required this.featured, required this.all});

  final List<LicenceEntry> featured;
  final List<LicenceEntry> all;

  bool get isEmpty => featured.isEmpty && all.isEmpty;
}

/// The first non-empty line of [text], at most 60 characters ("…" marks a cut).
String licenceLine(String text) {
  for (final raw in text.split('\n')) {
    final line = raw.trim();
    if (line.isEmpty) continue;
    final runes = line.runes.toList();
    return runes.length <= 60
        ? line
        : '${String.fromCharCodes(runes.take(59))}…';
  }
  return '';
}

class _Section {
  _Section(this.level, this.title);
  final int level;
  final String title;
  final lines = <String>[];
  // Parents' titles at the time the section started.
  String parent2 = '';
  String parent3 = '';
}

/// Headings outside code fences, with the lines that follow each one.
List<_Section> _sections(String md) {
  final out = <_Section>[];
  var inFence = false;
  var h2 = '';
  var h3 = '';
  _Section? cur;
  for (final line in md.split('\n')) {
    if (line.startsWith('```')) {
      inFence = !inFence;
      cur?.lines.add(line);
      continue;
    }
    final m = inFence ? null : RegExp(r'^(#{2,4}) (.+)$').firstMatch(line);
    if (m != null) {
      final level = m.group(1)!.length;
      final title = m.group(2)!.trim();
      if (level == 2) {
        h2 = title;
        h3 = '';
      } else if (level == 3) {
        h3 = title;
      }
      cur = _Section(level, title)
        ..parent2 = h2
        ..parent3 = h3;
      out.add(cur);
      continue;
    }
    cur?.lines.add(line);
  }
  return out;
}

/// The contents of the code fences of [lines], or all lines without the fence marks when there
/// is no fence.
String _bodyText(List<String> lines) {
  final fenced = <String>[];
  var inFence = false;
  var any = false;
  final cur = <String>[];
  for (final l in lines) {
    if (l.startsWith('```')) {
      if (inFence) {
        fenced.add(cur.join('\n').trim());
        cur.clear();
      }
      inFence = !inFence;
      any = true;
      continue;
    }
    if (inFence) cur.add(l);
  }
  if (any && fenced.isNotEmpty) return fenced.join('\n\n');
  return lines.join('\n').trim();
}

String _plainBody(List<String> lines) =>
    lines.where((l) => !l.startsWith('```')).join('\n').trim();

/// The sections of THIRD_PARTY_NOTICES.md that are listed, by the start of their heading (some
/// carry the crate in brackets: `PDFium Foxit font notice (hayro-font)`).
const _noticeNames = {
  'OpenH264 notice': 'Cisco OpenH264',
  'Atkinson Hyperlegible notice': 'Atkinson Hyperlegible',
  'Open Screen Library notice': 'Open Screen Library',
  'PDFium Foxit font notice': 'PDFium Foxit font',
  'Adobe CMap notice': 'Adobe CMap',
};

String? _noticeKey(String title) {
  for (final k in _noticeNames.keys) {
    if (title.startsWith(k)) return k;
  }
  return null;
}

/// Builds the list by the rules of spec 15.9. Featured order: Cisco OpenH264, UxPlay, Atkinson
/// Hyperlegible, Liberation/Carlito/Caladea, then the rest in file order.
LicenceSet parseLicences(LicenceInput input) {
  LicenceEntry entry(String name, String text, {required bool featured}) =>
      LicenceEntry(
        name: name,
        licence: licenceLine(text),
        text: text,
        featured: featured,
      );

  final notices = <String, LicenceEntry>{};
  for (final s in _sections(input.notices)) {
    final key = _noticeKey(s.title);
    if (s.level != 2 || key == null) continue;
    if (key == 'Open Screen Library notice' && !input.desktop) continue;
    notices[key] = entry(
      _noticeNames[key]!,
      _plainBody(s.lines),
      featured: true,
    );
  }

  LicenceEntry? fonts;
  final all = <LicenceEntry>[];
  for (final s in _sections(input.licences)) {
    if (s.level == 4 &&
        (s.parent3 == 'Rust licence texts' ||
            s.parent3 == 'Flutter licence texts')) {
      all.add(entry(s.title, _bodyText(s.lines), featured: false));
    } else if (s.level == 2 &&
        s.title.startsWith('Fonts bundled in the repository') &&
        input.officeFonts) {
      fonts = entry(
        'Liberation, Carlito, Caladea',
        _plainBody(s.lines),
        featured: true,
      );
    } else if (s.level == 2 && s.title.startsWith('npm packages')) {
      for (final line in s.lines) {
        final m = RegExp(r'^\| ([^|]+) \| ([^|]+) \| ([^|]+) \|$')
            .firstMatch(line);
        if (m == null) continue;
        final name = m.group(1)!.trim();
        if (name == 'Package' || name.startsWith('---')) continue;
        final spdx = m.group(3)!.trim();
        all.add(entry(name, spdx, featured: false));
      }
    }
  }

  final featured = <LicenceEntry>[
    ?notices['OpenH264 notice'],
    for (final e in input.extra.entries) entry(e.key, e.value, featured: true),
    ?notices['Atkinson Hyperlegible notice'],
    ?fonts,
    for (final k in notices.keys)
      if (k != 'OpenH264 notice' && k != 'Atkinson Hyperlegible notice')
        notices[k]!,
  ];
  return LicenceSet(featured: featured, all: all);
}

LicenceSet _parseIsolate(LicenceInput input) => parseLicences(input);

/// Reads the two bundled files and parses them off the UI thread. [extraAssets] are further
/// asset paths (UxPlay's licence files) with the name to show.
Future<LicenceSet> loadLicences({
  bool desktop = true,
  bool officeFonts = true,
  Map<String, String> extraAssets = const {},
}) async {
  final licences = await rootBundle.loadString(
    'assets/licenses/THIRD_PARTY_LICENSES.md',
  );
  final notices = await rootBundle.loadString(
    'assets/licenses/THIRD_PARTY_NOTICES.md',
  );
  final extra = <String, String>{};
  for (final e in extraAssets.entries) {
    try {
      extra[e.key] = await rootBundle.loadString(e.value);
    } catch (_) {
      // Not shipped in this build.
    }
  }
  return compute(
    _parseIsolate,
    LicenceInput(
      licences: licences,
      notices: notices,
      desktop: desktop,
      officeFonts: officeFonts,
      extra: extra,
    ),
  );
}

/// Case-insensitive filter on the name (spec 15.9).
List<LicenceEntry> filterLicences(List<LicenceEntry> list, String query) {
  final q = query.trim().toLowerCase();
  if (q.isEmpty) return list;
  return [
    for (final e in list)
      if (e.name.toLowerCase().contains(q)) e,
  ];
}

/// Flutter's own licences as the text of the one entry `about.flutter`.
Future<String> flutterLicenceText() async {
  final b = StringBuffer();
  try {
    await for (final l in LicenseRegistry.licenses) {
      b.writeln(l.packages.join(', '));
      for (final p in l.paragraphs) {
        b.writeln(p.text);
      }
      b.writeln();
    }
  } catch (_) {
    // Whatever was read is shown.
  }
  return b.toString();
}

/// The list with the entry for Flutter's licences ([name] = `about.flutter`) added at the end
/// of "All parts".
LicenceSet withFlutterEntry(LicenceSet set, String name) => LicenceSet(
  featured: set.featured,
  all: [
    ...set.all,
    LicenceEntry(
      name: name,
      licence: '',
      text: '',
      featured: false,
      lazyText: flutterLicenceText,
    ),
  ],
);
