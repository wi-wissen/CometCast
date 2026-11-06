// SPDX-License-Identifier: GPL-3.0-or-later
//! Parent side of the render worker process.
//!
//! One short-lived child per call (open or render): the PDF goes to its stdin, one framed
//! reply comes back on stdout (`wire.rs`). The child is the memory and time boundary. This
//! is a **resource-limited child process, not a full sandbox**; see `docs/documents.md` for
//! what it does and does not stop.
//!
//! * The child is started **directly** (no shell) from an absolute path, with an **empty
//!   environment**, `/` as working directory, `stderr` closed, and in its **own process
//!   group** (Unix). Other file descriptors of the parent are not inherited: `std` opens
//!   every file and socket close-on-exec, so only stdin and stdout (pipes) and stderr
//!   (`/dev/null`) exist in the child. A descriptor that other code in the application opened
//!   *without* close-on-exec would leak into the child; the worker lowers `RLIMIT_NOFILE` but
//!   cannot close what it did not open (tested with a worker that lists its descriptors).
//! * The child limits itself (`hardening.rs`): address space, CPU, core, file size, open
//!   files, then Landlock and seccomp where the kernel has them. The parent only sees an
//!   abnormal end when an allocation fails.
//! * macOS and other Unix: no address-space limit (`RLIMIT_AS` is not enforced on macOS); the
//!   parent polls the child's resident memory with `ps` and kills it above the cap.
//! * Windows: no job object (that needs `unsafe`); the parent polls with `tasklist`. Weaker
//!   and untested, see `docs/documents.md`. No process group: descendants of a worker are
//!   not killed there.
//! * Everywhere: a wall-clock deadline, a cancel flag, then the **whole process group** is
//!   killed (Unix; `SIGKILL` to `-pgid`) and the child reaped. The two pipe threads are
//!   **detached** and report over a channel; the parent never joins a thread that could be
//!   blocked on a pipe held by a grandchild, so a call returns within its deadline plus a
//!   small margin whatever the worker (or its descendants) do. A detached thread ends when
//!   the pipe closes, which the group kill causes on Unix.
//! * A global cap on running workers (default 4, [`set_max_concurrent_workers`]): a call waits
//!   for a free place (at most its own timeout, and never more than 5 s) and then answers
//!   [`DocError::TooManyWorkers`] instead of starting one more process.
//!
//! The parent never allocates more than it asked for: the reply sizes are checked against
//! the request before the buffer is made.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

#[cfg(unix)]
use crate::clamp::clamp_timeout;
use crate::clamp::deadline;
use crate::document::{Limits, PageSize, RenderedPage};
use crate::error::DocError;
use crate::isolation::WorkerSpec;
use crate::process_stats::{POLL_MS, rss_kib_of};
use crate::protection::{Mask, Protection};
use crate::wire::{self, KIND_ERROR, KIND_INFO, KIND_PAGE, OP_INFO, OP_RENDER, REPLY_HEAD};

/// What the caller wants from the worker.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Call {
    Info,
    Render {
        index: usize,
        width: u32,
        height: u32,
    },
}

/// Result of a successful call.
pub(crate) enum Output {
    Info(Vec<PageSize>),
    Page(RenderedPage),
}

/// Result of one call: the answer, the highest memory seen (KiB, 0 if unknown) and the
/// protections that were in place.
pub(crate) struct Finished {
    pub result: Result<Output, DocError>,
    pub peak_kib: u64,
    pub protections: Mask,
}

impl Finished {
    fn failed(e: DocError) -> Self {
        Self {
            result: Err(e),
            peak_kib: 0,
            protections: Mask::default(),
        }
    }
}

// ---- global cap on running workers ------------------------------------------------------

/// Default for [`set_max_concurrent_workers`].
const DEFAULT_MAX_WORKERS: usize = 4;

/// Longest a call waits for a free place under the cap.
const SLOT_WAIT_MAX: Duration = Duration::from_secs(5);

static MAX_WORKERS: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_WORKERS);
/// Workers running now, with the condition variable that wakes waiting callers.
static ACTIVE_WORKERS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

/// Set how many render worker processes may run at the same time in this program (default
/// 4, at least 1). A call that finds all places taken waits for one (at most its own timeout,
/// never more than 5 s) and then answers [`DocError::TooManyWorkers`] without starting a
/// process. Each worker may use [`Limits::max_worker_memory_bytes`] and one CPU, so the cap
/// bounds what a flood of documents can take.
pub fn set_max_concurrent_workers(n: usize) {
    MAX_WORKERS.store(n.max(1), Ordering::SeqCst);
    ACTIVE_WORKERS.1.notify_all();
}

/// The current cap, see [`set_max_concurrent_workers`].
pub fn max_concurrent_workers() -> usize {
    MAX_WORKERS.load(Ordering::SeqCst)
}

/// Render worker processes running right now in this program.
pub fn active_workers() -> usize {
    *ACTIVE_WORKERS.0.lock().unwrap_or_else(|p| p.into_inner())
}

/// Holds one place under the cap until dropped.
struct Slot;

impl Slot {
    fn acquire(wait: Duration) -> Result<Slot, DocError> {
        let give_up = Instant::now() + wait.min(SLOT_WAIT_MAX);
        let mut n = ACTIVE_WORKERS.0.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if *n < MAX_WORKERS.load(Ordering::SeqCst) {
                *n += 1;
                return Ok(Slot);
            }
            let left = give_up.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(DocError::TooManyWorkers);
            }
            n = ACTIVE_WORKERS
                .1
                .wait_timeout(n, left)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut n = ACTIVE_WORKERS.0.lock().unwrap_or_else(|p| p.into_inner());
        *n = n.saturating_sub(1);
        ACTIVE_WORKERS.1.notify_one();
    }
}

// ---- killing ------------------------------------------------------------------------------

/// Kills the process group and reaps the child when dropped, so no path leaves a process
/// behind.
struct ChildGuard {
    child: Option<Child>,
    #[cfg_attr(not(unix), allow(dead_code))]
    pid: u32,
}

impl ChildGuard {
    /// `SIGKILL` to every process of the worker's group (also after the leader was reaped:
    /// a group id is not reused while the group has members).
    #[cfg(unix)]
    fn kill_group(&self) {
        use rustix::process::{Pid, Signal, kill_process_group};
        if let Some(pgid) = i32::try_from(self.pid).ok().and_then(Pid::from_raw) {
            let _ = kill_process_group(pgid, Signal::KILL);
        }
    }

    #[cfg(not(unix))]
    fn kill_group(&self) {}

    fn kill_and_wait(&mut self) -> Option<ExitStatus> {
        self.kill_group();
        let mut c = self.child.take()?;
        let _ = c.kill();
        c.wait().ok()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

// ---- starting -----------------------------------------------------------------------------

fn command(spec: &WorkerSpec) -> Command {
    let mut c = Command::new(&spec.program);
    c.args(&spec.args);
    // Nothing from the parent's environment (tokens, paths, locale) reaches the worker.
    c.env_clear();
    #[cfg(windows)]
    {
        // Windows programs may need the system directory to start.
        if let Some(root) = std::env::var_os("SystemRoot") {
            c.env("SystemRoot", root);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.current_dir("/");
        // Own process group: the parent can kill the worker and everything it started.
        c.process_group(0);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        c.current_dir(root);
    }
    c
}

/// Protections the parent side provides on this platform.
fn parent_protections() -> Mask {
    let mut m = Mask::default().with(Protection::CleanEnvironment);
    if cfg!(unix) {
        m = m.with(Protection::ProcessGroupKill);
    }
    m
}

/// Can the worker limit itself with resource limits on this platform? (Used by tests to skip
/// with a note.) `true` on Linux and Android, where the address-space limit is enforced.
pub fn worker_limits_available() -> bool {
    cfg!(any(target_os = "linux", target_os = "android"))
}

/// A reply read from the worker's stdout.
struct Reply {
    output: Result<Output, DocError>,
    peak_kib: u64,
    protections: Mask,
}

fn read_reply(mut out: ChildStdout, call: Call, limits: &Limits) -> Result<Reply, DocError> {
    let bad = DocError::WorkerFailed("the renderer sent no valid answer");
    let mut head = [0u8; REPLY_HEAD];
    out.read_exact(&mut head).map_err(|_| bad.clone())?;
    let h = wire::decode_head(&head).ok_or_else(|| bad.clone())?;
    let output = match (h.kind, call) {
        (KIND_ERROR, _) => Err(wire::error_from_wire(h.code, h.x, h.y)),
        (KIND_INFO, Call::Info) => {
            let n = usize::try_from(h.x).map_err(|_| bad.clone())?;
            if n == 0 || n > limits.max_pages {
                return Err(bad);
            }
            let mut buf = vec![0u8; n * 8];
            out.read_exact(&mut buf).map_err(|_| bad.clone())?;
            let sizes = buf
                .chunks_exact(8)
                .map(|c| PageSize {
                    width_pt: f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                    height_pt: f32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                })
                .collect();
            Ok(Output::Info(sizes))
        }
        (KIND_PAGE, Call::Render { width, height, .. }) => {
            // The picture must have exactly the size that was asked for.
            if h.x != u64::from(width) || h.y != u64::from(height) {
                return Err(bad);
            }
            let len = usize::try_from(u64::from(width) * u64::from(height) * 4)
                .map_err(|_| bad.clone())?;
            let mut rgba = vec![0u8; len];
            out.read_exact(&mut rgba).map_err(|_| bad.clone())?;
            Ok(Output::Page(RenderedPage {
                width,
                height,
                rgba,
            }))
        }
        _ => return Err(bad),
    };
    // Only bits this crate knows; the rest is ignored.
    let known = Protection::ALL.into_iter().fold(Mask::default(), |m, p| {
        if h.z & p.bit() != 0 { m.with(p) } else { m }
    });
    Ok(Reply {
        output,
        peak_kib: h.peak_kib,
        protections: known,
    })
}

/// Run one call in a child process.
pub(crate) fn run(
    spec: &WorkerSpec,
    bytes: &Arc<Vec<u8>>,
    limits: &Limits,
    call: Call,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> Finished {
    let _slot = match Slot::acquire(timeout) {
        Ok(s) => s,
        Err(e) => return Finished::failed(e),
    };
    if !spec.program.is_absolute() {
        return Finished::failed(DocError::WorkerUnavailable(
            "the worker path is not absolute".into(),
        ));
    }
    let (op, index, width, height) = match call {
        Call::Info => (OP_INFO, 0u64, 0u32, 0u32),
        Call::Render {
            index,
            width,
            height,
        } => (OP_RENDER, index as u64, width, height),
    };
    // The CPU limit is the wall-clock limit plus a margin: it only catches a worker that
    // ignores the parent's kill.
    #[cfg(unix)]
    let cpu_secs = clamp_timeout(timeout).as_secs().saturating_add(2);
    #[cfg(not(unix))]
    let cpu_secs = 0;
    let head = wire::encode_request(
        op,
        index,
        width,
        height,
        limits,
        cpu_secs,
        bytes.len() as u64,
    );

    let mut cmd = command(spec);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Finished::failed(DocError::WorkerUnavailable(format!(
                "cannot start {}: {e}",
                spec.program.display()
            )));
        }
    };
    let pid = child.id();
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let mut guard = ChildGuard {
            child: Some(child),
            pid,
        };
        guard.kill_and_wait();
        return Finished::failed(DocError::Internal("no pipes to the worker"));
    };
    let mut guard = ChildGuard {
        child: Some(child),
        pid,
    };
    let cap_kib = limits.max_worker_memory_bytes / 1024;
    let end = deadline(timeout);

    // Detached helpers: they own the pipes and talk back over a channel. Nobody joins them.
    let (tx, rx) = mpsc::channel::<Result<Reply, DocError>>();
    let reader_limits = *limits;
    let reader = std::thread::Builder::new()
        .name("cc-docs-reader".into())
        .spawn(move || {
            let _ = tx.send(read_reply(stdout, call, &reader_limits));
        });
    let payload = Arc::clone(bytes);
    let writer = std::thread::Builder::new()
        .name("cc-docs-writer".into())
        .spawn(move || write_request(stdin, &head, &payload));
    if reader.is_err() || writer.is_err() {
        guard.kill_and_wait();
        return Finished::failed(DocError::Internal("cannot start a helper thread"));
    }

    // Watch the child. Every exit from this loop has killed it or seen it end.
    let mut peak = 0u64;
    let mut watchdog = false;
    let mut last_poll = Instant::now();
    let mut verdict: Option<DocError> = None;
    let mut reply: Option<Result<Reply, DocError>> = None;
    let mut channel_open = true;
    let status = loop {
        if cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
            verdict = Some(DocError::Cancelled);
            break guard.kill_and_wait();
        }
        let waited = guard.child.as_mut().map(Child::try_wait);
        match waited {
            Some(Ok(Some(st))) => {
                guard.child = None;
                break Some(st);
            }
            Some(Ok(None)) => {}
            _ => {
                verdict = Some(DocError::Internal("cannot watch the worker"));
                break guard.kill_and_wait();
            }
        }
        let now = Instant::now();
        if now >= end {
            verdict = Some(DocError::Timeout);
            break guard.kill_and_wait();
        }
        if now.duration_since(last_poll) >= Duration::from_millis(POLL_MS) {
            last_poll = now;
            if let Some(kib) = rss_kib_of(pid) {
                watchdog = true;
                peak = peak.max(kib);
                if kib > cap_kib {
                    verdict = Some(DocError::MemoryLimit);
                    break guard.kill_and_wait();
                }
            }
        }
        if channel_open && reply.is_none() {
            match rx.recv_timeout(Duration::from_millis(2)) {
                Ok(r) => reply = Some(r),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => channel_open = false,
            }
        } else {
            std::thread::sleep(Duration::from_millis(2));
        }
    };

    // The leader ended (or was killed). Kill everything else in its group now, so that a
    // grandchild cannot keep the pipes open or keep running.
    guard.kill_group();

    let mut protections = parent_protections();
    if watchdog {
        protections = protections.with(Protection::MemoryWatchdog);
    }
    if let Some(e) = verdict {
        return Finished {
            result: Err(e),
            peak_kib: peak,
            protections,
        };
    }
    // The reply may still be on its way (the worker wrote it and exited); the group is dead
    // so the pipe is closed and the reader finishes. Wait for it, but never past the deadline
    // (a little grace if the deadline is just now).
    if reply.is_none() && channel_open {
        let left = end
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(100));
        reply = match rx.recv_timeout(left) {
            Ok(r) => Some(r),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Finished {
                    result: Err(DocError::Timeout),
                    peak_kib: peak,
                    protections,
                };
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => None,
        };
    }
    let reply = reply.unwrap_or(Err(DocError::Panicked));
    let (result, reply_peak) = match reply {
        Ok(r) => {
            protections = Mask(protections.0 | r.protections.0);
            let ok_exit = status.is_some_and(|s| s.success());
            // An error reply is trusted whatever the exit code; a picture needs exit 0.
            match r.output {
                Ok(_) if !ok_exit => (
                    Err(DocError::WorkerFailed("the renderer ended abnormally")),
                    r.peak_kib,
                ),
                other => (other, r.peak_kib),
            }
        }
        Err(e) => (Err(refine(e, status)), 0),
    };
    Finished {
        result,
        peak_kib: peak.max(reply_peak),
        protections,
    }
}

/// Turn "no valid answer" into a more useful message from the exit status.
fn refine(e: DocError, status: Option<ExitStatus>) -> DocError {
    let Some(st) = status else { return e };
    // The worker program could not be run at all.
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = st.signal() {
            // SIGXCPU = 24 on Linux: the CPU-time limit.
            return DocError::WorkerFailed(if sig == 24 {
                "the renderer used too much CPU time"
            } else {
                "the renderer was stopped by a signal (crash or memory limit)"
            });
        }
    }
    if st.success() {
        e
    } else {
        DocError::WorkerFailed("the renderer ended abnormally (crash or memory limit)")
    }
}

fn write_request(mut stdin: ChildStdin, head: &[u8], bytes: &[u8]) {
    // A broken pipe is normal when the worker rejects the request early or is killed.
    if stdin.write_all(head).is_ok() {
        let _ = stdin.write_all(bytes);
    }
    // Dropping `stdin` closes the pipe.
}
