# Fix list 4: German widths, desktop entry, small leftovers

From the designer's review of main f99ba94 and the 479 screenshots in `/tmp/shots-flutter4`. Decisions: spec 13, "Decisions after the third Flutter round" (N60–N72); answers: [implementation-questions-flutter.md](implementation-questions-flutter.md) J.

**Already right, no work:** every nearby state on phone and laptop, light and dark, EN and DE du (text lengths fit everywhere except A1); links under Connect (12, 44 apart, Windows variant); `gone` line after the refill; connecting lock of Scan, fields and Connect; relay page; `error.capture` in all four "why" variants, phone and laptop, EN and DE (button row, order, styles, grey icon); the unsupported codec state; all display and other presenter shots (unchanged).

Each item: **What**, **Where**, **Acceptance**.

### A1. Laptop columns and row line breaking (N61, N62)
- **What:** the start page with the nearby card splits the width after the 32 gap 4 : 5 (left at least 260). Titles break only at spaces or after hyphens (a word is split only when it alone is wider than the line). A row whose text column would be under 160 puts the code on its own line under the subtitle.
- **Where:** `FirstRunPage` laptop branch in `lib/src/presenter/pages.dart`; the nearby row widget.
- **Acceptance:** `nearby-laptop-light-de-found` (1100 wide): „Besprechungsraum / Elbe“ breaks between the words; "Mit Freigabe · / über cast.musterfirma.de" with the host whole; heading may wrap (N63). Same at a 720-wide window (new shot) with no word split inside; widget test for the code-below fallback at a narrow row.

### A2. Host names break only after a dot (N62)
- **What:** when `{host}` is inserted into running text (`nearby.via`, relay page body), add U+200B after each "." of the value. Not in ARB text, not in the mono host line or fields.
- **Where:** where `nearbyVia` and the relay body are filled.
- **Acceptance:** widget test with `cast.abteilung-nord.musterfirma-gruppe.de` in a narrow row: lines break only after dots or the hyphen; "über" stays with the first part.

### A3. A connecting row keeps its size (N64)
- **What:** the right slot keeps the code's width while connecting; the spinner (20) sits at its right edge.
- **Where:** the nearby row widget.
- **Acceptance:** `nearby-*-de-connecting` (phone and laptop): the connecting row has the same height and title lines as in `*-de-found`; the rows below do not move.

### A4. macOS German permission text (N70)
- **What:** `de.lproj/InfoPlist.strings` with `NSLocalNetworkUsageDescription` = du text of `ios.localNetworkUsage` (Sie build: Sie text).
- **Where:** `apps/flutter/macos/Runner` (variant group in the project file).
- **Acceptance:** the built app bundle contains `de.lproj/InfoPlist.strings` with that text.

### A5. Linux desktop entry and icon (N68)
- **What:** add to `de.cometcast.cometcast.desktop`: `GenericName`, `GenericName[de]`, `Comment`, `Comment[de]`, `Icon=de.cometcast.cometcast`, `Categories=Office;Presentation;`, `Keywords`, `Keywords[de]` (values in native-apps.md 20.3). Install `assets/logo/cometcast-app-icon.svg` and the 32/64/512 PNGs into hicolor under that name; the GTK window sets the icon name.
- **Where:** `apps/flutter/linux/` (desktop file, `CMakeLists.txt` install rules, runner).
- **Acceptance:** `desktop-file-validate` passes; after install the launcher and the portal picker show the comet icon and "Comet Cast".

### A6. Screenshots (N71)
- **What:** drop `ErrorKind.capture` from the error loop of `test/screenshots_test.dart` (its `presenter-*-error-capture` shot shows a placeholder "why"); add the 720-wide laptop DE shots of A1 and `nearby-phone-light-de-found` at 360 wide.
- **Acceptance:** no `presenter-*-error-capture.png` (the `-unavailable` one stays); new shots as listed.

Not design work (no review needed): the device test plans (Android, iPad, Apple TV, macOS, Linux Wayland, Windows) must pass before release; the Apple TV room name comes with its setup wizard (N69).
