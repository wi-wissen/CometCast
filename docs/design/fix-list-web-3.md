# Fix list 3: web app after the third design review

From the review of 2026-11-07 of main 7d33eb7 (308 screenshots in `/tmp/shots3`: 1280 × 720 and phone 390 × 844; display dark / light / OLED, presenter light / dark; keyboard-driven focus captures). Decisions: spec [first-run-and-display.md](first-run-and-display.md) section 13, items 57–69; answers to F1–F8 in [implementation-questions.md](implementation-questions.md).

**Verdict:** the web display and the web presenter pages are **design-complete** once this list is done (decision 13.69). No further design review round is needed; check each item against its screenshot. The list is short on purpose: items 1–4 are visible defects, 5–9 are polish.

All 16 items of [fix-list-web-2.md](fix-list-web-2.md) pass: relay confirmation on the canvas alone and as its own presenter page (chip "7UZ FHX" after Cancel), QR box edge and empty state, PDF page row above Stop with "Page 1 of 3" and a disabled Previous, no-app texts, `error.other`, busy and STUN/TURN pages, content states on black with the pill and the page-error box, wizard chip reselection and field focus ring, everything on the canvas on the phone, media warning lines, laptop top padding, fixed wizard card. Item 3 (du set) and item 8 (reconnect timing) are covered by their checks, not by stills.

`*` in a screenshot name means every theme. Paths are under `apps/web/src`. Copy changes go into `locales/en.json`, `locales/de.json` and `locales/de-du.json` with the values of spec 11.7 (whichever German file is the shipped du set after the parallel "du" change, both sets get the value).

## Must fix

1. **Frozen badge is covered by the toolbar.**
   Where: `components/display/PresentingStage.vue` (or the frozen badge component), `styles/display.css`.
   Found: `display-*-frozen` (1280 and phone): the badge "Frozen. The live…" sits under the toolbar and shows through its 92 % fill.
   Change: spec 3.5 / decision 13.65: hide the frozen badge while the annotation toolbar is visible (same 400 ms fade, inverse); at rest it shows as before. The Freeze button keeps its selected `--acc` border.
   Check: `display-*-frozen` (controls shown) has no badge and a selected Freeze button; a new `display-*-frozen-rest` (no input for 6 s) shows the badge alone, top left.

2. **Text on black: "Screen hidden" and the PDF opening line (F1).**
   Where: `styles/display.css` (tokens), the black-screen hint, `components/DocumentView.vue` / the transfer pill.
   Found: `display-1280-light-black`: `--mut` `#343C51` on `#000000` is unreadable.
   Change: add `--onblack: #A1A9BA` to all three themes; `toolbar.blackHint` 15/400 `--onblack`, centred, no box. Remove the separate "Opening {name}…" line on black: while a received PDF is opened, the transfer pill shows `receive.document.loading` in the same place until the first page is drawn. DE value `{name} wird geöffnet …` (space before `…`) in both German sets.
   Check: `display-*-black` readable in all themes (light shot looks like the OLED one); new `display-*-pdf-opening` shows the pill with "Opening slides.pdf…".

3. **Media link card: one button row, matching words.**
   Where: locales (`receive.media.play`, `receive.media.hint`), `components/ContentOverlay.vue`.
   Found: `display-*-linkcard-media`: "Play from media.example.org" pushes Dismiss onto a second row; the hint says click "Play" with straight quotes while the button says something else.
   Change: `receive.media.play` = EN "Play", DE du "Abspielen", DE Sie "Abspielen". EN `receive.media.hint` = `This link points to a media file. It is loaded from {host} only if you click “Play”.` (typographic quotes). The player header keeps `receive.media.from` with the host.
   Check: `display-*-linkcard-media` and `-media-local`: [Play, Open, Dismiss] on one row at 1280 and on the phone canvas; no straight quotes.

4. **Transfer pill: one line, never over the name tag or End (F7).**
   Where: the transfer pill component, `styles/display.css`.
   Change: spec 3.5 / decision 13.63: max width 340 ref, `white-space: nowrap`; shorten only the file name, in the middle, keeping its start and its extension (`holiday-pho…2026.png`); the rest of `receive.transfer` and the percentage are never cut. Same for `receive.document.loading`.
   Check: a new shot `display-*-transfer-long` with the name `a-very-long-file-name-from-the-holiday-in-2026-final-version.png` and the sender name "Maximilian-Alexander" shows pill, name tag and End side by side without touching, on 1280 and phone.

## Should fix

5. **Wizard card top at 40.**
   Where: `components/display/SetupWizard.vue`, `styles/display.css`.
   Found: `display-*-setup-1-name`: with the chips on two lines the card ends ~10 ref above the canvas edge.
   Change: decision 13.64: steps 1–4 top edge fixed at 40 ref (welcome stays centred).
   Check: `display-*-setup-1` … `-4`: logo and dots at the same place in every step; ≥ 24 ref below the card on step 1, also with `lang=de`.

6. **Link address in `--fg`.**
   Where: `components/ContentOverlay.vue` (link card address).
   Change: decision 13.61: the address is 18 mono `--fg` (it was correctly `--mut`).
   Check: `display-*-linkcard*`: the address reads like the relay host in `display-*-relay-confirm`.

7. **Wizard step 2: the selected card looks selected.**
   Where: `components/display/SetupWizard.vue`, `styles/display.css`.
   Found: `display-*-setup-2-mode`: focus on "Approval on" while "Open" is selected shows two amber outlines of similar weight.
   Change: decision 13.66: selected card = 2 px `--acc` border + selected tile fill (3.2; OLED: no fill); unselected = 2 px `--line`, `--panel2`.
   Check: `display-*-setup-2-mode`: Open is tinted (Dark, Light) and Approval on carries only the focus ring.

8. **Dialogs over a presentation cover the controls.**
   Where: `components/display/PresentingStage.vue`, the dialog layer, `styles/display.css`.
   Found: `display-*-linkcard*`: name tag and End stay at full strength above the scrim.
   Change: decision 13.68: while a join request or link card is open, name tag, End, toolbar, frozen badge and pill are under the scrim and `inert`.
   Check: `display-*-linkcard`: the name tag is dimmed like the picture; a click on End does nothing while the card is open.

9. **Locked QR edge in Light.**
   Where: `components/display/QrCode.vue`.
   Change: decision 13.67: apply the 25 % to the code (quiet zone and modules), not to the box border.
   Check: `display-1280-light-idle-locked`: the thin grey edge is as visible as in `display-1280-light-idle`.

## Screenshot set

10. **Focus captures with Shift; no stale toasts (F6).**
    Where: `apps/web/e2e/screenshots.spec.mjs` (`dshot`).
    Change: press `Shift` instead of `ArrowRight` in `dshot` (shows the ring, moves nothing); keep the arrow only where a moved focus is the point (`*-presenting-controls`, `*-focus-setup-name-field`, `*-setup-1-name-emptied`). Make sure any key press, a modifier alone included, shows the ring (decision 13.62). Before `*-presenting-pdf`, `*-pdf-page-error`, `*-black` and `*-frozen`, wait until the toast "The image arrived incomplete…" is gone (it belongs to `*-toast-with-toolbar`), and confirm by a test that a toast leaves after 3.2 s. Add the shots of items 1, 2 and 4.
    Check: `display-*-relay-confirm` shows the ring on Cancel, `display-*-linkcard*` on Open, `display-*-setup-2-mode` on the selected card.
