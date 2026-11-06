// SPDX-License-Identifier: GPL-3.0-or-later
// "Send": enter the receiver's room code and send the built-in VP8 test video.
// No screen capture yet (see the notice on the screen).

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/core_api.dart';
import 'sender_controller.dart';
import 'settings.dart';
import 'widgets.dart';

/// Test videos longer than this are not offered.
const _maxSeconds = 3600;
const _defaultSeconds = 30;

class SendPage extends StatefulWidget {
  const SendPage({super.key, required this.core, required this.settings});

  final CoreApi core;
  final AppSettings settings;

  @override
  State<SendPage> createState() => _SendPageState();
}

class _SendPageState extends State<SendPage> {
  late final SenderController _controller = SenderController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
  );
  final _code = TextEditingController();
  final _seconds = TextEditingController(text: '$_defaultSeconds');
  final _codeFocus = textFieldFocusNode(debugLabel: 'codeField');
  final _secondsFocus = textFieldFocusNode(debugLabel: 'secondsField');
  bool _triedToStart = false;
  bool _serverEmpty = false;

  @override
  void dispose() {
    _controller.dispose();
    _codeFocus.dispose();
    _secondsFocus.dispose();
    _code.dispose();
    _seconds.dispose();
    super.dispose();
  }

  /// Error text for the code field, or null if the input is fine (or still being typed).
  String? _codeError(AppLocalizations t) {
    final text = _code.text;
    if (text.trim().isEmpty && !_triedToStart) return null;
    final check = widget.core.checkRoomCode(text);
    return switch (check) {
      RoomCodeCheck_Valid() => null,
      RoomCodeCheck_InvalidChar(:final ch) => t.roomCodeInvalidChar(ch),
      // Too short is only an error once the person tried to start (they may still be typing).
      RoomCodeCheck_WrongLength() =>
        (_triedToStart || text.trim().length > widget.core.roomCodeLength)
            ? t.roomCodeWrongLength(widget.core.roomCodeLength)
            : null,
    };
  }

  int? _parsedSeconds() {
    final n = int.tryParse(_seconds.text.trim());
    if (n == null || n < 1 || n > _maxSeconds) return null;
    return n;
  }

  void _start() {
    setState(() {
      _triedToStart = true;
      _serverEmpty = widget.settings.serverAddress.trim().isEmpty;
    });
    final check = widget.core.checkRoomCode(_code.text);
    final seconds = _parsedSeconds();
    if (check is! RoomCodeCheck_Valid || seconds == null || _serverEmpty) {
      return;
    }
    _controller.start(
      code: check.code,
      server: widget.settings.serverAddress,
      seconds: seconds,
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return EscapeToGoBack(
      child: Scaffold(
        appBar: AppBar(title: Text(t.sendTitle)),
        body: ListenableBuilder(
          listenable: _controller,
          builder: (context, _) {
            final active = _controller.isActive;
            return PageBody(
              children: [
                ServerField(
                  settings: widget.settings,
                  enabled: !active,
                  textInputAction: TextInputAction.next,
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
                TextField(
                  key: const Key('codeField'),
                  controller: _code,
                  focusNode: _codeFocus,
                  enabled: !active,
                  autofocus: true,
                  textCapitalization: TextCapitalization.characters,
                  textInputAction: TextInputAction.next,
                  autocorrect: false,
                  enableSuggestions: false,
                  inputFormatters: [
                    _UpperCaseFormatter(),
                    LengthLimitingTextInputFormatter(32),
                  ],
                  style: Theme.of(context).textTheme.headlineSmall
                      ?.copyWith(fontFamily: 'monospace', letterSpacing: 4),
                  decoration: InputDecoration(
                    labelText: t.roomCodeInputLabel,
                    helperText: t.roomCodeHelper(
                      widget.core.roomCodeLength,
                      widget.core.roomCodeAlphabet,
                    ),
                    errorText: _codeError(t),
                    border: const OutlineInputBorder(),
                  ),
                  onChanged: (_) => setState(() {}),
                ),
                const SizedBox(height: 16),
                TextField(
                  key: const Key('secondsField'),
                  controller: _seconds,
                  focusNode: _secondsFocus,
                  enabled: !active,
                  keyboardType: TextInputType.number,
                  textInputAction: TextInputAction.done,
                  inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                  decoration: InputDecoration(
                    labelText: t.durationLabel,
                    errorText: _triedToStart && _parsedSeconds() == null
                        ? t.durationInvalid
                        : null,
                    border: const OutlineInputBorder(),
                  ),
                  onChanged: (_) => setState(() {}),
                  onSubmitted: (_) => active ? null : _start(),
                ),
                const SizedBox(height: 16),
                Align(
                  alignment: Alignment.centerLeft,
                  child: active
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
                          label: Text(t.startSending),
                        ),
                ),
                const SizedBox(height: 24),
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
                NoticeBox(
                  noticeKey: const Key('sendNotice'),
                  text: t.sendNotice,
                ),
              ],
            );
          },
        ),
      ),
    );
  }

  String _statusText(AppLocalizations t) {
    final c = _controller;
    if (c.error != null) return t.statusError(c.error!);
    if (c.receiverLeft) return t.statusPeerLeftReceiver;
    switch (c.phase) {
      case SenderPhase.idle:
        return t.statusIdle;
      case SenderPhase.starting:
        return t.statusConnecting;
      case SenderPhase.finished:
        return t.sendFinished(c.framesSent);
      case SenderPhase.ended:
        return t.statusEnded;
      case SenderPhase.running:
        if (c.framesSent > 0) return t.sendStreaming(c.framesSent);
        if (c.connectionState != null) {
          return connectionStateText(t, c.connectionState);
        }
        return t.sendJoined;
    }
  }
}

/// Turns typed a-z into capitals (room codes are uppercase).
class _UpperCaseFormatter extends TextInputFormatter {
  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) => newValue.copyWith(
    // ASCII only, like Rust: toUpperCase() would turn 'ß' into 'SS'.
    text: newValue.text.replaceAllMapped(
      RegExp('[a-z]'),
      (m) => m[0]!.toUpperCase(),
    ),
  );
}
