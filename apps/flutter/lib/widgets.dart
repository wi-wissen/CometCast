// SPDX-License-Identifier: GPL-3.0-or-later
// Small widgets that the receive and send screens share.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/core_api.dart';
import 'settings.dart';

/// Translated text for a connection state (the words come from the ARB files).
String connectionStateText(AppLocalizations t, UiConnectionState? s) =>
    t.statusConnectionState(s?.name ?? 'other');

/// Wraps a screen so that the Escape key goes back to the previous screen, and so that the
/// arrow keys / D-pad of a TV remote move the focus inside the screen (one traversal group).
///
/// Flutter's built-in focus system does the work (no extra package): the arrow keys are bound to
/// directional focus by `WidgetsApp`, and select / Enter activate the focused control.
class EscapeToGoBack extends StatelessWidget {
  const EscapeToGoBack({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return FocusTraversalGroup(
      policy: ReadingOrderTraversalPolicy(),
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              Navigator.of(context).maybePop(),
        },
        child: child,
      ),
    );
  }
}

/// Draws a clearly visible frame around [child] while [child] or one of its descendants has the
/// keyboard / D-pad focus. Material's own focus overlay is too faint to see from a sofa.
class FocusRing extends StatefulWidget {
  const FocusRing({super.key, required this.child});

  final Widget child;

  @override
  State<FocusRing> createState() => _FocusRingState();
}

class _FocusRingState extends State<FocusRing> {
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    // A Focus node that cannot take focus itself, only reports focus inside its subtree.
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onFocusChange: (v) => setState(() => _focused = v),
      child: DecoratedBox(
        key: const Key('focusRing'),
        position: DecorationPosition.foreground,
        decoration: BoxDecoration(
          border: Border.all(
            width: 3,
            color: _focused
                ? Theme.of(context).colorScheme.tertiary
                : Colors.transparent,
          ),
          borderRadius: BorderRadius.circular(12),
        ),
        child: widget.child,
      ),
    );
  }
}

/// A focus node for a single-line text field that lets the up / down arrow keys (D-pad of a TV
/// remote) move the focus to the neighbouring control. Without it the text field keeps the focus
/// when the arrow keys are pressed (observed in test/dpad_focus_test.dart); left / right still move
/// the caret. Dispose it like any FocusNode.
FocusNode textFieldFocusNode({String? debugLabel}) => FocusNode(
  debugLabel: debugLabel,
  onKeyEvent: (node, event) {
    if (event is KeyUpEvent) return KeyEventResult.ignored;
    final direction = switch (event.logicalKey) {
      LogicalKeyboardKey.arrowDown => TraversalDirection.down,
      LogicalKeyboardKey.arrowUp => TraversalDirection.up,
      _ => null,
    };
    if (direction == null) return KeyEventResult.ignored;
    return node.focusInDirection(direction)
        ? KeyEventResult.handled
        : KeyEventResult.ignored;
  },
);

/// A thick border on buttons while they have focus (added to the button themes in main.dart).
ButtonStyle focusBorderStyle(ColorScheme scheme) => ButtonStyle(
  side: WidgetStateProperty.resolveWith(
    (states) => states.contains(WidgetState.focused)
        ? BorderSide(color: scheme.tertiary, width: 3)
        : null,
  ),
);

/// Text field for the server address, kept in sync with [AppSettings].
class ServerField extends StatefulWidget {
  const ServerField({
    super.key,
    required this.settings,
    required this.enabled,
    this.autofocus = false,
    this.onSubmitted,
    this.textInputAction = TextInputAction.done,
  });

  final AppSettings settings;
  final bool enabled;
  final bool autofocus;
  final VoidCallback? onSubmitted;
  final TextInputAction textInputAction;

  @override
  State<ServerField> createState() => _ServerFieldState();
}

class _ServerFieldState extends State<ServerField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.settings.serverAddress,
  );
  final FocusNode _focusNode = textFieldFocusNode(debugLabel: 'serverField');

  @override
  void dispose() {
    _focusNode.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return TextField(
      key: const Key('serverField'),
      controller: _controller,
      focusNode: _focusNode,
      enabled: widget.enabled,
      autofocus: widget.autofocus,
      keyboardType: TextInputType.url,
      textInputAction: widget.textInputAction,
      autocorrect: false,
      enableSuggestions: false,
      decoration: InputDecoration(
        labelText: t.serverLabel,
        helperText: t.serverHelper,
        border: const OutlineInputBorder(),
      ),
      onChanged: (v) => widget.settings.serverAddress = v,
      onSubmitted: (_) => widget.onSubmitted?.call(),
    );
  }
}

/// Puts content in a centred column that is at most 560 px wide (looks good on desktop).
class PageBody extends StatelessWidget {
  const PageBody({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 560),
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: children,
          ),
        ),
      ),
    );
  }
}

/// A note in a tonal box (used for the "no capture / no decoding yet" notices).
class NoticeBox extends StatelessWidget {
  const NoticeBox({super.key, required this.text, this.noticeKey});

  final String text;
  final Key? noticeKey;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      key: noticeKey,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.info_outline, size: 20, color: scheme.onSurfaceVariant),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              text,
              style: Theme.of(context).textTheme.bodyMedium
                  ?.copyWith(color: scheme.onSurfaceVariant),
            ),
          ),
        ],
      ),
    );
  }
}
