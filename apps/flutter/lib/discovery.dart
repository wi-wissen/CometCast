// SPDX-License-Identifier: GPL-3.0-or-later
// Server discovery in the UI: where a suggested server came from, and the list of receivers
// that announce themselves in the local network. The search itself is done by the Rust core
// (crate cc-discovery); see docs/discovery.md.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';

/// Where a suggested server address comes from (the source names are those of Rust).
enum ServerSourceKind {
  /// Policy file of an administrator (MDM).
  policy,

  /// DNS-SRV record of the network.
  dns,

  /// Announced by a receiving display in the local network.
  announced,

  /// Built-in default list.
  defaultList,

  /// The server that was used the last time (kept on this device).
  lastUsed,

  /// The development server on this computer.
  development,

  /// A source name this version does not know.
  unknown;

  static ServerSourceKind fromRust(String source) => switch (source) {
    'mdm' => policy,
    'dns-srv' => dns,
    'display-announcement' => announced,
    'default-list' => defaultList,
    _ => unknown,
  };
}

/// One suggested server: the address and where it came from.
@immutable
class ServerSuggestion {
  const ServerSuggestion(this.address, this.kind);

  ServerSuggestion.fromCandidate(ServerCandidate c)
    : this(c.address, ServerSourceKind.fromRust(c.source));

  final String address;
  final ServerSourceKind kind;

  @override
  bool operator ==(Object other) =>
      other is ServerSuggestion &&
      other.address == address &&
      other.kind == kind;

  @override
  int get hashCode => Object.hash(address, kind);

  @override
  String toString() => 'ServerSuggestion($address, ${kind.name})';
}

/// A receiver that announced itself in the local network.
@immutable
class NearbyReceiver {
  const NearbyReceiver({required this.id, required this.name, this.server});

  final String id;
  final String name;

  /// The server it prefers, if it announced a valid one. Not trusted: the person chooses.
  final String? server;
}

/// The receivers announced in the local network (mDNS), while this object is alive.
/// A receiver never announces its room code; the list only helps to pick a name and a server.
class NearbyReceivers extends ChangeNotifier {
  NearbyReceivers(this._core);

  /// More than this are not shown (a hostile network could announce thousands).
  static const maxShown = 32;

  final CoreApi _core;
  final _byId = <String, NearbyReceiver>{};
  StreamSubscription<NearbyReceiverEvent>? _subscription;
  bool _unavailable = false;
  bool _disposed = false;

  /// In the order in which they were found.
  List<NearbyReceiver> get receivers => List.unmodifiable(_byId.values);

  /// Listening is not possible on this system.
  bool get unavailable => _unavailable;

  void start() {
    if (_subscription != null || _disposed) return;
    _subscription = _core.watchReceivers().listen(
      _onEvent,
      onError: (Object _) => _setUnavailable(),
    );
  }

  void _setUnavailable() {
    if (_disposed) return;
    _unavailable = true;
    notifyListeners();
  }

  void _onEvent(NearbyReceiverEvent e) {
    if (_disposed) return;
    switch (e) {
      case NearbyReceiverEvent_Found(:final id, :final name, :final server):
        if (!_byId.containsKey(id) && _byId.length >= maxShown) return;
        _byId[id] = NearbyReceiver(id: id, name: name, server: server);
      case NearbyReceiverEvent_Lost(:final id):
        _byId.remove(id);
      case NearbyReceiverEvent_Unavailable():
        _unavailable = true;
    }
    notifyListeners();
  }

  /// Stops listening (here and in the core).
  @override
  void dispose() {
    _disposed = true;
    final started = _subscription != null;
    _subscription?.cancel();
    _subscription = null;
    if (started) {
      _core.stopWatchingReceivers().catchError((Object _) {});
    }
    super.dispose();
  }
}
