<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Documents: PDF rendering (`cc-docs`)

Status: PDF part implemented and tested on generated files only. Rendering runs in a
**resource-limited child process** that, on Linux, also restricts itself with Landlock and
seccomp where the kernel allows it (since the security reviews, see "Limits and safety"). It is
**not a sandbox in the full sense**: read "What this protects and what it does not" before
relying on it.
Office conversion and speaker notes are **not** done. The WASM build exists (`crates/cc-docs-wasm`) and is used by the web receiver, see [In the web client](#in-the-web-client). The Flutter UI (send a PDF, show pages, presenter controls) exists for the Linux desktop, see [In the Flutter app](#in-the-flutter-app).

## Idea (from the plan)

A PDF is sent **once as a file**, not streamed as video. Every device that shows it renders
the pages itself with the same renderer, [hayro](https://github.com/LaurenzV/hayro) (pure Rust).
The presenter's view (current page, next page, notes, buttons) uses the same renderer; only
the page number travels, as `ControlCmd` from `cc-proto` (`NextPage`, `PrevPage`,
`GoToPage { page }`, `Stop`; first page is 1). How a file travels over the control DataChannel
(image, link and PDF) is in [web-m2.md](web-m2.md).

## What the crate offers

| Item | Purpose |
| --- | --- |
| `Document::open(bytes)` / `open_with_limits` / `open_with_options(bytes, limits, Isolation)` | Checks limits **before** parsing, then parses in a child process (`Isolation::Auto`, the default; `Process(WorkerSpec)` to name the worker; `InProcess` for the weak thread engine) |
| `is_isolated()`, `protections()`, `last_worker_peak_kib()` | Which engine is used; **which protections were really in place** around the last worker (`Protection`: process-group kill, clean environment, memory limit, CPU limit, no core dumps, no file writes, fd limit, Landlock, Landlock network, seccomp, memory watchdog); highest memory the parent saw (diagnostics) |
| `Limits::require_os_sandbox` | `false` by default: a worker on a system without Landlock/seccomp still runs with its resource limits. `true`: the worker refuses to parse unless **both** are active (`DocError::SandboxUnavailable`) |
| `set_max_concurrent_workers(n)`, `active_workers()` | Global cap on running worker processes (default 4). A call waits for a free place (at most its own timeout, never more than 5 s), then fails with `DocError::TooManyWorkers` |
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
until memory ran out). The output file is created new (`O_EXCL`); an existing file is only
replaced with `--force`, and then **atomically**: the picture is written to a new temporary file
in the same directory and renamed over the target, so a symbolic link that appears there after
the check is replaced as a link and never written through (an existing link is refused up
front). The program renders by starting itself as `cometcast-cli docs-worker` (hidden command,
not for users), or the program named in `COMETCAST_DOCS_WORKER` (only if it passes the trust
check below). `main` decides `docs-worker` from argv **before** any tokio runtime or other
thread exists (`main` is a plain `fn`; the runtime is built only for the other commands): the
worker's seccomp allowlist does not cover a runtime's eventfd/epoll teardown, and `cc-docs`
itself depends on no async runtime. `docs info` prints `protections: ...` (what the worker
applied to itself, `none` for the in-process engine). The test `bins/cometcast-cli/tests/docs_worker.rs`
runs `docs info` and `docs render` through this path without `COMETCAST_DOCS_WORKER`.

## Limits and safety

A PDF comes from a remote sender and is **hostile input**. hayro cannot be interrupted, and a
failed allocation aborts the whole process (`abort`, not a panic), so no in-process limit can
protect the receiver. The boundary is therefore a **child process with resource limits**.

### What this protects and what it does not

Be exact about the words: the worker is a **resource-limited child process**. By itself it is
**not a sandbox**.

- What the child process alone gives: a memory bomb, an endless loop or a crash in the renderer
  ends the child, not the application (address-space limit, CPU limit, wall-clock kill of the
  whole process group, no core dump).
- What it does **not** give: the child runs as the **same user** as the application. Without
  Landlock and seccomp (see the table below) it can read every file the user can read, write
  where the user can write (the size limit `RLIMIT_FSIZE = 0` stops writing data, not creating
  or deleting files), open network connections and start programs. On Windows, macOS, Android,
  and on Linux kernels or containers without Landlock/seccomp, that is the whole picture.
- **A code-execution exploit in hayro (or in a library it uses) is equal to a compromise of the
  user's session on every system where `protections()` does not list `Landlock` and `Seccomp`.**
  Where both are active, an exploit starts inside a process that can only compute, read the
  request from stdin and write the reply to stdout; it still shares the kernel (kernel bugs,
  and everything seccomp allows, such as `mmap`, `futex`, `clone` of threads, are reachable), so
  this lowers the risk and does not remove it. hayro calls itself experimental and has seen no
  security audit that we know of.
- **Policy** (decided, not accidental): the resource limits are **mandatory and fail closed**;
  if the worker cannot set one it reports `WorkerFailed` and never parses. Landlock and seccomp
  are **best effort** by default: if they are unavailable the worker still runs with its limits
  and `Document::protections()` shows what is missing, so an application can warn or refuse. An
  application that must not render without them sets `Limits::require_os_sandbox = true`
  (error `SandboxUnavailable`, nothing parsed). The CLI keeps the default (best effort).
- Applications with a stronger need should additionally run the whole application, or a
  dedicated render service, in the platform's own sandbox (sandboxed app container, container
  or VM without network, separate user), or render PDFs on the sender or a server.

### The worker process

`Document::open` and `render_page` start `cc-docs-worker` (or any program that calls
`cc_docs::worker_main()`) once per call: the PDF goes to its stdin, one framed reply comes back
on stdout (`crates/cc-docs/src/wire.rs`: a fixed head, then page sizes or RGBA). The parent
checks every size in the reply against what it asked for before it allocates.

**Parent side** (`process.rs`):

- The worker is started **directly**, with no shell, from an **absolute path** that was fixed
  when the `WorkerSpec` was made (a bare name such as `sleep` is looked up once in `PATH`;
  the directory is canonicalised, the file name is kept so that multi-call binaries still
  work). There is no `PATH` search at spawn time, so what `discover` found is what runs.
- **Empty environment** (`env_clear`; on Windows only `SystemRoot`), working directory `/`,
  `stderr` to `/dev/null`. File descriptors: only stdin and stdout (pipes) and stderr exist in
  the child. The Rust standard library opens every file and socket close-on-exec, so nothing of
  the application's own is inherited. A descriptor that other code in the same program opened
  **without** close-on-exec would leak into the child; the worker lowers `RLIMIT_NOFILE` to 16
  but cannot close what it did not open. Applications must open descriptors close-on-exec
  (tested with a worker that lists its descriptors, environment and directory).
- The worker runs in its **own process group** (Unix). On timeout, cancel, memory excess, any
  error, and also when the leader exits normally, the parent sends `SIGKILL` to the **whole
  group** and reaps the child. A worker that forks, or a grandchild that holds the pipes, cannot
  keep the call open or survive it (tests with `sh -c 'sleep 30 & exit 0'`, `sleep 600` and the
  reviewer's `fork.sh`, `forkalive.sh`, `stall.sh`). Windows has no process group here: the
  descendants of a worker are not killed (UNCERTAIN, not run).
- The two pipe threads are **detached** and report over a channel; the parent never joins a
  thread that might be blocked on a pipe, so a call returns within its deadline plus a few
  milliseconds whatever the worker does. (The old text "nothing is abandoned" was wrong for a
  worker with children.)
- If the parent itself dies, its worker is not killed by the parent (no `PR_SET_PDEATHSIG`: that
  signal follows the *thread* that started the child and would kill workers whose starting
  thread ended, for example a pool thread). The worker then finds its stdin closed and ends, and
  `RLIMIT_CPU` bounds a stuck one.
- A **global cap** on running workers (default 4, `set_max_concurrent_workers`) bounds what a
  flood of documents can start.

**Worker side** (`hardening.rs`), applied right after the fixed-size request head and **before
the PDF is read**:

| Protection | Linux | Android | macOS, other Unix | Windows |
| --- | --- | --- | --- | --- |
| `RLIMIT_AS` = `max_worker_memory_bytes` (fail closed) | yes | yes | not enforced by the kernel: parent watchdog (`ps -o rss=` every 100 ms; a fast allocation can overshoot between polls) | no: watchdog (`tasklist` every 250 ms) |
| `RLIMIT_CPU` = timeout + 2 s, `RLIMIT_CORE` 0, `RLIMIT_FSIZE` 0, `RLIMIT_NOFILE` 16 (all fail closed) | yes | yes | yes | no |
| Landlock: no file system access at all, no TCP bind/connect (ABI 4+), no abstract Unix sockets or signals out (ABI 6+) | Linux 5.13+ when enabled; **tested** | not tried | no | no |
| seccomp-bpf allowlist | x86_64 and aarch64; **tested on x86_64** | not tried (apps already run under the zygote filter) | no | no |
| Parent kills the process group | yes | yes (UNCERTAIN) | yes | no |
| iOS, tvOS | no child processes: in-process thread engine only, **the weakest platform** (see below) | | | |
| `wasm32` | no threads or processes; inline engine, no time limit; recommendation below | | | |

The seccomp allowlist (written from `strace` of renders of the test PDFs): `brk`, `mmap`,
`munmap`, `mremap`, `mprotect`, `madvise`; `futex`, `set_robust_list`, `rseq`, `sched_yield`,
`sched_getaffinity`, `gettid`, `getpid`, `set_tid_address`, `exit`, `exit_group`, `arch_prctl`
(x86_64); `clone` **only with the thread flags** (`CLONE_VM|FS|FILES|SIGHAND|THREAD|SYSVSEM`) and
no namespace flag; signal setup (`rt_sigaction`, `rt_sigprocmask`, `rt_sigreturn`,
`sigaltstack`); `clock_gettime`, `clock_nanosleep`, `nanosleep`, `gettimeofday`, `getrandom`;
`read`, `pread64`, `lseek`, `close`; `write` and `writev` only to descriptors 1 and 2; `prctl`
only `PR_SET_NAME`. Everything else (`open*`, `socket`, `connect`, `fork`, `vfork`, `execve`,
`kill`, `ptrace` ...) fails with `ENOSYS`. `ENOSYS` rather than `EPERM`, because glibc falls back
from `clone3` (whose flags a filter cannot inspect) to `clone` only on `ENOSYS`. Running the
renders under `strace -f` showed two denied calls and no crash: `clone3` (fallback works) and one
`open` of `/proc/sys/vm/overcommit_memory` by the allocator (falls back). The peak memory
(`VmHWM`) is read from a `/proc/self/status` handle opened before Landlock is applied.

No `unsafe` code was needed in `cc-docs` (it keeps `unsafe_code = forbid`): `rlimit`, `landlock`,
`seccompiler` and `rustix` (safe `killpg`, `getuid`) contain the system calls. The tests check
the sandbox from the inside: a child copy of the test program applies exactly the worker's
function and then tries to open a file, list `/`, create a file, connect and listen on TCP,
bind UDP, fork-exec a program (all fail), and to start a thread and allocate 32 MiB (both
work). They skip with a note where the kernel lacks Landlock or seccomp.

`Limits.max_worker_memory_bytes` (default **1 GiB**) is both the `RLIMIT_AS` value and the
watchdog cap. UNCERTAIN: the macOS, Windows and Android code paths were not run; the Linux
paths were run on one machine (kernel 6.18, Landlock ABI 7, x86_64, a Firecracker VM).

How the parent finds the worker (`Isolation::Auto` uses `WorkerSpec::discover`):

1. `Isolation::Process(WorkerSpec)` given by the application (explicit path; always wins);
2. the environment variable `COMETCAST_DOCS_WORKER` (path of a program that calls `worker_main`),
   **honoured only if the path is absolute, a regular file, owned by the current user or by
   root, not writable by group or others, and in a directory that others cannot write to**
   (unless sticky). Decision: honoured in **every** build, release too; if the variable is set
   and fails the check, `open` fails with the reason (`WorkerUnavailable`) and nothing else is
   tried. Reason: whoever controls the environment of the application can already do worse, but
   a worker that another user could have swapped must not run. UNCERTAIN: no owner check on
   Windows (only "absolute and a regular file"); a directory further up the path is not checked;
3. a file `cc-docs-worker` next to the running executable (or one level up from a `deps`
   directory, the Cargo test layout).

If none is found, `open` fails with `DocError::WorkerUnavailable`; it does **not** silently fall
back to the unprotected in-process engine. The CLI uses itself (`cometcast-cli docs-worker`). For
the **Flutter app**: the Linux build ships `cc-docs-worker` next to the app executable
(`apps/flutter/linux/CMakeLists.txt`, see [flutter-bridge.md](flutter-bridge.md)) and `cc-ffi` passes that
path explicitly as `Isolation::Process`, so the app does not depend on the steps above; other platforms
(UNCERTAIN, not built): desktop builds should ship `cc-docs-worker` next to the app executable
or pass its path; Android cannot execute a binary from the app's
data directory, so the worker has to be packaged as a native library file (for example
`libcc_docs_worker.so` in `nativeLibraryDir`) and started by that path; iOS and tvOS cannot spawn
it at all.

### Limits

| Limit | Default | Where |
| --- | --- | --- |
| File size | 100 MiB | before parsing |
| `%PDF-` header | within the first 1024 bytes | before parsing |
| Pages | 5000 | raw-byte pre-check before any parser runs: counts **distinct page objects** (`N G obj` followed by `/Type /Page`; the same object number in several revisions of an incremental update counts once; `/Count` is not used because outlines carry one too and it can lie), and again after the page tree is parsed in the worker. Pages in compressed object streams are only caught by the second check |
| Nesting of `<<` and `[` | 256 | raw-byte lexer before parsing (skips strings, comments and stream bodies; a `stream` word only starts a body when it follows a dictionary that closed at depth 0, so a fake `stream` token cannot hide nesting; the lexer ends a body at `endstream`, hayro trusts `/Length`, so a file that makes the two disagree can still place nesting where this check does not look: hayro's own limit of 512 and the worker limits are the backstop) |
| Output side | 8192 px (hard cap 65535, hayro's `u16`) | before rendering |
| Output area | 7680 x 4320 px | before rendering |
| Open time, render time | 10 s each | wall clock in the parent (kill) and `RLIMIT_CPU` (timeout + 2 s, fail closed) |
| Worker memory | 1 GiB | `RLIMIT_AS` set by the worker (fail closed) and watchdog |
| Running workers | 4 | global cap, `TooManyWorkers` |
| Worker thread stack | 16 MiB (the in-process engine keeps 64 MiB) | worker process |
| Image area (`Width * Height`) | 64 Mpx (8000 x 8000) | pre-scan in the worker. A `Width` or `Height` that is not a readable non-negative number (above `u32::MAX`, negative, huge real) counts as **too large**, not as 0 (it used to be 0 and bypassed the check) |
| Image bytes (area x components x bits) | 4 x `max_image_pixels` = 256 MiB | pre-scan: gray 1, RGB 3 and any other colour space 4 components, `BitsPerComponent` (default 8, at most 16), image masks 1 bit. A 16 bit CMYK 8000 x 8000 image (512 MiB) is refused; 8 bit CMYK is exactly at the limit |
| Inline images (`BI /W n /H n ... ID`) | same area limit | rough scan of the first 8 MiB of each non-image stream (64 MiB in total) when the pre-scan inflates; an inline image later in a long stream is not seen, the memory limit catches it |
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
| Lowest working `RLIMIT_AS` (set by the worker itself, with Landlock and seccomp active; bisection in steps of 4 MiB, debug build, second review) | `minimal.pdf` at 1 px (open only plus a tiny render): **47 MiB** (fails at 43). `classroom.pdf` at 1920 px: **91 MiB** (fails at 87). `minimal.pdf` at 3840 px: **107 MiB** (fails at 103). `classroom.pdf` at 3840 px (20.8 Mpx): **270 MiB** (fails at 266). `minimal.pdf` at 7680 px (7680 x 3840, 29 Mpx): **338 MiB** (fails at 334). Earlier coarse numbers with `sh -c ulimit -v` (96, 128, 384, 384 MiB) were upper bounds from a coarser search and are replaced by these. The 1 GiB default stays: about 3 times the largest of these, 2.5 times the worst case measured for the largest image the caps allow |
| Largest image the caps allow: 8000 x 8000 RGB (192 MB decoded), square page | At 3840 x 3840: resident peak 272 MiB, 14 s (debug). At 5760 x 5760 (the output cap): peak 396 MiB, 26 s (debug, measured once by hand). So 1 GiB is about 2.5 times the worst case measured |
| Flate bomb, 2 GiB of zeros in a 2.1 MB content stream, process limit only (pre-scan off, 512 MiB) | `WorkerFailed` after 4.8 s; resident peak seen by the parent 303 MiB; the test process stayed far below 600 MiB. With the pre-scan on: `Rejected` after 0.7 s |
| Image declared 2e9 x 2e9 (tiny body), process limit only | `WorkerFailed` after 68 ms (peak seen 45 MiB). Pre-scan on: `Rejected` after 5 ms |
| Image 20000 x 20000 RGB (1.2 GB decoded, 1.2 MB file), process limit only | `WorkerFailed` after 4.8 s (peak seen 300 MiB). Pre-scan on: `Rejected` after 55 ms |
| 1 000 000 tiny pages (105 MB file) | Used to take 12 s and 900 MB. Now refused on the raw bytes in 30 ms (`TooManyPages`), also when `/Count` lies |
| 100 000-level nested dictionary | Walking it in the pre-scan took 12 s (debug); now refused by the raw nesting check before parsing |
| Cost of the process | Open 4.5 ms (was 0.2 to 1 ms) with `sh`; **5.6 ms** (median of 15, debug) now that the worker restricts itself with Landlock and seccomp and no shell is started. Render of a 1920 px page: debug minimal.pdf 66 ms now (67 before, 51 in process), classroom p1 272 ms (was 234); release minimal 17 ms (was 7.7), classroom p1 46 ms (was 25; release numbers not re-measured after the sandbox) |

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

- **Linux**: resource limits, process-group kill, and Landlock plus seccomp where the kernel offers
  them (tested on x86_64, kernel 6.18); the only platform run. **Android**: the same code paths
  without Landlock/seccomp attempted in practice (UNCERTAIN, not run).
- **macOS**: watchdog only. A hostile file can use memory for up to the polling gap (100 ms)
  before the kill. UNCERTAIN, not tested here.
- **Windows**: watchdog via `tasklist` (250 ms) and wall-clock kill, no job object. Weaker than
  Linux. UNCERTAIN, not run.
- **iOS / tvOS**: in-process engine with poisoning and the pre-scan **only**. A memory bomb that
  passes the pre-scan (inline image, object stream) can end the app. This is the weakest
  platform. Options: render PDFs on the sender or a server and send pictures to these devices,
  or lower `Limits` (file size, image area).
- **Web (wasm)**: render the hostile PDF in a **dedicated Web Worker** whose `WebAssembly.Memory`
  has a `maximum` (an allocation over it traps inside the worker; the poisoned instance is never reused) and that the page ends with
  `worker.terminate()` on a timeout. The crate itself has no time limit on wasm. Built and used
  this way by the web client, see [In the web client](#in-the-web-client).
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
- `tests/sandbox.rs` (**the regression tests for the first review**, all THROUGH THE CHILD PATH,
  skipped with a note where the address-space limit is not enforced): 2 GiB flate bomb, 2e9 x 2e9 image and 20000 x
  20000 RGB image against a 512 MiB limit with the pre-scan off (each returns an error, the worker
  peak stays inside the limit, this process stays small, a good render afterwards works); the same
  files with the pre-scan on (`Rejected`, fast); the largest allowed image (64 Mpx) renders; a
  64 MiB limit fails a 3840 px page; a million pages and a 100 000-level nesting are refused before
  parsing; a worker that sleeps is killed at the deadline; workers that print garbage, lie about
  the page count, crash or do not exist give errors. The bombs are generated at test time (a 2 GiB
  zero stream becomes a 2 MB file in milliseconds); `tests/data/review_gen.py` and
  `review_gen2.py` are the reviewer's generators, kept for provenance and not run.
- `tests/process_kill.rs` (second review, finding 1, 3, 4, 8): fake `sh -c` workers. A worker
  that exits while a background `sleep` holds the pipes returns in under 1.5 s with `WorkerFailed`
  and the `sleep` is dead; a worker and its child that both stay alive are killed together at a
  400 ms deadline (call returns in under 1.4 s); `sleep 600` likewise; cancel; a worker script
  that lists `/proc/self/fd`, `env`, `pwd` shows only descriptors 0 to 2 (plus the shell's own
  directory handle), no `HOME`, `PATH` or `CARGO*`, and `/` as directory; `WorkerSpec::new` makes
  bare and relative names absolute; `COMETCAST_DOCS_WORKER` is refused when relative, writable by
  others or a directory, and accepted when it is a 0755 file of the current user.
- `tests/hardening.rs`: the in-worker checks described above (child copy of the test program),
  `Document::protections()`, `require_os_sandbox` against what the system offers, and that all
  test PDFs still render under the filters.
- `tests/worker_cap.rs` (own process): with the cap at 2, two sleeping workers hold the places, a
  third call gets `TooManyWorkers` within its 300 ms timeout and starts nothing, the places come
  back.
- `tests/prescan_review.rs`: incremental update with 12 `/Type /Page` tokens for 6 pages passes a
  limit of 6 and shows the updated page size; a 100 000 `/Count` on an outline is no page claim; a
  lying `/Count`; fake `stream` token in front of 1000 nested dictionaries is refused; Width over
  `u32::MAX`, 4294967296, negative and huge real are refused; 16 bit CMYK 8000 x 8000 refused and
  8 bit CMYK accepted; inline images with 100 000 x 100 000 in plain and in Flate content streams
  refused, a small one accepted.
- `tests/poison.rs` (own process): the 725 byte `pattern_self.pdf` six times through the child
  (never `Busy`, nothing abandoned) and in process (poisoned after the first timeout).
- `tests/render.rs` also checks thumbnails of extreme page shapes and `/UserUnit`.
- Unit tests: wire format round trip, page-claim and nesting pre-checks, timeout arithmetic.

Findings from these tests (the two bomb rows describe the original problem that the worker
process now contains; measured numbers with the process limit are in "Memory actually measured"):

| What | Result |
| --- | --- |
| Deep nesting (content stream, objects, page tree) | No panic, no stack overflow in the 16 MiB worker stack (earlier also with 64 MiB). hayro has its own depth limit (`MAX_OBJECT_NESTING_DEPTH` is 512 in `hayro-syntax`) |
| Page tree loops | Returned without hang |
| 1e9 x 1e9 point media box | Rendered at 1920 px wide (the scale is derived from the box), no big buffer |
| Flate bomb, 256 MiB in a content stream | **hayro decodes the whole stream into memory, no ratio limit** (before the process limit: about 5 s, process peak 336 to 360 MiB; a 2 GiB bomb grew the process to 2.18 GiB) |
| Image 2e9 x 2e9 | Before the process limit: process peak about 2.7 GiB; the abandoned worker ended with `OutOfMemory(8000000000)` inside hayro (`image.rs:649`). hayro allocates `W * H` up front |
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

## In the web client

The web receiver (`apps/web`) shows PDFs that arrive over the `control` DataChannel (message format in
[web-m2.md](web-m2.md)). Rendering uses the **same code** as the native clients, compiled to WebAssembly:

| Piece | What it is |
| --- | --- |
| `crates/cc-docs-wasm` | A small crate (`cdylib` + `rlib`) around `cc_docs::Document` with `wasm-bindgen`: `PdfDocument.open(bytes)`, `page_count()`, `render(index, width)` returning RGBA. Opens with the **in-process engine**, the only one on `wasm32` |
| `apps/web/scripts/build-wasm.sh` (`npm run build:wasm`) | `cargo build --release --target wasm32-unknown-unknown`, with the linear memory capped at 512 MiB (`--max-memory`), then `wasm-bindgen --target web` into `apps/web/public/pdf/` (git-ignored). Needs a `wasm-bindgen` CLI of exactly the `Cargo.lock` version |
| `src/core/pdfWorker.ts` | A module Web Worker that loads `/pdf/cc_docs_wasm.js` and the `.wasm` file at run time (same origin) and holds the one opened document |
| `src/core/pdfRenderer.ts` | The page side: opens, renders **one page per request, never in advance**, one request at a time, with a wall-clock limit of 20 s for open and for each render. On a timeout, a crash or a memory failure the worker is **terminated** and the document is closed with a translated notice |
| `src/components/DocumentView.vue` | Draws the page on a `<canvas>` with `putImageData`. Only pixels: no text layer, no links, nothing of the PDF is clickable |

Limits in the browser (`web_limits()` in the crate; all UNCERTAIN guesses, none measured on real devices):
file 64 MiB (the protocol limit), 1000 pages, output side 4096 px and area 4096 x 2304 px (a requested width is
clamped, not refused), image area 16 Mpx, one inflated stream 128 MiB, all streams 512 MiB, plus the usual
pre-checks and the inflate pre-scan of this crate. Memory is capped by `--max-memory` (512 MiB; `build-wasm.sh` parses the memory section of the built file and fails if the cap is missing). An allocation over the cap **traps** in the module; the worker maps anything that is not a known error code (a trap, `unreachable`, an out-of-bounds access) to the fatal code `memory` or `failed`, ends the worker and closes the document. The PDF is
parsed and drawn only: hayro runs no JavaScript and this API exposes no link, so nothing in the file is
executed or opened. The page width asked for is the stage width times the device pixel ratio, at most 3000 px.

What this does **not** give, compared with the native worker process: no operating system sandbox and no
CPU or time limit inside the module (the page ends the Web Worker instead, so a runaway render blocks only
that worker, not the page), and on wasm every render re-parses the file (the inline engine parses per
render; speed is not measured). Renders are coalesced: one runs, at most one waits, a newer request replaces the waiting one. A trap in the module, which `catch_unwind` cannot prevent on wasm, ends the
worker and the document; the file is not opened again automatically.

Measured locally on one machine only (one Chromium/Chrome build on Linux, not in CI, not repeated elsewhere): the `.wasm` file is 5.3 MB raw and about 1.8 MB with
`gzip -9` (the server does not compress, so unless a proxy does, the raw size is transferred; the file is
loaded only when a PDF arrives). A generated three-page PDF and the classroom PDF render into a non-blank
canvas at the expected places and colours (rendered in Chromium by `npm run e2e:pdf`); a damaged PDF gives the notice; a link annotation inside a test PDF
is not followed. Not checked: Firefox, Safari, mobile browsers, large or hostile files in the browser (the
hostile-file tests run on the native target only), the fidelity of real classroom PDFs (point 7), and
accessibility of the page content (the canvas is labelled with the page number only, there is no text
alternative: UNCERTAIN, a text layer would need text extraction, which this API does not offer).

## Build and platforms

- hayro 0.9.0 (newest found with `cargo info` on 2026-11-06; the plan names 0.8.0) is
  `Apache-2.0 OR MIT` and declares `rust-version = 1.92`. The workspace `rust-version` was
  therefore raised from 1.88 to 1.92. **Uncertain:** not measured with a real 1.92 toolchain
  (1.97 was used). The same applies to 0.8.0 (also 1.92).
- hayro default features are kept: embedded substitute fonts (Foxit, about 240 KB per its
  documentation) and embedded CMaps. Binary size was not measured.
- `cargo check -p cc-docs --target wasm32-unknown-unknown` **passes** (checked on this
  machine). Since the web client work the crate is also linked (`crates/cc-docs-wasm`) and runs in
  Chromium (see [In the web client](#in-the-web-client)); only that browser was tried and the
  measurement of point 7 (20 real PDFs, speed on weak devices) is still open. On wasm the crate has no threads: it parses and renders inline, re-parsing the file
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

## In the Flutter app

Status: Linux desktop, tested with a fake core (widget tests), against the real library, and
end to end under `xvfb` with the real server and `cometcast-cli`
(`apps/flutter/integration_test/content_e2e_test.dart`). Everything below is a decision of the app,
not of `cc-docs`; the bridge is described in [flutter-bridge.md](flutter-bridge.md).

**Sending ("Send file or link").** The person chooses an image (PNG, JPEG, WebP, GIF) or a PDF with the file
dialog (`file_selector`), or types a link, and enters the receiver's room code like for screen sharing.
Rust checks everything (type by extension and bytes, size, `%PDF-` header, link rules; Dart does not
repeat the rules) and counts the PDF's pages in the render worker, then the file goes over the control
channel with a progress bar. For a PDF the presenter controls appear: previous, next, go to page, stop
showing, and the keys arrow right/down/Page Down (next) and arrow left/up/Page Up (previous); they map to
`ControlCmd` (`NextPage`, `PrevPage`, `GoToPage`, `Stop`). The sender draws nothing: the page number it shows is the
one it asked for (the receiver does not report back), clamped to the page count when that is known. The sender must
stay on the screen while the receiver shows the content; leaving it disconnects.

**Receiving.** The receive view shows what the sender pushes, instead of the live screen (the latest content
wins; closing returns to the live screen or to waiting):

- an **image**, fitted into the window, with a close button. The header is read first and an image of more than
  64 million pixels is refused (a small file can hold a huge picture; UNCERTAIN: the limit is a guess);
- a **PDF page** rendered by Rust at the width of the view in physical pixels (view width times device pixel ratio,
  reported again after a resize), with the indicator "Page N of M". **Page changes come from the sender only**;
  the person at the receiver can only close or stop (a deliberate choice: a presenter is in charge, and the
  receiver may be a wall display without input). The keys of the presenter are not bound on the receiver;
- a **link card** "The sender wants to open `<host>`" with Open and Dismiss. The host is shown in its punycode form
  (`xn--...`) so that look-alike names are visible, with the whole address under it. **Nothing is opened
  automatically**: Open asks Rust to validate the address again, checks the scheme once more in Dart, and only then calls
  `url_launcher` (external application mode). Dismiss has the initial focus, so a stray Enter does not open anything.

Failures are calm, translated notes chosen by an error **code** (`DocumentRejected`, `TooLarge`, protocol
violations, `transfer_timeout`, worker unavailable, render failed); text that came from the sender is never shown,
except the cleaned file name and the validated address. A refused PDF closes the shown document, like the core does.
Closing a document by hand makes the UI ignore its later pages (UNCERTAIN: the core cannot be told to close it, so
it may keep rendering pages for the sender's commands until `stop`).

Where the worker comes from, which platforms have it, and the limits of this section are in
[flutter-bridge.md](flutter-bridge.md) ("Content" and "Build integration").

## Not done

- 20 real PDFs (point 7), fidelity comparison with another viewer, WASM speed on weak devices (the WASM file size is in [In the web client](#in-the-web-client)).
- Worker for macOS, Windows, Android and Flutter packaging: written, but only compile-checked or
  not built (see "Platform statements"); a long-lived worker per document (today one process per
  call, which costs a few milliseconds per page); a job object on Windows.
- A page cache (every page change renders again); a local preview or thumbnail on the presenter's
  side (the Flutter sender only knows the page count); a receiver that turns pages itself.
- Office to PDF (`office2pdf`), notes from PPTX.
- Annotations (`lopdf` in the plan).

The generated test files and their provenance are in
[`crates/cc-docs/tests/data/PROVENANCE.md`](../crates/cc-docs/tests/data/PROVENANCE.md).
