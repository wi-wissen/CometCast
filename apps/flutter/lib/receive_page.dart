// SPDX-License-Identifier: GPL-3.0-or-later
// "Receive on this device": server field, big room code and status. When the sender's H.264
// screen arrives, the picture fills the window (F11 = full screen, I = statistics).

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/core_api.dart';
import 'receiver_controller.dart';
import 'settings.dart';
import 'widgets.dart';
import 'window_control.dart';

class ReceivePage extends StatefulWidget {
  const ReceivePage({
    super.key,
    required this.core,
    required this.settings,
    this.setFullscreen = setWindowFullscreen,
  });

  final CoreApi core;
  final AppSettings settings;

  /// How the window is made full screen (replaced in tests).
  final SetFullscreen setFullscreen;

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage> {
  late final ReceiverController _controller = ReceiverController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
    setFullscreen: widget.setFullscreen,
  );
  bool _serverEmpty = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _start() {
    final server = widget.settings.serverAddress.trim();
    setState(() => _serverEmpty = server.isEmpty);
    if (server.isEmpty) return;
    widget.settings.rememberServer();
    _controller.start(server);
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
          builder: (context, _) => _controller.image != null
              ? _buildVideo(context)
              : _buildForm(context),
        ),
      ),
    );
  }

  // --- the picture ---------------------------------------------------------------------

  Widget _buildVideo(BuildContext context) {
    final t = AppLocalizations.of(context);
    final c = _controller;
    final image = c.image!;
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
              LayoutBuilder(
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
                      // The boundary is exactly the picture (tests read its pixels).
                      child: Semantics(
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
                    ),
                  );
                },
              ),
              Positioned(
                top: 8,
                right: 8,
                child: _VideoToolbar(controller: c, onStop: c.stop),
              ),
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
                    onPressed: _controller.stop,
                    icon: const Icon(Icons.stop),
                    label: Text(t.stop),
                  )
                : FilledButton.icon(
                    key: const Key('startButton'),
                    onPressed: _start,
                    icon: const Icon(Icons.play_arrow),
                    label: Text(t.startReceiving),
                  ),
          ),
          const SizedBox(height: 24),
          _RoomCodeCard(code: _controller.roomCode),
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
  const _RoomCodeCard({required this.code});

  final String? code;

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
                          letterSpacing: 8,
                        ),
                      ),
                    ),
                  ),
                ),
                if (code != null)
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
    final img = c.image;
    String ms(int us) => (us / 1000).toStringAsFixed(1);
    final lines = <String>[
      '${t.videoStatsSize}: ${img?.width ?? 0} x ${img?.height ?? 0}',
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
