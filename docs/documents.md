<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Documents: PDF rendering (`cc-docs`)

Status: PDF part implemented and tested on generated files only. Office conversion, the WASM
build, speaker notes and the Flutter UI are **not** done.

## Idea (from the plan)

A PDF is sent **once as a file**, not streamed as video. Every device that shows it renders
the pages itself with the same renderer, [hayro](https://github.com/LaurenzV/hayro) (pure Rust).
The presenter's view (current page, next page, notes, buttons) uses the same renderer; only
the page number travels, as `ControlCmd` from `cc-proto` (`NextPage`, `PrevPage`,
`GoToPage { page }`, `Stop`; first page is 1). How a file travels over the control DataChannel
(image and link today) is in [web-m2.md](web-m2.md); sending a PDF file there is **not
implemented**.

## What the crate offers

| Item | Purpose |
| --- | --- |
| `Document::open(bytes)` / `open_with_limits` | Checks limits **before** parsing, then parses on a guarded worker thread |
| `page_count()`, `page_size(i)` | Pages (at least 1) and size in points after rotation and crop box. Indices are 0-based |
| `render_page(i, width_px)` | RGBA picture on opaque white; height follows the page shape. `render_page_with` adds a timeout override and a cancel flag |
| `thumbnail(i, max_side)` | Picture that fits into a square |
| `notes(i)` | Always `None` for a PDF. Speaker notes come from PPTX later (`cc-office`, not built) |
| `DocumentSession` | Receiver page state (1-based). `apply(&ControlCmd)` returns `PageChanged(n)`, `Unchanged`, `Stopped` or `Ignored`. Out-of-range pages are **clamped** (0 becomes 1, too large becomes the last page); after `Stop` every command is ignored |
| `encode_png`, `encode_webp_lossless`, `validate_webp` | Picture helpers, pure Rust (see below) |
| `abandoned_renders()` | How many timed-out renders still run inside hayro |

Command line (`bins/cometcast-cli`, module `docs.rs`):

```
cometcast-cli docs info <file.pdf>
cometcast-cli docs render <file.pdf> [--page N] [--width PX] --out <out.png|out.webp>
```

`--page` starts at 1, `--width` defaults to 1920. WebP output is lossless.

## Limits and safety

| Limit | Default | Where |
| --- | --- | --- |
| File size | 100 MiB | before parsing |
| `%PDF-` header | within the first 1024 bytes | before parsing |
| Pages | 5000 | after parsing the page tree |
| Output side | 8192 px (hard cap 65535, hayro's `u16`) | before rendering |
| Output area | 7680 x 4320 px | before rendering |
| Open time, render time | 10 s each | worker thread |
| Worker stack | 64 MiB | worker thread |
| Abandoned renders alive | 4, then `Busy` | process wide |

**Uncertain:** all defaults are guesses for a classroom setting, not derived from
measurements on real devices. They are public fields of `Limits`.

How hostile input is handled:

- Parsing and rendering run on a worker thread with a large stack, inside `catch_unwind`. A
  panic becomes `DocError::Panicked`, and the worker is replaced on the next call. This does
  not catch a stack overflow or an out-of-memory abort (the process would end). The tests
  below did not trigger either, but that is no proof (see "What is not covered").
- **hayro cannot be interrupted** (no cancel hook found in its API). On timeout or cancel the
  caller gets `Timeout` or `Cancelled` at once and the worker is *abandoned*: the thread keeps
  running until hayro returns and its result is dropped. The count of such threads is limited
  (`Busy`), so a hostile file cannot start unlimited background work through repeated renders.
- Memory is **not** capped in the process. An image with a declared size of 2 000 000 000 x
  2 000 000 000 pixels pushed the test process to about 2.7 GiB resident (see numbers) before
  hayro gave up with a caught panic in the abandoned thread. A real memory cap needs a child
  process with an operating system limit; that is **not built**. For files from untrusted
  people on small devices this is the largest open risk of this crate.
- Encryption: a file that needs a password gives `DocError::PasswordProtected`. Files that
  are encrypted with an **empty user password** open normally (tested with an RC4 128 bit file
  made by reportlab). hayro 0.9's source also contains AES-128 and AES-256 code; those were
  not tested here. hayro's own README says encrypted files are unsupported, but the code
  shows a decryption module; the tests above are what was observed.

## Tests (what was really run)

`cargo test -p cc-docs` (debug profile):

- `tests/render.rs`: renders the generated PDFs and checks size, colour at chosen points,
  non-blank pages, rotation (0/90/180/270), embedded JPEG, embedded TrueType font, gradient,
  empty-user-password encryption, password refusal, thumbnails, PNG and WebP round trip.
- `tests/hostile.rs`: truncation at 60 positions of each of 5 files; 400 random byte
  mutations (deterministic seed; 327 of 400 still rendered fully, the others returned errors,
  no panic reached the caller); nested arrays (200 000 deep) in a content stream; nested
  dictionaries (100 000 deep); page tree that contains itself; two page nodes pointing at each
  other; page tree 20 000 levels deep; page count over the limit; media boxes of 1e9 x 1e9,
  of 32 digits, negative, zero and malformed; random bytes after a valid header; self-drawing
  form XObject (finished in about 3 ms without error; how hayro guards against the loop was not checked).
- `tests/limits.rs` (own process, because it leaves abandoned renders behind): 256 MiB of
  zeros as a flate-compressed content stream (261 KB compressed); images with declared sizes
  of 100 000 x 100 000, 65 536 x 65 536 and 2e9 x 2e9; an exponential form blow-up
  (three nested forms with 60 calls each) with a 150 ms limit; cancel flag.

Findings from these tests:

| What | Result |
| --- | --- |
| Deep nesting (content stream, objects, page tree) | No panic, no stack overflow in a 64 MiB worker. hayro has its own depth limit (`MAX_OBJECT_NESTING_DEPTH` is 512 in `hayro-syntax`) |
| Page tree loops | Returned without hang |
| 1e9 x 1e9 point media box | Rendered at 1920 px wide (the scale is derived from the box), no big buffer |
| Flate bomb, 256 MiB in a content stream | Finished in about 5 s (debug); process peak about 336 to 360 MiB: **hayro decodes the whole stream into memory, no ratio limit** |
| Image 2e9 x 2e9 | Both calls hit the 5 s limit (`Timeout`); process peak about 2.7 GiB; the abandoned worker ended with `OutOfMemory(8000000000)` unwrap panic inside hayro (`image.rs:649`), caught by `catch_unwind` in the worker |
| Form blow-up, 150 ms limit | `Timeout` after 150 ms; cancel flag returned after about 50 ms |

Numbers measured on this machine (shared 4-core virtual machine, another job was using the
CPU at the same time, so they are **noisy**; release is the workspace `[profile.release]`:
`opt-level = "s"`, LTO, which is size-optimised and not the fastest setting). Render of one
page at 1920 px width, median of 5 renders after the first:

| File and page | Output | Debug (dev) | Release |
| --- | --- | --- | --- |
| minimal.pdf (200 x 100 pt, 2 rectangles, 2 letters) | 1920 x 960 | 51 ms | 7.7 ms |
| classroom.pdf p1 (text, vectors) | 1920 x 2715 | 234 ms | 25 ms |
| classroom.pdf p2 (JPEG) | 1920 x 2715 | 265 ms | 7.1 ms |
| classroom.pdf p3 (Vera font, gradient) | 1920 x 2715 | 157 ms | 13.7 ms |
| rotated_pages.pdf p1 (one box) | 1920 x 2880 | 123 ms | 22.4 ms |
| rotated_pages.pdf p2 (rotated 90) | 1920 x 1280 | 48 ms | 2.7 ms |
| two_pages_one_rotated.pdf p2 | 1920 x 3840 | 204 ms | 28.3 ms |
| encrypted_empty_user_password.pdf | 1920 x 2715 | 113 ms | 6.3 ms |

Open time was 0.2 to 1 ms for these tiny files. These pages are far simpler than classroom
PDFs; **no statement about real classroom files, the Apple TV or weak mini PCs follows from
them.** The plan's goal (a page in under 500 ms on weak hardware, measurement point 7) is
**not verified**.

Picture sizes for classroom.pdf page 1 at 300 px width: PNG 9 543 bytes, lossless WebP 16 678
bytes. So for this page the lossless WebP of `image-webp` was **larger** than the PNG; one
sample proves nothing general.

## Image encoders: what was chosen

- `png` 0.18 (pure Rust, MIT OR Apache-2.0) for PNG.
- `image-webp` 0.2 (pure Rust, MIT OR Apache-2.0) for **lossless** WebP. It cannot encode lossy
  WebP. Lossy WebP would need libwebp (C code); the plan mentions an `image` + `webp` pair, and
  the `webp` crate wraps libwebp. This crate does not use it, so no C code comes in. (The test
  generator for video frames in `cc-sender` uses Pillow's libwebp, but that is a dev-time tool.)
- `validate_webp` only parses the header (size, not animated, pixel budget). Pixels are not
  decoded, so a file with a valid header and a corrupt body passes.

## Build and platforms

- hayro 0.9.0 (newest found with `cargo info` on 2026-11-06; the plan names 0.8.0) is
  `Apache-2.0 OR MIT` and declares `rust-version = 1.92`. The workspace `rust-version` was
  therefore raised from 1.88 to 1.92. **Uncertain:** not measured with a real 1.92 toolchain
  (1.97 was used). The same applies to 0.8.0 (also 1.92).
- hayro default features are kept: embedded substitute fonts (Foxit, about 240 KB per its
  documentation) and embedded CMaps. Binary size was not measured.
- `cargo check -p cc-docs --target wasm32-unknown-unknown` **passes** (checked on this
  machine). That proves that the code compiles for wasm, nothing more: it was **not linked,
  run or measured** in a browser, so "hayro runs as WASM" is still unverified (measurement
  point 7). On wasm the crate has no threads: it parses and renders inline, re-parsing the file
  for every render, without time limit, cancel or panic protection (`catch_unwind` cannot
  catch there). Only the pre-checks (size, pages, output size) apply.
- Not built or checked: Android, iOS, tvOS targets.

## Fidelity of hayro

What the tests above show: standard fonts (Helvetica, Times) render through hayro's built-in
substitutes, an embedded TrueType subset renders, vector shapes, curves, a shading and a
DCT (JPEG) image render at the expected places and colours, and rotation works. I looked at
the PNG output of the three classroom pages; they look correct.

**Uncertain / not tested:** the plan's list of hayro gaps (no knockout groups, no non-embedded
CID fonts, experimental status), CJK and right-to-left text, transparency groups, JPEG 2000,
JBIG2, scanned pages, very large real files, PDFs with form fields or annotations, and the 20
real classroom PDFs. No fallback to pdfium-render exists in the code.

## Not done

- 20 real PDFs (point 7), fidelity comparison with another viewer, WASM build and size.
- Child-process isolation with a memory limit.
- Sending the PDF as a file over the transport; receiver UI; page cache; presenter view.
- Office to PDF (`office2pdf`), notes from PPTX.
- Annotations (`lopdf` in the plan).

The generated test files and their provenance are in
[`crates/cc-docs/tests/data/PROVENANCE.md`](../crates/cc-docs/tests/data/PROVENANCE.md).
