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
import '../../codec_notice.dart';
import '../../core/android_platform.dart';
import '../../core/core_api.dart';
import '../../discovery.dart';
import '../../receiver_controller.dart';
import '../../settings.dart';
import '../design/tokens.dart';
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
  final FocusNode _settingsNode = RowNode(row: 0, col: 0, debugLabel: 'settings');
  final FocusNode _endNode = RowNode(row: 0, col: 0, debugLabel: 'end');

  bool _ready = false;
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

  bool get _presenting {
    final shown = _c.shown;
    return _c.hasLiveVideo || shown is ReceivedImage || shown is ReceivedDocument;
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _c = ReceiverController(
      core: widget.core,
      udpBind: widget.settings.udpBind,
      hardwareDecoder: DeviceProfile.peek(context).videoDecoder,
      onNotice: _onNotice,
      restartOnPeerLeft: true,
    )..addListener(_onController);
    final profile = DeviceProfile.peek(context);
    _codec = CodecController(
      core: widget.core,
      settings: widget.settings,
      needed:
          profile.videoDecoder == null ||
          ReceiverController.hardwareBrokenReason != null,
    );
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
    await _codec?.init();
    if (!mounted) return;
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
    _c.removeListener(_onController);
    _c.dispose();
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
    _c.start(_server, joinMode: _defaultCoreMode(setup.mode));
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
    // Activity: a request, a joiner or a mode change counts as activity (spec 8.6).
    _dim.activity();
    // A join request closes Settings: one dialog at a time (spec 4.3).
    if (c.requests.isNotEmpty && _settingsOpen) _settingsOpen = false;
    if (!_presenting && _controlsShown) {
      _reveal.hideNow();
    }
    _syncDim();
    unawaited(_wake.set(_presenting && _life == AppLifecycleState.resumed));
    setState(() {});
    _refocusRoot();
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
      _c.start(_server, joinMode: _defaultCoreMode(_setup!.mode));
    });
  }

  void _onSettings() {
    if (mounted) setState(() {});
    final w = _wizard;
    if (w != null) w.updateOptions(_addressOptions());
  }

  List<AddressOption> _addressOptions() {
    final s = widget.settings;
    AddressSource source(ServerSourceKind k) => switch (k) {
      ServerSourceKind.dns || ServerSourceKind.policy => AddressSource.own,
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
          FocusManager.instance.primaryFocus == FocusManager.instance.rootScope) {
        _rootNode.requestFocus();
      }
    });
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent e) {
    if (e is! KeyDownEvent) {
      return e is KeyRepeatEvent ? KeyEventResult.ignored : KeyEventResult.ignored;
    }
    if (_keyGate(e)) return KeyEventResult.handled;
    final k = e.logicalKey;
    final back =
        k == LogicalKeyboardKey.escape || k == LogicalKeyboardKey.goBack;
    final menu =
        k == LogicalKeyboardKey.contextMenu || k == LogicalKeyboardKey.keyM;
    final c = _c;
    if (_setup == null) {
      if (back) return _back() ? KeyEventResult.handled : KeyEventResult.ignored;
      return KeyEventResult.ignored;
    }
    if (c.requests.isNotEmpty) {
      // A decision is required: Back and Menu do nothing.
      return (back || menu) ? KeyEventResult.handled : KeyEventResult.ignored;
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
            final transferPill =
                presenting && c.receiving != null
                ? t.receiveTransfer(c.receiving!.name, '0')
                : null;
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
                      view: _view,
                      cause: _cause,
                      dimmed: _dimmed,
                      settingsFocus: _settingsNode,
                      settingsAutofocus:
                          !_settingsOpen && head == null && link == null,
                      onSettings: () => setState(() => _settingsOpen = true),
                    ),
                  if (presenting)
                    PresentingControls(
                      name: c.peerName ?? '',
                      shown: _controlsShown,
                      endFocus: _endNode,
                      onEnd: () {
                        _c.endRoom();
                      },
                    ),
                  if (transferPill != null) TransferPill(text: transferPill),
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
                  if (_codecAttention(c))
                    _CodecLayer(controller: _codec!),
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

  bool _codecAttention(ReceiverController c) =>
      _codec != null &&
      _codec!.needed &&
      !_codec!.ready &&
      c.senderJoined &&
      (c.codec == 'h264');

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

/// The existing neutral codec card (consent, download, failure) while an H.264 picture cannot
/// be decoded yet. No designed place yet: docs/design/implementation-questions-flutter.md.
class _CodecLayer extends StatelessWidget {
  const _CodecLayer({required this.controller});
  final CodecController controller;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SizedBox(
        width: 560,
        child: Theme(
          data: ThemeData(useMaterial3: true),
          child: Material(
            type: MaterialType.transparency,
            child: CodecNotice(controller: controller),
          ),
        ),
      ),
    );
  }
}
