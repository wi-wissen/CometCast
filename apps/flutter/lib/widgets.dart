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

/// Wraps a screen so that the Escape key goes back to the previous screen.
class EscapeToGoBack extends StatelessWidget {
  const EscapeToGoBack({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () =>
            Navigator.of(context).maybePop(),
      },
      child: child,
    );
  }
}

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

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return TextField(
      key: const Key('serverField'),
      controller: _controller,
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
