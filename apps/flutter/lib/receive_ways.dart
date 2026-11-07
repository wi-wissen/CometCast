// SPDX-License-Identifier: GPL-3.0-or-later
// The "Without the app" ways of a desktop display (spec 15.4, N86 to N89, native-apps.md 21.4):
// receiving AirPlay (`cc-ffi`, UxPlay) and Miracast (`cc-miracast-ffi`). One object that owns
// both receivers, follows the join mode (Open: PIN at once; Approval on: request first; Locked:
// not announced, a running mirror stays) and Comet Cast first (N88). No widgets and no strings;
// the display root draws what this object says and shows the toasts for its notices.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';
import 'core/miracast.dart';
import 'miracast_controller.dart';
import 'settings.dart';
import 'src/display/screens.dart' show DisplayJoinMode;

enum RxWay { airplay, miracast }

/// A device asks to mirror and the host is asked first (Approval on). [name] is null for AirPlay
/// (UxPlay does not name the device).
class RxRequest {
  const RxRequest({required this.way, this.name, this.id});
  final RxWay way;
  final String? name;
  final int? id;
}

/// The PIN the device must enter. Only a PIN from the core's PIN event is ever shown.
class RxPin {
  const RxPin({required this.way, required this.pin, this.name});
  final RxWay way;
  final String pin;
  final String? name;
}

sealed class RxNotice {
  const RxNotice();
}

/// A Comet Cast presentation ended the mirror (`toast.rxEnded`).
class RxEnded extends RxNotice {
  const RxEnded(this.way);
  final RxWay way;
}

/// The Miracast device stopped (`toast.left`).
class RxLeft extends RxNotice {
  const RxLeft(this.name);
  final String name;
}

/// A request left the queue unanswered (`toast.expired`).
class RxExpired extends RxNotice {
  const RxExpired(this.name);
  final String name;
}

/// What a switch row shows.
enum RxLine { normal, noHw, noFeature, policy, failed, locked }

class RxRow {
  const RxRow({required this.on, required this.disabled, required this.line});

  /// The switch as drawn.
  final bool on;

  /// Switched off and not focusable (no hardware, no feature, policy).
  final bool disabled;
  final RxLine line;
}

class ReceiveWays extends ChangeNotifier {
  ReceiveWays({
    required this.core,
    required this.settings,
    this.miracastCore,
    this.pinTimeout = const Duration(seconds: 30),
    this.onNotice,
    MiracastController? miracastController,
  }) : _mc =
           miracastController ??
           (miracastCore == null
               ? null
               : MiracastController(
                   core: miracastCore,
                   pairing: displayMiracastPairing,
                 )) {
    _mc?.addListener(_onMiracast);
  }

  final CoreApi core;
  final AppSettings settings;
  final MiracastCore? miracastCore;
  final Duration pinTimeout;
  final void Function(RxNotice notice)? onNotice;
  final MiracastController? _mc;

  bool _disposed = false;
  String _name = 'Comet Cast';
  DisplayJoinMode _mode = DisplayJoinMode.open;
  bool _ccPresenting = false;
  bool _active = false;

  // --- AirPlay ---
  StreamSubscription<AirplayUiEvent>? _apSub;
  bool _apRunning = false;
  bool _apFailed = false;
  bool _apMirroring = false;
  bool? _apAvailable;

  // --- Miracast ---
  bool _mcStarted = false;
  bool _mcProbed = false;
  String? _mcConnectedName;
  int? _seenId;
  MiracastRequest? _lastPending;

  RxRequest? _request;
  RxPin? _pin;
  Timer? _pinTimer;

  /// Rows exist for the ways this device can do at all (spec 15.4): AirPlay where UxPlay is
  /// shipped, Miracast where the Windows receiver is present.
  bool get airplayListed => _apAvailable ??= _checkAirplay();
  bool get miracastListed => _mc != null;
  bool get anyListed => airplayListed || miracastListed;

  bool _checkAirplay() {
    try {
      return core.airplayReceiveAvailable();
    } catch (_) {
      return false;
    }
  }

  MiracastController? get miracast => _mc;

  RxRequest? get request => _request;
  RxPin? get pin => _pin;

  /// The AirPlay picture is shown by UxPlay's own window.
  bool get airplayMirroring => _apMirroring;

  /// The Miracast picture is drawn by the app.
  bool get miracastConnected => _mc?.phase == MiracastPhase.connected;

  /// The name of the connected Miracast device.
  String? get miracastName => _mc?.connectedName;

  /// A receiver runs and is offered (idle line, spec 15.4): switch on, started, not Locked.
  bool get airplayAnnouncing => _apRunning && _mode != DisplayJoinMode.locked;
  bool get miracastAnnouncing =>
      _mcStarted &&
      (_mc?.phase == MiracastPhase.listening ||
          _mc?.phase == MiracastPhase.consent ||
          _mc?.phase == MiracastPhase.connected) &&
      _mode != DisplayJoinMode.locked;

  RxRow row(RxWay way) {
    switch (way) {
      case RxWay.airplay:
        final on = settings.airplayReceive;
        if (_apFailed && on) {
          return const RxRow(on: true, disabled: false, line: RxLine.failed);
        }
        return RxRow(
          on: on,
          disabled: false,
          line: on && _mode == DisplayJoinMode.locked
              ? RxLine.locked
              : RxLine.normal,
        );
      case RxWay.miracast:
        final on = settings.miracastReceive;
        final issue = _mc?.issue;
        if (issue != null &&
            (_mc!.phase == MiracastPhase.idle ||
                _mc.phase == MiracastPhase.problem)) {
          final line = switch (miracastStateLine(issue)) {
            MiracastStateLine.noHw => RxLine.noHw,
            MiracastStateLine.noFeature => RxLine.noFeature,
            MiracastStateLine.policy => RxLine.policy,
            MiracastStateLine.failed => RxLine.failed,
          };
          if (line == RxLine.failed) {
            return RxRow(on: on, disabled: false, line: RxLine.failed);
          }
          return RxRow(on: false, disabled: true, line: line);
        }
        if (_mc?.phase == MiracastPhase.problem && on) {
          return const RxRow(on: true, disabled: false, line: RxLine.failed);
        }
        return RxRow(
          on: on,
          disabled: false,
          line: on && _mode == DisplayJoinMode.locked
              ? RxLine.locked
              : RxLine.normal,
        );
    }
  }

  // --- life --------------------------------------------------------------------------------

  /// Starts what is switched on and probes Miracast (to know `noHw`, `noFeature`, `policy`
  /// before the person switches it on).
  Future<void> start({
    required String roomName,
    required DisplayJoinMode mode,
  }) async {
    _active = true;
    _name = roomName.trim().isEmpty ? 'Comet Cast' : roomName.trim();
    _mode = mode;
    if (_mc != null && !_mcProbed) {
      _mcProbed = true;
      await _mc.init();
    }
    _apply();
    _notify();
  }

  /// The display left its role or the app closes.
  void stopAll() {
    _active = false;
    _stopAirplay();
    unawaited(_stopMiracast());
    _clearDialogs();
    _notify();
  }

  void setRoomName(String roomName) {
    final n = roomName.trim().isEmpty ? 'Comet Cast' : roomName.trim();
    if (n == _name) return;
    _name = n;
  }

  void setMode(DisplayJoinMode mode) {
    if (mode == _mode) return;
    _mode = mode;
    if (_apRunning || _apSub != null) {
      core.airplaySetAskFirst(mode == DisplayJoinMode.confirm);
      core.airplaySetLocked(mode == DisplayJoinMode.locked);
    }
    _apply();
    _notify();
  }

  /// A Comet Cast presenter presents (N88): pairings are refused without any dialog; when it
  /// starts, a running AirPlay or Miracast session ends (toast `toast.rxEnded`).
  void setCometCastPresenting(bool presenting) {
    if (presenting == _ccPresenting) return;
    _ccPresenting = presenting;
    if (_apSub != null) core.airplaySetRefusing(presenting);
    if (presenting) {
      _clearDialogs();
      if (_apMirroring) {
        core.airplayEndSession();
        _apMirroring = false;
        onNotice?.call(const RxEnded(RxWay.airplay));
      }
      if (miracastConnected) {
        unawaited(_mc!.disconnect());
        onNotice?.call(const RxEnded(RxWay.miracast));
      }
    }
    _notify();
  }

  /// The switch of a row.
  void setOn(RxWay way, bool on) {
    switch (way) {
      case RxWay.airplay:
        if (on == settings.airplayReceive && !(on && _apFailed)) return;
        settings.airplayReceive = on;
        _apFailed = false;
        _stopAirplay();
      case RxWay.miracast:
        if (on == settings.miracastReceive && !(on && _failedMiracast)) return;
        settings.miracastReceive = on;
        unawaited(_stopMiracast());
    }
    _apply();
    _notify();
  }

  bool get _failedMiracast => _mc?.phase == MiracastPhase.problem;

  // --- answering ---------------------------------------------------------------------------

  /// The request dialog was answered.
  void answerRequest(bool allow) {
    final r = _request;
    if (r == null) return;
    _request = null;
    switch (r.way) {
      case RxWay.airplay:
        core.airplayAnswer(allow);
      case RxWay.miracast:
        final id = r.id;
        if (id != null) {
          unawaited(_mc?.respond(id, allow) ?? Future<bool>.value(false));
          if (allow) _showPin(RxWay.miracast, _lastPending?.pin, r.name);
        }
    }
    _notify();
  }

  /// The PIN dialog was cancelled: the pairing is refused.
  void cancelPin() {
    final p = _pin;
    if (p == null) return;
    _clearPin();
    switch (p.way) {
      case RxWay.airplay:
        core.airplayAnswer(false);
      case RxWay.miracast:
        unawaited(_mc?.disconnect() ?? Future<void>.value());
    }
    _notify();
  }

  /// End (the host) while Miracast mirrors: the mirror stops.
  Future<void> endMiracast() async {
    await _mc?.disconnect();
  }

  // --- the receivers -----------------------------------------------------------------------

  void _apply() {
    if (!_active) return;
    // AirPlay: runs while the switch is on; Locked pauses the service inside the core.
    if (airplayListed && settings.airplayReceive) {
      if (_apSub == null && !_apFailed) _startAirplay();
    } else {
      _stopAirplay();
    }
    // Miracast: runs while the switch is on and the mode is not Locked (a running mirror stays).
    final wantMiracast =
        miracastListed &&
        settings.miracastReceive &&
        (_mode != DisplayJoinMode.locked || miracastConnected) &&
        _mc!.phase != MiracastPhase.problem;
    if (wantMiracast) {
      if (!_mcStarted) {
        _mcStarted = true;
        _mc.displayName = _name;
        unawaited(_mc.start());
      }
    } else if (_mcStarted && !miracastConnected) {
      unawaited(_stopMiracast());
    }
  }

  void _startAirplay() {
    _apFailed = false;
    _apSub = core
        .startAirplayReceive(
          name: _name,
          askFirst: _mode == DisplayJoinMode.confirm,
        )
        .listen(
          _onAirplay,
          onError: (Object _) {
            _apFailed = true;
            _stopAirplay();
            _notify();
          },
        );
    if (_mode == DisplayJoinMode.locked) core.airplaySetLocked(true);
    if (_ccPresenting) core.airplaySetRefusing(true);
  }

  void _stopAirplay() {
    final had = _apSub != null;
    _apSub?.cancel();
    _apSub = null;
    _apRunning = false;
    final wasMirroring = _apMirroring;
    _apMirroring = false;
    if (_request?.way == RxWay.airplay) _request = null;
    if (_pin?.way == RxWay.airplay) _clearPin();
    if (had) core.stopAirplayReceive();
    if (wasMirroring) _notify();
  }

  Future<void> _stopMiracast() async {
    final was = _mcStarted;
    _mcStarted = false;
    if (_request?.way == RxWay.miracast) _request = null;
    if (_pin?.way == RxWay.miracast) _clearPin();
    if (was || _mc?.phase != MiracastPhase.idle) {
      await _mc?.stop();
    }
  }

  void _onAirplay(AirplayUiEvent e) {
    if (_disposed) return;
    switch (e) {
      case AirplayUiEvent_Advertising():
        _apRunning = true;
        _apFailed = false;
      case AirplayUiEvent_NotAnnouncing():
        _apRunning = false;
      case AirplayUiEvent_Request():
        if (_ccPresenting) {
          core.airplayAnswer(false);
        } else {
          _request = const RxRequest(way: RxWay.airplay);
        }
      case AirplayUiEvent_Pin(:final pin):
        if (_ccPresenting) {
          core.airplayAnswer(false);
        } else {
          _request = null;
          _showPin(RxWay.airplay, pin, null);
        }
      case AirplayUiEvent_PinDone():
        if (_request?.way == RxWay.airplay) _request = null;
        if (_pin?.way == RxWay.airplay) _clearPin();
      case AirplayUiEvent_Mirroring(:final active):
        _apMirroring = active;
        if (active && _pin?.way == RxWay.airplay) _clearPin();
      case AirplayUiEvent_Failed():
        _apFailed = true;
        _apRunning = false;
        _apSub?.cancel();
        _apSub = null;
      case AirplayUiEvent_Stopped():
        _apRunning = false;
        _apMirroring = false;
        _apSub?.cancel();
        _apSub = null;
    }
    _notify();
  }

  // --- Miracast events ---------------------------------------------------------------------

  MiracastPhase _lastPhase = MiracastPhase.idle;

  void _onMiracast() {
    if (_disposed) return;
    final c = _mc!;
    final phase = c.phase;
    final pending = c.pending;
    if (pending != null) {
      _lastPending = pending;
      if (pending.id != _seenId) {
        _seenId = pending.id;
        _onMiracastRequest(pending);
      }
    } else if (_request?.way == RxWay.miracast &&
        phase != MiracastPhase.consent) {
      // The request went away unanswered (the device's own timeout, or it left).
      final r = _request!;
      _request = null;
      onNotice?.call(RxExpired(r.name ?? ''));
    }
    if (phase == MiracastPhase.connected) {
      _clearPin();
    }
    if (_lastPhase == MiracastPhase.connected &&
        phase != MiracastPhase.connected) {
      // The device stopped: Idle, `toast.left` with its name.
      final name = _mcConnectedName;
      if (name != null) onNotice?.call(RxLeft(name));
      _mcConnectedName = null;
      if (_mode == DisplayJoinMode.locked) unawaited(_stopMiracast());
    }
    if (phase == MiracastPhase.connected) _mcConnectedName = c.connectedName;
    _lastPhase = phase;
    _notify();
  }

  void _onMiracastRequest(MiracastRequest r) {
    if (_ccPresenting || _mode == DisplayJoinMode.locked) {
      // Comet Cast first (N88) and Locked: refused without any dialog.
      unawaited(_mc!.respond(r.id, false));
      return;
    }
    if (_mode == DisplayJoinMode.open) {
      // Consent answered by the app at once; the PIN dialog follows.
      unawaited(_mc!.respond(r.id, true));
      _showPin(RxWay.miracast, r.pin, r.name);
      return;
    }
    _request = RxRequest(way: RxWay.miracast, name: r.name, id: r.id);
  }

  void _showPin(RxWay way, String? pin, String? name) {
    if (pin == null || pin.isEmpty) return;
    _pin = RxPin(way: way, pin: pin, name: name);
    _pinTimer?.cancel();
    // The dialog closes by itself after 30 s (spec 15.4).
    _pinTimer = Timer(pinTimeout, () {
      if (_pin != null) {
        _pin = null;
        _notify();
      }
    });
    _notify();
  }

  void _clearPin() {
    _pinTimer?.cancel();
    _pinTimer = null;
    _pin = null;
  }

  void _clearDialogs() {
    final r = _request;
    final p = _pin;
    _request = null;
    _clearPin();
    if (r?.way == RxWay.airplay || p?.way == RxWay.airplay) {
      core.airplayAnswer(false);
    }
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _pinTimer?.cancel();
    _apSub?.cancel();
    if (_apSub != null) core.stopAirplayReceive();
    _mc?.removeListener(_onMiracast);
    _mc?.dispose();
    super.dispose();
  }
}
