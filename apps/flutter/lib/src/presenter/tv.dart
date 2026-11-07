// SPDX-License-Identifier: GPL-3.0-or-later
// TVs without Comet Cast in the presenter app (spec 15.5, fix list F10): the rows in the nearby
// card, the pair page and the TV variants of "What to show". The flow is in presenter_root.dart.

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../core/core_api.dart';
import '../../tv_controller.dart';
import '../beta.dart';
import '../design/icons.dart';
import 'kit.dart';

/// Quoted device name, as `room_name` is shown (`common.quoted`, or `common.noName`).
String quotedTvName(AppLocalizations t, String name) =>
    name.trim().isEmpty ? t.commonNoName : t.commonQuoted(name.trim());

String tvWay(AppLocalizations t, TvKind k) =>
    k == TvKind.cast ? t.tvCast : t.tvAirplay;

/// The part of the nearby card for TVs: label `tv.title` and one row per TV. Nothing at all
/// (no label, no placeholder) when the list is empty.
class TvSection extends StatelessWidget {
  const TvSection({super.key, required this.controller, required this.onTap});

  final TvController controller;
  final void Function(TvRow row) onTap;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final rows = controller.rows;
        if (rows.isEmpty) return const SizedBox.shrink();
        final s = PresenterScope.of(context);
        final t = AppLocalizations.of(context);
        return Padding(
          padding: const EdgeInsets.only(top: 14),
          child: Column(
            key: const Key('tv-section'),
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Semantics(
                header: true,
                child: Text(t.tvTitle, style: s.type.label),
              ),
              const SizedBox(height: 8),
              for (var i = 0; i < rows.length; i++) ...[
                if (i > 0) const SizedBox(height: 8),
                TvRowTile(
                  key: Key('tv-row-${rows[i].id}'),
                  row: rows[i],
                  connecting: controller.connectingId == rows[i].id,
                  othersBusy: controller.connectingId != null,
                  onPressed: () {
                    if (controller.canTap(rows[i])) onTap(rows[i]);
                  },
                ),
              ],
            ],
          ),
        );
      },
    );
  }
}

class TvRowTile extends StatelessWidget {
  const TvRowTile({
    super.key,
    required this.row,
    required this.connecting,
    required this.othersBusy,
    required this.onPressed,
  });

  final TvRow row;
  final bool connecting;
  final bool othersBusy;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final t = AppLocalizations.of(context);
    final title = quotedTvName(t, row.name);
    final way = tvWay(t, row.kind);
    final beta = isBeta(BetaFeature.otherTvs);
    final subtitle = connecting
        ? t.nearbyConnecting
        : (beta ? '$way · ${t.beta}' : way);
    return PTap(
      onPressed: othersBusy ? null : onPressed,
      radius: 12,
      label: beta ? t.tvAria(title, way) : '$title, $way',
      builder: (_) => Container(
        constraints: BoxConstraints(minHeight: s.laptop ? 50 : 58),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: s.laptop ? p.bg : p.surface,
          border: Border.all(width: 1, color: p.borderInput),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            CcIcon(CcIconName.screen, color: p.primary, size: 22),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: s.type.optionTitle.copyWith(height: 1.25),
                  ),
                  Text(subtitle, style: s.type.optionSub),
                ],
              ),
            ),
            // The right slot is empty (there is no code); the spinner sits at its right edge.
            if (connecting) const PSpinner(),
          ],
        ),
      ),
    );
  }
}

/// The pair page (spec 15.5), relay-page layout: icon disc, heading, body, field, [Pair, Back].
class TvPairPage extends StatelessWidget {
  const TvPairPage({
    super.key,
    required this.name,
    required this.controller,
    required this.onPair,
    required this.onBack,
    this.wrong = false,
    this.busy = false,
  });

  /// The TV's name, already quoted.
  final String name;
  final TextEditingController controller;
  final VoidCallback onPair;
  final VoidCallback onBack;
  final bool wrong;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actionsRow: true,
      actions: [
        PPrimaryButton(
          key: const Key('pair-go'),
          label: t.pairGo,
          onPressed: busy ? null : onPair,
        ),
        PSecondaryButton(
          key: const Key('pair-back'),
          label: t.commonBack,
          onPressed: onBack,
        ),
      ],
      children: [
        const Align(
          alignment: Alignment.centerLeft,
          child: PIconDisc(icon: CcIconName.screen, tone: PTone.acc),
        ),
        Text(
          t.pairTitle(name),
          key: const Key('pair-title'),
          style: s.type.heading,
        ),
        Text(t.pairBody, style: s.type.body),
        PField(
          fieldKey: const Key('pair-field'),
          label: t.pairLabel,
          controller: controller,
          code: true,
          digitsOnly: true,
          maxLength: 4,
          keyboardType: TextInputType.number,
          error: wrong,
          errorText: wrong ? t.pairWrong : null,
          textInputAction: TextInputAction.done,
          autofocus: true,
          onSubmitted: (_) => busy ? null : onPair(),
        ),
      ],
    );
  }
}
