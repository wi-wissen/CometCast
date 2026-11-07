// SPDX-License-Identifier: GPL-3.0-or-later
// Links, the install referrer and scanned QR codes (native-apps.md 20.4, spec 5.3 and 7.2):
// which room code and which server they name, and whether that server may be used at once or
// has to be confirmed first. No widgets and no strings.

import '../discovery.dart';

/// The room code and the server that a link, a referrer or a scanned QR code names.
class LinkTarget {
  const LinkTarget({required this.code, required this.server});

  /// The 6-character code, normalised (uppercase, no spaces or dashes).
  final String code;

  /// `host` or `host:port` (the default port 443 is left out, as `{dom}` is everywhere).
  final String server;

  @override
  bool operator ==(Object other) =>
      other is LinkTarget && other.code == code && other.server == server;

  @override
  int get hashCode => Object.hash(code, server);

  @override
  String toString() => 'LinkTarget($code @ $server)';
}

/// Normalises a room code typed or read from a link (spaces and dashes removed, uppercase) and
/// checks it: the callback is the core's `check_room_code` (the Rust code is the single source of
/// truth for the alphabet). Null = not a code.
typedef CodeNormalizer = String? Function(String raw);

/// `host` or `host:port`, lower case, without the default port 443, or null when [raw] is not a
/// plain server address (no scheme, path, user name or spaces).
String? canonicalServer(String raw) {
  final s = raw.trim().toLowerCase();
  final m = RegExp(
    r'^([a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*)(:([0-9]{1,5}))?$',
  ).firstMatch(s);
  if (m == null) return null;
  final host = m.group(1)!;
  final portText = m.group(6);
  if (host.length > 253) return null;
  if (portText == null) return host;
  final port = int.parse(portText);
  if (port < 1 || port > 65535) return null;
  return port == 443 ? host : '$host:$port';
}

/// The code and server of an accepted URL form: `https://{host}/{CODE}` and
/// `https://{host}/{CODE}?server={server}`. The server is `{server}` if given, else `{host}`.
/// Anything else (another scheme, no code, a code of the wrong length or alphabet, a bad
/// `server` parameter, a user name in the URL) is null: no code.
LinkTarget? parseLinkUrl(String text, CodeNormalizer normalize) {
  final uri = Uri.tryParse(text.trim());
  if (uri == null || uri.scheme.toLowerCase() != 'https') return null;
  if (uri.host.isEmpty || uri.userInfo.isNotEmpty || uri.hasFragment) {
    return null;
  }
  final segments = uri.pathSegments.where((s) => s.isNotEmpty).toList();
  if (segments.length != 1) return null;
  final code = normalize(Uri.decodeComponent(segments.single));
  if (code == null) return null;
  final hostPort = uri.hasPort ? '${uri.host}:${uri.port}' : uri.host;
  final server = _serverParam(uri.queryParametersAll, hostPort);
  return server == null ? null : LinkTarget(code: code, server: server);
}

/// The code and server of an install referrer (`code=K7QM2W&server=cast.example.org`, the
/// `referrer` string of the Google Play install referrer). Same rules; without `server` the
/// server is [defaultServer] (the built-in relay).
LinkTarget? parseReferrer(
  String referrer,
  CodeNormalizer normalize, {
  required String defaultServer,
}) {
  final Map<String, List<String>> params;
  try {
    params = Uri(query: referrer.trim()).queryParametersAll;
  } catch (_) {
    return null;
  }
  final codes = params['code'];
  if (codes == null || codes.length != 1) return null;
  final code = normalize(codes.single);
  if (code == null) return null;
  final server = _serverParam(params, defaultServer);
  return server == null ? null : LinkTarget(code: code, server: server);
}

/// The `server` parameter if given (exactly once, a plain address), else [fallback]; null when
/// the parameter is given but not acceptable.
String? _serverParam(Map<String, List<String>> params, String fallback) {
  final given = params['server'];
  if (given == null) return canonicalServer(fallback);
  if (given.length != 1) return null;
  return canonicalServer(given.single);
}

/// What to do with a server that a link, the referrer or a scanned code names.
enum ServerDecision {
  /// Use it at once (the code chip).
  use,

  /// Show the relay confirmation page first (spec 7.2): nothing connects before.
  confirm,
}

/// Which servers are used without asking: the built-in relay, the policy server, the DNS
/// server, the in-house box (an announcement of a display) and every server the person
/// confirmed on this device before. Anything else has to be confirmed.
class ServerTrust {
  const ServerTrust({
    this.suggestions = const [],
    this.confirmed = const {},
    this.builtIn = const {},
  });

  /// The servers the app knows from its sources (policy, DNS, announcements, built-in list).
  final List<ServerSuggestion> suggestions;

  /// Confirmed on this device (canonical addresses).
  final Set<String> confirmed;

  /// More built-in addresses (the app's own constants).
  final Set<String> builtIn;

  bool trusts(String server) {
    final s = canonicalServer(server);
    if (s == null) return false;
    if (confirmed.contains(s) || builtIn.contains(s)) return true;
    for (final x in suggestions) {
      final trusted = switch (x.kind) {
        ServerSourceKind.policy ||
        ServerSourceKind.dns ||
        ServerSourceKind.announced ||
        ServerSourceKind.defaultList => true,
        _ => false,
      };
      if (trusted && canonicalServer(x.address) == s) return true;
    }
    return false;
  }

  ServerDecision decide(String server) =>
      trusts(server) ? ServerDecision.use : ServerDecision.confirm;
}

/// What the first-run page gets from a link, the referrer or a scan.
sealed class LinkOutcome {
  const LinkOutcome();
}

/// Nothing usable: not our URL form, no valid code. The page changes nothing (a scan shows
/// `app.scanWrong`).
class LinkNothing extends LinkOutcome {
  const LinkNothing();
}

/// A valid code for a server that may be used at once: the chip.
class LinkUse extends LinkOutcome {
  const LinkUse(this.target);
  final LinkTarget target;
}

/// A valid code for a server this device has not confirmed: the relay page first (spec 7.2);
/// Cancel continues with the server the app would use anyway (the chip keeps the code), Use
/// remembers the server as confirmed and takes it.
class LinkConfirm extends LinkOutcome {
  const LinkConfirm(this.target);
  final LinkTarget target;
}

LinkOutcome outcomeFor(LinkTarget? target, ServerTrust trust) {
  if (target == null) return const LinkNothing();
  return switch (trust.decide(target.server)) {
    ServerDecision.use => LinkUse(target),
    ServerDecision.confirm => LinkConfirm(target),
  };
}
