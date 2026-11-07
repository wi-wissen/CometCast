// SPDX-License-Identifier: GPL-3.0-or-later
// "Receive Miracast on this PC" (Windows): status, hints, the consent question and the video view,
// built with the presenter kit (tokens, Atkinson, PPage, PField, buttons, icons). Opened from the
// text link `app.miracast` of the first-run page; content and flow are those of docs/miracast.md,
// the consent question is a page in the layout of the relay page (spec 7.2, decision N18).
//
// Privacy: the video view is only built while a sender is connected, and a sender is only
// connected after the person pressed "Allow" in the consent dialog (the Rust side never
// accepts on its own, see docs/miracast.md).
//
// UNCERTAIN: never run against the real receiver (docs/miracast-device-test-plan.md).

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/miracast.dart';
import 'miracast_controller.dart';
import 'src/design/icons.dart';
import 'src/design/spans.dart';
import 'src/display/miracast_view.dart';
import 'src/presenter/kit.dart';

/// Longest receiver name Windows accepts, as far as we know (crates/cc-miracast policy.rs).
// UNCERTAIN: a conservative guess, the Rust side validates too.
const miracastMaxNameChars = 32;

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
    if (mounted) setState(() {});
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

  void _answer(MiracastRequest r, bool accept) =>
      unawaited(_c.respond(r.id, accept));

  @override
  Widget build(BuildContext context) {
    return PresenterFrame(
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              Navigator.of(context).maybePop(),
        },
        child: Material(
          type: MaterialType.transparency,
          child: Builder(
            builder: (context) => _c.pending != null
                ? _consent(context, _c.pending!)
                : _main(context),
          ),
        ),
      ),
    );
  }

  /// The question whether a sender may show its screen: a page in the layout of the relay page.
  /// Focus starts on "Decline": an accidental Enter key press must not share a screen.
  Widget _consent(BuildContext context, MiracastRequest r) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    final pin = r.pin;
    return KeyedSubtree(
      key: const Key('miracastConsentDialog'),
      child: PPage(
        actionsRow: true,
        actions: [
          PSecondaryButton(
            key: const Key('miracastDeny'),
            label: t.miracastConsentDeny,
            autofocus: true,
            onPressed: () => _answer(r, false),
          ),
          PPrimaryButton(
            key: const Key('miracastAllow'),
            label: t.miracastConsentAllow,
            onPressed: () => _answer(r, true),
          ),
        ],
        children: [
          const Align(
            alignment: Alignment.centerLeft,
            child: PIconDisc(icon: CcIconName.screen, tone: PTone.warn),
          ),
          Text(t.miracastConsentTitle, style: s.type.heading),
          Text(
            t.miracastConsentBody(r.name),
            key: const Key('miracastConsentBody'),
            style: s.type.body,
          ),
          // Body text; only the digits are mono 700 (like `presenting.sas`, N39).
          if (pin != null && pin.isNotEmpty)
            Text.rich(
              TextSpan(
                children: valueSpans(
                  t.miracastConsentPin(spanMarker),
                  pin,
                  s.type.host.copyWith(fontSize: 15),
                ),
              ),
              key: const Key('miracastConsentPin'),
              style: s.type.body,
            ),
          Text(t.miracastConsentTimeout, style: s.type.small),
        ],
      ),
    );
  }

  Widget _main(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    final phase = _c.phase;
    final running =
        phase == MiracastPhase.listening ||
        phase == MiracastPhase.consent ||
        phase == MiracastPhase.connected;
    final issue = _c.issue;
    final busy =
        phase == MiracastPhase.starting || phase == MiracastPhase.stopping;
    // Phone: the content scrolls, the actions stay at the bottom; laptop: one row under the
    // content, 24 below the last note (N39, spec 7.4).
    return PPage(
      actionsRow: true,
      pinActions: true,
      actions: [
        if (!running && !busy)
          PPrimaryButton(
            key: const Key('miracastStartButton'),
            autofocus: true,
            label: t.miracastStart,
            onPressed: () {
              _c.displayName = _name.text;
              unawaited(_c.start());
            },
          ),
        if (running || phase == MiracastPhase.starting)
          PSecondaryButton(
            key: const Key('miracastStopButton'),
            label: t.miracastStop,
            onPressed: () => unawaited(_c.stop()),
          ),
        if (phase == MiracastPhase.connected) ...[
          PSecondaryButton(
            key: const Key('miracastDisconnectButton'),
            label: t.miracastDisconnect,
            onPressed: () => unawaited(_c.disconnect()),
          ),
          PSecondaryButton(
            key: const Key('miracastMuteButton'),
            label: _muted ? t.miracastUnmute : t.miracastMute,
            onPressed: () {
              setState(() => _muted = !_muted);
              unawaited(_c.setMuted(_muted));
            },
          ),
        ],
        PSecondaryButton(
          key: const Key('miracastBack'),
          label: t.commonBack,
          onPressed: () => Navigator.of(context).maybePop(),
        ),
      ],
      children: [
        Text(t.miracastTitle, style: s.type.heading),
        Text(t.miracastIntro, style: s.type.body),
        if (_canEdit) ...[
          PField(
            fieldKey: const Key('miracastNameField'),
            label: t.miracastNameLabel,
            controller: _name,
            maxLength: miracastMaxNameChars,
            onChanged: (v) => _c.displayName = v,
          ),
          Column(
            key: const Key('miracastPairingField'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(t.miracastPairingLabel, style: s.type.label),
              const SizedBox(height: 5),
              for (final m in MiracastPairing.values) ...[
                _PairingRow(
                  key: Key('miracastPairing-${m.name}'),
                  text: _pairingText(t, m),
                  selected: _c.pairing == m,
                  onPressed: () => setState(() => _c.pairing = m),
                ),
                const SizedBox(height: 6),
              ],
            ],
          ),
        ],
        PCard(
          key: const Key('miracastStatusCard'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  PIconDisc(
                    icon: phase == MiracastPhase.problem
                        ? CcIconName.x
                        : phase == MiracastPhase.connected
                        ? CcIconName.check
                        : CcIconName.screen,
                    tone: phase == MiracastPhase.problem
                        ? PTone.err
                        : phase == MiracastPhase.connected
                        ? PTone.ok
                        : PTone.acc,
                    size: 40,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      miracastStatusText(t, _c),
                      key: const Key('miracastStatusText'),
                      style: s.type.optionTitle,
                    ),
                  ),
                ],
              ),
              if (issue != null) ...[
                const SizedBox(height: 12),
                Text(
                  miracastIssueText(t, issue),
                  key: const Key('miracastHint'),
                  style: s.type.body,
                ),
              ],
              if (_c.state.rejected > 0) ...[
                const SizedBox(height: 12),
                Text(
                  t.miracastRejected(_c.state.rejected),
                  key: const Key('miracastRejectedNote'),
                  style: s.type.body,
                ),
              ],
              if (_c.state.lastError != null &&
                  (phase == MiracastPhase.problem || issue != null)) ...[
                const SizedBox(height: 12),
                Text(
                  t.miracastLastMessage(_c.state.lastError!),
                  key: const Key('miracastLastMessage'),
                  style: s.type.small,
                ),
              ],
            ],
          ),
        ),
        if (phase == MiracastPhase.connected) ...[
          MiracastVideoView(
            key: const Key('miracastVideo'),
            controller: _c,
            decoder: widget.decoder,
          ),
          Text(
            t.miracastPictureStats(
              _c.state.frames.width,
              _c.state.frames.height,
              _c.state.frames.dropped,
            ),
            key: const Key('miracastStats'),
            style: s.type.small,
          ),
        ],
        Text(t.miracastPrivacyNote, style: s.type.small),
        Text(t.miracastFirewallNote, style: s.type.small),
        Text(t.miracastLimitsNote, style: s.type.small),
      ],
    );
  }
}

/// One choice of the pairing method: an option row with the selected border in `p-primary`.
class _PairingRow extends StatelessWidget {
  const _PairingRow({
    super.key,
    required this.text,
    required this.selected,
    required this.onPressed,
  });
  final String text;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    return PTap(
      onPressed: onPressed,
      radius: 10,
      label: text,
      builder: (_) => Container(
        constraints: const BoxConstraints(minHeight: 48),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: p.surface,
          border: Border.all(
            width: selected ? 2 : 1,
            color: selected ? p.primary : p.borderInput,
          ),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          children: [
            if (selected) ...[
              CcIcon(CcIconName.check, color: p.primary, size: 18),
              const SizedBox(width: 8),
            ],
            Expanded(
              child: Text(
                text,
                style: s.type.body.copyWith(
                  color: p.fg,
                  fontWeight: selected ? FontWeight.w700 : FontWeight.w400,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
