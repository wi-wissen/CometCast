// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter (sender) as a whole: first run, waiting, what to show, presenting, ended and the
// error pages (spec 5 and 6, native-apps.md 4, 5 and 20.1). The room is joined once, on Connect,
// by PresenterSessionController; Stop keeps the join, Disconnect leaves. Pages are chosen by the
// structured codes of the core (errors.dart), never by message text. The only other sender is the
// iOS broadcast extension, which makes its own connection (SenderController, see _startIos).

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../codec_controller.dart';
import '../../capture_feeder.dart';
import '../../core/android_capture.dart';
import '../../core/core_api.dart';
import '../../core/ios_broadcast.dart';
import '../../core/ios_share_settings.dart';
import '../../core/launch_intake.dart';
import '../../core/nearby_platform.dart';
import '../../core/link_target.dart';
import '../../core/share_intake.dart';
import '../../licences.dart';
import '../../tv_controller.dart';
import '../../join_state.dart';
import '../../nearby_controller.dart';
import '../../presenter_session_controller.dart';
import '../../receiver_controller.dart' show UrlOpener;
import '../../sender_controller.dart';
import '../../settings.dart';
import 'about.dart';
import 'address_line.dart';
import 'errors.dart';
import 'file_picker.dart';
import 'share.dart';
import 'tv.dart';
import '../display/display_logic.dart' show displayHost;
import 'kit.dart';
import 'nearby.dart';
import 'pages.dart';

enum PresenterPage {
  firstRun,
  settings,
  about,
  licenceText,
  share,
  pair,
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
  'capture_unavailable' => t.errorsCaptureUnavailable,
  'capture_os_old' => t.errorsCaptureOsOld,
  'capture_no_portal' => t.errorsCaptureNoPortal,
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
    this.nearbyPlatform,
    this.nearbyController,
    this.pickFileFor,
    this.openUrl,
    this.shareIntake,
    this.tvController,
    this.licenceLoader,
    this.leaveApp,
    this.iosShareSettings,
  });

  /// iOS: hands the name, the confirmed servers and the last server to the share extension
  /// (App Group, spec 15.3); null elsewhere.
  final IosShareSettings? iosShareSettings;

  /// The file dialog by kind (spec 15.6: Office types only while the switch is on; PDF only for
  /// "Pick my own PDF instead"; photos for a TV). Null: [pickFile] for every kind.
  final PickFileFor? pickFileFor;

  /// Opens a link in the system browser (About). Null: `url_launcher`.
  final UrlOpener? openUrl;

  /// The item of the Android share sheet (spec 15.3); null: none.
  final ShareIntake? shareIntake;

  /// Tests: a ready list of TVs without Comet Cast instead of one built from [core].
  final TvController? tvController;

  /// Reads and parses the licence list (tests: a fake without `compute`).
  final Future<LicenceSet> Function()? licenceLoader;

  /// Closes the share page and returns to the app that shared (Android; tests).
  final VoidCallback? leaveApp;

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

  /// The platform part of "screens on this network" (spec 14). Null: the section is never shown
  /// (the web, tests that do not need it).
  final NearbyPlatform? nearbyPlatform;

  /// Tests: a ready controller instead of one built from [nearbyPlatform].
  final NearbyController? nearbyController;

  @override
  State<PresenterRoot> createState() => PresenterRootState();
}

class PresenterRootState extends State<PresenterRoot>
    with WidgetsBindingObserver {
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

  // --- app settings and About (spec 15.8, 15.9) ---
  final _licenceSearch = TextEditingController();
  LicenceSet? _licences;
  Future<LicenceSet>? _licenceLoad;
  LicenceEntry? _licenceEntry;
  String? _licenceText;
  bool _aboutFromSettings = false;

  // --- TVs without Comet Cast (spec 15.5) ---
  TvController? _tvCtl;
  bool _ownsTv = false;
  TvRow? _tvRow;
  TvErrorWhy _tvWhy = TvErrorWhy.cast;
  final _pair = TextEditingController();
  bool _pairBusy = false;

  // --- the share sheet (spec 15.3) ---
  late final ShareIntake _intake = widget.shareIntake ?? const NoShareIntake();
  StreamSubscription<void>? _shareSub;
  SharedItem? _shareItem;

  /// The person asked to show the shared item: it is presented as soon as the join is done.
  bool _shareWanted = false;

  // --- screens on this network (spec 14) ---
  NearbyController? _nearby;
  bool _ownsNearby = false;

  /// The server a nearby screen announced, for the session started from a row (null: the
  /// settings' server). Never saved as "last used" (spec 14.8, point 11).
  String? _sessionServer;

  /// The row that is being connected (a failure `room_not_found` removes it, spec 14.4).
  String? _nearbyConnectId;

  /// The screen waiting for the relay confirmation page (spec 14.4).
  NearbyScreenInfo? _nearbyRelay;

  /// The row a join was started from; remembered (name and id) once the join succeeded.
  NearbyScreenInfo? _joiningScreen;

  late final PresenterSessionController _session = PresenterSessionController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
  );

  /// Android: the capture service and the encoder that feed the session.
  CaptureFeeder? _feeder;

  /// iOS only: the broadcast extension joins on its own, outside the session.
  SenderController? _ios;

  /// iOS: the room the extension was given, for the way back after the broadcast (20.5).
  ({String code, String server, String? name})? _iosRoom;

  /// iOS: the token the display holds for this person since the hand-over of the current
  /// broadcast; reused when the broadcast is not started at all (the sheet is cancelled).
  String? _iosOwnToken;

  /// iOS: the way back was taken for this broadcast already.
  bool _iosRejoined = false;

  bool get _desktop =>
      widget.desktop ?? !(Platform.isAndroid || Platform.isIOS);

  /// iOS: the broadcast runs (its status fills the Presenting page).
  bool get _iosPresenting => _ios != null && _ios!.isActive;

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
    widget.settings.addListener(_onSettings);
    WidgetsBinding.instance.addObserver(this);
    _nearby =
        widget.nearbyController ??
        (widget.nearbyPlatform == null
            ? null
            : NearbyController(
                core: widget.core,
                platform: widget.nearbyPlatform!,
              ));
    _ownsNearby = widget.nearbyController == null && _nearby != null;
    _nearby?.addListener(_onNearby);
    // The screens joined before: the core marks a row with a known name and a new id.
    unawaited(
      widget.settings.loadRememberedScreens().then((_) {
        if (mounted) {
          _nearby?.setRemembered(widget.settings.rememberedScreens);
        }
      }),
    );
    _tvCtl =
        widget.tvController ??
        (_desktop ? TvController(core: widget.core) : null);
    _ownsTv = widget.tvController == null && _tvCtl != null;
    _tvCtl?.addListener(_onNearby);
    _shareSub = _intake.arrivals.listen((_) => _takeShared());
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _syncNearby();
      _takeShared();
      unawaited(_syncShareSettings(readBack: true));
    });
  }

  void _onNearby() {
    if (mounted) setState(() {});
    _syncTv();
  }

  void _onSettings() {
    if (!mounted) return;
    unawaited(_syncShareSettings());
    _syncNearby();
    setState(() {});
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Spec 14.5: the search stops in the background and starts again on the page when the app
    // comes back. (`inactive` is also what a system dialog causes: it is not a background.)
    if (state == AppLifecycleState.paused) {
      _nearby?.stop();
    } else if (state == AppLifecycleState.resumed) {
      _syncNearby();
    }
  }

  /// The section is shown, and the search runs, on the start page without a code, unless the
  /// administrator's policy forbids announcements or locks the server (spec 14.1).
  bool get _nearbyWanted =>
      _nearby != null &&
      (_page == PresenterPage.firstRun ||
          (_page == PresenterPage.share && !_shareConnected)) &&
      _chip == null &&
      widget.settings.announcementsAllowed &&
      !widget.settings.locked;

  void _syncNearby() {
    final n = _nearby;
    if (n == null || !mounted) return;
    if (_nearbyWanted) {
      n.setEnabled(true);
      if (!n.active &&
          _nearbyConnectId == null &&
          _session.phase != PresenterSessionPhase.connecting) {
        unawaited(n.start());
      }
    } else if (_page == PresenterPage.relay && _nearbyRelay != null) {
      n.freeze();
    } else {
      n.stop();
      n.setEnabled(false);
    }
    _syncTv();
  }

  /// The TVs are searched exactly while the nearby search runs and only with the switch on
  /// (spec 15.5); a TV whose address is a Comet Cast row's address is hidden (N84).
  void _syncTv() {
    final tv = _tvCtl;
    if (tv == null || !mounted) return;
    final wanted =
        _desktop &&
        widget.settings.otherTvs &&
        _nearbyWanted &&
        (_nearby?.status ?? NearbyStatus.hidden) != NearbyStatus.hidden &&
        _nearbyConnectId == null &&
        _tvRow == null;
    if (wanted) {
      tv.setHiddenAddresses(
        hiddenTvAddresses([
          for (final r in _nearby?.rows ?? const <NearbyRow>[]) r.screen,
        ]),
      );
      tv.start();
    } else if (_tvRow == null) {
      tv.stop();
    }
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
    widget.settings.removeListener(_onSettings);
    WidgetsBinding.instance.removeObserver(this);
    _nearby?.removeListener(_onNearby);
    if (_ownsNearby) _nearby?.dispose();
    _tvCtl?.removeListener(_onNearby);
    if (_ownsTv) _tvCtl?.dispose();
    _shareSub?.cancel();
    _licenceSearch.dispose();
    _pair.dispose();
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
    if (p != PresenterPage.firstRun && p != PresenterPage.share) {
      _nearbyConnectId = null;
    }
    if (p == PresenterPage.firstRun || p == PresenterPage.share) {
      // Back at a start page: no TV is connecting any more.
      if (_session.phase == PresenterSessionPhase.idle ||
          _session.phase == PresenterSessionPhase.failed ||
          _session.phase == PresenterSessionPhase.ended) {
        _tvRow = null;
        _tvCtl?.endConnect();
      }
    }
    _syncNearby();
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
    unawaited(_syncShareSettings());
    _nearby?.stop();
    _nearbyConnectId = null;
    _sessionServer = null;
    setState(() {
      _roomCode = check.code;
      _codeEmpty = false;
    });
    _beginSession();
  }

  /// iOS: the settings the share extension reads (native-apps.md 21.3). [readBack]: first take
  /// the name the extension may have changed.
  bool _shareSettingsRead = false;

  Future<void> _syncShareSettings({bool readBack = false}) async {
    final s = widget.iosShareSettings;
    if (s == null) return;
    if (!readBack && !_shareSettingsRead) return; // the first read comes first
    try {
      if (readBack) {
        final d = await s.read();
        if (!mounted) return;
        if (d != null &&
            d.name.isNotEmpty &&
            d.name != widget.settings.displayName) {
          widget.settings.setDisplayName(d.name);
          _name.text = widget.settings.displayName;
        }
      }
      _shareSettingsRead = true;
      await s.write(
        ShareSettingsData(
          name: widget.settings.displayName,
          confirmedServers: widget.settings.confirmedServers.toList()..sort(),
          lastServer: await widget.settings.store?.loadServer(),
        ),
      );
    } catch (_) {
      // No App Group (a build without signing): the extension then has the defaults.
    }
  }

  /// Join once: the host is asked here, not again for what is shown next (native-apps.md 20.1).
  void _beginSession() {
    _shareWanted = _shareItem != null;
    _waited = false;
    _lastPhase = PresenterSessionPhase.idle;
    _lastStartFailure = null;
    _session.connect(
      code: _roomCode!,
      server: _sessionServer ?? _server,
      name: _myName,
    );
    setState(() {});
  }

  // --- screens on this network: tapping a row (spec 14.4) --------------------------------

  /// The server of a row is one the app may use at once (default relay, policy, DNS-SRV, or
  /// confirmed on this device). The announcement alone never makes a server trusted.
  ServerTrust get _trust =>
      widget.links?.trust ??
      ServerTrust(
        suggestions: widget.settings.serverSuggestions,
        confirmed: widget.settings.confirmedServers,
        builtIn: const {builtInRelay},
      );

  String? _viaFor(NearbyScreenInfo s) =>
      _trust.isKnownSource(s.server) ? null : displayHost(s.server);

  String _nearbyQuoted(AppLocalizations t, NearbyScreenInfo s) =>
      s.name.isEmpty ? t.commonNoName : t.commonQuoted(s.name);

  void _tapNearby(NearbyRow row) {
    final s = row.screen;
    if (s.code == null) return;
    if (!_trust.trustsForNearby(s.server)) {
      // Announced only: the relay confirmation page first; nothing is sent before.
      _nearbyRelay = s;
      _go(PresenterPage.relay);
      return;
    }
    _connectNearby(s);
  }

  void _connectNearby(NearbyScreenInfo s) {
    widget.settings.setDisplayName(_name.text);
    _nearbyConnectId = s.id;
    _joiningScreen = s;
    _nearby?.beginConnect(s.id);
    _sessionServer = s.server;
    _roomCode = s.code;
    _codeEmpty = false;
    _beginSession();
  }

  /// `room_not_found` after a tap: no error page; the row is removed, `nearby.gone` shows and
  /// the search starts again (spec 14.4).
  void _nearbyGone() {
    final id = _nearbyConnectId;
    _nearbyConnectId = null;
    _joiningScreen = null;
    _roomCode = null;
    _sessionServer = null;
    if (id != null) _nearby?.markGone(id);
    unawaited(_nearby?.restartKeepingLine());
  }

  /// The scan found something that is not a code of ours (shown as `app.scanWrong` by the
  /// first-run page, spec 5.3); cleared by the next scan.
  bool scanWrong = false;

  /// The server of a scanned code or link that waits for the relay confirmation page.
  String? _scannedRelay;

  Future<void> _scan() async {
    _nearby?.stop(); // spec 14.5: the page is left while the scanner is open
    final scanned = await widget.scanQr?.call();
    if (!mounted) return;
    if (scanned == null) {
      _syncNearby(); // cancelled: nothing changes, the search starts again
      return;
    }
    final outcome = widget.links?.fromUrl(scanned);
    if (outcome == null || outcome is LinkNothing) {
      setState(() => scanWrong = true);
      _syncNearby();
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
    _sessionServer = null;
    _syncNearby();
  }

  // --- errors ----------------------------------------------------------------------------

  /// The error page for [code] (`errorKindForCode`). [keepsSession]: the presenter is still in
  /// the room (a presentation that could not start): Try again goes back to "What to show".
  void _showError(String code, {bool keepsSession = false}) {
    _joiningScreen = null;
    final tv = _tvRow;
    if (tv != null &&
        const {
          'tv_refused',
          'tv_lost',
          'connection_lost',
          'peer_failed',
        }.contains(code)) {
      // A TV that does not take the connection, or that went away (spec 15.5).
      _tvWhy = code == 'tv_refused'
          ? (tv.kind == TvKind.cast ? TvErrorWhy.cast : TvErrorWhy.airplay)
          : TvErrorWhy.lost;
      _error = ErrorKind.tv;
      _errorCode = null;
      _lockedWhile = false;
      _relayWhy = false;
      _errorKeepsSession = false;
      _go(PresenterPage.error);
      return;
    }
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
        _sessionServer = null;
        _go(_home);
        if (a == ErrorAction.rescan) unawaited(_scan());
      case ErrorAction.retry || ErrorAction.ask:
        if (_errorKeepsSession && _session.isJoined) {
          _go(PresenterPage.what);
        } else if (_roomCode != null) {
          _go(_home);
          _beginSession();
        } else {
          _go(_home);
        }
      case ErrorAction.back:
        // Still connected: "What to show" (the join was kept); without a session the start page
        // (the share page when an item waits, spec 15.3 item 5).
        _go(_session.isJoined ? PresenterPage.what : _home);
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
    // A TV asks for the code it shows (spec 15.5): the pair page; nothing is shown before.
    if (c.pairingRequired &&
        _tvRow != null &&
        phase == PresenterSessionPhase.connecting &&
        _page != PresenterPage.pair) {
      _pair.clear();
      _go(PresenterPage.pair);
    }
    if (c.tvUnsupported && _tvRow != null) {
      _fileError = t.linkTvUnsupported;
      _linkError = t.linkTvUnsupported;
    }
    if (changed) {
      switch (phase) {
        case PresenterSessionPhase.waiting:
          _waited = true;
          _waitStart = DateTime.now();
          _go(PresenterPage.waiting);
        case PresenterSessionPhase.joined:
          final joined = _joiningScreen;
          if (joined != null) {
            // Trust on first use: this name now belongs to this id for this person.
            _joiningScreen = null;
            unawaited(
              widget.settings.rememberScreen(joined.name, joined.id).then((_) {
                _nearby?.setRemembered(widget.settings.rememberedScreens);
              }),
            );
          }
          if (_tvRow == null) {
            widget.settings.rememberServer().then((_) => _syncShareSettings());
          }
          _tvCtl?.endConnect();
          if (from == PresenterSessionPhase.presenting) _releaseCapture();
          if (_shareWanted && _shareItem != null) {
            // The content is chosen: no "What to show" before it (spec 15.3 item 5).
            _shareWanted = false;
            _go(PresenterPage.share);
            _presentShared();
          } else if (_page == PresenterPage.firstRun ||
              _page == PresenterPage.pair ||
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
          _showError(
            _tvRow != null ? 'tv_lost' : (c.endedCode ?? 'removed_by_host'),
          );
        case PresenterSessionPhase.failed:
          _releaseCapture();
          if (_nearbyConnectId != null && c.errorCode == 'room_not_found') {
            _nearbyGone();
          } else {
            _showError(c.errorCode ?? 'unknown');
          }
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
    // Desktops: the system picker opens on every tap (N55). A forced display (development and
    // tests, `COMETCAST_CAPTURE_DISPLAY`) captures that X11 display instead.
    final forced = widget.settings.captureDisplay;
    _session.presentScreen(
      forced == null && _desktop
          ? null
          : ScreenRequest(display: forced, fps: 15, bitrateKbps: 2000),
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

  /// iOS: the broadcast extension joins on its own connection (the app is suspended while another
  /// app is in front), so the approval is handed over (native-apps.md 20.5, N32): the display is
  /// asked to hold a single-use token for this person, the token goes to the extension with the
  /// name, the session is left, and the system sheet opens. A display that does not answer
  /// (old, web) leaves the extension to join the ordinary way: the waiting page with the
  /// extension's check number (step 6).
  Future<void> _startIos() async {
    final code = _roomCode;
    if (code == null) return;
    final server = _sessionServer ?? _server;
    final name = _myName;
    final token = newHandoverToken();
    // Test builds only: option (a) skips the hand-over (never in a release build).
    final held = iosOptionA ? false : await _session.requestHandover(token);
    if (!mounted) return;
    await _session.disconnect();
    if (!mounted) return;
    _iosRoom = (code: code, server: server, name: name);
    _iosOwnToken = held ? token : null;
    _iosRejoined = false;
    final ios = (_ios ??= SenderController(
      core: widget.core,
      udpBind: widget.settings.udpBind,
      iosBroadcast: widget.iosBroadcast,
    )..addListener(_onIos));
    widget.settings.rememberServer();
    _go(PresenterPage.firstRun);
    await ios.startBroadcast(
      code: code,
      server: server,
      name: name,
      handover: _iosOwnToken,
    );
  }

  /// iOS, after the broadcast: join again and land on "What to show" (step 5). With the token of
  /// the way back the display lets the person in without a dialog; without one this is
  /// Connect again (the waiting page where the room asks, step 6). Once per broadcast.
  void _rejoinIos({String? token}) {
    if (_iosRejoined) return;
    _iosRejoined = true;
    final r = _iosRoom;
    if (r == null) {
      _go(PresenterPage.firstRun);
      return;
    }
    _roomCode = r.code;
    _sessionServer = r.server;
    _waited = false;
    _lastPhase = PresenterSessionPhase.idle;
    _lastStartFailure = null;
    _session.connect(
      code: r.code,
      server: r.server,
      name: r.name,
      handover: token,
    );
    _go(PresenterPage.firstRun);
    setState(() {});
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
        } else if (c.broadcastEndedCleanly) {
          // Stopped by the person or the app: the extension asked the display for the way back.
          _rejoinIos(token: c.nextHandover);
        } else if (_page != PresenterPage.error) {
          _go(PresenterPage.firstRun);
        }
      case SenderPhase.idle:
        if (c.broadcastNotStarted) {
          // The sheet was cancelled or nobody started the broadcast: the display still holds
          // the token of the hand-over (60 s); with it the person is back without a dialog.
          _rejoinIos(token: _iosOwnToken);
        }
      case SenderPhase.starting:
        break;
    }
  }

  /// Office files are converted here: the worker is found and the switch is on (spec 15.6).
  bool get _officeOn =>
      widget.core.officeWorkerPath() != null && widget.settings.officeConvert;

  Future<String?> _pick(PickKind kind) {
    final t = AppLocalizations.of(context);
    // Every filter is named (N109): the GTK dialog shows the name.
    final label = switch (kind) {
      PickKind.imagePdfOffice => t.sendContentPickerTypes,
      PickKind.imagePdf => t.sendContentPickerTypesNoOffice,
      PickKind.pdf => t.sendContentPickerPdf,
      PickKind.photo => t.sendContentPickerPhotos,
    };
    final f = widget.pickFileFor;
    return f != null ? f(label, kind) : widget.pickFile(label);
  }

  void _pickFile() async {
    final kind = _tvRow != null
        ? PickKind.photo
        : _officeOn
        ? PickKind.imagePdfOffice
        : PickKind.imagePdf;
    final path = await _pick(kind);
    if (path == null || !mounted) return; // cancelling changes nothing
    _fileError = null;
    _kind = PresentingKind.file;
    _session.presentFile(path);
    setState(() {});
  }

  /// "Pick my own PDF instead" (spec 15.6): stops presenting and opens the PDF-only dialog as
  /// the direct result of the tap; cancelling leaves "What to show" (connected).
  Future<void> _ownPdf() async {
    _session.stopPresenting();
    final path = await _pick(PickKind.pdf);
    if (!mounted) return;
    if (path == null) {
      if (_session.isJoined) _go(PresenterPage.what);
      return;
    }
    _fileError = null;
    _kind = PresentingKind.file;
    _session.presentFile(path);
    setState(() {});
  }

  void _sendLink() {
    final t = AppLocalizations.of(context);
    _session.tvUnsupported = false;
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
      case PresenterPage.settings:
        _go(PresenterPage.firstRun);
      case PresenterPage.about:
        _go(
          _aboutFromSettings ? PresenterPage.settings : PresenterPage.firstRun,
        );
      case PresenterPage.licenceText:
        _go(PresenterPage.about);
      case PresenterPage.share:
        // Back on the share page is Cancel (spec 15.3).
        _closeShare();
      case PresenterPage.pair:
        await _withdraw();
      case PresenterPage.waiting:
        await _withdraw();
      case PresenterPage.what:
        await _disconnect();
      case PresenterPage.file || PresenterPage.link || PresenterPage.codec:
        _go(PresenterPage.what);
      case PresenterPage.presenting:
        break; // Stop is the only way out
      case PresenterPage.error || PresenterPage.bye:
        _go(_home);
      case PresenterPage.relay:
        _go(PresenterPage.firstRun);
    }
    return true;
  }

  /// Where "back to the start" leads: the share page while a shared item waits (spec 15.3).
  PresenterPage get _home =>
      _shareItem != null ? PresenterPage.share : PresenterPage.firstRun;

  Future<void> _withdraw() async {
    await _releaseCapture();
    await _ios?.stop();
    await _session.disconnect();
    _go(_home);
  }

  /// Leaves the room (the display shows `toast.left`).
  Future<void> _disconnect() async {
    await _releaseCapture();
    await _session.disconnect();
    _roomCode = null;
    _sessionServer = null;
    _chip = null;
    _clearShare();
    _go(PresenterPage.bye);
  }

  /// Ends the presentation only: the presenter stays joined (decision N5).
  Future<void> _stop() async {
    if (_ios != null && _ios!.isActive) {
      // The extension reports "ended" with the token of the way back; `_onIos` joins again.
      await _ios!.stopBroadcastAndWait();
      return;
    }
    _session.stopPresenting();
    await _releaseCapture();
    _clearShare();
    _go(PresenterPage.what);
  }

  // --- the share sheet (spec 15.3, native-apps.md 21.2) ----------------------------------

  Future<void> _takeShared() async {
    final item = await _intake.take();
    if (item == null || !mounted) return;
    final old = _shareItem;
    if (old?.path != null && old!.path != item.path) {
      unawaited(_intake.discard(old.path!));
    }
    _shareItem = item;
    _shareWanted = false;
    _fileError = null;
    // A relay confirmation or a pairing in progress is not interrupted; everything else makes
    // way (while presenting, the item replaces what is shown, spec 15.3 item 3).
    if (_page != PresenterPage.relay && _page != PresenterPage.pair) {
      _go(PresenterPage.share);
    }
  }

  /// The app holds a join (waiting, joined or presenting): "Show on {room}" is offered.
  bool get _shareConnected =>
      _session.phase == PresenterSessionPhase.waiting || _session.isJoined;

  void _clearShare() {
    final item = _shareItem;
    _shareItem = null;
    _shareWanted = false;
    if (item?.path != null) unawaited(_intake.discard(item!.path!));
  }

  /// Cancel: closes the page; on Android the person is back in the app that shared and Comet Cast
  /// moves to the back with its join (`ShareBridge.returnToCaller`).
  void _closeShare() {
    _clearShare();
    if (widget.leaveApp != null) {
      widget.leaveApp!();
    } else {
      // Back to the app that shared; Comet Cast and its join stay (N112).
      unawaited(_intake.returnToCaller());
    }
    if (mounted) {
      _go(_session.isJoined ? PresenterPage.what : PresenterPage.firstRun);
    }
  }

  /// Why the shared item cannot be shown, as text, or null (spec 15.3 item 2).
  String? _shareError(AppLocalizations t) {
    final item = _shareItem;
    if (item == null) return null;
    switch (item.kind) {
      case SharedKind.noLink:
      case SharedKind.tooLarge:
        return null; // `shareViewOf` has their texts
      case SharedKind.link:
        final check = widget.core.validateLink(item.url ?? '');
        return check is LinkCheck_Valid ? null : t.sendContentLinkInvalid;
      case SharedKind.file:
        final k = shareKindOfFile(item.name, item.mime);
        if (k == null) return t.sendRejectType;
        if (k.$1 == ShareKind.office) {
          if (widget.core.officeWorkerPath() == null) {
            return t.sendRejectOfficeUnavailable;
          }
          if (!widget.settings.officeConvert) return t.sendRejectOfficeOff;
        }
        final r = _session.rejection;
        return r == null ? null : sendRejectionText(t, r);
    }
  }

  /// Presents the shared item in the joined session, at once (one tap on "Show on …").
  void _presentShared() {
    final item = _shareItem;
    if (item == null) return;
    final t = AppLocalizations.of(context);
    if (_shareError(t) != null) return;
    if (_session.phase == PresenterSessionPhase.presenting) {
      _session.stopPresenting(); // Stop, then present: no question
    }
    switch (item.kind) {
      case SharedKind.file:
        _kind = PresentingKind.file;
        _session.presentFile(item.path!);
      case SharedKind.link:
        final check = widget.core.validateLink(item.url ?? '');
        if (check is LinkCheck_Valid) {
          _kind = PresentingKind.link;
          _session.presentLink(check.url);
        }
      case SharedKind.noLink || SharedKind.tooLarge:
        break;
    }
    setState(() {});
  }

  /// "Show on {room}": waiting for approval, the item follows the join.
  void _showShared() {
    if (_session.phase == PresenterSessionPhase.waiting) {
      _shareWanted = true;
      _go(PresenterPage.waiting);
      return;
    }
    _presentShared();
  }

  /// "Another screen": leaves the room, then the not-connected variant (item kept).
  Future<void> _otherScreen() async {
    await _releaseCapture();
    await _session.disconnect();
    _roomCode = null;
    _sessionServer = null;
    _chip = null;
    _go(PresenterPage.share);
  }

  // --- the screen's address (spec 15.7) --------------------------------------------------

  Future<AddressVerdict> _useAddress(String input) async {
    final parsed = widget.core.serverForAddress(input);
    if (parsed is! AddressParse_Ok) return AddressVerdict.notCometCast;
    final r = await widget.core.probeServer(parsed.server);
    if (r != ProbeResult.ok) return AddressVerdict.notCometCast;
    // Used for the next join; stored as last used after that join succeeded.
    widget.settings.serverAddress = parsed.server;
    return AddressVerdict.ok;
  }

  // --- app settings and About (spec 15.8, 15.9) ------------------------------------------

  bool get _hasSwitch => _desktop || widget.core.officeWorkerPath() != null;

  void _openAppSettings() {
    if (_hasSwitch) {
      _go(PresenterPage.settings);
    } else {
      _openAbout(fromSettings: false);
    }
  }

  void _openAbout({required bool fromSettings}) {
    _aboutFromSettings = fromSettings;
    _ensureLicences();
    _go(PresenterPage.about);
  }

  void _ensureLicences() {
    if (_licenceLoad != null) return;
    final loader =
        widget.licenceLoader ??
        () => loadLicences(
          desktop: _desktop,
          officeFonts: widget.core.officeWorkerPath() != null,
          extraAssets: const {uxplayEntryName: uxplayLicenceAsset},
        );
    final flutterName = AppLocalizations.of(context).aboutFlutter;
    final flutterSub = AppLocalizations.of(context).aboutFlutterSub;
    _licenceLoad = loader().then((v) {
      final set = withFlutterEntry(v, flutterName, flutterSub);
      if (mounted) setState(() => _licences = set);
      return set;
    });
  }

  Future<void> _openLicence(LicenceEntry e) async {
    _licenceEntry = e;
    final lazy = e.lazyText;
    _licenceText = lazy == null ? e.text : null;
    _go(PresenterPage.licenceText);
    if (lazy != null) {
      final text = await lazy();
      if (mounted && identical(_licenceEntry, e)) {
        setState(() => _licenceText = text);
      }
    }
  }

  Future<void> _open(Uri url) async {
    final f = widget.openUrl;
    if (f != null) {
      await f(url);
      return;
    }
    try {
      await launchUrl(url, mode: LaunchMode.externalApplication);
    } catch (_) {
      // No browser: nothing opens.
    }
  }

  // --- TVs without Comet Cast (spec 15.5) ------------------------------------------------

  void _tapTv(TvRow row) {
    _tvRow = row;
    _tvCtl?.beginConnect(row.id);
    _roomCode = null;
    _sessionServer = null;
    _codeEmpty = false;
    _nearby?.stop();
    _waited = false;
    _lastPhase = PresenterSessionPhase.idle;
    _lastStartFailure = null;
    _fileError = null;
    _linkError = null;
    _session.connectTv(id: row.id);
    setState(() {});
  }

  Future<void> _pairGo() async {
    if (_pairBusy) return;
    final pin = _pair.text.trim();
    setState(() => _pairBusy = true);
    final ok = await _session.pair(pin);
    if (!mounted) return;
    setState(() => _pairBusy = false);
    if (!ok) _pair.clear();
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
        return _firstRunPage(t);
      case PresenterPage.share:
        return _sharePage(t);
      case PresenterPage.settings:
        return AppSettingsPage(
          version: widget.core.appVersion(),
          showOffice: widget.core.officeWorkerPath() != null,
          showTvs: _desktop,
          officeOn: widget.settings.officeConvert,
          tvsOn: widget.settings.otherTvs,
          onOffice: (v) => widget.settings.officeConvert = v,
          onTvs: (v) => widget.settings.otherTvs = v,
          onAbout: () => _openAbout(fromSettings: true),
          onBack: () => _go(PresenterPage.firstRun),
        );
      case PresenterPage.about:
        return AboutPage(
          version: widget.core.appVersion(),
          licences: _licences,
          search: _licenceSearch,
          onQuery: (_) => setState(() {}),
          onOpenLicence: _openLicence,
          onOpenUrl: _open,
          onBack: () => _go(
            _aboutFromSettings
                ? PresenterPage.settings
                : PresenterPage.firstRun,
          ),
          languageCode: Localizations.localeOf(context).languageCode == 'de'
              ? 'de'
              : 'en',
        );
      case PresenterPage.licenceText:
        return LicenceTextPage(
          name: _licenceEntry?.name ?? '',
          text: _licenceText,
          onBack: () => _go(PresenterPage.about),
        );
      case PresenterPage.pair:
        return TvPairPage(
          name: quotedTvName(t, _tvRow?.name ?? ''),
          controller: _pair,
          wrong: c.pairWrong,
          busy: _pairBusy,
          onPair: _pairGo,
          onBack: _withdraw,
        );
      case PresenterPage.what:
        return WhatPage(
          device: _deviceKind(context),
          room: c.roomName,
          tv: _tvRow,
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
          office: _officeOn,
          bodyTv: _tvRow != null,
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
          hintTv: _tvRow != null,
        );
      case PresenterPage.codec:
        return CodecPage(
          controller: _codecCtl!,
          onBack: () => _go(PresenterPage.what),
          onScreen: _startScreen,
          onAction: _errorAction,
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
          // iOS: the extension is connected, not this session; its status has both lines.
          room: _iosPresenting ? _ios!.broadcastStatus?.roomName : c.roomName,
          sas: _iosPresenting ? _ios!.broadcastStatus?.sas : c.connectionCode,
          page: c.page,
          pages: c.pages,
          onPrev: () => c.sendCommand(const UiContentCmd.prevPage()),
          onNext: () => c.sendCommand(const UiContentCmd.nextPage()),
          onStop: _stop,
          onOwnPdf: c.converted && _tvRow == null ? _ownPdf : null,
          // Pause is offered while a screen is shown (not for files, links, TVs or the iOS
          // broadcast extension, which has no UI: docs/ios.md).
          onPause: c.canPause && !_iosPresenting ? c.setPaused : null,
          paused: c.pauseRequested,
          banner: c.pauseBanner,
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
          tvName: _tvRow == null ? null : quotedTvName(t, _tvRow!.name),
          tvWhy: _tvWhy,
          canRescan: widget.scanQr != null && !_desktop,
          onAction: _errorAction,
        );
      case PresenterPage.relay:
        final nearbyScreen = _nearbyRelay;
        if (nearbyScreen != null) {
          // A screen found nearby announced a relay this device does not know (spec 14.4).
          final host = displayHost(nearbyScreen.server);
          return RelayConfirmPage(
            host: host,
            nearbyName: _nearbyQuoted(t, nearbyScreen),
            // Cancel: back to the start page, nothing was sent.
            onCancel: () {
              _nearbyRelay = null;
              _go(PresenterPage.firstRun);
            },
            // Use: the host is remembered as confirmed on this device (not as last used), then
            // the join goes on as for a known server.
            onUse: () async {
              final canonical = canonicalServer(
                stripScheme(nearbyScreen.server),
              );
              if (canonical != null) {
                await widget.settings.confirmServer(canonical);
              }
              if (!mounted) return;
              _nearbyRelay = null;
              setState(() => _page = PresenterPage.firstRun);
              _connectNearby(nearbyScreen);
            },
          );
        }
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

  Widget? _nearbyView({required bool share}) {
    final n = _nearby;
    if (n == null || !_nearbyWanted || n.status == NearbyStatus.hidden) {
      return null;
    }
    final tvs = _tvCtl;
    return NearbySection(
      controller: n,
      onTap: _tapNearby,
      via: _viaFor,
      tvs: tvs != null && widget.settings.otherTvs && _desktop
          ? TvSection(controller: tvs, onTap: _tapTv)
          : null,
    );
  }

  Widget _addressLine() => AddressLine(
    dom: widget.core.joinAddress(widget.settings.serverAddress.trim()),
    policy: widget.settings.locked,
    locked: _nearbyConnectId != null,
    onUse: _useAddress,
  );

  Widget _firstRunPage(
    AppLocalizations t, {
    List<Widget>? intro,
    String? primary,
  }) {
    final c = _session;
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
      connecting: _nearbyConnectId != null || _tvCtl?.connectingId != null,
      onAnyTap: _nearby?.clearGoneLine,
      onConnect: _connect,
      nearby: _nearbyView(share: intro != null),
      addressLine: _addressLine(),
      intro: intro,
      primaryLabel: primary,
      onAppSettings: intro == null ? _openAppSettings : null,
      settingsIsAbout: !_hasSwitch,
    );
  }

  /// The share page (spec 15.3): connected, refused or not connected.
  Widget _sharePage(AppLocalizations t) {
    final item = _shareItem;
    if (item == null) return _firstRunPage(t);
    final c = _session;
    final lang = Localizations.localeOf(context).languageCode == 'de'
        ? 'de'
        : 'en';
    final view = shareViewOf(t, item, lang, error: _shareError(t));
    final converting = c.contentPhase == PresenterContentPhase.converting
        ? t.fileConverting(c.contentName ?? item.name ?? '')
        : null;
    if (view.error != null) {
      return SharePage(view: view, onCancel: _closeShare);
    }
    if (_shareConnected) {
      final room = c.roomName;
      return SharePage(
        view: view,
        onCancel: _closeShare,
        room: room == null || room.isEmpty ? null : t.commonQuoted(room),
        onShow: _showShared,
        onOtherScreen: _otherScreen,
        converting: converting,
      );
    }
    return _firstRunPage(
      t,
      intro: shareIntro(view, converting: converting),
      primary: t.shareShow,
    );
  }

  String _spaced(String c) =>
      c.length == 6 ? '${c.substring(0, 3)} ${c.substring(3)}' : c;
}
