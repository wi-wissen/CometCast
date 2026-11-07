// SPDX-License-Identifier: GPL-3.0-or-later
// Opens a link in the system's browser after the person chose "Open".

import 'package:url_launcher/url_launcher.dart';

Future<bool> launchInBrowser(Uri url) =>
    launchUrl(url, mode: LaunchMode.externalApplication);
