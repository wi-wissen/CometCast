// SPDX-License-Identifier: GPL-3.0-or-later
// Home screen: three placeholder rows. No functionality yet.

import 'package:flutter/material.dart';
import 'package:cometcast/l10n/app_localizations.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    // t.<key> returns the text in the current language.
    final t = AppLocalizations.of(context);

    void comingSoon() {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(t.comingSoon)));
    }

    return Scaffold(
      appBar: AppBar(title: Text(t.appTitle)),
      body: SafeArea(
        child: Column(
          children: [
            Expanded(
              child: ListView(
                children: [
                  ListTile(
                    leading: const Icon(Icons.screen_share_outlined),
                    title: Text(t.homeSendScreen),
                    subtitle: Text(t.homeSendScreenHint),
                    onTap: comingSoon,
                  ),
                  ListTile(
                    leading: const Icon(Icons.attach_file),
                    title: Text(t.homeSendFileOrLink),
                    subtitle: Text(t.homeSendFileOrLinkHint),
                    onTap: comingSoon,
                  ),
                  ListTile(
                    leading: const Icon(Icons.download_outlined),
                    title: Text(t.homeReceive),
                    subtitle: Text(t.homeReceiveHint),
                    onTap: comingSoon,
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
    );
  }
}
