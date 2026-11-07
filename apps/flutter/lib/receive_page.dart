// SPDX-License-Identifier: GPL-3.0-or-later
// "Receive on this device": server field, big room code and status. When the sender's H.264
// screen arrives, the picture fills the window (F11 = full screen, I = statistics). Images,
// PDF pages and links that the sender shares take the window instead (the latest content wins);
// the close button goes back. Page changes come from the sender only.

import 'dart:async';
import 'dart:io' show Platform;
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'annotations.dart';
import 'core/android_platform.dart';
import 'codec_controller.dart';
import 'codec_notice.dart';
import 'core/core_api.dart';
import 'discovery.dart';
import 'join_widgets.dart';
import 'receiver_controller.dart';
import 'settings.dart';
import 'widgets.dart';
import 'window_control.dart';

/// The name under which this receiver announces itself in the local network.
// UNCERTAIN: the host name is visible to everybody in the local network (mDNS shows it
// anyway). A per-install random or user-chosen name would be more private; not decided.
String defaultReceiverName() {
  try {
    final h = Platform.localHostname.trim();
    if (h.isNotEmpty) return h;
  } catch (_) {
    // Not available: use the product name.
  }
  return 'Comet Cast';
}

/// Opens a link in the system's browser (the real [UrlOpener]).
Future<bool> launchInBrowser(Uri url) =>
    launchUrl(url, mode: LaunchMode.externalApplication);

class ReceivePage extends StatefulWidget {
  const ReceivePage({
    super.key,
    required this.core,
    required this.settings,
    this.setFullscreen = setWindowFullscreen,
    this.receiverName = defaultReceiverName,
    this.openUrl = launchInBrowser,
  });

  final CoreApi core;
  final AppSettings settings;

  /// How the window is made full screen (replaced in tests).
  final SetFullscreen setFullscreen;

  /// The name announced in the local network (replaced in tests).
  final String Function() receiverName;

  /// How a link is opened, only after the person chose "Open" (replaced in tests).
  final UrlOpener openUrl;

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage> {
  late final ReceiverController _controller = ReceiverController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
    setFullscreen: widget.setFullscreen,
    hardwareDecoder: _profile.videoDecoder,
  );

  /// TV / hardware decoder / keep-screen-on settings of this device (fixed for the app's life).
  DeviceProfile get _profile => DeviceProfile.peek(context);
  bool get _tv => _profile.tv;
  late final CodecController _codec = CodecController(
    core: widget.core,
    settings: widget.settings,
    // The device's own decoder (MediaCodec) shows the picture: no OpenH264, no download.
    needed:
        _profile.videoDecoder == null ||
        ReceiverController.hardwareBrokenReason != null,
  );
  bool _serverEmpty = false;

  /// Pen, highlighter, eraser, laser, undo, clear and freeze over the picture or page. Local
  /// to this device (nothing is sent), dropped when the picture or page underneath changes.
  final AnnotationController _annotations = AnnotationController();

  /// What the annotations belong to (see [_syncAnnotations]).
  Object? _annotationKey;

  /// The name under which this receiver is announced now, or null.
  String? _advertisedAs;

  /// The server that the running announcement names.
  String? _advertisedServer;
  int _advertiseRun = 0;
  late final SerialQueue _advertiseQueue = serialQueueFor(
    widget.core,
    'advertise',
  );

  /// A running or queued announcement has been stopped because of the policy.
  bool _announcementStopped = false;

  /// The join menu chip is shown while something moves (pointer, touch, any key) and fades out.
  bool _menuActive = false;
  Timer? _menuTimer;

  void _activity() {
    _menuTimer?.cancel();
    if (!_menuActive && mounted) setState(() => _menuActive = true);
    _menuTimer = Timer(const Duration(seconds: 6), () {
      if (mounted) setState(() => _menuActive = false);
    });
  }

  @override
  void initState() {
    super.initState();
    // Neither call blocks the screen: the results arrive later.
    widget.settings.discover(widget.core);
    widget.settings.addListener(_onSettings);
    _advertise();
    // A screen that stays awake while the receive view is open (a TV or tablet on a table
    // would otherwise go dark in the middle of a talk). UNCERTAIN: not run on a device.
    final platform = _profile.platform;
    if (platform != null) {
      platform.setKeepScreenOn(true).catchError((Object _) {});
      _keepAwakePlatform = platform;
    }
    // A television has no practical way to press "Start": receiving starts by itself.
    if (_tv && widget.settings.serverAddress.trim().isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_controller.isActive) _start();
      });
    }
    _controller.addListener(_onControllerChanged);
    _codec.init();
  }

  /// The hardware decoder failed: the software decoder (OpenH264) takes over, so the codec card
  /// (consent, download, or why it is not available) must be able to show.
  void _onControllerChanged() {
    if (_controller.hardwareError != null && !_codec.needed) {
      unawaited(_codec.require());
    }
  }

  AndroidPlatform? _keepAwakePlatform;

  @override
  void dispose() {
    _menuTimer?.cancel();
    widget.settings.removeListener(_onSettings);
    _controller.removeListener(_onControllerChanged);
    widget.settings.cancelDiscovery();
    _advertiseRun++;
    _keepAwakePlatform?.setKeepScreenOn(false).catchError((Object _) {});
    // Queued behind a start that is still running, so that it cannot outlive this view.
    _advertiseQueue
        .run(() => widget.core.stopAdvertising())
        .catchError((Object _) {});
    _codec.dispose();
    _controller.dispose();
    _annotations.dispose();
    super.dispose();
  }

  /// Drop the annotations when what they were drawn on is gone: the live picture ended, or
  /// another image or page (a page turn included) is shown. Called while building, so the
  /// reset does not notify. The key is the content (its epoch, which changes whenever other
  /// content is shown) and, for a document, the page number: a new picture of the same page
  /// (a resize renders it again) keeps the marks, a page turn or another document drops them.
  void _syncAnnotations(ReceiverController c) {
    final shown = c.shown;
    final Object? key = switch (shown) {
      ReceivedImage() => ('image', c.contentEpoch),
      ReceivedDocument() => ('document', c.contentEpoch, shown.page),
      ReceivedLink() => null,
      null => c.hasLiveVideo ? 'video' : null,
    };
    if (key != _annotationKey) {
      _annotationKey = key;
      _annotations.reset(notify: false);
    }
  }

  /// A policy forbids announcing (also one that is learned late, from the network search).
  bool get _announcingForbidden =>
      !widget.settings.announcementsAllowed || widget.settings.locked;

  void _onSettings() {
    if (!_announcingForbidden || _announcementStopped || !mounted) return;
    _announcementStopped = true;
    _advertiseRun++; // a running start must not go on with its fallback
    _advertiseQueue
        .run(() => widget.core.stopAdvertising())
        .catchError((Object _) {});
    if (_advertisedAs != null || _advertisedServer != null) {
      setState(() {
        _advertisedAs = null;
        _advertisedServer = null;
      });
    }
  }

  /// Announce this receiver and [server] (null = no server) in the local network while this
  /// view is open. The room code is not part of it. A server that the checks of the core
  /// refuse (for example 127.0.0.1) is left out; the receiver is announced all the same.
  /// Nothing is announced where the policy forbids it. The calls to the core are queued, and
  /// a view that was closed (or a newer call) is checked before every one of them.
  Future<void> _advertise() async {
    if (_announcingForbidden) return;
    _announcementStopped = false;
    final run = ++_advertiseRun;
    bool stale() => !mounted || run != _advertiseRun;
    final name = _profile.deviceName ?? widget.receiverName();
    final server = widget.settings.serverAddress.trim();
    try {
      final result = await _advertiseQueue.run<(AdvertiseOutcome, String?)?>(
        () async {
          if (stale()) return null;
          var outcome = await widget.core.startAdvertising(
            name: name,
            preferredServer: server.isEmpty ? null : server,
          );
          var named = server.isEmpty ? null : server;
          if (outcome == AdvertiseOutcome.refused && named != null) {
            if (stale()) return null;
            outcome = await widget.core.startAdvertising(name: name);
            named = null;
          }
          return (outcome, named);
        },
      );
      if (result == null || stale()) return;
      setState(() {
        _advertisedAs = result.$1 == AdvertiseOutcome.started ? name : null;
        _advertisedServer = result.$2;
      });
    } catch (_) {
      // Announcing is a convenience; the receiver works without.
      if (!stale()) setState(() => _advertisedAs = null);
    }
  }

  void _start() {
    final server = widget.settings.serverAddress.trim();
    setState(() => _serverEmpty = server.isEmpty);
    if (server.isEmpty) return;
    widget.settings.rememberServer();
    // The announcement names the server that is really used now.
    if (server != _advertisedServer) _advertise();
    _controller.start(server, joinMode: widget.settings.joinMode);
  }

  /// Escape leaves full screen first, and only then goes back to the previous screen.
  void _escape() {
    if (_controller.fullscreen) {
      _controller.toggleFullscreen();
    } else {
      Navigator.of(context).maybePop();
    }
  }

  @override
  Widget build(BuildContext context) {
    return FocusTraversalGroup(
      policy: ReadingOrderTraversalPolicy(),
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): _escape,
          const SingleActivator(LogicalKeyboardKey.f11):
              _controller.toggleFullscreen,
        },
        child: ListenableBuilder(
          listenable: _controller,
          builder: (context, _) {
            _syncAnnotations(_controller);
            final onForm =
                _controller.shown == null &&
                !(_controller.textureId != null && _controller.hasLiveVideo) &&
                _controller.image == null;
            final page = _controller.shown != null
                ? _buildContent(context)
                : _controller.textureId != null && _controller.hasLiveVideo
                ? _buildTextureVideo(context)
                : _controller.image != null
                ? _buildVideo(context)
                : _buildForm(context);
            final running =
                _controller.roomCode != null &&
                _controller.phase == ReceiverPhase.running;
            if (!running) return page;
            // A request takes over the display (a dialog on the code page, a banner while
            // somebody presents). While somebody presents the join menu is a small chip that
            // appears when something moves and fades out; it opens on demand.
            return Listener(
              onPointerHover: (_) => _activity(),
              onPointerDown: (_) => _activity(),
              onPointerMove: (_) => _activity(),
              child: Focus(
                canRequestFocus: false,
                skipTraversal: true,
                onKeyEvent: (_, _) {
                  _activity();
                  return KeyEventResult.ignored;
                },
                child: Stack(
                  children: [
                    page,
                    if (!onForm)
                      Positioned(
                        top: 8,
                        left: 8,
                        child: SafeArea(
                          child: JoinChip(
                            controller: _controller,
                            settings: widget.settings,
                            tv: _tv,
                            visible: _menuActive,
                          ),
                        ),
                      ),
                    Positioned.fill(
                      child: JoinTakeover(
                        controller: _controller,
                        banner: !onForm,
                        tv: _tv,
                      ),
                    ),
                  ],
                ),
              ),
            );
          },
        ),
      ),
    );
  }

  // --- the picture ---------------------------------------------------------------------

  /// The picture of the hardware decoder: a Flutter texture. No pixels pass through Dart, so the
  /// toolbar of annotations (which needs the picture for "freeze") and the size-to-fit scaling
  /// of the software path are not available here. UNCERTAIN: not run on a device.
  Widget _buildTextureVideo(BuildContext context) {
    final t = AppLocalizations.of(context);
    final c = _controller;
    final (w, h) = c.pictureSize;
    return CallbackShortcuts(
      bindings: {const SingleActivator(LogicalKeyboardKey.keyI): c.toggleStats},
      child: Focus(
        autofocus: true,
        child: Scaffold(
          backgroundColor: Colors.black,
          body: Stack(
            fit: StackFit.expand,
            children: [
              Center(
                child: AspectRatio(
                  aspectRatio: w / h,
                  child: Semantics(
                    image: true,
                    label: t.videoViewLabel,
                    child: Texture(
                      key: const Key('videoTexture'),
                      textureId: c.textureId!,
                    ),
                  ),
                ),
              ),
              Positioned(
                top: 8,
                right: 8,
                child: _VideoToolbar(controller: c, onStop: c.stop),
              ),
              ..._contentOverlays(c, t),
              if (c.showStats)
                Positioned(
                  top: 8,
                  left: 8,
                  child: _VideoStatsOverlay(controller: c, t: t),
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildVideo(BuildContext context) {
    final t = AppLocalizations.of(context);
    final c = _controller;
    return CallbackShortcuts(
      bindings: {const SingleActivator(LogicalKeyboardKey.keyI): c.toggleStats},
      child: Focus(
        // Keeps the keys (F11, Escape, I) working although the form is gone.
        autofocus: true,
        child: Scaffold(
          backgroundColor: Colors.black,
          body: Stack(
            fit: StackFit.expand,
            children: [
              ListenableBuilder(
                listenable: _annotations,
                builder: (context, _) {
                  // Frozen: the picture of the moment; the live stream keeps being decoded.
                  final image = _annotations.frozenImage ?? c.image!;
                  return LayoutBuilder(
                    builder: (context, box) {
                      final dpr = MediaQuery.devicePixelRatioOf(context);
                      final w = (box.maxWidth * dpr).round();
                      final h = (box.maxHeight * dpr).round();
                      WidgetsBinding.instance.addPostFrameCallback(
                        (_) => c.reportViewSize(w, h),
                      );
                      return Center(
                        child: AspectRatio(
                          aspectRatio: image.width / image.height,
                          child: Stack(
                            fit: StackFit.expand,
                            children: [
                              // The boundary is exactly the picture (tests read its pixels).
                              Semantics(
                                image: true,
                                label: t.videoViewLabel,
                                child: RepaintBoundary(
                                  key: const Key('videoView'),
                                  child: RawImage(
                                    image: image,
                                    fit: BoxFit.fill,
                                    filterQuality: FilterQuality.low,
                                  ),
                                ),
                              ),
                              AnnotationLayer(controller: _annotations),
                            ],
                          ),
                        ),
                      );
                    },
                  );
                },
              ),
              Positioned(
                top: 8,
                right: 8,
                child: _VideoToolbar(controller: c, onStop: c.stop),
              ),
              _annotationToolbar(freezeSource: () => c.image),
              ListenableBuilder(
                listenable: _annotations,
                builder: (context, _) => _annotations.frozen
                    ? const Positioned(
                        top: 56,
                        left: 0,
                        right: 0,
                        child: Center(child: FrozenHint()),
                      )
                    : const SizedBox.shrink(),
              ),
              ..._contentOverlays(c, t),
              if (c.showStats)
                Positioned(
                  top: 8,
                  left: 8,
                  child: _VideoStatsOverlay(controller: c, t: t),
                ),
            ],
          ),
        ),
      ),
    );
  }

  /// The toolbar, bottom left. UNCERTAIN: fixed in place (the web toolbar can be dragged);
  /// on a very narrow window it scrolls sideways.
  Widget _annotationToolbar({ui.Image? Function()? freezeSource}) =>
      _tv ? const SizedBox.shrink() : _annotationToolbarBox(freezeSource);

  Widget _annotationToolbarBox(ui.Image? Function()? freezeSource) =>
      Positioned(
        left: 8,
        bottom: 8,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: math.max(48, MediaQuery.sizeOf(context).width - 16),
          ),
          child: AnnotationToolbar(
            controller: _annotations,
            freezeSource: freezeSource,
          ),
        ),
      );

  // --- received content (image, PDF page, link) -----------------------------------------

  /// The note and the "receiving" line that float over the picture or the content.
  List<Widget> _contentOverlays(ReceiverController c, AppLocalizations t) => [
    if (c.receiving != null)
      Positioned(
        top: 8,
        left: 8,
        child: Container(
          key: const Key('receivingBanner'),
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: Colors.black54,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Text(
            t.contentReceiving(c.receiving!.name),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(color: Colors.white),
          ),
        ),
      ),
    if (c.note != null)
      Positioned(
        left: 16,
        right: 16,
        bottom: 16,
        child: _ContentNote(note: c.note!, onHide: c.dismissNote),
      ),
  ];

  Widget _buildContent(BuildContext context) {
    final t = AppLocalizations.of(context);
    final c = _controller;
    final shown = c.shown!;
    return Focus(
      // Keeps F11 and Escape working although the form is gone. A link card focuses its own
      // (safe) button instead.
      autofocus: shown is! ReceivedLink,
      child: Scaffold(
        backgroundColor: Colors.black,
        body: Stack(
          fit: StackFit.expand,
          children: [
            switch (shown) {
              ReceivedImage() => _ImageContent(
                image: shown,
                t: t,
                overlay: AnnotationLayer(controller: _annotations),
              ),
              ReceivedDocument() => _DocumentContent(
                doc: shown,
                controller: c,
                t: t,
                overlay: AnnotationLayer(controller: _annotations),
              ),
              ReceivedLink() => _LinkCard(
                link: shown,
                t: t,
                onOpen: () => c.openLink(widget.openUrl),
                onDismiss: c.closeContent,
              ),
            },
            Positioned(
              top: 8,
              right: 8,
              child: _ContentToolbar(controller: c, onStop: c.stop),
            ),
            if (shown is! ReceivedLink) _annotationToolbar(),
            ..._contentOverlays(c, t),
          ],
        ),
      ),
    );
  }

  // --- the form (before the picture arrives) -----------------------------------------

  Widget _buildForm(BuildContext context) {
    final t = AppLocalizations.of(context);
    final hint = _videoHint(t);
    return Scaffold(
      appBar: AppBar(title: Text(t.receiveTitle)),
      body: PageBody(
        children: [
          ServerField(
            settings: widget.settings,
            enabled: !_controller.isActive,
            autofocus: true,
            onSubmitted: _controller.isActive ? null : _start,
          ),
          if (_controller.hardwareError != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                t.hardwareDecoderFailed,
                key: const Key('hardwareError'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          CodecNotice(controller: _codec),
          if (_serverEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                t.serverEmpty,
                key: const Key('serverError'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          const SizedBox(height: 16),
          Align(
            alignment: Alignment.centerLeft,
            child: _controller.isActive
                ? OutlinedButton.icon(
                    key: const Key('stopButton'),
                    autofocus: _tv,
                    onPressed: _controller.stop,
                    icon: const Icon(Icons.stop),
                    label: Text(t.stop),
                  )
                : FilledButton.icon(
                    key: const Key('startButton'),
                    autofocus: _tv,
                    onPressed: _start,
                    icon: const Icon(Icons.play_arrow),
                    label: Text(t.startReceiving),
                  ),
          ),
          const SizedBox(height: 24),
          _RoomCodeCard(code: _controller.roomCode, tv: _tv),
          const SizedBox(height: 16),
          // liveRegion: screen readers announce changes of the status.
          Semantics(
            liveRegion: true,
            child: Text(
              _statusText(t),
              key: const Key('statusText'),
              style: Theme.of(context).textTheme.bodyLarge,
            ),
          ),
          if (hint != null) ...[
            const SizedBox(height: 8),
            Text(
              hint,
              key: const Key('videoHint'),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ],
          if (_controller.note != null) ...[
            const SizedBox(height: 8),
            _ContentNote(
              note: _controller.note!,
              onHide: _controller.dismissNote,
              onLight: true,
            ),
          ],
          if (_advertisedAs != null) ...[
            const SizedBox(height: 8),
            Text(
              t.advertisedNote(_advertisedAs!),
              key: const Key('advertisedNote'),
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
          if (_controller.roomCode != null &&
              _controller.phase == ReceiverPhase.running) ...[
            const SizedBox(height: 16),
            JoinHostPanel(
              controller: _controller,
              settings: widget.settings,
              tv: _tv,
            ),
          ],
          const SizedBox(height: 16),
          _StatsCard(controller: _controller),
          const SizedBox(height: 16),
          NoticeBox(
            noticeKey: const Key('receiveNotice'),
            text: t.receiveNotice,
          ),
        ],
      ),
    );
  }

  /// A line under the status: why no picture is visible yet.
  String? _videoHint(AppLocalizations t) {
    final c = _controller;
    if (c.phase != ReceiverPhase.running) return null;
    if (c.codecNotShown) return t.videoCodecNotShown(c.codec!);
    if (c.senderJoined || c.connectionState != null) return t.videoWaiting;
    return null;
  }

  String _statusText(AppLocalizations t) {
    final c = _controller;
    if (c.error != null) return t.statusError(c.error!);
    if (c.senderLeft) return t.statusPeerLeftSender;
    if (c.receiving != null && c.phase == ReceiverPhase.running) {
      return t.contentReceiving(c.receiving!.name);
    }
    switch (c.phase) {
      case ReceiverPhase.idle:
        return t.statusIdle;
      case ReceiverPhase.starting:
        return t.statusConnecting;
      case ReceiverPhase.ended:
        return t.statusEnded;
      case ReceiverPhase.running:
        if (c.connectionState != null) {
          return connectionStateText(t, c.connectionState);
        }
        if (c.senderJoined) return t.statusSenderJoined;
        return t.statusWaitingForSender;
    }
  }
}

/// Shows the room code very large, with a copy button.
class _RoomCodeCard extends StatelessWidget {
  const _RoomCodeCard({required this.code, this.tv = false});

  final String? code;

  /// 10-foot display: the code is shown as large as the width allows and the copy button
  /// (useless without a pointer and a keyboard) is left out.
  final bool tv;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final theme = Theme.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 24, horizontal: 16),
        child: Column(
          children: [
            Text(t.roomCodeLabel, style: theme.textTheme.labelLarge),
            const SizedBox(height: 8),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Flexible(
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: Semantics(
                      // Screen readers read the letters one by one.
                      label: code?.split('').join(' '),
                      child: SelectableText(
                        code ?? '······',
                        key: const Key('roomCode'),
                        style: theme.textTheme.displayLarge?.copyWith(
                          fontFamily: 'monospace',
                          fontWeight: FontWeight.bold,
                          letterSpacing: tv ? 16 : 8,
                          // FittedBox scales it down if the screen is narrower than this.
                          fontSize: tv ? 160 : null,
                        ),
                      ),
                    ),
                  ),
                ),
                if (code != null && !tv)
                  IconButton(
                    key: const Key('copyCodeButton'),
                    tooltip: t.copyCode,
                    icon: const Icon(Icons.copy),
                    onPressed: () async {
                      await Clipboard.setData(ClipboardData(text: code!));
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context)
                          .showSnackBar(SnackBar(content: Text(t.codeCopied)));
                    },
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _StatsCard extends StatelessWidget {
  const _StatsCard({required this.controller});

  final ReceiverController controller;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final theme = Theme.of(context);
    Widget row(String label, String value, Key key) => Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Expanded(child: Text(label)),
          Text(
            value,
            key: key,
            style: theme.textTheme.titleMedium?.copyWith(
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
          ),
        ],
      ),
    );
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(t.statsTitle, style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            row(
              t.statsCodec,
              controller.codec ?? t.statsCodecUnknown,
              const Key('statCodec'),
            ),
            row(
              t.statsPackets,
              '${controller.packets}',
              const Key('statPackets'),
            ),
            row(t.statsBytes, '${controller.bytes}', const Key('statBytes')),
            row(t.statsFrames, '${controller.frames}', const Key('statFrames')),
            row(
              t.statsKeyframes,
              '${controller.keyframes}',
              const Key('statKeyframes'),
            ),
          ],
        ),
      ),
    );
  }
}

/// Buttons over the picture: statistics, full screen, stop.
class _VideoToolbar extends StatelessWidget {
  const _VideoToolbar({required this.controller, required this.onStop});

  final ReceiverController controller;
  final VoidCallback onStop;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final c = controller;
    return Material(
      color: Colors.black54,
      borderRadius: BorderRadius.circular(24),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            key: const Key('statsButton'),
            tooltip: c.showStats ? t.videoStatsHide : t.videoStatsShow,
            color: Colors.white,
            icon: const Icon(Icons.analytics_outlined),
            onPressed: c.toggleStats,
          ),
          IconButton(
            key: const Key('fullscreenButton'),
            tooltip: c.fullscreen ? t.fullscreenExit : t.fullscreenEnter,
            color: Colors.white,
            icon: Icon(c.fullscreen ? Icons.fullscreen_exit : Icons.fullscreen),
            onPressed: c.toggleFullscreen,
          ),
          IconButton(
            key: const Key('stopButton'),
            tooltip: t.stop,
            color: Colors.white,
            icon: const Icon(Icons.stop),
            onPressed: onStop,
          ),
        ],
      ),
    );
  }
}

/// Optional statistics over the picture.
class _VideoStatsOverlay extends StatelessWidget {
  const _VideoStatsOverlay({required this.controller, required this.t});

  final ReceiverController controller;
  final AppLocalizations t;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    final v = c.videoStats;
    String ms(int us) => (us / 1000).toStringAsFixed(1);
    final lines = <String>[
      '${t.videoStatsSize}: ${c.pictureSize.$1} x ${c.pictureSize.$2}'
          '${c.textureId != null ? ' (${t.videoStatsHardware})' : ''}',
      '${t.videoStatsDisplayRate}: ${c.displayFps.toStringAsFixed(1)} fps',
      '${t.videoStatsDecoded}: ${v.decoded}',
      '${t.videoStatsDropped}: ${v.dropped}',
      '${t.videoStatsDecodeTime}: ${ms(v.avgDecodeUs)} ms',
      '${t.videoStatsKeyRequests}: ${v.keyframeRequests}',
      '${t.videoStatsDecodeErrors}: ${v.decodeErrors}',
      t.videoStatsUiTime(ms(c.avgFetchUs), ms(c.avgImageUs)),
      if (v.error != null) t.videoStatsError(v.error!),
    ];
    return Semantics(
      container: true,
      label: t.videoStatsLabel,
      child: Container(
        key: const Key('videoStats'),
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: Colors.black54,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Text(
          lines.join('\n'),
          style: const TextStyle(
            color: Colors.white,
            fontFamily: 'monospace',
            fontSize: 12,
          ),
        ),
      ),
    );
  }
}

/// The text of a [ContentNote] (the words are in the ARB files).
String contentNoteText(AppLocalizations t, ContentNote n) => switch (n) {
  ContentNote.badMessage => t.contentNoteBadMessage,
  ContentNote.incomplete => t.contentNoteIncomplete,
  ContentNote.tooLarge => t.contentNoteTooLarge,
  ContentNote.documentRejected => t.contentNoteDocumentRejected,
  ContentNote.renderFailed => t.contentNoteRenderFailed,
  ContentNote.tooManyErrors => t.contentNoteTooManyErrors,
  ContentNote.transferTimeout => t.contentNoteTimeout,
  ContentNote.workerUnavailable => t.contentNoteWorkerUnavailable,
  ContentNote.imageUndecodable => t.contentImageUndecodable,
  ContentNote.imageTooBig => t.contentImageTooBig,
  ContentNote.linkOpenFailed => t.contentNoteLinkOpenFailed,
};

/// A calm, dismissable note (never alarming, never text from the sender).
class _ContentNote extends StatelessWidget {
  const _ContentNote({
    required this.note,
    required this.onHide,
    this.onLight = false,
  });

  final ContentNote note;
  final VoidCallback onHide;

  /// On the light form (not over a black picture).
  final bool onLight;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    final fg = onLight ? scheme.onSecondaryContainer : Colors.white;
    return Semantics(
      liveRegion: true,
      container: true,
      child: Material(
        key: const Key('contentNote'),
        color: onLight ? scheme.secondaryContainer : Colors.black87,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.only(left: 12, top: 4, bottom: 4),
          child: Row(
            children: [
              Icon(Icons.info_outline, color: fg),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  contentNoteText(t, note),
                  key: const Key('contentNoteText'),
                  style: TextStyle(color: fg),
                ),
              ),
              IconButton(
                key: const Key('contentNoteHide'),
                tooltip: t.contentNoteHide,
                color: fg,
                icon: const Icon(Icons.close),
                onPressed: onHide,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A received image, fitted into the window.
class _ImageContent extends StatelessWidget {
  const _ImageContent({required this.image, required this.t, this.overlay});

  final ReceivedImage image;
  final AppLocalizations t;

  /// The drawing layer; it covers exactly the picture.
  final Widget? overlay;

  /// Larger pictures are decoded at this size at most (memory).
  /// UNCERTAIN: 4096 is a guess; a PNG may still be decoded in full by the engine first.
  static const _maxDecodeSide = 4096;

  @override
  Widget build(BuildContext context) {
    final big = math.max(image.width, image.height) > _maxDecodeSide;
    final wide = image.width >= image.height;
    final picture = Image.memory(
      image.bytes,
      key: const Key('contentImage'),
      fit: BoxFit.contain,
      cacheWidth: big && wide ? _maxDecodeSide : null,
      cacheHeight: big && !wide ? _maxDecodeSide : null,
      filterQuality: FilterQuality.medium,
      errorBuilder: (context, error, stack) => Text(
        t.contentImageUndecodable,
        key: const Key('contentImageError'),
        style: const TextStyle(color: Colors.white),
      ),
    );
    final o = overlay;
    return Center(
      child: Semantics(
        image: true,
        label: t.contentImageLabel,
        child: (o == null || image.width <= 0 || image.height <= 0)
            ? picture
            : AspectRatio(
                aspectRatio: image.width / image.height,
                child: Stack(fit: StackFit.expand, children: [picture, o]),
              ),
      ),
    );
  }
}

/// A received PDF page, drawn by the core at the width of this view, with a page indicator.
/// The page changes only when the sender says so.
class _DocumentContent extends StatelessWidget {
  const _DocumentContent({
    required this.doc,
    required this.controller,
    required this.t,
    this.overlay,
  });

  /// The drawing layer; it covers exactly the page.
  final Widget? overlay;

  final ReceivedDocument doc;
  final ReceiverController controller;
  final AppLocalizations t;

  @override
  Widget build(BuildContext context) {
    final img = doc.image;
    return LayoutBuilder(
      builder: (context, box) {
        final dpr = MediaQuery.devicePixelRatioOf(context);
        final w = (box.maxWidth * dpr).round();
        WidgetsBinding.instance.addPostFrameCallback(
          (_) => controller.reportContentWidth(w),
        );
        if (img == null) {
          return Center(
            child: Text(
              t.contentPageOpening(doc.name),
              key: const Key('pageOpening'),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(color: Colors.white),
            ),
          );
        }
        return Stack(
          fit: StackFit.expand,
          children: [
            Center(
              child: AspectRatio(
                aspectRatio: img.width / img.height,
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    Semantics(
                      image: true,
                      label: t.contentPageLabel,
                      child: RepaintBoundary(
                        // The boundary is exactly the page (tests read its pixels).
                        key: const Key('pageView'),
                        child: RawImage(
                          image: img,
                          fit: BoxFit.fill,
                          filterQuality: FilterQuality.medium,
                        ),
                      ),
                    ),
                    ?overlay,
                  ],
                ),
              ),
            ),
            Positioned(
              bottom: 16,
              left: 0,
              right: 0,
              child: Center(
                child: Semantics(
                  liveRegion: true,
                  child: Container(
                    key: const Key('pageIndicatorBox'),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 6,
                    ),
                    decoration: BoxDecoration(
                      color: Colors.black54,
                      borderRadius: BorderRadius.circular(16),
                    ),
                    child: Text(
                      t.contentPageIndicator(doc.page ?? 1, doc.pages),
                      key: const Key('pageIndicator'),
                      style: const TextStyle(color: Colors.white),
                    ),
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

/// "The sender wants to open <host>": Open or Dismiss. Nothing opens before "Open" is
/// pressed; "Dismiss" has the initial focus so that a stray Enter key does not open anything.
class _LinkCard extends StatelessWidget {
  const _LinkCard({
    required this.link,
    required this.t,
    required this.onOpen,
    required this.onDismiss,
  });

  final ReceivedLink link;
  final AppLocalizations t;
  final VoidCallback onOpen;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Card(
          key: const Key('linkCard'),
          margin: const EdgeInsets.all(24),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    const Icon(Icons.link),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        t.contentLinkTitle(link.host),
                        key: const Key('linkTitle'),
                        style: theme.textTheme.titleLarge,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                // The whole address, so that the person can judge it.
                SelectableText(
                  link.url,
                  key: const Key('linkUrl'),
                  maxLines: 3,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    fontFamily: 'monospace',
                  ),
                ),
                const SizedBox(height: 12),
                Text(t.contentLinkHint, style: theme.textTheme.bodySmall),
                const SizedBox(height: 16),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    OutlinedButton(
                      key: const Key('linkDismissButton'),
                      autofocus: true,
                      onPressed: onDismiss,
                      child: Text(t.contentLinkDismiss),
                    ),
                    const SizedBox(width: 12),
                    FilledButton(
                      key: const Key('linkOpenButton'),
                      onPressed: onOpen,
                      child: Text(t.contentLinkOpen),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Buttons over received content: full screen, close, stop receiving.
class _ContentToolbar extends StatelessWidget {
  const _ContentToolbar({required this.controller, required this.onStop});

  final ReceiverController controller;
  final VoidCallback onStop;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final c = controller;
    return Material(
      color: Colors.black54,
      borderRadius: BorderRadius.circular(24),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            key: const Key('fullscreenButton'),
            tooltip: c.fullscreen ? t.fullscreenExit : t.fullscreenEnter,
            color: Colors.white,
            icon: Icon(c.fullscreen ? Icons.fullscreen_exit : Icons.fullscreen),
            onPressed: c.toggleFullscreen,
          ),
          IconButton(
            key: const Key('contentCloseButton'),
            tooltip: t.contentClose,
            color: Colors.white,
            icon: const Icon(Icons.close),
            onPressed: c.closeContent,
          ),
          IconButton(
            key: const Key('stopButton'),
            tooltip: t.stop,
            color: Colors.white,
            icon: const Icon(Icons.stop),
            onPressed: onStop,
          ),
        ],
      ),
    );
  }
}
