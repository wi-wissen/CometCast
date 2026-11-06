// SPDX-License-Identifier: GPL-3.0-or-later
//! Starts, watches, restarts and stops the `uxplay` program.
//!
//! Plain threads and channels (no async), so the library is easy to use from any app.
//!
//! How it works, in short:
//! * One "manager" thread starts UxPlay inside a pseudo-terminal (PTY) and checks every
//!   50 ms whether it is still alive, and whether a command (stop, restart) arrived.
//! * One "reader" thread per UxPlay run reads the output lines, runs the parser and sends
//!   [`AirplayEvent`]s to the channel the app listens on.
//! * If UxPlay dies without being asked, the manager waits a growing time ([`Backoff`]) and
//!   starts it again.
//! * Stopping sends SIGTERM to the whole process group, waits up to `stop_timeout`, then
//!   kills, then reaps the process (so no zombie is left). After the leader has ended, the
//!   rest of its group gets SIGKILL, so helper processes do not survive.
//!
//! Why a PTY: when UxPlay's output goes into a normal pipe, its C library collects the
//! text in a block buffer, so a line like the PIN only arrives when UxPlay exits. We saw
//! this with the real UxPlay 1.68 (see docs/airplay.md). A PTY looks like a terminal, so
//! the lines are written at once.
//!
//! Policy is the app's job: this supervisor enforces nothing by itself (with
//! `confirm: None` every device that knows the PIN gets in). The app tells us whether a
//! mirroring session is active with [`Supervisor::set_session_active`].

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

use crate::config::{AirplayConfig, ConfigError};
use crate::parser::{Recognized, parse_line};
use crate::policy::{AirplayStatus, ConfirmHook, ConfirmRequest, Decision};

/// An event from the supervisor, for the app and its UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirplayEvent {
    /// The UxPlay program was started (attempt 1 = first start; planned restarts count).
    Starting {
        /// Count of starts so far, beginning with 1.
        attempt: u32,
    },
    /// UxPlay says it is ready for devices.
    ServerStarted,
    /// A device asked to pair (before its PIN is shown).
    PairingRequested,
    /// UxPlay shows a PIN, right after a pairing request. The app shows it on the display
    /// device. The PIN comes from the program's text output only: never treat it as proof
    /// of anything, and never let a UI show a PIN that was not announced by this event.
    PinShown {
        /// Four digits. Display only, not a decision of ours.
        pin: String,
    },
    /// A network connection to UxPlay appeared (the count went from 0 to more).
    /// UNCERTAIN: this is TCP level, not "a screen is mirrored". A pairing request itself
    /// is such a connection, and the line only appears when UxPlay runs with `-d`.
    ClientConnected {
        /// UxPlay's number of open connections.
        open_connections: u32,
    },
    /// All connections are closed again (same limits as above).
    ClientDisconnected,
    /// UxPlay printed an error.
    Error(String),
    /// Hint only: UxPlay may get no data from the device (line wording is UNCERTAIN).
    StreamStalled,
    /// The operator denied a device in the confirmation dialog; UxPlay is restarted.
    Denied,
    /// The operator denied a device, but a session is active, so nothing was done.
    DenyIgnored,
    /// The app told us a session became active or inactive (see
    /// [`Supervisor::set_session_active`]).
    SessionChanged {
        /// True = a session is active.
        active: bool,
    },
    /// The UxPlay process ended.
    ProcessExited {
        /// Exit code, if it ended by itself. `None` when it was killed by a signal.
        code: Option<u32>,
        /// Name of the signal that ended it, if any (as text from the system).
        signal: Option<String>,
        /// Process id of the run that ended (it is already reaped when you see this).
        pid: Option<u32>,
        /// True when we asked for it (stop, restart, denial); false = it died by itself.
        expected: bool,
    },
    /// UxPlay will be started again after the delay.
    Restarting {
        /// Number of failures in a row so far (1 = first). Planned restarts do not count.
        attempt: u32,
        /// The wait before the start.
        delay: Duration,
    },
    /// We give up: UxPlay cannot be started or keeps failing.
    Failed(String),
    /// The supervisor has stopped everything. This is the last event.
    Stopped,
}

/// Fold an event into the status the UI shows.
///
/// `Connected` is set only by [`AirplayEvent::SessionChanged`] (the app knows when a
/// session is really active). A PIN seen while `Connected` never overwrites that status.
pub fn status_after(prev: &AirplayStatus, ev: &AirplayEvent) -> AirplayStatus {
    match ev {
        AirplayEvent::Starting { .. } => AirplayStatus::Starting,
        AirplayEvent::ServerStarted => AirplayStatus::Waiting,
        AirplayEvent::PinShown { pin } => {
            if *prev == AirplayStatus::Connected {
                prev.clone()
            } else {
                AirplayStatus::PinShown { pin: pin.clone() }
            }
        }
        AirplayEvent::SessionChanged { active: true } => AirplayStatus::Connected,
        AirplayEvent::SessionChanged { active: false } => AirplayStatus::Waiting,
        AirplayEvent::Restarting { attempt, .. } => AirplayStatus::Restarting { attempt: *attempt },
        AirplayEvent::Failed(r) => AirplayStatus::Failed { reason: r.clone() },
        AirplayEvent::Stopped => AirplayStatus::Off,
        _ => prev.clone(),
    }
}

/// Waiting time before a restart: doubles each time up to a cap.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// First wait. UNCERTAIN: placeholder values, not tuned on real hardware.
    pub initial: Duration,
    /// Longest wait.
    pub max: Duration,
    /// A run at least this long counts as healthy and resets the wait.
    pub healthy_after: Duration,
    /// We give up at this many failed runs in a row (the 10th failure is the last).
    pub max_failures: u32,
    current: Duration,
    failures: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(1),
            Duration::from_secs(30),
            Duration::from_secs(30),
            10,
        )
    }
}

impl Backoff {
    /// Create a backoff with the given values.
    pub fn new(
        initial: Duration,
        max: Duration,
        healthy_after: Duration,
        max_failures: u32,
    ) -> Self {
        Self {
            initial,
            max,
            healthy_after,
            max_failures,
            current: initial,
            failures: 0,
        }
    }

    /// Forget earlier failures (after a planned restart).
    pub fn reset(&mut self) {
        self.current = self.initial;
        self.failures = 0;
    }

    /// Failures in a row so far.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// Call after an unexpected exit. `ran_for` = how long the process lived.
    /// Returns the wait before the next start, or `None` to give up (when this was the
    /// `max_failures`-th failure in a row).
    pub fn on_unexpected_exit(&mut self, ran_for: Duration) -> Option<Duration> {
        if ran_for >= self.healthy_after {
            self.current = self.initial;
            self.failures = 0;
        }
        self.failures += 1;
        if self.failures >= self.max_failures {
            return None;
        }
        let wait = self.current;
        self.current = (self.current * 2).min(self.max);
        Some(wait)
    }
}

/// Settings of the supervisor itself.
#[derive(Clone)]
pub struct SupervisorOptions {
    /// How long UxPlay gets to end after SIGTERM before it is killed.
    /// UNCERTAIN: 5 s is a placeholder.
    pub stop_timeout: Duration,
    /// Restart timing.
    pub backoff: Backoff,
    /// How long, at most, the manager waits for the output thread of an ended run to
    /// deliver its last lines (they are sent before the exit event). Normally the wait is
    /// over at once; the limit only matters if a helper outside the process group still
    /// holds the terminal, or if the machine is so loaded that the thread is late.
    pub reader_grace: Duration,
    /// The confirmation dialog of the app. `None` = no dialog, and then nothing here
    /// enforces any policy: every device that knows the PIN is accepted by UxPlay.
    /// Only one dialog is open at a time; a PIN that appears while a dialog is open gets
    /// no dialog of its own (it is dropped, not queued).
    pub confirm: Option<Arc<dyn ConfirmHook>>,
}

impl Default for SupervisorOptions {
    fn default() -> Self {
        Self {
            stop_timeout: Duration::from_secs(5),
            backoff: Backoff::default(),
            reader_grace: Duration::from_secs(1),
            confirm: None,
        }
    }
}

/// Commands to the manager thread.
enum Cmd {
    Stop,
    Restart,
    /// The operator denied; `generation` says which run it was about.
    Deny {
        generation: u32,
    },
}

/// State shared between the handle, the manager and the reader threads.
struct Shared {
    /// The app says a session is active.
    session_active: AtomicBool,
    /// A confirmation dialog is open.
    confirm_pending: AtomicBool,
    /// Number of the current run. Events of older runs are dropped.
    generation: AtomicU32,
}

/// Handle to a running supervisor. Dropping it stops UxPlay.
pub struct Supervisor {
    ctl: Sender<Cmd>,
    tx: Sender<AirplayEvent>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Supervisor {
    /// Validate the settings and start the manager thread. UxPlay is started by that thread;
    /// problems (program not found, ...) arrive as [`AirplayEvent::Failed`].
    pub fn start(
        config: AirplayConfig,
        options: SupervisorOptions,
    ) -> Result<(Self, Receiver<AirplayEvent>), ConfigError> {
        let args = config.build_args()?;
        let (tx, rx) = mpsc::channel();
        let (ctl, ctl_rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            session_active: AtomicBool::new(false),
            confirm_pending: AtomicBool::new(false),
            generation: AtomicU32::new(0),
        });
        let ctx = ManagerCtx {
            tx: tx.clone(),
            ctl: ctl.clone(),
            shared: Arc::clone(&shared),
        };
        let thread = thread::Builder::new()
            .name("cc-airplay-supervisor".into())
            .spawn(move || manage(&config, &args, options, &ctx, &ctl_rx))
            .expect("could not start the supervisor thread");
        Ok((
            Self {
                ctl,
                tx,
                shared,
                thread: Some(thread),
            },
            rx,
        ))
    }

    /// Tell the supervisor whether a mirroring session is active. The app decides this
    /// (UxPlay's output has no line we can trust for it). While it is active, a denial
    /// from the confirmation dialog changes nothing and no dialog is opened for new PINs,
    /// so a running picture is never cut by the dialog.
    pub fn set_session_active(&self, active: bool) {
        self.shared.session_active.store(active, Ordering::SeqCst);
        let _ = self.tx.send(AirplayEvent::SessionChanged { active });
    }

    /// Restart UxPlay now (no waiting time). Does nothing if the supervisor has ended.
    /// This also ends a running session: the app should call it only on purpose.
    pub fn restart(&self) {
        let _ = self.ctl.send(Cmd::Restart);
    }

    /// Ask for a stop without waiting. The last event is [`AirplayEvent::Stopped`]; call
    /// [`stop`](Self::stop) afterwards to wait for the thread.
    pub fn request_stop(&self) {
        let _ = self.ctl.send(Cmd::Stop);
    }

    /// Stop UxPlay (SIGTERM to its group, kill after the timeout) and wait until it is
    /// gone. Safe to call twice.
    pub fn stop(&mut self) {
        let _ = self.ctl.send(Cmd::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// What the manager and reader threads need to talk to the outside.
#[derive(Clone)]
struct ManagerCtx {
    tx: Sender<AirplayEvent>,
    ctl: Sender<Cmd>,
    shared: Arc<Shared>,
}

/// One running UxPlay.
struct Running {
    child: Box<dyn Child + Send + Sync>,
    /// Keeps the PTY open while UxPlay runs.
    _master: Box<dyn portable_pty::MasterPty + Send>,
    /// Gets a message when the reader thread has finished.
    reader_done: Receiver<()>,
    started: Instant,
    pid: Option<u32>,
}

fn spawn_uxplay(
    config: &AirplayConfig,
    args: &[String],
    generation: u32,
    options: &SupervisorOptions,
    ctx: &ManagerCtx,
) -> Result<Running, String> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 200,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("cannot open a pseudo-terminal: {e}"))?;
    let mut cmd = CommandBuilder::new(&config.uxplay_path);
    cmd.args(args);
    if let Ok(dir) = std::env::current_dir() {
        cmd.cwd(dir);
    }
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("cannot start {}: {e}", config.uxplay_path.display()))?;
    let pid = child.process_id();
    // Close our copy of the slave side, or the reader never sees the end of the output.
    drop(pair.slave);
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("cannot read the output: {e}"))?;
    let (done_tx, reader_done) = mpsc::channel();
    let reader_ctx = ReaderCtx {
        ctx: ctx.clone(),
        confirm: options.confirm.clone(),
        generation,
    };
    thread::Builder::new()
        .name("cc-airplay-reader".into())
        .spawn(move || {
            read_lines(reader, &reader_ctx);
            let _ = done_tx.send(());
        })
        .map_err(|e| format!("cannot start the reader thread: {e}"))?;
    Ok(Running {
        child,
        _master: pair.master,
        reader_done,
        started: Instant::now(),
        pid,
    })
}

/// Longest line we keep. Longer lines are cut (the rest is thrown away).
const MAX_LINE: usize = 8192;

struct ReaderCtx {
    ctx: ManagerCtx,
    confirm: Option<Arc<dyn ConfirmHook>>,
    generation: u32,
}

impl ReaderCtx {
    /// Send an event, unless a newer run has started (a late reader must stay silent).
    fn emit(&self, ev: AirplayEvent) {
        if self.ctx.shared.generation.load(Ordering::SeqCst) == self.generation {
            let _ = self.ctx.tx.send(ev);
        }
    }
}

/// Read the output, split it into lines and send events.
fn read_lines(mut reader: Box<dyn Read + Send>, rc: &ReaderCtx) {
    let mut state = LineState::default();
    let mut buf = [0u8; 4096];
    let mut line: Vec<u8> = Vec::new();
    // On Linux the read ends with an error (EIO) when the other side closes; that is normal.
    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b == b'\n' {
                handle_line(&line, &mut state, rc);
                line.clear();
            } else if line.len() < MAX_LINE {
                line.push(b);
            }
        }
    }
    if !line.is_empty() {
        handle_line(&line, &mut state, rc);
    }
}

/// What the line handler remembers between lines.
#[derive(Default)]
struct LineState {
    connected: bool,
    /// A pairing request was seen and its PIN line has not come yet.
    pairing: bool,
}

fn handle_line(raw: &[u8], state: &mut LineState, rc: &ReaderCtx) {
    let text = String::from_utf8_lossy(raw);
    let Some(rec) = parse_line(text.trim_end_matches('\r')) else {
        return;
    };
    let ev = match rec {
        Recognized::ServerReady => AirplayEvent::ServerStarted,
        Recognized::PairingRequested => {
            state.pairing = true;
            AirplayEvent::PairingRequested
        }
        Recognized::PinShown { pin } => {
            // A PIN line without a pairing request just before it is not believed.
            if !std::mem::take(&mut state.pairing) {
                return;
            }
            maybe_ask_operator(&pin, rc);
            AirplayEvent::PinShown { pin }
        }
        Recognized::OpenConnections(n) => {
            if n > 0 && !state.connected {
                state.connected = true;
                AirplayEvent::ClientConnected {
                    open_connections: n,
                }
            } else if n == 0 && state.connected {
                state.connected = false;
                AirplayEvent::ClientDisconnected
            } else {
                return;
            }
        }
        Recognized::Error(m) => AirplayEvent::Error(m),
        Recognized::StreamStalled => AirplayEvent::StreamStalled,
        Recognized::Stopping => return,
    };
    rc.emit(ev);
}

/// Open the confirmation dialog on its own thread, but only if there is a hook, no session
/// is active and no other dialog is open.
fn maybe_ask_operator(pin: &str, rc: &ReaderCtx) {
    let Some(hook) = &rc.confirm else { return };
    let shared = &rc.ctx.shared;
    if shared.session_active.load(Ordering::SeqCst) {
        return;
    }
    if shared
        .confirm_pending
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return; // a dialog is open already: drop this one
    }
    let hook = Arc::clone(hook);
    let ctx = rc.ctx.clone();
    let generation = rc.generation;
    let req = ConfirmRequest {
        pin: pin.to_string(),
    };
    let spawned = thread::Builder::new()
        .name("cc-airplay-confirm".into())
        .spawn(move || {
            let decision = hook.confirm(&req);
            ctx.shared.confirm_pending.store(false, Ordering::SeqCst);
            if decision == Decision::Deny {
                if ctx.shared.session_active.load(Ordering::SeqCst) {
                    let _ = ctx.tx.send(AirplayEvent::DenyIgnored);
                } else {
                    let _ = ctx.ctl.send(Cmd::Deny { generation });
                }
            }
        });
    if spawned.is_err() {
        shared.confirm_pending.store(false, Ordering::SeqCst);
    }
}

/// Exit code and signal name of an ended process.
type ExitInfo = (Option<u32>, Option<String>);

fn exit_info(status: &portable_pty::ExitStatus) -> ExitInfo {
    match status.signal() {
        Some(sig) => (None, Some(sig.to_string())),
        None => (Some(status.exit_code()), None),
    }
}

/// Send a signal to the whole process group of the child (the PTY makes the child a
/// session leader, so its group id is its pid). Unix only.
#[cfg(unix)]
fn signal_group(pid: Option<u32>, sig: nix::sys::signal::Signal) {
    if let Some(pid) = pid.and_then(|p| i32::try_from(p).ok()) {
        let pid = nix::unistd::Pid::from_raw(pid);
        if nix::sys::signal::killpg(pid, sig).is_err() {
            let _ = nix::sys::signal::kill(pid, sig);
        }
    }
}

/// Kill whatever is left of the child's group after the leader has ended.
/// UNCERTAIN: once the leader is reaped, its number could in theory be reused by another
/// process group before this call; this needs the pid space to wrap within milliseconds,
/// and while any group member is alive the number cannot be reused.
fn kill_leftovers(pid: Option<u32>) {
    #[cfg(unix)]
    signal_group(pid, nix::sys::signal::SIGKILL);
    #[cfg(not(unix))]
    let _ = pid;
}

/// Ask the process to end; kill it if it does not within `timeout`. Always reaps it.
fn terminate(run: &mut Running, timeout: Duration) -> ExitInfo {
    #[cfg(unix)]
    signal_group(run.pid, nix::sys::signal::SIGTERM);
    #[cfg(not(unix))]
    let _ = run.child.kill();

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = run.child.try_wait() {
            kill_leftovers(run.pid);
            return exit_info(&status);
        }
        thread::sleep(Duration::from_millis(20));
    }
    // Too slow. Note: portable-pty's own kill() sends SIGHUP on Unix (read in its source),
    // which a program can ignore, so we send SIGKILL ourselves, then reap.
    kill_leftovers(run.pid);
    #[cfg(not(unix))]
    let _ = run.child.kill();
    match run.child.wait() {
        Ok(status) => exit_info(&status),
        Err(_) => (None, None),
    }
}

/// Wait (bounded) for the reader thread. If a helper outside our group still holds the
/// terminal, the reader may live on; its events are dropped by the generation check.
fn finish_reader(run: &Running, grace: Duration) {
    let _ = run.reader_done.recv_timeout(grace);
}

/// Why the inner loop of one run ended.
enum RunEnd {
    /// The process ended by itself.
    Died(ExitInfo),
    /// We ended it on request; `stop` = the app wants a stop, otherwise a restart.
    Asked { stop: bool, denied: bool },
}

fn manage(
    config: &AirplayConfig,
    args: &[String],
    mut options: SupervisorOptions,
    ctx: &ManagerCtx,
    ctl_rx: &Receiver<Cmd>,
) {
    let send = |ev: AirplayEvent| {
        let _ = ctx.tx.send(ev);
    };
    let mut generation: u32 = 0;
    'outer: loop {
        generation += 1;
        ctx.shared.generation.store(generation, Ordering::SeqCst);
        send(AirplayEvent::Starting {
            attempt: generation,
        });
        let mut run = match spawn_uxplay(config, args, generation, &options, ctx) {
            Ok(r) => r,
            Err(msg) => {
                // Most likely "program not found". Trying again would not help.
                send(AirplayEvent::Failed(msg));
                break;
            }
        };

        let end = loop {
            match ctl_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Cmd::Stop) | Err(RecvTimeoutError::Disconnected) => {
                    break RunEnd::Asked {
                        stop: true,
                        denied: false,
                    };
                }
                Ok(Cmd::Restart) => {
                    break RunEnd::Asked {
                        stop: false,
                        denied: false,
                    };
                }
                Ok(Cmd::Deny { generation: g }) => {
                    // Ignore answers about older runs, and never cut a running session.
                    if g == generation && !ctx.shared.session_active.load(Ordering::SeqCst) {
                        break RunEnd::Asked {
                            stop: false,
                            denied: true,
                        };
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            if let Ok(Some(status)) = run.child.try_wait() {
                kill_leftovers(run.pid);
                break RunEnd::Died(exit_info(&status));
            }
        };

        let ran_for = run.started.elapsed();
        match end {
            RunEnd::Asked { mut stop, denied } => {
                let (code, signal) = terminate(&mut run, options.stop_timeout);
                finish_reader(&run, options.reader_grace);
                if denied {
                    send(AirplayEvent::Denied);
                }
                send(AirplayEvent::ProcessExited {
                    code,
                    signal,
                    pid: run.pid,
                    expected: true,
                });
                // Commands that piled up while we were busy are stale, except a stop.
                while let Ok(c) = ctl_rx.try_recv() {
                    if matches!(c, Cmd::Stop) {
                        stop = true;
                    }
                }
                if stop {
                    break 'outer;
                }
                options.backoff.reset(); // a planned restart starts fresh
            }
            RunEnd::Died((code, signal)) => {
                // Already reaped by try_wait. Clean up the output thread.
                finish_reader(&run, options.reader_grace);
                send(AirplayEvent::ProcessExited {
                    code,
                    signal,
                    pid: run.pid,
                    expected: false,
                });
                let Some(delay) = options.backoff.on_unexpected_exit(ran_for) else {
                    send(AirplayEvent::Failed(format!(
                        "UxPlay failed {} times in a row; giving up",
                        options.backoff.failures()
                    )));
                    break 'outer;
                };
                send(AirplayEvent::Restarting {
                    attempt: options.backoff.failures(),
                    delay,
                });
                // Wait, but stay responsive to stop and restart.
                let until = Instant::now() + delay;
                while Instant::now() < until {
                    let step = until
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(50));
                    match ctl_rx.recv_timeout(step) {
                        Ok(Cmd::Stop) | Err(RecvTimeoutError::Disconnected) => break 'outer,
                        Ok(Cmd::Restart) => break,
                        Ok(Cmd::Deny { .. }) | Err(RecvTimeoutError::Timeout) => {}
                    }
                }
            }
        }
    }
    send(AirplayEvent::Stopped);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn backoff_doubles_up_to_cap() {
        let mut b = Backoff::new(ms(100), ms(350), ms(1000), 10);
        let q = ms(10);
        assert_eq!(b.on_unexpected_exit(q), Some(ms(100)));
        assert_eq!(b.on_unexpected_exit(q), Some(ms(200)));
        assert_eq!(b.on_unexpected_exit(q), Some(ms(350)));
        assert_eq!(b.on_unexpected_exit(q), Some(ms(350)));
    }

    #[test]
    fn backoff_resets_after_healthy_run() {
        let mut b = Backoff::new(ms(100), ms(800), ms(1000), 3);
        b.on_unexpected_exit(ms(1));
        b.on_unexpected_exit(ms(1));
        assert_eq!(b.on_unexpected_exit(ms(2000)), Some(ms(100)));
    }

    #[test]
    fn backoff_gives_up_at_the_max_failure() {
        // max_failures = 3: failures 1 and 2 restart, the 3rd failure in a row gives up.
        let mut b = Backoff::new(ms(1), ms(2), ms(1000), 3);
        assert!(b.on_unexpected_exit(ms(1)).is_some());
        assert!(b.on_unexpected_exit(ms(1)).is_some());
        assert_eq!(b.on_unexpected_exit(ms(1)), None);
        assert_eq!(b.failures(), 3);
        // Default is 10: the 10th failure is the last.
        let mut d = Backoff::default();
        let n = (1..=20).find(|_| d.on_unexpected_exit(ms(1)).is_none());
        assert_eq!(n, Some(10));
    }

    #[test]
    fn status_follows_events() {
        let mut s = AirplayStatus::Off;
        let steps = [
            (
                AirplayEvent::Starting { attempt: 1 },
                AirplayStatus::Starting,
            ),
            (AirplayEvent::ServerStarted, AirplayStatus::Waiting),
            (
                AirplayEvent::ClientConnected {
                    open_connections: 1,
                },
                AirplayStatus::Waiting,
            ),
            (
                AirplayEvent::PinShown { pin: "1234".into() },
                AirplayStatus::PinShown { pin: "1234".into() },
            ),
            (
                AirplayEvent::SessionChanged { active: true },
                AirplayStatus::Connected,
            ),
            // A PIN or a stall hint while a session is active does not change the status.
            (
                AirplayEvent::PinShown { pin: "5555".into() },
                AirplayStatus::Connected,
            ),
            (AirplayEvent::StreamStalled, AirplayStatus::Connected),
            (AirplayEvent::DenyIgnored, AirplayStatus::Connected),
            (
                AirplayEvent::SessionChanged { active: false },
                AirplayStatus::Waiting,
            ),
            (
                AirplayEvent::Restarting {
                    attempt: 2,
                    delay: ms(1),
                },
                AirplayStatus::Restarting { attempt: 2 },
            ),
            (AirplayEvent::Stopped, AirplayStatus::Off),
        ];
        for (ev, want) in steps {
            s = status_after(&s, &ev);
            assert_eq!(s, want, "after {ev:?}");
        }
    }
}
