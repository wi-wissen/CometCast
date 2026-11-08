// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Receive on this device" screen. Like a Vue composable (store + actions):
// the screen listens to it and rebuilds when it calls notifyListeners().

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart' show WidgetsBinding;

import 'core/android_video.dart';
import 'core/core_api.dart';
import 'join_state.dart';
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
  const ReceivingInfo({
    required this.name,
    required this.document,
    this.received = 0,
    this.total = 0,
  });

  final String name;
  final bool document;

  /// Bytes received so far and announced (0 until the first progress event).
  final int received;
  final int total;

  /// Whole percent, 0 to 100 (0 while the size is unknown).
  int get percent =>
      total <= 0 ? 0 : (received * 100 ~/ total).clamp(0, 100).toInt();

  ReceivingInfo withProgress(int received, int total) => ReceivingInfo(
    name: name,
    document: document,
    received: received,
    total: total,
  );
}

/// A sender that waits for the host's decision (join mode `confirm`). The name is untrusted text
/// that the server cleaned: show it as plain text.
class JoinRequestView {
  const JoinRequestView({
    required this.id,
    required this.name,
    required this.check,
    required this.receivedAt,
  });

  final BigInt id;
  final String name;

  /// Two-digit number that the sender's waiting screen shows too.
  final int check;
  final DateTime receivedAt;
}

/// A sender on the room's block list (blocked for the rest of this room session).
class BlockedView {
  const BlockedView({required this.id, required this.name});

  final BigInt id;
  final String name;
}

/// A short message about the join settings.
enum JoinNote {
  /// The room was locked and this many waiting requests were discarded.
  lockedDiscarded,

  /// The server forbids open rooms: "ask first" is used instead.
  openDisabled,
}

/// Something the display tells the host in a toast (docs/design/first-run-and-display.md 4.6).
sealed class ReceiverNotice {
  const ReceiverNotice();
}

/// A sender connected (join mode open, or after the host allowed it).
class NoticeJoined extends ReceiverNotice {
  const NoticeJoined(this.name);
  final String name;
}

/// The server ended a waiting request (`join_approval_secs` ran out).
class NoticeExpired extends ReceiverNotice {
  const NoticeExpired(this.name);
  final String name;
}

/// A waiting sender withdrew its request or went away.
class NoticeWithdrawn extends ReceiverNotice {
  const NoticeWithdrawn(this.name);
  final String name;
}

/// The connected sender left.
class NoticeLeft extends ReceiverNotice {
  const NoticeLeft(this.name);
  final String name;
}

/// The session ended by End: everybody dropped, a new code follows.
class NoticeSessionEnded extends ReceiverNotice {
  const NoticeSessionEnded();
}

/// The server forbids open rooms; the room is Approval on now.
class NoticeOpenDisabled extends ReceiverNotice {
  const NoticeOpenDisabled();
}

/// The server refused a command of the host (`no_such_request`, `block_list_full`, ...) but
/// the room goes on.
class NoticeServer extends ReceiverNotice {
  const NoticeServer(this.code);
  final String code;
}

/// How long the server lets a request wait (its default). UNCERTAIN: an operator can change it;
/// the server decides when a request ends.
const joinRequestSeconds = 60;

class ReceiverController extends ChangeNotifier {
  ReceiverController({
    required this.core,
    this.udpBind,
    SetFullscreen? setFullscreen,
    this.fetchTimeout = const Duration(seconds: 5),
    this.content = true,
    this.maxImagePixels = defaultMaxImagePixels,
    this.hardwareDecoder,
    this.onNotice,
    this.restartMode,
    this.keepRoomOnPeerLeft = false,
  }) : _setFullscreen = setFullscreen ?? setWindowFullscreen;

  /// Called for what the display tells the host in a toast. Null in the old screens.
  final void Function(ReceiverNotice notice)? onNotice;

  /// The mode of a room that starts after End (the default mode; Locked never survives End,
  /// spec 4.3). Null: the current mode is kept.
  UiJoinMode? restartMode;

  /// The sender leaving does not end the receiver: the room stays with the **same code** and the
  /// display is idle again (spec 4.5, N8); the core keeps its room after a presenter left. The
  /// old receive page ends instead (and stops the core).
  final bool keepRoomOnPeerLeft;

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

  /// A presenter's hand-over token is stored (iOS, native-apps.md 20.5): its leaving is not a
  /// leave for the host (no `toast.left`) until the token is used or runs out.
  bool handoverPending = false;

  /// The presenter left with a hand-over pending while something was shown: the display stays
  /// in Presenting on black for at most [handoverHoldSeconds] (spec N32, 20.5 step 2). Nothing
  /// shown: Idle at once, as for any leave.
  bool handoverHold = false;
  Timer? _holdTimer;

  /// How long a hand-over token lives on the display (the core's own limit is the same).
  static const handoverHoldSeconds = 60;

  /// The connection check code of the connected presenter (`123 456`), or null until it
  /// connected. The presenter shows the same digits.
  String? connectionCode;

  /// The name of the display as senders see it (sent with `create_room`), or null.
  String? roomName;

  /// The code of the error that ended the receiver (`room_locked`, `connection_lost`, ...), or
  /// null. [error] has the English text for logs.
  String? errorCode;

  // --- join approval ---------------------------------------------------------------------

  /// Who may join (as confirmed by the server).
  UiJoinMode joinMode = UiJoinMode.open;

  /// Waiting requests, oldest first.
  List<JoinRequestView> requests = const [];

  /// Name of the connected sender, or null.
  String? peerName;

  /// Blocked senders (the host can unblock them).
  List<BlockedView> blocks = const [];

  /// Something to tell the host about the join settings, or null.
  JoinNote? joinNote;
  int discardedCount = 0;

  /// Mode and server of the last [start] (for the retry without an open room).
  UiJoinMode _startMode = UiJoinMode.open;
  String _startServer = '';
  bool _openRetried = false;
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

  /// Set once the hardware decoder failed during this run of the app: later sessions go straight
  /// to the software decoder instead of failing the same way again. Not stored on disk.
  static String? hardwareBrokenReason;

  /// Forget a remembered hardware failure (tests).
  @visibleForTesting
  static void resetHardwareBroken() => hardwareBrokenReason = null;

  /// A live picture is on screen: a decoded image, or a texture that knows its size.
  bool get hasLiveVideo =>
      image != null || (textureId != null && videoWidth > 0 && videoHeight > 0);

  /// Size of the live picture in pixels (0 x 0 if none).
  (int, int) get pictureSize =>
      image != null ? (image!.width, image!.height) : (videoWidth, videoHeight);

  // --- pause (docs/design/pause.md) --------------------------------------------------------

  /// The presenter paused its screen (it stopped sending video). Set by the core's
  /// `pauseChanged` event; only the presenter can clear it.
  bool pausedByPresenter = false;

  /// The display's own pause flag (the Pause control of the display). Only the display clears it.
  bool pausedByDisplay = false;

  /// The picture is frozen while either flag is set: the last picture stays, new ones are held.
  bool get frozen => pausedByPresenter || pausedByDisplay;

  /// The Pause control is offered: a live screen is on show (not an image, PDF page or link,
  /// which stay until replaced anyway).
  bool get canPause => isActive && shown == null && hasLiveVideo;

  /// Press Pause (`true`) or Resume (`false`) on the display: sets or clears the display's own
  /// flag. The core confirms with `pauseChanged`; the presenter is told.
  void setDisplayPaused(bool paused) {
    if (paused && !canPause) return;
    core.setDisplayPaused(paused);
  }

  /// The newest picture that arrived while frozen: shown when the picture is live again.
  VideoFrame? _held;

  /// Encoded units were dropped while frozen (hardware decoder): a key frame is needed when the
  /// picture is live again, because delta frames would refer to what was dropped.
  bool _hwNeedsKey = false;

  void _onPauseChanged(bool presenter, bool display) {
    final was = frozen;
    pausedByPresenter = presenter;
    pausedByDisplay = display;
    if (was && !frozen) {
      // Live again: what arrived meanwhile is shown at once, and the hardware decoder, which
      // missed units, asks for a key frame.
      final held = _held;
      _held = null;
      if (held != null) unawaited(_showHeld(held));
      if (_hwNeedsKey) {
        _hwNeedsKey = false;
        core.requestVideoKeyframe();
      }
    }
  }

  Future<void> _showHeld(VideoFrame frame) async {
    final session = _session;
    try {
      final img = await _toImage(frame, fetchTimeout);
      if (_disposed || session != _session || !isActive || frozen) {
        img.dispose();
        return;
      }
      _swapImage(img);
    } catch (e) {
      debugPrint('cometcast: cannot show a held picture: $e');
    }
  }

  /// The screen stopped (presentation ended, presenter left, new room): both flags go.
  void _clearPause() {
    pausedByPresenter = false;
    pausedByDisplay = false;
    _held = null;
    _hwNeedsKey = false;
  }

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

  /// The error behind [note] (the display's toast text depends on it), or null.
  UiContentError? noteError;

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

  /// Create a room on [server] and start listening. [joinMode] is who may join from the start.
  ///
  /// [roomName] is the display's name for senders ("Meeting room Elbe"); the server cleans it.
  void start(
    String server, {
    UiJoinMode joinMode = UiJoinMode.open,
    String? roomName,
  }) {
    _startMode = joinMode;
    _startServer = server;
    this.roomName = roomName;
    _openRetried = false;
    _begin(server, joinMode);
  }

  void _begin(String server, UiJoinMode joinMode) {
    _cancel();
    _newSession();
    _clearImage();
    _stopHardware();
    _startHardware();
    phase = ReceiverPhase.starting;
    roomCode = null;
    _endHandover();
    senderJoined = false;
    senderLeft = false;
    connectionCode = null;
    errorCode = null;
    this.joinMode = joinMode;
    requests = const [];
    peerName = null;
    blocks = const [];
    if (!_openRetried) joinNote = null;
    discardedCount = 0;
    connectionState = null;
    codec = null;
    _clearPause();
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
          joinMode: joinMode,
          roomName: roomName,
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
    _clearPause();
    requests = const [];
    _clearImage();
    _stopHardware();
    _clearContent();
    unawaited(_leaveFullscreen());
    notifyListeners();
  }

  void _onEvent(ReceiverUiEvent e) {
    switch (e) {
      case ReceiverUiEvent_RoomCode(:final code, joinMode: final mode):
        roomCode = code;
        joinMode = mode;
        phase = ReceiverPhase.running;
      case ReceiverUiEvent_JoinRequest(:final id, :final name, :final check):
        if (requests.every((r) => r.id != id)) {
          requests = [
            ...requests,
            JoinRequestView(
              id: id,
              name: name,
              check: check,
              receivedAt: joinClock(),
            ),
          ];
        }
      case ReceiverUiEvent_JoinRequestCancelled(:final id, :final reason):
        final gone = requests.where((r) => r.id == id).firstOrNull;
        if (gone != null) {
          if (reason == 'timeout') {
            onNotice?.call(NoticeExpired(gone.name));
          } else if (reason == 'cancelled' || reason == 'left') {
            onNotice?.call(NoticeWithdrawn(gone.name));
          }
        }
        requests = requests.where((r) => r.id != id).toList();
        if (reason == 'locked') {
          discardedCount++;
          joinNote = JoinNote.lockedDiscarded;
        }
      case ReceiverUiEvent_JoinModeChanged(:final mode):
        joinMode = mode;
        if (mode != UiJoinMode.locked && joinNote == JoinNote.lockedDiscarded) {
          joinNote = null;
          discardedCount = 0;
        }
      case ReceiverUiEvent_ServerNotice(:final code):
        onNotice?.call(NoticeServer(code));
        return;
      case ReceiverUiEvent_Blocked(:final id, :final name):
        if (blocks.every((b) => b.id != id)) {
          blocks = [...blocks, BlockedView(id: id, name: name)];
        }
      case ReceiverUiEvent_Unblocked(:final id):
        blocks = blocks.where((b) => b.id != id).toList();
      case ReceiverUiEvent_RoomEnded():
        // The host ended the session: the room and its code are gone. A new room (a new
        // code) is created in the same mode at once.
        onNotice?.call(const NoticeSessionEnded());
        _begin(_startServer, restartMode ?? joinMode);
        return;
      case ReceiverUiEvent_PeerJoined(:final name, :final handover):
        peerName = name;
        senderJoined = true;
        // A join with the display's own hand-over token: the person was approved before and
        // came back; no "connected" notice, no dialog.
        _endHandover();
        if (!handover) onNotice?.call(NoticeJoined(name));
      case ReceiverUiEvent_HandoverStored():
        handoverPending = true;
      case ReceiverUiEvent_HandoverExpired():
        _endHandover();
      case ReceiverUiEvent_ConnectionState(:final state):
        connectionState = state;
      case ReceiverUiEvent_ConnectionCode(:final sas):
        connectionCode = sas;
      case ReceiverUiEvent_PauseChanged(:final presenter, :final display):
        _onPauseChanged(presenter, display);
      case ReceiverUiEvent_PresentationStopped():
        // The presenter stopped but stays in the room: Idle with the same code, no notice.
        _resetPresenter();
        notifyListeners();
        return;
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
      case ReceiverUiEvent_ContentTransferProgress(
        :final received,
        :final total,
      ):
        final r = receiving;
        if (r == null) return;
        receiving = r.withProgress(received.toInt(), total.toInt());
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
        noteError = error;
        // The core closes the shown document when a PDF is refused.
        if (error == UiContentError.documentRejected ||
            error == UiContentError.workerUnavailable) {
          if (shown is ReceivedDocument) _replaceContent(null);
        }
      case ReceiverUiEvent_PeerLeft():
        senderLeft = true;
        final left = peerName;
        peerName = null;
        // A leave with a hand-over pending is no leave for the host (native-apps.md 20.5).
        final quiet = handoverPending;
        if (quiet && keepRoomOnPeerLeft && (hasLiveVideo || shown != null)) {
          handoverHold = true;
          _holdTimer?.cancel();
          _holdTimer = Timer(const Duration(seconds: handoverHoldSeconds), () {
            if (_disposed || !handoverHold) return;
            _endHandover();
            notifyListeners();
          });
        }
        if (left != null && !quiet) onNotice?.call(NoticeLeft(left));
        if (keepRoomOnPeerLeft) {
          // The room outlives the presenter: same code, same mode, block list and queue.
          senderJoined = false;
          senderLeft = false;
          _resetPresenter();
          notifyListeners();
          return;
        }
        // The core keeps the room open for the next sender; this page is done with it.
        unawaited(core.stopReceiver());
        _end();
        return;
      case ReceiverUiEvent_Error(:final code, :final message):
        // The server forbids open rooms: ask for one that needs approval instead of failing.
        if (code == 'open_rooms_disabled' &&
            !_openRetried &&
            _startMode == UiJoinMode.open) {
          _openRetried = true;
          joinNote = JoinNote.openDisabled;
          onNotice?.call(const NoticeOpenDisabled());
          _begin(_startServer, UiJoinMode.confirm);
          return;
        }
        error = message;
        errorCode = code;
        _end();
        return;
    }
    notifyListeners();
  }

  /// The hand-over is over (used, expired or the room is new): normal rules again.
  void _endHandover() {
    handoverPending = false;
    handoverHold = false;
    _holdTimer?.cancel();
    _holdTimer = null;
  }

  /// Forget what belonged to the connected presenter: picture, content, counters and the
  /// connection code. The room itself (code, mode, queue, blocks) is untouched. The hardware
  /// decoder starts over for the next presenter, as for a new room.
  void _resetPresenter() {
    _newSession();
    _clearImage();
    _stopHardware();
    _startHardware();
    connectionState = null;
    connectionCode = null;
    codec = null;
    _clearPause();
    packets = bytes = frames = keyframes = 0;
    imagesShown = 0;
    displayFps = 0;
    _shownAtLastStats = 0;
    videoStats = const VideoStatsView();
    avgFetchUs = avgImageUs = _fetchUsTotal = _imageUsTotal = _fetched = 0;
    _clearContent();
    _lastStatsMs = 0;
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
    final broken = hardwareBrokenReason;
    if (broken != null) {
      // Failed before in this run: stay with the software decoder.
      hardwareError = broken;
      return;
    }
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
    hardwareBrokenReason = message;
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
        if (frozen) {
          // Frozen: the texture keeps its last picture. The units are dropped, so a key frame
          // is asked for when the picture is live again.
          if (units.isNotEmpty) _hwNeedsKey = true;
          continue;
        }
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
        if (frozen) {
          // Frozen (docs/design/pause.md): the last picture stays; keep the newest one for later.
          _held = frame;
          continue;
        }
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
    noteError = null;
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

  // --- join approval (the host decides who may join) -------------------------------------

  /// Change who may join, at any time. The mode shown changes when the server confirms.
  void setJoinMode(UiJoinMode mode) => core.setJoinMode(mode);

  /// Allow or deny a waiting request. It leaves the list at once. With [block] (only a denial)
  /// the sender's address is also blocked for the rest of this room session.
  void decideJoin(BigInt id, {required bool allow, bool block = false}) {
    if (requests.every((r) => r.id != id)) return;
    if (block && !allow) {
      core.denyAndBlockJoin(id);
    } else {
      core.decideJoin(id: id, allow: allow);
    }
    requests = requests.where((r) => r.id != id).toList();
    notifyListeners();
  }

  /// Deny everybody who waits.
  void denyAllJoins() {
    if (requests.isEmpty) return;
    core.decideJoin(allow: false);
    requests = const [];
    notifyListeners();
  }

  /// End the presenter's session: remove the connected sender (optionally blocking it).
  /// UNCERTAIN: one sender per receiver, so the receiver ends (`senderLeft`) and a new room with a
  /// new code has to be started.
  void removePeer({bool block = false}) => core.removePeer(block: block);

  /// Take a sender off the block list.
  void unblock(BigInt id) => core.unblock(id);

  /// End the room session: everybody is dropped; a new room with a new code starts at once.
  bool endRoom() => core.endRoom();

  void dismissJoinNote() {
    if (joinNote == null) return;
    joinNote = null;
    discardedCount = 0;
    notifyListeners();
  }

  /// Stop receiving. The room code disappears (the room is gone).
  Future<void> stop() async {
    _cancel();
    _newSession();
    phase = ReceiverPhase.idle;
    roomCode = null;
    requests = const [];
    peerName = null;
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
    _holdTimer?.cancel();
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
