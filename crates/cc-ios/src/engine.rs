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
    ExternalInput, ExternalStats, PushOutcome, SenderCore, SenderEvent, SenderOptions,
};
use cc_transport::{ConnectionState, TransportError};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot;
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
    /// Technical English text for the log; never shown to the person (the extension has its own
    /// translated messages, chosen by [`Status::error`]).
    pub message: String,
}

impl Status {
    fn new(state: State, error: ErrorCode, stats: &ExternalStats, message: &str) -> Self {
        Self {
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
}

struct Running {
    runtime: Runtime,
    input: ExternalInput,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    log: Arc<Log>,
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
    let task = runtime.spawn(run(
        config,
        code,
        input.clone(),
        sink,
        Arc::clone(&log),
        stop_rx,
    ));
    *guard = Some(Running {
        runtime,
        input,
        stop: Some(stop_tx),
        task,
        log,
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
) {
    let emit = |state: State, error: ErrorCode, message: &str, input: &ExternalInput| {
        log.line(&format!("{state:?} {error:?} {message}"));
        sink(&Status::new(state, error, &input.stats(), message));
    };
    emit(
        State::Connecting,
        ErrorCode::None,
        "joining the room",
        &input,
    );
    let mut options = SenderOptions::new(config.relay_url.clone(), code);
    options.external = Some(input.clone());
    if let Some(bind) = config.udp_bind {
        options.session.udp_bind = vec![bind];
    }
    let started = tokio::select! {
        r = SenderCore::start(options) => r,
        _ = &mut stop_rx => {
            input.close();
            return;
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
        assert_eq!(ErrorCode::RoomNotFound as i32, 3);
        assert_eq!(ErrorCode::AlreadyRunning as i32, 8);
        assert_eq!(PushCode::NoSender as i32, 6);
    }
}
