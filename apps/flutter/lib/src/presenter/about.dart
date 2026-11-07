// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter's app settings page, About and the licence text page (spec 15.8, 15.9; fix list
// F2 and F3). Plain widgets: the flow (which page is shown) is in presenter_root.dart.

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../licences.dart';
import '../beta.dart';
import '../branding_info.dart';
import '../design/icons.dart';
import '../design/spans.dart';
import '../design/tokens.dart' show fontMono;
import 'kit.dart';

// --- app settings (15.8) ------------------------------------------------------------------

class AppSettingsPage extends StatelessWidget {
  const AppSettingsPage({
    super.key,
    required this.version,
    required this.onAbout,
    required this.onBack,
    required this.showOffice,
    required this.showTvs,
    required this.officeOn,
    required this.tvsOn,
    required this.onOffice,
    required this.onTvs,
  });

  final String version;
  final VoidCallback onAbout;
  final VoidCallback onBack;

  /// Where the Office worker is found.
  final bool showOffice;

  /// Desktops.
  final bool showTvs;
  final bool officeOn;
  final bool tvsOn;
  final ValueChanged<bool> onOffice;
  final ValueChanged<bool> onTvs;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actions: [
        PSecondaryButton(
          key: const Key('settings-back'),
          label: t.commonBack,
          onPressed: onBack,
        ),
      ],
      children: [
        Text(t.appSettingsTitle, style: s.type.heading),
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (showOffice) ...[
              PSwitchRow(
                key: const Key('switch-office'),
                title: t.appSettingsOffice,
                sub: t.appSettingsOfficeSub,
                on: officeOn,
                onToggle: () => onOffice(!officeOn),
              ),
              const SizedBox(height: 8),
            ],
            if (showTvs) ...[
              PSwitchRow(
                key: const Key('switch-tvs'),
                title: t.appSettingsOtherTvs,
                sub: t.appSettingsOtherTvsSub,
                beta: isBeta(BetaFeature.otherTvs),
                on: tvsOn,
                onToggle: () => onTvs(!tvsOn),
              ),
              const SizedBox(height: 8),
            ],
            POption(
              key: const Key('about-open'),
              icon: CcIconName.info,
              title: t.aboutTitle,
              subtitle: t.aboutVersion(version),
              onPressed: onAbout,
            ),
          ],
        ),
      ],
    );
  }
}

// --- About (15.9) -------------------------------------------------------------------------

/// A scrolling page of the presenter: the header items, then a lazy list, then the actions at
/// the end (the page scrolls as a whole).
class PScrollPage extends StatelessWidget {
  const PScrollPage({
    super.key,
    required this.header,
    required this.itemCount,
    required this.itemBuilder,
    required this.footer,
  });

  final List<Widget> header;
  final int itemCount;
  final IndexedWidgetBuilder itemBuilder;
  final Widget footer;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final height = MediaQuery.sizeOf(context).height;
    final pad = s.laptop
        ? EdgeInsets.fromLTRB(40, height * .1 < 40 ? 40 : height * .1, 40, 40)
        : const EdgeInsets.fromLTRB(22, 18, 22, 22);
    Widget spaced(List<Widget> items) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (var i = 0; i < items.length; i++) ...[
          if (i > 0) const SizedBox(height: 14),
          items[i],
        ],
      ],
    );
    // Phone: [footer] (common.back) stands in the fixed bottom action area, always visible, the
    // content scrolls above it, also when the content is short (spec 15.14 N104). Laptop: it
    // follows the content.
    final scroll = CustomScrollView(
      slivers: [
        SliverPadding(
          padding: EdgeInsets.fromLTRB(pad.left, pad.top, pad.right, 0),
          sliver: SliverToBoxAdapter(child: spaced(header)),
        ),
        SliverPadding(
          padding: EdgeInsets.fromLTRB(pad.left, 8, pad.right, 0),
          sliver: SliverList.separated(
            itemCount: itemCount,
            itemBuilder: itemBuilder,
            separatorBuilder: (_, _) => const SizedBox(height: 8),
          ),
        ),
        if (s.laptop)
          SliverPadding(
            padding: EdgeInsets.fromLTRB(pad.left, 24, pad.right, pad.bottom),
            sliver: SliverToBoxAdapter(
              child: Align(alignment: Alignment.centerLeft, child: footer),
            ),
          )
        else
          const SliverToBoxAdapter(child: SizedBox(height: 24)),
      ],
    );
    return SafeArea(
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 744),
          child: s.laptop
              ? scroll
              : Column(
                  children: [
                    Expanded(child: scroll),
                    Padding(
                      key: const Key('scroll-page-actions'),
                      padding: EdgeInsets.fromLTRB(
                        pad.left,
                        0,
                        pad.right,
                        pad.bottom,
                      ),
                      child: SizedBox(width: double.infinity, child: footer),
                    ),
                  ],
                ),
        ),
      ),
    );
  }
}

/// One list row of the licences (option-row look, min 48, padding 8 14).
class LicenceRow extends StatelessWidget {
  const LicenceRow({super.key, required this.entry, required this.onPressed});
  final LicenceEntry entry;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    return PTap(
      onPressed: onPressed,
      radius: 12,
      builder: (_) => Container(
        constraints: const BoxConstraints(minHeight: 48),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: p.surface,
          border: Border.all(width: 1, color: p.borderInput),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              entry.name,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: s.type.optionTitle.copyWith(fontSize: 15),
            ),
            if (entry.licence.isNotEmpty)
              Text(
                entry.licence,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: s.type.optionSub,
              ),
          ],
        ),
      ),
    );
  }
}

class AboutPage extends StatelessWidget {
  const AboutPage({
    super.key,
    required this.version,
    required this.licences,
    required this.search,
    required this.onQuery,
    required this.onOpenLicence,
    required this.onOpenUrl,
    required this.onBack,
    required this.languageCode,
  });

  final String version;

  /// Null while the list is being read.
  final LicenceSet? licences;
  final TextEditingController search;
  final ValueChanged<String> onQuery;
  final ValueChanged<LicenceEntry> onOpenLicence;
  final ValueChanged<Uri> onOpenUrl;
  final VoidCallback onBack;

  /// `de` or `en` (the language of the page: privacy and imprint links).
  final String languageCode;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final t = AppLocalizations.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final de = languageCode == 'de';
    final b = BrandingInfo.current;
    final privacy = de ? b.privacyUrlDe : b.privacyUrlEn;
    final imprint = de ? b.imprintUrlDe : b.imprintUrlEn;
    final source = b.sourceUrl;
    void open(String url) {
      final u = Uri.tryParse(url);
      if (u != null) onOpenUrl(u);
    }

    final links = <Widget>[
      if (source.isNotEmpty)
        PTextLink(
          key: const Key('about-source'),
          label: t.aboutSource,
          left: true,
          onPressed: () => open('$source/tree/v$version'),
        ),
      if (privacy.isNotEmpty)
        PTextLink(
          key: const Key('about-privacy'),
          label: t.aboutPrivacy,
          left: true,
          onPressed: () => open(privacy),
        ),
      if (imprint.isNotEmpty)
        PTextLink(
          key: const Key('about-imprint'),
          label: t.aboutImprint,
          left: true,
          onPressed: () => open(imprint),
        ),
    ];
    final set = licences;
    final query = search.text;
    final featured = set == null
        ? <LicenceEntry>[]
        : filterLicences(set.featured, query);
    final all = set == null ? <LicenceEntry>[] : filterLicences(set.all, query);
    // One flat list for the lazy builder: group labels and rows.
    final items = <Object>[
      if (featured.isNotEmpty) 'about.featured',
      ...featured,
      if (all.isNotEmpty) 'about.all',
      ...all,
    ];
    return PScrollPage(
      header: [
        Text(t.aboutTitle, style: s.type.heading),
        PCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  ExcludeSemantics(
                    child: SvgPicture.asset(
                      dark
                          ? 'assets/logo/cometcast-logo-dark.svg'
                          : 'assets/logo/cometcast-logo-light.svg',
                      width: 28,
                      height: 28,
                    ),
                  ),
                  const SizedBox(width: 10),
                  Text(t.appTitle, style: s.type.optionTitle),
                ],
              ),
              const SizedBox(height: 6),
              Text.rich(
                TextSpan(
                  children: valueSpans(
                    t.aboutVersion(spanMarker),
                    version,
                    s.type.body.copyWith(
                      fontFamily: fontMono,
                      fontWeight: FontWeight.w500,
                      color: p.fg2,
                    ),
                  ),
                ),
                key: const Key('about-version'),
                style: s.type.body.copyWith(color: p.fg2),
              ),
              const SizedBox(height: 6),
              Text(
                t.aboutLicence,
                key: const Key('about-licence'),
                style: s.type.body.copyWith(color: p.fg2),
              ),
              if (b.publisherName.isNotEmpty) ...[
                const SizedBox(height: 6),
                Text(
                  t.aboutPublisher(b.publisherName),
                  key: const Key('about-publisher'),
                  style: s.type.small,
                ),
              ],
            ],
          ),
        ),
        if (links.isNotEmpty)
          Column(
            // In-content links are left-aligned in both layouts (N104).
            crossAxisAlignment: CrossAxisAlignment.start,
            children: links,
          ),
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Semantics(
              header: true,
              child: Text(t.aboutLicences, style: s.type.label),
            ),
            Text(t.aboutLicencesSub, style: s.type.small),
          ],
        ),
        PField(
          fieldKey: const Key('licence-search'),
          controller: search,
          hint: t.aboutSearch,
          leading: ExcludeSemantics(
            child: CcIcon(CcIconName.search, color: p.fg3, size: 18),
          ),
          onChanged: onQuery,
          textInputAction: TextInputAction.search,
        ),
        if (set != null && items.isEmpty)
          Text(
            t.aboutNone,
            key: const Key('about-none'),
            style: s.type.body.copyWith(color: p.fg2),
          ),
      ],
      itemCount: items.length,
      itemBuilder: (context, i) {
        final item = items[i];
        if (item is String) {
          return Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Semantics(
              header: true,
              child: Text(
                item == 'about.featured' ? t.aboutFeatured : t.aboutAll,
                style: s.type.label,
              ),
            ),
          );
        }
        final e = item as LicenceEntry;
        return LicenceRow(
          key: ValueKey('licence-${e.featured ? 'f' : 'a'}-${e.name}'),
          entry: e,
          onPressed: () => onOpenLicence(e),
        );
      },
      footer: PSecondaryButton(
        key: const Key('about-back'),
        label: t.commonBack,
        onPressed: onBack,
      ),
    );
  }
}

/// The licence text page: heading = the entry name, the text in 13 mono 500 `p-fg2`, line height
/// 1.4, selectable, wrapping; [common.back]. The page scrolls as a whole (lazy by paragraphs).
class LicenceTextPage extends StatelessWidget {
  const LicenceTextPage({
    super.key,
    required this.name,
    required this.text,
    required this.onBack,
  });

  final String name;

  /// Null while the text is read (Flutter's own licences).
  final String? text;
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    const mono = fontMono;
    final style = TextStyle(
      fontFamily: mono,
      fontSize: 13,
      fontWeight: FontWeight.w500,
      height: 1.4,
      color: s.tokens.fg2,
    );
    // Long licence texts are cut at blank lines so the list builds only what is visible.
    final blocks = (text ?? '').split(RegExp(r'\n{2,}'));
    return PScrollPage(
      header: [
        Text(name, key: const Key('licence-title'), style: s.type.heading),
      ],
      itemCount: text == null ? 0 : blocks.length,
      itemBuilder: (context, i) => SelectableText(
        blocks[i],
        key: i == 0 ? const Key('licence-text') : null,
        style: style,
      ),
      footer: PSecondaryButton(
        key: const Key('licence-back'),
        label: t.commonBack,
        onPressed: onBack,
      ),
    );
  }
}
