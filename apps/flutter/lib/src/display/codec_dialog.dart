// SPDX-License-Identifier: GPL-3.0-or-later
// The video codec dialog of the display (spec 4.7): the link-card dialog with the consent for the
// free H.264 decoder. Over Presenting only, once per presentation (the caller decides when it is
// built); it closes by itself when the decoder is ready.

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../codec_controller.dart';
import '../../core/core_api.dart';
import '../design/icons.dart';
import 'display_logic.dart';
import 'kit.dart';
import 'screens.dart' show InputGuard;

class CodecDialog extends StatelessWidget {
  const CodecDialog({
    super.key,
    required this.controller,
    required this.onLater,
    this.guard = requestGuard,
  });

  final CodecController controller;

  /// "Not now", Back and Escape: hides the dialog until the next presentation.
  final VoidCallback onLater;
  final Duration guard;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        if (controller.ready) return const SizedBox.shrink();
        final info = controller.info;
        final state = controller.state;
        return InputGuard(
          identity: 'codec',
          guard: guard,
          builder: (context, armed) {
            void guarded(VoidCallback f) {
              if (armed) f();
            }

            Widget source() => info == null
                ? const SizedBox.shrink()
                : Text(
                    t.codecSource(
                      info.host,
                      info.fileName,
                      codecSizeText(info.downloadBytes),
                    ),
                    key: const Key('codec-source'),
                    style: s.type.footer,
                  );
            Widget offline() => Text(
              info?.adminDir != null
                  ? t.codecOffline(info!.fileName, info.adminDir!)
                  : t.codecOfflineNoDir(info?.fileName ?? 'libopenh264'),
              key: const Key('codec-offline'),
              style: s.type.description.copyWith(fontSize: 14, height: 1.4),
            );
            Widget later(int col, {bool autofocus = false}) => DButton(
              key: const Key('codec-later'),
              label: t.codecLater,
              row: 0,
              col: col,
              autofocus: autofocus,
              onPressed: () => guarded(onLater),
            );
            Widget explain() => Text(
              t.codecExplain,
              key: const Key('codec-explain'),
              style: s.type.cardText,
            );
            // Spec 4.7: the states differ in the line and the row; the body (explain, source,
            // admin note) stays in missing, downloading and failed. Heights at 1080p: missing
            // 626 px, downloading 730, failed 702, unsupported 400.
            final children = switch (state) {
              UiCodecState_Ready() => <Widget>[],
              UiCodecState_Missing() => [
                explain(),
                source(),
                offline(),
                Row(
                  children: [
                    DButton(
                      key: const Key('codec-download'),
                      label: t.codecDownload,
                      kind: DButtonKind.primary,
                      row: 0,
                      col: 0,
                      autofocus: true,
                      onPressed: () => guarded(controller.download),
                    ),
                    const SizedBox(width: 12),
                    later(1),
                  ],
                ),
              ],
              UiCodecState_Downloading(:final progress) => [
                explain(),
                source(),
                offline(),
                // One line, then the bar 8 below it (the card's gap of 16 is 8 less here).
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      '${t.codecDownloading} '
                      '${t.codecProgress((progress * 100).round().toString())}',
                      key: const Key('codec-downloading'),
                      style: s.type.label17Bold.copyWith(fontSize: 16),
                    ),
                    const SizedBox(height: 8),
                    _Bar(fraction: progress),
                  ],
                ),
                Row(children: [later(0, autofocus: true)]),
              ],
              UiCodecState_Failed() => [
                explain(),
                // `info` 20 in `--acc` and the key's text, no red (4.5 warning lines, N46).
                Row(
                  children: [
                    CcIcon(CcIconName.info, color: s.tokens.acc, size: 20),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        t.codecFailed,
                        key: const Key('codec-failed'),
                        style: s.type.label17Bold.copyWith(fontSize: 16),
                      ),
                    ),
                  ],
                ),
                source(),
                offline(),
                Row(
                  children: [
                    DButton(
                      key: const Key('codec-retry'),
                      label: t.codecRetry,
                      kind: DButtonKind.primary,
                      row: 0,
                      col: 0,
                      autofocus: true,
                      onPressed: () => guarded(controller.download),
                    ),
                    const SizedBox(width: 12),
                    later(1),
                  ],
                ),
              ],
              UiCodecState_Unsupported() => [
                Text(
                  t.codecUnsupported,
                  key: const Key('codec-unsupported'),
                  style: s.type.cardText,
                ),
                Row(
                  children: [
                    DButton(
                      key: const Key('codec-close'),
                      label: t.settingsClose,
                      kind: DButtonKind.primary,
                      row: 0,
                      col: 0,
                      autofocus: true,
                      onPressed: () => guarded(onLater),
                    ),
                  ],
                ),
              ],
            };
            return DScrim(
              child: Semantics(
                scopesRoute: true,
                namesRoute: true,
                explicitChildNodes: true,
                label: t.codecTitle,
                child: DCard(
                  width: 780,
                  shadow: true,
                  children: [DSectionLabel(t.codecTitle), ...children],
                ),
              ),
            );
          },
        );
      },
    );
  }
}

/// 6 ref bar: track `--panel2` with a 1 px `--line` border, fill `--acc`, radius 3.
class _Bar extends StatelessWidget {
  const _Bar({required this.fraction});
  final double fraction;

  @override
  Widget build(BuildContext context) {
    final t = DisplayScope.of(context).tokens;
    return Container(
      key: const Key('codec-progress'),
      height: 6,
      decoration: BoxDecoration(
        color: t.panel2,
        borderRadius: BorderRadius.circular(3),
        border: Border.all(width: 1, color: t.line),
      ),
      child: Align(
        alignment: Alignment.centerLeft,
        child: FractionallySizedBox(
          widthFactor: fraction.clamp(0.0, 1.0),
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: t.acc,
              borderRadius: BorderRadius.circular(3),
            ),
          ),
        ),
      ),
    );
  }
}
