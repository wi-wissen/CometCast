// SPDX-License-Identifier: GPL-3.0-or-later
// Finds and loads the Rust library (libcc_ffi.so on Linux).

import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'package:cometcast/src/rust/frb_generated.dart';

/// Name of the environment variable that points to the library file (development and tests).
const ffiLibraryEnvVar = 'COMETCAST_FFI_LIB';

/// Where the library is looked for, first match wins:
/// 1. the file named in `COMETCAST_FFI_LIB`,
/// 2. `lib/` next to the executable (the layout of `flutter build linux` output),
/// 3. the system loader path (plain file name).
String resolveCoreLibraryPath() {
  final fileName = _fileName();
  final override = Platform.environment[ffiLibraryEnvVar];
  if (override != null && override.isNotEmpty) return override;
  final exeDir = File(Platform.resolvedExecutable).parent.path;
  final bundled =
      '$exeDir${Platform.pathSeparator}lib${Platform.pathSeparator}$fileName';
  if (File(bundled).existsSync()) return bundled;
  // UNCERTAIN: macOS bundles (Contents/Frameworks), Windows (next to the .exe), Android
  // and iOS need their own packaging; only the Linux desktop bundle is built and tested.
  return fileName;
}

String _fileName() {
  if (Platform.isWindows) return 'cc_ffi.dll';
  if (Platform.isMacOS || Platform.isIOS) return 'libcc_ffi.dylib';
  return 'libcc_ffi.so';
}

/// Load the Rust library once. Call before using any function of the core.
Future<void> initRustCore({String? libraryPath}) {
  return RustLib.init(
    externalLibrary: ExternalLibrary.open(
      libraryPath ?? resolveCoreLibraryPath(),
    ),
  );
}
