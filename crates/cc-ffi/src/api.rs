// SPDX-License-Identifier: GPL-3.0-or-later
//! The API that the Flutter app calls (through flutter_rust_bridge, "FRB").
//!
//! How to read this file as a Vue developer: every `pub fn` / `pub async fn` here becomes a
//! Dart function with the same name (camelCase), and every `pub enum` / `pub struct` becomes a
//! Dart class. A function with a `StreamSink` parameter becomes a Dart `Stream`: Rust pushes
//! events into the sink, Dart listens (like an event emitter).
//!
//! Scope of this version (milestone M1): a receiver and a sender that exchange a synthetic
//! VP8 test stream. No screen capture, no decoding and no rendering yet.
//!
//! Only one receiver and one sender can run at a time. Starting a new one stops the old one.
//!
//! After changing this file, run the code generator again (docs/flutter-bridge.md).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cc_proto::{RoomCode, RoomCodeError};
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{SenderCore, SenderEvent, SenderOptions};
use cc_transport::{ConnectionState, MediaSnapshot};
use flutter_rust_bridge::frb;
use tokio::sync::oneshot;

use crate::frb_generated::StreamSink;

/// Called once when the library is loaded.
#[frb(init)]
pub fn init_app() {
    // Default FRB helpers (turns Rust panics into Dart exceptions, etc.).
    flutter_rust_bridge::setup_default_user_utils();
}

// ---------------------------------------------------------------------------
// Room code validation (one source of truth: cc-proto)
// ---------------------------------------------------------------------------

/// Result of checking text that a person typed as a room code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomCodeCheck {
    /// Good. `code` is the normalised code (uppercase, spaces trimmed).
    Valid {
        /// The normalised room code.
        code: String,
    },
    /// Not exactly [`room_code_length`] characters.
    WrongLength,
    /// A character outside [`room_code_alphabet`].
    InvalidChar {
        /// The first offending character.
        ch: String,
    },
}

/// Check typed text with the same rules as `cc_proto::RoomCode::parse`.
#[frb(sync)]
pub fn check_room_code(input: String) -> RoomCodeCheck {
    // A bad character is reported before a wrong length, so the UI can warn while the person
    // is still typing. The rule itself (alphabet, ASCII-only uppercase) is cc-proto's.
    let upper = input.trim().to_ascii_uppercase();
    if let Some(bad) = upper.chars().find(|c| !RoomCode::ALPHABET.contains(*c)) {
        return RoomCodeCheck::InvalidChar {
            ch: bad.to_string(),
        };
    }
    match RoomCode::parse(&input) {
        Ok(code) => RoomCodeCheck::Valid {
            code: code.as_str().to_owned(),
        },
        Err(RoomCodeError::WrongLength) => RoomCodeCheck::WrongLength,
        Err(RoomCodeError::InvalidChar(c)) => RoomCodeCheck::InvalidChar { ch: c.to_string() },
    }
}

/// The characters a room code may contain (uppercase).
#[frb(sync)]
pub fn room_code_alphabet() -> String {
    RoomCode::ALPHABET.to_owned()
}

/// Number of characters in a room code.
#[frb(sync)]
pub fn room_code_length() -> u32 {
    // The constant is 6; the cast cannot truncate.
    RoomCode::LEN as u32
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// WebRTC connection state (a copy of the transport's enum so Dart gets its own type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiConnectionState {
    /// Not started.
    New,
    /// ICE / DTLS in progress.
    Connecting,
    /// Media can flow.
    Connected,
    /// Temporarily lost (may recover).
    Disconnected,
    /// Gave up.
    Failed,
    /// Closed.
    Closed,
}

impl From<ConnectionState> for UiConnectionState {
    fn from(s: ConnectionState) -> Self {
        match s {
            ConnectionState::New => Self::New,
            ConnectionState::Connecting => Self::Connecting,
            ConnectionState::Connected => Self::Connected,
            ConnectionState::Disconnected => Self::Disconnected,
            ConnectionState::Failed => Self::Failed,
            ConnectionState::Closed => Self::Closed,
        }
    }
}

// ---------------------------------------------------------------------------
// Receiver
// ---------------------------------------------------------------------------

/// What the receiver tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverUiEvent {
    /// The room exists; the sender must enter this code. Always the first event.
    RoomCode {
        /// The room code, uppercase.
        code: String,
    },
    /// A sender joined the room.
    PeerJoined,
    /// The WebRTC connection state changed.
    ConnectionState {
        /// New state.
        state: UiConnectionState,
    },
    /// The sender's video track arrived.
    VideoTrack {
        /// MIME type, e.g. `video/VP8`.
        mime: String,
        /// Codec name (`vp8`, `vp9`, `h264`, `av1`) or `None` if unknown.
        codec: Option<String>,
    },
    /// Counters of incoming video, about once per second.
    Stats {
        /// RTP packets received.
        packets: u64,
        /// RTP payload bytes received.
        bytes: u64,
        /// Frames received.
        frames: u64,
        /// Key frames received.
        keyframes: u64,
    },
    /// The sender left. The stream ends after this.
    PeerLeft,
    /// Something failed. The stream ends after this.
    Error {
        /// Technical message in English (for logs and for showing next to a translated title).
        message: String,
    },
}

fn stats_event(s: MediaSnapshot) -> ReceiverUiEvent {
    ReceiverUiEvent::Stats {
        packets: s.packets,
        bytes: s.bytes,
        frames: s.frames,
        keyframes: s.keyframes,
    }
}

fn map_receiver_event(e: ReceiverEvent) -> ReceiverUiEvent {
    match e {
        ReceiverEvent::PeerJoined => ReceiverUiEvent::PeerJoined,
        ReceiverEvent::State(s) => ReceiverUiEvent::ConnectionState { state: s.into() },
        ReceiverEvent::VideoTrack { mime, codec } => ReceiverUiEvent::VideoTrack {
            mime,
            // Debug of the enum gives "Vp8", "H264", ... so lowercase it.
            codec: codec.map(|c| format!("{c:?}").to_lowercase()),
        },
        ReceiverEvent::Stats(s) => stats_event(s),
        ReceiverEvent::PeerLeft => ReceiverUiEvent::PeerLeft,
        ReceiverEvent::Error(message) => ReceiverUiEvent::Error { message },
    }
}

/// Bookkeeping for "the one running receiver (or sender)".
///
/// `generation` counts starts and stops. A start remembers the number it got; if the number
/// changed by the time `Core::start(..).await` returns, a stop (or a newer start) happened
/// while connecting, and the new core must be closed at once. Without it a stop during the
/// connect phase would be lost.
struct Slot {
    stop: Mutex<Option<oneshot::Sender<()>>>,
    generation: AtomicU64,
}

impl Slot {
    const fn new() -> Self {
        Self {
            stop: Mutex::new(None),
            generation: AtomicU64::new(0),
        }
    }

    /// Stop the running one (if any) and return the generation number for the new start.
    fn begin(&self) -> u64 {
        let g = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.fire();
        g
    }

    /// Stop the running one, and invalidate a start that is still connecting.
    fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.fire();
    }

    /// True if no stop or newer start happened since `begin` returned `generation`.
    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    fn fire(&self) {
        // A poisoned lock only means another thread panicked; the data is still usable.
        let tx = self.stop.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(tx) = tx {
            let _ = tx.send(());
        }
    }

    fn store(&self, tx: oneshot::Sender<()>) {
        *self.stop.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx);
    }
}

static RECEIVER: Slot = Slot::new();
static SENDER: Slot = Slot::new();

/// Run `task` in the background. If it panics, report that on `on_panic` instead of
/// silently losing the stream (a panic inside a spawned task is otherwise swallowed).
fn spawn_guarded(
    task: impl std::future::Future<Output = ()> + Send + 'static,
    on_panic: impl FnOnce() + Send + 'static,
) {
    let handle = tokio::spawn(task);
    tokio::spawn(async move {
        if let Err(e) = handle.await
            && e.is_panic()
        {
            on_panic();
        }
    });
}

/// Start a receiver: connect to `server` (`host:port`), create a room and report events.
///
/// Returns once the room exists (the first event is then [`ReceiverUiEvent::RoomCode`]).
/// A failure to connect is reported as [`ReceiverUiEvent::Error`] and the stream ends. (Errors
/// are events, not return values: for a function that returns a stream, FRB cannot deliver a
/// returned error to the listener.)
/// `udp_bind` is for development only (for example `127.0.0.1:0` for same-machine tests);
/// `None` uses the default (all interfaces).
pub async fn start_receiver(
    server: String,
    udp_bind: Option<String>,
    events: StreamSink<ReceiverUiEvent>,
) {
    let generation = RECEIVER.begin();
    let mut options = ReceiverOptions::new(server);
    if let Some(b) = udp_bind {
        options.session.udp_bind = vec![b];
    }
    let mut core = match ReceiverCore::start(options).await {
        Ok(core) => core,
        Err(e) => {
            let _ = events.add(ReceiverUiEvent::Error {
                message: e.to_string(),
            });
            return;
        }
    };
    // A stop (or newer start) may have happened while we were connecting.
    // If Dart stopped listening, `add` fails too. Either way: close the new core.
    if !RECEIVER.is_current(generation)
        || events
            .add(ReceiverUiEvent::RoomCode {
                code: core.room_code().as_str().to_owned(),
            })
            .is_err()
    {
        core.stop().await;
        return;
    }
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    RECEIVER.store(stop_tx);
    // A stop between the check above and `store` found an empty slot: check once more.
    if !RECEIVER.is_current(generation) {
        core.stop().await;
        return;
    }
    let panic_sink = events.clone();
    spawn_guarded(
        async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        core.stop().await;
                        break;
                    }
                    ev = core.next_event() => {
                        let Some(ev) = ev else { break };
                        // If Dart stopped listening, `add` fails: shut down.
                        if events.add(map_receiver_event(ev)).is_err() {
                            core.stop().await;
                            break;
                        }
                    }
                }
            }
            // `events` is dropped here, which ends the Dart stream.
        },
        move || {
            let _ = panic_sink.add(ReceiverUiEvent::Error {
                message: "internal error: the receiver task panicked".into(),
            });
        },
    );
}

/// Stop the running receiver (does nothing if none runs). The Dart stream then ends.
pub fn stop_receiver() {
    RECEIVER.stop();
}

// ---------------------------------------------------------------------------
// Sender (synthetic test stream)
// ---------------------------------------------------------------------------

/// What the sender tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderUiEvent {
    /// The room was joined and the offer sent. Always the first event.
    Joined,
    /// The WebRTC connection state changed.
    ConnectionState {
        /// New state.
        state: UiConnectionState,
    },
    /// Frames sent so far, about once per second.
    Streaming {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The requested duration is over. The stream ends after this.
    Finished {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The receiver left. The stream ends after this.
    PeerLeft,
    /// Something failed. The stream ends after this.
    Error {
        /// Technical message in English.
        message: String,
    },
}

fn map_sender_event(e: SenderEvent) -> SenderUiEvent {
    match e {
        SenderEvent::State(s) => SenderUiEvent::ConnectionState { state: s.into() },
        SenderEvent::Streaming { frames_sent } => SenderUiEvent::Streaming { frames_sent },
        SenderEvent::Finished { frames_sent } => SenderUiEvent::Finished { frames_sent },
        SenderEvent::PeerLeft => SenderUiEvent::PeerLeft,
        SenderEvent::Error(message) => SenderUiEvent::Error { message },
    }
}

/// Start a sender: join the room `code` on `server` and send the built-in VP8 test stream
/// for `seconds` seconds (`None` = until stopped).
///
/// An invalid code or an unknown room is reported as [`SenderUiEvent::Error`] and the stream
/// ends (see [`start_receiver`] for why errors are events).
/// `udp_bind` is for development only, see [`start_receiver`].
pub async fn start_sender(
    code: String,
    server: String,
    seconds: Option<u32>,
    udp_bind: Option<String>,
    events: StreamSink<SenderUiEvent>,
) {
    let generation = SENDER.begin();
    let code = match RoomCode::parse(&code) {
        Ok(code) => code,
        Err(e) => {
            let _ = events.add(SenderUiEvent::Error {
                message: e.to_string(),
            });
            return;
        }
    };
    let mut options = SenderOptions::new(server, code);
    options.duration = seconds.map(|s| Duration::from_secs(u64::from(s)));
    if let Some(b) = udp_bind {
        options.session.udp_bind = vec![b];
    }
    let mut core = match SenderCore::start(options).await {
        Ok(core) => core,
        Err(e) => {
            let _ = events.add(SenderUiEvent::Error {
                message: e.to_string(),
            });
            return;
        }
    };
    if !SENDER.is_current(generation) || events.add(SenderUiEvent::Joined).is_err() {
        core.stop().await;
        return;
    }
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    SENDER.store(stop_tx);
    if !SENDER.is_current(generation) {
        core.stop().await;
        return;
    }
    let panic_sink = events.clone();
    spawn_guarded(
        async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        core.stop().await;
                        break;
                    }
                    ev = core.next_event() => {
                        let Some(ev) = ev else { break };
                        if events.add(map_sender_event(ev)).is_err() {
                            core.stop().await;
                            break;
                        }
                    }
                }
            }
        },
        move || {
            let _ = panic_sink.add(SenderUiEvent::Error {
                message: "internal error: the sender task panicked".into(),
            });
        },
    );
}

/// Stop the running sender (does nothing if none runs). The Dart stream then ends.
pub fn stop_sender() {
    SENDER.stop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_during_connect_invalidates_the_start() {
        let slot = Slot::new();
        let g = slot.begin();
        assert!(slot.is_current(g));
        // A stop while the start is still connecting (before `store`) must be noticed.
        slot.stop();
        assert!(!slot.is_current(g));
        // A newer start also invalidates an older one.
        let g2 = slot.begin();
        let g3 = slot.begin();
        assert!(!slot.is_current(g2) && slot.is_current(g3));
    }

    #[test]
    fn stop_fires_the_stored_signal_once() {
        let slot = Slot::new();
        let g = slot.begin();
        let (tx, mut rx) = oneshot::channel::<()>();
        slot.store(tx);
        assert!(slot.is_current(g));
        slot.stop();
        assert!(rx.try_recv().is_ok());
    }

    #[tokio::test]
    async fn panic_in_a_guarded_task_is_reported() {
        let (tx, rx) = oneshot::channel::<()>();
        spawn_guarded(async { panic!("boom") }, move || {
            let _ = tx.send(());
        });
        assert!(rx.await.is_ok());
    }

    #[test]
    fn room_code_check_matches_cc_proto() {
        assert_eq!(
            check_room_code(" k7m2qx ".into()),
            RoomCodeCheck::Valid {
                code: "K7M2QX".into()
            }
        );
        assert_eq!(check_room_code("K7M2".into()), RoomCodeCheck::WrongLength);
        // Bad character wins over wrong length (early feedback while typing).
        assert_eq!(
            check_room_code("K70".into()),
            RoomCodeCheck::InvalidChar { ch: "0".into() }
        );
        // 'O' and '0' are not in the alphabet.
        assert_eq!(
            check_room_code("K7M2QO".into()),
            RoomCodeCheck::InvalidChar { ch: "O".into() }
        );
        assert_eq!(room_code_alphabet(), RoomCode::ALPHABET);
        assert_eq!(room_code_length(), 6);
    }

    #[test]
    fn receiver_events_are_mapped() {
        assert_eq!(
            map_receiver_event(ReceiverEvent::State(ConnectionState::Connected)),
            ReceiverUiEvent::ConnectionState {
                state: UiConnectionState::Connected
            }
        );
        assert_eq!(
            map_receiver_event(ReceiverEvent::Stats(MediaSnapshot {
                packets: 1,
                bytes: 2,
                frames: 3,
                keyframes: 4
            })),
            ReceiverUiEvent::Stats {
                packets: 1,
                bytes: 2,
                frames: 3,
                keyframes: 4
            }
        );
        assert_eq!(
            map_receiver_event(ReceiverEvent::VideoTrack {
                mime: "video/VP8".into(),
                codec: Some(cc_transport::VideoCodec::Vp8)
            }),
            ReceiverUiEvent::VideoTrack {
                mime: "video/VP8".into(),
                codec: Some("vp8".into())
            }
        );
    }

    #[test]
    fn sender_events_are_mapped() {
        assert_eq!(
            map_sender_event(SenderEvent::Finished { frames_sent: 9 }),
            SenderUiEvent::Finished { frames_sent: 9 }
        );
        assert_eq!(
            map_sender_event(SenderEvent::Error("x".into())),
            SenderUiEvent::Error {
                message: "x".into()
            }
        );
    }
}
