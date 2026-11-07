// SPDX-License-Identifier: GPL-3.0-or-later
// The state of the section "screens on this network" (spec 14 of
// docs/design/first-run-and-display.md): the rows, the states of 14.2, when the search runs
// (14.5), the tap guard of 14.3. The list itself is kept and checked by the Rust core
// (crates/cc-discovery, nearby.rs); this controller only follows its events. No widgets, no
// strings. Nothing here picks a screen or saves a server (14.8, point 11).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';
import 'core/nearby_platform.dart';

/// What the section shows (spec 14.2).
enum NearbyStatus {
  /// The section is not shown and no search runs (policy, a code already known, ...).
  hidden,

  /// The platform is being asked what it can do; only the label is shown.
  resolving,

  /// "Not asked yet": a permission is needed and was never asked (iOS).
  notAsked,

  /// The search runs, nothing found yet, less than 8 s.
  searching,

  /// At least one screen.
  found,

  /// The search ran 8 s and found nothing; it goes on.
  none,

  /// Phones and tablets: the search ran 60 s.
  paused,

  /// The system permission was refused.
  denied,

  /// No Wi-Fi or Ethernet.
  noNetwork,
}

/// One row: the screen as the core reported it and when it appeared, moved or changed.
class NearbyRow {
  NearbyRow(this.screen, this.changedAt);

  NearbyScreenInfo screen;
  DateTime changedAt;

  String get id => screen.id;
  bool get locked => screen.mode == NearbyMode.locked;
}

class NearbyController extends ChangeNotifier {
  NearbyController({
    required this.core,
    required this.platform,
    DateTime Function()? now,
    this.noneAfter = const Duration(seconds: 8),
    this.pauseAfter = const Duration(seconds: 60),
    this.tapGuard = const Duration(milliseconds: 500),
  }) : now = now ?? DateTime.now;

  final CoreApi core;
  final NearbyPlatform platform;
  final DateTime Function() now;

  /// Spec 14.2: "Nothing found" after this long.
  final Duration noneAfter;

  /// Spec 14.5: phones and tablets stop after this long.
  final Duration pauseAfter;

  /// Spec 14.3: a row that appeared, moved or changed ignores a tap for this long.
  final Duration tapGuard;

  /// At most this many rows are shown (the core keeps 32 as well).
  static const maxRows = 32;

  final List<NearbyRow> _rows = [];
  StreamSubscription<NearbyUiEvent>? _sub;
  StreamSubscription<bool>? _netSub;
  Timer? _noneTimer;
  Timer? _pauseTimer;
  int _run = 0;
  bool _disposed = false;

  bool _enabled = false;
  bool _active = false;
  bool _forbidden = false;
  bool _resolved = false;
  bool _searching = false;
  bool _network = true;
  bool _noneShown = false;
  bool _paused = false;
  bool _unavailable = false;
  NearbyPermission _permission = NearbyPermission.granted;
  String? _connectingId;
  bool _goneLine = false;

  /// The rows in the order they were first found. New rows are added at the end; rows never
  /// re-sort (spec 14.3).
  List<NearbyRow> get rows => List.unmodifiable(_rows);

  /// The id of the row that is being connected (its row shows the spinner, the others are
  /// disabled), or null.
  String? get connectingId => _connectingId;

  /// `nearby.gone` is shown under the list (spec 14.4).
  bool get goneLine => _goneLine;

  NearbyStatus get status {
    if (!_enabled || _forbidden) return NearbyStatus.hidden;
    if (_rows.isNotEmpty && _connectingId != null) return NearbyStatus.found;
    if (!_resolved) return NearbyStatus.resolving;
    if (!_network) return NearbyStatus.noNetwork;
    if (_permission == NearbyPermission.denied) return NearbyStatus.denied;
    if (_paused) return NearbyStatus.paused;
    if (_permission == NearbyPermission.notAsked && !_searching) {
      return NearbyStatus.notAsked;
    }
    if (_rows.isNotEmpty) return NearbyStatus.found;
    if (_noneShown || _unavailable) return NearbyStatus.none;
    return NearbyStatus.searching;
  }

  /// The search runs now.
  bool get searching => _searching;

  /// The page asked for the search ([start] was called and [stop] was not).
  bool get active => _active;

  /// The page goes to the relay confirmation: nothing runs, but the rows stay so that "Use this
  /// relay" can connect to the row that was tapped (spec 14.4). The next [start] clears them.
  void freeze() {
    _active = false;
    _run++;
    _teardown(clearRows: false);
    _netSub?.cancel();
    _netSub = null;
    _goneLine = false;
    if (!_disposed) notifyListeners();
  }

  // --- switching on and off ---------------------------------------------------------------

  /// The section may be shown (false: policy forbids it, the page already has a code, or the
  /// page is not a presenter start page). Turning it off stops everything; turning it on starts
  /// the search if the page is [start]ed.
  void setEnabled(bool enabled) {
    if (_enabled == enabled || _disposed) return;
    _enabled = enabled;
    if (!enabled) {
      _teardown(clearRows: true);
      _resolved = false;
      notifyListeners();
    } else if (_active) {
      unawaited(start());
    } else {
      notifyListeners();
    }
  }

  /// The start page is shown, the app is in the foreground, or Wi-Fi came: (re)start. Rows from
  /// an earlier search are never kept (14.8, point 9).
  Future<void> start({bool keepGoneLine = false}) async {
    if (_disposed) return;
    _active = true;
    if (!keepGoneLine) _goneLine = false;
    if (!_enabled) return;
    final run = ++_run;
    _teardown(clearRows: true);
    _connectingId = null;
    _paused = false;
    _unavailable = false;
    _forbidden = false;
    _noneShown = false;
    _netSub ??= platform.networkChanges.listen(_onNetwork);
    notifyListeners();
    final net = await platform.hasLocalNetwork();
    if (run != _run || _disposed) return;
    _network = net;
    if (!net) {
      _resolved = true;
      notifyListeners();
      return;
    }
    final perm = await platform.permission();
    if (run != _run || _disposed) return;
    _permission = perm;
    _resolved = true;
    if (perm == NearbyPermission.notAsked) {
      notifyListeners();
      return;
    }
    _begin(run);
  }

  /// The page is left (any tap on a row, Connect, Scan), the app goes to the background, or
  /// the device lost the local network: nothing runs and no row is kept.
  void stop() {
    _active = false;
    _run++;
    _teardown(clearRows: true);
    _netSub?.cancel();
    _netSub = null;
    _connectingId = null;
    _goneLine = false;
    _resolved = false;
    if (!_disposed) notifyListeners();
  }

  /// The compact button `nearby.search`: asks the platform for its permission (a system dialog
  /// only as the direct result of this tap) or starts another 60 s.
  void searchNow() {
    if (_disposed || !_enabled) return;
    if (status == NearbyStatus.notAsked) {
      final run = ++_run;
      _begin(run);
      return;
    }
    unawaited(start());
  }

  /// `nearby.allow`: open the system settings page of the permission.
  Future<bool> openSettings() => platform.openSettings();

  // --- the search ------------------------------------------------------------------------

  void _begin(int run) {
    if (run != _run || _disposed) return;
    _teardown(clearRows: true);
    _searching = true;
    _paused = false;
    _noneShown = false;
    _unavailable = false;
    _sub = core
        .watchNearbyScreens(native: platform.nativeBrowse)
        .listen(_onEvent, onError: (Object _) => _setUnavailable());
    _noneTimer = Timer(noneAfter, () {
      if (_disposed) return;
      _noneShown = true;
      notifyListeners();
    });
    if (platform.limitedTime) {
      _pauseTimer = Timer(pauseAfter, _pause);
    }
    notifyListeners();
  }

  void _pause() {
    if (_disposed) return;
    _teardown(clearRows: true);
    _paused = true;
    notifyListeners();
  }

  void _setUnavailable() {
    if (_disposed) return;
    _unavailable = true;
    notifyListeners();
  }

  void _onEvent(NearbyUiEvent e) {
    if (_disposed) return;
    switch (e) {
      case NearbyUiEvent_SearchStarted():
        // The core has its feed now: the platform may start browsing (on iOS this is what makes
        // the system ask).
        unawaited(platform.startBrowse());
        return;
      case NearbyUiEvent_SearchStopped():
        return;
      case NearbyUiEvent_Added(:final screen):
        _put(screen);
      case NearbyUiEvent_Updated(:final screen):
        _put(screen);
      case NearbyUiEvent_Removed(:final id):
        final i = _rows.indexWhere((r) => r.id == id);
        if (i < 0) return;
        _rows.removeAt(i);
        // The rows below moved: they ignore a tap for a moment (spec 14.3).
        final t = now();
        for (var k = i; k < _rows.length; k++) {
          _rows[k].changedAt = t;
        }
      case NearbyUiEvent_Permission(:final state):
        _permission = state;
        if (state == NearbyPermission.denied) {
          _teardown(clearRows: true);
        }
      case NearbyUiEvent_Unavailable():
        _unavailable = true;
      case NearbyUiEvent_Forbidden():
        _forbidden = true;
        _teardown(clearRows: true);
    }
    notifyListeners();
  }

  void _put(NearbyScreenInfo s) {
    final i = _rows.indexWhere((r) => r.id == s.id);
    final t = now();
    if (i >= 0) {
      _rows[i]
        ..screen = s
        ..changedAt = t;
    } else if (_rows.length < maxRows) {
      _rows.add(NearbyRow(s, t));
    }
  }

  void _onNetwork(bool has) {
    if (_disposed || !_active || has == _network) return;
    _network = has;
    if (has) {
      unawaited(start());
    } else {
      _teardown(clearRows: true);
      _resolved = true;
      notifyListeners();
    }
  }

  void _teardown({required bool clearRows}) {
    final was = _searching || _sub != null;
    _noneTimer?.cancel();
    _pauseTimer?.cancel();
    _noneTimer = null;
    _pauseTimer = null;
    _sub?.cancel();
    _sub = null;
    _searching = false;
    if (was) {
      unawaited(core.stopNearbyScreens().catchError((Object _) {}));
      unawaited(platform.stopBrowse().catchError((Object _) {}));
    }
    if (clearRows) _rows.clear();
  }

  // --- tapping a row ---------------------------------------------------------------------

  /// May [row] be tapped now? Not a locked row, not while another connects, and not within the
  /// guard time after it appeared, moved or changed (spec 14.3).
  bool canTap(NearbyRow row) =>
      !row.locked &&
      _connectingId == null &&
      now().difference(row.changedAt) >= tapGuard;

  /// A row was tapped: the search stops, the row shows the spinner, the others are disabled,
  /// the rows stay as they are (spec 14.4). The status line goes away.
  void beginConnect(String id) {
    _connectingId = id;
    _goneLine = false;
    _run++;
    _teardown(clearRows: false);
    notifyListeners();
  }

  /// The connection attempt is over (the page is about to change or search again).
  void endConnect() {
    _connectingId = null;
    notifyListeners();
  }

  /// `room_not_found` after a tap: the row is removed and `nearby.gone` shows under the list
  /// (spec 14.4). The caller starts the search again with [start].
  void markGone(String id) {
    _rows.removeWhere((r) => r.id == id);
    _connectingId = null;
    _goneLine = true;
    notifyListeners();
  }

  /// Start again after [markGone] keeping the status line: the refilling list does not remove
  /// it (decision N51); it goes with the next tap on the page ([clearGoneLine]), [beginConnect],
  /// [stop], or a [start] that is not this one.
  Future<void> restartKeepingLine() => start(keepGoneLine: true);

  /// A tap anywhere on the page (row, field, Scan, Connect) removes `nearby.gone`.
  void clearGoneLine() {
    if (!_goneLine || _disposed) return;
    _goneLine = false;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _run++;
    _teardown(clearRows: true);
    _netSub?.cancel();
    _netSub = null;
    super.dispose();
  }
}
