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
              style: s.type.description.copyWith(fontSize: 14),
            );
            Widget later(int col, {bool autofocus = false}) => DButton(
              key: const Key('codec-later'),
              label: t.codecLater,
              row: 0,
              col: col,
              autofocus: autofocus,
              onPressed: () => guarded(onLater),
            );
            final children = switch (state) {
              UiCodecState_Ready() => <Widget>[],
              UiCodecState_Missing() => [
                Text(
                  t.codecExplain,
                  key: const Key('codec-explain'),
                  style: s.type.cardText,
                ),
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
                Text(
                  '${t.codecDownloading} '
                  '${t.codecProgress((progress * 100).round().toString())}',
                  key: const Key('codec-downloading'),
                  style: s.type.label17Bold.copyWith(fontSize: 16),
                ),
                _Bar(fraction: progress),
                Row(children: [later(0, autofocus: true)]),
              ],
              UiCodecState_Failed(:final reason) => [
                Row(
                  children: [
                    CcIcon(CcIconName.info, color: s.tokens.acc, size: 20),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        t.codecFailed(reason),
                        key: const Key('codec-failed'),
                        style: s.type.label17Bold.copyWith(fontSize: 16),
                      ),
                    ),
                  ],
                ),
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
              UiCodecState_Unsupported(:final reason) => [
                Text(
                  t.codecUnsupported(reason),
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
                  width: 600,
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
