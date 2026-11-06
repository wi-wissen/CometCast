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
//! * Stopping sends SIGTERM, waits up to `stop_timeout`, then kills, then reaps the process
//!   (so no zombie is left).
//!
//! Why a PTY: when UxPlay's output goes into a normal pipe, its C library collects the
//! text in a block buffer, so a line like the PIN only arrives when UxPlay exits. We saw
//! this with the real UxPlay 1.68 (see docs/airplay.md). A PTY looks like a terminal, so
//! the lines are written at once.

use std::io::Read;
use std::sync::Arc;
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
    /// The UxPlay program was started (attempt 1 = first start).
    Starting {
        /// Count of starts so far, beginning with 1.
        attempt: u32,
    },
    /// UxPlay says it is ready for devices.
    ServerStarted,
    /// A device asked to pair (before its PIN is shown).
    PairingRequested,
    /// UxPlay shows a PIN. The app must show it on the display device.
    PinShown {
        /// Four digits. Display only, not a decision of ours.
        pin: String,
    },
    /// A network connection to UxPlay appeared (the count went from 0 to more).
    /// UNCERTAIN: this is TCP level. A short probe also counts, as the real log shows.
    ClientConnected {
        /// UxPlay's number of open connections.
        open_connections: u32,
    },
    /// All connections are closed again.
    ClientDisconnected,
    /// UxPlay printed an error.
    Error(String),
    /// UxPlay seems to get no data from the device (wording of the line is UNCERTAIN).
    StreamStalled,
    /// The operator denied a device in the confirmation dialog; UxPlay is restarted.
    Denied,
    /// The UxPlay process ended.
    ProcessExited {
        /// Exit code if known.
        code: Option<u32>,
        /// Process id of the run that ended (it is already reaped when you see this).
        pid: Option<u32>,
        /// True when we asked for it (stop, restart, denial); false = it died by itself.
        expected: bool,
    },
    /// UxPlay will be started again after the delay.
    Restarting {
        /// Which restart this is (1 = first).
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
pub fn status_after(prev: &AirplayStatus, ev: &AirplayEvent) -> AirplayStatus {
    match ev {
        AirplayEvent::Starting { .. } => AirplayStatus::Starting,
        AirplayEvent::ServerStarted | AirplayEvent::ClientDisconnected => AirplayStatus::Waiting,
        AirplayEvent::PinShown { pin } => AirplayStatus::PinShown { pin: pin.clone() },
        AirplayEvent::ClientConnected { .. } => AirplayStatus::Connected,
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
    /// After this many failed runs in a row we give up.
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

    /// Call after an unexpected exit. `ran_for` = how long the process lived.
    /// Returns the wait before the next start, or `None` to give up.
    pub fn on_unexpected_exit(&mut self, ran_for: Duration) -> Option<Duration> {
        if ran_for >= self.healthy_after {
            self.current = self.initial;
            self.failures = 0;
        }
        self.failures += 1;
        if self.failures > self.max_failures {
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
    /// The confirmation dialog of the app. `None` = no dialog.
    pub confirm: Option<Arc<dyn ConfirmHook>>,
}

impl Default for SupervisorOptions {
    fn default() -> Self {
        Self {
            stop_timeout: Duration::from_secs(5),
            backoff: Backoff::default(),
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

/// Handle to a running supervisor. Dropping it stops UxPlay.
pub struct Supervisor {
    ctl: Sender<Cmd>,
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
        let ctl2 = ctl.clone();
        let thread = thread::Builder::new()
            .name("cc-airplay-supervisor".into())
            .spawn(move || manage(&config, &args, options, &tx, &ctl_rx, &ctl2))
            .expect("could not start the supervisor thread");
        Ok((
            Self {
                ctl,
                thread: Some(thread),
            },
            rx,
        ))
    }

    /// Restart UxPlay now (no waiting time). Does nothing if the supervisor has ended.
    pub fn restart(&self) {
        let _ = self.ctl.send(Cmd::Restart);
    }

    /// Ask for a stop without waiting. The last event is [`AirplayEvent::Stopped`]; call
    /// [`stop`](Self::stop) afterwards to wait for the thread.
    pub fn request_stop(&self) {
        let _ = self.ctl.send(Cmd::Stop);
    }

    /// Stop UxPlay (SIGTERM, then kill after the timeout) and wait until it is gone.
    /// Safe to call twice.
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
    tx: &Sender<AirplayEvent>,
    ctl: &Sender<Cmd>,
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
    let tx2 = tx.clone();
    let ctl2 = ctl.clone();
    let confirm = options.confirm.clone();
    thread::Builder::new()
        .name("cc-airplay-reader".into())
        .spawn(move || {
            read_lines(reader, &tx2, &ctl2, confirm, generation);
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

/// Read the output, split it into lines and send events.
fn read_lines(
    mut reader: Box<dyn Read + Send>,
    tx: &Sender<AirplayEvent>,
    ctl: &Sender<Cmd>,
    confirm: Option<Arc<dyn ConfirmHook>>,
    generation: u32,
) {
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
                handle_line(&line, &mut state, tx, ctl, confirm.as_ref(), generation);
                line.clear();
            } else if line.len() < MAX_LINE {
                line.push(b);
            }
        }
    }
    if !line.is_empty() {
        handle_line(&line, &mut state, tx, ctl, confirm.as_ref(), generation);
    }
}

/// What the line handler remembers between lines.
#[derive(Default)]
struct LineState {
    connected: bool,
}

fn handle_line(
    raw: &[u8],
    state: &mut LineState,
    tx: &Sender<AirplayEvent>,
    ctl: &Sender<Cmd>,
    confirm: Option<&Arc<dyn ConfirmHook>>,
    generation: u32,
) {
    let text = String::from_utf8_lossy(raw);
    let Some(rec) = parse_line(text.trim_end_matches('\r')) else {
        return;
    };
    let ev = match rec {
        Recognized::ServerReady => AirplayEvent::ServerStarted,
        Recognized::PairingRequested => AirplayEvent::PairingRequested,
        Recognized::PinShown { pin } => {
            if let Some(hook) = confirm {
                // The dialog may block, so it gets its own thread.
                let hook = Arc::clone(hook);
                let ctl = ctl.clone();
                let req = ConfirmRequest { pin: pin.clone() };
                thread::spawn(move || {
                    if hook.confirm(&req) == Decision::Deny {
                        let _ = ctl.send(Cmd::Deny { generation });
                    }
                });
            }
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
    let _ = tx.send(ev);
}

/// Ask the process to end; kill it if it does not within `timeout`. Always reaps it.
/// Returns the exit code if known.
fn terminate(run: &mut Running, timeout: Duration) -> Option<u32> {
    #[cfg(unix)]
    if let Some(pid) = run.child.process_id().and_then(|p| i32::try_from(p).ok()) {
        let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), nix::sys::signal::SIGTERM);
    }
    #[cfg(not(unix))]
    let _ = run.child.kill();

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(Some(status)) = run.child.try_wait() {
            return Some(status.exit_code());
        }
        thread::sleep(Duration::from_millis(20));
    }
    // Too slow: kill the whole process group (UxPlay may have helper processes), then reap.
    // The child is a session leader (the PTY does that), so its group id is its pid.
    // The process is not reaped yet, so the pid cannot have been reused.
    // Note: portable-pty's own kill() sends SIGHUP on Unix (read in its source), which a
    // program can ignore, so we send SIGKILL ourselves.
    #[cfg(unix)]
    if let Some(pid) = run.child.process_id().and_then(|p| i32::try_from(p).ok()) {
        let pid = nix::unistd::Pid::from_raw(pid);
        let _ = nix::sys::signal::killpg(pid, nix::sys::signal::SIGKILL);
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::SIGKILL);
    }
    #[cfg(not(unix))]
    let _ = run.child.kill();
    run.child.wait().ok().map(|s| s.exit_code())
}

/// Wait (bounded) for the reader thread, so late lines are not mixed into the next run.
fn finish_reader(run: &Running) {
    let _ = run.reader_done.recv_timeout(Duration::from_secs(1));
}

/// Why the inner loop of one run ended.
enum RunEnd {
    /// The process ended by itself.
    Died(Option<u32>),
    /// We ended it on request; `true` = the app wants a stop, `false` = restart.
    Asked { stop: bool, denied: bool },
}

fn manage(
    config: &AirplayConfig,
    args: &[String],
    mut options: SupervisorOptions,
    tx: &Sender<AirplayEvent>,
    ctl_rx: &Receiver<Cmd>,
    ctl: &Sender<Cmd>,
) {
    let send = |ev: AirplayEvent| {
        let _ = tx.send(ev);
    };
    let mut attempt: u32 = 0;
    'outer: loop {
        attempt += 1;
        send(AirplayEvent::Starting { attempt });
        let mut run = match spawn_uxplay(config, args, attempt, &options, tx, ctl) {
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
                Ok(Cmd::Deny { generation }) if generation == attempt => {
                    break RunEnd::Asked {
                        stop: false,
                        denied: true,
                    };
                }
                Ok(Cmd::Deny { .. }) => {} // about an older run: ignore
                Err(RecvTimeoutError::Timeout) => {}
            }
            if let Ok(Some(status)) = run.child.try_wait() {
                break RunEnd::Died(Some(status.exit_code()));
            }
        };

        let ran_for = run.started.elapsed();
        match end {
            RunEnd::Asked { stop, denied } => {
                let code = terminate(&mut run, options.stop_timeout);
                finish_reader(&run);
                if denied {
                    send(AirplayEvent::Denied);
                }
                send(AirplayEvent::ProcessExited {
                    code,
                    pid: run.pid,
                    expected: true,
                });
                if stop {
                    break 'outer;
                }
                options.backoff.reset(); // a planned restart starts fresh
            }
            RunEnd::Died(code) => {
                // Already reaped by try_wait. Clean up the output thread.
                finish_reader(&run);
                send(AirplayEvent::ProcessExited {
                    code,
                    pid: run.pid,
                    expected: false,
                });
                let Some(delay) = options.backoff.on_unexpected_exit(ran_for) else {
                    send(AirplayEvent::Failed(format!(
                        "UxPlay failed {} times in a row; giving up",
                        options.backoff.max_failures
                    )));
                    break 'outer;
                };
                send(AirplayEvent::Restarting { attempt, delay });
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
    fn backoff_gives_up() {
        let mut b = Backoff::new(ms(1), ms(2), ms(1000), 2);
        assert!(b.on_unexpected_exit(ms(1)).is_some());
        assert!(b.on_unexpected_exit(ms(1)).is_some());
        assert_eq!(b.on_unexpected_exit(ms(1)), None);
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
                AirplayEvent::PinShown { pin: "1234".into() },
                AirplayStatus::PinShown { pin: "1234".into() },
            ),
            (
                AirplayEvent::ClientConnected {
                    open_connections: 1,
                },
                AirplayStatus::Connected,
            ),
            (AirplayEvent::StreamStalled, AirplayStatus::Connected),
            (AirplayEvent::ClientDisconnected, AirplayStatus::Waiting),
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
