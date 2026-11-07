// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter session as the UI sees it (native-apps.md 20.1, N5): the room is joined once,
// on Connect; presentations come and go inside it; Stop keeps the join, Disconnect leaves.
// No widgets and no strings here: the pages read the fields, the codes are mapped to pages
// by `errorKindForCode` (lib/src/presenter/errors.dart).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';

enum PresenterSessionPhase {
  /// Not connected.
  idle,

  /// Connecting; the host has not answered yet (or the room is open and the join is under way).
  connecting,

  /// The room asks its host: waiting for the decision ([PresenterSessionController.check]).
  waiting,

  /// In the room, nothing shown ("What to show").
  joined,

  /// A presentation is connected.
  presenting,

  /// The room is gone or this device was removed ([PresenterSessionController.endedCode]).
  ended,

  /// Something failed ([PresenterSessionController.errorCode]).
  failed,
}

/// What happens to the content that is being shown.
enum PresenterContentPhase {
  none,

  /// An Office file is being converted to a PDF.
  converting,

  /// Checked, about to be sent.
  prepared,

  /// Bytes are on their way ([PresenterSessionController.progress]).
  sending,

  /// Delivered; page commands can follow.
  sent,
}

/// Errors of a presentation that could not start: the session stays joined and the person is
/// back at "What to show" (`capture_denied`: no page at all, spec 6). Every other error code
/// ends the session. Same list as the Rust documentation of `PresenterUiEvent`.
const presenterStartFailureCodes = {'capture_denied', 'capture_unavailable'};

class PresenterSessionController extends ChangeNotifier {
  PresenterSessionController({required this.core, this.udpBind});

  final CoreApi core;
  final String? udpBind;

  StreamSubscription<PresenterUiEvent>? _sub;
  bool _disposed = false;

  PresenterSessionPhase phase = PresenterSessionPhase.idle;

  /// The name the display gave its room (decision 49: lenient), or null.
  String? roomName;

  /// The check number and the wait while the host decides.
  int? check;
  int? timeoutSecs;

  /// The connection check code (`123 456`) of the running presentation, or null.
  String? connectionCode;

  /// The PDF page the display shows and the page count, or null (hidden until the display
  /// reports it; both buttons stay enabled meanwhile).
  int? page;
  int? pages;

  /// Previous and Next of a PDF page row: disabled at the ends once the display reported.
  bool get hasPreviousPage => page == null || page! > 1;
  bool get hasNextPage => page == null || pages == null || page! < pages!;

  // --- content -----------------------------------------------------------------------------

  PresenterContentPhase contentPhase = PresenterContentPhase.none;
  String? contentName;
  UiContentKind? contentKind;
  int contentSize = 0;
  int? contentPages;
  String? contentHost;

  /// Why the last file or link was refused (the session stays joined), or null.
  UiSendRejection? rejection;
  int sent = 0;
  int total = 0;

  /// 0 to 100.
  int get progress =>
      total <= 0 ? 0 : (sent * 100 ~/ total).clamp(0, 100).toInt();

  // --- results -----------------------------------------------------------------------------

  /// `removed_by_host` or `display_left` after [PresenterSessionPhase.ended].
  String? endedCode;

  /// The code and the English text (logs only) of the error behind
  /// [PresenterSessionPhase.failed], or of the last [startFailure].
  String? errorCode;
  String? errorMessage;

  /// A presentation could not start (see [presenterStartFailureCodes]); cleared by the next
  /// presentation. The session is still joined.
  String? startFailure;

  /// True once the room was joined, also after the end: "was waiting" and "was connected" differ
  /// for `room_not_found` (spec 6).
  bool wasJoined = false;

  bool get isJoined =>
      phase == PresenterSessionPhase.joined ||
      phase == PresenterSessionPhase.presenting;

  /// Join the room once. Replaces a running session.
  void connect({required String code, required String server, String? name}) {
    _cancel();
    _reset();
    phase = PresenterSessionPhase.connecting;
    notifyListeners();
    _sub = core
        .startPresenter(
          code: code,
          server: server.trim(),
          name: name,
          udpBind: udpBind,
        )
        .listen(
          _onEvent,
          onError: (Object e) {
            _fail('unknown', e.toString());
          },
          onDone: () {
            // The stream also ends after disconnect(); only an unexpected end is a failure.
            if (!_disposed &&
                (phase == PresenterSessionPhase.connecting ||
                    phase == PresenterSessionPhase.waiting ||
                    isJoined)) {
              _fail('connection_lost', 'the presenter stream ended');
            }
          },
        );
  }

  void _reset() {
    roomName = null;
    check = null;
    timeoutSecs = null;
    endedCode = null;
    errorCode = null;
    errorMessage = null;
    startFailure = null;
    wasJoined = false;
    _resetPresentation();
  }

  void _resetPresentation() {
    connectionCode = null;
    page = null;
    pages = null;
    contentPhase = PresenterContentPhase.none;
    contentName = null;
    contentKind = null;
    contentSize = 0;
    contentPages = null;
    contentHost = null;
    rejection = null;
    sent = 0;
    total = 0;
  }

  void _fail(String code, String message) {
    phase = PresenterSessionPhase.failed;
    errorCode = code;
    errorMessage = message;
    _cancel();
    notifyListeners();
  }

  void _onEvent(PresenterUiEvent e) {
    switch (e) {
      case PresenterUiEvent_WaitingForApproval(
        :final timeoutSecs,
        :final check,
        :final roomName,
      ):
        phase = PresenterSessionPhase.waiting;
        this.timeoutSecs = timeoutSecs;
        this.check = check;
        this.roomName = roomName ?? this.roomName;
      case PresenterUiEvent_Joined(:final roomName):
        phase = PresenterSessionPhase.joined;
        wasJoined = true;
        this.roomName = roomName ?? this.roomName;
      case PresenterUiEvent_ConnectionCode(:final sas):
        connectionCode = sas;
      case PresenterUiEvent_Page(:final page, :final pages):
        this.page = page;
        this.pages = pages;
      case PresenterUiEvent_Converting(:final name):
        _resetPresentation();
        startFailure = null;
        contentPhase = PresenterContentPhase.converting;
        contentName = name;
      case PresenterUiEvent_ContentPrepared(
        :final name,
        :final kind,
        :final size,
        :final pages,
        :final host,
      ):
        contentPhase = PresenterContentPhase.prepared;
        contentName = name;
        contentKind = kind;
        contentSize = size.toInt();
        contentPages = pages;
        contentHost = host;
        rejection = null;
      case PresenterUiEvent_ContentRejected(:final reason):
        _resetPresentation();
        rejection = reason;
      case PresenterUiEvent_Progress(:final sent, :final total):
        contentPhase = PresenterContentPhase.sending;
        this.sent = sent.toInt();
        this.total = total.toInt();
      case PresenterUiEvent_ContentSent():
        contentPhase = PresenterContentPhase.sent;
      case PresenterUiEvent_Presenting():
        phase = PresenterSessionPhase.presenting;
        startFailure = null;
      case PresenterUiEvent_PresentingStopped():
        // Stop keeps the join: back to "What to show".
        phase = PresenterSessionPhase.joined;
        _resetPresentation();
      case PresenterUiEvent_Ended(:final code):
        phase = PresenterSessionPhase.ended;
        endedCode = code;
        _cancel();
      case PresenterUiEvent_Error(:final code, :final message):
        if (presenterStartFailureCodes.contains(code) &&
            phase != PresenterSessionPhase.connecting &&
            phase != PresenterSessionPhase.waiting) {
          startFailure = code;
          errorCode = code;
          errorMessage = message;
        } else {
          _fail(code, message);
          return;
        }
    }
    notifyListeners();
  }

  // --- what the person does ------------------------------------------------------------------

  /// Share the screen. [screen] set = X11 capture, null = the system picker of the desktop.
  /// Mobile: use [presentEncoded] after the capture started. False if there is no session.
  bool presentScreen([ScreenRequest? screen]) {
    _resetPresentation();
    startFailure = null;
    return core.presentScreen(screen);
  }

  /// Share the H.264 that the host pushes (Android, iOS).
  bool presentEncoded() {
    _resetPresentation();
    startFailure = null;
    return core.presentEncoded();
  }

  /// Show a file. The bytes never cross the bridge.
  bool presentFile(String path) => _present(ContentSource.file(path: path));

  /// Show a link (the core validates it; a bad one is a `rejection`).
  bool presentLink(String url) => _present(ContentSource.link(url: url));

  bool _present(ContentSource source) {
    _resetPresentation();
    startFailure = null;
    final ok = core.presentContent(source);
    if (ok) {
      contentPhase = PresenterContentPhase.prepared;
      notifyListeners();
    }
    return ok;
  }

  /// Next page, previous page or a page number of the shown PDF.
  bool sendCommand(UiContentCmd cmd) => core.sendContentCmd(cmd);

  /// End the presentation only. The display goes Idle with the same code and the session stays
  /// joined (`PresentingStopped` brings the phase back to [PresenterSessionPhase.joined]).
  bool stopPresenting() => core.stopPresenting();

  /// Leave the room (Disconnect, withdraw while waiting). Ends the session at once.
  Future<void> disconnect() async {
    _cancel();
    final was = phase;
    _reset();
    phase = PresenterSessionPhase.idle;
    if (was != PresenterSessionPhase.idle) notifyListeners();
    await core.stopPresenter();
  }

  void _cancel() {
    _sub?.cancel();
    _sub = null;
  }

  @override
  void dispose() {
    _disposed = true;
    _cancel();
    // Fire and forget: the screen is gone, the core must not keep a session open.
    if (phase != PresenterSessionPhase.idle) unawaited(core.stopPresenter());
    super.dispose();
  }
}
