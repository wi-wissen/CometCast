// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter (sender) as a whole: first run, waiting, what to show, presenting, ended and the
// error pages (spec 5 and 6, native-apps.md 4, 5 and 20.1). The room is joined once, on Connect,
// by PresenterSessionController; Stop keeps the join, Disconnect leaves. Pages are chosen by the
// structured codes of the core (errors.dart), never by message text. The only other sender is the
// iOS broadcast extension, which makes its own connection (SenderController, see _startIos).

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../codec_controller.dart';
import '../../capture_feeder.dart';
import '../../core/android_capture.dart';
import '../../core/core_api.dart';
import '../../core/ios_broadcast.dart';
import '../../core/launch_intake.dart';
import '../../core/link_target.dart';
import '../../join_state.dart';
import '../../presenter_session_controller.dart';
import '../../sender_controller.dart';
import '../../settings.dart';
import 'errors.dart';
import 'file_picker.dart';
import 'kit.dart';
import 'pages.dart';

enum PresenterPage {
  firstRun,
  what,
  file,
  link,
  codec,
  waiting,
  presenting,
  bye,
  error,
  relay,
}

/// A QR scanner (system UI). Not built in this round: null hides the scan button.
typedef ScanQr = Future<String?> Function();

String errorsText(AppLocalizations t, String code) => switch (code) {
  'bad_hello' => t.errorsBadHello,
  'proto_mismatch' => t.errorsProtoMismatch,
  'wrong_role' => t.errorsWrongRole,
  'room_not_found' => t.errorsRoomNotFound,
  'room_full' => t.errorsRoomFull,
  'not_in_room' => t.errorsNotInRoom,
  'bad_message' => t.errorsBadMessage,
  'altcha_required' => t.errorsAltchaRequired,
  'altcha_invalid' => t.errorsAltchaInvalid,
  'altcha_failed' => t.errorsAltchaFailed,
  'altcha_too_hard' => t.errorsAltchaTooHard,
  'altcha_expired' => t.errorsAltchaExpired,
  'rate_limited' => t.errorsRateLimited,
  'banned' => t.errorsBanned,
  'forbidden' => t.errorsForbidden,
  'server_full' => t.errorsServerFull,
  'connection_lost' => t.errorsConnectionLost,
  'peer_failed' => t.errorsPeerFailed,
  'capture_denied' => t.errorsCaptureDenied,
  'capture_failed' => t.errorsCaptureFailed,
  'join_pending' => t.errorsJoinPending,
  'join_denied' => t.errorsJoinDenied,
  'room_locked' => t.errorsRoomLocked,
  'room_locked_waiting' => t.errorsRoomLockedWaiting,
  'approval_timeout' => t.errorsApprovalTimeout,
  'removed_by_host' => t.errorsRemovedByHost,
  'open_rooms_disabled' => t.errorsOpenRoomsDisabled,
  'no_such_request' => t.errorsNoSuchRequest,
  'no_such_block' => t.errorsNoSuchBlock,
  'block_list_full' => t.errorsBlockListFull,
  _ => t.errorsUnknown,
};

/// The structured code of a failed iOS broadcast (the extension reports its own names).
String iosErrorCode(IosBroadcastError? e) => switch (e) {
  IosBroadcastError.roomNotFound ||
  IosBroadcastError.invalidRoomCode => 'room_not_found',
  IosBroadcastError.peerLeft => 'display_left',
  IosBroadcastError.serverUnreachable ||
  IosBroadcastError.connectionLost ||
  IosBroadcastError.invalidRelay => 'connection_lost',
  _ => 'capture_failed',
};

class PresenterRoot extends StatefulWidget {
  const PresenterRoot({
    super.key,
    required this.core,
    required this.settings,
    required this.pickFile,
    this.androidCapture,
    this.iosBroadcast,
    this.scanQr,
    this.onBeDisplay,
    this.onMiracast,
    this.initialCode,
    this.codeFromLink = true,
    this.relayToConfirm,
    this.links,
    this.linkStream,
    this.desktop,
    this.device,
    this.codec,
    this.firstLaunch = false,
  });

  final CoreApi core;
  final AppSettings settings;
  final PickFile pickFile;
  final AndroidCapture? androidCapture;
  final IosBroadcast? iosBroadcast;
  final ScanQr? scanQr;

  /// Tablets and computers: starts the display setup (`app.beDisplay`).
  final VoidCallback? onBeDisplay;

  /// Windows: opens the Miracast receiver (`app.miracast`).
  final VoidCallback? onMiracast;

  /// A code that came from a link, the install referrer or a scan.
  final String? initialCode;
  final bool codeFromLink;

  /// A server address from a link that the app did not know: confirmed first (spec 7.2).
  final String? relayToConfirm;

  /// Turns a scanned code or a link into a chip, a relay confirmation or nothing, and remembers
  /// a confirmed relay (native-apps.md 20.4). Null in tests that do not scan.
  final LinkHandler? links;

  /// Links that arrive while the app runs (App Links, Universal Links).
  final Stream<String>? linkStream;

  /// Computers (subtitle `what.screenLaptop`, no scan button); null = decide by platform.
  final bool? desktop;

  /// Phone, tablet or computer (decision N15); null = by platform and window size.
  final PresenterDevice? device;

  /// The H.264 codec state for desktop screen sharing (a fake in tests).
  final CodecController? codec;

  /// First launch after the install and it brought no code: `app.scanAgainBody` is shown.
  final bool firstLaunch;

  @override
  State<PresenterRoot> createState() => PresenterRootState();
}

class PresenterRootState extends State<PresenterRoot> {
  PresenterPage _page = PresenterPage.firstRun;
  final _code = TextEditingController();
  final _name = TextEditingController();
  final _url = TextEditingController();
  String? _chip;
  bool _chipFromLink = true;
  bool _codeEmpty = false;
  String? _roomCode;
  String? _linkError;
  String? _fileError;
  ErrorKind _error = ErrorKind.other;
  String? _errorCode;
  bool _errorKeepsSession = false;
  bool _lockedWhile = false;
  bool _relayWhy = false;
  PresentingKind _kind = PresentingKind.screen;
  DateTime? _busyAt;
  bool _waited = false;
  Timer? _tick;
  DateTime _waitStart = DateTime.now();
  DateTime _refusedAt = DateTime.now();
  CodecController? _codecCtl;
  PresenterSessionPhase _lastPhase = PresenterSessionPhase.idle;
  String? _lastStartFailure;
  UiSendRejection? _lastRejection;
  StreamSubscription<String>? _linkSub;

  late final PresenterSessionController _session = PresenterSessionController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
  );

  /// Android: the capture service and the encoder that feed the session.
  CaptureFeeder? _feeder;

  /// iOS only: the broadcast extension joins on its own, outside the session.
  SenderController? _ios;

  bool get _desktop =>
      widget.desktop ?? !(Platform.isAndroid || Platform.isIOS);

  PresenterDevice _deviceKind(BuildContext context) {
    final d = widget.device;
    if (d != null) return d;
    if (_desktop) return PresenterDevice.computer;
    return MediaQuery.sizeOf(context).shortestSide >= 600
        ? PresenterDevice.tablet
        : PresenterDevice.phone;
  }

  @override
  void initState() {
    super.initState();
    _name.text = widget.settings.displayName;
    if (widget.initialCode != null) {
      _chip = widget.initialCode;
      _chipFromLink = widget.codeFromLink;
      _roomCode = widget.initialCode;
    }
    if (widget.relayToConfirm != null) _page = PresenterPage.relay;
    _session.addListener(_onSession);
    _linkSub = widget.linkStream?.listen(_onLink);
    widget.settings.discover(widget.core);
  }

  @override
  void dispose() {
    _tick?.cancel();
    _linkSub?.cancel();
    _session.removeListener(_onSession);
    _session.dispose();
    _releaseCapture();
    _ios?.dispose();
    if (widget.codec == null) {
      _codecCtl?.dispose(); // an injected one is not ours
    }
    widget.settings.cancelDiscovery();
    _code.dispose();
    _name.dispose();
    _url.dispose();
    super.dispose();
  }

  String get _server => widget.settings.serverAddress.trim();
  String? get _myName {
    final n = cleanDisplayName(_name.text);
    return n.isEmpty ? null : n;
  }

  void _go(PresenterPage p) {
    setState(() => _page = p);
    _tick?.cancel();
    if (p == PresenterPage.waiting ||
        (p == PresenterPage.error && _error == ErrorKind.denied)) {
      _tick = Timer.periodic(const Duration(seconds: 1), (_) {
        if (mounted) setState(() {});
      });
    }
  }

  // --- first run -------------------------------------------------------------------------

  void _connect() {
    final raw = _chip ?? _code.text;
    if (raw.trim().isEmpty) {
      setState(() => _codeEmpty = true);
      return;
    }
    final check = widget.core.checkRoomCode(
      raw.replaceAll(RegExp(r'[\s-]'), ''),
    );
    if (check is! RoomCodeCheck_Valid) {
      // Wrong characters or length: the error page, without asking the server.
      _error = ErrorKind.code;
      _errorCode = null;
      _lockedWhile = false;
      _relayWhy = false;
      _errorKeepsSession = false;
      _go(PresenterPage.error);
      return;
    }
    widget.settings.setDisplayName(_name.text);
    setState(() {
      _roomCode = check.code;
      _codeEmpty = false;
    });
    _beginSession();
  }

  /// Join once: the host is asked here, not again for what is shown next (native-apps.md 20.1).
  void _beginSession() {
    _waited = false;
    _lastPhase = PresenterSessionPhase.idle;
    _lastStartFailure = null;
    _session.connect(code: _roomCode!, server: _server, name: _myName);
    setState(() {});
  }

  /// The scan found something that is not a code of ours (shown as `app.scanWrong` by the
  /// first-run page, spec 5.3); cleared by the next scan.
  bool scanWrong = false;

  /// The server of a scanned code or link that waits for the relay confirmation page.
  String? _scannedRelay;

  Future<void> _scan() async {
    final scanned = await widget.scanQr?.call();
    if (scanned == null || !mounted) return; // cancelled: nothing changes
    final outcome = widget.links?.fromUrl(scanned);
    if (outcome == null || outcome is LinkNothing) {
      setState(() => scanWrong = true);
      return;
    }
    scanWrong = false;
    _takeOutcome(outcome, fromLink: false);
  }

  /// A link that arrives while the app runs: the same rules as the one that started the app.
  void _onLink(String link) {
    if (!mounted) return;
    if (_page != PresenterPage.firstRun &&
        _page != PresenterPage.error &&
        _page != PresenterPage.bye) {
      return; // somebody is connected: a link changes nothing
    }
    final outcome = widget.links?.fromUrl(link);
    if (outcome == null || outcome is LinkNothing) return;
    _takeOutcome(outcome, fromLink: true);
  }

  void _takeOutcome(LinkOutcome outcome, {required bool fromLink}) {
    setState(() {
      switch (outcome) {
        case LinkUse(:final target):
          _chip = target.code;
          _chipFromLink = fromLink;
          _roomCode = target.code;
          widget.links?.useServer(target.server);
          if (_page != PresenterPage.firstRun) _page = PresenterPage.firstRun;
        case LinkConfirm(:final target):
          // Nothing connects before the person decided; the chip keeps the code.
          _chip = target.code;
          _chipFromLink = fromLink;
          _roomCode = target.code;
          _scannedRelay = target.server;
          _page = PresenterPage.relay;
        case LinkNothing():
          break;
      }
    });
  }

  // --- errors ----------------------------------------------------------------------------

  /// The error page for [code] (`errorKindForCode`). [keepsSession]: the presenter is still in
  /// the room (a presentation that could not start): Try again goes back to "What to show".
  void _showError(String code, {bool keepsSession = false}) {
    if (code == 'rate_limited' || code == 'banned' || code == 'server_full') {
      _busyAt = DateTime.now();
    }
    final recent =
        _busyAt != null &&
        DateTime.now().difference(_busyAt!) < const Duration(seconds: 60);
    final r = errorKindForCode(
      code,
      wasWaiting: _waited || _session.wasJoined,
      recentBusy: recent,
    );
    if (r == null) {
      // `capture_denied`: no page, back to "What to show".
      if (_session.isJoined) _go(PresenterPage.what);
      return;
    }
    _relayWhy = displayOffWhyRelay(code);
    _lockedWhile = code == 'room_locked_waiting';
    if (r.$1 == ErrorKind.denied) _refusedAt = joinClock();
    _error = r.$1;
    _errorCode = r.$2;
    _errorKeepsSession = keepsSession;
    _go(PresenterPage.error);
  }

  int get _cooldown {
    final left =
        deniedCooldownSeconds - joinClock().difference(_refusedAt).inSeconds;
    return left < 0 ? 0 : left;
  }

  void _errorAction(ErrorAction a) {
    switch (a) {
      case ErrorAction.retype:
      case ErrorAction.rescan:
        unawaited(_session.disconnect());
        _chip = null;
        _code.clear();
        _roomCode = null;
        _go(PresenterPage.firstRun);
        if (a == ErrorAction.rescan) unawaited(_scan());
      case ErrorAction.retry || ErrorAction.ask:
        if (_errorKeepsSession && _session.isJoined) {
          _go(PresenterPage.what);
        } else if (_roomCode != null) {
          _go(PresenterPage.firstRun);
          _beginSession();
        } else {
          _go(PresenterPage.firstRun);
        }
      case ErrorAction.showFile:
        _go(PresenterPage.file);
      case ErrorAction.showLink:
        _go(PresenterPage.link);
    }
  }

  // --- reacting to the session -----------------------------------------------------------

  void _onSession() {
    if (!mounted) return;
    final c = _session;
    final t = AppLocalizations.of(context);
    final phase = c.phase;
    final changed = phase != _lastPhase;
    final from = _lastPhase;
    _lastPhase = phase;
    if (changed) {
      switch (phase) {
        case PresenterSessionPhase.waiting:
          _waited = true;
          _waitStart = DateTime.now();
          _go(PresenterPage.waiting);
        case PresenterSessionPhase.joined:
          widget.settings.rememberServer();
          if (from == PresenterSessionPhase.presenting) _releaseCapture();
          if (_page == PresenterPage.firstRun ||
              _page == PresenterPage.waiting ||
              _page == PresenterPage.presenting) {
            _go(PresenterPage.what);
          }
        case PresenterSessionPhase.presenting:
          _kind = switch (c.contentKind) {
            UiContentKind.pdf || UiContentKind.image => PresentingKind.file,
            UiContentKind.link => PresentingKind.link,
            _ when c.contentPhase != PresenterContentPhase.none =>
              PresentingKind.file,
            _ => _kind,
          };
          _go(PresenterPage.presenting);
        case PresenterSessionPhase.ended:
          _releaseCapture();
          _showError(c.endedCode ?? 'removed_by_host');
        case PresenterSessionPhase.failed:
          _releaseCapture();
          _showError(c.errorCode ?? 'unknown');
        case PresenterSessionPhase.idle || PresenterSessionPhase.connecting:
          break;
      }
    }
    final sf = c.startFailure;
    if (sf != _lastStartFailure) {
      _lastStartFailure = sf;
      if (sf != null) {
        _releaseCapture();
        _showError(sf, keepsSession: true);
      }
    }
    if (c.rejection != _lastRejection) {
      _lastRejection = c.rejection;
      if (c.rejection != null) {
        _fileError = sendRejectionText(t, c.rejection!);
        if (_page == PresenterPage.link) _linkError = _fileError;
      }
    }
    setState(() {});
  }

  // --- starting what to show -------------------------------------------------------------

  Future<void> _startScreen() async {
    _kind = PresentingKind.screen;
    if (widget.iosBroadcast != null) return _startIos();
    if (widget.androidCapture != null) return _startAndroid();
    if (_desktop) {
      // Desktop capture is encoded with OpenH264, which release builds download first (5.8).
      final codec =
          widget.codec ??
          (_codecCtl ??= CodecController(
            core: widget.core,
            settings: widget.settings,
          )..init());
      if (!codec.ready) {
        _codecCtl = codec;
        _go(PresenterPage.codec);
        return;
      }
    }
    _session.presentScreen(
      ScreenRequest(
        display: widget.settings.captureDisplay,
        fps: 15,
        bitrateKbps: 2000,
      ),
    );
  }

  Future<void> _startAndroid() async {
    final t = AppLocalizations.of(context);
    _releaseCapture();
    late final CaptureFeeder feeder;
    feeder = CaptureFeeder(
      core: widget.core,
      capture: widget.androidCapture!,
      // The system or the person ended the projection: the same as Stop (no error page).
      onStopped: (_) {
        if (_feeder != feeder) return;
        _feeder = null;
        _session.stopPresenting();
        if (mounted && _page == PresenterPage.presenting) {
          _go(PresenterPage.what);
        }
      },
      onError: (message) {
        if (_feeder != feeder) return;
        _feeder = null;
        _session.stopPresenting();
        if (mounted) _showError('capture_failed', keepsSession: true);
      },
    );
    _feeder = feeder;
    final CaptureStartResult result;
    try {
      result = await feeder.start(
        CaptureNotificationTexts(
          title: t.captureNotificationTitle,
          text: t.captureNotificationText,
          stopLabel: t.captureNotificationStop,
        ),
      );
    } catch (_) {
      if (_feeder == feeder) _feeder = null;
      if (mounted) _showError('capture_failed', keepsSession: true);
      return;
    }
    if (!mounted || _feeder != feeder) return;
    if (result != CaptureStartResult.started) {
      // Cancelling the system dialog goes back without an error (spec 5.5).
      _feeder = null;
      return;
    }
    if (!_session.presentEncoded()) _releaseCapture();
  }

  /// iOS: the broadcast extension joins on its own connection, so the session is left first and
  /// the extension is asked in the system sheet (docs/design/implementation-questions-flutter.md,
  /// F2). When it ends the person is back at the first page with the same code.
  Future<void> _startIos() async {
    final code = _roomCode;
    if (code == null) return;
    await _session.disconnect();
    if (!mounted) return;
    final ios = (_ios ??= SenderController(
      core: widget.core,
      udpBind: widget.settings.udpBind,
      iosBroadcast: widget.iosBroadcast,
    )..addListener(_onIos));
    widget.settings.rememberServer();
    _go(PresenterPage.firstRun);
    await ios.startBroadcast(code: code, server: _server);
  }

  void _onIos() {
    final c = _ios;
    if (c == null || !mounted) return;
    switch (c.phase) {
      case SenderPhase.waitingApproval:
        final w = c.approval;
        if (w != null && _page != PresenterPage.waiting) {
          _waited = true;
          _waitStart = DateTime.now();
          _go(PresenterPage.waiting);
        }
      case SenderPhase.running:
        if (_page != PresenterPage.presenting) {
          _kind = PresentingKind.screen;
          _go(PresenterPage.presenting);
        }
      case SenderPhase.ended || SenderPhase.finished:
        if (c.refusal != null) {
          _refusedAt = joinClock();
          final (k, w) = refusalPage(c.refusal!);
          _error = k;
          _errorCode = null;
          _lockedWhile = w;
          _errorKeepsSession = false;
          _go(PresenterPage.error);
        } else if (c.broadcastError != null || c.error != null) {
          _showError(iosErrorCode(c.broadcastError));
        } else if (_page != PresenterPage.error) {
          _go(PresenterPage.firstRun);
        }
      case SenderPhase.idle:
        if (c.broadcastNotStarted && _page != PresenterPage.firstRun) {
          _go(PresenterPage.firstRun);
        }
      case SenderPhase.starting:
        break;
    }
  }

  void _pickFile() async {
    final t = AppLocalizations.of(context);
    final path = await widget.pickFile(t.sendContentPickerTypes);
    if (path == null || !mounted) return; // cancelling changes nothing
    _fileError = null;
    _kind = PresentingKind.file;
    _session.presentFile(path);
    setState(() {});
  }

  void _sendLink() {
    final t = AppLocalizations.of(context);
    final check = widget.core.validateLink(_url.text.trim());
    if (check is! LinkCheck_Valid) {
      setState(() => _linkError = t.sendContentLinkInvalid);
      return;
    }
    _linkError = null;
    _kind = PresentingKind.link;
    _session.presentLink(check.url);
    setState(() {});
  }

  Future<void> _releaseCapture() async {
    final f = _feeder;
    _feeder = null;
    if (f != null) await f.stop();
  }

  // --- back (native-apps.md 5) -----------------------------------------------------------

  Future<bool> handleBack() async {
    switch (_page) {
      case PresenterPage.firstRun:
        return false; // leaves the app
      case PresenterPage.waiting:
        await _withdraw();
      case PresenterPage.what:
        await _disconnect();
      case PresenterPage.file || PresenterPage.link || PresenterPage.codec:
        _go(PresenterPage.what);
      case PresenterPage.presenting:
        break; // Stop is the only way out
      case PresenterPage.error || PresenterPage.bye || PresenterPage.relay:
        _go(PresenterPage.firstRun);
    }
    return true;
  }

  Future<void> _withdraw() async {
    await _releaseCapture();
    await _ios?.stop();
    await _session.disconnect();
    _go(PresenterPage.firstRun);
  }

  /// Leaves the room (the display shows `toast.left`).
  Future<void> _disconnect() async {
    await _releaseCapture();
    await _session.disconnect();
    _roomCode = null;
    _chip = null;
    _go(PresenterPage.bye);
  }

  /// Ends the presentation only: the presenter stays joined (decision N5).
  Future<void> _stop() async {
    if (_ios != null && _ios!.isActive) {
      await _ios!.stop();
      _go(PresenterPage.firstRun);
      return;
    }
    _session.stopPresenting();
    await _releaseCapture();
    _go(PresenterPage.what);
  }

  // --- build -----------------------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return PresenterFrame(
      child: PopScope(
        canPop: _page == PresenterPage.firstRun,
        onPopInvokedWithResult: (didPop, _) {
          if (!didPop) handleBack();
        },
        child: Material(type: MaterialType.transparency, child: _buildPage(t)),
      ),
    );
  }

  Widget _buildPage(AppLocalizations t) {
    final c = _session;
    switch (_page) {
      case PresenterPage.firstRun:
        return FirstRunPage(
          codeController: _code,
          nameController: _name,
          codeChip: _chip == null ? null : _spaced(_chip!),
          chipFromLink: _chipFromLink,
          onScan: widget.scanQr == null || _desktop ? null : _scan,
          scanWrong: scanWrong,
          scanAgainBody: widget.firstLaunch && _chip == null,
          onBeDisplay: widget.onBeDisplay,
          onMiracast: widget.onMiracast,
          codeEmpty: _codeEmpty,
          canConnect: c.phase != PresenterSessionPhase.connecting,
          onConnect: _connect,
        );
      case PresenterPage.what:
        return WhatPage(
          device: _deviceKind(context),
          room: c.roomName,
          onScreen: _startScreen,
          onFile: () => _go(PresenterPage.file),
          onLink: () => _go(PresenterPage.link),
          onDisconnect: _disconnect,
        );
      case PresenterPage.file:
        final busy =
            c.contentPhase == PresenterContentPhase.prepared ||
            c.contentPhase == PresenterContentPhase.sending;
        final converting = c.contentPhase == PresenterContentPhase.converting;
        return FilePage(
          onPick: _pickFile,
          onBack: () => _go(PresenterPage.what),
          // Where Office files are converted (the conversion component was found).
          office: widget.core.officeWorkerPath() != null,
          convertingName: converting ? (c.contentName ?? '') : null,
          progressName: busy ? (c.contentName ?? '') : null,
          progressPercent: busy ? c.progress : null,
          error: _fileError,
        );
      case PresenterPage.link:
        return LinkPage(
          controller: _url,
          onGo: _sendLink,
          onBack: () => _go(PresenterPage.what),
          error: _linkError,
        );
      case PresenterPage.codec:
        return CodecPage(
          controller: _codecCtl!,
          onBack: () => _go(PresenterPage.what),
          onScreen: _startScreen,
        );
      case PresenterPage.waiting:
        final total = c.timeoutSecs ?? _ios?.approval?.timeoutSecs ?? 60;
        final used = DateTime.now().difference(_waitStart).inSeconds;
        return WaitingPage(
          check: c.check ?? _ios?.approval?.check ?? 0,
          totalSeconds: total,
          secondsLeft: (total - used).clamp(0, total),
          room: c.roomName,
          onWithdraw: _withdraw,
        );
      case PresenterPage.presenting:
        return PresentingPage(
          kind: _kind,
          pdf: c.contentKind == UiContentKind.pdf,
          room: c.roomName,
          sas: c.connectionCode,
          page: c.page,
          pages: c.pages,
          onPrev: () => c.sendCommand(const UiContentCmd.prevPage()),
          onNext: () => c.sendCommand(const UiContentCmd.nextPage()),
          onStop: _stop,
        );
      case PresenterPage.bye:
        return ByePage(onAgain: () => _go(PresenterPage.firstRun));
      case PresenterPage.error:
        return ErrorPage(
          kind: _error,
          lockedWhile: _lockedWhile,
          cooldown: _error == ErrorKind.denied ? _cooldown : 0,
          otherWhy: _errorCode == null ? null : errorsText(t, _errorCode!),
          relayWhy: _relayWhy,
          canRescan: widget.scanQr != null && !_desktop,
          onAction: _errorAction,
        );
      case PresenterPage.relay:
        final relay = _scannedRelay ?? widget.relayToConfirm ?? '';
        return RelayConfirmPage(
          host: relay,
          // Cancel continues with the server the app would use anyway (the chip stays).
          onCancel: () {
            _scannedRelay = null;
            _go(PresenterPage.firstRun);
          },
          // Use remembers the server as confirmed on this device and takes it.
          onUse: () async {
            _scannedRelay = null;
            await widget.links?.confirmAndUse(relay);
            if (mounted) _go(PresenterPage.firstRun);
          },
        );
    }
  }

  String _spaced(String c) =>
      c.length == 6 ? '${c.substring(0, 3)} ${c.substring(3)}' : c;
}
