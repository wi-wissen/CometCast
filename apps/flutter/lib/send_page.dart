// SPDX-License-Identifier: GPL-3.0-or-later
// "Send": enter the receiver's room code and send the built-in VP8 test video, or switch on
// "Share this screen" to send the screen of this computer (Linux/X11 only for now; the Rust
// core says whether that works on this system). On Android the same switch starts the native
// screen capture (consent dialog, MediaCodec) and sends what it encodes, see capture_feeder.dart.
// On iOS it opens the system broadcast sheet; the ReplayKit extension sends (docs/ios.md).

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'codec_controller.dart';
import 'codec_notice.dart';
import 'core/android_capture.dart';
import 'core/core_api.dart';
import 'core/ios_broadcast.dart';
import 'discovery.dart';
import 'join_state.dart';
import 'join_widgets.dart';
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
    this.iosBroadcast,
  });

  final CoreApi core;
  final AppSettings settings;

  /// The Android screen capture; null where there is none.
  final AndroidCapture? androidCapture;

  /// The iOS broadcast hand-over; null where there is none.
  final IosBroadcast? iosBroadcast;

  @override
  State<SendPage> createState() => _SendPageState();
}

class _SendPageState extends State<SendPage> {
  late final SenderController _controller = SenderController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
    capture: widget.androidCapture,
    iosBroadcast: widget.iosBroadcast,
  );
  late final CodecController _codec = CodecController(
    core: widget.core,
    settings: widget.settings,
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

  /// Receivers announced in the local network; null where announcements must not be used
  /// (a policy locks the server or forbids them).
  NearbyReceivers? _nearby;

  bool get _nearbyAllowed =>
      !widget.settings.locked && widget.settings.announcementsAllowed;

  /// The lock or the ban on announcements can arrive late (the network answer): the list is
  /// then hidden and the listening stops.
  void _syncNearby() {
    if (!mounted) return;
    if (_nearbyAllowed && _nearby == null) {
      setState(() => _nearby = NearbyReceivers(widget.core)..start());
    } else if (!_nearbyAllowed && _nearby != null) {
      final old = _nearby!;
      setState(() => _nearby = null);
      old.dispose();
    }
  }

  @override
  void initState() {
    super.initState();
    widget.settings.discover(widget.core);
    if (_nearbyAllowed) {
      _nearby = NearbyReceivers(widget.core)..start();
    }
    widget.settings.addListener(_syncNearby);
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

  /// The core listed the iOS broadcast (it does so only when built for iOS): the screen is
  /// sent by the broadcast extension, not by this app.
  bool get _iosMode =>
      widget.iosBroadcast != null &&
      (_capture?.monitors.any((m) => m.id == iosBroadcastMonitorId) ?? false);

  /// The screen is captured and encoded by native code, so neither the OpenH264 card nor the
  /// monitor list apply.
  bool get _nativeCapture => _androidMode || _iosMode;

  @override
  void dispose() {
    widget.settings.removeListener(_syncNearby);
    widget.settings.cancelDiscovery();
    _nearby?.dispose();
    _codec.dispose();
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
    // Desktop screen sharing encodes with OpenH264, which release builds download first (the
    // card under the switch explains it). Android encodes with MediaCodec.
    if (sharing && !_nativeCapture && !_codec.ready) return;
    if (_controller.cooldownLeft > 0) return;
    widget.settings.rememberServer();
    final name = widget.settings.displayName;
    if (sharing && _iosMode) {
      _controller.startBroadcast(
        code: check.code,
        server: widget.settings.serverAddress.trim(),
      );
      return;
    }
    if (sharing && _androidMode) {
      final t = AppLocalizations.of(context);
      _controller.startCapture(
        code: check.code,
        server: widget.settings.serverAddress.trim(),
        notification: CaptureNotificationTexts(
          title: t.captureNotificationTitle,
          text: t.captureNotificationText,
          stopLabel: t.captureNotificationStop,
        ),
        name: name,
      );
      return;
    }
    _controller.start(
      code: check.code,
      server: widget.settings.serverAddress.trim(),
      name: name,
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
                if (_nearby != null) _buildNearby(context, t, active),
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
                    UpperCaseFormatter(),
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
                DisplayNameField(settings: widget.settings, enabled: !active),
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
                          onPressed: _controller.cooldownLeft > 0
                              ? null
                              : _start,
                          icon: const Icon(Icons.play_arrow),
                          label: Text(t.startSending),
                        ),
                ),
                const SizedBox(height: 24),
                if (_controller.approval != null &&
                    _controller.phase == SenderPhase.waitingApproval) ...[
                  ApprovalWaitingCard(
                    wait: _controller.approval!,
                    onCancel: _controller.stop,
                  ),
                  const SizedBox(height: 16),
                ],
                // liveRegion: screen readers announce changes of the status.
                Semantics(
                  liveRegion: true,
                  child: Text(
                    _statusText(t),
                    key: const Key('statusText'),
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                ),
                if (_controller.refusal == JoinRefusal.denied)
                  RefusalNote(
                    cooldownLeft: () => _controller.cooldownLeft,
                    onElapsed: () => setState(() {}),
                  ),
                if (_controller.awaitingBroadcast)
                  NoticeBox(
                    noticeKey: const Key('iosBroadcastInstructions'),
                    text: _controller.broadcastPickerFailed
                        ? t.iosBroadcastPickerFailed
                        : t.iosBroadcastInstructions,
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

  /// "Nearby receivers": name and announced server of the displays in this network. A tap
  /// takes over the server (if one was announced) and moves to the code field: the room code
  /// is never announced, so the person types it.
  Widget _buildNearby(BuildContext context, AppLocalizations t, bool active) {
    final nearby = _nearby!;
    return ListenableBuilder(
      listenable: nearby,
      builder: (context, _) {
        final list = nearby.receivers;
        return Padding(
          padding: const EdgeInsets.only(top: 16),
          child: Column(
            key: const Key('nearbyReceivers'),
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                t.nearbyTitle,
                style: Theme.of(context).textTheme.titleSmall,
              ),
              if (nearby.unavailable)
                Text(
                  t.nearbyUnavailable,
                  key: const Key('nearbyUnavailable'),
                  style: Theme.of(context).textTheme.bodySmall,
                )
              else if (list.isEmpty)
                Text(
                  t.nearbyEmpty,
                  key: const Key('nearbyEmpty'),
                  style: Theme.of(context).textTheme.bodySmall,
                )
              else ...[
                for (final r in list)
                  ListTile(
                    key: Key('nearbyReceiver:${r.id}'),
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                    leading: const Icon(Icons.tv),
                    title: Text(r.name),
                    isThreeLine: r.server != null,
                    subtitle: r.server == null
                        ? Text(t.nearbyNoServer)
                        : Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(t.nearbyServer(r.server!)),
                              // Anybody in the network can announce anything.
                              Text(
                                t.serverSourceAnnounced,
                                key: Key('nearbyUnverified:${r.id}'),
                              ),
                            ],
                          ),
                    enabled: !active,
                    onTap: () {
                      final server = r.server;
                      if (server != null) {
                        widget.settings.chooseSuggestion(
                          ServerSuggestion(server, ServerSourceKind.announced),
                        );
                      }
                      _codeFocus.requestFocus();
                    },
                  ),
                if (nearby.truncated)
                  Text(
                    t.nearbyMore,
                    key: const Key('nearbyMore'),
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                Text(
                  t.nearbyHint,
                  key: const Key('nearbyHint'),
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ],
          ),
        );
      },
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
            _iosMode
                ? t.sendShareScreenHintIos
                : (_androidMode
                      ? t.sendShareScreenHintAndroid
                      : t.sendShareScreenHint),
          ),
          value: _shareScreen && supported,
          onChanged: (supported && !active)
              ? (v) {
                  setState(() => _shareScreen = v);
                  if (v && !_nativeCapture) _codec.init();
                }
              : null,
        ),
        if (info == null)
          Text(t.sendCaptureChecking, key: const Key('captureChecking'))
        else if (reason != null)
          NoticeBox(noticeKey: const Key('captureUnsupported'), text: reason),
        if (_shareScreen && supported && !_nativeCapture)
          CodecNotice(controller: _codec),
        if (_shareScreen &&
            supported &&
            monitors.isNotEmpty &&
            !_nativeCapture) ...[
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
    if (c.broadcastError != null) {
      return _broadcastErrorText(t, c.broadcastError!);
    }
    if (c.awaitingBroadcast) return t.iosBroadcastWaitingForStart;
    if (c.broadcastNotStarted) return t.iosBroadcastNotStarted;
    if (c.awaitingConsent) return t.sendCaptureWaitingForConsent;
    if (c.captureDenied) return t.sendCaptureDenied;
    if (c.captureStopped) return t.sendCaptureStopped;
    if (c.receiverLeft) return t.statusPeerLeftReceiver;
    final refusal = c.refusal;
    if (refusal != null) return joinRefusalText(t, refusal);
    switch (c.phase) {
      case SenderPhase.idle:
        return t.statusIdle;
      case SenderPhase.starting:
        if (c.broadcastStatus?.state == IosBroadcastState.waiting) {
          return t.iosBroadcastWaitingForReceiver;
        }
        return t.statusConnecting;
      case SenderPhase.waitingApproval:
        return t.sendWaitingStatus;
      case SenderPhase.finished:
        return t.sendFinished(c.framesSent);
      case SenderPhase.ended:
        return t.statusEnded;
      case SenderPhase.running:
        switch (c.broadcastStatus?.state) {
          case IosBroadcastState.reconnecting:
            return t.iosBroadcastReconnecting;
          case IosBroadcastState.paused:
            return t.iosBroadcastPaused;
          default:
            break;
        }
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

/// The translated sentence for a failed or lost iOS broadcast.
String _broadcastErrorText(AppLocalizations t, IosBroadcastError e) =>
    switch (e) {
      IosBroadcastError.none ||
      IosBroadcastError.stoppedByApp => t.sendCaptureStopped,
      IosBroadcastError.peerLeft => t.statusPeerLeftReceiver,
      IosBroadcastError.noConfig => t.iosErrorNoConfig,
      IosBroadcastError.configExpired => t.iosErrorConfigExpired,
      IosBroadcastError.invalidRoomCode => t.iosErrorInvalidRoomCode,
      IosBroadcastError.invalidRelay => t.iosErrorInvalidRelay,
      IosBroadcastError.roomNotFound => t.iosErrorRoomNotFound,
      IosBroadcastError.serverUnreachable => t.iosErrorServerUnreachable,
      IosBroadcastError.connectionLost => t.iosErrorConnectionLost,
      IosBroadcastError.encoderFailed => t.iosErrorEncoderFailed,
      IosBroadcastError.memory => t.iosErrorMemory,
      IosBroadcastError.internalError => t.iosErrorInternal,
    };

/// Turns typed a-z into capitals (room codes are uppercase). Also used by the content screen.
class UpperCaseFormatter extends TextInputFormatter {
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
