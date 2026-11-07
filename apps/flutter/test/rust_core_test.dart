// SPDX-License-Identifier: GPL-3.0-or-later
// Calls the REAL Rust library through flutter_rust_bridge.
// Needs the library: `cargo build -p cc-ffi` (default path target/debug/libcc_ffi.so) or
// COMETCAST_FFI_LIB=/path/to/libcc_ffi.so. Without the library the tests are skipped.

import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/rust_loader.dart';

import 'content_support.dart' show makeDocx;
import 'fake_core.dart';

String? findLibrary() {
  final env = Platform.environment[ffiLibraryEnvVar];
  if (env != null && env.isNotEmpty) return env;
  // `flutter test` runs in apps/flutter; the workspace target folder is two levels up.
  const candidates = [
    '../../target/debug/libcc_ffi.so',
    '../../target/debug/libcc_ffi.dylib',
  ];
  for (final c in candidates) {
    if (File(c).existsSync()) return File(c).absolute.path;
  }
  return null;
}

/// Checks that hold whether or not this machine has an MDM policy file.
void _expectConsistent(
  List<ServerCandidate> servers,
  bool locked,
  List<DiscoveryWarning> notices,
) {
  const known = {'mdm', 'dns-srv', 'display-announcement', 'default-list'};
  for (final s in servers) {
    expect(known, contains(s.source));
    expect(s.address.trim(), isNotEmpty);
  }
  expect(
    servers.map((s) => s.address.toLowerCase()).toSet().length,
    servers.length,
    reason: 'each address once',
  );
  // A lock offers policy servers only.
  if (locked) expect(servers.every((s) => s.source == 'mdm'), isTrue);
  // An unusable policy locks and offers nothing.
  if (notices.contains(DiscoveryWarning.policyUnusable)) {
    expect(locked, isTrue);
    expect(servers, isEmpty);
  }
}

void main() {
  final lib = findLibrary();
  final skip = lib == null
      ? 'Rust library not built (run: cargo build -p cc-ffi)'
      : null;

  setUpAll(() async {
    if (lib != null) await initRustCore(libraryPath: lib);
  });

  test('room code rules come from Rust and match the Dart fake', () {
    const core = RustCoreApi();
    final fake = FakeCoreApi();
    expect(core.roomCodeLength, 6);
    for (final input in [
      'K7M2QX',
      ' k7m2qx ',
      'K7M2',
      'K70',
      'K7M2QO',
      'K7M2Q1',
      '',
      'ſ7M2QX',
    ]) {
      final real = core.checkRoomCode(input);
      final copy = fake.checkRoomCode(input);
      expect(real.runtimeType, copy.runtimeType, reason: 'input "$input"');
    }
    final ok = core.checkRoomCode(' k7m2qx ');
    expect(ok, isA<RoomCodeCheck_Valid>());
    expect((ok as RoomCodeCheck_Valid).code, 'K7M2QX');
  }, skip: skip);

  test('starting a receiver on a closed port reports an error event', () async {
    const core = RustCoreApi();
    final events = await core.startReceiver(server: '127.0.0.1:1').toList();
    expect(events, hasLength(1));
    expect(events.single, isA<ReceiverUiEvent_Error>());
  }, skip: skip);

  test('starting a sender with a bad code reports an error event', () async {
    const core = RustCoreApi();
    final events = await core
        .startSender(code: 'nope', server: '127.0.0.1:1')
        .toList();
    expect(events, hasLength(1));
    expect(events.single, isA<SenderUiEvent_Error>());
  }, skip: skip);

  test('pushing encoded video without an encoded sender is refused', () {
    const core = RustCoreApi();
    final au = Uint8List.fromList([0, 0, 0, 1, 0x65, 1, 2]);
    expect(
      core.pushEncodedFrame(au, isKey: true, ptsUs: 0),
      PushResult.noSender,
    );
    expect(core.pushCodecConfig(au), isFalse);
    expect(core.keyframeRequested(), isFalse);
  }, skip: skip);

  test('server defaults come from cc-discovery', () {
    const core = RustCoreApi();
    final d = core.serverDefaults();
    expect(d.devFallback, '127.0.0.1:8443');
    final fake = FakeCoreApi().serverDefaults();
    expect(fake.devFallback, d.devFallback);
    // A machine may have an MDM policy file (/etc/cometcast/policy.json) that changes the
    // list, so only what holds with and without a policy is checked.
    _expectConsistent(d.servers, d.locked, d.notices);
    if (!d.locked && !d.servers.any((s) => s.source == 'mdm')) {
      // No usable policy: the built-in list, as the fake has it.
      expect(
        d.servers.map((s) => (s.address, s.source)),
        fake.servers.map((s) => (s.address, s.source)),
      );
    }
  }, skip: skip);

  test(
    'server discovery answers with ordered, labelled, consistent data',
    () async {
      const core = RustCoreApi();
      final r = await core.discoverServers();
      _expectConsistent(r.servers, r.locked, r.notices);
      // Policy servers always come first.
      final sources = r.servers.map((s) => s.source).toList();
      final lastMdm = sources.lastIndexOf('mdm');
      if (lastMdm >= 0) {
        expect(sources.take(lastMdm + 1).every((s) => s == 'mdm'), isTrue);
      }
      // A policy that forbids other servers blocks announcements.
      if (r.locked) expect(r.announcementsAllowed, isFalse);
      // The raw warnings are for logs; the UI only uses the codes.
      expect(
        r.warnings.length >= r.notices.length || r.notices.isEmpty,
        isTrue,
      );
    },
    skip: skip,
  );

  test('announcing and listing receivers can be started and stopped', () async {
    const core = RustCoreApi();
    // A loopback server is refused by the checks of cc-discovery; the name alone is fine.
    final refused = await core.startAdvertising(
      name: 'Test receiver',
      preferredServer: '127.0.0.1:8443',
    );
    expect(refused, AdvertiseOutcome.refused);
    final outcome = await core.startAdvertising(name: 'Test receiver');
    // No multicast in a sandbox is possible: then it fails, but never throws.
    expect(outcome, anyOf(AdvertiseOutcome.started, AdvertiseOutcome.failed));
    await core.stopAdvertising();
    final events = core.watchReceivers().listen((_) {});
    await Future<void>.delayed(const Duration(milliseconds: 200));
    await core.stopWatchingReceivers();
    await events.cancel();
  }, skip: skip);

  test('no picture is waiting before a receiver ran', () async {
    const core = RustCoreApi();
    expect(await core.takeFrame(), isNull);
    core.setVideoMaxSize(640, 360); // must not throw
  }, skip: skip);

  test(
    'capture check: an unreachable display is reported, not thrown',
    () async {
      const core = RustCoreApi();
      final info = await core.captureInfo(display: ':9999');
      expect(info.support, isA<CaptureSupport_Failed>());
      expect(info.monitors, isEmpty);
    },
    skip: skip,
  );

  group('content', () {
    test('link rules come from Rust; the host is the punycode form', () {
      const core = RustCoreApi();
      final fake = FakeCoreApi();
      final ok = core.validateLink(' https://bücher.example/a?b=1 ');
      expect(ok, isA<LinkCheck_Valid>());
      final valid = ok as LinkCheck_Valid;
      expect(valid.host, 'xn--bcher-kva.example');
      expect(valid.url, startsWith('https://xn--bcher-kva.example/'));
      for (final bad in [
        'javascript:alert(1)',
        'file:///etc/passwd',
        'ftp://x.example/',
        'https://user:pw@x.example/',
        'https://x.example/a b',
        'https://',
        '',
      ]) {
        expect(core.validateLink(bad), isA<LinkCheck_Invalid>(), reason: bad);
        expect(fake.validateLink(bad), isA<LinkCheck_Invalid>(), reason: bad);
      }
      // A link that passes passes again unchanged (idempotent).
      final again = core.validateLink(valid.url) as LinkCheck_Valid;
      expect(again.url, valid.url);
    }, skip: skip);

    test('commands are refused while no content sender runs', () {
      const core = RustCoreApi();
      expect(core.sendContentCmd(const UiContentCmd.nextPage()), isFalse);
      expect(
        core.sendContentCmd(const UiContentCmd.goToPage(page: 0)),
        isFalse,
      );
    }, skip: skip);

    test('no content frame waits before a receiver ran', () async {
      const core = RustCoreApi();
      expect(await core.takeContentFrame(), isNull);
      core.setContentTargetWidth(1280); // must not throw
    }, skip: skip);

    test(
      'files and links that cannot be sent are refused before connecting',
      () async {
        const core = RustCoreApi();
        final dir = Directory.systemTemp.createTempSync('cometcast_content_');
        addTearDown(() => dir.deleteSync(recursive: true));
        Future<UiSendRejection> rejected(ContentSource source) async {
          final events = await core
              .startContentSender(
                code: 'K7M2QX',
                server: '127.0.0.1:1',
                source: source,
              )
              .toList();
          expect(events, hasLength(1), reason: '$source');
          return (events.single as ContentSenderUiEvent_Rejected).reason;
        }

        expect(
          await rejected(ContentSource.file(path: '${dir.path}/missing.png')),
          UiSendRejection.unreadable,
        );
        File('${dir.path}/words.png')
            .writeAsStringSync('this is not a picture');
        expect(
          await rejected(ContentSource.file(path: '${dir.path}/words.png')),
          UiSendRejection.unsupportedType,
        );
        File('${dir.path}/empty.pdf').writeAsBytesSync([]);
        expect(
          await rejected(ContentSource.file(path: '${dir.path}/empty.pdf')),
          UiSendRejection.empty,
        );
        File('${dir.path}/fake.pdf').writeAsStringSync('not a pdf at all');
        expect(
          await rejected(ContentSource.file(path: '${dir.path}/fake.pdf')),
          UiSendRejection.notPdf,
        );
        File('${dir.path}/vector.svg').writeAsStringSync('<svg/>');
        expect(
          await rejected(ContentSource.file(path: '${dir.path}/vector.svg')),
          UiSendRejection.unsupportedType,
        );
        expect(
          await rejected(const ContentSource.link(url: 'javascript:alert(1)')),
          UiSendRejection.badLink,
        );
      },
      skip: skip,
    );

    test('a bad room code is an error event', () async {
      const core = RustCoreApi();
      final events = await core
          .startContentSender(
            code: 'nope',
            server: '127.0.0.1:1',
            source: const ContentSource.link(url: 'https://example.org/'),
          )
          .toList();
      expect(events.single, isA<ContentSenderUiEvent_Error>());
    }, skip: skip);

    test('the render worker counts the pages of a real PDF', () async {
      const core = RustCoreApi();
      final worker = File('${File(lib!).parent.path}/cc-docs-worker');
      final pdf = File('../../crates/cc-docs/tests/data/slides.pdf');
      if (!worker.existsSync() || !pdf.existsSync()) {
        markTestSkipped(
          'cc-docs-worker or the test PDF is missing (cargo build --workspace --bins)',
        );
        return;
      }
      core.setDocsWorker(worker.absolute.path);
      addTearDown(() => core.setDocsWorker(null));
      expect(core.docsWorkerPath(), worker.absolute.path);
      // No server on port 1: the file is checked first, then the connection fails.
      final events = await core
          .startContentSender(
            code: 'K7M2QX',
            server: '127.0.0.1:1',
            source: ContentSource.file(path: pdf.absolute.path),
          )
          .toList();
      final prepared = events.first as ContentSenderUiEvent_Prepared;
      expect(prepared.kind, UiContentKind.pdf);
      expect(prepared.name, 'slides.pdf');
      expect(prepared.pages, 3);
      expect(events.last, isA<ContentSenderUiEvent_Error>());
    }, skip: skip);

    test('Office files are refused with their own reasons, after a "converting" event', () async {
      const core = RustCoreApi();
      final dir = Directory.systemTemp.createTempSync('cometcast_office_');
      addTearDown(() => dir.deleteSync(recursive: true));
      Future<List<ContentSenderUiEvent>> run(String name) => core
          .startContentSender(
            code: 'K7M2QX',
            server: '127.0.0.1:1',
            source: ContentSource.file(path: '${dir.path}/$name'),
          )
          .toList();
      void expectRejected(List<ContentSenderUiEvent> e, UiSendRejection r) {
        expect(e, hasLength(2), reason: '$e');
        expect(e.first, isA<ContentSenderUiEvent_Converting>());
        expect((e.last as ContentSenderUiEvent_Rejected).reason, r);
      }

      // Not a ZIP: the container check refuses it before any worker starts.
      File('${dir.path}/broken.docx').writeAsStringSync('this is not a zip');
      expectRejected(await run('broken.docx'), UiSendRejection.officeFailed);
      // An old binary (OLE) file or a password-protected one.
      final ole = Uint8List(600)
        ..setAll(0, [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
      File('${dir.path}/old.pptx').writeAsBytesSync(ole);
      expectRejected(await run('old.pptx'), UiSendRejection.officeEncrypted);
      // A valid file, but no worker program: unavailable, not "damaged".
      core.setOfficeWorker('/nonexistent/cc-office-worker');
      addTearDown(() => core.setOfficeWorker(null));
      expect(core.officeWorkerPath(), isNull);
      File('${dir.path}/ok.docx').writeAsBytesSync(makeDocx('Hello'));
      expectRejected(await run('ok.docx'), UiSendRejection.officeUnavailable);
      // OpenDocument and legacy names are not converted at all (no converting event).
      for (final n in ['a.odt', 'a.doc', 'a.xls']) {
        File('${dir.path}/$n').writeAsStringSync('x');
        final e = await run(n);
        expect(e, hasLength(1), reason: n);
        expect(
          (e.single as ContentSenderUiEvent_Rejected).reason,
          UiSendRejection.unsupportedType,
        );
      }
    }, skip: skip);

    test(
      'the real Office worker converts a .docx to a PDF before connecting',
      () async {
        const core = RustCoreApi();
        final worker = File('${File(lib!).parent.path}/cc-office-worker');
        if (!worker.existsSync()) {
          markTestSkipped(
            'cc-office-worker is missing (cargo build -p cc-office --bins)',
          );
          return;
        }
        core.setOfficeWorker(worker.absolute.path);
        addTearDown(() => core.setOfficeWorker(null));
        expect(core.officeWorkerPath(), worker.absolute.path);
        final dir = Directory.systemTemp.createTempSync('cometcast_office_');
        addTearDown(() => dir.deleteSync(recursive: true));
        File('${dir.path}/Class notes.docx')
            .writeAsBytesSync(makeDocx('Hello Comet Cast'));
        final events = await core
            .startContentSender(
              code: 'K7M2QX',
              server: '127.0.0.1:1',
              source: ContentSource.file(path: '${dir.path}/Class notes.docx'),
            )
            .toList();
        expect(events.first, isA<ContentSenderUiEvent_Converting>());
        final prepared = events[1] as ContentSenderUiEvent_Prepared;
        expect(prepared.kind, UiContentKind.pdf);
        expect(prepared.name, 'Class notes.pdf');
        expect(prepared.size.toInt(), greaterThan(100));
        // No server on port 1: after the conversion the connection fails.
        expect(events.last, isA<ContentSenderUiEvent_Error>());
      },
      skip: skip,
    );

    test(
      'a missing worker is named, never replaced by an unprotected engine',
      () {
        const core = RustCoreApi();
        core.setDocsWorker('/nonexistent/cc-docs-worker');
        addTearDown(() => core.setDocsWorker(null));
        expect(core.docsWorkerPath(), isNull);
      },
      skip: skip,
    );
  });
  group('screens on this network (the real core)', () {
    test(
      'a native bridge feeds the list; the core checks and cleans',
      () async {
        const core = RustCoreApi();
        final events = <NearbyUiEvent>[];
        final sub = core.watchNearbyScreens(native: true).listen(events.add);
        addTearDown(sub.cancel);
        Future<void> until(bool Function() ok) async {
          for (var i = 0; i < 100 && !ok(); i++) {
            await Future<void>.delayed(const Duration(milliseconds: 20));
          }
        }

        await until(() => events.isNotEmpty);
        expect(events.first, isA<NearbyUiEvent_SearchStarted>());
        const id = '0123456789abcdef';
        core.nearbyNativeSeen(
          instance: 'CometCast-01234567',
          addresses: ['10.0.0.5'],
          txt: [
            ('v', '1'),
            ('id', id),
            ('srv', '10.0.0.5:8443'),
            ('mode', 'approval'),
            ('code', 'P4X9TR'),
            ('name', '\u202e Room\t 204 '),
          ],
        );
        // Hostile records leave no trace: wrong code alphabet, public IP, other layout version.
        for (final txt in [
          [
            ('v', '1'),
            ('id', 'fedcba9876543210'),
            ('srv', '10.0.0.6:8443'),
            ('mode', 'open'),
            ('code', 'P4X9T0'),
          ],
          [
            ('v', '1'),
            ('id', 'fedcba9876543211'),
            ('srv', '8.8.8.8:443'),
            ('mode', 'open'),
            ('code', 'P4X9TR'),
          ],
          [
            ('v', '2'),
            ('id', 'fedcba9876543212'),
            ('srv', '10.0.0.6:8443'),
            ('mode', 'open'),
            ('code', 'P4X9TR'),
          ],
        ]) {
          core.nearbyNativeSeen(
            instance: 'evil',
            addresses: ['10.0.0.66'],
            txt: txt,
          );
        }
        await until(() => events.any((e) => e is NearbyUiEvent_Added));
        await Future<void>.delayed(const Duration(milliseconds: 100));
        final added = events.whereType<NearbyUiEvent_Added>().toList();
        expect(added, hasLength(1));
        final s = added.single.screen;
        expect(s.id, id);
        expect(s.name, 'Room 204');
        expect(s.server, 'ws://10.0.0.5:8443');
        expect(s.mode, NearbyMode.approval);
        expect(s.code, 'P4X9TR');
        // A goodbye removes the row at once; Locked carries no code.
        core.nearbyNativeGone('CometCast-01234567');
        await until(() => events.any((e) => e is NearbyUiEvent_Removed));
        expect(events.whereType<NearbyUiEvent_Removed>().single.id, id);
        await core.stopNearbyScreens();
        await until(() => events.any((e) => e is NearbyUiEvent_SearchStopped));
        expect(events.last, isA<NearbyUiEvent_SearchStopped>());
      },
      skip: skip,
    );

    test('the display announcement needs a valid id', () async {
      const core = RustCoreApi();
      expect(
        await core.setDisplayAnnouncing(active: true, instanceId: 'nope'),
        AnnounceStatus.badId,
      );
      expect(
        await core.setDisplayAnnouncing(
          active: false,
          instanceId: '0123456789abcdef',
        ),
        anyOf(AnnounceStatus.off, AnnounceStatus.forbiddenByPolicy),
      );
      expect(core.localNetworkAvailable(), isA<bool>());
    }, skip: skip);
  });
}
