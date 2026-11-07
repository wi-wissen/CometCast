// SPDX-License-Identifier: GPL-3.0-or-later
// The list of TVs without Comet Cast (spec 15.5, fix list F10): Google Cast and AirPlay devices
// that the core finds (`watch_tvs`). Runs exactly while the nearby search runs and only while
// the switch is on; silent while searching and absent when nothing was found. No widgets and no
// strings. The session to a TV is a normal presenter session (`start_tv_presenter`).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';

class TvRow {
  TvRow({
    required this.id,
    required this.name,
    required this.kind,
    required this.addresses,
    required this.canScreen,
    required this.canPhoto,
    required this.canVideo,
    required this.changedAt,
  });

  final String id;
  final String name;
  final TvKind kind;
  final List<String> addresses;
  final bool canScreen;
  final bool canPhoto;
  final bool canVideo;
  DateTime changedAt;

  /// Google Cast devices are listed when they can show the screen (those without video are
  /// audio-only); AirPlay receivers when they offer video playback or photos (spec 15.5).
  bool get listed => switch (kind) {
    TvKind.cast => canScreen,
    TvKind.airPlay => canPhoto || canVideo,
  };
}

class TvController extends ChangeNotifier {
  TvController({
    required this.core,
    DateTime Function()? now,
    this.tapGuard = const Duration(milliseconds: 500),
  }) : now = now ?? DateTime.now;

  final CoreApi core;
  final DateTime Function() now;
  final Duration tapGuard;

  /// Rows are counted separately from the Comet Cast rows (spec 14.3).
  static const maxRows = 32;

  final List<TvRow> _all = [];
  StreamSubscription<TvUiEvent>? _sub;
  Set<String> _hidden = {};
  String? _connectingId;
  bool _disposed = false;

  /// The rows to show: listed, not hidden by an address of a Comet Cast row (N84), at most 32.
  List<TvRow> get rows {
    final out = <TvRow>[];
    for (final r in _all) {
      if (!r.listed) continue;
      if (r.addresses.map(normaliseAddress).any(_hidden.contains)) continue;
      if (out.length >= maxRows) break;
      out.add(r);
    }
    return List.unmodifiable(out);
  }

  String? get connectingId => _connectingId;

  bool get running => _sub != null;

  /// The addresses of the Comet Cast rows in the nearby list: a TV with one of them is hidden
  /// (N84), so that people pick the Comet Cast row, which honours the join mode.
  void setHiddenAddresses(Set<String> addresses) {
    final norm = {for (final a in addresses) normaliseAddress(a)};
    if (setEquals(norm, _hidden)) return;
    _hidden = norm;
    _notify();
  }

  void start() {
    if (_disposed || _sub != null) return;
    _sub = core.watchTvs().listen(_onEvent, onError: (Object _) {});
  }

  void stop() {
    if (_sub == null && _all.isEmpty && _connectingId == null) return;
    _sub?.cancel();
    _sub = null;
    _all.clear();
    _connectingId = null;
    unawaited(Future<void>(() => core.stopWatchingTvs()));
    _notify();
  }

  void _onEvent(TvUiEvent e) {
    if (_disposed) return;
    switch (e) {
      case TvUiEvent_Seen(
        :final id,
        :final name,
        :final kind,
        :final addresses,
        :final canScreen,
        :final canPhoto,
        :final canVideo,
      ):
        final row = TvRow(
          id: id,
          name: name,
          kind: kind,
          addresses: addresses,
          canScreen: canScreen,
          canPhoto: canPhoto,
          canVideo: canVideo,
          changedAt: now(),
        );
        final i = _all.indexWhere((r) => r.id == id);
        if (i >= 0) {
          _all[i] = row;
        } else {
          _all.add(row);
        }
      case TvUiEvent_Gone(:final id):
        final before = rows;
        _all.removeWhere((r) => r.id == id);
        // The rows below moved: they ignore a tap for a moment (spec 14.3).
        final idx = before.indexWhere((r) => r.id == id);
        if (idx >= 0) {
          final t = now();
          for (final r in rows.skip(idx)) {
            r.changedAt = t;
          }
        }
      case TvUiEvent_Unavailable():
        _sub?.cancel();
        _sub = null;
    }
    _notify();
  }

  bool canTap(TvRow row) =>
      _connectingId == null && now().difference(row.changedAt) >= tapGuard;

  /// A row is tapped: the search stops, the other rows are disabled.
  void beginConnect(String id) {
    _connectingId = id;
    _sub?.cancel();
    _sub = null;
    unawaited(Future<void>(() => core.stopWatchingTvs()));
    _notify();
  }

  void endConnect() {
    if (_connectingId == null) return;
    _connectingId = null;
    _notify();
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _sub?.cancel();
    _sub = null;
    super.dispose();
  }
}

/// An IP address as comparable text: brackets and zone id removed, lower case, an IPv4-mapped
/// IPv6 address as the IPv4 address (the core's `addresses` are normalised the same way).
String normaliseAddress(String a) {
  var t = a.trim().toLowerCase();
  if (t.startsWith('[') && t.endsWith(']')) t = t.substring(1, t.length - 1);
  final zone = t.indexOf('%');
  if (zone >= 0) t = t.substring(0, zone);
  final mapped = RegExp(r'^::ffff:(\d{1,3}(?:\.\d{1,3}){3})$').firstMatch(t);
  return mapped == null ? t : mapped.group(1)!;
}

/// The addresses of the Comet Cast rows of the nearby list that hide a TV row (spec 15.14 N105):
/// every address the announcement resolved to and the host of the row's `server`.
Set<String> hiddenTvAddresses(Iterable<NearbyScreenInfo> screens) {
  final out = <String>{};
  for (final s in screens) {
    out.addAll(s.addresses.map(normaliseAddress));
    final host = Uri.tryParse(s.server)?.host ?? '';
    if (host.isNotEmpty) out.add(normaliseAddress(host));
  }
  return out;
}
