// SPDX-License-Identifier: GPL-3.0-or-later
// The share page of the presenter (spec 15.3, fix list F7): the item that arrived from the
// system share sheet, as a card, and the actions around it. The not-connected variant is the
// start page with this card as its heading (`FirstRunPage.intro`); this file has the card and
// the connected and refused variants. No flow here: presenter_root.dart decides.

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../core/share_intake.dart';
import '../design/icons.dart';
import 'kit.dart';

/// What the card shows about an item.
enum ShareKind { pdf, image, office, link }

enum OfficeKind { word, powerpoint, excel }

/// The kind of a shared file by its name (the MIME type is the fallback); null for a type the
/// presenter does not show.
(ShareKind, OfficeKind?)? shareKindOfFile(String? name, String? mime) {
  final n = (name ?? '').toLowerCase();
  final m = (mime ?? '').toLowerCase();
  bool ext(List<String> e) => e.any(n.endsWith);
  if (ext(['.pdf']) || m == 'application/pdf') return (ShareKind.pdf, null);
  if (ext(['.png', '.jpg', '.jpeg', '.webp', '.gif']) ||
      m.startsWith('image/')) {
    return (ShareKind.image, null);
  }
  if (ext(['.docx']) || m.contains('wordprocessingml')) {
    return (ShareKind.office, OfficeKind.word);
  }
  if (ext(['.pptx']) || m.contains('presentationml')) {
    return (ShareKind.office, OfficeKind.powerpoint);
  }
  if (ext(['.xlsx']) || m.contains('spreadsheetml')) {
    return (ShareKind.office, OfficeKind.excel);
  }
  return null;
}

/// "1.8 MB" / "1,8 MB" (one decimal, `MB` from 1 MiB, else `KB`).
String formatShareSize(int bytes, String languageCode) {
  final mb = bytes >= 1024 * 1024;
  final v = mb ? bytes / (1024 * 1024) : bytes / 1024;
  final text = v.toStringAsFixed(1);
  return '${languageCode == 'de' ? text.replaceFirst('.', ',') : text} ${mb ? 'MB' : 'KB'}';
}

/// A file name shortened in the middle keeping the extension.
String shortenMiddle(String name, {int max = 28}) {
  final runes = name.runes.toList();
  if (runes.length <= max) return name;
  final dot = name.lastIndexOf('.');
  final extRunes = dot > 0 && name.length - dot <= 6
      ? name.substring(dot).runes.toList()
      : <int>[];
  final tail = extRunes.length + 3;
  final head = max - tail - 1;
  return '${String.fromCharCodes(runes.take(head))}…'
      '${String.fromCharCodes(runes.skip(runes.length - tail))}';
}

/// The item card (spec 15.3 item 2): icon 28 `p-primary` on a 56 disc `p-primary-soft`, title
/// 17/700, meta line 13 `p-fg3`; a refusal replaces the meta line by [error] in 13/700 `p-error`.
class ShareCard extends StatelessWidget {
  const ShareCard({
    super.key,
    required this.kind,
    required this.title,
    this.meta,
    this.error,
    this.converting,
    this.isLink = false,
  });

  final ShareKind kind;
  final String title;
  final String? meta;
  final String? error;

  /// `file.converting` while an Office file converts after "Show".
  final String? converting;
  final bool isLink;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    return Container(
      key: const Key('share-card'),
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: p.surface,
        border: Border.all(width: 1, color: p.borderCard),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Row(
        // The disc keeps the position it has with a one-line meta line; a longer meta or
        // refusal text grows downward (spec 15.14 N113).
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            key: const Key('share-disc'),
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: p.primarySoft,
            ),
            child: Center(
              child: CcIcon(
                kind == ShareKind.link ? CcIconName.link : CcIconName.file,
                color: p.primary,
                size: 28,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (isLink)
                  Text(
                    title,
                    key: const Key('share-title'),
                    style: s.type.small.copyWith(
                      fontFamily: 'AtkinsonHyperlegibleMono',
                      fontSize: 15,
                      color: p.fg,
                    ),
                  )
                else
                  Text(
                    title,
                    key: const Key('share-title'),
                    maxLines: 1,
                    style: s.type.optionTitle,
                  ),
                const SizedBox(height: 2),
                if (error != null)
                  Semantics(
                    liveRegion: true,
                    child: Text(
                      error!,
                      key: const Key('share-error'),
                      style: s.type.errorText,
                    ),
                  )
                else if (converting != null)
                  Text(
                    converting!,
                    key: const Key('share-converting'),
                    style: s.type.small,
                  )
                else if (meta != null)
                  Text(
                    meta!,
                    key: const Key('share-meta'),
                    style: s.type.small,
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// The text of the item's meta line.
String shareMeta(
  AppLocalizations t,
  ShareKind kind,
  OfficeKind? office,
  int? size,
  String lang,
) {
  final sz = formatShareSize(size ?? 0, lang);
  return switch (kind) {
    ShareKind.pdf => t.shareMetaPdf(sz),
    ShareKind.image => t.shareMetaImage(sz),
    ShareKind.link => t.shareMetaLink,
    ShareKind.office => t.shareMetaOffice(switch (office) {
      OfficeKind.powerpoint => 'PowerPoint',
      OfficeKind.excel => 'Excel',
      _ => 'Word',
    }, sz),
  };
}

/// What an item looks like to the page, computed by the root.
class ShareView {
  const ShareView({
    required this.kind,
    required this.title,
    this.meta,
    this.error,
    this.isLink = false,
  });
  final ShareKind kind;
  final String title;
  final String? meta;

  /// A refusal: replaces the meta line; the only action is `common.cancel`.
  final String? error;
  final bool isLink;
}

/// The heading of the share page.
class ShareHeading extends StatelessWidget {
  const ShareHeading({super.key});

  @override
  Widget build(BuildContext context) => Text(
    AppLocalizations.of(context).shareTitle,
    key: const Key('share-heading'),
    style: PresenterScope.of(context).type.heading,
  );
}

/// The not-connected variant's heading and card (`FirstRunPage.intro`).
List<Widget> shareIntro(ShareView v, {String? converting}) => [
  const ShareHeading(),
  ShareCard(
    kind: v.kind,
    title: v.title,
    meta: v.meta,
    error: v.error,
    converting: converting,
    isLink: v.isLink,
  ),
];

/// Connected (Android, the app holds a join) and refused variants.
class SharePage extends StatelessWidget {
  const SharePage({
    super.key,
    required this.view,
    required this.onCancel,
    this.room,
    this.onShow,
    this.onOtherScreen,
    this.converting,
  });

  final ShareView view;
  final VoidCallback onCancel;

  /// The connected screen's name, already quoted (`common.quoted`) or `common.noName`; null
  /// without a room name (`share.showHere`).
  final String? room;
  final VoidCallback? onShow;
  final VoidCallback? onOtherScreen;
  final String? converting;

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final refused = view.error != null;
    return PPage(
      actions: [
        if (!refused && onShow != null) ...[
          PPrimaryButton(
            key: const Key('share-show'),
            label: room == null ? t.shareShowHere : t.shareShowOn(room!),
            // Both buttons are disabled while an Office file converts (N113).
            onPressed: converting == null ? onShow : null,
          ),
          if (onOtherScreen != null)
            PSecondaryButton(
              key: const Key('share-other'),
              label: t.shareOtherScreen,
              onPressed: converting == null ? onOtherScreen : null,
            ),
        ] else
          PSecondaryButton(
            key: const Key('share-cancel'),
            label: t.commonCancel,
            onPressed: onCancel,
          ),
      ],
      children: shareIntro(view, converting: converting),
    );
  }
}

/// The item as the page sees it, from the channel's [SharedItem].
ShareView shareViewOf(
  AppLocalizations t,
  SharedItem item,
  String lang, {
  String? error,
}) {
  switch (item.kind) {
    case SharedKind.link:
      final u = Uri.tryParse(item.url ?? '');
      final text = u == null || u.host.isEmpty
          ? (item.url ?? '')
          : '${u.host}${u.path == '/' ? '' : u.path}';
      return ShareView(
        kind: ShareKind.link,
        title: text,
        meta: shareMeta(t, ShareKind.link, null, null, lang),
        error: error,
        isLink: true,
      );
    case SharedKind.noLink:
      return ShareView(
        kind: ShareKind.link,
        title: '',
        error: t.shareNoLink,
        isLink: true,
      );
    case SharedKind.tooLarge:
    case SharedKind.file:
      final k = shareKindOfFile(item.name, item.mime);
      final kind = k?.$1 ?? ShareKind.pdf;
      return ShareView(
        kind: kind,
        title: shortenMiddle(item.name ?? ''),
        meta: item.kind == SharedKind.tooLarge
            ? null
            : shareMeta(t, kind, k?.$2, item.size, lang),
        error: item.kind == SharedKind.tooLarge ? t.sendRejectTooLarge : error,
      );
  }
}
