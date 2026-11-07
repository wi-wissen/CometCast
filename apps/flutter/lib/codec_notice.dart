// SPDX-License-Identifier: GPL-3.0-or-later
// The card that explains and offers the download of the H.264 codec. Shown on the screens that
// handle shared screens while the codec is missing; nothing is shown once it is ready.

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'codec_controller.dart';
import 'core/core_api.dart';

/// "About 620 KB" / "about 1.2 MB" for a size in bytes.
String codecSizeText(int bytes) {
  if (bytes >= 1024 * 1024) {
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
  return '${(bytes / 1024).round()} KB';
}

class CodecNotice extends StatelessWidget {
  const CodecNotice({super.key, required this.controller});

  final CodecController controller;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        if (controller.ready) return const SizedBox.shrink();
        final t = AppLocalizations.of(context);
        final info = controller.info;
        final state = controller.state;
        final theme = Theme.of(context);
        return Card(
          key: const Key('codecNotice'),
          margin: const EdgeInsets.symmetric(vertical: 8),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    const Icon(Icons.movie_outlined),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        t.codecTitle,
                        style: theme.textTheme.titleMedium,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                ..._body(context, t, info, state),
              ],
            ),
          ),
        );
      },
    );
  }

  List<Widget> _body(
    BuildContext context,
    AppLocalizations t,
    UiCodecInfo? info,
    UiCodecState state,
  ) {
    final small = Theme.of(context).textTheme.bodySmall;
    Widget source() => info == null
        ? const SizedBox.shrink()
        : Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              t.codecSource(
                info.host,
                info.fileName,
                codecSizeText(info.downloadBytes),
              ),
              key: const Key('codecSource'),
              style: small,
            ),
          );
    Widget offline() => Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Text(
        info?.adminDir != null
            ? t.codecOffline(info!.fileName, info.adminDir!)
            : t.codecOfflineNoDir(info?.fileName ?? 'libopenh264'),
        key: const Key('codecOffline'),
        style: small,
      ),
    );
    return switch (state) {
      UiCodecState_Ready() => const [],
      UiCodecState_Missing() => [
        Text(t.codecExplain),
        source(),
        const SizedBox(height: 12),
        FilledButton(
          key: const Key('codecDownload'),
          onPressed: controller.download,
          child: Text(t.codecDownload),
        ),
        offline(),
      ],
      UiCodecState_Downloading(:final progress) => [
        Text(t.codecDownloading),
        const SizedBox(height: 8),
        Semantics(
          label: t.codecDownloading,
          child: LinearProgressIndicator(
            key: const Key('codecProgress'),
            value: progress > 0 ? progress : null,
          ),
        ),
        const SizedBox(height: 4),
        Text(t.codecProgress((progress * 100).round().toString())),
        source(),
      ],
      UiCodecState_Failed(:final reason) => [
        Text(t.codecFailed(reason), key: const Key('codecFailed')),
        const SizedBox(height: 12),
        FilledButton(
          key: const Key('codecRetry'),
          onPressed: controller.download,
          child: Text(t.codecRetry),
        ),
        offline(),
      ],
      UiCodecState_Unsupported(:final reason) => [
        Text(t.codecUnsupported(reason), key: const Key('codecUnsupported')),
      ],
    };
  }
}
