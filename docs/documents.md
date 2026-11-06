<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Documents: PDF rendering (`cc-docs`)

Status: PDF part implemented and tested on generated files only. Rendering runs in a **child
process with memory and CPU limits** (since the security review, see "Limits and safety").
Office conversion, the WASM build, speaker notes and the Flutter UI are **not** done.

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
| `Document::open(bytes)` / `open_with_limits` / `open_with_options(bytes, limits, Isolation)` | Checks limits **before** parsing, then parses in a child process (`Isolation::Auto`, the default; `Process(WorkerSpec)` to name the worker; `InProcess` for the weak thread engine) |
| `is_sandboxed()`, `last_worker_peak_kib()` | Which engine is used; highest memory the parent saw from the last worker (diagnostics) |
| `worker_main()`, `WorkerSpec`, `WORKER_ENV` | The worker side and how the parent finds it (see "The worker process") |
| `page_count()`, `page_size(i)` | Pages (at least 1) and size in points after rotation and crop box. Indices are 0-based |
| `render_page(i, width_px)` | RGBA picture on opaque white; height follows the page shape. `render_page_with` adds a timeout override and a cancel flag |
| `thumbnail(i, max_side)` | Picture that fits into a square; both sides are at most `max_side` also for extreme shapes (a 10 x 10 000 pt page into 64 gives 1 x 64, distorted rather than too big) |
| `notes(i)` | Always `None` for a PDF. Speaker notes come from PPTX later (`cc-office`, not built) |
| `DocumentSession` | Receiver page state (1-based). `apply(&ControlCmd)` returns `PageChanged(n)`, `Unchanged`, `Stopped` or `Ignored`. Out-of-range pages are **clamped** (0 becomes 1, too large becomes the last page); after `Stop` every command is ignored |
| `encode_png`, `encode_webp_lossless`, `validate_webp` | Picture helpers, pure Rust (see below) |
| `abandoned_renders()` | How many timed-out renders still run inside hayro (in-process engine only; always 0 with worker processes, which are killed) |

Command line (`bins/cometcast-cli`, module `docs.rs`):

```
cometcast-cli docs info <file.pdf>
cometcast-cli docs render <file.pdf> [--page N] [--width PX] --out <out.png|out.webp> [--force]
```

`--page` starts at 1, `--width` defaults to 1920. WebP output is lossless. The input must be a
regular file of at most 100 MiB (a device such as `/dev/zero` is refused; it used to be read
until memory ran out). The output file is created new; an existing file is only replaced with
`--force`, and a symbolic link is never followed or replaced. The program renders by starting
itself as `cometcast-cli docs-worker` (hidden command, not for users), or the program named in
`COMETCAST_DOCS_WORKER`.

## Limits and safety

A PDF comes from a remote sender and is **hostile input**. hayro cannot be interrupted, and a
failed allocation aborts the whole process (`abort`, not a panic), so no in-process limit can
protect the receiver. The boundary is therefore a **child process**.

### The worker process

`Document::open` and `render_page` start `cc-docs-worker` (or any program that calls
`cc_docs::worker_main()`) once per call: the PDF goes to its stdin, one framed reply comes back
on stdout (`crates/cc-docs/src/wire.rs`: a fixed head, then page sizes or RGBA). The parent
checks every size in the reply against what it asked for before it allocates. Nothing is
abandoned: on timeout, cancel, memory excess or any error the parent calls `Child::kill()` and
`wait()`, which closes the pipes and ends the two helper threads.

| Platform | Memory | CPU / time |
| --- | --- | --- |
| Linux, Android | `sh -c 'ulimit -c 0; ulimit -t N; ulimit -v KiB \|\| exit 97; exec worker'` (address space); the parent also reads `VmHWM` from `/proc/PID/status` every 10 ms and kills above the cap | `ulimit -t` (timeout + 2 s) and the parent's wall-clock deadline |
| macOS and other Unix | `RLIMIT_AS` is not enforced on macOS, so only the parent watchdog: `ps -o rss= -p PID` every 100 ms, kill above the cap. A fast allocation can overshoot between two polls | `ulimit -t`, wall-clock deadline |
| Windows | No job object (that needs `unsafe`, forbidden here). Only the watchdog (`tasklist` every 250 ms) and the wall-clock kill. **Weaker, untested**: compile-checked with `cargo check --target x86_64-pc-windows-gnu`, never run | wall-clock kill |
| iOS, tvOS | No child processes. In-process thread engine only: **the weakest platform** (see below) | thread, abandoned on timeout |
| `wasm32` | No threads or processes. Inline engine, no time limit. Recommendation below | none |

`Limits.max_worker_memory_bytes` (default **1 GiB**) is both the `ulimit -v` value and the
watchdog cap. UNCERTAIN: the macOS, Windows and Android code paths were not run; `ulimit -v` and
`/proc` were tested on one Linux machine (`sh` is dash there).

How the parent finds the worker (`Isolation::Auto` uses `WorkerSpec::discover`):

1. `Isolation::Process(WorkerSpec)` given by the application (explicit path; always wins);
2. the environment variable `COMETCAST_DOCS_WORKER` (path of a program that calls `worker_main`);
3. a file `cc-docs-worker` next to the running executable (or one level up from a `deps`
   directory, the Cargo test layout).

If none is found, `open` fails with `DocError::WorkerUnavailable`; it does **not** silently fall
back to the unprotected in-process engine. The CLI uses itself (`cometcast-cli docs-worker`). For
the **Flutter app** (UNCERTAIN, not built): desktop builds should ship `cc-docs-worker` next to the
app executable (found by step 3) or pass its path; Android cannot execute a binary from the app's
data directory, so the worker has to be packaged as a native library file (for example
`libcc_docs_worker.so` in `nativeLibraryDir`) and started by that path; iOS and tvOS cannot spawn
it at all.

### Limits

| Limit | Default | Where |
| --- | --- | --- |
| File size | 100 MiB | before parsing |
| `%PDF-` header | within the first 1024 bytes | before parsing |
| Pages | 5000 | raw-byte pre-check of `/Count N` and `/Type /Page` before any parser runs, and again after the page tree is parsed in the worker |
| Nesting of `<<` and `[` | 256 | raw-byte lexer before parsing (skips strings, comments and stream bodies) |
| Output side | 8192 px (hard cap 65535, hayro's `u16`) | before rendering |
| Output area | 7680 x 4320 px | before rendering |
| Open time, render time | 10 s each | wall clock in the parent (kill) and `ulimit -t` |
| Worker memory | 1 GiB | `ulimit -v` and watchdog |
| Worker thread stack | 16 MiB (the in-process engine keeps 64 MiB) | worker process |
| Image area (`Width * Height`) | 64 Mpx (8000 x 8000) | pre-scan in the worker |
| One `FlateDecode` stream inflated | 256 MiB | pre-scan (the review suggested 128 MiB; an 8-bit CMYK image of 64 Mpx is exactly 256 MiB, so 128 MiB would reject images the area cap allows) |
| All streams inflated together | 1 GiB | pre-scan |
| Objects | 2 000 000 | pre-scan |
| Filters on one stream | 8 | pre-scan |

**Uncertain:** all defaults are guesses for a classroom setting. The memory default comes from the
measurements below on one Linux machine; nothing is measured on real devices. They are public
fields of `Limits`.

**Trade-off of the image cap:** a 10 000 x 10 000 pixel scan (100 Mpx) is **rejected**
(`DocError::Rejected`). Applications that need such files raise `max_image_pixels`,
`max_stream_inflate_bytes` and the memory limit together. The pre-scan (`prescan.rs`) walks all
objects once at open and inflates every `FlateDecode` stream with a byte cap (`Limits.inflate_scan
= false` turns the inflating off). It only gives early, clear errors for the common bomb shapes. It
**never replaces the process limits**: inline images, streams that only hayro decodes and objects
inside compressed object streams are not seen, and a damaged zlib stream counts only what it
produced.

### Memory actually measured (Linux, debug build, this machine)

| What | Result |
| --- | --- |
| Lowest working `ulimit -v`, 16 MiB stack | `classroom.pdf` at 1920 px: 96 MiB works, 64 MiB fails. `minimal.pdf` at 3840 px: 128 MiB works, 96 MiB fails. 7680 x 4320-class output (33 Mpx): 384 MiB works, 256 MiB fails (resident peak 248 MiB). `classroom.pdf` at 3840 px (20.8 Mpx): 384 MiB works, 256 MiB fails (resident peak 182 MiB). The review had found 128 and 256 MiB failing with the old 64 MiB stack |
| Largest image the caps allow: 8000 x 8000 RGB (192 MB decoded), square page | At 3840 x 3840: resident peak 272 MiB, 14 s (debug). At 5760 x 5760 (the output cap): peak 396 MiB, 26 s (debug, measured once by hand). So 1 GiB is about 2.5 times the worst case measured |
| Flate bomb, 2 GiB of zeros in a 2.1 MB content stream, process limit only (pre-scan off, 512 MiB) | `WorkerFailed` after 4.8 s; resident peak seen by the parent 303 MiB; the test process stayed far below 600 MiB. With the pre-scan on: `Rejected` after 0.7 s |
| Image declared 2e9 x 2e9 (tiny body), process limit only | `WorkerFailed` after 68 ms (peak seen 45 MiB). Pre-scan on: `Rejected` after 5 ms |
| Image 20000 x 20000 RGB (1.2 GB decoded, 1.2 MB file), process limit only | `WorkerFailed` after 4.8 s (peak seen 300 MiB). Pre-scan on: `Rejected` after 55 ms |
| 1 000 000 tiny pages (105 MB file) | Used to take 12 s and 900 MB. Now refused on the raw bytes in 30 ms (`TooManyPages`), also when `/Count` lies |
| 100 000-level nested dictionary | Walking it in the pre-scan took 12 s (debug); now refused by the raw nesting check before parsing |
| Cost of the process | Open 4.5 ms (was 0.2 to 1 ms). Render of a 1920 px page: debug minimal.pdf 67 ms (was 51), classroom p1 272 ms (was 234); release minimal 17 ms (was 7.7), classroom p1 46 ms (was 25) |

"Peak seen" values are read by the parent every 10 ms, so they are lower bounds when the worker
dies in between; the hard limit is the kernel's.

### What happens on abuse

- **No Busy lock-out.** The old design abandoned a thread per timeout and refused new documents
  (`Busy`) after four; a 725 byte file (`pattern_self.pdf`, a tiling pattern that uses itself)
  made four renders time out and `open` return `Busy` for every document. With worker processes
  nothing is abandoned: six renders of that file end with `Timeout` after 0.8 s each and an
  unrelated document opens afterwards.
- **In-process engine** (iOS, tvOS, wasm, or `Isolation::InProcess`): the first timeout
  **poisons** the document and the content hash of the file (`DocError::Poisoned` on every later
  render or reopen of the same bytes); cancelled renders poison it after three. One bad file
  costs one thread. A global cap of 8 abandoned threads remains (`Busy`) and the counter is exact
  now (a guard on the thread and a compare-and-swap on abandon; before, it could leak when the
  thread ended while it was being abandoned). A different hostile file still costs one more
  thread each: that is the weakness of this engine.
- Timeouts of any size are accepted: `Duration::MAX` used to panic in `Instant::now() +
  timeout`; every timeout is now cut to one day and added with `checked_add`.
- Panics inside the worker become `DocError::Panicked`; a stack overflow or abort only ends the
  child (`WorkerFailed`).
- `/UserUnit` (PDF 1.6, a page unit other than 1/72 inch) is **not applied**: hayro ignores it, so
  `page_size` and the render use the point size from the boxes. A page with `/UserUnit 100` and a
  100 pt box is shown as 100 pt (tested).
- Encryption: a file that needs a password gives `DocError::PasswordProtected`. Files that are
  encrypted with an **empty user password** open normally (tested with an RC4 128 bit file made
  by reportlab). hayro 0.9's source also contains AES-128 and AES-256 code; those were not tested
  here. hayro's own README says encrypted files are unsupported, but the code shows a decryption
  module; the tests below are what was observed. The pre-scan counts streams it cannot decrypt
  as 0 bytes.

### Platform statements

- **Linux / Android**: sandboxed as above; the strongest case and the only one tested (Linux).
- **macOS**: watchdog only. A hostile file can use memory for up to the polling gap (100 ms)
  before the kill. UNCERTAIN, not tested here.
- **Windows**: watchdog via `tasklist` (250 ms) and wall-clock kill, no job object. Weaker than
  Linux. UNCERTAIN, not run.
- **iOS / tvOS**: in-process engine with poisoning and the pre-scan **only**. A memory bomb that
  passes the pre-scan (inline image, object stream) can end the app. This is the weakest
  platform. Options: render PDFs on the sender or a server and send pictures to these devices,
  or lower `Limits` (file size, image area).
- **Web (wasm)**: render the hostile PDF in a **dedicated Web Worker** whose `WebAssembly.Memory`
  has a `maximum` (an allocation over it fails inside the worker) and that the page ends with
  `worker.terminate()` on a timeout. The crate itself has no time limit on wasm. Not built or
  tested here.
- **Server** (`cc-server`) only validates WebP headers; it does not render PDFs.

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
- `tests/limits.rs` (own process, because the in-process tests leave abandoned renders behind):
  256 MiB flate bomb and images of 100 000 x 100 000, 65 536 x 65 536 and 2e9 x 2e9 through the
  default (process) path; an exponential form blow-up with a 150 ms limit in both engines (process:
  `Timeout`, nothing abandoned; in process: poisoned, reopen refused, the abandoned counter returns
  to 0); `Duration::MAX` timeouts; cancel flag in both engines.
- `tests/sandbox.rs` (**the regression tests for the review**, all THROUGH THE CHILD PATH, skipped
  with a note where `sh`/`ulimit -v` are missing): 2 GiB flate bomb, 2e9 x 2e9 image and 20000 x
  20000 RGB image against a 512 MiB limit with the pre-scan off (each returns an error, the worker
  peak stays inside the limit, this process stays small, a good render afterwards works); the same
  files with the pre-scan on (`Rejected`, fast); the largest allowed image (64 Mpx) renders; a
  64 MiB limit fails a 3840 px page; a million pages and a 100 000-level nesting are refused before
  parsing; a worker that sleeps is killed at the deadline; workers that print garbage, lie about
  the page count, crash or do not exist give errors. The bombs are generated at test time (a 2 GiB
  zero stream becomes a 2 MB file in milliseconds); `tests/data/review_gen.py` and
  `review_gen2.py` are the reviewer's generators, kept for provenance and not run.
- `tests/poison.rs` (own process): the 725 byte `pattern_self.pdf` six times through the child
  (never `Busy`, nothing abandoned) and in process (poisoned after the first timeout).
- `tests/render.rs` also checks thumbnails of extreme page shapes and `/UserUnit`.
- Unit tests: wire format round trip, page-claim and nesting pre-checks, timeout arithmetic.

Findings from these tests (the two bomb rows describe the original problem that the worker
process now contains; measured numbers with the sandbox are in "Memory actually measured"):

| What | Result |
| --- | --- |
| Deep nesting (content stream, objects, page tree) | No panic, no stack overflow in the 16 MiB worker stack (earlier also with 64 MiB). hayro has its own depth limit (`MAX_OBJECT_NESTING_DEPTH` is 512 in `hayro-syntax`) |
| Page tree loops | Returned without hang |
| 1e9 x 1e9 point media box | Rendered at 1920 px wide (the scale is derived from the box), no big buffer |
| Flate bomb, 256 MiB in a content stream | **hayro decodes the whole stream into memory, no ratio limit** (before the sandbox: about 5 s, process peak 336 to 360 MiB; a 2 GiB bomb grew the process to 2.18 GiB) |
| Image 2e9 x 2e9 | Before the sandbox: process peak about 2.7 GiB; the abandoned worker ended with `OutOfMemory(8000000000)` inside hayro (`image.rs:649`). hayro allocates `W * H` up front |
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
  catch there). Only the pre-checks (size, pages, nesting, output size) and the pre-scan apply.
- `cargo check -p cc-docs --target x86_64-pc-windows-gnu --all-targets` passes (compile only).
- Not built or checked: Android, iOS, tvOS targets.
- New dependency `flate2` (pure Rust `miniz_oxide` backend, no C code) for the inflate pre-scan.
  The crate also builds a second program, `cc-docs-worker` (Cargo feature `worker-bin`, on by
  default; an application with its own worker entry can switch it off).

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
- Worker for macOS, Windows, Android and Flutter packaging: written, but only compile-checked or
  not built (see "Platform statements"); a long-lived worker per document (today one process per
  call, which costs a few milliseconds per page); a job object on Windows.
- Sending the PDF as a file over the transport; receiver UI; page cache; presenter view.
- Office to PDF (`office2pdf`), notes from PPTX.
- Annotations (`lopdf` in the plan).

The generated test files and their provenance are in
[`crates/cc-docs/tests/data/PROVENANCE.md`](../crates/cc-docs/tests/data/PROVENANCE.md).
