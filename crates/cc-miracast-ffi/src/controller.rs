// SPDX-License-Identifier: GPL-3.0-or-later
//! The state the Flutter app polls: lifecycle (start and stop run on threads of their own so
//! the UI thread never waits for Windows), the pending consent request, the connected device,
//! errors and counters. Platform independent; tested on Linux with a fake receiver.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cc_miracast::{
    ConnectionId, FrameCounters, Issue, MiracastConfig, MiracastError, MiracastEvent,
    MiracastStatus, PairingPolicy, Probe, ReceiverBackend, diagnose_start_error, diagnose_status,
    status_after,
};
use serde::Serialize;

/// Creates the receiver (on Windows: `MiracastReceiver::start`). Runs on its own thread.
pub type Starter = Box<
    dyn FnOnce(
            MiracastConfig,
        )
            -> Result<(Box<dyn ReceiverBackend>, Receiver<MiracastEvent>), MiracastError>
        + Send,
>;

/// Where the receiver is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Idle,
    Starting,
    Running,
    Stopping,
}

/// Why `start` or another call was refused (the C layer turns these into numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallError {
    /// A receiver is starting, running or stopping.
    Busy,
    /// No receiver is running.
    NotRunning,
    /// The id does not match the waiting request.
    NoSuchRequest,
    /// Windows refused the answer.
    Failed,
}

struct Pending {
    id: ConnectionId,
    name: String,
    pin: Option<String>,
}

struct Inner {
    lifecycle: Lifecycle,
    receiver: Option<Box<dyn ReceiverBackend>>,
    events: Option<Receiver<MiracastEvent>>,
    status: MiracastStatus,
    probe: Probe,
    /// Why the last start failed; stays until the next start.
    start_issue: Option<Issue>,
    pending: Option<Pending>,
    connected: Option<String>,
    last_error: Option<String>,
    rejected: u32,
    failed: u32,
    generation: u64,
    /// `stop` arrived while the receiver was still starting.
    stop_requested: bool,
    /// Sequence number of the newest picture handed to the UI.
    last_frame_seq: u64,
}

/// The one receiver of the process.
#[derive(Clone)]
pub struct Controller {
    inner: Arc<Mutex<Inner>>,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON the UI reads (see `docs/miracast.md`, "C interface"). Field names are part of the
/// interface with `apps/flutter/lib/core/miracast.dart`.
#[derive(Debug, Serialize)]
struct StateJson<'a> {
    /// Version of this JSON layout.
    v: u32,
    status: &'a str,
    issue: Option<&'a str>,
    generation: u64,
    pending: Option<PendingJson<'a>>,
    connected: Option<&'a str>,
    last_error: Option<&'a str>,
    rejected: u32,
    failed: u32,
    frames: FramesJson,
    feature: &'a str,
    adapter: &'a str,
}

#[derive(Debug, Serialize)]
struct PendingJson<'a> {
    id: u64,
    name: &'a str,
    pin: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct FramesJson {
    published: u64,
    taken: u64,
    dropped: u64,
    failed: u64,
    width: u32,
    height: u32,
}

impl From<FrameCounters> for FramesJson {
    fn from(c: FrameCounters) -> Self {
        Self {
            published: c.published,
            taken: c.taken,
            dropped: c.dropped,
            failed: c.failed,
            width: c.width,
            height: c.height,
        }
    }
}

/// Name of a status in the JSON.
#[must_use]
pub fn status_name(s: MiracastStatus) -> &'static str {
    match s {
        MiracastStatus::Off => "off",
        MiracastStatus::Starting => "starting",
        MiracastStatus::Listening => "listening",
        MiracastStatus::ConnectionPending => "connection_pending",
        MiracastStatus::Connected => "connected",
        MiracastStatus::Unavailable => "unavailable",
        MiracastStatus::DisabledByPolicy => "disabled_by_policy",
        MiracastStatus::Unsupported => "unsupported",
        MiracastStatus::Failed => "failed",
    }
}

/// Name of an issue in the JSON.
#[must_use]
pub fn issue_name(i: Issue) -> &'static str {
    match i {
        Issue::NotWindows => "not_windows",
        Issue::FeatureMissing => "feature_missing",
        Issue::AdapterUnsupported => "adapter_unsupported",
        Issue::AdapterNotOptimized => "adapter_not_optimized",
        Issue::DisabledByPolicy => "disabled_by_policy",
        Issue::TemporarilyUnavailable => "temporarily_unavailable",
        Issue::AccessDenied => "access_denied",
        Issue::InvalidSettings => "invalid_settings",
        Issue::Other => "other",
    }
}

/// Parse the pairing name of the start JSON. Unknown names are refused (not defaulted to
/// something weaker).
#[must_use]
pub fn parse_pairing(s: &str) -> Option<PairingPolicy> {
    match s {
        "confirm" => Some(PairingPolicy::ConfirmConnection),
        "pin_if_requested" => Some(PairingPolicy::PinIfRequested),
        "pin_required" => Some(PairingPolicy::RequirePin),
        _ => None,
    }
}

fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    fn apply(&mut self, ev: &MiracastEvent) {
        self.status = status_after(self.status, ev);
        match ev {
            MiracastEvent::ConnectionRequested {
                id,
                device_name,
                pin,
            } => {
                self.pending = Some(Pending {
                    id: *id,
                    name: device_name.clone(),
                    pin: pin.clone(),
                });
            }
            MiracastEvent::Connected { id, device_name } => {
                if self.pending.as_ref().is_some_and(|p| p.id == *id) {
                    self.pending = None;
                }
                self.connected = Some(device_name.clone());
            }
            MiracastEvent::Disconnected { id } => {
                if self.pending.as_ref().is_some_and(|p| p.id == *id) {
                    self.pending = None;
                }
                // Only one session exists, so any disconnect of an accepted one ends it.
                if self.pending.is_none() {
                    self.connected = None;
                }
            }
            MiracastEvent::ConnectionRejected { .. } => {
                self.rejected = self.rejected.saturating_add(1)
            }
            MiracastEvent::ConnectionFailed { message, .. } => {
                self.failed = self.failed.saturating_add(1);
                self.last_error = Some(message.clone());
            }
            MiracastEvent::Error(message) => {
                self.pending = None;
                self.last_error = Some(message.clone());
            }
            MiracastEvent::StatusChanged(
                MiracastStatus::Off
                | MiracastStatus::Unsupported
                | MiracastStatus::DisabledByPolicy,
            ) => self.pending = None,
            MiracastEvent::StatusChanged(_) => {}
            MiracastEvent::Listening | MiracastEvent::StreamReady { .. } => {}
        }
    }

    fn drain(&mut self) {
        let Some(rx) = &self.events else { return };
        let events: Vec<MiracastEvent> = rx.try_iter().collect();
        for ev in &events {
            self.apply(ev);
        }
    }
}

impl Controller {
    /// A controller with no receiver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                lifecycle: Lifecycle::Idle,
                receiver: None,
                events: None,
                status: MiracastStatus::Off,
                probe: Probe::default(),
                start_issue: None,
                pending: None,
                connected: None,
                last_error: None,
                rejected: 0,
                failed: 0,
                generation: 0,
                stop_requested: false,
                last_frame_seq: 0,
            })),
        }
    }

    /// Start the receiver on a thread of its own. Returns at once; poll [`Controller::state_json`].
    /// `probe` is the system check made before (feature, adapter) and used for the hints.
    pub fn start(
        &self,
        config: MiracastConfig,
        probe: Probe,
        starter: Starter,
    ) -> Result<(), CallError> {
        {
            let mut g = lock(&self.inner);
            if g.lifecycle != Lifecycle::Idle {
                return Err(CallError::Busy);
            }
            g.lifecycle = Lifecycle::Starting;
            g.generation += 1;
            g.status = MiracastStatus::Starting;
            g.probe = probe;
            g.start_issue = None;
            g.pending = None;
            g.connected = None;
            g.last_error = None;
            g.rejected = 0;
            g.failed = 0;
            g.stop_requested = false;
            g.last_frame_seq = 0;
        }
        let inner = Arc::clone(&self.inner);
        std::thread::Builder::new()
            .name("cc-miracast-start".into())
            .spawn(move || {
                let result = starter(config);
                let mut g = lock(&inner);
                match result {
                    Ok((receiver, events)) => {
                        if g.stop_requested {
                            // Stopped while starting: drop the receiver here, off the lock.
                            g.lifecycle = Lifecycle::Stopping;
                            drop(g);
                            drop(receiver);
                            let mut g = lock(&inner);
                            g.lifecycle = Lifecycle::Idle;
                            g.status = MiracastStatus::Off;
                            return;
                        }
                        g.receiver = Some(receiver);
                        g.events = Some(events);
                        g.lifecycle = Lifecycle::Running;
                        g.drain();
                    }
                    Err(e) => {
                        g.start_issue = Some(diagnose_start_error(&e, &g.probe));
                        g.status = match e {
                            MiracastError::Unsupported => MiracastStatus::Unsupported,
                            _ => MiracastStatus::Failed,
                        };
                        g.last_error = Some(e.to_string());
                        g.lifecycle = Lifecycle::Idle;
                    }
                }
            })
            .map(|_| ())
            .map_err(|_| {
                let mut g = lock(&self.inner);
                g.lifecycle = Lifecycle::Idle;
                g.status = MiracastStatus::Failed;
                g.last_error = Some("could not start a thread".into());
                CallError::Failed
            })
    }

    /// Stop the receiver on a thread of its own (dropping it disconnects everything and puts
    /// the Windows settings back). Safe to call when nothing runs.
    pub fn stop(&self) {
        let (receiver, events) = {
            let mut g = lock(&self.inner);
            match g.lifecycle {
                Lifecycle::Idle => {
                    // Nothing runs; forget the outcome of a failed start.
                    g.status = MiracastStatus::Off;
                    g.start_issue = None;
                    g.last_error = None;
                    return;
                }
                Lifecycle::Stopping => return,
                Lifecycle::Starting => {
                    g.stop_requested = true;
                    return;
                }
                Lifecycle::Running => {}
            }
            g.lifecycle = Lifecycle::Stopping;
            g.pending = None;
            g.connected = None;
            (g.receiver.take(), g.events.take())
        };
        let inner = Arc::clone(&self.inner);
        let spawned = std::thread::Builder::new()
            .name("cc-miracast-stop".into())
            .spawn(move || {
                drop(events);
                drop(receiver);
                let mut g = lock(&inner);
                g.lifecycle = Lifecycle::Idle;
                g.status = MiracastStatus::Off;
            });
        if spawned.is_err() {
            // The closure (and with it the receiver) was dropped by the failed spawn.
            let mut g = lock(&self.inner);
            g.lifecycle = Lifecycle::Idle;
            g.status = MiracastStatus::Off;
        }
    }

    /// Answer the waiting request. The id must be the one shown in the state.
    pub fn respond(&self, id: u64, accept: bool) -> Result<(), CallError> {
        let mut g = lock(&self.inner);
        g.drain();
        if g.pending.as_ref().is_none_or(|p| p.id.0 != id) {
            return Err(CallError::NoSuchRequest);
        }
        let Some(r) = &g.receiver else {
            return Err(CallError::NotRunning);
        };
        let result = r.respond(ConnectionId(id), accept);
        g.pending = None;
        g.drain();
        result.map_err(|_| CallError::Failed)
    }

    /// End the active connection and keep listening.
    pub fn disconnect(&self) -> Result<(), CallError> {
        let mut g = lock(&self.inner);
        let Some(r) = &g.receiver else {
            return Err(CallError::NotRunning);
        };
        r.disconnect();
        g.drain();
        Ok(())
    }

    /// Largest picture size handed out.
    pub fn set_max_frame_size(&self, w: u32, h: u32) {
        if let Some(r) = &lock(&self.inner).receiver {
            r.set_max_frame_size(w, h);
        }
    }

    /// Mute or unmute the audio.
    pub fn set_muted(&self, muted: bool) {
        if let Some(r) = &lock(&self.inner).receiver {
            r.set_muted(muted);
        }
    }

    /// Size of the pending picture, if any.
    #[must_use]
    pub fn peek_frame_size(&self) -> Option<(u32, u32)> {
        lock(&self.inner)
            .receiver
            .as_ref()
            .and_then(|r| r.peek_frame_size())
    }

    /// Take the newest picture and copy it into `buf`. Returns the size and sequence number,
    /// or `None` when there is none. A picture that does not fit is NOT taken.
    pub fn take_frame_into(&self, buf: &mut [u8]) -> FrameTake {
        let mut g = lock(&self.inner);
        let Some(r) = &g.receiver else {
            return FrameTake::None;
        };
        let Some((w, h)) = r.peek_frame_size() else {
            return FrameTake::None;
        };
        let need = (w as usize) * (h as usize) * 4;
        if need > buf.len() {
            return FrameTake::TooSmall {
                width: w,
                height: h,
            };
        }
        let Some(f) = r.take_frame() else {
            return FrameTake::None;
        };
        if f.bgra.len() > buf.len() {
            // A bigger picture arrived between the size check and the take.
            return FrameTake::TooSmall {
                width: f.width,
                height: f.height,
            };
        }
        buf[..f.bgra.len()].copy_from_slice(&f.bgra);
        g.last_frame_seq = f.seq;
        FrameTake::Frame {
            width: f.width,
            height: f.height,
            seq: f.seq,
        }
    }

    /// The state as JSON text.
    #[must_use]
    pub fn state_json(&self) -> String {
        let mut g = lock(&self.inner);
        g.drain();
        let status = if g.lifecycle == Lifecycle::Stopping {
            "stopping"
        } else {
            status_name(g.status)
        };
        let counters = g
            .receiver
            .as_ref()
            .map(|r| r.frame_counters())
            .unwrap_or_default();
        let pending = g.pending.as_ref().map(|p| PendingJson {
            id: p.id.0,
            name: &p.name,
            pin: p.pin.as_deref(),
        });
        let feature = match g.probe.feature {
            cc_miracast::FeatureState::Installed => "installed",
            cc_miracast::FeatureState::NotInstalled => "not_installed",
            cc_miracast::FeatureState::Unknown => "unknown",
        };
        let adapter = match g.probe.adapter {
            cc_miracast::AdapterSupport::Undetermined => "undetermined",
            cc_miracast::AdapterSupport::NotSupported => "not_supported",
            cc_miracast::AdapterSupport::NotOptimized => "not_optimized",
            cc_miracast::AdapterSupport::Supported => "supported",
        };
        let issue = g.start_issue.or_else(|| {
            if g.lifecycle == Lifecycle::Running {
                diagnose_status(g.status, &g.probe)
            } else {
                None
            }
        });
        let json = StateJson {
            v: 1,
            status,
            issue: issue.map(issue_name),
            generation: g.generation,
            pending,
            connected: g.connected.as_deref(),
            last_error: g.last_error.as_deref(),
            rejected: g.rejected,
            failed: g.failed,
            frames: counters.into(),
            feature,
            adapter,
        };
        serde_json::to_string(&json).unwrap_or_else(|_| "{\"v\":1,\"status\":\"failed\"}".into())
    }
}

/// Result of [`Controller::take_frame_into`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameTake {
    /// No new picture.
    None,
    /// The buffer is too small for the pending picture; it was not taken.
    TooSmall {
        /// Width of the picture.
        width: u32,
        /// Height of the picture.
        height: u32,
    },
    /// A picture was copied into the buffer.
    Frame {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
        /// Sequence number.
        seq: u64,
    },
}

/// The system check as JSON (for the screen that shows hints before the receiver starts).
#[must_use]
pub fn probe_json(p: &Probe) -> String {
    #[derive(Serialize)]
    struct ProbeJson<'a> {
        v: u32,
        feature: &'a str,
        adapter: &'a str,
        receiver_available: bool,
        issue: Option<&'a str>,
    }
    let feature = match p.feature {
        cc_miracast::FeatureState::Installed => "installed",
        cc_miracast::FeatureState::NotInstalled => "not_installed",
        cc_miracast::FeatureState::Unknown => "unknown",
    };
    let adapter = match p.adapter {
        cc_miracast::AdapterSupport::Undetermined => "undetermined",
        cc_miracast::AdapterSupport::NotSupported => "not_supported",
        cc_miracast::AdapterSupport::NotOptimized => "not_optimized",
        cc_miracast::AdapterSupport::Supported => "supported",
    };
    // A hint before starting only when the probe itself is clear: Windows said the adapter
    // cannot do it, or it could not even create the receiver object.
    let issue = if cfg!(not(windows)) {
        Some("not_windows")
    } else if p.adapter == cc_miracast::AdapterSupport::NotSupported {
        Some("adapter_unsupported")
    } else if !p.receiver_available {
        Some("feature_missing")
    } else if p.adapter == cc_miracast::AdapterSupport::NotOptimized {
        Some("adapter_not_optimized")
    } else {
        None
    };
    serde_json::to_string(&ProbeJson {
        v: 1,
        feature,
        adapter,
        receiver_available: p.receiver_available,
        issue,
    })
    .unwrap_or_else(|_| "{\"v\":1}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_miracast::VideoFrame;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::{Sender, channel};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct FakeState {
        responses: Mutex<Vec<(u64, bool)>>,
        frame: Mutex<Option<VideoFrame>>,
        disconnects: AtomicU64,
        dropped: AtomicBool,
        muted: AtomicBool,
        fail_respond: AtomicBool,
    }
    struct Fake(Arc<FakeState>);
    impl ReceiverBackend for Fake {
        fn status(&self) -> MiracastStatus {
            MiracastStatus::Listening
        }
        fn respond(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
            self.0.responses.lock().unwrap().push((id.0, accept));
            if self.0.fail_respond.load(Ordering::SeqCst) {
                Err(MiracastError::UnknownConnection)
            } else {
                Ok(())
            }
        }
        fn disconnect(&self) {
            self.0.disconnects.fetch_add(1, Ordering::SeqCst);
        }
        fn take_frame(&self) -> Option<VideoFrame> {
            self.0.frame.lock().unwrap().take()
        }
        fn peek_frame_size(&self) -> Option<(u32, u32)> {
            self.0
                .frame
                .lock()
                .unwrap()
                .as_ref()
                .map(|f| (f.width, f.height))
        }
        fn frame_counters(&self) -> FrameCounters {
            FrameCounters::default()
        }
        fn set_max_frame_size(&self, _: u32, _: u32) {}
        fn set_muted(&self, m: bool) {
            self.0.muted.store(m, Ordering::SeqCst);
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            self.0.dropped.store(true, Ordering::SeqCst);
        }
    }

    fn start_fake(c: &Controller) -> (Arc<FakeState>, Sender<MiracastEvent>) {
        let st = Arc::new(FakeState::default());
        let (tx, rx) = channel();
        // The real backend emits `Listening` before `start` returns.
        tx.send(MiracastEvent::Listening).unwrap();
        let st2 = Arc::clone(&st);
        c.start(
            MiracastConfig::default(),
            Probe::default(),
            Box::new(move |_| Ok((Box::new(Fake(st2)) as Box<dyn ReceiverBackend>, rx))),
        )
        .unwrap();
        wait_for(c, |j| j["status"] != "starting");
        (st, tx)
    }

    fn json(c: &Controller) -> serde_json::Value {
        serde_json::from_str(&c.state_json()).unwrap()
    }

    fn wait_for(c: &Controller, ok: impl Fn(&serde_json::Value) -> bool) {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(5) {
            if ok(&json(c)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timeout, state: {}", c.state_json());
    }

    #[test]
    fn idle_state_is_off() {
        let j = json(&Controller::new());
        assert_eq!(j["v"], 1);
        assert_eq!(j["status"], "off");
        assert!(j["pending"].is_null());
        assert!(j["issue"].is_null());
    }

    #[test]
    fn start_failure_reports_status_and_issue() {
        let c = Controller::new();
        c.start(
            MiracastConfig::default(),
            Probe::default(),
            Box::new(|_| Err(MiracastError::Unsupported)),
        )
        .unwrap();
        wait_for(&c, |j| j["status"] == "unsupported");
        let j = json(&c);
        assert!(j["last_error"].is_string());
        // On Linux the issue is "not_windows"; on Windows an adapter or feature issue.
        assert!(j["issue"].is_string());
        // Stop forgets the failed start; a new start is possible again.
        c.stop();
        assert_eq!(json(&c)["status"], "off");
        assert!(json(&c)["issue"].is_null());
        let (_st, _tx) = start_fake(&c);
        assert!(
            json(&c)["last_error"].is_null(),
            "a new start clears the old error"
        );
    }

    #[test]
    fn second_start_is_busy() {
        let c = Controller::new();
        let (_st, _tx) = start_fake(&c);
        let r = c.start(
            MiracastConfig::default(),
            Probe::default(),
            Box::new(|_| Err(MiracastError::Unsupported)),
        );
        assert_eq!(r, Err(CallError::Busy));
    }

    #[test]
    fn consent_flow_shows_name_and_pin_then_connects() {
        let c = Controller::new();
        let (st, tx) = start_fake(&c);
        tx.send(MiracastEvent::Listening).unwrap();
        tx.send(MiracastEvent::ConnectionRequested {
            id: ConnectionId(7),
            device_name: "Pixel".into(),
            pin: Some("4711".into()),
        })
        .unwrap();
        wait_for(&c, |j| j["status"] == "connection_pending");
        let j = json(&c);
        assert_eq!(j["pending"]["id"], 7);
        assert_eq!(j["pending"]["name"], "Pixel");
        assert_eq!(j["pending"]["pin"], "4711");
        // Wrong id is refused and nothing is sent to the receiver.
        assert_eq!(c.respond(8, true), Err(CallError::NoSuchRequest));
        assert!(st.responses.lock().unwrap().is_empty());
        c.respond(7, true).unwrap();
        assert_eq!(st.responses.lock().unwrap().as_slice(), &[(7, true)]);
        assert!(json(&c)["pending"].is_null());
        tx.send(MiracastEvent::Connected {
            id: ConnectionId(7),
            device_name: "Pixel".into(),
        })
        .unwrap();
        wait_for(&c, |j| j["status"] == "connected");
        assert_eq!(json(&c)["connected"], "Pixel");
        c.disconnect().unwrap();
        assert_eq!(st.disconnects.load(Ordering::SeqCst), 1);
        tx.send(MiracastEvent::Disconnected {
            id: ConnectionId(7),
        })
        .unwrap();
        wait_for(&c, |j| j["status"] == "listening");
        assert!(json(&c)["connected"].is_null());
    }

    #[test]
    fn timeout_of_the_request_clears_it() {
        let c = Controller::new();
        let (_st, tx) = start_fake(&c);
        tx.send(MiracastEvent::ConnectionRequested {
            id: ConnectionId(1),
            device_name: "A".into(),
            pin: None,
        })
        .unwrap();
        wait_for(&c, |j| j["pending"].is_object());
        // The engine times a request out by reporting Disconnected.
        tx.send(MiracastEvent::Disconnected {
            id: ConnectionId(1),
        })
        .unwrap();
        wait_for(&c, |j| j["pending"].is_null());
    }

    #[test]
    fn rejections_and_failures_are_counted_without_changing_status() {
        let c = Controller::new();
        let (_st, tx) = start_fake(&c);
        tx.send(MiracastEvent::Listening).unwrap();
        tx.send(MiracastEvent::ConnectionRejected {
            reason: cc_miracast::RejectReason::Busy,
            device_name: Some("B".into()),
        })
        .unwrap();
        tx.send(MiracastEvent::ConnectionFailed {
            id: None,
            message: "x".into(),
        })
        .unwrap();
        wait_for(&c, |j| j["failed"] == 1);
        let j = json(&c);
        assert_eq!(j["rejected"], 1);
        assert_eq!(j["status"], "listening");
        assert_eq!(j["last_error"], "x");
    }

    #[test]
    fn respond_failure_is_reported_and_request_cleared() {
        let c = Controller::new();
        let (st, tx) = start_fake(&c);
        tx.send(MiracastEvent::ConnectionRequested {
            id: ConnectionId(3),
            device_name: "A".into(),
            pin: None,
        })
        .unwrap();
        wait_for(&c, |j| j["pending"].is_object());
        st.fail_respond.store(true, Ordering::SeqCst);
        assert_eq!(c.respond(3, true), Err(CallError::Failed));
        assert!(json(&c)["pending"].is_null());
    }

    #[test]
    fn frames_are_copied_and_too_small_buffers_keep_the_picture() {
        let c = Controller::new();
        let (st, _tx) = start_fake(&c);
        let mut small = [0u8; 8];
        assert_eq!(c.take_frame_into(&mut small), FrameTake::None);
        *st.frame.lock().unwrap() = Some(VideoFrame {
            seq: 5,
            width: 2,
            height: 2,
            bgra: vec![9; 16],
        });
        assert_eq!(
            c.take_frame_into(&mut small),
            FrameTake::TooSmall {
                width: 2,
                height: 2
            }
        );
        assert!(st.frame.lock().unwrap().is_some(), "picture stays");
        let mut big = [0u8; 32];
        assert_eq!(
            c.take_frame_into(&mut big),
            FrameTake::Frame {
                width: 2,
                height: 2,
                seq: 5
            }
        );
        assert_eq!(&big[..16], &[9; 16]);
        assert_eq!(c.take_frame_into(&mut big), FrameTake::None);
    }

    #[test]
    fn stop_drops_the_receiver_and_allows_a_new_start() {
        let c = Controller::new();
        let (st, _tx) = start_fake(&c);
        c.stop();
        wait_for(&c, |j| j["status"] == "off");
        assert!(st.dropped.load(Ordering::SeqCst));
        assert_eq!(c.disconnect(), Err(CallError::NotRunning));
        let (_st2, _tx2) = start_fake(&c);
        // stop without a receiver is harmless
        c.stop();
        c.stop();
        wait_for(&c, |j| j["status"] == "off");
    }

    #[test]
    fn stop_during_start_ends_idle() {
        let c = Controller::new();
        let st = Arc::new(FakeState::default());
        let (_tx, rx) = channel();
        let st2 = Arc::clone(&st);
        c.start(
            MiracastConfig::default(),
            Probe::default(),
            Box::new(move |_| {
                std::thread::sleep(Duration::from_millis(150));
                Ok((Box::new(Fake(st2)) as Box<dyn ReceiverBackend>, rx))
            }),
        )
        .unwrap();
        c.stop();
        wait_for(&c, |j| j["status"] == "off");
        assert!(st.dropped.load(Ordering::SeqCst));
        // And a start works afterwards.
        let (_s, _t) = start_fake(&c);
    }

    #[test]
    fn pairing_names() {
        assert_eq!(
            parse_pairing("confirm"),
            Some(PairingPolicy::ConfirmConnection)
        );
        assert_eq!(
            parse_pairing("pin_required"),
            Some(PairingPolicy::RequirePin)
        );
        assert_eq!(
            parse_pairing("pin_if_requested"),
            Some(PairingPolicy::PinIfRequested)
        );
        assert_eq!(parse_pairing("none"), None);
        assert_eq!(parse_pairing(""), None);
    }

    #[test]
    fn probe_json_hints() {
        let p = Probe {
            adapter: cc_miracast::AdapterSupport::NotSupported,
            ..Probe::default()
        };
        let v: serde_json::Value = serde_json::from_str(&probe_json(&p)).unwrap();
        // not_windows on Linux, adapter_unsupported on Windows
        assert!(v["issue"].is_string());
        assert_eq!(v["v"], 1);
    }
}
