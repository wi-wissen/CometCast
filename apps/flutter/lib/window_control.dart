// SPDX-License-Identifier: GPL-3.0-or-later
// Full screen for the app window, through a small platform channel.

import 'package:flutter/services.dart';

/// Channel name; the Linux runner (`linux/runner/my_application.cc`) answers it.
const windowChannelName = 'de.cometcast/window';

/// Puts the window into or out of full screen. Returns whether the platform did it.
typedef SetFullscreen = Future<bool> Function(bool on);

// UNCERTAIN: only the Linux (GTK) runner implements the channel. On other platforms the call
// finds no handler, this returns false and the app stays windowed.
Future<bool> setWindowFullscreen(bool on) async {
  try {
    await const MethodChannel(windowChannelName)
        .invokeMethod<void>('setFullscreen', on);
    return true;
  } on MissingPluginException {
    return false;
  } on PlatformException {
    return false;
  }
}
