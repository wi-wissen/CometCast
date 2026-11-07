// SPDX-License-Identifier: GPL-3.0-or-later
//! The display's AirPlay receiver as the app uses it (native-apps.md 21.4, decisions N87, N88).
//!
//! [`AirplayReceiver`] wraps the [`Supervisor`] and the rules of [`crate::policy`]:
//!
//! * UxPlay always runs with a PIN (never `-reg`) and with `-d`, so the end of a mirroring can be
//!   read from its output.
//! * **Admission by join mode.** Open: a device gets the PIN at once ([`ReceiveEvent::Pin`]).
//!   Approval on: [`ReceiveEvent::Request`] first, the app answers with
//!   [`AirplayReceiver::answer`]; a refusal restarts UxPlay (that drops the pairing). Locked: no
//!   new device, the service stops as soon as no mirroring runs (a running one stays until it
//!   ends). While a Comet Cast presenter presents ([`AirplayReceiver::set_refusing`]) every
//!   pairing is refused without a request.
//! * **Mirroring signal.** [`ReceiveEvent::Mirroring`] comes from the supervisor's trusted
//!   `MirroringChanged` (UxPlay's own mirror line after a PIN, see [`crate::parser`]). It is the
//!   only input for wake lock, "Comet Cast first" and `toast.rxEnded`.
//! * The PIN shown is always one that the supervisor read from UxPlay's output.
//!
//! This type has no UI. `cc-ffi` turns [`ReceiveEvent`] into the Dart events.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{AirplayConfig, ConfigError};
use crate::parser::MIRRORING_STARTED_LINE;
use crate::policy::{
    Admission, ConfirmHook, ConfirmRequest, Decision, JoinMode, PolicyState, admit_new_session,
};
use crate::supervisor::{AirplayEvent, Supervisor, SupervisorOptions};

/// How long a request waits for the operator before it is refused. UNCERTAIN: placeholder; the
/// server's own request expiry is not known to this crate.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// What the app is told about the receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveEvent {
    /// UxPlay is ready: the receiver is offered to devices.
    Advertising,
    /// The receiver is not offered (Locked stopped it).
    NotAnnouncing,
    /// A device wants to connect and the display is in Approval on: ask first, then the PIN.
    Request,
    /// Show this PIN on the screen (read from UxPlay's output, only after the join mode let the
    /// device through).
    Pin {
        /// Four digits.
        pin: String,
    },
    /// The pairing attempt is over (mirroring started, the device went away, it was refused or
    /// timed out): close the request or PIN dialog.
    PinDone,
    /// A device started or stopped mirroring.
    Mirroring {
        /// True = a picture is shown by UxPlay.
        active: bool,
    },
    /// The receiver cannot run. `code`: `not_installed`, `gstreamer_missing`, `no_dnssd`,
    /// `bad_config`, `failed`.
    Failed {
        /// Short machine-readable code; the app has its own texts.
        code: String,
    },
    /// The receiver was stopped by the app. The last event.
    Stopped,
}

/// Settings of [`AirplayReceiver::start`].
#[derive(Clone)]
pub struct ReceiveOptions {
    /// UxPlay's settings. `debug_log` is forced on; the PIN is always requested.
    pub config: AirplayConfig,
    /// Restart timing and so on. `confirm` is replaced.
    pub supervisor: SupervisorOptions,
    /// Approval on (true) or Open (false).
    pub ask_first: bool,
    /// How long a request waits for [`AirplayReceiver::answer`].
    pub request_timeout: Duration,
}

impl ReceiveOptions {
    /// Defaults around `config`: Open, 60 s for a request.
    pub fn new(config: AirplayConfig) -> Self {
        Self {
            config,
            supervisor: SupervisorOptions::default(),
            ask_first: false,
            request_timeout: REQUEST_TIMEOUT,
        }
    }
}

struct State {
    ask_first: bool,
    locked: bool,
    refusing: bool,
    mirroring: bool,
    pin_open: bool,
    /// A request waits for the operator.
    waiting: bool,
    answer: Option<bool>,
    stopping: bool,
    supervisor: Option<Supervisor>,
}

impl State {
    fn join_mode(&self) -> JoinMode {
        if self.locked {
            JoinMode::Locked
        } else if self.ask_first {
            JoinMode::Approval
        } else {
            JoinMode::Open
        }
    }

    fn policy(&self) -> PolicyState {
        PolicyState {
            join_mode: self.join_mode(),
            active_sessions: usize::from(self.mirroring),
            comet_cast_presenting: self.refusing,
        }
    }
}

struct Shared {
    tx: Mutex<Sender<ReceiveEvent>>,
    state: Mutex<State>,
    cond: Condvar,
    config: AirplayConfig,
    supervisor_options: SupervisorOptions,
    request_timeout: Duration,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    fn send(&self, ev: ReceiveEvent) {
        let _ = lock(&self.tx).send(ev);
    }

    /// Tell the app to show the PIN and remember that a dialog is open.
    fn show_pin(&self, pin: &str) {
        lock(&self.state).pin_open = true;
        self.send(ReceiveEvent::Pin {
            pin: pin.to_owned(),
        });
    }

    /// Close request and PIN dialogs, if one was shown.
    fn close_dialogs(&self) {
        let was_open = std::mem::take(&mut lock(&self.state).pin_open);
        if was_open {
            self.send(ReceiveEvent::PinDone);
        }
    }
}

/// The confirmation hook of the supervisor: admission by join mode.
struct Hook(Arc<Shared>);

impl ConfirmHook for Hook {
    fn confirm(&self, request: &ConfirmRequest) -> Decision {
        let sh = &self.0;
        let mut st = lock(&sh.state);
        match admit_new_session(st.policy()) {
            Admission::Reject(_) => Decision::Deny,
            Admission::Allow => {
                drop(st);
                sh.show_pin(&request.pin);
                Decision::Accept
            }
            Admission::Ask => {
                st.waiting = true;
                st.answer = None;
                sh.send(ReceiveEvent::Request);
                let deadline = Instant::now() + sh.request_timeout;
                let allowed = loop {
                    if let Some(a) = st.answer.take() {
                        break a;
                    }
                    let now = Instant::now();
                    if st.stopping || now >= deadline {
                        break false;
                    }
                    st = sh
                        .cond
                        .wait_timeout(st, deadline - now)
                        .unwrap_or_else(|p| p.into_inner())
                        .0;
                };
                st.waiting = false;
                drop(st);
                if allowed {
                    sh.show_pin(&request.pin);
                    Decision::Accept
                } else {
                    // The request dialog is still open on the screen: close it.
                    sh.send(ReceiveEvent::PinDone);
                    Decision::Deny
                }
            }
        }
    }
}

/// The failure code for a message of the supervisor's `Failed` event.
fn failure_code(msg: &str) -> &'static str {
    if msg.starts_with("cannot start") {
        "not_installed"
    } else {
        "failed"
    }
}

/// The code for an error line of UxPlay that makes it unusable, if it is one.
fn error_line_code(line: &str) -> Option<&'static str> {
    if line.contains("gstreamer plugin") {
        Some("gstreamer_missing")
    } else if line.contains("No DNS-SD Server") {
        Some("no_dnssd")
    } else {
        None
    }
}

fn pump(sh: &Arc<Shared>, rx: &Receiver<AirplayEvent>) {
    for ev in rx {
        match ev {
            AirplayEvent::ServerStarted => sh.send(ReceiveEvent::Advertising),
            AirplayEvent::MirroringChanged { active } => mirroring_changed(sh, active),
            AirplayEvent::ClientDisconnected | AirplayEvent::Denied => sh.close_dialogs(),
            AirplayEvent::Failed(msg) => sh.send(ReceiveEvent::Failed {
                code: failure_code(&msg).to_owned(),
            }),
            AirplayEvent::Error(m) => {
                if let Some(code) = error_line_code(&m) {
                    sh.send(ReceiveEvent::Failed {
                        code: code.to_owned(),
                    });
                }
            }
            AirplayEvent::Stopped => break,
            _ => {}
        }
    }
}

fn mirroring_changed(sh: &Arc<Shared>, active: bool) {
    let stop = {
        let mut st = lock(&sh.state);
        st.mirroring = active;
        if let Some(s) = &st.supervisor {
            s.set_session_active(active);
        }
        if !active && st.locked && !st.stopping {
            // Locked and the last picture is gone: the service stops now (service_action).
            st.supervisor.take()
        } else {
            None
        }
    };
    if active {
        sh.close_dialogs();
    }
    sh.send(ReceiveEvent::Mirroring { active });
    if let Some(mut s) = stop {
        s.stop();
        sh.send(ReceiveEvent::NotAnnouncing);
    }
}

/// The display's AirPlay receiver. Dropping it stops UxPlay.
pub struct AirplayReceiver {
    shared: Arc<Shared>,
}

impl AirplayReceiver {
    /// Start UxPlay with `options` and report what happens on the returned channel.
    pub fn start(
        mut options: ReceiveOptions,
    ) -> Result<(Self, Receiver<ReceiveEvent>), ConfigError> {
        options.config.debug_log = true;
        options.config.build_args()?;
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            tx: Mutex::new(tx),
            state: Mutex::new(State {
                ask_first: options.ask_first,
                locked: false,
                refusing: false,
                mirroring: false,
                pin_open: false,
                waiting: false,
                answer: None,
                stopping: false,
                supervisor: None,
            }),
            cond: Condvar::new(),
            config: options.config,
            supervisor_options: options.supervisor,
            request_timeout: options.request_timeout,
        });
        let me = Self { shared };
        me.spawn_supervisor();
        Ok((me, rx))
    }

    fn spawn_supervisor(&self) {
        let sh = &self.shared;
        let mut opts = sh.supervisor_options.clone();
        opts.confirm = Some(Arc::new(Hook(Arc::clone(sh))));
        match Supervisor::start(sh.config.clone(), opts) {
            Ok((sup, rx)) => {
                lock(&sh.state).supervisor = Some(sup);
                let sh = Arc::clone(sh);
                let spawned = thread::Builder::new()
                    .name("cc-airplay-receive".into())
                    .spawn(move || pump(&sh, &rx));
                if spawned.is_err() {
                    self.shared.send(ReceiveEvent::Failed {
                        code: "failed".into(),
                    });
                }
            }
            Err(_) => sh.send(ReceiveEvent::Failed {
                code: "bad_config".into(),
            }),
        }
    }

    /// Change the join mode between Open (`false`) and Approval on (`true`).
    pub fn set_ask_first(&self, ask: bool) {
        lock(&self.shared.state).ask_first = ask;
    }

    /// Locked (`true`) or not. Locked refuses new devices and stops the service as soon as no
    /// mirroring runs (at once if none does; a running one stays until it ends); unlocking
    /// starts the service again.
    pub fn set_locked(&self, locked: bool) {
        let sh = &self.shared;
        let (stop, start) = {
            let mut st = lock(&sh.state);
            if st.stopping || st.locked == locked {
                return;
            }
            st.locked = locked;
            if locked {
                if st.waiting {
                    st.answer = Some(false);
                    sh.cond.notify_all();
                }
                let stop = if st.mirroring {
                    None
                } else {
                    st.supervisor.take()
                };
                (stop, false)
            } else {
                (None, st.supervisor.is_none())
            }
        };
        if let Some(mut s) = stop {
            s.stop();
            sh.close_dialogs();
            sh.send(ReceiveEvent::NotAnnouncing);
        }
        if start {
            self.spawn_supervisor();
        }
    }

    /// A Comet Cast presenter presents (`true`): every new pairing is refused without a request
    /// (decision N88). A running mirroring is not touched here, see [`Self::end_session`].
    pub fn set_refusing(&self, refusing: bool) {
        let sh = &self.shared;
        let mut st = lock(&sh.state);
        st.refusing = refusing;
        if refusing && st.waiting {
            st.answer = Some(false);
            sh.cond.notify_all();
        }
    }

    /// Answer the open [`ReceiveEvent::Request`]. False = there was none.
    pub fn answer(&self, allow: bool) -> bool {
        let mut st = lock(&self.shared.state);
        if !st.waiting || st.answer.is_some() {
            return false;
        }
        st.answer = Some(allow);
        self.shared.cond.notify_all();
        true
    }

    /// Is a mirroring running right now?
    pub fn mirroring(&self) -> bool {
        lock(&self.shared.state).mirroring
    }

    /// End the running mirroring (UxPlay is restarted, which drops the device). Without a
    /// mirroring this only restarts UxPlay.
    pub fn end_session(&self) {
        if let Some(s) = &lock(&self.shared.state).supervisor {
            s.restart();
        }
    }

    /// Stop UxPlay and end the event stream with [`ReceiveEvent::Stopped`]. Safe to call twice.
    pub fn stop(&self) {
        let sup = {
            let mut st = lock(&self.shared.state);
            if st.stopping {
                return;
            }
            st.stopping = true;
            self.shared.cond.notify_all();
            st.supervisor.take()
        };
        if let Some(mut s) = sup {
            s.stop();
        }
        self.shared.close_dialogs();
        self.shared.send(ReceiveEvent::Stopped);
    }
}

impl Drop for AirplayReceiver {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The `uxplay` program the app would start: next to the running executable first (where the
/// packages put it), then `PATH`. `None` when there is none.
pub fn locate_uxplay() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "uxplay.exe"
    } else {
        "uxplay"
    };
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let p = dir.join(name);
        if is_program(&p) {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| is_program(p))
}

fn is_program(p: &Path) -> bool {
    let Ok(m) = std::fs::metadata(p) else {
        return false;
    };
    if !m.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Largest program file looked into by [`has_mirroring_signal`].
const MAX_PROGRAM_BYTES: u64 = 256 * 1024 * 1024;

/// Does this UxPlay program print the lines the mirroring signal needs? Looks for the text of
/// the mirror line ([`MIRRORING_STARTED_LINE`]) and of the connection count in the program file
/// (their format strings are stored there as plain text). A UxPlay build without them gives no
/// trusted signal, and AirPlay is then not offered (`airplay_receive_available()`).
pub fn has_mirroring_signal(program: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(program) else {
        return false;
    };
    if meta.len() > MAX_PROGRAM_BYTES {
        return false;
    }
    let Ok(bytes) = std::fs::read(program) else {
        return false;
    };
    contains(&bytes, MIRRORING_STARTED_LINE.as_bytes()) && contains(&bytes, b"Open connections: ")
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_check_reads_the_program_text() {
        let dir = std::env::temp_dir().join(format!("cc-airplay-sig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good");
        std::fs::write(
            &good,
            b"\x7fELF....raop_rtp_mirror starting mirroring\0Open connections: %i\0..",
        )
        .unwrap();
        let half = dir.join("half");
        std::fs::write(&half, b"raop_rtp_mirror starting mirroring\0").unwrap();
        let none = dir.join("none");
        std::fs::write(&none, b"some other program").unwrap();
        assert!(has_mirroring_signal(&good));
        assert!(!has_mirroring_signal(&half));
        assert!(!has_mirroring_signal(&none));
        assert!(!has_mirroring_signal(&dir.join("missing")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn failure_codes() {
        assert_eq!(failure_code("cannot start uxplay: x"), "not_installed");
        assert_eq!(failure_code("UxPlay failed 3 times in a row"), "failed");
        assert_eq!(
            error_line_code("Required gstreamer plugin 'app' not found"),
            Some("gstreamer_missing")
        );
        assert_eq!(
            error_line_code("*** ERROR: No DNS-SD Server found (x)"),
            Some("no_dnssd")
        );
        assert_eq!(error_line_code("*** ERROR: other"), None);
    }
}
