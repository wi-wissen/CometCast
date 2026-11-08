// SPDX-License-Identifier: GPL-3.0-or-later
// About on the display (spec 15.9, fix list F4): the dialog opened from Settings and the licence
// viewer (list and text pane). D-pad: Up and Down in the list, Right to the pane, Up and Down
// there scroll by half a pane, Left back to the list, Back closes. Search on tablets and
// computers only.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../licences.dart';
import '../branding_info.dart';
import '../design/icons.dart';
import '../design/spans.dart';
import '../design/tokens.dart';
import 'kit.dart';

/// `github.com/wi-wissen/CometCast` from the branding file's source URL.
String aboutSourceText() =>
    BrandingInfo.current.sourceUrl.replaceFirst(RegExp(r'^https?://'), '');

/// The host of the privacy page (`cometcast.de`), or null when the build has no privacy URL
/// (spec 15.14 N108: `about.webTv` only then).
String? aboutWebText() {
  final b = BrandingInfo.current;
  final u = Uri.tryParse(
    b.privacyUrlDe.isNotEmpty ? b.privacyUrlDe : b.privacyUrlEn,
  );
  final h = u?.host ?? '';
  return h.isEmpty ? null : h;
}

/// The About dialog: link-card family (600 card over the scrim), buttons [Licences, Close].
class AboutDialogView extends StatelessWidget {
  const AboutDialogView({
    super.key,
    required this.version,
    this.idTag,
    required this.remote,
    required this.onLicences,
    required this.onClose,
    this.licencesFocus,
  });

  final String version;

  /// The short tag of this display's id (`display.tag`), or null.
  final String? idTag;
  final bool remote;
  final VoidCallback onLicences;
  final VoidCallback onClose;
  final FocusNode? licencesFocus;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    final mono = s.type.cardText.copyWith(
      fontFamily: fontMono,
      fontSize: 16,
      color: s.tokens.fg,
    );
    Widget line(String text, {Key? key}) =>
        Text(text, key: key, style: s.type.cardText);
    Widget valueLine(String template, String value, Key key) => Text.rich(
      TextSpan(children: valueSpans(template, value, mono)),
      key: key,
      style: s.type.cardText,
    );
    final web = aboutWebText();
    return DScrim(
      child: Semantics(
        scopesRoute: true,
        namesRoute: true,
        explicitChildNodes: true,
        label: t.aboutTitle,
        child: DCard(
          width: 600,
          shadow: true,
          children: [
            DSectionLabel(t.aboutTitle),
            Row(
              children: [
                const DLogo(size: 40),
                const SizedBox(width: 14),
                Text(t.appTitle, style: s.type.choiceTitle),
              ],
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                valueLine(
                  t.aboutVersion(spanMarker),
                  version,
                  const Key('about-version'),
                ),
                if (idTag != null) ...[
                  const SizedBox(height: 4),
                  line(t.displayTag(idTag!), key: const Key('about-tag')),
                ],
                const SizedBox(height: 4),
                line(t.aboutLicence, key: const Key('about-licence')),
                if (BrandingInfo.current.publisherName.isNotEmpty) ...[
                  const SizedBox(height: 4),
                  line(
                    t.aboutPublisher(BrandingInfo.current.publisherName),
                    key: const Key('about-publisher'),
                  ),
                ],
                if (BrandingInfo.current.sourceUrl.isNotEmpty) ...[
                  const SizedBox(height: 4),
                  valueLine(
                    t.aboutSourceTv(spanMarker),
                    aboutSourceText(),
                    const Key('about-source'),
                  ),
                ],
                if (web != null) ...[
                  const SizedBox(height: 4),
                  valueLine(
                    t.aboutWebTv(spanMarker),
                    web,
                    const Key('about-web'),
                  ),
                ],
              ],
            ),
            Row(
              children: [
                DButton(
                  key: const Key('about-licences'),
                  label: t.aboutLicences,
                  row: 0,
                  col: 0,
                  autofocus: true,
                  focusNode: licencesFocus,
                  onPressed: onLicences,
                ),
                const SizedBox(width: 12),
                DButton(
                  key: const Key('about-close'),
                  label: remote ? t.settingsCloseRemote : t.settingsClose,
                  row: 0,
                  col: 1,
                  onPressed: onClose,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

// --- licence viewer -------------------------------------------------------------------------

/// Full canvas on `--bg`: the top row (label left, Close right), then two columns: the list
/// (300 wide) and the text pane. [search] false on TVs.
class LicenceViewer extends StatefulWidget {
  const LicenceViewer({
    super.key,
    required this.licences,
    required this.onClose,
    required this.search,
    required this.remote,
  });

  /// Null while the list is being read.
  final LicenceSet? licences;
  final VoidCallback onClose;
  final bool search;
  final bool remote;

  @override
  State<LicenceViewer> createState() => _LicenceViewerState();
}

class _LicenceViewerState extends State<LicenceViewer> {
  final _query = TextEditingController();
  final _scroll = ScrollController();
  final _paneScroll = ScrollController();
  late final FocusNode _pane = FocusNode(
    debugLabel: 'licence:pane',
    onKeyEvent: _paneKey,
  );
  late final RowNode _searchNode = RowNode(
    row: 1,
    col: 0,
    debugLabel: 'licence:search',
    onKeyEvent: _searchKey,
  );
  final Map<String, RowNode> _nodes = {};
  RowNode? _lastRow;
  LicenceEntry? _shown;
  String? _shownText;
  bool _paneFocused = false;

  @override
  void initState() {
    super.initState();
    _pane.addListener(() {
      if (mounted && _paneFocused != _pane.hasFocus) {
        setState(() => _paneFocused = _pane.hasFocus);
      }
    });
    _query.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _query.dispose();
    _scroll.dispose();
    _paneScroll.dispose();
    _pane.dispose();
    _searchNode.dispose();
    for (final n in _nodes.values) {
      n.dispose();
    }
    super.dispose();
  }

  KeyEventResult _paneKey(FocusNode node, KeyEvent e) {
    if (e is KeyUpEvent) return KeyEventResult.ignored;
    if (e is KeyDownEvent &&
        (DisplayScope.maybeOf(context)?.keyGate?.call(e) ?? false)) {
      return KeyEventResult.handled;
    }
    final k = e.logicalKey;
    if (k == LogicalKeyboardKey.arrowLeft) {
      _lastRow?.requestFocus();
      return KeyEventResult.handled;
    }
    if (k == LogicalKeyboardKey.arrowUp || k == LogicalKeyboardKey.arrowDown) {
      if (_paneScroll.hasClients) {
        final half = _paneScroll.position.viewportDimension / 2;
        final to =
            (_paneScroll.offset +
                    (k == LogicalKeyboardKey.arrowDown ? half : -half))
                .clamp(0.0, _paneScroll.position.maxScrollExtent);
        _paneScroll.jumpTo(to);
      }
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  KeyEventResult _searchKey(FocusNode node, KeyEvent e) {
    if (e is KeyUpEvent) return KeyEventResult.ignored;
    final k = e.logicalKey;
    final dir = k == LogicalKeyboardKey.arrowUp
        ? TraversalDirection.up
        : k == LogicalKeyboardKey.arrowDown
        ? TraversalDirection.down
        : null;
    if (dir != null) {
      node.focusInDirection(dir);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  RowNode _nodeFor(String key, int row) =>
      _nodes.putIfAbsent(key, () => RowNode(row: row, col: 0, debugLabel: key));

  Future<void> _show(LicenceEntry e) async {
    if (identical(_shown, e)) return;
    _shown = e;
    final lazy = e.lazyText;
    setState(() => _shownText = lazy == null ? e.text : null);
    if (_paneScroll.hasClients) _paneScroll.jumpTo(0);
    if (lazy != null) {
      final text = await lazy();
      if (mounted && identical(_shown, e)) setState(() => _shownText = text);
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    final tok = s.tokens;
    final set = widget.licences;
    final q = _query.text;
    final featured = set == null
        ? <LicenceEntry>[]
        : filterLicences(set.featured, q);
    final all = set == null ? <LicenceEntry>[] : filterLicences(set.all, q);
    final items = <Object>[
      if (featured.isNotEmpty) 'about.featured',
      ...featured,
      if (all.isNotEmpty) 'about.all',
      ...all,
    ];
    final entries = items.whereType<LicenceEntry>().toList();
    if (entries.isNotEmpty && (_shown == null || !entries.contains(_shown))) {
      final first = entries.first;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _show(first);
      });
    }
    final shown = entries.contains(_shown) ? _shown : null;
    final list = ListView.builder(
      controller: _scroll,
      itemCount: items.length,
      itemBuilder: (context, i) {
        final item = items[i];
        if (item is String) {
          return Padding(
            padding: const EdgeInsets.only(top: 10, bottom: 4),
            child: DSectionLabel(
              item == 'about.featured' ? t.aboutFeatured : t.aboutAll,
            ),
          );
        }
        final e = item as LicenceEntry;
        // The row number follows the item index: lazily built rows keep their place.
        final r = 2 + i;
        final key = '${e.featured ? 'f' : 'a'}:${e.name}';
        final node = _nodeFor('$key@$r', r);
        return Padding(
          padding: const EdgeInsets.only(bottom: 2),
          child: _LicenceRowView(
            key: ValueKey('licence-row-$key'),
            entry: e,
            node: node,
            selected: identical(shown, e),
            autofocus: i == (featured.isNotEmpty ? 1 : 0),
            onFocused: () {
              _lastRow = node;
              _show(e);
            },
            onTap: () {
              _lastRow = node;
              _show(e);
            },
            onRight: () => _pane.requestFocus(),
          ),
        );
      },
    );
    final text = _shownText;
    final blocks = (text ?? '').split(RegExp(r'\n{2,}'));
    final paneBox = Container(
      key: const Key('licence-pane'),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: tok.panel,
        borderRadius: BorderRadius.circular(14),
        // Always the same 1 px border: the text must not move when the pane gains or loses
        // focus (N100).
        border: Border.all(width: 1, color: tok.line),
      ),
      child: shown == null
          ? Text(
              set != null && entries.isEmpty ? t.aboutNone : '',
              key: const Key('licence-none'),
              style: s.type.cardText,
            )
          : ListView.builder(
              controller: _paneScroll,
              itemCount: text == null ? 0 : blocks.length,
              itemBuilder: (context, i) => Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: SelectableText(
                  blocks[i],
                  key: i == 0 ? const Key('licence-text') : null,
                  style: TextStyle(
                    fontFamily: fontMono,
                    fontSize: 13,
                    fontWeight: FontWeight.w500,
                    height: 1.4,
                    color: tok.fg,
                  ),
                ),
              ),
            ),
    );
    final pane = Focus(
      focusNode: _pane,
      child: Stack(
        clipBehavior: Clip.none,
        fit: StackFit.passthrough,
        children: [
          paneBox,
          // The focus ring of 3.5, drawn outside the pane's border box.
          if (_paneFocused) ...[
            Positioned(
              left: -9,
              top: -9,
              right: -9,
              bottom: -9,
              child: IgnorePointer(
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(14 + 9),
                    border: Border.all(
                      width: 9,
                      color: DisplayTokens.focusGlow,
                    ),
                  ),
                ),
              ),
            ),
            Positioned(
              key: const Key('licence-pane-ring'),
              left: -7,
              top: -7,
              right: -7,
              bottom: -7,
              child: IgnorePointer(
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(14 + 7),
                    border: Border.all(width: 4, color: tok.focus),
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
    return Container(
      color: tok.bg,
      child: Semantics(
        scopesRoute: true,
        namesRoute: true,
        explicitChildNodes: true,
        label: t.aboutLicences,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(40, 26, 40, 18),
          child: Column(
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  DSectionLabel(t.aboutLicences),
                  DCloseButton(
                    key: const Key('licences-close'),
                    label: widget.remote
                        ? t.settingsCloseRemote
                        : t.settingsClose,
                    row: 0,
                    col: 0,
                    onPressed: widget.onClose,
                  ),
                ],
              ),
              const SizedBox(height: 14),
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SizedBox(
                      width: 300,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          if (widget.search) ...[
                            _SearchField(
                              controller: _query,
                              node: _searchNode,
                              hint: t.aboutSearch,
                            ),
                            const SizedBox(height: 8),
                          ],
                          Expanded(child: list),
                        ],
                      ),
                    ),
                    const SizedBox(width: 20),
                    Expanded(child: pane),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _LicenceRowView extends StatefulWidget {
  const _LicenceRowView({
    super.key,
    required this.entry,
    required this.node,
    required this.selected,
    required this.autofocus,
    required this.onFocused,
    required this.onTap,
    required this.onRight,
  });

  final LicenceEntry entry;
  final RowNode node;
  final bool selected;
  final bool autofocus;
  final VoidCallback onFocused;
  final VoidCallback onTap;
  final VoidCallback onRight;

  @override
  State<_LicenceRowView> createState() => _LicenceRowViewState();
}

class _LicenceRowViewState extends State<_LicenceRowView> {
  bool _had = false;

  void _listen() {
    // The pane shows the focused entry (spec 15.9).
    if (widget.node.hasFocus && !_had) widget.onFocused();
    _had = widget.node.hasFocus;
  }

  @override
  void initState() {
    super.initState();
    widget.node.addListener(_listen);
  }

  @override
  void dispose() {
    widget.node.removeListener(_listen);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    final entry = widget.entry;
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onKeyEvent: (n, e) {
        if (e is KeyDownEvent &&
            e.logicalKey == LogicalKeyboardKey.arrowRight) {
          widget.onRight();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: DisplayControl(
        row: widget.node.row,
        col: 0,
        radius: 8,
        scale: false,
        autofocus: widget.autofocus,
        focusNode: widget.node,
        label: '${entry.name}, ${entry.licence}',
        onPressed: widget.onTap,
        child: Container(
          height: 40,
          // The entry the pane shows has the selected-tile look of 3.2: 2 px `--acc` border and
          // the fill (OLED black: border only); the border is always there so that the text
          // does not move (N100).
          padding: const EdgeInsets.symmetric(horizontal: 8),
          decoration: BoxDecoration(
            color: widget.selected ? t.selectedTileFill : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(
              width: 2,
              color: widget.selected ? t.acc : Colors.transparent,
            ),
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                entry.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: s.type.nameTag.copyWith(height: 1.15),
              ),
              if (entry.licence.isNotEmpty)
                Text(
                  entry.licence,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: s.type.nameTagSub.copyWith(height: 1.1),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _SearchField extends StatelessWidget {
  const _SearchField({
    required this.controller,
    required this.node,
    required this.hint,
  });

  final TextEditingController controller;
  final RowNode node;
  final String hint;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    return DisplayControl(
      row: node.row,
      col: 0,
      radius: 10,
      scale: false,
      externalFocus: true,
      focusNode: node,
      onPressed: () => node.requestFocus(),
      label: hint,
      child: Container(
        height: 46,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: t.bg,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(width: 2, color: t.line),
        ),
        child: Row(
          children: [
            ExcludeSemantics(
              child: CcIcon(CcIconName.search, color: t.mut, size: 18),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Stack(
                alignment: Alignment.centerLeft,
                children: [
                  ValueListenableBuilder<TextEditingValue>(
                    valueListenable: controller,
                    builder: (context, v, _) => v.text.isEmpty
                        ? IgnorePointer(
                            child: Text(
                              hint,
                              key: const Key('licence-search-hint'),
                              style: s.type.field.copyWith(
                                fontSize: 17,
                                color: t.mut,
                              ),
                            ),
                          )
                        : const SizedBox.shrink(),
                  ),
                  EditableText(
                    key: const Key('licence-search'),
                    controller: controller,
                    focusNode: node,
                    style: s.type.field.copyWith(fontSize: 17),
                    cursorColor: t.acc,
                    backgroundCursorColor: t.line,
                    maxLines: 1,
                    keyboardType: TextInputType.text,
                    textInputAction: TextInputAction.search,
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
