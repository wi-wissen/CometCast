// SPDX-License-Identifier: GPL-3.0-or-later
// The display (receiver) as a whole: setup wizard, idle screen, Settings, join requests, link
// card and presenting, with the rules of docs/design/first-run-and-display.md sections 4, 7, 8
// and docs/design/native-apps.md (keys, Back, dimming, wake lock, lifecycle). The state of the
// room itself stays in ReceiverController.

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../codec_controller.dart';
import '../../core/android_platform.dart';
import '../../core/core_api.dart';
import '../../core/nearby_platform.dart' show setAnnounceLock;
import '../../discovery.dart';
import '../../receiver_controller.dart';
import '../../settings.dart';
import '../design/tokens.dart';
import 'codec_dialog.dart';
import 'display_logic.dart';
import 'kit.dart';
import 'picture_stage.dart';
import 'screens.dart';
import 'setup_wizard.dart';
import 'wake_lock.dart';
import '../../window_control.dart';

DisplayJoinMode fromCoreMode(UiJoinMode m) => switch (m) {
  UiJoinMode.open => DisplayJoinMode.open,
  UiJoinMode.confirm => DisplayJoinMode.confirm,
  UiJoinMode.locked => DisplayJoinMode.locked,
};

UiJoinMode toCoreMode(DisplayJoinMode m) => switch (m) {
  DisplayJoinMode.open => UiJoinMode.open,
  DisplayJoinMode.confirm => UiJoinMode.confirm,
  DisplayJoinMode.locked => UiJoinMode.locked,
};

UiJoinMode _defaultCoreMode(DefaultMode m) =>
    m == DefaultMode.open ? UiJoinMode.open : UiJoinMode.confirm;

/// Can the system open web links? (Android TV without a browser cannot; spec 4.5, native-apps 3.)
typedef CanOpenLinks = Future<bool> Function();

Future<bool> systemCanOpenLinks() async {
  try {
    return await canLaunchUrl(Uri.parse('https://example.org'));
  } catch (_) {
    return false;
  }
}

class DisplayRoot extends StatefulWidget {
  const DisplayRoot({
    super.key,
    required this.core,
    required this.settings,
    required this.openUrl,
    this.store = const PrefsDisplayStore(),
    this.wakeLock,
    this.canOpenLinks,
    this.dimAfter = idleDimAfter,
    this.guard = requestGuard,
    this.leaveApp,
    this.endOnBackground,
    this.setFullscreenOnSetupDone = false,
    this.onFullscreen,
    this.deviceName,
    this.announceLock = setAnnounceLock,
  });

  final CoreApi core;
  final AppSettings settings;
  final UrlOpener openUrl;
  final DisplayStore store;

  /// The wake lock (a fake in tests). Defaults to the platform's.
  final WakeLockControl? wakeLock;
  final CanOpenLinks? canOpenLinks;
  final Duration dimAfter;
  final Duration guard;

  /// Leaves the app (Android TV: second Back within 2 s on idle). Defaults to the system's.
  final VoidCallback? leaveApp;

  /// The display session ends when the app goes to the background (Android, iOS; native-apps.md
  /// 15). Null = decide by platform.
  final bool? endOnBackground;

  /// Desktop: the window goes full screen after setup and on every start in this role
  /// (native-apps.md 7).
  final bool setFullscreenOnSetupDone;
  final Future<bool> Function(bool on)? onFullscreen;

  /// The name this device announces in the local network.
  final String Function()? deviceName;

  /// Android: hold a multicast lock while the core announces (decision N50); a no-op elsewhere.
  final Future<void> Function(bool on) announceLock;

  @override
  State<DisplayRoot> createState() => DisplayRootState();
}

class DisplayRootState extends State<DisplayRoot> with WidgetsBindingObserver {
  late final ReceiverController _c;
  CodecController? _codec;
  final ValueNotifier<bool> _ring = ValueNotifier(true);
  final FocusNode _rootNode = FocusNode(
    debugLabel: 'display:root',
    skipTraversal: true,
  );
  final FocusNode _settingsNode = RowNode(
    row: 0,
    col: 0,
    debugLabel: 'settings',
  );
  final FocusNode _endNode = RowNode(row: 0, col: 0, debugLabel: 'end');

  bool _ready = false;
  String? _instanceId;
  bool? _announcing;
  DisplaySetup? _setup;
  Appearance _appearance = Appearance.dark;
  SetupController? _wizard;
  bool _settingsOpen = false;
  bool _controlsShown = false;
  bool _dimmed = false;
  bool _canOpen = true;
  AppLifecycleState _life = AppLifecycleState.resumed;
  String? _toast;
  Timer? _toastTimer;
  bool _silentEnd = false;
  bool _justAllowed = false;
  Duration _lastBack = const Duration(days: -1);

  late final IdleDim _dim = IdleDim(
    delay: widget.dimAfter,
    onChange: (d) {
      if (mounted) setState(() => _dimmed = d);
    },
  );
  late final ControlsReveal _reveal = ControlsReveal(
    onChange: (v) {
      if (!mounted) return;
      setState(() => _controlsShown = v);
      if (!v) _refocusRoot();
    },
  );

  // connection tracking (spec 4.2)
  /// Virtual time, advanced by the 250 ms tick (so that the schedule follows the timers).
  Duration _t = Duration.zero;
  Duration _lostAt = Duration.zero;
  int _attempts = 0;
  bool _failed = false;
  OfflineCause _cause = OfflineCause.network;
  Timer? _retryTimer;
  Timer? _tick;
  bool _hadCode = false;

  bool get _tv => DeviceProfile.peek(context).tv;

  WakeLockControl get _wake =>
      widget.wakeLock ??
      PlatformWakeLock(android: DeviceProfile.peek(context).platform);

  /// An H.264 presentation waits for the decoder: it is Presenting (black stage, the controls
  /// under the consent dialog, spec 4.7).
  bool get _codecWaiting =>
      _codec != null &&
      _codec!.needed &&
      !_codec!.ready &&
      _c.senderJoined &&
      _c.codec == 'h264';

  /// "Not now" was chosen for this presentation (spec 4.7: at most once per presentation).
  bool _codecLater = false;

  bool get _presenting {
    final shown = _c.shown;
    return _c.hasLiveVideo ||
        shown is ReceivedImage ||
        shown is ReceivedDocument ||
        _codecWaiting ||
        // The presenter left for the iOS hand-over: black stage until it is back (20.5).
        _c.handoverHold;
  }

  bool get _codecDialog => _codecWaiting && !_codecLater;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _c = ReceiverController(
      core: widget.core,
      udpBind: widget.settings.udpBind,
      hardwareDecoder: DeviceProfile.peek(context).videoDecoder,
      onNotice: _onNotice,
      keepRoomOnPeerLeft: true,
    )..addListener(_onController);
    final profile = DeviceProfile.peek(context);
    _codec = CodecController(
      core: widget.core,
      settings: widget.settings,
      needed:
          profile.videoDecoder == null ||
          ReceiverController.hardwareBrokenReason != null,
    );
    _codec!.addListener(_onController);
    widget.settings.discover(widget.core);
    widget.settings.addListener(_onSettings);
    _tick = Timer.periodic(const Duration(milliseconds: 250), (_) {
      _t += const Duration(milliseconds: 250);
      if (mounted && _view != ConnectionView.online) setState(() {});
    });
    unawaited(_load());
    unawaited(
      (widget.canOpenLinks ?? systemCanOpenLinks)().then((v) {
        if (mounted) setState(() => _canOpen = v);
      }),
    );
  }

  Future<void> _load() async {
    final setup = await widget.store.loadSetup();
    final look = await widget.store.loadAppearance();
    final id = await widget.store.instanceId();
    // Not awaited: with a consent on file `init` downloads the codec (or follows a running
    // download), which can take minutes, and the display must not stay blank meanwhile.
    unawaited(_codec?.init());
    if (!mounted) return;
    _instanceId = id;
    setState(() {
      _setup = setup;
      _appearance = look;
      _ready = true;
    });
    _refocusRoot();
    if (setup != null) {
      _startRoom(setup);
    }
    _syncDim();
    _syncAnnounce();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _toastTimer?.cancel();
    _retryTimer?.cancel();
    _tick?.cancel();
    _dim.dispose();
    _reveal.dispose();
    widget.settings.removeListener(_onSettings);
    widget.settings.cancelDiscovery();
    _syncAnnounce(force: false);
    _c.removeListener(_onController);
    _c.dispose();
    _codec?.removeListener(_onController);
    _codec?.dispose();
    _wizard?.dispose();
    _ring.dispose();
    _rootNode.dispose();
    _settingsNode.dispose();
    _endNode.dispose();
    unawaited(_wake.set(false));
    super.dispose();
  }

  // --- room ------------------------------------------------------------------------------

  String get _server =>
      (_setup?.server ?? widget.settings.serverAddress).trim();

  void _startRoom(DisplaySetup setup) {
    _c.restartMode = _defaultCoreMode(setup.mode);
    _lostAt = _t;
    _attempts = 0;
    _failed = false;
    _hadCode = false;
    _c.start(
      _server,
      joinMode: _defaultCoreMode(setup.mode),
      roomName: setup.room,
    );
    if (widget.setFullscreenOnSetupDone) {
      unawaited((widget.onFullscreen ?? setWindowFullscreen)(true));
    }
  }

  ConnectionView get _view => connectionView(
    hasCode: _c.roomCode != null,
    errored: _c.phase == ReceiverPhase.ended && _c.error != null,
    failed: _failed,
    withoutCode: _t - _lostAt,
  );

  void _onController() {
    if (!mounted) return;
    final c = _c;
    // Connection tracking.
    if (c.roomCode != null && c.phase == ReceiverPhase.running) {
      _hadCode = true;
      _failed = false;
      _attempts = 0;
      _lostAt = _t;
      _retryTimer?.cancel();
    } else if (c.phase == ReceiverPhase.ended && c.error != null) {
      if (_hadCode || _attempts == 0) {
        // The first failure since the display had a code.
      }
      _failed = true;
      _cause = offlineCauseOf(c.error);
      _scheduleRetry();
    } else if (c.roomCode == null && _hadCode) {
      // The code went away without an error (a new room is being made): count from now.
      _hadCode = false;
      _lostAt = _t;
    }
    if (!c.senderJoined) {
      _codecLater = false; // the next presentation asks again
    }
    // Activity: a request, a joiner or a mode change counts as activity (spec 8.6).
    _dim.activity();
    // A join request closes Settings: one dialog at a time (spec 4.3).
    if (c.requests.isNotEmpty && _settingsOpen) _settingsOpen = false;
    if (!_presenting && _controlsShown) {
      _reveal.hideNow();
    }
    _syncDim();
    unawaited(_wake.set(_presenting && _life == AppLifecycleState.resumed));
    _syncAnnounce();
    setState(() {});
    _refocusRoot();
  }

  /// The display announces itself in the local network only while it shows Idle or Presenting
  /// (a room with a code) and is not in the background; never in setup or offline (spec 14.8,
  /// point 1). What is announced comes from the running room in the core, so the code is the one
  /// on the screen; the core also checks the administrator's policy.
  void _syncAnnounce({bool? force}) {
    final id = _instanceId;
    if (id == null) return;
    final active =
        force ??
        (_ready &&
            _setup != null &&
            _view == ConnectionView.online &&
            _life != AppLifecycleState.paused);
    if (active == _announcing) return;
    _announcing = active;
    // Android: the multicast lock goes first, so the core's sockets work from the first packet.
    if (active) unawaited(widget.announceLock(true).catchError((Object _) {}));
    unawaited(
      widget.core
          .setDisplayAnnouncing(active: active, instanceId: id)
          .then<void>((_) {}, onError: (Object _) {})
          .then<void>((_) {
            if (!active) {
              return widget.announceLock(false).catchError((Object _) {});
            }
          }),
    );
  }

  void _scheduleRetry() {
    _retryTimer?.cancel();
    final delay = retryDelay(
      attempt: _attempts,
      sinceLoss: _t - _lostAt,
      cause: _cause,
    );
    if (delay == null) return;
    _retryTimer = Timer(delay, () {
      if (!mounted || _setup == null) return;
      _attempts++;
      _c.start(
        _server,
        joinMode: _defaultCoreMode(_setup!.mode),
        roomName: _setup!.room,
      );
    });
  }

  void _onSettings() {
    if (mounted) setState(() {});
    final w = _wizard;
    if (w != null) w.updateOptions(_addressOptions());
  }

  /// The IPv4 address of the in-house box (spec 4.2, N40): the server of this display came from
  /// a box announcement (`sourceBox`) and its address is a host name; the announcement's IPv4
  /// address then is a second way in (`idle.altAddress`) and the QR code uses it. Null
  /// otherwise (the IP is never shown next to an IP, nor for any other source).
  String? get _boxIp {
    final server = _server;
    for (final x in widget.settings.serverSuggestions) {
      if (x.address.trim() != server || x.kind != ServerSourceKind.announced) {
        continue;
      }
      final ip = x.ip;
      if (ip == null || ip.isEmpty) return null;
      return isIpHost(displayHost(server)) ? null : ip;
    }
    return null;
  }

  List<AddressOption> _addressOptions() {
    final s = widget.settings;
    AddressSource source(ServerSourceKind k) => switch (k) {
      ServerSourceKind.dns => AddressSource.own,
      ServerSourceKind.policy => AddressSource.policy,
      ServerSourceKind.announced => AddressSource.box,
      _ => AddressSource.defaultServer,
    };
    final out = <AddressOption>[];
    final current = s.serverAddress.trim();
    final byAddr = {for (final x in s.serverSuggestions) x.address: x.kind};
    if (current.isNotEmpty) {
      out.add(
        AddressOption(
          current,
          source(byAddr[current] ?? ServerSourceKind.defaultList),
        ),
      );
    }
    for (final x in s.serverSuggestions) {
      if (out.every((o) => o.address != x.address)) {
        out.add(AddressOption(x.address, source(x.kind)));
      }
    }
    return out;
  }

  void _syncDim() {
    _dim.setEnabled(_ready && _setup != null && !_presenting);
  }

  // --- notices and toasts ----------------------------------------------------------------

  String _quoted(AppLocalizations t, String name) {
    final full = stripName(name);
    return full.isEmpty ? t.commonNoName : t.commonQuoted(shortName(full));
  }

  void _onNotice(ReceiverNotice n) {
    if (!mounted) return;
    final t = AppLocalizations.of(context);
    final text = switch (n) {
      NoticeJoined(:final name) =>
        _justAllowed ? null : t.toastJoined(_quoted(t, name)),
      NoticeExpired(:final name) => t.toastExpired(_quoted(t, name)),
      NoticeWithdrawn(:final name) => t.toastWithdrawn(_quoted(t, name)),
      NoticeLeft(:final name) => t.toastLeft(_quoted(t, name)),
      NoticeSessionEnded() => _silentEnd ? null : t.toastSessionEnded,
      NoticeOpenDisabled() => t.toastOpenDisabled,
    };
    if (n is NoticeJoined) _justAllowed = false;
    if (n is NoticeSessionEnded) _silentEnd = false;
    if (text != null) _showToast(text);
  }

  void _showToast(String text) {
    _toastTimer?.cancel();
    setState(() => _toast = text);
    _toastTimer = Timer(toastFor, () {
      if (mounted) setState(() => _toast = null);
    });
  }

  /// A content error is a toast with the existing wording (decision 13.41).
  String? _contentToast(AppLocalizations t, UiContentError e) => switch (e) {
    UiContentError.badMessage => t.receiveTransferErrorBadMessage,
    UiContentError.unexpectedChunk => t.receiveTransferErrorUnexpectedChunk,
    UiContentError.tooLarge => t.receiveTransferErrorTooLarge,
    UiContentError.sizeMismatch => t.receiveTransferErrorSizeMismatch,
    UiContentError.noTransfer => t.receiveTransferErrorNoTransfer,
    UiContentError.badChunk => t.receiveTransferErrorBadChunk,
    UiContentError.interrupted ||
    UiContentError.transferTimeout => t.receiveTransferErrorStalled,
    UiContentError.documentRejected => t.receiveTransferErrorDocumentRejected,
    UiContentError.renderFailed ||
    UiContentError.tooManyErrors => t.receiveTransferErrorDocumentFailed,
    UiContentError.workerUnavailable =>
      t.receiveTransferErrorDocumentUnsupported,
  };

  // --- input -----------------------------------------------------------------------------

  /// First look at every key: shows the ring; true = the key only restored the brightness.
  bool _keyGate(KeyEvent e) {
    if (e is! KeyDownEvent) return false;
    if (!_tv || !_ring.value) _ring.value = true;
    return _dim.activity();
  }

  void _refocusRoot() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (FocusManager.instance.primaryFocus == null ||
          FocusManager.instance.primaryFocus ==
              FocusManager.instance.rootScope) {
        _rootNode.requestFocus();
      }
    });
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent e) {
    if (e is! KeyDownEvent) {
      return e is KeyRepeatEvent
          ? KeyEventResult.ignored
          : KeyEventResult.ignored;
    }
    if (_keyGate(e)) return KeyEventResult.handled;
    final k = e.logicalKey;
    final back =
        k == LogicalKeyboardKey.escape || k == LogicalKeyboardKey.goBack;
    final menu =
        k == LogicalKeyboardKey.contextMenu || k == LogicalKeyboardKey.keyM;
    final c = _c;
    if (_setup == null) {
      if (back) {
        return _back() ? KeyEventResult.handled : KeyEventResult.ignored;
      }
      return KeyEventResult.ignored;
    }
    if (c.requests.isNotEmpty) {
      // A decision is required: Back and Menu do nothing.
      return (back || menu) ? KeyEventResult.handled : KeyEventResult.ignored;
    }
    if (_codecDialog && c.shown is! ReceivedLink) {
      // The consent dialog: Back and Menu mean Not now; the other keys are the dialog's.
      if (back || menu) {
        setState(() => _codecLater = true);
        return KeyEventResult.handled;
      }
      return KeyEventResult.ignored;
    }
    if (_presenting) {
      if (!_controlsShown) {
        // First input only reveals; arrows also focus End (spec 7.1).
        _reveal.poke();
        if (k == LogicalKeyboardKey.arrowUp ||
            k == LogicalKeyboardKey.arrowDown ||
            k == LogicalKeyboardKey.arrowLeft ||
            k == LogicalKeyboardKey.arrowRight) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted && _controlsShown) _endNode.requestFocus();
          });
        }
        return KeyEventResult.handled;
      }
      if (back) {
        _reveal.hideNow();
        return KeyEventResult.handled;
      }
      _reveal.poke();
      return KeyEventResult.ignored;
    }
    if (back) return _back() ? KeyEventResult.handled : KeyEventResult.ignored;
    if (menu) {
      _toggleSettings();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _toggleSettings() {
    if (_c.requests.isNotEmpty || _presenting) return;
    setState(() => _settingsOpen = !_settingsOpen);
    if (!_settingsOpen) _focusSettingsButton();
  }

  void _focusSettingsButton() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _settingsNode.requestFocus();
    });
  }

  /// The system Back (Android back button or gesture, Escape on desktop). True = consumed.
  bool _back() {
    if (!_ready) return true;
    if (_setup == null) {
      final w = _wizard;
      if (w != null && (w.wizard.step > 0 || w.showField)) {
        w.back();
        return true;
      }
      return _doubleBack();
    }
    final c = _c;
    if (c.requests.isNotEmpty) return true;
    if (_codecDialog && c.shown is! ReceivedLink) {
      setState(() => _codecLater = true); // Back = Not now (spec 4.7)
      return true;
    }
    final shown = c.shown;
    if (shown is ReceivedLink) {
      c.closeContent();
      return true;
    }
    if (_settingsOpen) {
      setState(() => _settingsOpen = false);
      _focusSettingsButton();
      return true;
    }
    if (_presenting) {
      _controlsShown ? _reveal.hideNow() : _reveal.poke();
      return true;
    }
    return _doubleBack();
  }

  /// Android TV: the first Back is consumed, a second within 2 s leaves the app (spec 7.1).
  /// Elsewhere Back does nothing.
  bool _doubleBack() {
    if (!_tv) return true;
    final now = _t;
    if (now - _lastBack <= doubleBackWindow) {
      (widget.leaveApp ?? () => SystemNavigator.pop())();
    }
    _lastBack = now;
    return true;
  }

  @override
  Future<bool> didPopRoute() async => _back();

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _life = state;
    final ends =
        widget.endOnBackground ?? (_tv || Platform.isAndroid || Platform.isIOS);
    if (state == AppLifecycleState.paused && ends && _setup != null) {
      // The session ends like End, without a toast (native-apps.md 15).
      _silentEnd = true;
      if (!_c.endRoom()) _silentEnd = false;
    }
    unawaited(_wake.set(_presenting && state == AppLifecycleState.resumed));
    _syncAnnounce();
  }

  void _pointer(PointerEvent e, {required bool press}) {
    if (e.kind == PointerDeviceKind.mouse && !press && e.delta == Offset.zero) {
      return;
    }
    if (!_tv) _ring.value = false;
    final was = _dim.activity();
    if (_presenting && _setup != null) {
      if (e.kind == PointerDeviceKind.mouse && !press) {
        _reveal.mouseMove();
      } else {
        _reveal.poke();
      }
    }
    if (was && mounted) setState(() {});
  }

  // --- actions ---------------------------------------------------------------------------

  void _finishSetup(DisplaySetup s) {
    unawaited(widget.store.saveSetup(s));
    setState(() {
      _setup = s;
      _wizard = null;
    });
    _startRoom(s);
    _syncDim();
    _focusSettingsButton();
  }

  void _chooseMode(DisplayJoinMode m) {
    _c.setJoinMode(toCoreMode(m));
    if (m != DisplayJoinMode.locked && _setup != null) {
      // Open and Approval on become the new default; Locked is never stored (spec 4.3).
      final mode = m == DisplayJoinMode.open
          ? DefaultMode.open
          : DefaultMode.confirm;
      _setup = _setup!.copyWith(mode: mode);
      _c.restartMode = _defaultCoreMode(mode);
      unawaited(widget.store.saveSetup(_setup!));
    }
  }

  void _chooseAppearance(Appearance a) {
    setState(() => _appearance = a);
    unawaited(widget.store.saveAppearance(a));
  }

  void _decide(JoinRequestView r, {required bool allow, bool block = false}) {
    final t = AppLocalizations.of(context);
    final who = _quoted(t, r.name);
    _c.decideJoin(r.id, allow: allow, block: block);
    if (allow) _justAllowed = true;
    _showToast(
      allow
          ? t.toastAllowed(who)
          : block
          ? t.toastBlocked(who)
          : t.toastDenied(who),
    );
    if (_c.requests.isEmpty) _focusSettingsButton();
  }

  // --- build -----------------------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    final platform = MediaQuery.platformBrightnessOf(context);
    final tokens = DisplayTokens.of(_appearance, platform);
    final reduce = MediaQuery.disableAnimationsOf(context);
    final t = AppLocalizations.of(context);
    return DisplayScope(
      tokens: tokens,
      ringVisible: _ring,
      reduceMotion: reduce,
      keyGate: _keyGate,
      child: Theme(
        data: displayTheme(tokens),
        child: Builder(
          builder: (context) {
            if (!_ready) return ColoredBox(color: tokens.bg);
            final presenting = _presenting && _setup != null;
            final c = _c;
            final head = c.requests.isEmpty ? null : c.requests.first;
            final shown = c.shown;
            final link = shown is ReceivedLink ? shown : null;
            final hidePointer = presenting && !_controlsShown;
            final receiving = presenting ? c.receiving : null;
            final errorToast = c.noteError == null
                ? null
                : _contentToast(t, c.noteError!);
            if (errorToast != null) {
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (!mounted) return;
                _c.dismissNote();
                _showToast(errorToast);
              });
            }
            final canvas = DisplayCanvas(
              background: !presenting,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  if (_setup == null)
                    _setupLayer()
                  else if (!presenting)
                    IdleScreen(
                      room: _setup!.room,
                      mode: fromCoreMode(c.joinMode),
                      code: c.roomCode,
                      dom: _server,
                      altAddress: _boxIp,
                      view: _view,
                      cause: _cause,
                      dimmed: _dimmed,
                      settingsFocus: _settingsNode,
                      ringSuppressed:
                          _settingsOpen || head != null || link != null,
                      settingsAutofocus:
                          !_settingsOpen && head == null && link == null,
                      onSettings: () => setState(() => _settingsOpen = true),
                    ),
                  // After "Not now" (also while the download goes on): the state line on the black
                  // stage, no box, at rest too (spec 4.7, N31). It goes with the picture, the
                  // presenter or End; input shows the controls above it.
                  if (presenting && _codecWaiting && _codecLater)
                    Center(
                      child: Text(
                        t.codecStageHint,
                        key: const Key('codec-stage-hint'),
                        textAlign: TextAlign.center,
                        style: DisplayScope.of(context).type.stageHint,
                      ),
                    ),
                  if (presenting)
                    PresentingControls(
                      name: c.peerName ?? '',
                      sas: c.connectionCode,
                      inert: _codecDialog,
                      // The dialog's appearance does not reveal the controls (N31): they stay
                      // under the scrim and fade on their normal timer.
                      shown: _controlsShown,
                      endFocus: _endNode,
                      onEnd: () {
                        _c.endRoom();
                      },
                    ),
                  // A page that cannot be drawn: its text centred in a name-tag box (spec 4.5).
                  if (presenting &&
                      shown is ReceivedDocument &&
                      c.note == ContentNote.renderFailed)
                    PageErrorBox(text: t.receiveDocumentPageErrorFailed),
                  if (receiving != null)
                    TransferPill(
                      name: receiving.name,
                      percent: '${receiving.percent}',
                      format: t.receiveTransfer,
                    ),
                  if (_settingsOpen && !presenting && head == null)
                    SettingsDialog(
                      mode: fromCoreMode(c.joinMode),
                      appearance: _appearance,
                      remote: _tv,
                      onMode: _chooseMode,
                      onAppearance: _chooseAppearance,
                      onClose: () {
                        setState(() => _settingsOpen = false);
                        _focusSettingsButton();
                      },
                    ),
                  if (link != null && head == null)
                    LinkCard(
                      host: link.host,
                      url: link.url,
                      canOpen: _canOpen,
                      guard: widget.guard,
                      onOpen: () => _c.openLink(widget.openUrl),
                      onDismiss: _c.closeContent,
                    ),
                  if (head != null)
                    RequestDialog(
                      id: head.id,
                      name: head.name,
                      check: head.check,
                      more: c.requests.length - 1,
                      guard: widget.guard,
                      onAllow: () => _decide(head, allow: true),
                      onDeny: () => _decide(head, allow: false),
                      onBlock: () => _decide(head, allow: false, block: true),
                    ),
                  if (_codecDialog && head == null && link == null)
                    CodecDialog(
                      controller: _codec!,
                      guard: widget.guard,
                      onLater: () => setState(() => _codecLater = true),
                    ),
                  if (_toast != null) DToast(text: _toast!),
                ],
              ),
            );
            return Listener(
              onPointerDown: (e) => _pointer(e, press: true),
              onPointerMove: (e) => _pointer(e, press: false),
              onPointerHover: (e) => _pointer(e, press: false),
              child: MouseRegion(
                cursor: hidePointer
                    ? SystemMouseCursors.none
                    : MouseCursor.defer,
                child: Focus(
                  focusNode: _rootNode,
                  onKeyEvent: _onKey,
                  child: FocusTraversalGroup(
                    policy: RowsPolicy(),
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        ColoredBox(color: tokens.bg),
                        if (presenting) PictureStage(controller: c),
                        canvas,
                      ],
                    ),
                  ),
                ),
              ),
            );
          },
        ),
      ),
    );
  }

  Widget _setupLayer() {
    final t = AppLocalizations.of(context);
    _wizard ??= SetupController(
      suggestions: [
        t.setupNameS1,
        t.setupNameS2,
        t.setupNameS3,
        t.setupNameS4,
        t.setupNameS5,
      ],
      options: _addressOptions(),
      changeAllowed: !widget.settings.locked,
      onFinish: _finishSetup,
    );
    return SetupWizardView(controller: _wizard!, remote: _tv);
  }
}
