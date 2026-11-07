// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter pages (spec 5 and 6; native-apps.md 4): first run, waiting, what to show, file,
// link, presenting, disconnected, errors and the relay confirmation. Plain widgets; the flow is
// in presenter_root.dart.

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../codec_controller.dart';
import '../../core/core_api.dart';
import '../design/icons.dart';
import '../design/spans.dart';
import 'kit.dart';
import 'nearby.dart' show NearbyCard, PDividerOr;

/// What kind of device the presenter runs on (spec 5.5, decision N15: by device kind, not by
/// layout width).
enum PresenterDevice { phone, tablet, computer }

// --- first run (5.3) --------------------------------------------------------------------

class FirstRunPage extends StatelessWidget {
  const FirstRunPage({
    super.key,
    required this.codeController,
    required this.nameController,
    required this.onConnect,
    this.codeChip,
    this.chipFromLink = true,
    this.onScan,
    this.scanWrong = false,
    this.scanAgainBody = false,
    this.onBeDisplay,
    this.onMiracast,
    this.codeEmpty = false,
    this.canConnect = true,
    this.nearby,
  });

  final TextEditingController codeController;
  final TextEditingController nameController;

  /// A code that came from a link or a scan (shown as a chip), or null.
  final String? codeChip;
  final bool chipFromLink;

  /// The system scanner; null where there is none (the code field is shown alone).
  final VoidCallback? onScan;

  /// The last scan found no code of ours (`app.scanWrong` under the scan button).
  final bool scanWrong;

  /// First launch after an install that brought no code (`app.scanAgainBody`).
  final bool scanAgainBody;

  /// Tablets and computers: the text link that starts the display setup.
  final VoidCallback? onBeDisplay;

  /// Windows: the text link to the Miracast receiver.
  final VoidCallback? onMiracast;
  final VoidCallback onConnect;
  final bool codeEmpty;
  final bool canConnect;

  /// The section `nearby.title` (spec 14.1), or null when it is not shown. It is placed between
  /// the name field and the code (phone) or in a card on the right (laptop); with it the
  /// divider `start.or` comes after it and `app.scanAgainBody` is not shown.
  final Widget? nearby;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final p = s.tokens;
    final t = AppLocalizations.of(context);
    // Spec 5.3, N37: each link is one line of 15 `p-primary`, underlined, in a tap area 44 high
    // with the text centred in it; the two areas touch, so the lines are 44 apart. Laptop:
    // left-aligned, the first area starts 12 below Connect; phone: centred, 8 below Connect.
    Widget link(String label, Key key, VoidCallback onTap) => PTap(
      onPressed: onTap,
      radius: 4,
      builder: (_) => SizedBox(
        height: 44,
        child: Center(
          widthFactor: 1,
          child: Text(
            label,
            key: key,
            style: s.type.body.copyWith(
              fontSize: 15,
              color: p.primary,
              decoration: TextDecoration.underline,
            ),
          ),
        ),
      ),
    );
    final links = <Widget>[
      if (onBeDisplay != null)
        link(t.appBeDisplay, const Key('be-display'), onBeDisplay!),
      if (onMiracast != null)
        link(t.appMiracast, const Key('miracast-link'), onMiracast!),
    ];
    final actions = <Widget>[
      PPrimaryButton(
        label: t.appConnect,
        onPressed: canConnect ? onConnect : null,
      ),
      if (links.isNotEmpty)
        _LinksBlock(
          // The stacked action area leaves 8 above; the laptop wants 12.
          top: s.laptop ? 4 : 0,
          alignment: s.laptop ? Alignment.centerLeft : Alignment.center,
          children: links,
        ),
    ];
    final brand = <Widget>[
      const PBrand(),
      Text(t.appWelcome, style: s.type.heading),
      PField(
        fieldKey: const Key('name-field'),
        label: t.startNameLabel,
        controller: nameController,
        maxLength: 40,
        textInputAction: TextInputAction.next,
      ),
    ];
    final withNearby = nearby != null && codeChip == null;
    final code = <Widget>[
      if (codeChip != null)
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            color: p.surface,
            border: Border.all(color: p.borderInput),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                codeChip!,
                key: const Key('code-chip'),
                style: s.type.codeChip,
              ),
              Row(
                children: [
                  CcIcon(CcIconName.check, color: p.ok, size: 16),
                  const SizedBox(width: 5),
                  Text(
                    chipFromLink ? t.appCodeFromLink : t.appCodeScanned,
                    style: s.type.label.copyWith(color: p.ok),
                  ),
                ],
              ),
            ],
          ),
        )
      else ...[
        if (onScan != null) ...[
          // `app.scanAgainBody` speaks of scanning "once more": not with the nearby section
          // (spec 14.1).
          if (scanAgainBody && !withNearby)
            Text(
              t.appScanAgainBody,
              key: const Key('scan-again-body'),
              style: s.type.body,
            ),
          PSecondaryButton(
            key: const Key('scan-button'),
            label: t.appScanQr,
            icon: CcIconName.scan,
            onPressed: onScan,
          ),
          if (scanWrong)
            Semantics(
              liveRegion: true,
              child: Text(
                t.appScanWrong,
                key: const Key('scan-wrong'),
                style: s.type.errorText,
              ),
            ),
        ],
        PField(
          fieldKey: const Key('code-field'),
          label: t.startCodeLabel,
          controller: codeController,
          code: true,
          maxLength: 9,
          hint: 'XXX XXX',
          error: codeEmpty,
          errorText: codeEmpty ? t.startCodeEmpty : null,
          textInputAction: TextInputAction.done,
          onSubmitted: (_) => onConnect(),
        ),
      ],
    ];
    if (withNearby && s.laptop) {
      // Laptop (spec 14.1): left column brand, heading, name, code, Connect and the links; the
      // section in a card on the right.
      Widget column(List<Widget> items) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < items.length; i++) ...[
            // The links start 12 below Connect (N37), everything else 14 apart.
            if (i > 0) SizedBox(height: items[i] is _LinksBlock ? 12 : 14),
            // Buttons are as wide as their content on the laptop (spec 7.4).
            if (items[i] is PPrimaryButton || items[i] is PSecondaryButton)
              Align(alignment: Alignment.centerLeft, child: items[i])
            else
              items[i],
          ],
        ],
      );
      return PPage(
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                flex: 100,
                child: column([...brand, ...code, ...actions]),
              ),
              const SizedBox(width: 32),
              Expanded(flex: 105, child: NearbyCard(child: nearby!)),
            ],
          ),
        ],
      );
    }
    return PPage(
      actions: actions,
      children: [
        ...brand,
        if (withNearby) ...[nearby!, const PDividerOr()],
        ...code,
      ],
    );
  }
}

/// The text links under Connect (`app.beDisplay`, on Windows `app.miracast`): tap areas of 44
/// that touch each other (N37), as one block so that nothing is added between them.
class _LinksBlock extends StatelessWidget {
  const _LinksBlock({
    required this.children,
    required this.alignment,
    this.top = 0,
  });
  final List<Widget> children;
  final Alignment alignment;
  final double top;

  @override
  Widget build(BuildContext context) => Padding(
    padding: EdgeInsets.only(top: top),
    child: Align(
      alignment: alignment,
      heightFactor: 1,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        // Left-aligned on the laptop, centred on the phone (N37).
        crossAxisAlignment: alignment == Alignment.centerLeft
            ? CrossAxisAlignment.start
            : CrossAxisAlignment.center,
        children: children,
      ),
    ),
  );
}

/// Brand row: logo 28 + "Comet Cast" (spec 5.2), the logo file that fits the device's
/// light or dark setting.
class PBrand extends StatelessWidget {
  const PBrand({super.key});

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Row(
      key: const Key('brand'),
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
        Text(t.appTitle, style: s.type.brand),
      ],
    );
  }
}

// --- waiting (5.4) ----------------------------------------------------------------------

class WaitingPage extends StatelessWidget {
  const WaitingPage({
    super.key,
    required this.check,
    required this.secondsLeft,
    required this.totalSeconds,
    required this.onWithdraw,
    this.room,
  });
  final int check;
  final int secondsLeft;
  final int totalSeconds;
  final VoidCallback onWithdraw;
  final String? room;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actions: [PSecondaryButton(label: t.waitWithdraw, onPressed: onWithdraw)],
      children: [
        Row(
          children: [
            const PSpinner(),
            if (room != null) ...[
              const SizedBox(width: 10),
              Flexible(child: Text(room!, style: s.type.label)),
            ],
          ],
        ),
        Text(t.waitTitle, style: s.type.heading),
        PCard(
          child: Column(
            children: [
              Text(t.waitCheckLabel, style: s.type.label),
              const SizedBox(height: 4),
              Text(
                '$check',
                key: const Key('waiting-check'),
                style: s.type.checkNumber,
              ),
              const SizedBox(height: 4),
              Text(
                t.waitCheckHint,
                textAlign: TextAlign.center,
                style: s.type.small,
              ),
            ],
          ),
        ),
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            PProgressBar(
              fraction: totalSeconds <= 0 ? 0 : secondsLeft / totalSeconds,
            ),
            const SizedBox(height: 5),
            Text(
              t.waitExpires('$secondsLeft'),
              key: const Key('waiting-left'),
              style: s.type.small,
            ),
          ],
        ),
      ],
    );
  }
}

// --- what to show (5.5) -----------------------------------------------------------------

class WhatPage extends StatelessWidget {
  const WhatPage({
    super.key,
    required this.onScreen,
    required this.onFile,
    required this.onLink,
    required this.onDisconnect,
    required this.device,
    this.room,
  });
  final VoidCallback onScreen;
  final VoidCallback onFile;
  final VoidCallback onLink;
  final VoidCallback onDisconnect;

  /// Screen subtitle by device kind: `what.screenApp` (phone), `what.screenTablet`,
  /// `what.screenLaptop` (computer).
  final PresenterDevice device;

  /// The room name from `joined.room_name`; without one the header says only "Connected".
  final String? room;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actions: [
        PSecondaryButton(label: t.whatDisconnect, onPressed: onDisconnect),
      ],
      children: [
        Row(
          children: [
            const PIconDisc(icon: CcIconName.check, tone: PTone.ok, size: 40),
            const SizedBox(width: 10),
            Flexible(
              child: Text(
                room == null ? t.whatConnected : t.whatConnectedTo(room!),
                key: const Key('connected-to'),
                style: s.type.label,
              ),
            ),
          ],
        ),
        Text(t.whatTitle, style: s.type.heading),
        Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            POption(
              key: const Key('what-screen'),
              icon: CcIconName.screen,
              title: t.whatScreen,
              subtitle: switch (device) {
                PresenterDevice.phone => t.whatScreenApp,
                PresenterDevice.tablet => t.whatScreenTablet,
                PresenterDevice.computer => t.whatScreenLaptop,
              },
              onPressed: onScreen,
            ),
            const SizedBox(height: 8),
            POption(
              key: const Key('what-file'),
              icon: CcIconName.file,
              title: t.whatFile,
              subtitle: t.whatFileSub,
              onPressed: onFile,
            ),
            const SizedBox(height: 8),
            POption(
              key: const Key('what-link'),
              icon: CcIconName.link,
              title: t.whatLink,
              subtitle: t.whatLinkSub,
              onPressed: onLink,
            ),
          ],
        ),
      ],
    );
  }
}

// --- file (5.5) -------------------------------------------------------------------------

class FilePage extends StatelessWidget {
  const FilePage({
    super.key,
    required this.onPick,
    required this.onBack,
    this.progressName,
    this.progressPercent,
    this.convertingName,
    this.office = false,
    this.error,
  });
  final VoidCallback onPick;
  final VoidCallback onBack;
  final String? progressName;
  final int? progressPercent;

  /// An Office file is being converted: `file.converting` as small print, no bar.
  final String? convertingName;

  /// The conversion component exists (desktops): the body names Office files.
  final bool office;
  final String? error;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actions: [PSecondaryButton(label: t.commonBack, onPressed: onBack)],
      children: [
        Text(t.whatFile, style: s.type.heading),
        // The browser variant on every app platform; `file.bodyApp` where Office files are
        // converted (decision N16).
        Text(office ? t.fileBodyApp : t.fileBodyWeb, style: s.type.body),
        PPrimaryButton(
          label: t.filePick,
          onPressed: progressName == null && convertingName == null
              ? onPick
              : null,
        ),
        if (convertingName != null)
          Text(
            t.fileConverting(convertingName!),
            key: const Key('file-converting'),
            style: s.type.small,
          )
        else if (progressName != null)
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                '$progressName · ${progressPercent ?? 0} %',
                style: s.type.small,
              ),
              const SizedBox(height: 5),
              PProgressBar(fraction: (progressPercent ?? 0) / 100),
            ],
          ),
        if (error != null)
          Semantics(
            liveRegion: true,
            child: Text(
              error!,
              key: const Key('file-error'),
              style: s.type.errorText,
            ),
          ),
      ],
    );
  }
}

// --- link (5.5) -------------------------------------------------------------------------

class LinkPage extends StatelessWidget {
  const LinkPage({
    super.key,
    required this.controller,
    required this.onGo,
    required this.onBack,
    this.error,
  });
  final TextEditingController controller;
  final VoidCallback onGo;
  final VoidCallback onBack;
  final String? error;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actions: [PSecondaryButton(label: t.commonBack, onPressed: onBack)],
      children: [
        Text(t.whatLink, style: s.type.heading),
        PField(
          fieldKey: const Key('link-url'),
          label: t.linkLabel,
          controller: controller,
          keyboardType: TextInputType.url,
          error: error != null,
          helper: t.linkHint,
          errorText: error,
          textInputAction: TextInputAction.go,
          onSubmitted: (_) => onGo(),
        ),
        PPrimaryButton(label: t.linkGo, onPressed: onGo),
      ],
    );
  }
}

// --- presenting (5.6) -------------------------------------------------------------------

enum PresentingKind { screen, file, link }

class PresentingPage extends StatelessWidget {
  const PresentingPage({
    super.key,
    required this.kind,
    required this.onStop,
    this.pdf = false,
    this.onPrev,
    this.onNext,
    this.room,
    this.sas,
    this.page,
    this.pages,
  });
  final PresentingKind kind;
  final VoidCallback onStop;
  final bool pdf;
  final VoidCallback? onPrev;
  final VoidCallback? onNext;
  final String? room;

  /// The security code (`123 456`), when the core reported it.
  final String? sas;

  /// The page the display shows and the page count; null until the display reported them
  /// (both buttons stay enabled meanwhile, no "Page n of m" line).
  final int? page;
  final int? pages;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    final title = switch (kind) {
      PresentingKind.screen => t.presentingScreen,
      PresentingKind.file => t.presentingFile,
      PresentingKind.link => t.presentingLink,
    };
    final icon = switch (kind) {
      PresentingKind.screen => CcIconName.screen,
      PresentingKind.file => CcIconName.file,
      PresentingKind.link => CcIconName.link,
    };
    final known = page != null && pages != null;
    final prevOk = !known || page! > 1;
    final nextOk = !known || page! < pages!;
    final prev = PSecondaryButton(
      key: const Key('prev-page'),
      label: t.presentingPrevPage,
      fullWidth: !s.laptop,
      onPressed: prevOk ? onPrev : null,
    );
    final next = PSecondaryButton(
      key: const Key('next-page'),
      label: t.presentingNextPage,
      fullWidth: !s.laptop,
      onPressed: nextOk ? onNext : null,
    );
    return PPage(
      actions: [
        if (pdf && known)
          Text(
            t.presentingPageOf('$page', '$pages'),
            key: const Key('page-of'),
            style: s.type.small,
          ),
        if (pdf)
          // Previous and Next side by side directly above Stop (8 between them): half width
          // each on the phone, content width (min 160) and left-aligned on the laptop.
          s.laptop
              ? Row(children: [prev, const SizedBox(width: 8), next])
              : Row(
                  children: [
                    Expanded(child: prev),
                    const SizedBox(width: 8),
                    Expanded(child: next),
                  ],
                ),
        PPrimaryButton(label: t.presentingStop, onPressed: onStop),
      ],
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: PIconDisc(icon: icon, tone: PTone.acc),
        ),
        Text(title, key: const Key('presenting-title'), style: s.type.heading),
        if (room != null)
          Text(
            t.presentingWhere(room!),
            key: const Key('presenting-where'),
            style: s.type.body,
          ),
        if (sas != null)
          Text.rich(
            TextSpan(
              children: valueSpans(
                t.presentingSas(spanMarker),
                sas!,
                s.type.smallDigits,
              ),
            ),
            key: const Key('presenting-sas'),
            style: s.type.small,
          ),
      ],
    );
  }
}

// --- disconnected (5.7) -----------------------------------------------------------------

class ByePage extends StatelessWidget {
  const ByePage({super.key, required this.onAgain});
  final VoidCallback onAgain;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    // In the apps `bye.body` ("You can close this tab") is left out (native-apps.md 4).
    return PPage(
      actions: [PPrimaryButton(label: t.byeAgain, onPressed: onAgain)],
      children: [
        const Align(
          alignment: Alignment.centerLeft,
          child: PIconDisc(icon: CcIconName.check, tone: PTone.mut),
        ),
        Text(t.byeTitle, style: s.type.heading),
      ],
    );
  }
}

// --- errors (6) -------------------------------------------------------------------------

enum ErrorKind {
  code,
  expired,
  noInternet,
  noScreen,
  displayOff,
  denied,
  locked,
  timeout,
  ended,
  occupied,
  busy,
  outdated,
  other,
}

enum ErrorAction { retype, rescan, retry, showFile, showLink, ask, back }

class ErrorPage extends StatelessWidget {
  const ErrorPage({
    super.key,
    required this.kind,
    required this.onAction,
    this.lockedWhile = false,
    this.cooldown = 0,
    this.otherWhy,
    this.canRescan = false,
    this.relayWhy = false,
    this.boxNow = false,
    this.dom = '',
    this.useBack = false,
  });
  final ErrorKind kind;
  final void Function(ErrorAction) onAction;

  /// The action is `common.back` to "What to show" (the person stays connected) instead of
  /// Try again: Try again would only fail again (spec 6, N33, N46: the unsupported codec page
  /// and `capture_unavailable`).
  final bool useBack;
  final bool lockedWhile;
  final int cooldown;

  /// `errors.<code>` text for [ErrorKind.other].
  final String? otherWhy;

  /// "Scan QR code again": phones and tablets with a camera.
  final bool canRescan;

  /// `error.displayOff.whyRelay` instead of `.why`.
  final bool relayWhy;

  /// `error.noInternet.nowBox` (in-house box).
  final bool boxNow;
  final String dom;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    final (icon, tone) = switch (kind) {
      ErrorKind.code => (CcIconName.x, PTone.err),
      ErrorKind.expired => (CcIconName.clock, PTone.warn),
      ErrorKind.noInternet => (CcIconName.x, PTone.warn),
      ErrorKind.noScreen => (CcIconName.info, PTone.acc),
      ErrorKind.displayOff => (CcIconName.x, PTone.warn),
      ErrorKind.denied => (CcIconName.x, PTone.err),
      ErrorKind.locked => (CcIconName.locked, PTone.mut),
      ErrorKind.timeout => (CcIconName.clock, PTone.warn),
      ErrorKind.ended => (CcIconName.info, PTone.mut),
      ErrorKind.occupied => (CcIconName.info, PTone.mut),
      ErrorKind.busy => (CcIconName.clock, PTone.warn),
      ErrorKind.outdated => (CcIconName.info, PTone.mut),
      ErrorKind.other => (CcIconName.info, PTone.mut),
    };
    final (title, why, now) = switch (kind) {
      ErrorKind.code => (t.errorCodeTitle, t.errorCodeWhy, t.errorCodeNow),
      ErrorKind.expired => (
        t.errorExpiredTitle,
        t.errorExpiredWhy,
        t.errorExpiredNow,
      ),
      ErrorKind.noInternet => (
        t.errorNoInternetTitle,
        t.errorNoInternetWhy,
        boxNow ? t.errorNoInternetNowBox(dom) : t.errorNoInternetNow,
      ),
      ErrorKind.noScreen => (
        t.errorNoScreenTitle,
        t.errorNoScreenWhy,
        t.errorNoScreenNow,
      ),
      ErrorKind.displayOff => (
        t.errorDisplayOffTitle,
        relayWhy ? t.errorDisplayOffWhyRelay : t.errorDisplayOffWhy,
        t.errorDisplayOffNow,
      ),
      ErrorKind.denied => (
        t.errorDeniedTitle,
        t.errorDeniedWhy,
        t.errorDeniedNow,
      ),
      ErrorKind.locked => (
        t.errorLockedTitle,
        lockedWhile ? t.errorLockedWhyWhile : t.errorLockedWhy,
        t.errorLockedNow,
      ),
      ErrorKind.timeout => (
        t.errorTimeoutTitle,
        t.errorTimeoutWhy,
        t.errorTimeoutNow,
      ),
      ErrorKind.ended => (t.errorEndedTitle, t.errorEndedWhy, t.errorEndedNow),
      ErrorKind.occupied => (
        t.errorOccupiedTitle,
        t.errorOccupiedWhy,
        t.errorOccupiedNow,
      ),
      ErrorKind.busy => (t.errorBusyTitle, t.errorBusyWhy, t.errorBusyNow),
      // The apps say "app", never "page" (decision N14).
      ErrorKind.outdated => (
        t.errorOutdatedTitleApp,
        t.errorOutdatedWhy,
        t.errorOutdatedNowApp,
      ),
      ErrorKind.other => (
        t.errorOtherTitle,
        otherWhy ?? t.errorsUnknown,
        t.errorOtherNow,
      ),
    };
    final rescan =
        canRescan &&
        (kind == ErrorKind.code ||
            kind == ErrorKind.expired ||
            kind == ErrorKind.ended);
    final actions = <Widget>[
      switch (kind) {
        ErrorKind.code ||
        ErrorKind.expired ||
        ErrorKind.ended => PPrimaryButton(
          key: const Key('error-retype'),
          label: t.errorActionRetype,
          onPressed: () => onAction(ErrorAction.retype),
        ),
        ErrorKind.noScreen => PPrimaryButton(
          key: const Key('error-showFile'),
          label: t.errorActionShowFile,
          onPressed: () => onAction(ErrorAction.showFile),
        ),
        ErrorKind.denied => PPrimaryButton(
          key: const Key('error-ask'),
          label: cooldown > 0
              ? t.errorActionAskIn('$cooldown')
              : t.errorActionAsk,
          onPressed: cooldown > 0 ? null : () => onAction(ErrorAction.ask),
        ),
        ErrorKind.timeout => PPrimaryButton(
          key: const Key('error-ask'),
          label: t.errorActionAsk,
          onPressed: () => onAction(ErrorAction.ask),
        ),
        _ when useBack => PPrimaryButton(
          key: const Key('error-back'),
          label: t.commonBack,
          onPressed: () => onAction(ErrorAction.back),
        ),
        _ => PPrimaryButton(
          key: const Key('error-retry'),
          label: t.errorActionRetry,
          onPressed: () => onAction(ErrorAction.retry),
        ),
      },
      if (kind == ErrorKind.noScreen)
        PSecondaryButton(
          key: const Key('error-showLink'),
          label: t.errorActionShowLink,
          onPressed: () => onAction(ErrorAction.showLink),
        ),
      if (rescan)
        PSecondaryButton(
          key: const Key('error-rescan'),
          label: t.errorActionRescan,
          onPressed: () => onAction(ErrorAction.rescan),
        ),
    ];
    return PPage(
      actionsRow: true,
      actions: actions,
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: PIconDisc(icon: icon, tone: tone),
        ),
        Text(title, key: const Key('error-title'), style: s.type.heading),
        PCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(t.errorWhy.toUpperCase(), style: s.type.cardLabel),
              const SizedBox(height: 4),
              Text(
                why,
                key: const Key('error-why'),
                style: s.type.body.copyWith(color: s.tokens.fg),
              ),
              const SizedBox(height: 10),
              Text(t.errorNow.toUpperCase(), style: s.type.cardLabel),
              const SizedBox(height: 4),
              Text(
                now,
                key: const Key('error-now'),
                style: s.type.body.copyWith(color: s.tokens.fg),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

// --- relay confirmation (7.2) -----------------------------------------------------------

class RelayConfirmPage extends StatelessWidget {
  const RelayConfirmPage({
    super.key,
    required this.host,
    required this.onCancel,
    required this.onUse,
    this.nearbyName,
  });
  final String host;
  final VoidCallback onCancel;
  final VoidCallback onUse;

  /// A screen found nearby announced this relay (spec 14.4): the screen's name, already quoted
  /// (`common.quoted`) or `common.noName`. The body is `relay.confirm.bodyNearby` and Cancel is
  /// `common.cancel` (there is no default relay to fall back to here).
  final String? nearbyName;

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    return PPage(
      actionsRow: true,
      // Cancel first, Use this relay second; Cancel has the initial focus (decision 48).
      actions: [
        PSecondaryButton(
          key: const Key('relay-cancel'),
          label: nearbyName == null ? t.relayConfirmCancel : t.commonCancel,
          autofocus: true,
          onPressed: onCancel,
        ),
        PPrimaryButton(
          key: const Key('relay-use'),
          label: t.relayConfirmUse,
          onPressed: onUse,
        ),
      ],
      children: [
        const Align(
          alignment: Alignment.centerLeft,
          child: PIconDisc(icon: CcIconName.info, tone: PTone.warn),
        ),
        Text(t.relayConfirmTitle, style: s.type.heading),
        Text(
          nearbyName == null
              ? t.relayConfirmBodyApp(host)
              : t.relayConfirmBodyNearby(nearbyName!, host),
          style: s.type.body,
        ),
        Text(host, key: const Key('relay-host'), style: s.type.host),
        Text(t.relayConfirmRisk, style: s.type.small),
      ],
    );
  }
}

// --- video codec (5.8) --------------------------------------------------------------------

/// The page before the system picker on desktops without the H.264 encoder: layout of the relay
/// page, [Download, Back]; when the encoder is ready the primary opens the picker.
class CodecPage extends StatelessWidget {
  const CodecPage({
    super.key,
    required this.controller,
    required this.onBack,
    required this.onScreen,
  });

  final CodecController controller;
  final VoidCallback onBack;

  /// Ready: the primary reads `what.screen` and starts the screen share.
  final VoidCallback onScreen;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final s = PresenterScope.of(context);
        final t = AppLocalizations.of(context);
        final info = controller.info;
        final state = controller.state;
        if (state is UiCodecState_Unsupported) {
          // `error.other` with the codec text as its "why" (spec 5.8).
          return ErrorPage(
            kind: ErrorKind.other,
            otherWhy: t.codecUnsupported,
            useBack: true,
            onAction: (_) => onBack(),
          );
        }
        final back = PSecondaryButton(
          key: const Key('codec-back'),
          label: t.commonBack,
          onPressed: onBack,
        );
        final primary = switch (state) {
          UiCodecState_Ready() => PPrimaryButton(
            key: const Key('codec-screen'),
            label: t.whatScreen,
            autofocus: true,
            onPressed: onScreen,
          ),
          UiCodecState_Downloading() => PPrimaryButton(
            key: const Key('codec-download'),
            label: t.codecDownloading,
            onPressed: null,
          ),
          UiCodecState_Failed() => PPrimaryButton(
            key: const Key('codec-retry'),
            label: t.codecRetry,
            autofocus: true,
            onPressed: controller.download,
          ),
          _ => PPrimaryButton(
            key: const Key('codec-download'),
            label: t.codecDownload,
            autofocus: true,
            onPressed: controller.download,
          ),
        };
        final small = <Widget>[
          if (info != null)
            Text(
              t.codecSource(
                info.host,
                info.fileName,
                codecSizeText(info.downloadBytes),
              ),
              key: const Key('codec-source'),
              style: s.type.small,
            ),
          if (state is! UiCodecState_Ready)
            Text(
              info?.adminDir != null
                  ? t.codecOffline(info!.fileName, info.adminDir!)
                  : t.codecOfflineNoDir(info?.fileName ?? 'libopenh264'),
              key: const Key('codec-offline'),
              style: s.type.small,
            ),
        ];
        return PPage(
          actionsRow: true,
          actions: [primary, back],
          children: [
            const Align(
              alignment: Alignment.centerLeft,
              child: PIconDisc(icon: CcIconName.download, tone: PTone.acc),
            ),
            Text(t.codecTitle, style: s.type.heading),
            Text(t.codecExplain, style: s.type.body),
            if (state is UiCodecState_Downloading)
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    t.codecProgress((state.progress * 100).round().toString()),
                    key: const Key('codec-progress-text'),
                    style: s.type.small,
                  ),
                  const SizedBox(height: 5),
                  PProgressBar(fraction: state.progress),
                ],
              ),
            if (state is UiCodecState_Failed)
              Semantics(
                liveRegion: true,
                child: Text(
                  t.codecFailed,
                  key: const Key('codec-failed'),
                  style: s.type.errorText,
                ),
              ),
            ...small,
          ],
        );
      },
    );
  }
}
