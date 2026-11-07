// SPDX-License-Identifier: GPL-3.0-or-later
// Start-up hook for the H.264 codec: on Android the Rust core needs the app-private directory
// in which the downloaded library is stored (other systems use their per-user data directory).

import 'dart:io';

import 'package:path_provider/path_provider.dart';

import 'core/core_api.dart';

/// Tell the core where it may store the codec. Never throws: without a directory the core
/// reports the codec as failed when the download is tried.
Future<void> configureCodecDir(CoreApi core) async {
  if (!Platform.isAndroid) return;
  try {
    final dir = await getApplicationSupportDirectory();
    core.setCodecDir(dir.path);
  } catch (_) {
    // The download will fail with a message; the rest of the app works.
  }
}
