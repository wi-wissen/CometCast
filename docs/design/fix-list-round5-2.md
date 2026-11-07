# Fix list round 5-2: after the round 5 review

From the designer's review of main 0154c13, the 238 screenshots in `/tmp/shots-flutter6` (`r5-*`) and the web footer shots in `/tmp/shots5-web`. Decisions: spec [15.14](first-run-and-display.md) (N97–N118); platform: [native-apps.md](native-apps.md) 21.10; answers: [implementation-questions-flutter.md](implementation-questions-flutter.md) L and M. New strings go into all three ARB sets with the values of spec 15.13; `tools/check-i18n.mjs` must pass.

**Already right, no work:** idle lines (all three variants, offline), request dialogs (AirPlay, Miracast), display About dialog on computers and TV, licence viewer layout and D-pad, presenter app settings (desktop, Android), About page with search and no-match state, TV rows and the connecting row, pair page with and without `pair.wrong`, TV "What to show" (Cast, AirPlay), `error.tv` Cast and AirPlay, share page connected and not connected, address states field/checking/error, iOS About-only entry, web footer on cometcast.de and self-hosted (apart from U8). DE du lengths fit everywhere except U1 and U2.

## Part 1: Flutter UI

| # | What | Where | Acceptance |
| --- | --- | --- | --- |
| U1 | Display Settings: DE `settings.receive.failed` = "Startet nicht. Aus- und wieder einschalten oder App neu starten." (Sie: same); line precedence noHw/noFeature/policy > locked > failed > sub; track colour switches with the knob in 120 ms (N97) | ARB (3 sets), `src/display/screens.dart` switch row, `src/display/kit.dart` switch | `r5-display-settings-failed-*-de` and `-locked-*`: every row one line, dialog ≤ 510 ref; `-locked-*` shows `settings.receive.locked` on both rows; `-on-*`: both tracks full `--acc` (shot after 200 ms) |
| U2 | Laptop brand row spans both columns; Settings at the right end of the content (N98) | `src/presenter/pages.dart` (`FirstRunPage` laptop branch, `PBrand`) | `r5-tv-rows-*-de`, `r5-start-desktop-brand-*-de`: "Comet Cast" on one line; button's right edge = nearby card's right edge |
| U3 | Address line in a 44-high box from the code field's bottom edge, both variants (N99) | `FirstRunPage` code part, share page | `r5-address-default-*` and `r5-address-policy-*`: the line text at the same y |
| U4 | Licence viewer: current row in OLED = 2 px `--acc` border + bold; pane focus ring outside the box, no text shift (N100) | `src/display/about.dart` | `r5-display-licences-oled-*`, `r5-display-licences-pane-oled-*`, `r5-display-tv-licences-pane-oled-*`: Cisco row marked; pane text at the same position in `-licences-*` and `-licences-pane-*` |
| U5 | PIN left-aligned with the body in both PIN dialogs (N110) | `src/display/screens.dart` PIN dialog | `r5-display-pin-airplay-*`: "4821" starts at the body's left edge, like `-pin-miracast-*` |
| U6 | "Flutter engine" subtitle `about.flutterSub` (N102) | `lib/licences.dart`, ARB | Row two lines in `r5-about-*` and `r5-display-licences*` |
| U7 | About and licence text page: phone Back in the fixed bottom area; in-content links left-aligned (`about.*` links, `presenting.ownPdf`) (N104) | `src/presenter/about.dart`, `PresentingPage` | `r5-about-none-*`, `r5-licence-text-*`: Back at the bottom; `r5-about-android-*`, `r5-presenting-converted-*`: link at the content's left edge |
| U8 | Web footer separators: no-break space before "·" (N107) | `apps/web/src/components/presenter/StartPage.vue` | `start-cometcast-de-phone.png`: no line starts with "·" |
| U9 | Share page: both buttons disabled while converting; icon disc keeps its one-line position (N113) | `src/presenter/share.dart` | `r5-share-converting-*`: buttons in `p-disabled`; `r5-share-refusal-*`: disc top at the same y as in `r5-share-connected-*` |
| U10 | `error.tv.titleLost` heading for a lost TV (N114) | `src/presenter/pages.dart` error page, ARB | `r5-error-tv-lost-*`: "„Living room TV“ ist nicht mehr verbunden" |
| U11 | `about.publisher` = "Publisher: {name}" / "Herausgeber: {name}"; legal links each only with their URL (N108) | ARB, `src/presenter/about.dart`, `src/display/about.dart` | New shots `r5-about-android-branded-*` and `r5-display-about-branded-*` with the official branding: publisher, Privacy, Imprint; without branding as today |
| U12 | `tv.aria` = "{name}, {way}" + ", Beta" per 15.10; named picker filters `send.content.pickerTypesNoOffice`, `.pickerPdf`, `.pickerPhotos` (N103, N109) | ARB, `src/beta.dart`, `src/presenter/file_picker.dart` | Semantics test "„Apple TV“, AirPlay, Beta"; GTK dialog shows a filter name in every case |
| U13 | One request queue, FIFO by arrival (N111) | `display_logic.dart`, `receiver_controller.dart` | Test: an AirPlay request then a Comet Cast request → AirPlay shown first, the other next |
| U14 | Android share Cancel/Back → `ShareBridge.returnToCaller()` (N112) | `src/presenter/share.dart`, Kotlin `ShareBridge` | Joined app, share, Cancel: back in the sharing app; reopening Comet Cast shows the same join |
| U15 | Screenshots: Presenting shots ≥ 500 ms after input (N101); add the branded About shots (U11) | screenshot test | `r5-display-miracast-presenting-*` shows the name tag at full opacity |
| U16 | F9 after the Miracast device test; move `MiracastVideoView` first (N117) | `miracast_page.dart`, display code | As fix list round 5 F9 |

## Part 2: core and platform

| # | What | Where | Acceptance |
| --- | --- | --- | --- |
| P1 | `NearbyScreenInfo.addresses` (N105); app hides TV rows by it | `crates/cc-discovery`, `crates/cc-ffi`, bridge, `nearby_controller.dart` | Fake announcement with address 192.168.1.20 and an AirPlay TV at the same address: one Comet Cast row, no TV row |
| P2 | UxPlay licence entry where UxPlay ships; release guard fails otherwise (N106) | `tools/gen-third-party.mjs`, `tools/release-guard.sh`, packaging (C10) | Packaged build with UxPlay: "UxPlay · GPL-3.0" second in the featured group |
| P3 | Cast trust anchors only with recorded source and licence; no skip option (N115) | `crates/cc-cast`, `THIRD_PARTY_NOTICES.md` | Without anchors only AirPlay TVs appear; with anchors a Chromecast passes the device check of `docs/cast.md` |
| P4 | AirPlay mirroring signal verified with a real iPhone (N115) | `docs/airplay.md`, device test plan | Plan records one start line per mirroring and the count reaching 0 |
| P5 | iOS share extension: predicate without `public.file-url`; section 13 icons as vector templates; 64 MiB PDF measurement; share sheet screenshots (N107, N116) | `apps/flutter/ios/ShareExtension`, `docs/ios.md` | Sharing a ZIP from Files does not list Comet Cast; screenshots of native-apps.md 21.10 item 9c reviewed against spec 15.3 |
| P6 | Windows Miracast device test incl. "no second PIN" (N110, N117) | `docs/miracast-device-test-plan.md` | Section I and 21.10 item 9b passed |
| P7 | Web display: fresh session on a second offer of a sender still in the room (native-apps.md 21.10 item 8) | `apps/web/src/core` receiver | Native presenter: Stop, then show a file → the web display shows the file |
| P8 | Apple TV About, dialog and licence viewer (round 5 C9, still open; spec 15.9 display part, N100 marking) and UxPlay/Miracast packaging (C10) | `apps/apple-tv`, packaging | As fix list round 5 C9/C10; tvOS shots in Dark, Light, OLED black |

Size: 16 Flutter UI items (one web line), 8 core/platform items.
