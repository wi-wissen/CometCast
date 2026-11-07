// SPDX-License-Identifier: GPL-3.0-or-later
// State of the H.264 codec for the screens that need it (docs/capture.md, "H.264 licensing").
//
// Release builds do not contain Cisco's OpenH264. The Rust core downloads it once, from Cisco,
// after the person agreed. This controller asks the core for the state, starts the download,
// follows its progress and remembers the consent in the settings. Where the administrator set
// COMETCAST_CODEC_AUTO=1 (info.autoDownload) the download starts without asking.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';
import 'settings.dart';

/// "620 KB" / "1.2 MB" for a size in bytes.
String codecSizeText(int bytes) {
  if (bytes >= 1024 * 1024) {
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
  return '${(bytes / 1024).round()} KB';
}

class CodecController extends ChangeNotifier {
  CodecController({
    required this.core,
    required this.settings,
    this.pollInterval = const Duration(milliseconds: 300),
    bool needed = true,
  }) : _needed = needed {
    if (needed) {
      state = core.codecState();
      info = core.codecInfo();
    } else {
      state = const UiCodecState.ready();
    }
  }

  /// False where the OpenH264 library is never used on this screen: the Android sender (the
  /// device's own MediaCodec makes the H.264) and a receiver that decodes with the device's
  /// MediaCodec. The card then never shows and nothing is downloaded.
  bool get needed => _needed;
  bool _needed;

  final CoreApi core;
  final AppSettings settings;
  final Duration pollInterval;

  late UiCodecState state;
  UiCodecInfo? info;
  Timer? _poll;
  Completer<void>? _followDone;
  bool _disposed = false;
  bool _busy = false;

  /// Nothing is missing: the screen can go on.
  bool get ready => state is UiCodecState_Ready;

  /// The card has something to say (missing, running, failed or not available).
  bool get needsAttention => !ready;

  /// The person has to be asked before a download starts.
  bool get needsConsent =>
      state is UiCodecState_Missing &&
      !settings.codecConsentFor(info) &&
      !(info?.autoDownload ?? false);

  /// The device's own decoder failed: the OpenH264 codec is needed after all. Reads the state,
  /// shows the card and, if the person agreed before (or the administrator allows it), starts
  /// the download. Without consent nothing is downloaded.
  Future<void> require() async {
    if (_needed || _disposed) return;
    _needed = true;
    state = core.codecState();
    info = core.codecInfo();
    notifyListeners();
    await init();
  }

  /// Call once when the screen opens: reads the remembered consent and, if the person agreed
  /// before or the administrator allows it, starts the download at once.
  Future<void> init() async {
    if (!needed) return;
    await settings.loadCodecConsent();
    if (_disposed) return;
    state = core.codecState();
    info = core.codecInfo();
    notifyListeners();
    final allowed =
        settings.codecConsentFor(info) || (info?.autoDownload ?? false);
    if (state is UiCodecState_Downloading) {
      // Another screen, controller or process owns the download: follow it.
      if (_busy) return;
      _busy = true;
      try {
        await _follow();
      } finally {
        _busy = false;
      }
    } else if (state is UiCodecState_Missing && allowed) {
      await download(askedBefore: true);
    }
  }

  /// Poll the core until the state is no longer "downloading" (ready, failed or missing).
  /// The download that this state belongs to is not ours (or ensureCodec said so), so only
  /// the core's state tells when it is over.
  Future<void> _follow() {
    _poll?.cancel();
    final done = _followDone = Completer<void>();
    _poll = Timer.periodic(pollInterval, (timer) {
      if (_disposed) return;
      try {
        state = core.codecState();
      } catch (e) {
        state = UiCodecState.failed(reason: e.toString());
      }
      notifyListeners();
      if (state is! UiCodecState_Downloading) {
        timer.cancel();
        _poll = null;
        if (!done.isCompleted) done.complete();
      }
    });
    return done.future;
  }

  /// Start the download. [askedBefore] is false when the person just pressed the button: the
  /// consent is then remembered.
  Future<void> download({bool askedBefore = false}) async {
    if (_busy || _disposed) return;
    _busy = true;
    if (!askedBefore && !(info?.autoDownload ?? false)) {
      final i = info;
      if (i != null) await settings.rememberCodecConsent(i);
    }
    if (_disposed) return;
    state = const UiCodecState.downloading(progress: 0);
    notifyListeners();
    _poll = Timer.periodic(pollInterval, (_) {
      final s = core.codecState();
      if (s is UiCodecState_Downloading && !_disposed) {
        state = s;
        notifyListeners();
      }
    });
    try {
      state = await core.ensureCodec();
      _poll?.cancel();
      _poll = null;
      if (!_disposed && state is UiCodecState_Downloading) {
        // Somebody else runs the download (ensureCodec returns at once then): wait for it.
        notifyListeners();
        await _follow();
      }
    } catch (e) {
      state = UiCodecState.failed(reason: e.toString());
    } finally {
      _poll?.cancel();
      _poll = null;
      _busy = false;
    }
    if (!_disposed) notifyListeners();
  }

  String? _loggedReason;

  /// The technical reason of a failure or of "unsupported" goes to the log, never on screen
  /// (spec N46): the screens show the key's text only.
  @override
  void notifyListeners() {
    final s = state;
    final r = switch (s) {
      UiCodecState_Failed(:final reason) => 'download failed: $reason',
      UiCodecState_Unsupported(:final reason) => 'unsupported: $reason',
      _ => null,
    };
    if (r != null && r != _loggedReason) {
      _loggedReason = r;
      debugPrint('cometcast: video codec $r');
    }
    super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _poll?.cancel();
    final d = _followDone;
    if (d != null && !d.isCompleted) d.complete();
    super.dispose();
  }
}
