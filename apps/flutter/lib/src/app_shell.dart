// SPDX-License-Identifier: GPL-3.0-or-later
// Chooses the role of the app (docs/design/native-apps.md 1): a television is a display, a phone
// is a presenter, tablets and computers are presenters until the person chose "Use this device
// as the screen"; after the display setup they start as a display on every launch.

import 'dart:io' show Platform;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../core/android_capture.dart';
import '../core/android_platform.dart';
import '../core/core_api.dart';
import '../core/ios_broadcast.dart';
import '../core/launch_intake.dart';
import '../core/link_target.dart';
import '../core/miracast.dart';
import '../miracast_page.dart';
import '../receiver_controller.dart' show UrlOpener;
import '../settings.dart';
import 'display/display_logic.dart';
import 'display/display_root.dart';
import 'display/wake_lock.dart';
import 'presenter/file_picker.dart';
import 'presenter/presenter_root.dart';

enum AppRole { loading, presenter, display }

class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.core,
    required this.settings,
    required this.openUrl,
    required this.pickFile,
    this.store = const PrefsDisplayStore(),
    this.androidCapture,
    this.iosBroadcast,
    this.miracast,
    this.wakeLock,
    this.forceDisplayCapable,
    this.initialCode,
    this.scanQr,
    this.launch,
    this.links,
    this.linkStream,
    this.firstLaunch = false,
    this.canOpenLinks,
  });

  final CoreApi core;
  final AppSettings settings;
  final UrlOpener openUrl;
  final PickFile pickFile;
  final DisplayStore store;
  final AndroidCapture? androidCapture;
  final IosBroadcast? iosBroadcast;
  final MiracastCore? miracast;
  final WakeLockControl? wakeLock;

  /// Tests: decide "tablet or computer" without a window of that size.
  final bool? forceDisplayCapable;
  final String? initialCode;
  final ScanQr? scanQr;

  /// What started the app: a link, or the install referrer (native-apps.md 20.4). A usable one
  /// gives the first-run page its code chip; an unknown server first shows the relay page.
  final LinkOutcome? launch;
  final LinkHandler? links;

  /// Links that arrive while the app runs.
  final Stream<String>? linkStream;

  /// First launch after the install, and it brought no code (`app.scanAgainBody`).
  final bool firstLaunch;
  final CanOpenLinks? canOpenLinks;

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  AppRole _role = AppRole.loading;
  bool _tv = false;

  @override
  void initState() {
    super.initState();
    _tv = DeviceProfile.peek(context).tv;
    _decide();
  }

  Future<void> _decide() async {
    if (_tv) {
      setState(() => _role = AppRole.display);
      return;
    }
    final setup = await widget.store.loadSetup();
    if (!mounted) return;
    setState(
      () => _role = setup != null && _capable(context)
          ? AppRole.display
          : AppRole.presenter,
    );
  }

  /// Tablets (shortest side from 600 logical px) and computers can be a display; phones not
  /// (spec 7.2).
  bool _capable(BuildContext context) {
    final f = widget.forceDisplayCapable;
    if (f != null) return f;
    if (!(Platform.isAndroid || Platform.isIOS)) return true;
    return MediaQuery.sizeOf(context).shortestSide >= 600;
  }

  @override
  Widget build(BuildContext context) {
    switch (_role) {
      case AppRole.loading:
        return const ColoredBox(color: Color(0xFF0E1630));
      case AppRole.display:
        _chrome(true);
        return DisplayRoot(
          core: widget.core,
          settings: widget.settings,
          openUrl: widget.openUrl,
          store: widget.store,
          wakeLock: widget.wakeLock,
          canOpenLinks: widget.canOpenLinks,
          setFullscreenOnSetupDone: !(Platform.isAndroid || Platform.isIOS),
        );
      case AppRole.presenter:
        _chrome(false);
        final m = widget.miracast;
        return PresenterRoot(
          core: widget.core,
          settings: widget.settings,
          pickFile: widget.pickFile,
          androidCapture: widget.androidCapture,
          iosBroadcast: widget.iosBroadcast,
          scanQr: widget.scanQr,
          links: widget.links,
          linkStream: widget.linkStream,
          firstLaunch: widget.firstLaunch,
          initialCode: switch (widget.launch) {
            LinkUse(:final target) || LinkConfirm(:final target) => target.code,
            _ => widget.initialCode,
          },
          relayToConfirm: switch (widget.launch) {
            LinkConfirm(:final target) => target.server,
            _ => null,
          },
          onBeDisplay: _capable(context)
              ? () => setState(() => _role = AppRole.display)
              : null,
          // Windows only: the text link below `app.beDisplay` (decision N18).
          onMiracast: m == null
              ? null
              : () => Navigator.of(context).push(
                  MaterialPageRoute<void>(
                    builder: (_) => MiracastPage(core: m),
                  ),
                ),
        );
    }
  }

  /// Tablets in the display role: immersive and landscape only (native-apps.md 7).
  void _chrome(bool display) {
    if (!(Platform.isAndroid || Platform.isIOS)) return;
    if (display) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.immersiveSticky);
      SystemChrome.setPreferredOrientations(const [
        DeviceOrientation.landscapeLeft,
        DeviceOrientation.landscapeRight,
      ]);
    } else {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
      SystemChrome.setPreferredOrientations(const []);
    }
  }
}
