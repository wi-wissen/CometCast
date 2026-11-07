// SPDX-License-Identifier: GPL-3.0-or-later
// Home screen: a quiet list. "Send screen", "Send file or link" and "Receive on this device"
// open the screens.

import 'package:flutter/material.dart';
import 'package:cometcast/l10n/app_localizations.dart';

import 'core/android_capture.dart';
import 'core/core_api.dart';
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
    this.pickFile = pickImageOrPdf,
    this.openUrl = launchInBrowser,
  });

  final CoreApi core;
  final AppSettings settings;
  final AndroidCapture? androidCapture;

  /// The file dialog of "Send file or link" (replaced in tests).
  final PickFile pickFile;

  /// How a received link is opened after the person chose "Open" (replaced in tests).
  final UrlOpener openUrl;

  @override
  Widget build(BuildContext context) {
    // t.<key> returns the text in the current language.
    final t = AppLocalizations.of(context);

    void open(Widget page) {
      Navigator.of(context).push(MaterialPageRoute<void>(builder: (_) => page));
    }

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
                        onTap: () => open(
                          ReceivePage(
                            core: core,
                            settings: settings,
                            openUrl: openUrl,
                          ),
                        ),
                      ),
                    ),
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
