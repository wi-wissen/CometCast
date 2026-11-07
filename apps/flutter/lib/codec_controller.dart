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

class CodecController extends ChangeNotifier {
  CodecController({
    required this.core,
    required this.settings,
    this.pollInterval = const Duration(milliseconds: 300),
  }) {
    state = core.codecState();
    info = core.codecInfo();
  }

  final CoreApi core;
  final AppSettings settings;
  final Duration pollInterval;

  late UiCodecState state;
  UiCodecInfo? info;
  Timer? _poll;
  bool _disposed = false;
  bool _busy = false;

  /// Nothing is missing: the screen can go on.
  bool get ready => state is UiCodecState_Ready;

  /// The card has something to say (missing, running, failed or not available).
  bool get needsAttention => !ready;

  /// The person has to be asked before a download starts.
  bool get needsConsent =>
      state is UiCodecState_Missing &&
      !settings.codecConsent &&
      !(info?.autoDownload ?? false);

  /// Call once when the screen opens: reads the remembered consent and, if the person agreed
  /// before or the administrator allows it, starts the download at once.
  Future<void> init() async {
    await settings.loadCodecConsent();
    if (_disposed) return;
    state = core.codecState();
    info = core.codecInfo();
    notifyListeners();
    final allowed = settings.codecConsent || (info?.autoDownload ?? false);
    if (state is UiCodecState_Missing && allowed) {
      await download(askedBefore: true);
    }
  }

  /// Start the download. [askedBefore] is false when the person just pressed the button: the
  /// consent is then remembered.
  Future<void> download({bool askedBefore = false}) async {
    if (_busy || _disposed) return;
    _busy = true;
    if (!askedBefore && !(info?.autoDownload ?? false)) {
      await settings.rememberCodecConsent();
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
    } catch (e) {
      state = UiCodecState.failed(reason: e.toString());
    } finally {
      _poll?.cancel();
      _poll = null;
      _busy = false;
    }
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _poll?.cancel();
    super.dispose();
  }
}
