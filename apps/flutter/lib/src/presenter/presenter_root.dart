// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter (sender) as a whole: first run, what to show, waiting, presenting, ended and the
// error pages (spec 5 and 6, native-apps.md 4 and 5). The sending itself stays in
// SenderController and ContentSenderController.
//
// UNCERTAIN / design gap: the core joins a room only when it starts sending, so "What to show"
// comes before the join here (docs/design/implementation-questions-flutter.md, P1).

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../codec_controller.dart';
import '../../codec_notice.dart';
import '../../content_sender_controller.dart';
import '../../core/android_capture.dart';
import '../../core/core_api.dart';
import '../../core/ios_broadcast.dart';
import '../../join_state.dart';
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
    this.initialCode,
    this.codeFromLink = true,
    this.relayToConfirm,
    this.desktop,
    this.codec,
  });

  final CoreApi core;
  final AppSettings settings;
  final PickFile pickFile;
  final AndroidCapture? androidCapture;
  final IosBroadcast? iosBroadcast;
  final ScanQr? scanQr;

  /// Tablets and computers: starts the display setup (`app.beDisplay`).
  final VoidCallback? onBeDisplay;

  /// A code that came from a link, the install referrer or a scan.
  final String? initialCode;
  final bool codeFromLink;

  /// A server address from a link that the app did not know: confirmed first (spec 7.2).
  final String? relayToConfirm;

  /// Computers (subtitle `what.screenLaptop`); null = decide by platform.
  final bool? desktop;

  /// The H.264 codec state for desktop screen sharing (a fake in tests).
  final CodecController? codec;

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
  bool _lockedWhile = false;
  bool _relayWhy = false;
  PresentingKind _kind = PresentingKind.screen;
  bool _pdf = false;
  DateTime? _busyAt;
  bool _wasWaiting = false;
  Timer? _tick;
  DateTime _waitStart = DateTime.now();
  DateTime _refusedAt = DateTime.now();
  void Function()? _lastStart;
  CodecController? _codecCtl;

  late final SenderController _screen = SenderController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
    capture: widget.androidCapture,
    iosBroadcast: widget.iosBroadcast,
  );
  late final ContentSenderController _content = ContentSenderController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
  );
  bool _useContent = false;

  bool get _desktop =>
      widget.desktop ?? !(Platform.isAndroid || Platform.isIOS);

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
    _screen.addListener(_sync);
    _content.addListener(_sync);
    widget.settings.discover(widget.core);
  }

  @override
  void dispose() {
    _tick?.cancel();
    _screen.removeListener(_sync);
    _content.removeListener(_sync);
    _screen.dispose();
    _content.dispose();
    _codecCtl?.dispose();
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
      _fail(ErrorKind.code);
      return;
    }
    widget.settings.setDisplayName(_name.text);
    setState(() {
      _roomCode = check.code;
      _codeEmpty = false;
    });
    _go(PresenterPage.what);
  }

  Future<void> _scan() async {
    final scanned = await widget.scanQr?.call();
    if (scanned == null || !mounted) return;
    final tail = scanned.split('/').last;
    setState(() {
      _chip = tail.toUpperCase();
      _chipFromLink = false;
    });
  }

  void _fail(ErrorKind k, {String? code, bool lockedWhile = false}) {
    _error = k;
    _errorCode = code;
    _lockedWhile = lockedWhile;
    _go(PresenterPage.error);
  }

  // --- starting what to show -------------------------------------------------------------

  Future<void> _stopAll() async {
    await _screen.stop();
    await _content.stop();
  }

  void _startScreen() {
    _useContent = false;
    _kind = PresentingKind.screen;
    final code = _roomCode!;
    final t = AppLocalizations.of(context);
    void go() {
      widget.settings.rememberServer();
      if (widget.iosBroadcast != null) {
        _screen.startBroadcast(code: code, server: _server);
      } else if (widget.androidCapture != null) {
        _screen.startCapture(
          code: code,
          server: _server,
          notification: CaptureNotificationTexts(
            title: t.captureNotificationTitle,
            text: t.captureNotificationText,
            stopLabel: t.captureNotificationStop,
          ),
          name: _myName,
        );
      } else {
        _screen.start(
          code: code,
          server: _server,
          name: _myName,
          screen: ScreenRequest(
            display: widget.settings.captureDisplay,
            fps: 15,
            bitrateKbps: 2000,
          ),
        );
      }
    }

    _lastStart = _startScreen;
    if (_desktop) {
      // Desktop capture is encoded with OpenH264, which release builds download first. No
      // designed place for the consent yet: the existing card, on its own page.
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
    go();
    _wasWaiting = false;
  }

  void _pickFile() async {
    final t = AppLocalizations.of(context);
    final path = await widget.pickFile(t.sendContentPickerTypes);
    if (path == null || !mounted) return; // cancelling changes nothing
    _useContent = true;
    _fileError = null;
    _lastStart = _pickFile;
    widget.settings.rememberServer();
    _content.start(
      code: _roomCode!,
      server: _server,
      source: ContentSource.file(path: path),
      name: _myName,
    );
    _wasWaiting = false;
  }

  void _sendLink() {
    final t = AppLocalizations.of(context);
    final check = widget.core.validateLink(_url.text.trim());
    if (check is! LinkCheck_Valid) {
      setState(() => _linkError = t.sendContentLinkInvalid);
      return;
    }
    _useContent = true;
    _linkError = null;
    _kind = PresentingKind.link;
    _lastStart = _sendLink;
    widget.settings.rememberServer();
    _content.start(
      code: _roomCode!,
      server: _server,
      source: ContentSource.link(url: check.url),
      name: _myName,
    );
    _wasWaiting = false;
  }

  // --- reacting to the controllers -------------------------------------------------------

  void _sync() {
    if (!mounted) return;
    final t = AppLocalizations.of(context);
    if (_useContent) {
      final c = _content;
      if (c.kind == UiContentKind.pdf || c.kind == UiContentKind.image) {
        _kind = PresentingKind.file;
      }
      _pdf = c.kind == UiContentKind.pdf;
      if (c.rejection != null) {
        _fileError = sendRejectionText(t, c.rejection!);
        if (_page == PresenterPage.link) {
          _linkError = _fileError;
        }
        setState(() {});
        return;
      }
      switch (c.phase) {
        case ContentSendPhase.waitingApproval:
          _enterWaiting(c.approval!);
        case ContentSendPhase.sending || ContentSendPhase.ready:
          if (_page != PresenterPage.presenting) {
            _go(PresenterPage.presenting);
          }
        case ContentSendPhase.stopped:
          _go(PresenterPage.what);
        case ContentSendPhase.ended:
          _ended(
            refusal: c.refusal,
            receiverLeft: c.receiverLeft,
            error: c.error,
          );
        default:
          break;
      }
    } else {
      final c = _screen;
      switch (c.phase) {
        case SenderPhase.waitingApproval:
          _enterWaiting(c.approval!);
        case SenderPhase.running:
          if (_page != PresenterPage.presenting) {
            _go(PresenterPage.presenting);
          }
        case SenderPhase.ended || SenderPhase.finished:
          if (c.captureDenied && c.error == null && c.refusal == null) {
            // Cancelling the system dialog goes back without an error (spec 5.5).
            _go(PresenterPage.what);
          } else {
            _ended(
              refusal: c.refusal,
              receiverLeft: c.receiverLeft || c.phase == SenderPhase.finished,
              error: c.error,
            );
          }
        case SenderPhase.idle:
          if ((c.captureDenied || c.broadcastNotStarted) &&
              _page != PresenterPage.what) {
            _go(PresenterPage.what);
          }
        default:
          break;
      }
    }
    setState(() {});
  }

  void _enterWaiting(ApprovalWait w) {
    if (_page != PresenterPage.waiting) {
      _waitStart = DateTime.now();
      _wasWaiting = true;
      _go(PresenterPage.waiting);
    }
  }

  void _ended({
    JoinRefusal? refusal,
    bool receiverLeft = false,
    String? error,
  }) {
    if (_page == PresenterPage.error || _page == PresenterPage.bye) return;
    if (refusal != null) {
      final (k, w) = refusalPage(refusal);
      _refusedAt = joinClock();
      _fail(k, lockedWhile: w);
      return;
    }
    if (error != null) {
      final code = serverCodeOf(error);
      if (code == 'rate_limited' || code == 'banned' || code == 'server_full') {
        _busyAt = DateTime.now();
      }
      final recent =
          _busyAt != null &&
          DateTime.now().difference(_busyAt!) < const Duration(seconds: 60);
      final r = errorKindFor(
        error,
        wasWaiting: _wasWaiting,
        recentBusy: recent,
      );
      if (r == null) {
        _go(PresenterPage.what);
        return;
      }
      _relayWhy = false;
      _fail(r.$1, code: r.$2);
      return;
    }
    if (receiverLeft) {
      _fail(ErrorKind.ended);
      return;
    }
    _go(PresenterPage.what);
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
        _chip = null;
        _code.clear();
        _roomCode = null;
        _go(PresenterPage.firstRun);
        if (a == ErrorAction.rescan) unawaited(_scan());
      case ErrorAction.retry || ErrorAction.ask:
        final again = _lastStart;
        if (again != null && _roomCode != null) {
          _page = PresenterPage.what;
          again();
        } else {
          _go(PresenterPage.firstRun);
        }
      case ErrorAction.showFile:
        _go(PresenterPage.file);
      case ErrorAction.showLink:
        _go(PresenterPage.link);
      case ErrorAction.reload:
        _go(PresenterPage.firstRun);
    }
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
    await _stopAll();
    _go(PresenterPage.firstRun);
  }

  Future<void> _disconnect() async {
    await _stopAll();
    _roomCode = null;
    _chip = null;
    _go(PresenterPage.bye);
  }

  Future<void> _stop() async {
    await _stopAll();
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
    final left = () {
      final w = _screen.approval ?? _content.approval;
      final total = w?.timeoutSecs ?? 60;
      final used = DateTime.now().difference(_waitStart).inSeconds;
      return (total, (total - used).clamp(0, total));
    }();
    switch (_page) {
      case PresenterPage.firstRun:
        return FirstRunPage(
          codeController: _code,
          nameController: _name,
          codeChip: _chip == null ? null : _spaced(_chip!),
          chipFromLink: _chipFromLink,
          onScan: widget.scanQr == null || _desktop ? null : _scan,
          onBeDisplay: widget.onBeDisplay,
          codeEmpty: _codeEmpty,
          onConnect: _connect,
        );
      case PresenterPage.what:
        return WhatPage(
          laptop: _desktop,
          onScreen: _startScreen,
          onFile: () => _go(PresenterPage.file),
          onLink: () => _go(PresenterPage.link),
          onDisconnect: _disconnect,
        );
      case PresenterPage.file:
        final c = _content;
        final busy =
            _useContent &&
            (c.phase == ContentSendPhase.preparing ||
                c.phase == ContentSendPhase.converting ||
                c.phase == ContentSendPhase.connecting ||
                c.phase == ContentSendPhase.sending);
        return FilePage(
          onPick: _pickFile,
          onBack: () => _go(PresenterPage.what),
          progressName: busy ? (c.name ?? '') : null,
          progressPercent: busy ? c.percent : null,
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
        return PPage(
          actions: [
            PSecondaryButton(
              label: t.commonBack,
              onPressed: () => _go(PresenterPage.what),
            ),
          ],
          children: [
            Theme(
              data: ThemeData(useMaterial3: true),
              child: CodecNotice(controller: _codecCtl!),
            ),
          ],
        );
      case PresenterPage.waiting:
        final w = _screen.approval ?? _content.approval;
        return WaitingPage(
          check: w?.check ?? 0,
          totalSeconds: left.$1,
          secondsLeft: left.$2,
          onWithdraw: _withdraw,
        );
      case PresenterPage.presenting:
        return PresentingPage(
          kind: _kind,
          pdf: _pdf && _useContent,
          onPrev: () => _content.prevPage(),
          onNext: () => _content.nextPage(),
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
        return RelayConfirmPage(
          host: widget.relayToConfirm ?? '',
          onCancel: () => _go(PresenterPage.firstRun),
          onUse: () => _go(PresenterPage.firstRun),
        );
    }
  }

  String _spaced(String c) =>
      c.length == 6 ? '${c.substring(0, 3)} ${c.substring(3)}' : c;
}
