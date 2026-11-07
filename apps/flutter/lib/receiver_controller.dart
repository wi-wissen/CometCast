// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Receive on this device" screen. Like a Vue composable (store + actions):
// the screen listens to it and rebuilds when it calls notifyListeners().

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart' show WidgetsBinding;

import 'core/android_video.dart';
import 'core/core_api.dart';
import 'window_control.dart';

/// Opens a link in the system's browser. Replaced in tests; the real one is `launchUrl`.
typedef UrlOpener = Future<bool> Function(Uri url);

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

/// What the receive view shows besides the live screen (the latest content wins).
sealed class ReceivedContent {
  const ReceivedContent();
}

/// A received image. [bytes] is the file as sent; the screen decodes it (scaled down).
class ReceivedImage extends ReceivedContent {
  const ReceivedImage({
    required this.name,
    required this.bytes,
    required this.width,
    required this.height,
  });

  final String name;
  final Uint8List bytes;
  final int width;
  final int height;
}

/// A received PDF. [image] is null until the first page is drawn.
class ReceivedDocument extends ReceivedContent {
  const ReceivedDocument({
    required this.name,
    required this.pages,
    this.page,
    this.image,
  });

  final String name;
  final int pages;

  /// Page number of [image] (1-based).
  final int? page;
  final ui.Image? image;
}

/// A link the sender wants the person to open. Nothing is opened before a click.
class ReceivedLink extends ReceivedContent {
  const ReceivedLink({required this.url, required this.host});

  /// The normalised URL (validated by Rust).
  final String url;

  /// Host in its punycode form.
  final String host;
}

/// A calm note about received content. Always a code, never text from the sender.
enum ContentNote {
  badMessage,
  incomplete,
  tooLarge,
  documentRejected,
  renderFailed,
  tooManyErrors,
  transferTimeout,
  workerUnavailable,
  imageUndecodable,
  imageTooBig,
  linkOpenFailed,
}

ContentNote contentNoteFor(UiContentError e) => switch (e) {
  UiContentError.badMessage => ContentNote.badMessage,
  UiContentError.unexpectedChunk ||
  UiContentError.sizeMismatch ||
  UiContentError.noTransfer ||
  UiContentError.badChunk ||
  UiContentError.interrupted => ContentNote.incomplete,
  UiContentError.tooLarge => ContentNote.tooLarge,
  UiContentError.documentRejected => ContentNote.documentRejected,
  UiContentError.renderFailed => ContentNote.renderFailed,
  UiContentError.tooManyErrors => ContentNote.tooManyErrors,
  UiContentError.transferTimeout => ContentNote.transferTimeout,
  UiContentError.workerUnavailable => ContentNote.workerUnavailable,
};

/// An image with more pixels than this is not decoded.
/// UNCERTAIN: 64 Mpx (8000 x 8000) is a guess, chosen like the image limit of cc-docs; not measured.
const defaultMaxImagePixels = 64 * 1000 * 1000;

/// What is being received right now (a transfer in progress).
class ReceivingInfo {
  const ReceivingInfo({required this.name, required this.document});

  final String name;
  final bool document;
}

class ReceiverController extends ChangeNotifier {
  ReceiverController({
    required this.core,
    this.udpBind,
    SetFullscreen? setFullscreen,
    this.fetchTimeout = const Duration(seconds: 5),
    this.content = true,
    this.maxImagePixels = defaultMaxImagePixels,
    this.hardwareDecoder,
  }) : _setFullscreen = setFullscreen ?? setWindowFullscreen;

  final CoreApi core;
  final String? udpBind;
  final SetFullscreen _setFullscreen;

  /// The device's own H.264 decoder (Android `MediaCodec` into a texture), or null for the
  /// software decoder of the core. When set, the core hands over undecoded video; if the
  /// hardware path fails, the controller switches to the software decoder by itself.
  final AndroidVideoDecoder? hardwareDecoder;

  /// Accept images, PDFs and links from the sender.
  final bool content;

  /// An image with more pixels than this is not decoded (a small file can hold a huge picture).
  final int maxImagePixels;

  /// How long one picture fetch (core call plus image creation) may take before it is given
  /// up, so that a stuck call cannot block all later pictures.
  /// UNCERTAIN: 5 s is a placeholder.
  final Duration fetchTimeout;

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

  /// Id of the Flutter texture that shows the hardware decoder's picture, or null while the
  /// software path (or nothing) is used.
  int? textureId;

  /// Picture size reported by the hardware decoder (0 until known).
  int videoWidth = 0;
  int videoHeight = 0;

  /// Why the hardware path was given up (technical English), or null.
  String? hardwareError;

  /// A live picture is on screen: a decoded image, or a texture that knows its size.
  bool get hasLiveVideo =>
      image != null || (textureId != null && videoWidth > 0 && videoHeight > 0);

  /// Size of the live picture in pixels (0 x 0 if none).
  (int, int) get pictureSize =>
      image != null ? (image!.width, image!.height) : (videoWidth, videoHeight);

  /// Number of pictures drawn since [start].
  int imagesShown = 0;

  /// Pictures drawn per second, measured over the last interval between two `Stats` events.
  double displayFps = 0;

  VideoStatsView videoStats = const VideoStatsView();

  /// The received image, PDF page or link that is shown now, or null. It is shown instead of
  /// the live screen; closing it goes back to the screen (or to waiting).
  ReceivedContent? shown;

  /// A transfer that is arriving, or null.
  ReceivingInfo? receiving;

  /// A note about content that failed or was refused, or null.
  ContentNote? note;

  /// Bumped when the shown content is replaced or removed; a picture fetch that began earlier
  /// drops its result.
  int _contentEpoch = 0;

  /// Identifies the shown content: it changes when other content is shown (or none), not when
  /// the same page is drawn again (a resize renders the same page into a new picture).
  int get contentEpoch => _contentEpoch;
  bool _contentFetching = false;
  bool _contentRefetch = false;

  /// The person closed the document: its pages are ignored until the next one opens.
  bool _documentClosed = false;

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

  /// Bumped by [start] and [stop]; a fetch that began in an older session drops its result.
  int _session = 0;
  Timer? _retryTimer;
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
      codec != null && codec != 'h264' && !hasLiveVideo && isActive;

  /// Create a room on [server] and start listening.
  void start(String server) {
    _cancel();
    _newSession();
    _clearImage();
    _stopHardware();
    _startHardware();
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
    _clearContent();
    _clock
      ..reset()
      ..start();
    _lastStatsMs = 0;
    notifyListeners();
    _sub = core
        .startReceiver(
          server: server.trim(),
          udpBind: udpBind,
          content: content,
        )
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
    _stopHardware();
    _clearContent();
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
      case ReceiverUiEvent_EncodedReady():
        _onEncodedReady();
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
      case ReceiverUiEvent_ContentTransferStarted(:final document, :final name):
        receiving = ReceivingInfo(name: name, document: document);
      case ReceiverUiEvent_ContentFrameReady():
        _onContentFrameReady();
        return;
      case ReceiverUiEvent_ContentDocumentOpened(:final name, :final pages):
        _replaceContent(ReceivedDocument(name: name, pages: pages));
        _documentClosed = false;
        note = null;
      case ReceiverUiEvent_ContentLink(:final url, :final host):
        _replaceContent(ReceivedLink(url: url, host: host));
        _documentClosed = false;
        note = null;
      case ReceiverUiEvent_ContentStopped():
        _replaceContent(null);
        _documentClosed = false;
      case ReceiverUiEvent_ContentError(:final error):
        receiving = null;
        note = contentNoteFor(error);
        // The core closes the shown document when a PDF is refused.
        if (error == UiContentError.documentRejected ||
            error == UiContentError.workerUnavailable) {
          if (shown is ReceivedDocument) _replaceContent(null);
        }
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

  // --- hardware decoding (Android) -------------------------------------------------------

  StreamSubscription<VideoDecoderEvent>? _hwSub;
  Future<void>? _hwReady;
  bool _hwFetching = false;
  bool _hwRefetch = false;

  /// Switch the core to pass-through and bring up the decoder and its texture. The decoder start
  /// is asynchronous; units that arrive earlier wait for it in [_hwFetchLoop].
  void _startHardware() {
    final d = hardwareDecoder;
    if (d == null) return;
    hardwareError = null;
    core.setVideoPassthrough(true);
    final session = _session;
    _hwSub = d.events.listen(_onHardwareEvent);
    _hwReady = () async {
      try {
        final id = await d.start();
        if (_disposed || session != _session) return;
        textureId = id;
        notifyListeners();
      } catch (e) {
        if (!_disposed && session == _session) {
          _hardwareFailed('cannot start the hardware decoder: $e');
        }
      }
    }();
  }

  void _stopHardware() {
    final d = hardwareDecoder;
    if (d == null) return;
    _hwSub?.cancel();
    _hwSub = null;
    _hwReady = null;
    _hwFetching = false;
    _hwRefetch = false;
    textureId = null;
    videoWidth = videoHeight = 0;
    core.setVideoPassthrough(false);
    unawaited(d.stop());
  }

  void _onHardwareEvent(VideoDecoderEvent e) {
    if (_disposed) return;
    switch (e) {
      case VideoSize(:final width, :final height):
        videoWidth = width;
        videoHeight = height;
        notifyListeners();
      case VideoNeedKeyFrame():
        core.requestVideoKeyframe();
      case VideoDecoderError(:final message):
        _hardwareFailed(message);
    }
  }

  /// The hardware path does not work on this device (or just now): release it and let the core
  /// decode in software. The core asks the sender for a key frame when the mode changes.
  void _hardwareFailed(String message) {
    debugPrint('cometcast: hardware video decoder failed: $message');
    hardwareError = message;
    _stopHardware(); // also switches the core back to the software decoder
    notifyListeners();
  }

  void _onEncodedReady() {
    if (hardwareDecoder == null) return;
    if (_hwFetching) {
      _hwRefetch = true;
      return;
    }
    unawaited(_hwFetchLoop());
  }

  Future<void> _hwFetchLoop() async {
    final session = _session;
    _hwFetching = true;
    try {
      await _hwReady;
      do {
        _hwRefetch = false;
        final units = await core.takeEncodedVideo().timeout(fetchTimeout);
        if (_disposed || session != _session) return;
        final d = hardwareDecoder;
        // Hardware path gone (failed, stopped): the units are of no use.
        if (d == null || _hwReady == null) return;
        for (final u in units) {
          d.queue(u.data, ptsUs: u.ptsUs.toInt(), key: u.isKey);
        }
      } while (_hwRefetch);
    } catch (e) {
      debugPrint('cometcast: cannot pass video to the hardware decoder: $e');
    } finally {
      if (session == _session) _hwFetching = false;
    }
  }

  /// A new session invalidates fetches in flight and frees the fetch slot for the new one.
  void _newSession() {
    _session++;
    _fetching = false;
    _refetch = false;
    _contentFetching = false;
    _contentRefetch = false;
    _retryTimer?.cancel();
    _retryTimer = null;
  }

  Future<void> _fetchLoop() async {
    final session = _session;
    _fetching = true;
    try {
      do {
        _refetch = false;
        final t0 = _clock.elapsedMicroseconds;
        final frame = await core.takeFrame().timeout(fetchTimeout);
        if (_disposed || session != _session) return;
        if (frame == null) continue;
        final t1 = _clock.elapsedMicroseconds;
        final img = await _toImage(frame, fetchTimeout);
        if (_disposed || session != _session || !isActive) {
          img.dispose();
          return;
        }
        final t2 = _clock.elapsedMicroseconds;
        _fetched++;
        _fetchUsTotal += t1 - t0;
        _imageUsTotal += t2 - t1;
        avgFetchUs = _fetchUsTotal ~/ _fetched;
        avgImageUs = _imageUsTotal ~/ _fetched;
        _swapImage(img);
      } while (_refetch);
    } catch (e) {
      // A bad picture must not stop the stream. The core sends a new "frame ready" only
      // after a fetch reached it, so ask again a little later in case this one did not.
      debugPrint('cometcast: cannot show a picture: $e');
      if (!_disposed && session == _session) {
        _retryTimer?.cancel();
        _retryTimer = Timer(const Duration(milliseconds: 500), () {
          if (!_disposed && session == _session && isActive && !_fetching) {
            unawaited(_fetchLoop());
          }
        });
      }
    } finally {
      // A newer session owns the flag now.
      if (session == _session) _fetching = false;
    }
  }

  static Future<ui.Image> _toImage(VideoFrame f, Duration timeout) =>
      _pixelsToImage(f.rgba, f.width, f.height, timeout);

  static Future<ui.Image> _pixelsToImage(
    Uint8List rgba,
    int width,
    int height,
    Duration timeout,
  ) {
    final completer = Completer<ui.Image>();
    var abandoned = false;
    ui.decodeImageFromPixels(rgba, width, height, ui.PixelFormat.rgba8888, (
      img,
    ) {
      // Too late: nobody waits for it any more.
      if (abandoned) {
        img.dispose();
      } else {
        completer.complete(img);
      }
    });
    return completer.future.timeout(
      timeout,
      onTimeout: () {
        abandoned = true;
        throw TimeoutException(
          'decodeImageFromPixels did not complete',
          timeout,
        );
      },
    );
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

  // --- received content ----------------------------------------------------------------

  /// The core says an image or page waits. At most one fetch runs; a second notice during a
  /// fetch makes the loop fetch again (the core only sends a new notice after a fetch).
  void _onContentFrameReady() {
    if (_contentFetching) {
      _contentRefetch = true;
      return;
    }
    unawaited(_contentFetchLoop());
  }

  Future<void> _contentFetchLoop() async {
    final session = _session;
    _contentFetching = true;
    try {
      do {
        _contentRefetch = false;
        final frame = await core.takeContentFrame().timeout(fetchTimeout);
        if (_disposed || session != _session) return;
        // Read after the take: the core clears its mailbox when content is replaced, so a
        // frame taken now belongs to the content shown now (the first page of a new
        // document must not be dropped because the epoch moved while the take ran).
        // `_showPage` still ignores a page when no document is shown.
        final epoch = _contentEpoch;
        if (frame == null) continue;
        switch (frame) {
          case ContentFrame_Image(:final name, :final bytes):
            await _showImage(session, epoch, name, bytes);
          case ContentFrame_Page(
            :final page,
            :final width,
            :final height,
            :final rgba,
          ):
            await _showPage(session, epoch, page, width, height, rgba);
        }
      } while (_contentRefetch);
    } catch (e) {
      debugPrint('cometcast: cannot show received content: $e');
      if (!_disposed && session == _session) {
        note = ContentNote.renderFailed;
        notifyListeners();
      }
    } finally {
      if (session == _session) _contentFetching = false;
    }
  }

  Future<void> _showImage(
    int session,
    int epoch,
    String name,
    Uint8List bytes,
  ) async {
    // Read the size from the header first: a small file can hold a huge picture.
    int w;
    int h;
    try {
      final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
      try {
        final d = await ui.ImageDescriptor.encoded(buffer);
        w = d.width;
        h = d.height;
        d.dispose();
      } finally {
        buffer.dispose();
      }
    } catch (_) {
      if (_stale(session, epoch)) return;
      _replaceContent(null);
      note = ContentNote.imageUndecodable;
      notifyListeners();
      return;
    }
    if (_stale(session, epoch)) return;
    if (w <= 0 || h <= 0) {
      _replaceContent(null);
      note = ContentNote.imageUndecodable;
    } else if (w * h > maxImagePixels) {
      _replaceContent(null);
      note = ContentNote.imageTooBig;
    } else {
      _replaceContent(
        ReceivedImage(name: name, bytes: bytes, width: w, height: h),
      );
      _documentClosed = false;
      note = null;
    }
    notifyListeners();
  }

  Future<void> _showPage(
    int session,
    int epoch,
    int page,
    int width,
    int height,
    Uint8List rgba,
  ) async {
    final doc = shown;
    if (_documentClosed || doc is! ReceivedDocument) return;
    final img = await _pixelsToImage(rgba, width, height, fetchTimeout);
    if (_stale(session, epoch) || !identical(shown, doc)) {
      img.dispose();
      return;
    }
    final old = doc.image;
    shown = ReceivedDocument(
      name: doc.name,
      pages: doc.pages,
      page: page,
      image: img,
    );
    receiving = null;
    notifyListeners();
    if (old != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => old.dispose());
    }
  }

  bool _stale(int session, int epoch) =>
      _disposed || session != _session || epoch != _contentEpoch;

  /// Make [next] the shown content (null = nothing). Fetches of older content are dropped.
  void _replaceContent(ReceivedContent? next) {
    _contentEpoch++;
    final old = shown;
    shown = next;
    receiving = null;
    if (old is ReceivedDocument && old.image != null) {
      final img = old.image!;
      WidgetsBinding.instance.addPostFrameCallback((_) => img.dispose());
    }
  }

  void _clearContent() {
    final old = shown;
    shown = null;
    receiving = null;
    note = null;
    _contentEpoch++;
    _contentRefetch = false;
    _documentClosed = false;
    if (old is ReceivedDocument) old.image?.dispose();
  }

  /// The person closes what is shown (image, page or link). The receiver keeps running;
  /// the live screen (if any) comes back, otherwise the waiting view. Pages that still
  /// arrive for a closed document are ignored. UNCERTAIN: the core cannot be told to close a
  /// document, so it may go on rendering pages for the sender's page commands until the
  /// sender sends `stop` or a new document (no UI effect, some wasted work).
  void closeContent() {
    if (shown is ReceivedDocument) _documentClosed = true;
    _replaceContent(null);
    notifyListeners();
  }

  void dismissNote() {
    if (note == null) return;
    note = null;
    notifyListeners();
  }

  /// Open the shown link in the browser. Only after the person asked for it (a click on
  /// "Open"); the link is checked again by Rust and its scheme once more here. Returns true
  /// if the system accepted it.
  Future<bool> openLink(UrlOpener open) async {
    final link = shown;
    if (link is! ReceivedLink) return false;
    final check = core.validateLink(link.url);
    final uri = switch (check) {
      LinkCheck_Valid(:final url) => Uri.tryParse(url),
      _ => null,
    };
    if (uri == null ||
        (uri.scheme != 'http' && uri.scheme != 'https') ||
        uri.host.isEmpty) {
      note = ContentNote.linkOpenFailed;
      _replaceContent(null);
      notifyListeners();
      return false;
    }
    var ok = false;
    try {
      ok = await open(uri);
    } catch (_) {
      ok = false;
    }
    if (_disposed) return ok;
    // The card is done either way; a failure leaves a note.
    if (identical(shown, link)) _replaceContent(null);
    note = ok ? null : ContentNote.linkOpenFailed;
    notifyListeners();
    return ok;
  }

  int _lastContentWidth = 0;

  /// Tell the core how wide pages should be drawn (physical pixels of the view).
  void reportContentWidth(int width) {
    if (width <= 0 || width == _lastContentWidth) return;
    _lastContentWidth = width;
    core.setContentTargetWidth(width);
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
    _newSession();
    phase = ReceiverPhase.idle;
    roomCode = null;
    _clearImage();
    _stopHardware();
    _clearContent();
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
    _retryTimer?.cancel();
    _cancel();
    _clearImage();
    _stopHardware();
    _clearContent();
    // Fire and forget: the screen is gone, the core must not keep a room open.
    unawaited(core.stopReceiver());
    if (fullscreen) unawaited(_setFullscreen(false));
    super.dispose();
  }
}
