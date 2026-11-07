// SPDX-License-Identifier: GPL-3.0-or-later
// "Receive via Miracast (Windows)": status card, hints, consent dialog and the video view.
// Shown only on Windows (home_page.dart decides). All texts come from the l10n files.
//
// Privacy: the video view is only built while a sender is connected, and a sender is only
// connected after the person pressed "Allow" in the consent dialog (the Rust side never
// accepts on its own, see docs/miracast.md).
//
// UNCERTAIN: never run against the real receiver (docs/miracast-device-test-plan.md).

import 'dart:async';
import 'dart:io' show Platform;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/miracast.dart';
import 'miracast_controller.dart';
import 'widgets.dart';

/// Longest receiver name Windows accepts, as far as we know (crates/cc-miracast policy.rs).
// UNCERTAIN: a conservative guess, the Rust side validates too.
const miracastMaxNameChars = 32;

/// Turns a picture into a [ui.Image] (replaced in tests).
typedef FrameDecoder = Future<ui.Image> Function(MiracastFrame frame);

/// The real [FrameDecoder]: BGRA bytes to an image.
Future<ui.Image> decodeMiracastFrame(MiracastFrame f) {
  final c = Completer<ui.Image>();
  ui.decodeImageFromPixels(
    f.bgra,
    f.width,
    f.height,
    ui.PixelFormat.bgra8888,
    c.complete,
  );
  return c.future;
}

/// The name this PC announces: the host name, cut to the length limit.
String defaultMiracastName() {
  var name = 'Comet Cast';
  try {
    final h = Platform.localHostname.trim();
    if (h.isNotEmpty) name = h;
  } catch (_) {
    // Not available: use the product name.
  }
  return name.characters.take(miracastMaxNameChars).toString();
}

/// Is the Miracast entry offered on this platform? Only Windows (the receiver API exists
/// nowhere else). The home page asks this; tests inject their own value.
bool miracastSupportedHere() {
  try {
    return Platform.isWindows;
  } catch (_) {
    return false;
  }
}

/// Translated text of a problem.
String miracastIssueText(AppLocalizations t, MiracastIssue issue) {
  switch (issue) {
    case MiracastIssue.notWindows:
      return t.miracastHintNotWindows;
    case MiracastIssue.featureMissing:
      return t.miracastHintFeatureMissing;
    case MiracastIssue.adapterUnsupported:
      return t.miracastHintAdapterUnsupported;
    case MiracastIssue.adapterNotOptimized:
      return t.miracastHintAdapterNotOptimized;
    case MiracastIssue.disabledByPolicy:
      return t.miracastHintDisabledByPolicy;
    case MiracastIssue.temporarilyUnavailable:
      return t.miracastHintTemporarilyUnavailable;
    case MiracastIssue.accessDenied:
      return t.miracastHintAccessDenied;
    case MiracastIssue.invalidSettings:
      return t.miracastHintInvalidSettings;
    case MiracastIssue.libraryMissing:
      return t.miracastHintLibraryMissing;
    case MiracastIssue.other:
      return t.miracastHintOther;
  }
}

/// Translated one-line status.
String miracastStatusText(AppLocalizations t, MiracastController c) {
  switch (c.phase) {
    case MiracastPhase.idle:
      return t.miracastStatusOff;
    case MiracastPhase.starting:
      return t.miracastStatusStarting;
    case MiracastPhase.stopping:
      return t.miracastStatusStopping;
    case MiracastPhase.listening:
      return t.miracastStatusListening(c.displayName.trim());
    case MiracastPhase.consent:
      return t.miracastStatusConsent;
    case MiracastPhase.connected:
      return t.miracastStatusConnected(c.connectedName ?? '');
    case MiracastPhase.problem:
      return t.miracastStatusProblem;
  }
}

String _pairingText(AppLocalizations t, MiracastPairing p) {
  switch (p) {
    case MiracastPairing.confirm:
      return t.miracastPairingConfirm;
    case MiracastPairing.pinIfRequested:
      return t.miracastPairingPinIfRequested;
    case MiracastPairing.pinRequired:
      return t.miracastPairingPinRequired;
  }
}

class MiracastPage extends StatefulWidget {
  const MiracastPage({
    super.key,
    required this.core,
    this.initialName,
    this.decoder = decodeMiracastFrame,
    this.pollInterval = const Duration(milliseconds: 16),
  });

  /// The receiver (the DLL, or a fake in tests).
  final MiracastCore core;

  /// Receiver name to start with; defaults to the host name.
  final String? initialName;
  final FrameDecoder decoder;
  final Duration pollInterval;

  @override
  State<MiracastPage> createState() => _MiracastPageState();
}

class _MiracastPageState extends State<MiracastPage> {
  late final MiracastController _c;
  late final TextEditingController _name;
  int? _dialogForId;
  bool _muted = false;

  @override
  void initState() {
    super.initState();
    final initial = widget.initialName ?? defaultMiracastName();
    _name = TextEditingController(text: initial);
    _c = MiracastController(
      core: widget.core,
      displayName: initial,
      pollInterval: widget.pollInterval,
    )..addListener(_onChange);
    unawaited(_c.init());
  }

  void _onChange() {
    if (!mounted) return;
    setState(() {});
    final p = _c.pending;
    if (p == null) {
      _dialogForId = null;
      return;
    }
    if (_dialogForId == p.id) return;
    _dialogForId = p.id;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _c.pending?.id != p.id) return;
      unawaited(
        showDialog<void>(
          context: context,
          barrierDismissible: false,
          builder: (_) => MiracastConsentDialog(controller: _c, request: p),
        ),
      );
    });
  }

  @override
  void dispose() {
    _c.removeListener(_onChange);
    _c.dispose(); // also stops receiving
    _name.dispose();
    super.dispose();
  }

  bool get _canEdit =>
      _c.phase == MiracastPhase.idle || _c.phase == MiracastPhase.problem;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    final phase = _c.phase;
    final running = phase == MiracastPhase.listening ||
        phase == MiracastPhase.consent ||
        phase == MiracastPhase.connected;
    final issue = _c.issue;
    return EscapeToGoBack(
      child: Scaffold(
        appBar: AppBar(title: Text(t.miracastTitle)),
        body: PageBody(
          children: [
            NoticeBox(text: t.miracastIntro),
            const SizedBox(height: 16),
            if (_canEdit) ...[
              TextField(
                key: const Key('miracastNameField'),
                controller: _name,
                maxLength: miracastMaxNameChars,
                inputFormatters: [
                  LengthLimitingTextInputFormatter(miracastMaxNameChars),
                ],
                decoration: InputDecoration(labelText: t.miracastNameLabel),
                onChanged: (v) => _c.displayName = v,
              ),
              const SizedBox(height: 8),
              DropdownMenu<MiracastPairing>(
                key: const Key('miracastPairingField'),
                width: 520,
                label: Text(t.miracastPairingLabel),
                initialSelection: _c.pairing,
                requestFocusOnTap: false,
                dropdownMenuEntries: [
                  for (final p in MiracastPairing.values)
                    DropdownMenuEntry(value: p, label: _pairingText(t, p)),
                ],
                onSelected: (p) {
                  if (p != null) _c.pairing = p;
                },
              ),
              const SizedBox(height: 16),
            ],
            Card(
              key: const Key('miracastStatusCard'),
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Icon(
                          phase == MiracastPhase.problem
                              ? Icons.error_outline
                              : (phase == MiracastPhase.connected
                                    ? Icons.cast_connected
                                    : Icons.cast),
                          color: phase == MiracastPhase.problem
                              ? scheme.error
                              : scheme.primary,
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            miracastStatusText(t, _c),
                            key: const Key('miracastStatusText'),
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                        ),
                      ],
                    ),
                    if (issue != null) ...[
                      const SizedBox(height: 12),
                      Text(
                        miracastIssueText(t, issue),
                        key: const Key('miracastHint'),
                      ),
                    ],
                    if (_c.state.rejected > 0) ...[
                      const SizedBox(height: 12),
                      Text(
                        t.miracastRejected(_c.state.rejected),
                        key: const Key('miracastRejectedNote'),
                      ),
                    ],
                    if (_c.state.lastError != null &&
                        (phase == MiracastPhase.problem || issue != null)) ...[
                      const SizedBox(height: 12),
                      Text(
                        t.miracastLastMessage(_c.state.lastError!),
                        key: const Key('miracastLastMessage'),
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            Wrap(
              spacing: 12,
              runSpacing: 8,
              children: [
                if (!running &&
                    phase != MiracastPhase.starting &&
                    phase != MiracastPhase.stopping)
                  FilledButton.icon(
                    key: const Key('miracastStartButton'),
                    autofocus: true,
                    onPressed: () {
                      _c.displayName = _name.text;
                      unawaited(_c.start());
                    },
                    icon: const Icon(Icons.play_arrow),
                    label: Text(t.miracastStart),
                  ),
                if (running || phase == MiracastPhase.starting)
                  OutlinedButton.icon(
                    key: const Key('miracastStopButton'),
                    onPressed: () => unawaited(_c.stop()),
                    icon: const Icon(Icons.stop),
                    label: Text(t.miracastStop),
                  ),
                if (phase == MiracastPhase.connected) ...[
                  OutlinedButton.icon(
                    key: const Key('miracastDisconnectButton'),
                    onPressed: () => unawaited(_c.disconnect()),
                    icon: const Icon(Icons.link_off),
                    label: Text(t.miracastDisconnect),
                  ),
                  OutlinedButton.icon(
                    key: const Key('miracastMuteButton'),
                    onPressed: () {
                      setState(() => _muted = !_muted);
                      unawaited(_c.setMuted(_muted));
                    },
                    icon: Icon(_muted ? Icons.volume_off : Icons.volume_up),
                    label: Text(_muted ? t.miracastUnmute : t.miracastMute),
                  ),
                ],
              ],
            ),
            if (phase == MiracastPhase.connected) ...[
              const SizedBox(height: 16),
              MiracastVideoView(
                key: const Key('miracastVideo'),
                controller: _c,
                decoder: widget.decoder,
              ),
              const SizedBox(height: 8),
              Text(
                t.miracastPictureStats(
                  _c.state.frames.width,
                  _c.state.frames.height,
                  _c.state.frames.dropped,
                ),
                key: const Key('miracastStats'),
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
            const SizedBox(height: 16),
            NoticeBox(text: t.miracastPrivacyNote),
            const SizedBox(height: 8),
            NoticeBox(text: t.miracastFirewallNote),
            const SizedBox(height: 8),
            NoticeBox(text: t.miracastLimitsNote),
          ],
        ),
      ),
    );
  }
}

/// Asks the person whether a sender may show its screen. Closes itself when the request
/// goes away (timeout on the Rust side, sender left).
class MiracastConsentDialog extends StatefulWidget {
  const MiracastConsentDialog({
    super.key,
    required this.controller,
    required this.request,
  });

  final MiracastController controller;
  final MiracastRequest request;

  @override
  State<MiracastConsentDialog> createState() => _MiracastConsentDialogState();
}

class _MiracastConsentDialogState extends State<MiracastConsentDialog> {
  bool _closing = false;

  void _close() {
    if (_closing) return;
    _closing = true;
    Navigator.of(context).pop();
  }

  void _answer(bool accept) {
    _close();
    unawaited(widget.controller.respond(widget.request.id, accept));
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return ListenableBuilder(
      listenable: widget.controller,
      builder: (context, _) {
        if (widget.controller.pending?.id != widget.request.id && !_closing) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) _close();
          });
        }
        final pin = widget.request.pin;
        return AlertDialog(
          key: const Key('miracastConsentDialog'),
          title: Text(t.miracastConsentTitle),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                t.miracastConsentBody(widget.request.name),
                key: const Key('miracastConsentBody'),
              ),
              if (pin != null && pin.isNotEmpty) ...[
                const SizedBox(height: 12),
                Text(
                  t.miracastConsentPin(pin),
                  key: const Key('miracastConsentPin'),
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ],
              const SizedBox(height: 12),
              Text(
                t.miracastConsentTimeout,
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
          actions: [
            // Focus starts on "Decline": an accidental Enter key press must not share a screen.
            TextButton(
              key: const Key('miracastDeny'),
              autofocus: true,
              onPressed: () => _answer(false),
              child: Text(t.miracastConsentDeny),
            ),
            FilledButton(
              key: const Key('miracastAllow'),
              onPressed: () => _answer(true),
              child: Text(t.miracastConsentAllow),
            ),
          ],
        );
      },
    );
  }
}

/// The received picture. Decodes each new picture of the controller and draws the newest.
class MiracastVideoView extends StatefulWidget {
  const MiracastVideoView({
    super.key,
    required this.controller,
    this.decoder = decodeMiracastFrame,
  });

  final MiracastController controller;
  final FrameDecoder decoder;

  @override
  State<MiracastVideoView> createState() => _MiracastVideoViewState();
}

class _MiracastVideoViewState extends State<MiracastVideoView> {
  ui.Image? _image;
  int _lastSeq = -1;
  bool _decoding = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onChange);
    _onChange();
  }

  @override
  void didUpdateWidget(MiracastVideoView old) {
    super.didUpdateWidget(old);
    if (old.controller != widget.controller) {
      old.controller.removeListener(_onChange);
      widget.controller.addListener(_onChange);
    }
  }

  void _onChange() {
    final f = widget.controller.frame;
    if (f == null) {
      // Not connected any more: never keep showing the last picture.
      if (_image != null || _lastSeq != -1) {
        _lastSeq = -1;
        if (mounted) {
          setState(() {
            _image?.dispose();
            _image = null;
          });
        }
      }
      return;
    }
    if (_decoding || f.seq == _lastSeq) return;
    _decoding = true;
    _lastSeq = f.seq;
    widget.decoder(f).then((img) {
      _decoding = false;
      if (!mounted) {
        img.dispose();
        return;
      }
      setState(() {
        final old = _image;
        _image = img;
        old?.dispose();
      });
      // A newer picture may have arrived while decoding.
      _onChange();
    }, onError: (Object _) {
      _decoding = false;
    });
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onChange);
    _image?.dispose();
    _image = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final img = _image;
    final ratio = img == null || img.height == 0
        ? 16 / 9
        : img.width / img.height;
    return Semantics(
      label: t.miracastVideoLabel(widget.controller.connectedName ?? ''),
      image: true,
      child: AspectRatio(
        aspectRatio: ratio,
        child: ColoredBox(
          color: Colors.black,
          child: img == null
              ? const SizedBox.expand()
              : RawImage(image: img, fit: BoxFit.contain),
        ),
      ),
    );
  }
}
