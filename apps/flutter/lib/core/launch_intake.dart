// SPDX-License-Identifier: GPL-3.0-or-later
// What a link, the install referrer or a scan turns into on the first-run page: the code chip,
// the relay confirmation page for a server this device does not know, or nothing
// (native-apps.md 20.4, fix list B8). No widgets and no strings.

import '../discovery.dart' show ServerSourceKind;
import '../settings.dart';
import 'app_links.dart';
import 'core_api.dart';
import 'link_target.dart';

/// The address used when a referrer or a link names no server of its own (the built-in relay,
/// `cc_discovery::DEFAULT_SERVERS`).
const builtInRelay = 'relay.cometcast.de';

class LinkHandler {
  LinkHandler({required this.core, required this.settings});

  final CoreApi core;
  final AppSettings settings;

  /// The code of [raw] (spaces and dashes removed), checked by the core; null when it is none.
  String? normalizeCode(String raw) {
    final r = core.checkRoomCode(raw.replaceAll(RegExp(r'[\s-]'), ''));
    return r is RoomCodeCheck_Valid ? r.code : null;
  }

  ServerTrust get trust => ServerTrust(
    suggestions: settings.serverSuggestions,
    confirmed: settings.confirmedServers,
    builtIn: {builtInRelay},
  );

  String get _defaultServer {
    for (final s in settings.serverSuggestions) {
      final c = canonicalServer(s.address);
      if (c != null && s.kind == ServerSourceKind.defaultList) return c;
    }
    return builtInRelay;
  }

  /// A link or a scanned QR code.
  LinkOutcome fromUrl(String text) =>
      outcomeFor(parseLinkUrl(text, normalizeCode), trust);

  /// The install referrer.
  LinkOutcome fromReferrer(String referrer) => outcomeFor(
    parseReferrer(referrer, normalizeCode, defaultServer: _defaultServer),
    trust,
  );

  /// Take [server] for this run without remembering it: "a link never changes the stored last
  /// used server until a join succeeded" (`AppSettings.rememberServer` runs after a join). A
  /// policy that locks the server keeps its own.
  void useServer(String server) {
    if (canonicalServer(server) == _defaultServer) return;
    settings.serverAddress = server;
  }

  /// "Use this relay": the server is confirmed on this device and taken for this run.
  Future<void> confirmAndUse(String server) async {
    await settings.confirmServer(server);
    useServer(server);
  }
}

/// What starts the app: the link first, else (on the first launch only) the install referrer.
Future<LinkOutcome> resolveLaunch({
  required AppLinks links,
  required LinkHandler handler,
  required SettingsStore? store,
}) async {
  final link = await links.initialLink();
  if (link != null) {
    final o = handler.fromUrl(link);
    if (o is! LinkNothing) return o;
  }
  if (store == null || await store.loadReferrerRead()) {
    return const LinkNothing();
  }
  final referrer = await links.installReferrer();
  // Read once per install, found or not: the referrer exists on the first launch only.
  await store.saveReferrerRead();
  if (referrer == null) return const LinkNothing();
  return handler.fromReferrer(referrer);
}
