// SPDX-License-Identifier: GPL-3.0-or-later
//! Parent side of the render worker process.
//!
//! One short-lived child per call (open or render): the PDF goes to its stdin, one framed
//! reply comes back on stdout (`wire.rs`). The child is the memory and time boundary:
//!
//! * Linux and Android: started through `sh -c 'ulimit ...; exec worker'` with limits on
//!   address space (`-v`), CPU seconds (`-t`) and core dumps (`-c 0`). An allocation over
//!   the limit makes the child abort; the parent only sees an abnormal end.
//! * macOS and other Unix: no address-space limit (`RLIMIT_AS` is not enforced on macOS); the
//!   parent polls the child's resident memory with `ps` and kills it above the cap.
//! * Windows: no job object (that needs `unsafe`); the parent polls with `tasklist`. Weaker
//!   and untested, see `docs/documents.md`.
//! * Everywhere: a wall-clock deadline, a cancel flag, then `Child::kill` and `wait`. Nothing
//!   is abandoned: killing the child closes its pipes and ends the two helper threads.
//!
//! The parent never allocates more than it asked for: the reply sizes are checked against
//! the request before the buffer is made.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[cfg(unix)]
use crate::clamp::clamp_timeout;
use crate::clamp::deadline;
use crate::document::{Limits, PageSize, RenderedPage};
use crate::error::DocError;
use crate::isolation::WorkerSpec;
use crate::process_stats::{POLL_MS, rss_kib_of};
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

/// Kills and reaps the child when dropped, so no path leaves a process behind.
struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn kill_and_wait(&mut self) -> Option<ExitStatus> {
        let mut c = self.0.take()?;
        let _ = c.kill();
        c.wait().ok()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn command(spec: &WorkerSpec, limits: &Limits, timeout: Duration) -> Command {
    // `exec` replaces the shell, so the child's pid is the worker's pid. A failing `-v` is
    // fatal (exit 97): a worker without its memory limit is not started.
    let kib = limits.max_worker_memory_bytes / 1024;
    let cpu = clamp_timeout(timeout).as_secs().saturating_add(2);
    let script = format!(
        "ulimit -c 0 2>/dev/null; ulimit -t {cpu} 2>/dev/null; ulimit -v {kib} 2>/dev/null || exit 97; exec \"$0\" \"$@\""
    );
    let mut c = Command::new("sh");
    c.arg("-c").arg(script).arg(&spec.program).args(&spec.args);
    c
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn command(spec: &WorkerSpec, _limits: &Limits, timeout: Duration) -> Command {
    // No `ulimit -v` here (not enforced on macOS); the RSS watchdog in `run` is the memory cap.
    let cpu = clamp_timeout(timeout).as_secs().saturating_add(2);
    let script =
        format!("ulimit -c 0 2>/dev/null; ulimit -t {cpu} 2>/dev/null; exec \"$0\" \"$@\"");
    let mut c = Command::new("sh");
    c.arg("-c").arg(script).arg(&spec.program).args(&spec.args);
    c
}

#[cfg(not(unix))]
fn command(spec: &WorkerSpec, _limits: &Limits, _timeout: Duration) -> Command {
    let mut c = Command::new(&spec.program);
    c.args(&spec.args);
    c
}

/// Is `sh` with a working `ulimit -v` available? (Used by tests to skip with a note.)
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn sandbox_tools_available() -> bool {
    Command::new("sh")
        .args(["-c", "ulimit -v 4000000 && ulimit -t 100"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Is `sh` available? (Other platforms rely on the memory watchdog only.)
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn sandbox_tools_available() -> bool {
    cfg!(unix)
        && Command::new("sh")
            .args(["-c", "true"])
            .status()
            .is_ok_and(|s| s.success())
}

/// A reply read from the worker's stdout.
struct Reply {
    output: Result<Output, DocError>,
    peak_kib: u64,
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
    Ok(Reply {
        output,
        peak_kib: h.peak_kib,
    })
}

/// Run one call in a child process. Returns the result and the highest resident memory seen
/// (KiB, 0 if unknown) for diagnostics.
pub(crate) fn run(
    spec: &WorkerSpec,
    bytes: &Arc<Vec<u8>>,
    limits: &Limits,
    call: Call,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> (Result<Output, DocError>, u64) {
    let mut cmd = command(spec, limits, timeout);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return (
                Err(DocError::WorkerUnavailable(format!(
                    "cannot start {}: {e}",
                    spec.program.display()
                ))),
                0,
            );
        }
    };
    let pid = child.id();
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return (Err(DocError::Internal("no pipes to the worker")), 0);
    };
    let mut guard = ChildGuard(Some(child));
    let (op, index, width, height) = match call {
        Call::Info => (OP_INFO, 0u64, 0u32, 0u32),
        Call::Render {
            index,
            width,
            height,
        } => (OP_RENDER, index as u64, width, height),
    };
    let head = wire::encode_request(op, index, width, height, limits, bytes.len() as u64);
    let cap_kib = limits.max_worker_memory_bytes / 1024;
    let end = deadline(timeout);

    std::thread::scope(|scope| {
        let writer = scope.spawn(move || write_request(stdin, &head, bytes));
        let reader = scope.spawn(move || read_reply(stdout, call, limits));

        // Watch the child. Every exit from this loop has killed it or seen it end.
        let mut peak = 0u64;
        let mut last_poll = Instant::now();
        let mut verdict: Option<DocError> = None;
        let status = loop {
            if cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
                verdict = Some(DocError::Cancelled);
                break guard.kill_and_wait();
            }
            let waited = guard.0.as_mut().map(Child::try_wait);
            match waited {
                Some(Ok(Some(st))) => {
                    guard.0 = None;
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
                    peak = peak.max(kib);
                    if kib > cap_kib {
                        verdict = Some(DocError::MemoryLimit);
                        break guard.kill_and_wait();
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        };

        // The pipes are closed now (the child ended), so both helpers finish.
        let _ = writer.join();
        let reply = reader.join().unwrap_or(Err(DocError::Panicked));
        let (result, reply_peak) = match (verdict, reply) {
            (Some(e), _) => (Err(e), 0),
            (None, Ok(r)) => {
                let p = r.peak_kib;
                let ok_exit = status.is_some_and(|s| s.success());
                // An error reply is trusted whatever the exit code; a picture needs exit 0.
                match r.output {
                    Ok(_) if !ok_exit => (
                        Err(DocError::WorkerFailed("the renderer ended abnormally")),
                        p,
                    ),
                    other => (other, p),
                }
            }
            (None, Err(e)) => (Err(refine(e, status)), 0),
        };
        (result, peak.max(reply_peak))
    })
}

/// Turn "no valid answer" into a more useful message from the exit status.
fn refine(e: DocError, status: Option<ExitStatus>) -> DocError {
    let Some(st) = status else { return e };
    if st.code() == Some(97) {
        return DocError::WorkerFailed("the memory limit could not be applied");
    }
    // 126 and 127: the shell could not run the worker program.
    if matches!(st.code(), Some(126 | 127)) {
        return DocError::WorkerUnavailable("the worker program cannot be run".into());
    }
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
