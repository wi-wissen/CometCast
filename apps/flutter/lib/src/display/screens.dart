// SPDX-License-Identifier: GPL-3.0-or-later
// The display screens after setup: idle (spec 4.2), Settings (4.3), join request (4.4), link
// card and the controls of a presentation (4.5), toast (4.6). Plain widgets: the state lives in
// display_root.dart.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:qr_flutter/qr_flutter.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../design/icons.dart';
import '../design/spans.dart';
import '../design/tokens.dart';
import 'display_logic.dart';
import 'kit.dart';

String modeWord(AppLocalizations t, DisplayJoinMode m) => switch (m) {
  DisplayJoinMode.open => t.modeOpen,
  DisplayJoinMode.confirm => t.modeApproval,
  DisplayJoinMode.locked => t.modeLocked,
};

CcIconName modeIcon(DisplayJoinMode m) => switch (m) {
  DisplayJoinMode.open => CcIconName.open,
  DisplayJoinMode.confirm => CcIconName.approval,
  DisplayJoinMode.locked => CcIconName.locked,
};

/// Who may join, as the display shows it (a copy of the core's enum so these widgets do not
/// depend on the generated bridge classes).
enum DisplayJoinMode { open, confirm, locked }

// --- idle -------------------------------------------------------------------------------

class IdleScreen extends StatelessWidget {
  const IdleScreen({
    super.key,
    required this.room,
    required this.mode,
    required this.code,
    required this.dom,
    required this.view,
    required this.cause,
    required this.dimmed,
    required this.onSettings,
    this.altAddress,
    this.appLineStores = false,
    this.settingsFocus,
    this.settingsAutofocus = true,
    this.ringSuppressed = false,
  });

  final String room;
  final DisplayJoinMode mode;

  /// The join code (6 characters), or null while there is none.
  final String? code;

  /// `host[:port]` of the server.
  final String dom;
  final ConnectionView view;
  final OfflineCause cause;
  final bool dimmed;
  final VoidCallback onSettings;

  /// The IP address of an in-house box (`idle.altAddress`), or null.
  final String? altAddress;

  /// A store or download link exists: the footer names them (`idle.appLine`); else
  /// `idle.appLineWeb`.
  final bool appLineStores;
  final FocusNode? settingsFocus;
  final bool settingsAutofocus;

  /// A dialog is over the screen: the Settings button draws no ring (spec 7.1: one ring).
  final bool ringSuppressed;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final tok = s.tokens;
    final t = AppLocalizations.of(context);
    final locked = mode == DisplayJoinMode.locked;
    final offline = view == ConnectionView.offline;
    final online = view == ConnectionView.online && code != null;
    // Empty QR box while connecting or offline: an old code must never be scannable.
    final qrEmpty = !online;
    final url = online ? joinUrl(dom, code!) : '';
    final host = displayHost(dom);
    return Stack(
      fit: StackFit.expand,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(40, 26, 40, 18),
          child: Column(
            children: [
              Row(
                children: [
                  const DLogo(size: 46),
                  const SizedBox(width: 14),
                  Expanded(
                    child: Text(
                      room,
                      key: const Key('room-name'),
                      style: s.type.roomName,
                    ),
                  ),
                  _Pill(mode: mode),
                ],
              ),
              const SizedBox(height: 14),
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    Expanded(
                      child: offline
                          ? Center(
                              child: _OfflineBox(dom: host, cause: cause),
                            )
                          : _Steps(
                              dom: host,
                              code: online ? code! : null,
                              locked: locked,
                              altAddress: altAddress,
                            ),
                    ),
                    const SizedBox(width: 36),
                    SizedBox(
                      width: 236,
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          _QrBox(
                            url: url,
                            empty: qrEmpty,
                            dim: locked && !qrEmpty,
                          ),
                          const SizedBox(height: 8),
                          Visibility(
                            visible: !qrEmpty,
                            maintainSize: true,
                            maintainAnimation: true,
                            maintainState: true,
                            child: Text(
                              locked ? t.idleQrLocked : t.idleQrCaption,
                              key: const Key('qr-caption'),
                              textAlign: TextAlign.center,
                              style: s.type.footer,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 14),
              Row(
                children: [
                  Expanded(
                    child: Text(
                      appLineStores ? t.idleAppLine(host) : t.idleAppLineWeb,
                      key: const Key('idle-foot'),
                      style: s.type.footer,
                    ),
                  ),
                  const SizedBox(width: 16),
                  DisplayControl(
                    row: 0,
                    col: 0,
                    radius: 10,
                    autofocus: settingsAutofocus,
                    suppressRing: ringSuppressed,
                    focusNode: settingsFocus,
                    onPressed: onSettings,
                    child: Container(
                      key: const Key('settings-open'),
                      constraints: const BoxConstraints(minHeight: 46),
                      padding: const EdgeInsets.fromLTRB(12, 0, 16, 0),
                      decoration: BoxDecoration(
                        color: tok.panel,
                        borderRadius: BorderRadius.circular(10),
                        border: Border.all(width: 2, color: tok.line),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          CcIcon(CcIconName.gear, color: tok.fg, size: 20),
                          const SizedBox(width: 8),
                          Text(t.settingsOpen, style: s.type.label17Bold),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        // Idle dimming: an overlay of --bg at 60 % over 2 s, restored in 200 ms (spec 8.6).
        Positioned.fill(
          child: IgnorePointer(
            child: AnimatedOpacity(
              key: const Key('dim'),
              opacity: dimmed ? 0.6 : 0,
              duration: s.reduceMotion
                  ? Duration.zero
                  : Duration(milliseconds: dimmed ? 2000 : 200),
              curve: Curves.easeOut,
              child: ColoredBox(color: tok.bg),
            ),
          ),
        ),
      ],
    );
  }
}

class _Pill extends StatelessWidget {
  const _Pill({required this.mode});
  final DisplayJoinMode mode;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    return Container(
      key: const Key('mode-pill'),
      padding: const EdgeInsets.fromLTRB(10, 7, 14, 7),
      decoration: BoxDecoration(
        color: s.tokens.panel2,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(width: 1, color: s.tokens.line),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          CcIcon(modeIcon(mode), color: s.tokens.acc, size: 20),
          const SizedBox(width: 8),
          Text(modeWord(t, mode), style: s.type.pill),
        ],
      ),
    );
  }
}

class _Steps extends StatelessWidget {
  const _Steps({
    required this.dom,
    required this.code,
    required this.locked,
    required this.altAddress,
  });
  final String dom;
  final String? code;
  final bool locked;
  final String? altAddress;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    Widget step(String n, String label, Widget value) => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          margin: const EdgeInsets.only(top: 4),
          width: 38,
          height: 38,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: s.tokens.acc,
            shape: BoxShape.circle,
          ),
          child: Text(n, style: s.type.stepNumber),
        ),
        const SizedBox(width: 16),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(label, style: s.type.stepLabel),
              value,
            ],
          ),
        ),
      ],
    );
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        step(
          '1',
          t.idleStep1,
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(dom, key: const Key('address'), style: s.type.address),
              if (altAddress != null)
                Text(
                  t.idleAltAddress(altAddress!),
                  key: const Key('alt-address'),
                  style: s.type.stepLabel.copyWith(fontSize: 16),
                ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        step(
          '2',
          t.idleStep2,
          // The code line keeps its height while there is no code.
          SizedBox(
            height: 80,
            child: Opacity(
              opacity: locked ? .35 : 1,
              child: Text(
                code == null ? ' ' : formatCode(code!),
                key: const Key('room-code'),
                style: s.type.code,
                softWrap: false,
              ),
            ),
          ),
        ),
        if (locked) ...[
          const SizedBox(height: 12),
          Padding(
            padding: const EdgeInsets.only(left: 54),
            child: Text(
              t.idleLocked,
              key: const Key('locked-note'),
              style: s.type.stepLabel,
            ),
          ),
        ],
      ],
    );
  }
}

class _OfflineBox extends StatelessWidget {
  const _OfflineBox({required this.dom, required this.cause});
  final String dom;
  final OfflineCause cause;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    final body = switch (cause) {
      OfflineCause.busy => t.idleOfflineBodyBusy,
      OfflineCause.outdated => t.idleOfflineBodyOutdatedApp,
      OfflineCause.network => t.idleOfflineBody,
    };
    // A box of its content height; the dashes are drawn over the fill (foregroundPainter).
    return CustomPaint(
      foregroundPainter: _DashedRRect(s.tokens.line, 16),
      child: Container(
        key: const Key('offline'),
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
        decoration: BoxDecoration(
          color: s.tokens.panel,
          borderRadius: BorderRadius.circular(16),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            CcIcon(CcIconName.wifiOff, color: s.tokens.acc, size: 34),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(t.idleOfflineTitle(dom), style: s.type.offlineTitle),
                  const SizedBox(height: 4),
                  Text(
                    body,
                    key: const Key('offline-body'),
                    style: s.type.offlineBody,
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

/// The dashed 2 px border of the offline box.
class _DashedRRect extends CustomPainter {
  _DashedRRect(this.color, this.radius);
  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..color = color;
    final path = Path()
      ..addRRect(
        RRect.fromRectAndRadius(
          Offset.zero & size,
          Radius.circular(radius),
        ).deflate(1),
      );
    for (final m in path.computeMetrics()) {
      var d = 0.0;
      while (d < m.length) {
        canvas.drawPath(m.extractPath(d, d + 6), paint);
        d += 10;
      }
    }
  }

  @override
  bool shouldRepaint(_DashedRRect old) =>
      old.color != color || old.radius != radius;
}

/// QR box: 212 x 212, padding 10, radius 14. With a code it has the `--qrbg` fill (Light: a
/// 1 px `--line` border too); empty (connecting, offline): `--panel2` fill, 1 px `--line`
/// border, full opacity, no modules (spec 3.2, decision 13.33). Locked: modules at 25 %.
class _QrBox extends StatelessWidget {
  const _QrBox({required this.url, required this.empty, required this.dim});
  final String url;
  final bool empty;
  final bool dim;

  @override
  Widget build(BuildContext context) {
    final t = DisplayScope.of(context).tokens;
    final border = empty || t.light;
    final box = Container(
      key: const Key('qr'),
      width: 212,
      height: 212,
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: empty ? t.panel2 : t.qrbg,
        borderRadius: BorderRadius.circular(14),
        border: border ? Border.all(width: 1, color: t.line) : null,
      ),
      child: empty
          ? null
          : Semantics(
              label: 'QR: $url',
              image: true,
              child: CustomPaint(
                size: const Size.square(192),
                painter: QrPainter(
                  data: url,
                  version: QrVersions.auto,
                  errorCorrectionLevel: QrErrorCorrectLevel.M,
                  gapless: true,
                  eyeStyle: const QrEyeStyle(
                    eyeShape: QrEyeShape.square,
                    color: DisplayTokens.qrModule,
                  ),
                  dataModuleStyle: const QrDataModuleStyle(
                    dataModuleShape: QrDataModuleShape.square,
                    color: DisplayTokens.qrModule,
                  ),
                ),
              ),
            ),
    );
    return dim ? Opacity(opacity: .25, child: box) : box;
  }
}

// --- settings ---------------------------------------------------------------------------

class SettingsDialog extends StatelessWidget {
  const SettingsDialog({
    super.key,
    required this.mode,
    required this.appearance,
    required this.remote,
    required this.onMode,
    required this.onAppearance,
    required this.onClose,
  });

  final DisplayJoinMode mode;
  final Appearance appearance;

  /// A TV remote: close label `settings.closeRemote`.
  final bool remote;
  final ValueChanged<DisplayJoinMode> onMode;
  final ValueChanged<Appearance> onAppearance;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    final desc = switch (mode) {
      DisplayJoinMode.open => t.settingsJoinOpenText,
      DisplayJoinMode.confirm => t.settingsJoinApprovalText,
      DisplayJoinMode.locked => t.settingsJoinLockedText,
    };
    final looks = {
      Appearance.dark: t.appearanceDark,
      Appearance.light: t.appearanceLight,
      Appearance.oled: t.appearanceOled,
      Appearance.auto: t.appearanceAuto,
    };
    return DScrim(
      child: Semantics(
        scopesRoute: true,
        namesRoute: true,
        explicitChildNodes: true,
        label: t.settingsTitle,
        child: DCard(
          width: 600,
          shadow: true,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                DSectionLabel(t.settingsTitle),
                DCloseButton(
                  key: const Key('settings-close'),
                  label: remote ? t.settingsCloseRemote : t.settingsClose,
                  row: 0,
                  col: 0,
                  onPressed: onClose,
                ),
              ],
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                DSectionLabel(t.settingsJoinLabel),
                const SizedBox(height: 14),
                Row(
                  children: [
                    for (final m in DisplayJoinMode.values) ...[
                      if (m != DisplayJoinMode.open) const SizedBox(width: 8),
                      Expanded(
                        child: _Tile(
                          key: Key('settings-mode-${m.name}'),
                          row: 1,
                          col: m.index,
                          height: 60,
                          selected: mode == m,
                          autofocus: mode == m,
                          column: true,
                          icon: modeIcon(m),
                          label: modeWord(t, m),
                          onPressed: () => onMode(m),
                        ),
                      ),
                    ],
                  ],
                ),
                const SizedBox(height: 14),
                Text(
                  desc,
                  key: const Key('settings-mode-desc'),
                  style: s.type.description,
                ),
              ],
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                DSectionLabel(t.settingsAppearance),
                const SizedBox(height: 14),
                Row(
                  children: [
                    for (final a in Appearance.values) ...[
                      if (a != Appearance.dark) const SizedBox(width: 6),
                      Expanded(
                        child: _Tile(
                          key: Key('settings-look-${a.name}'),
                          row: 2,
                          col: a.index,
                          height: 46,
                          selected: appearance == a,
                          label: looks[a]!,
                          small: true,
                          onPressed: () => onAppearance(a),
                        ),
                      ),
                    ],
                  ],
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// Tile of Settings: border 2, radius 12; selected: border `--acc`, bold, selected fill
/// (OLED black: no fill).
class _Tile extends StatelessWidget {
  const _Tile({
    super.key,
    required this.row,
    required this.col,
    required this.height,
    required this.selected,
    required this.label,
    required this.onPressed,
    this.icon,
    this.column = false,
    this.small = false,
    this.autofocus = false,
  });
  final int row;
  final int col;
  final double height;
  final bool selected;
  final bool column;
  final bool small;
  final bool autofocus;
  final CcIconName? icon;
  final String label;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    return DisplayControl(
      row: row,
      col: col,
      radius: 12,
      autofocus: autofocus,
      selected: selected,
      label: label,
      onPressed: onPressed,
      child: Container(
        height: height,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: selected ? t.selectedTileFill : t.panel2,
          borderRadius: BorderRadius.circular(12),
          border: Border.all(width: 2, color: selected ? t.acc : t.line),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              CcIcon(icon!, color: t.fg, size: 22),
              const SizedBox(height: 3),
            ],
            Text(
              label,
              style: small
                  ? s.type.lookTile(selected)
                  : s.type.modeTile(selected),
            ),
          ],
        ),
      ),
    );
  }
}

// --- join request -----------------------------------------------------------------------

/// The one-second guard of a dialog (spec 4.4): input is ignored until it has passed, counted
/// from when the dialog (or the next request) appeared.
class InputGuard extends StatefulWidget {
  const InputGuard({
    super.key,
    required this.identity,
    required this.builder,
    this.guard = requestGuard,
  });

  /// Changes when another request takes the dialog: the guard starts again.
  final Object identity;
  final Duration guard;
  final Widget Function(BuildContext context, bool armed) builder;

  @override
  State<InputGuard> createState() => _InputGuardState();
}

class _InputGuardState extends State<InputGuard> {
  bool _armed = false;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _start();
  }

  void _start() {
    _timer?.cancel();
    _armed = false;
    _timer = Timer(widget.guard, () {
      if (mounted) setState(() => _armed = true);
    });
  }

  @override
  void didUpdateWidget(InputGuard old) {
    super.didUpdateWidget(old);
    if (old.identity != widget.identity) _start();
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.builder(context, _armed);
}

class RequestDialog extends StatefulWidget {
  const RequestDialog({
    super.key,
    required this.id,
    required this.name,
    required this.check,
    required this.more,
    required this.onAllow,
    required this.onDeny,
    required this.onBlock,
    this.guard = requestGuard,
  });

  final Object id;
  final String name;
  final int check;

  /// Requests that wait behind this one.
  final int more;
  final VoidCallback onAllow;
  final VoidCallback onDeny;
  final VoidCallback onBlock;
  final Duration guard;

  @override
  State<RequestDialog> createState() => _RequestDialogState();
}

class _RequestDialogState extends State<RequestDialog> {
  final _yes = RowNode(row: 0, col: 0, debugLabel: 'request:yes');

  @override
  void didUpdateWidget(RequestDialog old) {
    super.didUpdateWidget(old);
    // After a decision the next request takes the dialog and focus returns to Yes.
    if (old.id != widget.id) _yes.requestFocus();
  }

  @override
  void dispose() {
    _yes.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    final full = stripName(widget.name);
    final shown = full.isEmpty
        ? t.commonNoName
        : t.commonQuoted(shortName(full));
    final aria = t.requestAria(
      full.isEmpty ? t.commonNoName : t.commonQuoted(full),
    );
    return InputGuard(
      identity: widget.id,
      guard: widget.guard,
      builder: (context, armed) {
        void guarded(VoidCallback f) {
          if (armed) f();
        }

        return DScrim(
          child: Semantics(
            scopesRoute: true,
            namesRoute: true,
            explicitChildNodes: true,
            label: aria,
            child: DCard(
              width: 600,
              shadow: true,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    DSectionLabel(t.requestTitle),
                    if (widget.more > 0)
                      Text(
                        t.requestQueue('${widget.more}'),
                        key: const Key('request-queue'),
                        style: s.type.description,
                      ),
                  ],
                ),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            shown,
                            key: const Key('request-name'),
                            style: s.type.heading,
                          ),
                          const SizedBox(height: 4),
                          Text(
                            t.requestSubtitle,
                            key: const Key('request-sub'),
                            style: s.type.body.copyWith(
                              fontSize: 18,
                              height: 1.3,
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 20),
                    Container(
                      constraints: const BoxConstraints(minWidth: 58),
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 3,
                      ),
                      decoration: BoxDecoration(
                        color: s.tokens.bg,
                        borderRadius: BorderRadius.circular(10),
                        // OLED black: a 1 px line border (spec 3.2).
                        border: s.tokens.oled
                            ? Border.all(width: 1, color: s.tokens.line)
                            : null,
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            t.requestCheck.toUpperCase(),
                            style: s.type.smallCaps,
                          ),
                          Text(
                            '${widget.check}',
                            key: const Key('request-check'),
                            style: s.type.checkNumber,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                Row(
                  children: [
                    DButton(
                      key: const Key('request-yes'),
                      label: t.requestYes,
                      kind: DButtonKind.primary,
                      row: 0,
                      col: 0,
                      autofocus: true,
                      focusNode: _yes,
                      onPressed: () => guarded(widget.onAllow),
                    ),
                    const SizedBox(width: 12),
                    DButton(
                      key: const Key('request-no'),
                      label: t.requestNo,
                      row: 0,
                      col: 1,
                      onPressed: () => guarded(widget.onDeny),
                    ),
                    const Spacer(),
                    DGhostButton(
                      key: const Key('request-block'),
                      label: t.requestNoBlock,
                      row: 0,
                      col: 2,
                      onPressed: () => guarded(widget.onBlock),
                    ),
                  ],
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

// --- link card --------------------------------------------------------------------------

/// A link sent by a presenter (spec 4.5): the display never opens a page by itself. [canOpen]
/// false: the no-browser variant (no Open button, `receive.link.hintTv`).
class LinkCard extends StatelessWidget {
  const LinkCard({
    super.key,
    required this.host,
    required this.url,
    required this.canOpen,
    required this.onOpen,
    required this.onDismiss,
    this.guard = requestGuard,
  });

  final String host;
  final String url;
  final bool canOpen;
  final VoidCallback onOpen;
  final VoidCallback onDismiss;
  final Duration guard;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    return InputGuard(
      identity: url,
      guard: guard,
      builder: (context, armed) {
        void guarded(VoidCallback f) {
          if (armed) f();
        }

        return DScrim(
          child: Semantics(
            scopesRoute: true,
            namesRoute: true,
            explicitChildNodes: true,
            label: t.receiveLinkTitle(host),
            child: DCard(
              width: 600,
              shadow: true,
              children: [
                DSectionLabel(t.receiveLinkTitle(host)),
                Text(
                  url,
                  key: const Key('link-address'),
                  style: s.type.linkAddress,
                ),
                Text(
                  canOpen ? t.receiveLinkHintApp : t.receiveLinkHintTv,
                  style: s.type.cardText,
                ),
                Row(
                  children: [
                    if (canOpen) ...[
                      DButton(
                        key: const Key('link-open'),
                        label: t.receiveLinkOpen,
                        kind: DButtonKind.primary,
                        row: 0,
                        col: 0,
                        autofocus: true,
                        onPressed: () => guarded(onOpen),
                      ),
                      const SizedBox(width: 12),
                    ],
                    DButton(
                      key: const Key('link-dismiss'),
                      label: t.receiveLinkDismiss,
                      row: 0,
                      col: canOpen ? 1 : 0,
                      autofocus: !canOpen,
                      onPressed: () => guarded(onDismiss),
                    ),
                  ],
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

// --- presenting -------------------------------------------------------------------------

/// Name tag (bottom left) and End (bottom right), faded together in 400 ms (spec 4.5). While
/// hidden nothing is focusable and nothing is drawn.
class PresentingControls extends StatelessWidget {
  const PresentingControls({
    super.key,
    required this.name,
    required this.shown,
    required this.onEnd,
    this.sas,
    this.endFocus,
    this.inert = false,
  });

  /// The presenter's cleaned name (already shortened as the name tag shows it).
  final String name;
  final bool shown;
  final VoidCallback onEnd;

  /// The security code as `123 456`, or null while it is not known.
  final String? sas;
  final FocusNode? endFocus;

  /// A dialog is over the controls (the codec dialog, decision 68): visible but inert.
  final bool inert;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = AppLocalizations.of(context);
    final dur = s.reduceMotion
        ? Duration.zero
        : const Duration(milliseconds: 400);
    final label = name.isEmpty
        ? t.commonNoName
        : t.commonQuoted(shortName(name));
    return Stack(
      fit: StackFit.expand,
      children: [
        Positioned(
          left: 12,
          bottom: 12,
          child: IgnorePointer(
            ignoring: !shown || inert,
            child: AnimatedOpacity(
              opacity: shown ? 1 : 0,
              duration: dur,
              curve: Curves.ease,
              child: Container(
                key: const Key('byline'),
                constraints: const BoxConstraints(minHeight: 46),
                padding: const EdgeInsets.symmetric(
                  horizontal: 14,
                  vertical: 6,
                ),
                decoration: overlayDecoration(s.tokens),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Text(t.presentByline(label), style: s.type.nameTag),
                    if (sas != null)
                      Text.rich(
                        TextSpan(
                          children: valueSpans(
                            t.presentSas(spanMarker),
                            sas!,
                            s.type.nameTagSub.copyWith(
                              fontFamily: fontMono,
                              fontWeight: FontWeight.w700,
                              color: s.tokens.fg,
                            ),
                          ),
                        ),
                        key: const Key('sas-line'),
                        style: s.type.nameTagSub,
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
        Positioned(
          right: 12,
          bottom: 12,
          child: ExcludeFocus(
            excluding: !shown || inert,
            child: IgnorePointer(
              ignoring: !shown || inert,
              child: AnimatedOpacity(
                opacity: shown ? 1 : 0,
                duration: dur,
                curve: Curves.ease,
                child: DisplayControl(
                  row: 0,
                  col: 0,
                  radius: 10,
                  focusNode: endFocus,
                  onPressed: shown ? onEnd : null,
                  child: Container(
                    key: const Key('end'),
                    constraints: const BoxConstraints(minHeight: 46),
                    padding: const EdgeInsets.symmetric(horizontal: 14),
                    decoration: overlayDecoration(s.tokens),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        CcIcon(CcIconName.stop, color: s.tokens.fg, size: 18),
                        const SizedBox(width: 6),
                        Text(t.presentEnd, style: s.type.nameTag),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Transfer progress pill (spec 3.5): name-tag look, bottom centre.
class TransferPill extends StatelessWidget {
  const TransferPill({super.key, required this.text});
  final String text;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    return Positioned(
      bottom: 12,
      left: 0,
      right: 0,
      child: Center(
        child: Container(
          key: const Key('transfer-pill'),
          constraints: const BoxConstraints(minHeight: 46),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
          alignment: Alignment.center,
          decoration: overlayDecoration(s.tokens),
          child: Text(text, style: s.type.nameTag),
        ),
      ),
    );
  }
}

/// One line, centred at the top, `--fg` background with `--bg` text (spec 4.6). Polite live
/// region; never takes focus.
class DToast extends StatelessWidget {
  const DToast({super.key, required this.text});
  final String text;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    return Positioned(
      top: 14,
      left: 0,
      right: 0,
      child: IgnorePointer(
        child: Center(
          child: Semantics(
            liveRegion: true,
            child: Container(
              key: const Key('toast'),
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
              decoration: BoxDecoration(
                color: t.fg,
                borderRadius: BorderRadius.circular(999),
                boxShadow: const [
                  BoxShadow(
                    color: Color(0x66000000),
                    blurRadius: 30,
                    offset: Offset(0, 10),
                  ),
                ],
              ),
              child: Text(
                text,
                style: s.type.label17Bold.copyWith(color: t.bg),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// A box in name-tag look for a page that cannot be drawn (spec 4.5).
class PageErrorBox extends StatelessWidget {
  const PageErrorBox({super.key, required this.text});
  final String text;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    return Center(
      child: Container(
        key: const Key('page-error'),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        decoration: overlayDecoration(s.tokens),
        child: Text(text, style: s.type.nameTag),
      ),
    );
  }
}
