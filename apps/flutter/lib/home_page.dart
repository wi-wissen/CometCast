// SPDX-License-Identifier: GPL-3.0-or-later
// Home screen: a quiet list. "Send screen", "Send file or link" and "Receive on this device"
// open the screens.

import 'package:flutter/material.dart';
import 'package:cometcast/l10n/app_localizations.dart';

import 'core/android_capture.dart';
import 'core/android_platform.dart';
import 'core/core_api.dart';
import 'core/ios_broadcast.dart';
import 'core/miracast.dart';
import 'miracast_page.dart';
import 'receive_page.dart';
import 'receiver_controller.dart' show UrlOpener;
import 'send_content_page.dart';
import 'send_page.dart';
import 'settings.dart';
import 'widgets.dart';

class HomePage extends StatelessWidget {
  const HomePage({
    super.key,
    required this.core,
    required this.settings,
    this.androidCapture,
    this.iosBroadcast,
    this.pickFile = pickImageOrPdf,
    this.openUrl = launchInBrowser,
    this.miracast,
  });

  final CoreApi core;
  final AppSettings settings;
  final AndroidCapture? androidCapture;

  /// The iOS broadcast hand-over (a fake in tests); null on other platforms.
  final IosBroadcast? iosBroadcast;

  /// The file dialog of "Send file or link" (replaced in tests).
  final PickFile pickFile;

  /// How a received link is opened after the person chose "Open" (replaced in tests).
  final UrlOpener openUrl;

  /// The Miracast receiver. Null on every platform except Windows: the row
  /// "Receive via Miracast (Windows)" exists only when this is given (main.dart passes it on
  /// Windows only), so other platforms can never show or open it.
  final MiracastCore? miracast;

  @override
  Widget build(BuildContext context) {
    // t.<key> returns the text in the current language.
    final t = AppLocalizations.of(context);

    void open(Widget page) {
      Navigator.of(context).push(MaterialPageRoute<void>(builder: (_) => page));
    }

    final tv = DeviceProfile.of(context).tv;
    void openReceive() =>
        open(ReceivePage(core: core, settings: settings, openUrl: openUrl));

    // One traversal group + visible focus frames: the screen works with the arrow keys and
    // select of a TV remote (Android TV / Google TV), see docs/android.md.
    return FocusTraversalGroup(
      policy: ReadingOrderTraversalPolicy(),
      child: Scaffold(
        appBar: AppBar(
          // App icon from assets/icon.png (decorative: the title next to it says the name).
          leading: Padding(
            padding: const EdgeInsets.all(8),
            child: Image.asset('assets/icon.png', excludeFromSemantics: true),
          ),
          title: Text(t.appTitle),
        ),
        body: SafeArea(
          child: Column(
            children: [
              Expanded(
                child: ListView(
                  children: [
                    // A television only receives (no touch screen, usually no screen capture
                    // and no file picker): one big entry, focused at start.
                    if (tv) ...[
                      FocusRing(
                        child: ListTile(
                          key: const Key('tvReceiveTile'),
                          autofocus: true,
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 48,
                            vertical: 24,
                          ),
                          leading: const Icon(
                            Icons.download_outlined,
                            size: 56,
                          ),
                          title: Text(
                            t.homeReceive,
                            style: Theme.of(context).textTheme.headlineMedium,
                          ),
                          subtitle: Text(
                            t.homeReceiveTvHint,
                            style: Theme.of(context).textTheme.titleLarge,
                          ),
                          onTap: openReceive,
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 48,
                          vertical: 16,
                        ),
                        child: Text(
                          t.homeReceiverOnlyTv,
                          key: const Key('tvReceiverOnlyNotice'),
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                      ),
                    ] else ...[
                      FocusRing(
                        child: ListTile(
                          autofocus: true,
                          leading: const Icon(Icons.screen_share_outlined),
                          title: Text(t.homeSendScreen),
                          subtitle: Text(t.homeSendScreenHint),
                          onTap: () => open(
                            SendPage(
                              core: core,
                              settings: settings,
                              androidCapture: androidCapture,
                              iosBroadcast: iosBroadcast,
                            ),
                          ),
                        ),
                      ),
                      FocusRing(
                        child: ListTile(
                          leading: const Icon(Icons.attach_file),
                          title: Text(t.homeSendFileOrLink),
                          subtitle: Text(t.homeSendFileOrLinkHint),
                          onTap: () => open(
                            SendContentPage(
                              core: core,
                              settings: settings,
                              pickFile: pickFile,
                            ),
                          ),
                        ),
                      ),
                      FocusRing(
                        child: ListTile(
                          leading: const Icon(Icons.download_outlined),
                          title: Text(t.homeReceive),
                          subtitle: Text(t.homeReceiveHint),
                          onTap: openReceive,
                        ),
                      ),
                      if (miracast != null)
                        FocusRing(
                          child: ListTile(
                            key: const Key('miracastTile'),
                            leading: const Icon(Icons.cast_connected),
                            title: Text(t.homeMiracast),
                            subtitle: Text(t.homeMiracastHint),
                            onTap: () => open(MiracastPage(core: miracast!)),
                          ),
                        ),
                    ],
                  ],
                ),
              ),
              Padding(
                padding: const EdgeInsets.all(16),
                child: Text(
                  t.previewNotice,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
