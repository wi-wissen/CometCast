// SPDX-License-Identifier: GPL-3.0-or-later
// Join approval on the screens: the request takeover (a large dialog on the code page, a banner
// while somebody presents), the host's menu (who may join, who asks, who is connected, the block
// list, "End session"), the corner chip, and the sender's waiting card.
//
// Rules (docs/protocol.md, "Join approval"), also for a TV remote (D-pad, no touch, no mouse):
// * Yes / No / No and block, nothing to hold. A request that just appeared ignores presses for
//   a short moment (700 ms), so an OK press meant for something else cannot answer it;
// * when a request takes over, the focus goes to "No" (the safe choice), never to "Yes"; after a
//   decision it goes to the next request's "No";
// * Back / Escape hides the takeover without deciding ("Later");
// * the menu chip appears when something moves and fades out; "End session" needs a second press;
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

/// A request ignores presses for this long after it appeared. UNCERTAIN: placeholder.
const requestGuard = Duration(milliseconds: 700);

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

/// One join request: the sender's name in quotes (untrusted text, plain), the check number, the
/// time left, and No / Yes / No and block. Ignores presses for [requestGuard] after it appeared.
class JoinRequestCard extends StatefulWidget {
  const JoinRequestCard({
    super.key,
    required this.request,
    required this.onAllow,
    required this.onDeny,
    required this.onBlock,
    this.denyNode,
    this.tv = false,
    this.big = false,
  });

  final JoinRequestView request;
  final VoidCallback onAllow;
  final VoidCallback onDeny;
  final VoidCallback onBlock;

  /// Focus node of "No" (the takeover moves the focus there).
  final FocusNode? denyNode;
  final bool tv;

  /// The takeover layout (larger).
  final bool big;

  @override
  State<JoinRequestCard> createState() => _JoinRequestCardState();
}

class _JoinRequestCardState extends State<JoinRequestCard> {
  Timer? _tick;

  @override
  void initState() {
    super.initState();
    _tick = Timer.periodic(const Duration(milliseconds: 250), (_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _tick?.cancel();
    super.dispose();
  }

  bool get _armed =>
      joinClock().difference(widget.request.receivedAt) >= requestGuard;

  int get _left {
    final left =
        joinRequestSeconds -
        joinClock().difference(widget.request.receivedAt).inSeconds;
    return left < 0 ? 0 : left;
  }

  void _act(VoidCallback fn) {
    // Not armed yet: the press is ignored.
    if (_armed) fn();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final r = widget.request;
    final big = widget.big || widget.tv;
    final size = Size(big ? 180 : 120, big ? 64 : 48);
    final textStyle = big
        ? theme.textTheme.titleLarge
        : theme.textTheme.titleMedium;
    final dim = _armed ? 1.0 : 0.6;
    return Card(
      key: Key('request-${r.id}'),
      margin: const EdgeInsets.only(top: 8),
      color: theme.colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: EdgeInsets.all(widget.big ? 24 : 12),
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
                _NameBox(
                  key: Key('requestName-${r.id}'),
                  name: r.name,
                  style: widget.big ? theme.textTheme.headlineMedium : null,
                ),
                Text(
                  t.joinWantsToPresent,
                  style: widget.big ? theme.textTheme.titleLarge : null,
                ),
              ],
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Text(
                  '${r.check}',
                  key: Key('requestCheck-${r.id}'),
                  style:
                      (widget.big
                              ? theme.textTheme.displayMedium
                              : theme.textTheme.headlineMedium)
                          ?.copyWith(fontFamily: 'monospace'),
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
            LinearProgressIndicator(value: _left / joinRequestSeconds),
            Text(t.joinSecondsLeft(_left), style: theme.textTheme.bodySmall),
            const SizedBox(height: 8),
            Opacity(
              opacity: dim,
              child: Wrap(
                spacing: 12,
                runSpacing: 8,
                children: [
                  // "No" comes first in the focus order; "Yes" is never the default.
                  FocusRing(
                    child: OutlinedButton(
                      key: Key('deny-${r.id}'),
                      focusNode: widget.denyNode,
                      style: OutlinedButton.styleFrom(
                        minimumSize: size,
                        textStyle: textStyle,
                      ),
                      onPressed: () => _act(widget.onDeny),
                      child: Text(t.joinDeny),
                    ),
                  ),
                  FocusRing(
                    child: FilledButton(
                      key: Key('allow-${r.id}'),
                      style: FilledButton.styleFrom(
                        minimumSize: size,
                        textStyle: textStyle,
                      ),
                      onPressed: () => _act(widget.onAllow),
                      child: Text(t.joinAllow),
                    ),
                  ),
                  FocusRing(
                    child: OutlinedButton(
                      key: Key('denyBlock-${r.id}'),
                      style: OutlinedButton.styleFrom(
                        minimumSize: size,
                        textStyle: textStyle,
                      ),
                      onPressed: () => _act(widget.onBlock),
                      child: Text(t.joinDenyBlock),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The host's menu: who may join (mode), requests that are not on the takeover, who is connected
/// (end the presentation, with or without blocking), the block list and "End session".
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
  bool _confirmEnd = false;
  Timer? _endTimer;

  ReceiverController get c => widget.controller;

  @override
  void dispose() {
    _endTimer?.cancel();
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

  List<JoinRequestView> get _listed {
    final top = c.takeoverRequest;
    return [
      for (final r in c.requests)
        if (r.id != top?.id) r,
    ];
  }

  void _afterDecision() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final shown = _listed.take(maxShownRequests).toList();
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

  void _onEnd() {
    if (!_confirmEnd) {
      setState(() => _confirmEnd = true);
      _endTimer = Timer(const Duration(seconds: 4), () {
        if (mounted) setState(() => _confirmEnd = false);
      });
      return;
    }
    _endTimer?.cancel();
    setState(() => _confirmEnd = false);
    c.endRoom();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final theme = Theme.of(context);
    return ListenableBuilder(
      listenable: c,
      builder: (context, _) {
        final listed = _listed;
        final shown = listed.take(maxShownRequests).toList();
        final more = listed.length - shown.length;
        _denyNodes.removeWhere((id, n) {
          final gone = c.requests.every((r) => r.id != id);
          if (gone) {
            WidgetsBinding.instance.addPostFrameCallback((_) => n.dispose());
          }
          return gone;
        });
        final buttonHeight = widget.tv ? 60.0 : 48.0;
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
                        minimumSize: Size(0, buttonHeight),
                      ),
                      onPressed: () => _setMode(UiJoinMode.locked),
                      child: Text(t.joinLockNow),
                    ),
                  ),
                ],
                const Divider(height: 24),
                Text(t.joinRequestsTitle, style: theme.textTheme.titleSmall),
                if (c.requests.isEmpty)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    child: Text(
                      t.joinNobodyWaiting,
                      key: const Key('joinNobodyWaiting'),
                    ),
                  ),
                for (final r in shown)
                  JoinRequestCard(
                    key: Key('listed-${r.id}'),
                    request: r,
                    tv: widget.tv,
                    denyNode: _denyNode(r.id),
                    onAllow: () {
                      c.decideJoin(r.id, allow: true);
                      _afterDecision();
                    },
                    onDeny: () {
                      c.decideJoin(r.id, allow: false);
                      _afterDecision();
                    },
                    onBlock: () {
                      c.decideJoin(r.id, allow: false, block: true);
                      _afterDecision();
                    },
                  ),
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
                else ...[
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: _NameBox(
                      key: const Key('peerName'),
                      name: c.peerName!,
                      max: 40,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      FocusRing(
                        child: OutlinedButton(
                          key: const Key('removePeerButton'),
                          style: OutlinedButton.styleFrom(
                            minimumSize: Size(0, buttonHeight),
                          ),
                          onPressed: c.removePeer,
                          child: Text(
                            t.joinEndPresentation,
                            semanticsLabel: t.joinRemoveLabel(
                              shortJoinName(c.peerName!),
                            ),
                          ),
                        ),
                      ),
                      FocusRing(
                        child: OutlinedButton(
                          key: const Key('removeBlockButton'),
                          style: OutlinedButton.styleFrom(
                            minimumSize: Size(0, buttonHeight),
                          ),
                          onPressed: () => c.removePeer(block: true),
                          child: Text(t.joinEndAndBlock),
                        ),
                      ),
                    ],
                  ),
                ],
                const Divider(height: 24),
                Text(t.joinBlockedTitle, style: theme.textTheme.titleSmall),
                if (c.blocks.isEmpty)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    child: Text(t.joinNoneBlocked),
                  )
                else
                  for (final b in c.blocks)
                    Padding(
                      key: Key('blocked-${b.id}'),
                      padding: const EdgeInsets.only(top: 8),
                      child: Row(
                        children: [
                          Expanded(child: _NameBox(name: b.name)),
                          const SizedBox(width: 8),
                          FocusRing(
                            child: OutlinedButton(
                              key: Key('unblock-${b.id}'),
                              style: OutlinedButton.styleFrom(
                                minimumSize: Size(0, buttonHeight),
                              ),
                              onPressed: () => c.unblock(b.id),
                              child: Text(t.joinUnblock),
                            ),
                          ),
                        ],
                      ),
                    ),
                Text(t.joinBlockedHelp, style: theme.textTheme.bodySmall),
                const Divider(height: 24),
                FocusRing(
                  child: OutlinedButton(
                    key: const Key('endSessionButton'),
                    style: OutlinedButton.styleFrom(
                      minimumSize: Size(0, buttonHeight),
                      foregroundColor: theme.colorScheme.error,
                    ),
                    onPressed: _onEnd,
                    child: Text(
                      _confirmEnd ? t.joinEndSessionConfirm : t.joinEndSession,
                    ),
                  ),
                ),
                Text(t.joinEndSessionHelp, style: theme.textTheme.bodySmall),
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
}

/// A name that somebody else chose, in quotes in a dashed box, shortened. Plain text.
class _NameBox extends StatelessWidget {
  const _NameBox({super.key, required this.name, this.max = 18, this.style});

  final String name;
  final int max;
  final TextStyle? style;

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
          '\u201C${shortJoinName(name, max: max)}\u201D',
          style: style ?? Theme.of(context).textTheme.titleMedium,
        ),
      ),
    );
  }
}

/// An incoming request takes over the display: a large dialog on the code page ([banner] false),
/// a banner at the top while somebody presents. When it appears the focus goes to "No"; Back /
/// Escape hides it without deciding ("Later").
class JoinTakeover extends StatefulWidget {
  const JoinTakeover({
    super.key,
    required this.controller,
    this.banner = false,
    this.tv = false,
  });

  final ReceiverController controller;
  final bool banner;
  final bool tv;

  @override
  State<JoinTakeover> createState() => _JoinTakeoverState();
}

class _JoinTakeoverState extends State<JoinTakeover> {
  final _deny = FocusNode(debugLabel: 'takeover-deny');
  BigInt? _shownId;
  FocusNode? _returnFocus;

  ReceiverController get c => widget.controller;

  @override
  void dispose() {
    _deny.dispose();
    super.dispose();
  }

  void _sync(JoinRequestView? r) {
    final id = r?.id;
    if (id == _shownId) return;
    final before = _shownId;
    _shownId = id;
    if (id != null && before == null) {
      _returnFocus = FocusManager.instance.primaryFocus;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (id != null) {
        _deny.requestFocus();
      } else {
        final back = _returnFocus;
        _returnFocus = null;
        if (back != null && back.context != null) back.requestFocus();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return ListenableBuilder(
      listenable: c,
      builder: (context, _) {
        final r = c.takeoverRequest;
        _sync(r);
        if (r == null) return const SizedBox.shrink();
        final more = c.takeoverMore;
        final card = JoinRequestCard(
          key: ValueKey(r.id),
          request: r,
          tv: widget.tv,
          big: !widget.banner,
          denyNode: _deny,
          onAllow: () => c.decideJoin(r.id, allow: true),
          onDeny: () => c.decideJoin(r.id, allow: false),
          onBlock: () => c.decideJoin(r.id, allow: false, block: true),
        );
        final content = Card(
          key: const Key('joinTakeover'),
          elevation: 8,
          child: Padding(
            padding: EdgeInsets.all(widget.banner ? 8 : 16),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  t.joinTakeoverTitle,
                  style: widget.banner
                      ? Theme.of(context).textTheme.titleMedium
                      : Theme.of(context).textTheme.headlineSmall,
                ),
                card,
                const SizedBox(height: 8),
                Wrap(
                  spacing: 12,
                  runSpacing: 8,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    if (more > 0)
                      Text(
                        t.joinMoreWaiting(more),
                        key: const Key('takeoverMore'),
                      ),
                    if (more > 0)
                      FocusRing(
                        child: OutlinedButton(
                          key: const Key('denyAllButton'),
                          onPressed: c.denyAllJoins,
                          child: Text(t.joinDenyAll),
                        ),
                      ),
                    FocusRing(
                      child: TextButton(
                        key: const Key('laterButton'),
                        onPressed: () => c.snoozeRequest(r.id),
                        child: Text(t.joinLater),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        );
        final body = CallbackShortcuts(
          bindings: {
            const SingleActivator(LogicalKeyboardKey.escape): () =>
                c.snoozeRequest(r.id),
            const SingleActivator(LogicalKeyboardKey.goBack): () =>
                c.snoozeRequest(r.id),
          },
          child: FocusScope(
            child: widget.banner
                ? Align(
                    alignment: Alignment.topCenter,
                    child: SafeArea(
                      child: Padding(
                        padding: const EdgeInsets.all(8),
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 760),
                          child: Material(
                            type: MaterialType.transparency,
                            child: content,
                          ),
                        ),
                      ),
                    ),
                  )
                : Material(
                    type: MaterialType.transparency,
                    child: Stack(
                      children: [
                        const Positioned.fill(
                          child: ColoredBox(color: Color(0xCC000000)),
                        ),
                        Center(
                          child: SingleChildScrollView(
                            padding: const EdgeInsets.all(16),
                            child: ConstrainedBox(
                              constraints: const BoxConstraints(maxWidth: 760),
                              child: content,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
          ),
        );
        return Semantics(
          container: true,
          liveRegion: true,
          label: t.joinAnnounce(shortJoinName(r.name), r.check),
          child: body,
        );
      },
    );
  }
}

/// Corner chip (with the number of waiting requests) while somebody presents. It appears when
/// something moves and fades out ([visible]); it opens the menu in a dialog, and Back closes that
/// dialog without deciding anything.
class JoinChip extends StatelessWidget {
  const JoinChip({
    super.key,
    required this.controller,
    required this.settings,
    this.tv = false,
    this.visible = true,
  });

  final ReceiverController controller;
  final AppSettings settings;
  final bool tv;
  final bool visible;

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
        return ExcludeFocus(
          excluding: !visible,
          child: IgnorePointer(
            ignoring: !visible,
            child: AnimatedOpacity(
              key: const Key('joinChipFade'),
              opacity: visible ? 1 : 0,
              duration: const Duration(milliseconds: 500),
              child: Material(
                type: MaterialType.transparency,
                child: FocusRing(
                  child: OutlinedButton(
                    key: const Key('joinChip'),
                    onPressed: () => _open(context),
                    child: Text(
                      n > 0
                          ? '${t.joinChip(joinModeLabel(t, controller.joinMode))} \u00B7 ${t.joinPill(n)}'
                          : t.joinChip(joinModeLabel(t, controller.joinMode)),
                    ),
                  ),
                ),
              ),
            ),
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

  /// Up / down arrows (D-pad) leave the field, like the other fields.
  final _focus = textFieldFocusNode(debugLabel: 'displayNameField');

  @override
  void dispose() {
    _focus.dispose();
    _text.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return TextField(
      key: const Key('displayNameField'),
      controller: _text,
      focusNode: _focus,
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
