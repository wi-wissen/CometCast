# Bundled Office fonts

The Office converter (`cc-office-worker`, engine `office2pdf` on Typst) must lay out documents with the same
metrics as the Microsoft fonts, which cannot be redistributed. It therefore carries the metric-compatible
open fonts below. They are compiled into the program (`src/fonts.rs`, as gzip files, about 3.7 MB for the
20 faces; the plain TrueType files would be 7.4 MB) and handed to the engine as `ConvertOptions::font_bytes`,
which takes priority over system fonts. **Nothing is downloaded at run time** and no system font is needed
for these families.

| Office font | Replaced by | Licence |
| --- | --- | --- |
| Arial | Liberation Sans | SIL OFL 1.1 |
| Times New Roman | Liberation Serif | SIL OFL 1.1 |
| Courier New | Liberation Mono | SIL OFL 1.1 |
| Calibri | Carlito | SIL OFL 1.1 |
| Cambria | Caladea | SIL OFL 1.1 |

The name mapping itself is the substitution table of `office2pdf` 0.10 (`src/render/font_subst.rs`); it already
lists Calibri to Carlito, Cambria to Caladea and the Liberation families for Arial, Times New Roman and Courier New.
No table of our own is needed. Other Office faces (for example Aptos) are not covered by these files; the engine
falls back to its own built-in faces for them (Noto Sans and Noto Serif, and the Typst fonts), so layout of such
documents can differ from Word.

## Licences

All three families are under the SIL Open Font License 1.1. The licence and author files sit next to the fonts and
must stay with every binary that contains them (they are also part of `THIRD_PARTY_LICENSES.md` and
`THIRD_PARTY_NOTICES.md`):

| Family | Licence file | Copyright holders (from the file) |
| --- | --- | --- |
| Liberation 2.1.5 | `LICENSE-Liberation.txt`, `AUTHORS-Liberation.txt` | Digitized data copyright 2010 Google Corporation (Arimo, Tinos, Cousine); copyright 2012 Red Hat, Inc. (Reserved Font Name "Liberation") |
| Carlito 20230309 | `OFL-Carlito.txt`, `AUTHORS-Carlito.txt` | Copyright 2013 The Carlito Project Authors (Reserved Font Name "Carlito") |
| Caladea 20200211 | `OFL-Caladea.txt`, `AUTHORS-Caladea.txt`, `CONTRIBUTORS-Caladea.txt` | Copyright 2012 The Caladea Project Authors |

The files are used **unmodified** (only compressed with gzip); the fonts keep their Reserved Font Names. Caladea is
OFL 1.1 in the 20200211 release read here (`OFL.txt` in the archive). The Google Fonts catalogue lists Caladea under the
Apache licence for an older release; the archive's own licence file is what applies to the bytes in this folder.

## Where the bytes come from

Downloaded on 2026-11-07 through the build proxy from the Debian archive (`deb.debian.org`), which carries the upstream
release sources. GitHub itself was not reachable from the build environment, so the upstream GitHub release tarball of
Liberation (which contains the built TrueType files) could not be used; its source tarball contains only the FontForge
sources, so the built files come from the Debian binary package instead (built by Debian from those sources).

| Archive | SHA-256 |
| --- | --- |
| `fonts-liberation_2.1.5-3_all.deb` (Liberation 2.1.5 TTF files) | `3d5c47480067b9615f6bf9b0eda6eefcab9d837af609827015445e8e759b5b8c` |
| `fonts-liberation2_2.1.5.orig.tar.gz` (upstream sources, licence and authors files) | `9a823ccb33c8a8a865e84b982bcdc44d03ba4914adb91e6000df035dc0e55936` |
| `fonts-crosextra-carlito_20230309.orig.tar.gz` (upstream) | `72f4f92c4e71c82ba261230ae7aa2e7a3f5fa85a3e8e820288d7eeb1ae55f6dc` |
| `fonts-crosextra-caladea_20200211.orig.tar.gz` (upstream) | `2737242dbeb1e7fc728a9e3ef5509af3513666a826f4d40205c2e5ce3a7768ea` |

URLs: `https://deb.debian.org/debian/pool/main/f/fonts-liberation/fonts-liberation_2.1.5-3_all.deb`,
`.../fonts-liberation2/fonts-liberation2_2.1.5.orig.tar.gz`,
`.../fonts-crosextra-carlito/fonts-crosextra-carlito_20230309.orig.tar.gz`,
`.../fonts-crosextra-caladea/fonts-crosextra-caladea_20200211.orig.tar.gz`.

## Checksums of the files

`.ttf.gz` was made with `gzip -9n`. The check `node tools/check-fonts.mjs` inflates every file and compares the
SHA-256 of the TrueType bytes with this table (and the SHA-256 of the `.gz` file).

| File | SHA-256 of the TrueType file | SHA-256 of the `.gz` file |
| --- | --- | --- |
| `Caladea-Bold.ttf` | `ae3cb2dcbc925809dd29d2a44e9802211cab66be541bacbfc9c08c74b27c3742` | `34ae505f789589c709517bf7a9a091241a5cc581da14203ed76113010837d76f` |
| `Caladea-BoldItalic.ttf` | `ccabaa7b7e2fdf253d2b1a5fa699dd8a3df8d835a9eb285ad82631a677eb76c0` | `9cbec21fdd60fa48361c773e10ecc01b9922c932889133939f19b2f53a7c189c` |
| `Caladea-Italic.ttf` | `4359a8e24f748b6447b1ff6d7a174febe70961d29f8bb8634b56dacd740a3deb` | `a701321d10dce40e28f75656a5510fe48e12099455ef2a8a41341bc0200b41b6` |
| `Caladea-Regular.ttf` | `f1e899278b7b4491aba5b6a8253c4b04c050cc59b21865be5c37559a775153cd` | `a8ed9141f19df59ebb36fde26fac03f9ef6f13ea88e476ed272f4a1f1feaaca5` |
| `Carlito-Bold.ttf` | `bb5d20f79b82599ec72983597437373a80f2d2085fa91fc144fd74e876a594db` | `ee3303fc30e64b5942762757527d95cb92ef17987c92205abb40348d880bcde5` |
| `Carlito-BoldItalic.ttf` | `b32928186c119599e03ca6a1ffc680fdcb7fac95772f4b95d989cf6cd3861517` | `10580c14a60d193c0498e4ae3562f0050bb7258ae8c402b23828c547b8073371` |
| `Carlito-Italic.ttf` | `0b019225e58d702bfedcbd35c21696769f8ee115cb6343f84c2f240312450d1c` | `f23b2d9e880bf748117b53ebae1ad204599b44de16bd46e6b3194d8d4b5a0f71` |
| `Carlito-Regular.ttf` | `f6418f708baede9789daef5d458c0f53d2a888af9820e8062934e504fedc6595` | `8933f886fb737b769dbf3ce050f70380423c467b99338b31048d8073e1fdb85e` |
| `LiberationMono-Bold.ttf` | `ddce09ee3483b2d5faf61f0baba8592e76e0ac6dd4ab36683e5334c288bf5eef` | `5b6e8b7371711a4e421bf727dace1f8286078f9d0c4011b52099e9ffd1c7b47f` |
| `LiberationMono-BoldItalic.ttf` | `2ec6f3e5bd9b24c7fb04c229a1ee64f600e4072c03a041c96c2d62974c2ede2c` | `2a33ab125a219291950fcf558473c90bf68d74647b2a3451bbf4d906eb2d8f01` |
| `LiberationMono-Italic.ttf` | `016743fc425a1b61eb349c72c02f3da6a1349d1da7a5d9aded27321397eed955` | `f8454b772a74885be35bd920416289bfffeb45a4dae7e6b42ea5a03cf58c3bd7` |
| `LiberationMono-Regular.ttf` | `5883330d94debd992952cd8f0571b225f478c2d797d3f36c7521b0a5c9bde0f2` | `8f698cbd51b44feea9c8923bc4cc3796ec5695e7ccc7a5f00fd3cc0ac8cbef50` |
| `LiberationSans-Bold.ttf` | `1b5f2da6f4cadce4c05b9ecebe3a6fcd374eb95ae443605e799f4c3287978939` | `d2a5a573d3d6029362eae4c25d6ac58689d33385b518200295246be96b8a39ce` |
| `LiberationSans-BoldItalic.ttf` | `352e9dc2be89f210dbd3c0fd5dfaf97135de70e0beb5f22378601861462493a7` | `ab1ab6ab424c4f4023457249f9f2592a565a58ae9c28d0e7d14895fdd403c774` |
| `LiberationSans-Italic.ttf` | `93ff5260834141b6f9d8f2b846d427f3a5dc492dbdce8ebdad406e38a38f34bb` | `16db053d177c746e1bbb47b631e9dec7787a2984d1c60b0439e049fad5b6500e` |
| `LiberationSans-Regular.ttf` | `bade59d822652f76e6941aa87b40a87c13d1cc70db98ededb5011127efafd1d3` | `b24113f663b97dee54cc97cf89711830130374a5a0f36dcd02b4aff74f9aae8d` |
| `LiberationSerif-Bold.ttf` | `443a4dbd694b6e4689925947b456e0532e10b19c90fa32eb39ea6c2cd9db9ebd` | `718ab03103e250cdd1ea37b99131170327de59d2dcab0387ec3b42c001104a19` |
| `LiberationSerif-BoldItalic.ttf` | `4b2635d2ec4d992373dcae577c742a98ecfd841ffc1b8a103b5f10db163dbc7a` | `460d1d8760bbb9214c7b4c89316af83bfb8b144463094edce2b34b1818d14c48` |
| `LiberationSerif-Italic.ttf` | `de18fe26337e03241952dca46398d5b66f3c5738d080bfd284f66e67b926f597` | `19849e4bb172652ec690010963b01dc9b4afd8771644ee05422c4826c09f6934` |
| `LiberationSerif-Regular.ttf` | `9caef765d2e891c10dd73658894f01e660fe0c1c83e0bda7d6edf561d8f623d4` | `cec233cf48b176d25e4b28f9a8871e55a0d93471a4f85fe865b5f542b22ba98a` |

## Test

`cargo test -p cc-office` checks that all 20 files inflate to TrueType (`fonts::tests`) and converts a .docx that names
Calibri, Cambria, Arial, Times New Roman and Courier New and reads the font list of the PDF with `pdffonts`
(`office_fonts_are_replaced_by_the_metric_compatible_ones`; skipped loudly where `pdffonts` is missing).
