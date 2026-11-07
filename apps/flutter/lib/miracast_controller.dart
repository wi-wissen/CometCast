// SPDX-License-Identifier: GPL-3.0-or-later
// The state machine of the Windows Miracast receive screen. No widgets in here, and no
// native code: it talks to a [MiracastCore] (the DLL, or a fake in tests) and is polled.
//
// Flow:  idle --start--> starting --listening--> listening
//        listening --sender asks--> consent (the person decides) --yes--> connected
//        connected --disconnect / sender leaves--> listening --stop--> idle
// Rules that the screen relies on:
//  * the video is only exposed while the status is `connected` (nothing is shown before the
//    person has said yes);
//  * a request is answered at most once, and a stale one (timed out on the Rust side) is not
//    answered at all;
//  * errors from the core never throw out of this class; they become a visible problem.
//
// UNCERTAIN: never run against the real receiver (docs/miracast-device-test-plan.md).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/miracast.dart';

/// The screen's view of the receiver, derived from the core state and local facts.
enum MiracastPhase {
  /// Nothing started. The start button is shown.
  idle,

  /// Start was requested; Windows has not said "listening" yet.
  starting,

  /// Visible to senders.
  listening,

  /// A sender waits for the person's decision (consent dialog).
  consent,

  /// A sender is connected; the video is shown.
  connected,

  /// Stop was requested.
  stopping,

  /// Something prevents receiving; see [MiracastController.issue].
  problem,
}

class MiracastController extends ChangeNotifier {
  MiracastController({
    required this.core,
    this.displayName = 'Comet Cast',
    this.pairing = MiracastPairing.confirm,
    this.pollInterval = const Duration(milliseconds: 16),
    this.statePollInterval = const Duration(milliseconds: 100),
  });

  final MiracastCore core;

  /// Name shown in the sender's list. Settable until started.
  String displayName;
  MiracastPairing pairing;
  final Duration pollInterval;
  final Duration statePollInterval;

  MiracastState _state = MiracastState.off;
  MiracastProbe? _probe;
  MiracastIssue? _libraryIssue;
  bool _starting = false;
  bool _disposed = false;
  bool _busy = false;
  MiracastFrame? _frame;
  int _framesSeen = 0;
  Timer? _timer;
  int _ticks = 0;

  /// Last state reported by the core.
  MiracastState get state => _state;
  MiracastProbe? get probe => _probe;

  /// The newest picture, only while connected (null otherwise).
  MiracastFrame? get frame => phase == MiracastPhase.connected ? _frame : null;

  /// Number of pictures received in this session (grows with every new one).
  int get framesSeen => _framesSeen;

  /// The waiting request, if any.
  MiracastRequest? get pending => _state.pending;

  /// Name of the connected sender.
  String? get connectedName => _state.connected;

  /// The problem to explain, or null. A clear result of the first look at the system counts
  /// while nothing is running.
  MiracastIssue? get issue {
    if (_libraryIssue != null) return _libraryIssue;
    if (_state.issue != null) return _state.issue;
    if (phase == MiracastPhase.idle) return _probe?.issue;
    if (_state.adapter == 'not_optimized') {
      return MiracastIssue.adapterNotOptimized;
    }
    return null;
  }

  MiracastPhase get phase {
    if (_libraryIssue != null) return MiracastPhase.problem;
    switch (_state.status) {
      case MiracastStatus.off:
        return _starting ? MiracastPhase.starting : MiracastPhase.idle;
      case MiracastStatus.starting:
        return MiracastPhase.starting;
      case MiracastStatus.stopping:
        return MiracastPhase.stopping;
      case MiracastStatus.listening:
        return MiracastPhase.listening;
      case MiracastStatus.connectionPending:
        return MiracastPhase.consent;
      case MiracastStatus.connected:
        return MiracastPhase.connected;
      case MiracastStatus.unavailable:
      case MiracastStatus.disabledByPolicy:
      case MiracastStatus.unsupported:
      case MiracastStatus.failed:
        return MiracastPhase.problem;
    }
  }

  /// A request is waiting although the status word has not caught up (the dialog must show
  /// as soon as there is a request).
  bool get consentNeeded => _state.pending != null;

  /// Look at the system once (feature, adapter). Call when the screen opens.
  Future<void> init() async {
    try {
      _probe = await core.probe();
      if (_probe?.issue == MiracastIssue.libraryMissing) {
        _libraryIssue = MiracastIssue.libraryMissing;
      }
    } catch (_) {
      _libraryIssue = MiracastIssue.libraryMissing;
    }
    _notify();
  }

  /// Start receiving with [displayName] and [pairing].
  Future<void> start() async {
    if (_starting ||
        phase == MiracastPhase.listening ||
        phase == MiracastPhase.consent ||
        phase == MiracastPhase.connected) {
      return;
    }
    _starting = true;
    _frame = null;
    _framesSeen = 0;
    _state = const MiracastState(status: MiracastStatus.starting);
    _notify();
    try {
      await core.start(name: displayName.trim(), pairing: pairing);
    } on MiracastCallException catch (e) {
      _starting = false;
      if (e.code == -1) {
        // Busy: a receiver is already running in this process (for example after a hot
        // restart). Take it over instead of failing.
        startPolling();
        await poll();
        return;
      }
      if (e.isLibraryProblem) {
        _libraryIssue = MiracastIssue.libraryMissing;
      } else {
        _state = MiracastState(
          status: MiracastStatus.failed,
          issue: e.code == -2
              ? MiracastIssue.invalidSettings
              : MiracastIssue.other,
          lastError: e.toString(),
        );
      }
      _notify();
      return;
    } catch (e) {
      _starting = false;
      _state = MiracastState(
        status: MiracastStatus.failed,
        issue: MiracastIssue.other,
        lastError: '$e',
      );
      _notify();
      return;
    }
    startPolling();
    await poll();
  }

  /// Stop receiving (the Rust side disconnects everything and restores the Windows settings).
  Future<void> stop() async {
    _timer?.cancel();
    _timer = null;
    _frame = null;
    _starting = false;
    try {
      await core.stop();
    } catch (_) {
      // Nothing to stop or the library is gone; the state below says "off".
    }
    _state = MiracastState.off;
    _notify();
  }

  /// The person's answer to the consent dialog. Answers only the request that is waiting
  /// right now; a stale call is ignored. Returns whether an answer was sent.
  Future<bool> respond(int requestId, bool accept) async {
    final p = _state.pending;
    if (p == null || p.id != requestId) return false;
    // Clear it locally first so the dialog cannot be answered twice.
    _state = _state.withoutPending();
    _notify();
    try {
      await core.respond(requestId, accept);
    } on MiracastCallException {
      // The request was gone already (timeout) or Windows refused; the next poll shows the truth.
    } catch (_) {}
    await poll();
    return true;
  }

  /// End the active connection and keep listening.
  Future<void> disconnect() async {
    _frame = null;
    try {
      await core.disconnect();
    } catch (_) {}
    await poll();
  }

  Future<void> setMuted(bool muted) async {
    try {
      await core.setMuted(muted);
    } catch (_) {}
  }

  /// Poll the core once: state, and the newest picture while connected. Public for tests.
  Future<void> poll({bool withState = true}) async {
    if (_busy || _disposed) return;
    _busy = true;
    try {
      if (withState) {
        final s = await core.state();
        if (_disposed) return;
        _applyState(s);
      }
      if (_state.status == MiracastStatus.connected) {
        final f = await core.pollFrame();
        if (_disposed) return;
        if (f != null) {
          _frame = f;
          _framesSeen++;
        }
      } else {
        _frame = null;
      }
    } on MiracastCallException catch (e) {
      if (e.isLibraryProblem) _libraryIssue = MiracastIssue.libraryMissing;
    } catch (_) {
      // A failed poll is skipped; the next one tries again.
    } finally {
      _busy = false;
    }
    _notify();
  }

  void _applyState(MiracastState s) {
    final old = _state;
    // "off" right after start() is not the truth yet: the core says "starting" first.
    if (_starting && s.status == MiracastStatus.off) {
      return;
    }
    if (s.status != MiracastStatus.off && s.status != MiracastStatus.starting) {
      _starting = false;
    }
    if (s.status == MiracastStatus.off && old.status != MiracastStatus.off) {
      _starting = false;
    }
    _state = s;
    if (s.generation != old.generation) {
      _framesSeen = 0;
    }
  }

  /// Poll regularly (pictures every [pollInterval], state every [statePollInterval]).
  void startPolling() {
    _timer?.cancel();
    _ticks = 0;
    // Counted in ticks, not read from a clock, so tests with a fake clock behave the same.
    final every =
        (statePollInterval.inMicroseconds / pollInterval.inMicroseconds)
            .ceil()
            .clamp(1, 1 << 20);
    _timer = Timer.periodic(pollInterval, (_) {
      _ticks++;
      final due = _ticks % every == 0;
      // Between state polls only the picture is polled, and only while connected.
      if (due || _state.status == MiracastStatus.connected) {
        unawaited(poll(withState: due));
      }
    });
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _timer?.cancel();
    _timer = null;
    // Leaving the screen stops receiving; never leave a hidden receiver running.
    unawaited(core.stop().catchError((Object _) {}));
    super.dispose();
  }
}
