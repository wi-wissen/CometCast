// SPDX-License-Identifier: GPL-3.0-or-later
// Finds and loads the Rust library (libcc_ffi.so, .dylib, cc_ffi.dll).

import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'package:cometcast/src/rust/frb_generated.dart';

/// Name of the environment variable that points to the library file (development and tests).
const ffiLibraryEnvVar = 'COMETCAST_FFI_LIB';

/// The platforms the search order distinguishes.
enum CoreOs { linux, windows, macos, ios, android, other }

CoreOs _currentOs() {
  if (Platform.isLinux) return CoreOs.linux;
  if (Platform.isWindows) return CoreOs.windows;
  if (Platform.isMacOS) return CoreOs.macos;
  if (Platform.isIOS) return CoreOs.ios;
  if (Platform.isAndroid) return CoreOs.android;
  return CoreOs.other;
}

/// File name of the core library on [os].
String coreLibraryFileName(CoreOs os) {
  switch (os) {
    case CoreOs.windows:
      return 'cc_ffi.dll';
    case CoreOs.macos:
    case CoreOs.ios:
      return 'libcc_ffi.dylib';
    case CoreOs.linux:
    case CoreOs.android:
    case CoreOs.other:
      return 'libcc_ffi.so';
  }
}

/// Candidate files next to the executable, in search order (pure, for tests).
/// - Linux and other: `<exeDir>/lib/` (layout of `flutter build linux`).
/// - Windows: `<exeDir>/lib/`, then next to the .exe (where the release workflow puts it).
/// - macOS: `<exeDir>/../Frameworks/` (the .app layout: Contents/MacOS/<exe>, Contents/Frameworks),
///   then `<exeDir>/lib/`.
/// - Android and iOS: none; the platform loader resolves the plain name.
List<String> coreLibraryCandidates({
  required CoreOs os,
  required String exeDir,
  String separator = '/',
}) {
  final name = coreLibraryFileName(os);
  String join(List<String> parts) => parts.join(separator);
  switch (os) {
    case CoreOs.macos:
      return [
        join([exeDir, '..', 'Frameworks', name]),
        join([exeDir, 'lib', name]),
      ];
    case CoreOs.windows:
      return [join([exeDir, 'lib', name]), join([exeDir, name])];
    case CoreOs.linux:
    case CoreOs.other:
      return [join([exeDir, 'lib', name])];
    case CoreOs.android:
    case CoreOs.ios:
      return const [];
  }
}

/// Where the library is looked for, first match wins:
/// 1. the file named in `COMETCAST_FFI_LIB`,
/// 2. the candidates of [coreLibraryCandidates] for the platform,
/// 3. the system loader path (plain file name).
///
/// All inputs can be injected for tests; the defaults describe the running process.
/// UNCERTAIN: the macOS and Windows locations were never exercised on those systems.
String resolveCoreLibraryPath({
  Map<String, String>? environment,
  CoreOs? os,
  String? executableDir,
  String? separator,
  bool Function(String path)? fileExists,
}) {
  final theOs = os ?? _currentOs();
  final override = (environment ?? Platform.environment)[ffiLibraryEnvVar];
  if (override != null && override.isNotEmpty) return override;
  final exeDir =
      executableDir ?? File(Platform.resolvedExecutable).parent.path;
  final exists = fileExists ?? (String p) => File(p).existsSync();
  for (final candidate in coreLibraryCandidates(
    os: theOs,
    exeDir: exeDir,
    separator: separator ?? Platform.pathSeparator,
  )) {
    if (exists(candidate)) return candidate;
  }
  return coreLibraryFileName(theOs);
}

/// Load the Rust library once. Call before using any function of the core.
Future<void> initRustCore({String? libraryPath}) {
  return RustLib.init(
    externalLibrary: ExternalLibrary.open(
      libraryPath ?? resolveCoreLibraryPath(),
    ),
  );
}
