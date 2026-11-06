// SPDX-License-Identifier: GPL-3.0-or-later
// "Send": enter the receiver's room code and send the built-in VP8 test video, or switch on
// "Share this screen" to send the screen of this computer (Linux/X11 only for now; the Rust
// core says whether that works on this system). On Android the same switch starts the native
// screen capture (consent dialog, MediaCodec) and sends what it encodes, see capture_feeder.dart.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/android_capture.dart';
import 'core/core_api.dart';
import 'sender_controller.dart';
import 'settings.dart';
import 'widgets.dart';

/// Test videos longer than this are not offered.
const _maxSeconds = 3600;
const _defaultSeconds = 30;

class SendPage extends StatefulWidget {
  const SendPage({
    super.key,
    required this.core,
    required this.settings,
    this.androidCapture,
  });

  final CoreApi core;
  final AppSettings settings;

  /// The Android screen capture; null where there is none.
  final AndroidCapture? androidCapture;

  @override
  State<SendPage> createState() => _SendPageState();
}

class _SendPageState extends State<SendPage> {
  late final SenderController _controller = SenderController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
    capture: widget.androidCapture,
  );
  final _code = TextEditingController();
  final _seconds = TextEditingController(text: '$_defaultSeconds');
  final _codeFocus = textFieldFocusNode(debugLabel: 'codeField');
  final _secondsFocus = textFieldFocusNode(debugLabel: 'secondsField');
  bool _triedToStart = false;
  bool _serverEmpty = false;

  /// Answer of the Rust capability check; null while it runs.
  CaptureInfo? _capture;
  bool _shareScreen = false;
  String? _monitorId;

  @override
  void initState() {
    super.initState();
    widget.core.captureInfo(display: widget.settings.captureDisplay).then((
      info,
    ) {
      if (mounted) setState(() => _capture = info);
    }, onError: (Object _) {});
  }

  bool get _captureSupported => _capture?.support is CaptureSupport_Supported;

  /// The core listed the Android projection (it does so only when built for Android): the
  /// screen is captured by the native code, not by the Rust core.
  bool get _androidMode =>
      widget.androidCapture != null &&
      (_capture?.monitors.any((m) => m.id == androidProjectionMonitorId) ??
          false);

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
    final sharing = _shareScreen && _captureSupported;
    if (check is! RoomCodeCheck_Valid ||
        (!sharing && seconds == null) ||
        _serverEmpty) {
      return;
    }
    widget.settings.rememberServer();
    if (sharing && _androidMode) {
      final t = AppLocalizations.of(context);
      _controller.startCapture(
        code: check.code,
        server: widget.settings.serverAddress,
        notification: CaptureNotificationTexts(
          title: t.captureNotificationTitle,
          text: t.captureNotificationText,
          stopLabel: t.captureNotificationStop,
        ),
      );
      return;
    }
    _controller.start(
      code: check.code,
      server: widget.settings.serverAddress,
      // A screen is shared until the person stops it.
      seconds: sharing ? null : seconds,
      screen: sharing
          ? ScreenRequest(
              display: widget.settings.captureDisplay,
              // "root" (the whole desktop) is also what null means; keep null.
              monitorId: _monitorId == 'root' ? null : _monitorId,
              fps: 15,
              bitrateKbps: 2000,
            )
          : null,
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
                _buildShareScreen(context, t, active),
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
                if (!_shareScreen)
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

  /// The "Share this screen" switch, the monitor list, and the reason if sharing is not
  /// possible here. All of it comes from `captureInfo` (Rust).
  Widget _buildShareScreen(
    BuildContext context,
    AppLocalizations t,
    bool active,
  ) {
    final info = _capture;
    final supported = _captureSupported;
    final reason = switch (info?.support) {
      null || CaptureSupport_Supported() => null,
      CaptureSupport_Wayland() => t.sendCaptureUnsupportedWayland,
      CaptureSupport_NoDisplay() => t.sendCaptureNoDisplay,
      CaptureSupport_UnsupportedOs() => t.sendCaptureUnsupportedSystem,
      CaptureSupport_Failed(:final message) => t.sendCaptureFailed(message),
    };
    final monitors = info?.monitors ?? const <CaptureMonitor>[];
    final selected = monitors.any((m) => m.id == _monitorId)
        ? _monitorId
        : (monitors.isEmpty ? null : monitors.first.id);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          key: const Key('shareScreenSwitch'),
          contentPadding: EdgeInsets.zero,
          title: Text(t.sendShareScreen),
          subtitle: Text(
            _androidMode ? t.sendShareScreenHintAndroid : t.sendShareScreenHint,
          ),
          value: _shareScreen && supported,
          onChanged: (supported && !active)
              ? (v) => setState(() => _shareScreen = v)
              : null,
        ),
        if (info == null)
          Text(t.sendCaptureChecking, key: const Key('captureChecking'))
        else if (reason != null)
          NoticeBox(noticeKey: const Key('captureUnsupported'), text: reason),
        if (_shareScreen &&
            supported &&
            monitors.isNotEmpty &&
            !_androidMode) ...[
          const SizedBox(height: 8),
          DropdownButtonFormField<String>(
            key: const Key('monitorDropdown'),
            initialValue: selected,
            decoration: InputDecoration(
              labelText: t.sendMonitorLabel,
              border: const OutlineInputBorder(),
            ),
            items: [
              for (final m in monitors)
                DropdownMenuItem(
                  value: m.id,
                  child: Text(
                    '${m.id == 'root' ? t.sendMonitorWholeDesktop : m.name}'
                    ' (${m.width} x ${m.height})',
                  ),
                ),
            ],
            onChanged: active ? null : (v) => setState(() => _monitorId = v),
          ),
        ],
        const SizedBox(height: 16),
      ],
    );
  }

  String _statusText(AppLocalizations t) {
    final c = _controller;
    if (c.error != null) return t.statusError(c.error!);
    if (c.awaitingConsent) return t.sendCaptureWaitingForConsent;
    if (c.captureDenied) return t.sendCaptureDenied;
    if (c.captureStopped) return t.sendCaptureStopped;
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
        if (c.framesSent > 0) {
          return c.sharingScreen
              ? t.sendStreamingScreen(c.framesSent)
              : t.sendStreaming(c.framesSent);
        }
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
