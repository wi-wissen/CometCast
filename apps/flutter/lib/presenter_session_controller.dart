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

/// Which banner the presenter's Presenting page shows (docs/design/pause.md, rule 5).
enum PauseBanner {
  /// "Paused: the audience sees the last picture." (the person's own pause, confirmed)
  presenter,

  /// "The display has frozen the picture." (only the display froze)
  display,
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
const presenterStartFailureCodes = {
  'capture_denied',
  'capture_unavailable',
  'capture_os_old',
  'capture_no_portal',
};

class PresenterSessionController extends ChangeNotifier {
  PresenterSessionController({required this.core, this.udpBind});

  final CoreApi core;
  final String? udpBind;

  StreamSubscription<PresenterUiEvent>? _sub;
  bool _disposed = false;

  PresenterSessionPhase phase = PresenterSessionPhase.idle;

  /// The name the display gave its room (decision 49: lenient), or null.
  String? roomName;

  /// A TV session waits for the code the TV shows (`PairingRequired`, until `Joined`).
  bool pairingRequired = false;

  /// The code the person typed for a TV was wrong (inline `pair.wrong`, the page stays; the
  /// field is emptied). Cleared by the next [pair] call.
  bool pairWrong = false;

  /// The TV cannot show the link or photo (`tv_unsupported_content`, inline `link.tvUnsupported`);
  /// the person stays connected. Cleared by the next presentation.
  bool tvUnsupported = false;

  /// The presented file is an Office file that was converted to a PDF here (spec 15.6): the
  /// limits line and the way out show for the whole presentation.
  bool converted = false;

  /// The check number and the wait while the host decides.
  int? check;
  int? timeoutSecs;

  /// The connection check code (`123 456`) of the running presentation, or null.
  String? connectionCode;

  /// The PDF page the display shows and the page count, or null (hidden until the display
  /// reports it; both buttons stay enabled meanwhile).
  int? page;
  int? pages;

  // --- pause (docs/design/pause.md) --------------------------------------------------------

  /// A screen (or pushed video) is being presented in this session, so Pause applies. Content
  /// presentations and TV sessions have no screen of ours to pause.
  bool presentingScreen = false;

  /// The person pressed Pause (their own flag as asked: the button reads Resume at once and the
  /// video stops). The display's echo confirms it in [pauseConfirmed].
  bool pauseRequested = false;

  /// The display holds the presenter's flag (its `pause_state` echoed the request).
  bool pauseConfirmed = false;

  /// The display froze the picture on its own (its own flag).
  bool displayFrozen = false;

  bool _tv = false;

  /// The Pause button is offered: a screen is presented, not on a TV, and the session is
  /// presenting. Not on iOS during a broadcast: the extension has no UI and runs elsewhere.
  bool get canPause =>
      phase == PresenterSessionPhase.presenting && presentingScreen && !_tv;

  /// Banner text to show: `presenter` while the display confirmed the person's pause,
  /// `display` while only the display froze the picture, or null. The presenter's text wins when
  /// both are set. A request the display has not echoed yet shows no banner (pending).
  PauseBanner? get pauseBanner => !canPause
      ? null
      : pauseConfirmed
      ? PauseBanner.presenter
      : displayFrozen
      ? PauseBanner.display
      : null;

  /// Pause (`true`) or resume (`false`) the shown screen. False if nothing can be paused or
  /// there is no session. The request counts at once; `pauseState` confirms it.
  bool setPaused(bool paused) {
    if (!canPause || pauseRequested == paused) return false;
    final ok = core.setPresenterPaused(paused);
    if (ok) {
      pauseRequested = paused;
      if (!paused) pauseConfirmed = false;
      notifyListeners();
    }
    return ok;
  }

  void _resetPause() {
    presentingScreen = false;
    pauseRequested = false;
    pauseConfirmed = false;
    displayFrozen = false;
  }

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

  /// Join the room once. Replaces a running session. [handover] is a single-use token that
  /// the display holds for this person (iOS, native-apps.md 20.5): the display lets the join in
  /// without a dialog; without it, or when the display does not know it, the host is asked.
  void connect({
    required String code,
    required String server,
    String? name,
    String? handover,
  }) {
    _tv = false;
    _listen(
      core.startPresenter(
        code: code,
        server: server.trim(),
        name: name,
        handover: handover,
        udpBind: udpBind,
      ),
    );
  }

  /// Open a session to a TV without Comet Cast (`start_tv_presenter`, spec 15.5): the same
  /// events, plus [pairingRequired]; `Joined` carries the TV's name.
  void connectTv({required String id}) {
    _tv = true;
    _listen(core.startTvPresenter(id: id));
  }

  /// The code the TV shows (AirPlay, the first time). False: wrong, [pairWrong] is set.
  Future<bool> pair(String pin) async {
    pairWrong = false;
    final ok = await core.tvPair(pin);
    if (_disposed) return ok;
    if (ok) {
      pairingRequired = false;
    } else {
      pairWrong = true;
    }
    notifyListeners();
    return ok;
  }

  void _listen(Stream<PresenterUiEvent> events) {
    _cancel();
    _reset();
    phase = PresenterSessionPhase.connecting;
    notifyListeners();
    _sub = events.listen(
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
    pairingRequired = false;
    pairWrong = false;
    _resetPresentation();
  }

  void _resetPresentation() {
    _resetPause();
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
    converted = false;
    tvUnsupported = false;
  }

  void _fail(String code, String message) {
    _answerHandover(false);
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
      case PresenterUiEvent_PairingRequired():
        // A TV asks for the code it shows (fix list F10 draws the pair page).
        pairingRequired = true;
      case PresenterUiEvent_Joined(:final roomName):
        pairingRequired = false;
        phase = PresenterSessionPhase.joined;
        wasJoined = true;
        this.roomName = roomName ?? this.roomName;
      case PresenterUiEvent_ConnectionCode(:final sas):
        connectionCode = sas;
      case PresenterUiEvent_Page(:final page, :final pages):
        this.page = page;
        this.pages = pages;
      case PresenterUiEvent_PauseState(:final presenter, :final display):
        // The display's flags are the truth: the person's own flag counts as confirmed only
        // when the display echoes it.
        pauseConfirmed = presenter;
        displayFrozen = display;
      case PresenterUiEvent_Converting(:final name):
        _resetPresentation();
        startFailure = null;
        contentPhase = PresenterContentPhase.converting;
        contentName = name;
        converted = true;
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
      case PresenterUiEvent_HandoverResult(:final ok):
        _answerHandover(ok);
      case PresenterUiEvent_Presenting():
        phase = PresenterSessionPhase.presenting;
        startFailure = null;
      case PresenterUiEvent_PresentingStopped():
        // Stop keeps the join: back to "What to show".
        phase = PresenterSessionPhase.joined;
        _resetPresentation();
      case PresenterUiEvent_Ended(:final code):
        _answerHandover(false);
        phase = PresenterSessionPhase.ended;
        endedCode = code;
        _cancel();
      case PresenterUiEvent_Error(:final code, :final message):
        if (code == 'tv_pair_wrong') {
          pairWrong = true; // inline, the stream goes on
        } else if (code == 'tv_unsupported_content') {
          _resetPresentation();
          tvUnsupported = true;
        } else if (presenterStartFailureCodes.contains(code) &&
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
    final ok = core.presentScreen(screen);
    presentingScreen = ok;
    return ok;
  }

  /// Share the H.264 that the host pushes (Android, iOS).
  bool presentEncoded() {
    _resetPresentation();
    startFailure = null;
    final ok = core.presentEncoded();
    presentingScreen = ok;
    return ok;
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

  Completer<bool>? _handover;

  void _answerHandover(bool ok) {
    final c = _handover;
    _handover = null;
    if (c != null && !c.isCompleted) c.complete(ok);
  }

  /// iOS hand-over (native-apps.md 20.5, step 1): ask the display to hold the single-use
  /// [token] for the broadcast extension. True when the display answered; false when it did
  /// not (an old or a web display, no session, a lost connection, [timeout]): the extension then
  /// joins the ordinary way and the host is asked.
  Future<bool> requestHandover(
    String token, {
    Duration timeout = const Duration(seconds: 20),
  }) async {
    if (!isJoined) return false;
    _answerHandover(false);
    final c = Completer<bool>();
    _handover = c;
    if (!core.requestHandover(token)) {
      _answerHandover(false);
      return false;
    }
    return c.future.timeout(
      timeout,
      onTimeout: () {
        if (identical(_handover, c)) _handover = null;
        return false;
      },
    );
  }

  /// Leave the room (Disconnect, withdraw while waiting). Ends the session at once.
  Future<void> disconnect() async {
    _answerHandover(false);
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
