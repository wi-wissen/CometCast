// SPDX-License-Identifier: GPL-3.0-or-later
// "Receive on this device": server field, big room code, status and live counters.
// No decoding or rendering yet: the core only counts incoming video.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/core_api.dart';
import 'receiver_controller.dart';
import 'settings.dart';
import 'widgets.dart';

class ReceivePage extends StatefulWidget {
  const ReceivePage({super.key, required this.core, required this.settings});

  final CoreApi core;
  final AppSettings settings;

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage> {
  late final ReceiverController _controller = ReceiverController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
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
    _controller.start(server);
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return EscapeToGoBack(
      child: Scaffold(
        appBar: AppBar(title: Text(t.receiveTitle)),
        body: ListenableBuilder(
          listenable: _controller,
          builder: (context, _) => PageBody(
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
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
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
              const SizedBox(height: 16),
              _StatsCard(controller: _controller),
              const SizedBox(height: 16),
              NoticeBox(
                noticeKey: const Key('receiveNotice'),
                text: t.receiveNotice,
              ),
            ],
          ),
        ),
      ),
    );
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
