// SPDX-License-Identifier: GPL-3.0-or-later
//! The safe core of the C ABI: one process-wide sender (an extension process has exactly one
//! broadcast), started with [`start`], fed with [`push_frame`], ended with [`stop`].
//!
//! Threads: [`start`] builds a small tokio runtime (2 worker threads, 512 KiB stacks, to keep
//! the extension below its memory limit; UNCERTAIN: the numbers are not measured). The status
//! sink is called on a runtime thread; it must not block and must not call [`stop`].

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cc_proto::RoomCode;
use cc_sender::{
    ControlSender, DisplayMessage, ExternalInput, ExternalStats, JoinProgress, PushOutcome,
    SenderCore, SenderEvent, SenderOptions,
};
use cc_transport::{ConnectionState, TransportError};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::log::Log;

/// How long [`stop`] waits for the sender to close its sessions before it gives up.
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

/// Seconds between status events while streaming (the counters change all the time).
const STATS_INTERVAL: Duration = Duration::from_secs(1);

/// Where the broadcast is. The numbers are part of the C ABI (`include/cc_ios.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum State {
    /// Nothing runs.
    Idle = 0,
    /// Connecting to the relay (signalling server) and joining the room.
    Connecting = 1,
    /// In the room; the connection to the receiver is being negotiated.
    Waiting = 2,
    /// Media can flow. Push frames.
    Streaming = 3,
    /// The connection to the receiver is temporarily lost; it may recover.
    Reconnecting = 4,
    /// The receiver left. Nothing more is sent.
    Ended = 5,
    /// Failed; see [`Status::error`].
    Failed = 6,
    /// In a room that asks the host: waiting for the host's decision. [`Status::check`] and
    /// [`Status::timeout_secs`] say what the waiting page shows (native-apps.md 20.5, step 6).
    Pending = 7,
}

/// Why a start or the broadcast failed. The numbers are part of the C ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    /// No error.
    None = 0,
    /// The room code is not valid (length or characters).
    InvalidRoomCode = 1,
    /// The relay address cannot be used.
    InvalidRelay = 2,
    /// The relay knows no room with this code (typo, or the receiver stopped).
    RoomNotFound = 3,
    /// The relay cannot be reached or did not answer in time.
    ServerUnreachable = 4,
    /// The connection to the receiver failed or was closed.
    ConnectionLost = 5,
    /// The receiver left the room.
    PeerLeft = 6,
    /// Something else failed (WebRTC, protocol, runtime). Details are in the log file.
    Internal = 7,
    /// `start` was called while a broadcast runs.
    AlreadyRunning = 8,
    /// The host denied the join (`join_denied`).
    JoinDenied = 9,
    /// Nobody decided on the join in time (`approval_timeout`).
    ApprovalTimeout = 10,
    /// The room is locked (`room_locked`): a hand-over join is refused like any join.
    RoomLocked = 11,
}

/// What [`push_frame`] did with an access unit. The numbers are part of the C ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PushCode {
    /// Queued for sending.
    Queued = 0,
    /// The receiver is not connected (yet): dropped. A key frame was requested for later.
    NotStreaming = 1,
    /// The bounded queue was full: dropped; deltas are dropped until the next key frame.
    QueueFull = 2,
    /// A delta frame while a key frame is awaited: dropped.
    WaitingForKeyFrame = 3,
    /// A key frame without SPS/PPS and none known: dropped.
    MissingParameterSets = 4,
    /// Not valid H.264 (Annex B or AVCC), empty, or too large: dropped.
    Invalid = 5,
    /// No broadcast runs (never started, ended or failed).
    NoSender = 6,
}

impl From<PushOutcome> for PushCode {
    fn from(o: PushOutcome) -> Self {
        match o {
            PushOutcome::Queued => Self::Queued,
            PushOutcome::NotStreaming => Self::NotStreaming,
            PushOutcome::QueueFull => Self::QueueFull,
            PushOutcome::WaitingForKeyFrame => Self::WaitingForKeyFrame,
            PushOutcome::MissingParameterSets => Self::MissingParameterSets,
            PushOutcome::Invalid => Self::Invalid,
            PushOutcome::Closed => Self::NoSender,
        }
    }
}

/// One status report: the state plus the counters of the encoded input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Where the broadcast is.
    pub state: State,
    /// The reason, for [`State::Failed`] and [`State::Ended`]; [`ErrorCode::None`] otherwise.
    pub error: ErrorCode,
    /// Access units handed to the sender.
    pub pushed: u64,
    /// Access units written to the transport.
    pub sent: u64,
    /// Access units dropped (not streaming, queue full, waiting for a key frame, invalid).
    pub dropped: u64,
    /// Bytes written to the transport.
    pub bytes_sent: u64,
    /// Key frames written to the transport.
    pub keyframes_sent: u64,
    /// Key frame requests (receiver PLI/FIR, new peer, drop policy).
    pub keyframe_requests: u64,
    /// Two-digit check number of a join that waits for the host ([`State::Pending`]); 0 = none.
    pub check: u8,
    /// Seconds the host has to decide, with [`Status::check`]; 0 = none.
    pub timeout_secs: u32,
    /// The room name the display gave (empty = none), known once the join succeeded.
    pub room_name: String,
    /// The connection check code (`"123 456"`) of the broadcast, empty until connected.
    pub sas: String,
    /// Technical English text for the log; never shown to the person (the extension has its own
    /// translated messages, chosen by [`Status::error`]).
    pub message: String,
}

/// What the status reports besides the state: it only grows while a broadcast runs.
#[derive(Debug, Clone, Default)]
struct Info {
    check: u8,
    timeout_secs: u32,
    room_name: String,
    sas: String,
}

impl Status {
    fn new(
        state: State,
        error: ErrorCode,
        stats: &ExternalStats,
        info: &Info,
        message: &str,
    ) -> Self {
        Self {
            check: info.check,
            timeout_secs: info.timeout_secs,
            room_name: info.room_name.clone(),
            sas: info.sas.clone(),
            state,
            error,
            pushed: stats.pushed,
            sent: stats.sent,
            dropped: stats.dropped() + stats.invalid,
            bytes_sent: stats.bytes_sent,
            keyframes_sent: stats.keyframes_sent,
            keyframe_requests: stats.keyframe_requests,
            message: message.to_owned(),
        }
    }
}

/// Receiver of status reports. Called from a runtime thread.
pub type Sink = Arc<dyn Fn(&Status) + Send + Sync>;

/// What [`start`] needs.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// The room code the receiver shows.
    pub room_code: String,
    /// Relay (signalling server) as `host:port` or a `ws://` / `wss://` URL.
    pub relay_url: String,
    /// Directory for the log file (the App Group container). `None` = no log.
    pub log_dir: Option<PathBuf>,
    /// Development and tests only: bind the WebRTC UDP socket to this local address.
    pub udp_bind: Option<String>,
    /// The person's name, as shown on the display's request (`None` = "Guest N").
    pub name: Option<String>,
    /// The single-use hand-over token the app got from the display (native-apps.md 20.5): the
    /// display admits the join without a dialog. `None` = an ordinary join.
    pub handover: Option<String>,
    /// The secret of the app's device key as base64url without padding (43 characters),
    /// handed over through the App Group (docs/design/presenter-identity.md): the join carries
    /// the proof of this key, so the display sees the same device as for the app. `None` or an
    /// invalid value: no proof.
    pub device_key: Option<String>,
}

struct Running {
    runtime: Runtime,
    input: ExternalInput,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    log: Arc<Log>,
    /// The control channel of the broadcast, once it exists (for [`request_handover`]).
    control: Arc<Mutex<Option<ControlSender>>>,
}

static CURRENT: Mutex<Option<Running>> = Mutex::new(None);

fn slot() -> MutexGuard<'static, Option<Running>> {
    // A poisoned lock only means another thread panicked; the data is still usable.
    CURRENT.lock().unwrap_or_else(|p| p.into_inner())
}

/// Start the sender: join the room and send what is pushed with [`push_frame`].
///
/// Returns at once; the connection is made in the background and reported to `sink`
/// ([`State::Connecting`], then [`State::Waiting`] or [`State::Failed`], ...). Only the code is
/// checked synchronously.
pub fn start(config: Config, sink: Sink) -> Result<(), ErrorCode> {
    let code = RoomCode::parse(&config.room_code).map_err(|_| ErrorCode::InvalidRoomCode)?;
    if config.relay_url.trim().is_empty() {
        return Err(ErrorCode::InvalidRelay);
    }
    let mut guard = slot();
    // A broadcast that ended or failed on its own is replaced without an explicit `stop`.
    if guard.as_ref().is_some_and(|r| r.task.is_finished())
        && let Some(old) = guard.take()
    {
        old.runtime.shutdown_background();
    }
    if guard.is_some() {
        return Err(ErrorCode::AlreadyRunning);
    }
    let log = Arc::new(Log::open(config.log_dir.clone()));
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(512 * 1024)
        .thread_name("cc-ios")
        .enable_all()
        .build()
        .map_err(|e| {
            log.line(&format!("runtime failed: {e}"));
            ErrorCode::Internal
        })?;
    let input = ExternalInput::default();
    let (stop_tx, stop_rx) = oneshot::channel();
    log.line(&format!("start: relay {}", config.relay_url));
    let control = Arc::new(Mutex::new(None));
    let task = runtime.spawn(run(
        config,
        code,
        input.clone(),
        sink,
        Arc::clone(&log),
        stop_rx,
        Arc::clone(&control),
    ));
    *guard = Some(Running {
        runtime,
        input,
        stop: Some(stop_tx),
        task,
        log,
        control,
    });
    Ok(())
}

/// Give the codec configuration (SPS/PPS as Annex B, or an `avcC` record). `false` if nothing
/// runs or `data` holds neither an SPS nor a PPS.
pub fn set_config(data: &[u8]) -> bool {
    current_input().is_some_and(|i| i.set_config(data))
}

/// Give one H.264 access unit (Annex B, or AVCC with 4-byte lengths). Never blocks; copies
/// `data`. `pts_us` is the presentation time in microseconds (negative = unknown).
pub fn push_frame(data: &[u8], is_key: bool, pts_us: i64) -> PushCode {
    match current_input() {
        Some(input) => input.push(data, is_key, u64::try_from(pts_us).ok()).into(),
        None => PushCode::NoSender,
    }
}

/// True once per key frame request: the encoder should produce a sync frame now.
pub fn keyframe_requested() -> bool {
    current_input().is_some_and(|i| i.keyframe_requested())
}

fn current_input() -> Option<ExternalInput> {
    slot().as_ref().map(|r| r.input.clone())
}

/// How long [`request_handover`] waits for the display's answer in all. UNCERTAIN: placeholder.
const HANDOVER_WAIT: Duration = Duration::from_secs(4);

/// Ask the display, on the broadcast's control channel, to hold `token` for the way back to the
/// app (native-apps.md 20.5, step 5): the app joins with it after the broadcast and needs no new
/// approval. Blocks for at most a few seconds; returns `true` when the display answered
/// (`handover_ok`). `false` when nothing runs, the channel is not open, the token is not valid
/// (1 to 64 base64url characters) or the display does not answer (an old or a web display): the
/// app then joins the ordinary way. Call it before [`stop`], never from the status sink.
pub fn request_handover(token: &str) -> bool {
    let (handle, control) = {
        let guard = slot();
        let Some(r) = guard.as_ref() else {
            return false;
        };
        let c = r.control.lock().unwrap_or_else(|p| p.into_inner()).clone();
        (r.runtime.handle().clone(), c)
    };
    let Some(control) = control else {
        return false;
    };
    let token = token.to_owned();
    handle.block_on(async move {
        let wait = async {
            control.send_handover(&token).await.ok()?;
            loop {
                match control.recv_message().await? {
                    DisplayMessage::HandoverOk => return Some(()),
                    DisplayMessage::Page { .. } | DisplayMessage::PauseState { .. } => {}
                }
            }
        };
        matches!(
            tokio::time::timeout(HANDOVER_WAIT, wait).await,
            Ok(Some(()))
        )
    })
}

/// Stop the broadcast and release everything (threads, sockets). Safe when nothing runs.
/// Blocks for at most a few seconds. Must not be called from the status sink.
pub fn stop() {
    let running = slot().take();
    let Some(mut running) = running else { return };
    running.log.line("stop requested by the host");
    running.input.close();
    if let Some(tx) = running.stop.take() {
        let _ = tx.send(());
    }
    let task = running.task;
    let _ = running
        .runtime
        .block_on(async { tokio::time::timeout(STOP_TIMEOUT, task).await });
    running.runtime.shutdown_timeout(Duration::from_secs(1));
}

/// Map an error of the sender's start (before streaming) to a code.
fn start_error(e: &TransportError) -> ErrorCode {
    match e {
        TransportError::Server { code, .. } if code == "room_not_found" => ErrorCode::RoomNotFound,
        TransportError::Server { code, .. } if code == "join_denied" => ErrorCode::JoinDenied,
        TransportError::Server { code, .. } if code == "approval_timeout" => {
            ErrorCode::ApprovalTimeout
        }
        TransportError::Server { code, .. } if code == "room_locked" => ErrorCode::RoomLocked,
        TransportError::Invalid(_) => ErrorCode::InvalidRelay,
        TransportError::Connect(_) | TransportError::Timeout(_) | TransportError::Closed => {
            ErrorCode::ServerUnreachable
        }
        _ => ErrorCode::Internal,
    }
}

async fn run(
    config: Config,
    code: RoomCode,
    input: ExternalInput,
    sink: Sink,
    log: Arc<Log>,
    mut stop_rx: oneshot::Receiver<()>,
    control_slot: Arc<Mutex<Option<ControlSender>>>,
) {
    let info = Mutex::new(Info::default());
    let emit = |state: State, error: ErrorCode, message: &str, input: &ExternalInput| {
        log.line(&format!("{state:?} {error:?} {message}"));
        let i = info.lock().unwrap_or_else(|p| p.into_inner()).clone();
        sink(&Status::new(state, error, &input.stats(), &i, message));
    };
    emit(
        State::Connecting,
        ErrorCode::None,
        "joining the room",
        &input,
    );
    let mut options = SenderOptions::new(config.relay_url.clone(), code);
    options.external = Some(input.clone());
    // The control channel carries the hand-over of the way back (step 5).
    options.control = true;
    options.name = config.name.clone();
    options.handover = config.handover.clone();
    options.device_key = config
        .device_key
        .as_deref()
        .and_then(cc_transport::device::DeviceKey::from_secret_b64);
    if let Some(bind) = config.udp_bind {
        options.session.udp_bind = vec![bind];
    }
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<JoinProgress>();
    options.join_progress = Some(progress_tx);
    let start = SenderCore::start(options);
    tokio::pin!(start);
    let started = loop {
        tokio::select! {
            r = &mut start => break r,
            Some(JoinProgress::Pending { timeout_secs, check, .. }) = progress_rx.recv() => {
                {
                    let mut i = info.lock().unwrap_or_else(|p| p.into_inner());
                    i.check = check;
                    i.timeout_secs = u32::try_from(timeout_secs).unwrap_or(u32::MAX);
                }
                emit(State::Pending, ErrorCode::None, "waiting for the host", &input);
            }
            _ = &mut stop_rx => {
                input.close();
                return;
            }
        }
    };
    let mut core = match started {
        Ok(core) => core,
        Err(e) => {
            input.close();
            emit(State::Failed, start_error(&e), &e.to_string(), &input);
            return;
        }
    };
    {
        let mut i = info.lock().unwrap_or_else(|p| p.into_inner());
        i.check = 0;
        i.timeout_secs = 0;
        i.room_name = core.room_name().unwrap_or_default().to_owned();
    }
    *control_slot.lock().unwrap_or_else(|p| p.into_inner()) = core.control();
    emit(
        State::Waiting,
        ErrorCode::None,
        "joined, waiting for the receiver",
        &input,
    );
    let mut streaming = false;
    let mut tick = tokio::time::interval(STATS_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                core.stop().await;
                break;
            }
            _ = tick.tick(), if streaming => {
                emit(State::Streaming, ErrorCode::None, "", &input);
            }
            ev = core.next_event() => {
                let Some(ev) = ev else {
                    input.close();
                    emit(State::Failed, ErrorCode::ConnectionLost, "sender stopped", &input);
                    break;
                };
                match ev {
                    SenderEvent::State(ConnectionState::Connected) => {
                        streaming = true;
                        emit(State::Streaming, ErrorCode::None, "connected", &input);
                    }
                    SenderEvent::State(ConnectionState::Disconnected) => {
                        streaming = false;
                        emit(State::Reconnecting, ErrorCode::None, "disconnected", &input);
                    }
                    SenderEvent::State(s @ (ConnectionState::Failed | ConnectionState::Closed)) => {
                        input.close();
                        emit(State::Failed, ErrorCode::ConnectionLost, &format!("{s:?}"), &input);
                        break;
                    }
                    SenderEvent::ConnectionCode(code) => {
                        info.lock().unwrap_or_else(|p| p.into_inner()).sas = code;
                        emit(
                            if streaming { State::Streaming } else { State::Waiting },
                            ErrorCode::None,
                            "connection code",
                            &input,
                        );
                    }
                    SenderEvent::State(_) | SenderEvent::Streaming { .. } => {}
                    SenderEvent::Finished { .. } => {
                        input.close();
                        emit(State::Ended, ErrorCode::None, "finished", &input);
                        break;
                    }
                    SenderEvent::PeerLeft => {
                        input.close();
                        emit(State::Ended, ErrorCode::PeerLeft, "the receiver left", &input);
                        break;
                    }
                    SenderEvent::RemovedByHost => {
                        // The host removed this sender: nothing more is sent. The app shows
                        // the same text as for a receiver that left (no separate code yet).
                        input.close();
                        emit(
                            State::Ended,
                            ErrorCode::PeerLeft,
                            "the host removed this sender",
                            &input,
                        );
                        break;
                    }
                    SenderEvent::Error(message) => {
                        input.close();
                        emit(State::Failed, ErrorCode::Internal, &message, &input);
                        break;
                    }
                }
            }
        }
    }
    // The task ends here; the slot keeps `Running` until the host calls `stop`, which is
    // harmless: the input is closed, pushes answer `NoSender`.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_input_before_starting_anything() {
        let sink: Sink = Arc::new(|_| {});
        let cfg = Config {
            room_code: "no".into(),
            relay_url: "127.0.0.1:1".into(),
            ..Config::default()
        };
        assert_eq!(start(cfg, sink.clone()), Err(ErrorCode::InvalidRoomCode));
        let cfg = Config {
            room_code: "ABCDEF".into(),
            relay_url: "  ".into(),
            ..Config::default()
        };
        assert_eq!(start(cfg, sink), Err(ErrorCode::InvalidRelay));
        assert_eq!(
            push_frame(&[0, 0, 0, 1, 0x65, 1], true, 0),
            PushCode::NoSender
        );
        assert!(!set_config(&[0, 0, 0, 1, 0x67, 1, 2, 3, 4]));
        assert!(!keyframe_requested());
        stop();
    }

    #[test]
    fn maps_errors() {
        let e = TransportError::Server {
            code: "room_not_found".into(),
            message: String::new(),
        };
        assert_eq!(start_error(&e), ErrorCode::RoomNotFound);
        let server = |c: &str| TransportError::Server {
            code: c.into(),
            message: String::new(),
        };
        assert_eq!(start_error(&server("join_denied")), ErrorCode::JoinDenied);
        assert_eq!(
            start_error(&server("approval_timeout")),
            ErrorCode::ApprovalTimeout
        );
        assert_eq!(start_error(&server("room_locked")), ErrorCode::RoomLocked);
        assert_eq!(
            start_error(&TransportError::Connect("x".into())),
            ErrorCode::ServerUnreachable
        );
        assert_eq!(
            start_error(&TransportError::Timeout("x")),
            ErrorCode::ServerUnreachable
        );
        assert_eq!(
            start_error(&TransportError::WebRtc("x".into())),
            ErrorCode::Internal
        );
    }

    #[test]
    fn abi_numbers_are_stable() {
        // include/cc_ios.h and the Swift code use these literals.
        assert_eq!(State::Streaming as i32, 3);
        assert_eq!(State::Failed as i32, 6);
        assert_eq!(State::Pending as i32, 7);
        assert_eq!(ErrorCode::RoomNotFound as i32, 3);
        assert_eq!(ErrorCode::AlreadyRunning as i32, 8);
        assert_eq!(ErrorCode::JoinDenied as i32, 9);
        assert_eq!(ErrorCode::ApprovalTimeout as i32, 10);
        assert_eq!(ErrorCode::RoomLocked as i32, 11);
        assert_eq!(PushCode::NoSender as i32, 6);
    }
}
