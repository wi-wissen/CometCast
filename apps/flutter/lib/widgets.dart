// SPDX-License-Identifier: GPL-3.0-or-later
// Small widgets that the receive and send screens share.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'core/core_api.dart';
import 'discovery.dart';
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
  void initState() {
    super.initState();
    widget.settings.addListener(_syncText);
  }

  @override
  void dispose() {
    widget.settings.removeListener(_syncText);
    _focusNode.dispose();
    _controller.dispose();
    super.dispose();
  }

  /// The settings changed the address (a tap on a suggestion, the result of the discovery).
  void _syncText() {
    if (_controller.text != widget.settings.serverAddress) {
      _controller.text = widget.settings.serverAddress;
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.settings,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final settings = widget.settings;
    final locked = settings.locked;
    final field = TextField(
      key: const Key('serverField'),
      controller: _controller,
      focusNode: _focusNode,
      enabled: widget.enabled,
      // Read-only, not disabled: the address can still be selected and copied.
      readOnly: locked,
      autofocus: widget.autofocus,
      keyboardType: TextInputType.url,
      textInputAction: widget.textInputAction,
      autocorrect: false,
      enableSuggestions: false,
      decoration: InputDecoration(
        labelText: t.serverLabel,
        helperText: locked ? null : t.serverHelper,
        suffixIcon: locked ? const Icon(Icons.lock_outline) : null,
        border: const OutlineInputBorder(),
      ),
      onChanged: (v) => settings.serverAddress = v,
      onSubmitted: (_) => widget.onSubmitted?.call(),
    );
    final suggestions = settings.serverSuggestions;
    final scheme = Theme.of(context).colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        field,
        if (settings.valueSource != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              t.serverFieldSource(serverSourceText(t, settings.valueSource!)),
              key: const Key('serverFieldSource'),
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        if (locked)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(Icons.lock_outline, size: 18, color: scheme.primary),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    t.serverLockedNote,
                    key: const Key('serverLockedNote'),
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                ),
              ],
            ),
          ),
        if (settings.discovering)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            // A still icon, not a spinner: nothing animates forever, and the screen is usable.
            child: Semantics(
              liveRegion: true,
              child: Row(
                children: [
                  Icon(Icons.search, size: 18, color: scheme.onSurfaceVariant),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      t.discoveryProgress,
                      key: const Key('discoveryProgress'),
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ),
                ],
              ),
            ),
          ),
        if (suggestions.isNotEmpty) ...[
          const SizedBox(height: 8),
          Text(
            t.serverSuggestionsLabel,
            style: Theme.of(context).textTheme.labelMedium,
          ),
          const SizedBox(height: 4),
          Wrap(
            spacing: 8,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              for (final s in suggestions)
                ActionChip(
                  key: Key('serverSuggestion:${s.address}'),
                  label: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(s.address),
                      Text(
                        serverSourceText(t, s.kind),
                        key: Key('serverSource:${s.address}'),
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ),
                  onPressed: widget.enabled
                      ? () => settings.chooseSuggestion(s)
                      : null,
                ),
            ],
          ),
        ],
        for (final n in settings.notices)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: NoticeBox(
              noticeKey: Key('discoveryNotice:${n.name}'),
              text: discoveryNoticeText(t, n),
            ),
          ),
      ],
    );
  }
}

/// Translated name of the source of a suggested server.
String serverSourceText(AppLocalizations t, ServerSourceKind k) => switch (k) {
  ServerSourceKind.policy => t.serverSourcePolicy,
  ServerSourceKind.dns => t.serverSourceDns,
  ServerSourceKind.announced => t.serverSourceAnnounced,
  ServerSourceKind.defaultList => t.serverSourceDefault,
  ServerSourceKind.lastUsed => t.serverSourceLastUsed,
  ServerSourceKind.development => t.serverSourceDevelopment,
  ServerSourceKind.unknown => t.serverSourceOther,
};

/// Translated, calm sentence for a note from the discovery. Only the code is used, never text
/// from the policy file or the network.
String discoveryNoticeText(
  AppLocalizations t,
  DiscoveryWarning w,
) => switch (w) {
  DiscoveryWarning.policyUnusable => t.discoveryNoticePolicyUnusable,
  DiscoveryWarning.policyNamesNoServer => t.discoveryNoticePolicyNamesNoServer,
  DiscoveryWarning.dnsTimeout => t.discoveryNoticeDnsTimeout,
  DiscoveryWarning.dnsFailed => t.discoveryNoticeDnsFailed,
  DiscoveryWarning.dnsServiceUnavailable => t.discoveryNoticeDnsUnavailable,
  DiscoveryWarning.entriesIgnored => t.discoveryNoticeEntriesIgnored,
  DiscoveryWarning.announcementFailed => t.discoveryNoticeAnnouncementFailed,
  DiscoveryWarning.other => t.discoveryNoticeOther,
};

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
