// SPDX-License-Identifier: GPL-3.0-or-later
// Links, install referrer and scanned codes (native-apps.md 20.4, fix list B8 and B9).

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/link_target.dart';
import 'package:cometcast/discovery.dart';

// The alphabet of the core (cc-proto): no 0, 1, I, O, ...; the test uses a stand-in.
String? normalize(String raw) {
  final c = raw.replaceAll(RegExp(r'[\s-]'), '').toUpperCase();
  return RegExp(r'^[A-HJ-NP-Z2-9]{6}$').hasMatch(c) ? c : null;
}

void main() {
  group('accepted URL forms', () {
    test('https://{host}/{CODE}', () {
      expect(
        parseLinkUrl('https://relay.cometcast.de/K7QM2W', normalize),
        const LinkTarget(code: 'K7QM2W', server: 'relay.cometcast.de'),
      );
      // The default port is left out, another port stays.
      expect(
        parseLinkUrl('https://relay.cometcast.de:443/k7q-m2w', normalize),
        const LinkTarget(code: 'K7QM2W', server: 'relay.cometcast.de'),
      );
      expect(
        parseLinkUrl('https://box.local:8443/K7QM2W', normalize)!.server,
        'box.local:8443',
      );
      expect(
        parseLinkUrl('https://relay.cometcast.de/K7Q%20M2W', normalize)!.code,
        'K7QM2W',
      );
    });

    test('https://cometcast.de/app/{CODE} names the built-in relay (21.6)', () {
      expect(
        parseLinkUrl('https://cometcast.de/app/K7QM2W', normalize),
        const LinkTarget(code: 'K7QM2W', server: 'relay.cometcast.de'),
      );
      expect(
        parseLinkUrl('https://cometcast.de:443/app/k7q-m2w/', normalize)!.code,
        'K7QM2W',
      );
      // Not a join link: other paths of the website, a wrong code, another host or port.
      expect(
        parseLinkUrl('https://cometcast.de/app/display', normalize),
        isNull,
      );
      expect(parseLinkUrl('https://cometcast.de/app/', normalize), isNull);
      expect(parseLinkUrl('https://cometcast.de/x/K7QM2W', normalize), isNull);
      expect(
        parseLinkUrl('https://cometcast.de:8443/app/K7QM2W', normalize),
        isNull,
      );
      expect(parseLinkUrl('https://example.org/app/K7QM2W', normalize), isNull);
    });

    test('?server= names the server', () {
      expect(
        parseLinkUrl(
          'https://relay.cometcast.de/K7QM2W?server=Cast.Example.org',
          normalize,
        ),
        const LinkTarget(code: 'K7QM2W', server: 'cast.example.org'),
      );
      expect(
        parseLinkUrl(
          'https://relay.cometcast.de/K7QM2W?server=cast.example.org:8443',
          normalize,
        )!.server,
        'cast.example.org:8443',
      );
    });

    test('anything else is no code', () {
      for (final bad in [
        'http://relay.cometcast.de/K7QM2W', // not https
        'ftp://relay.cometcast.de/K7QM2W',
        'https://relay.cometcast.de/', // no code
        'https://relay.cometcast.de/K7QM2', // too short
        'https://relay.cometcast.de/K7QM2WX', // too long
        'https://relay.cometcast.de/K7QM10', // not in the alphabet
        'https://relay.cometcast.de/K7QM2W/extra',
        'https://user@relay.cometcast.de/K7QM2W',
        'https://relay.cometcast.de/K7QM2W#x',
        'https://relay.cometcast.de/K7QM2W?server=',
        'https://relay.cometcast.de/K7QM2W?server=a.org&server=b.org',
        'https://relay.cometcast.de/K7QM2W?server=https://evil.example/x',
        'https://relay.cometcast.de/K7QM2W?server=evil.example/path',
        'https://relay.cometcast.de/K7QM2W?server=user@evil.example',
        'https://relay.cometcast.de/K7QM2W?server=evil.example:99999',
        'K7QM2W',
        '',
        'not a url',
      ]) {
        expect(parseLinkUrl(bad, normalize), isNull, reason: bad);
      }
    });
  });

  group('install referrer', () {
    LinkTarget? ref(String s) =>
        parseReferrer(s, normalize, defaultServer: 'relay.cometcast.de');

    test('code and optional server', () {
      expect(
        ref('code=K7QM2W'),
        const LinkTarget(code: 'K7QM2W', server: 'relay.cometcast.de'),
      );
      expect(
        ref('code=K7QM2W&server=cast.example.org'),
        const LinkTarget(code: 'K7QM2W', server: 'cast.example.org'),
      );
      expect(ref('utm_source=x&code=K7QM2W')!.code, 'K7QM2W');
    });

    test('same rules as a link', () {
      for (final bad in [
        '',
        'utm_source=google-play',
        'code=K7QM1',
        'code=K7QM2W&code=K7QM2X',
        'code=K7QM2W&server=evil.example/path',
        'code=K7QM2W&server=',
      ]) {
        expect(ref(bad), isNull, reason: bad);
      }
    });
  });

  group('trust', () {
    const trust = ServerTrust(
      suggestions: [
        ServerSuggestion(
          'relay.cometcast.de:443',
          ServerSourceKind.defaultList,
        ),
        ServerSuggestion('mdm.corp.example', ServerSourceKind.policy),
        ServerSuggestion('dns.corp.example:8443', ServerSourceKind.dns),
        ServerSuggestion('box.local:8443', ServerSourceKind.announced),
        ServerSuggestion('last.example.org', ServerSourceKind.lastUsed),
        ServerSuggestion('127.0.0.1:8443', ServerSourceKind.development),
      ],
      confirmed: {'cast.example.org'},
    );

    test(
      'built-in, policy, DNS, in-house box and confirmed are used at once',
      () {
        for (final s in [
          'relay.cometcast.de',
          'RELAY.cometcast.de:443',
          'mdm.corp.example',
          'dns.corp.example:8443',
          'box.local:8443',
          'cast.example.org',
        ]) {
          expect(trust.decide(s), ServerDecision.use, reason: s);
        }
      },
    );

    test('everything else is confirmed first', () {
      for (final s in [
        'evil.example',
        'dns.corp.example', // another port than the DNS entry
        'last.example.org', // last used is not on the list of the spec
        '127.0.0.1:8443',
        'relay.cometcast.de:8443',
        'not a server',
      ]) {
        expect(trust.decide(s), ServerDecision.confirm, reason: s);
      }
    });

    test('outcomes', () {
      final good = parseLinkUrl('https://relay.cometcast.de/K7QM2W', normalize);
      expect(outcomeFor(good, trust), isA<LinkUse>());
      final foreign = parseLinkUrl(
        'https://relay.cometcast.de/K7QM2W?server=cast.other.org',
        normalize,
      );
      final o = outcomeFor(foreign, trust);
      expect(o, isA<LinkConfirm>());
      expect((o as LinkConfirm).target.server, 'cast.other.org');
      expect(outcomeFor(null, trust), isA<LinkNothing>());
    });
  });

  test('canonicalServer', () {
    expect(canonicalServer(' Relay.Example.org:443 '), 'relay.example.org');
    expect(canonicalServer('relay.example.org:8443'), 'relay.example.org:8443');
    expect(canonicalServer('127.0.0.1:8443'), '127.0.0.1:8443');
    expect(canonicalServer('a b'), isNull);
    expect(canonicalServer('https://a.org'), isNull);
    expect(canonicalServer('a.org:0'), isNull);
    expect(canonicalServer('-a.org'), isNull);
  });
}
