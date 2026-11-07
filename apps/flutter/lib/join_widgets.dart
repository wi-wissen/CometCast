// SPDX-License-Identifier: GPL-3.0-or-later
// Join approval on the screens: the host's panel (who may join, who asks, who is connected), the
// hold-to-allow button, the corner chip while somebody presents, and the sender's waiting card.
//
// Rules (docs/protocol.md, "Join approval"), also for a TV remote (D-pad, no touch, no mouse):
// * a new request never takes the focus;
// * "Allow" needs a deliberate hold (about a second; OK held on a remote, press and hold with a
//   pointer); a short press only shows a hint;
// * "Deny" is one press; after a decision the focus moves to the next request's "Deny", never
//   to "Allow", and to the mode choice when nothing is left;
// * Back / Escape closes the panel without allowing anything;
// * big hit areas and a clearly visible focus frame on a TV.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/core_api.dart';
import 'join_state.dart';
import 'receiver_controller.dart';
import 'settings.dart';
import 'widgets.dart';

/// How long "Allow" must be held.
const holdToAllowDuration = Duration(milliseconds: 1000);

/// Requests shown at once; the rest is counted ("+n more waiting").
const maxShownRequests = 3;

/// Shortens a name for a narrow card. Plain text only; the full name is a tooltip.
String shortJoinName(String name, {int max = 18}) {
  final runes = name.runes.toList();
  if (runes.length <= max) return name;
  return '${String.fromCharCodes(runes.take(max))}…';
}

String joinModeLabel(AppLocalizations t, UiJoinMode m) => switch (m) {
  UiJoinMode.open => t.joinModeOpen,
  UiJoinMode.confirm => t.joinModeConfirm,
  UiJoinMode.locked => t.joinModeLocked,
};

String joinModeHelp(AppLocalizations t, UiJoinMode m) => switch (m) {
  UiJoinMode.open => t.joinModeOpenHelp,
  UiJoinMode.confirm => t.joinModeConfirmHelp,
  UiJoinMode.locked => t.joinModeLockedHelp,
};

String joinRefusalText(AppLocalizations t, JoinRefusal r) => switch (r) {
  JoinRefusal.denied => t.joinRefusalDenied,
  JoinRefusal.locked => t.joinRefusalLocked,
  JoinRefusal.lockedWhileWaiting => t.joinRefusalLockedWaiting,
  JoinRefusal.timedOut => t.joinRefusalTimedOut,
  JoinRefusal.removed => t.joinRefusalRemoved,
};

/// A button that acts only after it was held for [holdToAllowDuration]: OK / Enter / Space held
/// on a keyboard or remote, or a pressed pointer. A shorter press only shows [hint].
class HoldToAllowButton extends StatefulWidget {
  const HoldToAllowButton({
    super.key,
    required this.label,
    required this.holdingLabel,
    required this.hint,
    required this.onConfirmed,
    this.semanticsLabel,
    this.tv = false,
  });

  final String label;
  final String holdingLabel;
  final String hint;
  final VoidCallback onConfirmed;
  final String? semanticsLabel;
  final bool tv;

  @override
  State<HoldToAllowButton> createState() => _HoldToAllowButtonState();
}

class _HoldToAllowButtonState extends State<HoldToAllowButton>
    with SingleTickerProviderStateMixin {
  late final AnimationController _progress;
  late final FocusNode _focus;
  Timer? _hintTimer;
  bool _hint = false;

  @override
  void initState() {
    super.initState();
    _progress = AnimationController(vsync: this, duration: holdToAllowDuration)
      ..addStatusListener((s) {
        if (s == AnimationStatus.completed) {
          _progress.reset();
          widget.onConfirmed();
        }
      });
    // The node's own handler runs before the button's "activate" shortcut, so Enter / OK is
    // ours: down starts the hold, up ends it.
    _focus = FocusNode(debugLabel: 'hold-to-allow', onKeyEvent: _onKey);
  }

  static bool _isSelectKey(LogicalKeyboardKey k) =>
      k == LogicalKeyboardKey.select ||
      k == LogicalKeyboardKey.enter ||
      k == LogicalKeyboardKey.numpadEnter ||
      k == LogicalKeyboardKey.space ||
      k == LogicalKeyboardKey.gameButtonA;

  KeyEventResult _onKey(FocusNode node, KeyEvent e) {
    if (!_isSelectKey(e.logicalKey)) return KeyEventResult.ignored;
    if (e is KeyDownEvent) {
      _begin();
    } else if (e is KeyUpEvent) {
      _end();
    }
    // Key repeats are swallowed: holding must not restart the hold.
    return KeyEventResult.handled;
  }

  void _begin() {
    if (_progress.isAnimating) return;
    setState(() => _hint = false);
    _hintTimer?.cancel();
    _progress.forward(from: 0);
  }

  void _end() {
    if (!_progress.isAnimating) return;
    _progress.reset();
    setState(() => _hint = true);
    _hintTimer?.cancel();
    _hintTimer = Timer(const Duration(milliseconds: 2500), () {
      if (mounted) setState(() => _hint = false);
    });
  }

  @override
  void dispose() {
    _hintTimer?.cancel();
    _progress.dispose();
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        FocusRing(
          child: Semantics(
            label: widget.semanticsLabel,
            button: true,
            child: Listener(
              onPointerDown: (_) => _begin(),
              onPointerUp: (_) => _end(),
              onPointerCancel: (_) => _end(),
              child: FilledButton(
                key: const Key('allowButton'),
                focusNode: _focus,
                // A tap does nothing by itself; only a hold counts (see the Listener).
                onPressed: () {},
                style: FilledButton.styleFrom(
                  minimumSize: Size(widget.tv ? 200 : 120, widget.tv ? 64 : 48),
                  textStyle: widget.tv
                      ? theme.textTheme.titleLarge
                      : theme.textTheme.titleMedium,
                ),
                child: AnimatedBuilder(
                  animation: _progress,
                  builder: (context, _) => Stack(
                    alignment: Alignment.center,
                    children: [
                      Positioned.fill(
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: FractionallySizedBox(
                            widthFactor: _progress.value,
                            child: ColoredBox(
                              key: const Key('holdBar'),
                              color: theme.colorScheme.onPrimary.withValues(
                                alpha: 0.35,
                              ),
                              child: const SizedBox.expand(),
                            ),
                          ),
                        ),
                      ),
                      Text(
                        _progress.isAnimating
                            ? widget.holdingLabel
                            : widget.label,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
        if (_hint)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              widget.hint,
              key: const Key('holdHint'),
              style: theme.textTheme.bodySmall,
            ),
          ),
      ],
    );
  }
}

/// The host's panel: who may join, who asks, who is connected.
class JoinHostPanel extends StatefulWidget {
  const JoinHostPanel({
    super.key,
    required this.controller,
    required this.settings,
    this.tv = false,
    this.autofocusMode = false,
    this.onClose,
  });

  final ReceiverController controller;
  final AppSettings settings;
  final bool tv;

  /// The selected mode button takes the focus when the panel opens (the dialog). Otherwise
  /// nothing grabs the focus.
  final bool autofocusMode;

  /// Shown as a "Hide" button when the panel is in a dialog.
  final VoidCallback? onClose;

  @override
  State<JoinHostPanel> createState() => _JoinHostPanelState();
}

class _JoinHostPanelState extends State<JoinHostPanel> {
  final _modeNodes = {
    for (final m in UiJoinMode.values)
      m: FocusNode(debugLabel: 'mode-${m.name}'),
  };
  final _denyNodes = <BigInt, FocusNode>{};
  Timer? _tick;
  String _announce = '';
  int _lastRequestCount = 0;

  ReceiverController get c => widget.controller;

  @override
  void initState() {
    super.initState();
    _lastRequestCount = c.requests.length;
    c.addListener(_onController);
    _tick = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted && c.requests.isNotEmpty) setState(() {});
    });
  }

  void _onController() {
    // A new request is announced to screen readers; it never moves the focus.
    if (c.requests.length > _lastRequestCount && c.requests.isNotEmpty) {
      final r = c.requests.last;
      final t = AppLocalizations.of(context);
      setState(
        () => _announce = t.joinAnnounce(shortJoinName(r.name), r.check),
      );
    }
    _lastRequestCount = c.requests.length;
  }

  @override
  void dispose() {
    c.removeListener(_onController);
    _tick?.cancel();
    for (final n in _modeNodes.values) {
      n.dispose();
    }
    for (final n in _denyNodes.values) {
      n.dispose();
    }
    super.dispose();
  }

  FocusNode _denyNode(BigInt id) =>
      _denyNodes.putIfAbsent(id, () => FocusNode(debugLabel: 'deny-$id'));

  void _afterDecision() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final shown = c.requests.take(maxShownRequests).toList();
      if (shown.isNotEmpty) {
        _denyNode(shown.first.id).requestFocus();
      } else {
        _modeNodes[c.joinMode]?.requestFocus();
      }
    });
  }

  void _setMode(UiJoinMode m) {
    widget.settings.setJoinMode(m);
    c.setJoinMode(m);
  }

  int _secondsLeft(JoinRequestView r) {
    final left =
        joinRequestSeconds - joinClock().difference(r.receivedAt).inSeconds;
    return left < 0 ? 0 : left;
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final theme = Theme.of(context);
    return ListenableBuilder(
      listenable: c,
      builder: (context, _) {
        final shown = c.requests.take(maxShownRequests).toList();
        final more = c.requests.length - shown.length;
        // Forget the focus nodes of requests that are gone.
        _denyNodes.removeWhere((id, n) {
          final gone = c.requests.every((r) => r.id != id);
          if (gone) {
            WidgetsBinding.instance.addPostFrameCallback((_) => n.dispose());
          }
          return gone;
        });
        return Card(
          key: const Key('joinPanel'),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        t.joinPanelTitle,
                        style: theme.textTheme.titleMedium,
                      ),
                    ),
                    if (widget.onClose != null)
                      OutlinedButton(
                        key: const Key('joinCloseButton'),
                        onPressed: widget.onClose,
                        child: Text(t.joinClosePanel),
                      ),
                  ],
                ),
                // Screen readers announce a new request here; the focus is not moved.
                Semantics(
                  key: const Key('joinAnnounce'),
                  liveRegion: true,
                  label: _announce,
                  child: const SizedBox.shrink(),
                ),
                if (c.joinNote != null) _note(context, t),
                const SizedBox(height: 8),
                Semantics(
                  label: t.joinModeLabel,
                  child: Row(
                    children: [
                      for (final m in UiJoinMode.values) ...[
                        Expanded(child: _modeButton(context, t, m)),
                        if (m != UiJoinMode.locked) const SizedBox(width: 8),
                      ],
                    ],
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  joinModeHelp(t, c.joinMode),
                  key: const Key('joinModeHelp'),
                  style: theme.textTheme.bodyMedium,
                ),
                if (c.joinMode != UiJoinMode.locked) ...[
                  const SizedBox(height: 8),
                  FocusRing(
                    child: OutlinedButton(
                      key: const Key('lockNowButton'),
                      style: OutlinedButton.styleFrom(
                        minimumSize: Size(0, widget.tv ? 60 : 48),
                      ),
                      onPressed: () => _setMode(UiJoinMode.locked),
                      child: Text(t.joinLockNow),
                    ),
                  ),
                ],
                const Divider(height: 24),
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        t.joinRequestsTitle,
                        style: theme.textTheme.titleSmall,
                      ),
                    ),
                    if (c.requests.length >= 2)
                      FocusRing(
                        child: OutlinedButton(
                          key: const Key('denyAllButton'),
                          onPressed: () {
                            c.denyAllJoins();
                            _afterDecision();
                          },
                          child: Text(t.joinDenyAll),
                        ),
                      ),
                  ],
                ),
                if (c.requests.isEmpty)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    child: Text(
                      t.joinNobodyWaiting,
                      key: const Key('joinNobodyWaiting'),
                    ),
                  ),
                for (final r in shown) _requestCard(context, t, r),
                if (c.requests.isNotEmpty)
                  Text(t.joinAllowHelp, style: theme.textTheme.bodySmall),
                if (more > 0)
                  Text(
                    t.joinMoreWaiting(more),
                    key: const Key('joinMore'),
                    style: theme.textTheme.bodyMedium,
                  ),
                const Divider(height: 24),
                Text(t.joinConnectedTitle, style: theme.textTheme.titleSmall),
                if (c.peerName == null)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    child: Text(t.joinNobodyConnected),
                  )
                else
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Row(
                      children: [
                        Expanded(
                          child: _NameBox(
                            key: const Key('peerName'),
                            name: c.peerName!,
                            max: 40,
                          ),
                        ),
                        const SizedBox(width: 8),
                        FocusRing(
                          child: OutlinedButton(
                            key: const Key('removePeerButton'),
                            style: OutlinedButton.styleFrom(
                              minimumSize: Size(0, widget.tv ? 60 : 48),
                            ),
                            onPressed: c.removePeer,
                            child: Text(
                              t.joinRemove,
                              semanticsLabel: t.joinRemoveLabel(
                                shortJoinName(c.peerName!),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _note(BuildContext context, AppLocalizations t) {
    final text = switch (c.joinNote!) {
      JoinNote.lockedDiscarded => t.joinNoteLocked(c.discardedCount),
      JoinNote.openDisabled => t.joinNoteOpenDisabled,
    };
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Row(
        key: const Key('joinNote'),
        children: [
          Expanded(child: Text(text)),
          TextButton(
            key: const Key('joinNoteDismiss'),
            onPressed: c.dismissJoinNote,
            child: Text(t.joinDismiss),
          ),
        ],
      ),
    );
  }

  Widget _modeButton(BuildContext context, AppLocalizations t, UiJoinMode m) {
    final selected = c.joinMode == m;
    final scheme = Theme.of(context).colorScheme;
    return FocusRing(
      child: Semantics(
        selected: selected,
        inMutuallyExclusiveGroup: true,
        button: true,
        child: ElevatedButton(
          key: Key('mode-${m.name}'),
          focusNode: _modeNodes[m],
          autofocus: widget.autofocusMode && selected,
          style: ElevatedButton.styleFrom(
            minimumSize: Size(0, widget.tv ? 72 : 52),
            backgroundColor: selected ? scheme.primary : null,
            foregroundColor: selected ? scheme.onPrimary : null,
            textStyle: widget.tv
                ? Theme.of(context).textTheme.titleLarge
                : Theme.of(context).textTheme.titleMedium,
          ),
          onPressed: () => _setMode(m),
          child: Text(joinModeLabel(t, m)),
        ),
      ),
    );
  }

  Widget _requestCard(
    BuildContext context,
    AppLocalizations t,
    JoinRequestView r,
  ) {
    final theme = Theme.of(context);
    final left = _secondsLeft(r);
    return Card(
      key: Key('request-${r.id}'),
      margin: const EdgeInsets.only(top: 8),
      color: theme.colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(t.joinNameBoxLabel, style: theme.textTheme.bodySmall),
            const SizedBox(height: 4),
            Wrap(
              spacing: 8,
              runSpacing: 4,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                _NameBox(key: Key('requestName-${r.id}'), name: r.name),
                Text(t.joinWantsToPresent),
              ],
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Text(
                  '${r.check}',
                  key: Key('requestCheck-${r.id}'),
                  style: theme.textTheme.headlineMedium?.copyWith(
                    fontFamily: 'monospace',
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    t.joinCheckNumber(r.check),
                    style: theme.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            LinearProgressIndicator(value: left / joinRequestSeconds),
            Text(t.joinSecondsLeft(left), style: theme.textTheme.bodySmall),
            const SizedBox(height: 8),
            // Deny comes first in the focus order; Allow is never the default.
            Wrap(
              spacing: 12,
              runSpacing: 8,
              crossAxisAlignment: WrapCrossAlignment.start,
              children: [
                FocusRing(
                  child: OutlinedButton(
                    key: Key('deny-${r.id}'),
                    focusNode: _denyNode(r.id),
                    style: OutlinedButton.styleFrom(
                      minimumSize: Size(
                        widget.tv ? 200 : 120,
                        widget.tv ? 64 : 48,
                      ),
                      textStyle: widget.tv
                          ? theme.textTheme.titleLarge
                          : theme.textTheme.titleMedium,
                    ),
                    onPressed: () {
                      c.decideJoin(r.id, allow: false);
                      _afterDecision();
                    },
                    child: Text(t.joinDeny),
                  ),
                ),
                HoldToAllowButton(
                  key: Key('allow-${r.id}'),
                  tv: widget.tv,
                  label: t.joinAllow,
                  holdingLabel: t.joinAllowHolding,
                  hint: t.joinAllowHint,
                  semanticsLabel:
                      '${t.joinAllow}: ${shortJoinName(r.name)} (${t.joinAllowHint})',
                  onConfirmed: () {
                    c.decideJoin(r.id, allow: true);
                    _afterDecision();
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

/// A name that somebody else chose, in quotes in a dashed box, shortened. Plain text.
class _NameBox extends StatelessWidget {
  const _NameBox({super.key, required this.name, this.max = 18});

  final String name;
  final int max;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Tooltip(
      message: name,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        decoration: BoxDecoration(
          border: Border.all(color: scheme.outline),
          borderRadius: BorderRadius.circular(6),
        ),
        child: Text(
          '“${shortJoinName(name, max: max)}”',
          style: Theme.of(context).textTheme.titleMedium,
        ),
      ),
    );
  }
}

/// Corner chip (and pill with the number of waiting requests) while somebody presents. Opens
/// the panel in a dialog; Back closes it without allowing anything.
class JoinChip extends StatelessWidget {
  const JoinChip({
    super.key,
    required this.controller,
    required this.settings,
    this.tv = false,
  });

  final ReceiverController controller;
  final AppSettings settings;
  final bool tv;

  Future<void> _open(BuildContext context) => showDialog<void>(
    context: context,
    builder: (dialog) => Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: SingleChildScrollView(
          child: JoinHostPanel(
            controller: controller,
            settings: settings,
            tv: tv,
            autofocusMode: true,
            onClose: () => Navigator.of(dialog).pop(),
          ),
        ),
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final n = controller.requests.length;
        return Material(
          type: MaterialType.transparency,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (n > 0)
                FocusRing(
                  child: FilledButton.tonal(
                    key: const Key('joinPill'),
                    onPressed: () => _open(context),
                    child: Text(t.joinPill(n)),
                  ),
                ),
              if (n > 0) const SizedBox(width: 8),
              FocusRing(
                child: OutlinedButton(
                  key: const Key('joinChip'),
                  onPressed: () => _open(context),
                  child: Text(
                    t.joinChip(joinModeLabel(t, controller.joinMode)),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// The sender's waiting card: nothing is sent until the host allows; shows the check number.
class ApprovalWaitingCard extends StatelessWidget {
  const ApprovalWaitingCard({
    super.key,
    required this.wait,
    required this.onCancel,
  });

  final ApprovalWait wait;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final theme = Theme.of(context);
    return Card(
      key: const Key('approvalWaitingCard'),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(t.sendWaitingTitle, style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            Text(t.sendWaitingText),
            const SizedBox(height: 8),
            Row(
              children: [
                Text(t.sendWaitingCheck),
                const SizedBox(width: 12),
                Text(
                  '${wait.check}',
                  key: const Key('waitingCheck'),
                  style: theme.textTheme.displaySmall?.copyWith(
                    fontFamily: 'monospace',
                  ),
                ),
              ],
            ),
            Text(t.sendWaitingCheckHelp, style: theme.textTheme.bodySmall),
            const SizedBox(height: 12),
            OutlinedButton(
              key: const Key('cancelWaitButton'),
              onPressed: onCancel,
              child: Text(t.sendWaitingCancel),
            ),
          ],
        ),
      ),
    );
  }
}

/// The pause before asking again after a denial ("You can ask again in 12 s"). [onElapsed] is
/// called when the pause ends (so that the start button can enable). The reason itself is the
/// status line ([joinRefusalText]).
class RefusalNote extends StatefulWidget {
  const RefusalNote({
    super.key,
    required this.cooldownLeft,
    required this.onElapsed,
  });

  final int Function() cooldownLeft;
  final VoidCallback onElapsed;

  @override
  State<RefusalNote> createState() => _RefusalNoteState();
}

class _RefusalNoteState extends State<RefusalNote> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    if (widget.cooldownLeft() > 0) {
      _timer = Timer.periodic(const Duration(seconds: 1), (_) {
        if (!mounted) return;
        setState(() {});
        if (widget.cooldownLeft() == 0) {
          _timer?.cancel();
          widget.onElapsed();
        }
      });
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final left = widget.cooldownLeft();
    if (left <= 0) return const SizedBox.shrink();
    return Text(
      t.joinRetryIn(left),
      key: const Key('retryIn'),
      style: Theme.of(context).textTheme.bodySmall,
    );
  }
}

/// The optional name that a host sees when it has to approve this device.
class DisplayNameField extends StatefulWidget {
  const DisplayNameField({
    super.key,
    required this.settings,
    this.enabled = true,
  });

  final AppSettings settings;
  final bool enabled;

  @override
  State<DisplayNameField> createState() => _DisplayNameFieldState();
}

class _DisplayNameFieldState extends State<DisplayNameField> {
  late final TextEditingController _text = TextEditingController(
    text: widget.settings.displayName,
  );

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return TextField(
      key: const Key('displayNameField'),
      controller: _text,
      enabled: widget.enabled,
      maxLength: maxDisplayNameChars,
      textInputAction: TextInputAction.next,
      decoration: InputDecoration(
        labelText: t.joinNameFieldLabel,
        helperText: t.joinNameFieldHelper,
        border: const OutlineInputBorder(),
      ),
      onChanged: widget.settings.setDisplayName,
    );
  }
}
