# Comet Cast - Flutter app (phone, tablet, desktop, later Android TV)

> **UNVERIFIED: written without a Flutter/Xcode toolchain; first build may need fixes.**

Skeleton only: a Material 3 home screen with three placeholder rows
(Send screen, Send file or link, Receive on this device), English + German
translations, light/dark following the system. License: GPL-3.0-or-later.

## What is missing

The platform folders `android/ios/linux/macos/windows` are **not generated**.
Run these once on your own machine (Flutter SDK installed), inside `apps/flutter`:

```sh
cd apps/flutter
flutter create --project-name cometcast --org de.cometcast --platforms=android,ios,linux,macos,windows .
rm -f test/widget_test.dart   # template test from `flutter create`, does not fit this app
flutter pub get
flutter gen-l10n      # generates AppLocalizations into lib/l10n (git-ignored)
flutter test
flutter run -d <device>   # list devices with: flutter devices
```

`flutter create .` in an existing folder keeps our `lib/`, `pubspec.yaml` and
`test/`; if it overwrites `lib/main.dart` or `test/`, restore them from git (`git checkout -- lib test pubspec.yaml`).

Notes for a Vue developer: `pubspec.yaml` is like `package.json`;
`lib/main.dart` like `main.js`; widgets are like components; the `.arb` files
are like `locales/en.json`. `check-i18n` in `tools/` compares the `.arb` keys.

## Known uncertainties

- `l10n.yaml` writes generated code to `lib/l10n` (`synthetic-package: false`) and the
  code imports `package:cometcast/l10n/app_localizations.dart`. Not run against a real
  SDK; if the build complains, adapt to the error message.
- `intl: any` lets Flutter pick the version `flutter_localizations` needs. Other
  versions (`flutter_lints`, Dart SDK range) are not checked.
- `flutter create .` behaviour with existing files is not verified.

## Later (not in this skeleton)

- `flutter_rust_bridge` (Rust core) is intentionally **not** a dependency yet.
- Android TV / Google TV: D-pad focus (package `dpad`), 32-bit `armeabi-v7a`
  build, MediaCodec video surface. This is measurement point 1 / assumption A2
  in `docs/measurement-points.md`.
- Apple TV is a separate native SwiftUI app: see `apps/apple-tv`.
