// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Receive on this device" screen. Like a Vue composable (store + actions):
// the screen listens to it and rebuilds when it calls notifyListeners().

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart' show WidgetsBinding;

import 'core/core_api.dart';
import 'window_control.dart';

enum ReceiverPhase {
  /// Nothing started (or stopped by the user).
  idle,

  /// Waiting for the server to hand out a room code.
  starting,

  /// Room exists, events are flowing.
  running,

  /// The stream ended on its own (sender left or error).
  ended,
}

/// Counters of the decoder (from the core) plus what the UI itself measured.
class VideoStatsView {
  const VideoStatsView({
    this.decoded = 0,
    this.dropped = 0,
    this.decodeErrors = 0,
    this.droppedPackets = 0,
    this.keyframeRequests = 0,
    this.width = 0,
    this.height = 0,
    this.avgDecodeUs = 0,
    this.error,
  });

  final int decoded;
  final int dropped;
  final int decodeErrors;
  final int droppedPackets;
  final int keyframeRequests;
  final int width;
  final int height;
  final int avgDecodeUs;
  final String? error;
}

class ReceiverController extends ChangeNotifier {
  ReceiverController({
    required this.core,
    this.udpBind,
    SetFullscreen? setFullscreen,
  }) : _setFullscreen = setFullscreen ?? setWindowFullscreen;

  final CoreApi core;
  final String? udpBind;
  final SetFullscreen _setFullscreen;

  StreamSubscription<ReceiverUiEvent>? _sub;

  ReceiverPhase phase = ReceiverPhase.idle;
  String? roomCode;
  bool senderJoined = false;
  bool senderLeft = false;
  UiConnectionState? connectionState;
  String? codec;
  int packets = 0;
  int bytes = 0;
  int frames = 0;
  int keyframes = 0;

  /// The newest decoded picture, or null until one arrived. Owned by this controller
  /// (disposed when replaced); the screen only paints it.
  ui.Image? image;

  /// Number of pictures drawn since [start].
  int imagesShown = 0;

  /// Pictures drawn per second, measured over the last interval between two `Stats` events.
  double displayFps = 0;

  VideoStatsView videoStats = const VideoStatsView();

  /// Mean time (microseconds) the UI side needed per picture: fetching it from the core
  /// (`takeFrame`, includes copying the pixels over the bridge) and turning it into an image.
  int avgFetchUs = 0;
  int avgImageUs = 0;
  int _fetchUsTotal = 0;
  int _imageUsTotal = 0;
  int _fetched = 0;

  bool fullscreen = false;
  bool showStats = false;

  /// Technical error text in English (shown after a translated "Error:" prefix).
  String? error;

  bool _fetching = false;
  bool _refetch = false;
  bool _disposed = false;
  int _shownAtLastStats = 0;
  final Stopwatch _clock = Stopwatch();
  int _lastStatsMs = 0;

  bool get isActive =>
      phase == ReceiverPhase.starting || phase == ReceiverPhase.running;

  /// True if the sender's track is something this version cannot draw (not H.264).
  bool get codecNotShown =>
      codec != null && codec != 'h264' && image == null && isActive;

  /// Create a room on [server] and start listening.
  void start(String server) {
    _cancel();
    _clearImage();
    phase = ReceiverPhase.starting;
    roomCode = null;
    senderJoined = false;
    senderLeft = false;
    connectionState = null;
    codec = null;
    packets = bytes = frames = keyframes = 0;
    imagesShown = 0;
    displayFps = 0;
    _shownAtLastStats = 0;
    videoStats = const VideoStatsView();
    avgFetchUs = avgImageUs = _fetchUsTotal = _imageUsTotal = _fetched = 0;
    error = null;
    _clock
      ..reset()
      ..start();
    _lastStatsMs = 0;
    notifyListeners();
    _sub = core
        .startReceiver(server: server.trim(), udpBind: udpBind)
        .listen(
          _onEvent,
          onError: (Object e) {
            error = _describe(e);
            _end();
          },
          onDone: () {
            // The stream also ends after stop(); only mark "ended" if it was not the user.
            if (phase == ReceiverPhase.running ||
                phase == ReceiverPhase.starting) {
              _end();
            }
          },
        );
  }

  void _end() {
    phase = ReceiverPhase.ended;
    _clearImage();
    unawaited(_leaveFullscreen());
    notifyListeners();
  }

  void _onEvent(ReceiverUiEvent e) {
    switch (e) {
      case ReceiverUiEvent_RoomCode(:final code):
        roomCode = code;
        phase = ReceiverPhase.running;
      case ReceiverUiEvent_PeerJoined():
        senderJoined = true;
      case ReceiverUiEvent_ConnectionState(:final state):
        connectionState = state;
      case ReceiverUiEvent_VideoTrack(:final codec):
        this.codec = codec;
      case ReceiverUiEvent_Stats():
        packets = e.packets.toInt();
        bytes = e.bytes.toInt();
        frames = e.frames.toInt();
        keyframes = e.keyframes.toInt();
        _measureDisplayRate();
      case ReceiverUiEvent_FrameReady():
        _onFrameReady();
        return;
      case ReceiverUiEvent_VideoStats():
        videoStats = VideoStatsView(
          decoded: e.decoded.toInt(),
          dropped: e.dropped.toInt(),
          decodeErrors: e.decodeErrors.toInt(),
          droppedPackets: e.droppedPackets.toInt(),
          keyframeRequests: e.keyframeRequests.toInt(),
          width: e.width,
          height: e.height,
          avgDecodeUs: e.avgDecodeUs.toInt(),
          error: e.error,
        );
      case ReceiverUiEvent_PeerLeft():
        senderLeft = true;
        _end();
        return;
      case ReceiverUiEvent_Error(:final message):
        error = message;
        _end();
        return;
    }
    notifyListeners();
  }

  void _measureDisplayRate() {
    final now = _clock.elapsedMilliseconds;
    final dt = now - _lastStatsMs;
    if (dt > 0 && _lastStatsMs > 0) {
      displayFps = (imagesShown - _shownAtLastStats) * 1000 / dt;
    }
    _lastStatsMs = now;
    _shownAtLastStats = imagesShown;
  }

  // --- pictures ------------------------------------------------------------------------

  /// The core says a picture is waiting. At most one fetch runs; a second notice during a
  /// fetch makes the loop fetch again (the core only sends a new notice after a fetch).
  void _onFrameReady() {
    if (_fetching) {
      _refetch = true;
      return;
    }
    unawaited(_fetchLoop());
  }

  Future<void> _fetchLoop() async {
    _fetching = true;
    try {
      do {
        _refetch = false;
        final t0 = _clock.elapsedMicroseconds;
        final frame = await core.takeFrame();
        if (_disposed) return;
        if (frame == null) continue;
        final t1 = _clock.elapsedMicroseconds;
        final img = await _toImage(frame);
        final t2 = _clock.elapsedMicroseconds;
        _fetched++;
        _fetchUsTotal += t1 - t0;
        _imageUsTotal += t2 - t1;
        avgFetchUs = _fetchUsTotal ~/ _fetched;
        avgImageUs = _imageUsTotal ~/ _fetched;
        if (_disposed || !isActive) {
          img.dispose();
          return;
        }
        _swapImage(img);
      } while (_refetch);
    } catch (e) {
      // A bad picture must not stop the stream; the next one replaces it.
      debugPrint('cometcast: cannot show a picture: $e');
    } finally {
      _fetching = false;
    }
  }

  static Future<ui.Image> _toImage(VideoFrame f) {
    final completer = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      f.rgba,
      f.width,
      f.height,
      ui.PixelFormat.rgba8888,
      completer.complete,
    );
    return completer.future;
  }

  void _swapImage(ui.Image img) {
    final old = image;
    image = img;
    imagesShown++;
    notifyListeners();
    // The old picture may still be used by the frame being painted: dispose it after that.
    if (old != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => old.dispose());
    }
  }

  void _clearImage() {
    image?.dispose();
    image = null;
  }

  /// Tell the core how much room the picture has (physical pixels).
  int _lastW = 0;
  int _lastH = 0;
  void reportViewSize(int width, int height) {
    if (width <= 0 || height <= 0) return;
    if (width == _lastW && height == _lastH) return;
    _lastW = width;
    _lastH = height;
    core.setVideoMaxSize(width, height);
  }

  // --- window --------------------------------------------------------------------------

  Future<void> toggleFullscreen() async {
    final target = !fullscreen;
    final done = await _setFullscreen(target);
    if (_disposed) return;
    // Only believe the platform if it did it (no handler = stay windowed).
    fullscreen = done ? target : false;
    notifyListeners();
  }

  Future<void> _leaveFullscreen() async {
    if (!fullscreen) return;
    fullscreen = false;
    await _setFullscreen(false);
  }

  void toggleStats() {
    showStats = !showStats;
    notifyListeners();
  }

  /// Stop receiving. The room code disappears (the room is gone).
  Future<void> stop() async {
    _cancel();
    phase = ReceiverPhase.idle;
    roomCode = null;
    _clearImage();
    final wasFullscreen = fullscreen;
    notifyListeners();
    await core.stopReceiver();
    if (wasFullscreen) await _leaveFullscreen();
  }

  void _cancel() {
    _sub?.cancel();
    _sub = null;
  }

  static String _describe(Object e) => e.toString();

  @override
  void dispose() {
    _disposed = true;
    _cancel();
    _clearImage();
    // Fire and forget: the screen is gone, the core must not keep a room open.
    unawaited(core.stopReceiver());
    if (fullscreen) unawaited(_setFullscreen(false));
    super.dispose();
  }
}
