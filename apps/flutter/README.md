# Comet Cast - Flutter app (phone, tablet, desktop, later Android TV)

> **Verified with Flutter 3.47.9 (stable, Dart 3.13.7) on Linux: `flutter analyze` is clean and `flutter test` passes (2 widget tests). Not verified: builds for Android, iOS, macOS, Windows, Linux (no platform toolchains here).**

Skeleton only: a Material 3 home screen with three placeholder rows
(Send screen, Send file or link, Receive on this device), English + German
translations, light/dark following the system. License: GPL-3.0-or-later.

## Run

Platform folders (`android/ios/linux/macos/windows`) are generated and committed.
With the Flutter SDK installed, inside `apps/flutter`:

```sh
flutter pub get
flutter gen-l10n      # generates AppLocalizations into lib/l10n (git-ignored)
flutter analyze
flutter test
flutter run -d <device>   # list devices with: flutter devices
```

Notes for a Vue developer: `pubspec.yaml` is like `package.json`;
`lib/main.dart` like `main.js`; widgets are like components; the `.arb` files
are like `locales/en.json`. `check-i18n` in `tools/` compares the `.arb` keys.

## Known uncertainties

- `l10n.yaml` writes generated code to `lib/l10n` (`synthetic-package: false`) and the
  code imports `package:cometcast/l10n/app_localizations.dart`. Not run against a real
  SDK; if the build complains, adapt to the error message.
- `intl: any` lets Flutter pick the version `flutter_localizations` needs. Other
  versions (`flutter_lints`, Dart SDK range) are not checked.
- `flutter create .` was run once; it kept our `lib/` and `pubspec.yaml` and only added platform folders, a template test (removed) and analyzer excludes. The platform folders are now committed.

## Later (not in this skeleton)

- `flutter_rust_bridge` (Rust core) is intentionally **not** a dependency yet.
- Android TV / Google TV: D-pad focus (package `dpad`), 32-bit `armeabi-v7a`
  build, MediaCodec video surface. This is measurement point 1 / assumption A2
  in `docs/measurement-points.md`.
- Apple TV is a separate native SwiftUI app: see `apps/apple-tv`.
