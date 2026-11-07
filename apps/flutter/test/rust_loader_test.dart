// SPDX-License-Identifier: GPL-3.0-or-later
// Search order of the Rust library loader, with injected paths (no real library needed).

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/rust_loader.dart';

void main() {
  const app = '/Applications/Comet Cast.app/Contents/MacOS';

  test('macOS: Contents/Frameworks wins over lib/', () {
    final path = resolveCoreLibraryPath(
      environment: const {},
      os: CoreOs.macos,
      executableDir: app,
      separator: '/',
      fileExists: (p) => true,
    );
    expect(path, '$app/../Frameworks/libcc_ffi.dylib');
  });

  test('macOS: falls back to lib/, then to the plain name', () {
    expect(
      resolveCoreLibraryPath(
        environment: const {},
        os: CoreOs.macos,
        executableDir: app,
        separator: '/',
        fileExists: (p) => p == '$app/lib/libcc_ffi.dylib',
      ),
      '$app/lib/libcc_ffi.dylib',
    );
    expect(
      resolveCoreLibraryPath(
        environment: const {},
        os: CoreOs.macos,
        executableDir: app,
        separator: '/',
        fileExists: (p) => false,
      ),
      'libcc_ffi.dylib',
    );
  });

  test('Linux: lib/ next to the executable, else plain name', () {
    expect(
      resolveCoreLibraryPath(
        environment: const {},
        os: CoreOs.linux,
        executableDir: '/opt/cometcast',
        separator: '/',
        fileExists: (p) => p == '/opt/cometcast/lib/libcc_ffi.so',
      ),
      '/opt/cometcast/lib/libcc_ffi.so',
    );
    expect(
      resolveCoreLibraryPath(
        environment: const {},
        os: CoreOs.linux,
        executableDir: '/opt/cometcast',
        separator: '/',
        fileExists: (p) => false,
      ),
      'libcc_ffi.so',
    );
  });

  test('Windows: lib\\ first, then next to the exe, then plain name', () {
    const dir = r'C:\cc';
    expect(
      coreLibraryCandidates(os: CoreOs.windows, exeDir: dir, separator: r'\'),
      [r'C:\cc\lib\cc_ffi.dll', r'C:\cc\cc_ffi.dll'],
    );
    expect(
      resolveCoreLibraryPath(
        environment: const {},
        os: CoreOs.windows,
        executableDir: dir,
        separator: r'\',
        fileExists: (p) => p == r'C:\cc\cc_ffi.dll',
      ),
      r'C:\cc\cc_ffi.dll',
    );
    expect(
      resolveCoreLibraryPath(
        environment: const {},
        os: CoreOs.windows,
        executableDir: dir,
        separator: r'\',
        fileExists: (p) => false,
      ),
      'cc_ffi.dll',
    );
  });

  test(
    'Android and iOS: never look at files, plain name for the system loader',
    () {
      for (final os in [CoreOs.android, CoreOs.ios]) {
        expect(coreLibraryCandidates(os: os, exeDir: '/x'), isEmpty);
      }
      expect(
        resolveCoreLibraryPath(
          environment: const {},
          os: CoreOs.android,
          executableDir: '/x',
          fileExists: (p) => true,
        ),
        'libcc_ffi.so',
      );
      expect(
        resolveCoreLibraryPath(
          environment: const {},
          os: CoreOs.ios,
          executableDir: '/x',
          fileExists: (p) => true,
        ),
        'libcc_ffi.dylib',
      );
    },
  );

  test('the environment variable wins on every platform', () {
    for (final os in CoreOs.values) {
      expect(
        resolveCoreLibraryPath(
          environment: const {ffiLibraryEnvVar: '/tmp/custom.lib'},
          os: os,
          executableDir: '/x',
          fileExists: (p) => true,
        ),
        '/tmp/custom.lib',
      );
    }
    // An empty value is ignored.
    expect(
      resolveCoreLibraryPath(
        environment: const {ffiLibraryEnvVar: ''},
        os: CoreOs.linux,
        executableDir: '/x',
        separator: '/',
        fileExists: (p) => false,
      ),
      'libcc_ffi.so',
    );
  });
}
