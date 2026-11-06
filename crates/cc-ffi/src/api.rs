// SPDX-License-Identifier: GPL-3.0-or-later
//! The API that the Flutter app calls (through flutter_rust_bridge, "FRB").
//!
//! How to read this file as a Vue developer: every `pub fn` / `pub async fn` here becomes a
//! Dart function with the same name (camelCase), and every `pub enum` / `pub struct` becomes a
//! Dart class. A function with a `StreamSink` parameter becomes a Dart `Stream`: Rust pushes
//! events into the sink, Dart listens (like an event emitter).
//!
//! Scope of this version (milestone M1, desktop):
//!
//! * Receiver: counts the incoming video and, for H.264, decodes it (OpenH264, feature `h264`)
//!   and offers RGBA pictures to the UI: a `FrameReady` event, then [`take_frame`]. See
//!   `video.rs` for the bounds (one pending picture, drops are counted).
//! * Sender: the built-in VP8 test stream, or ([`ScreenRequest`]) the X11 screen as H.264.
//!   [`capture_info`] says whether screen capture works on this system and lists the monitors.
//! * [`server_defaults`]: the server addresses the UI pre-fills.
//!
//! Only one receiver and one sender can run at a time. Starting a new one stops the old one.
//!
//! After changing this file, run the code generator again (docs/flutter-bridge.md).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cc_proto::{RoomCode, RoomCodeError};
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{ScreenOptions, SenderCore, SenderEvent, SenderOptions};
use cc_transport::{ConnectionState, MediaSnapshot};
use flutter_rust_bridge::frb;
use tokio::sync::oneshot;

use crate::frb_generated::StreamSink;
use crate::video::{self, VIDEO};

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
    /// A decoded picture is waiting: call [`take_frame`]. Sent again only after `take_frame`
    /// was called (so a slow UI does not build up a queue).
    FrameReady,
    /// Counters of the decoder, sent together with `Stats`.
    VideoStats {
        /// Pictures decoded.
        decoded: u64,
        /// Pictures handed to the UI by `take_frame`.
        delivered: u64,
        /// Pictures replaced before the UI fetched them (the UI was too slow).
        dropped: u64,
        /// RTP packets thrown away because the decoder fell behind.
        dropped_packets: u64,
        /// Decoder errors.
        decode_errors: u64,
        /// Key frame requests (PLI) sent to the sender.
        keyframe_requests: u64,
        /// Size of the last picture handed out (after scaling).
        width: u32,
        /// Size of the last picture handed out (after scaling).
        height: u32,
        /// Mean decode + scale time per picture in microseconds.
        avg_decode_us: u64,
        /// Last decoder problem in English, or `None`.
        error: Option<String>,
    },
    /// The sender left. The stream ends after this.
    PeerLeft,
    /// Something failed. The stream ends after this.
    Error {
        /// Technical message in English (for logs and for showing next to a translated title).
        message: String,
    },
}

fn video_stats_event() -> ReceiverUiEvent {
    let c = VIDEO.counters();
    ReceiverUiEvent::VideoStats {
        decoded: c.decoded,
        delivered: c.delivered,
        dropped: c.dropped,
        dropped_packets: c.dropped_packets,
        decode_errors: c.decode_errors,
        keyframe_requests: c.keyframe_requests,
        width: c.width,
        height: c.height,
        avg_decode_us: c.avg_decode_us,
        error: c.error,
    }
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
    let video_generation = VIDEO.reset();
    let mut options = ReceiverOptions::new(server);
    if let Some(b) = udp_bind {
        options.session.udp_bind = vec![b];
    }
    // The decoder thread gets a copy of every incoming video packet. If the start fails below,
    // `options` is dropped, the channel closes and the thread ends.
    let frame_sink = events.clone();
    options.session.video_tap = video::spawn_decoder(video_generation, move || {
        let _ = frame_sink.add(ReceiverUiEvent::FrameReady);
    });
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
            // The decoder thread asks for key frames through a flag; this timer sends them.
            let mut keyframe_tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        core.stop().await;
                        break;
                    }
                    _ = keyframe_tick.tick() => {
                        if VIDEO.take_keyframe_request() {
                            // Fails while no track exists yet; the decoder asks again.
                            let _ = core.request_keyframe().await;
                        }
                    }
                    ev = core.next_event() => {
                        let Some(ev) = ev else { break };
                        if let ReceiverEvent::VideoTrack { codec, .. } = &ev {
                            VIDEO.set_codec_is_h264(matches!(codec, Some(cc_transport::VideoCodec::H264)));
                        }
                        let is_stats = matches!(ev, ReceiverEvent::Stats(_));
                        // If Dart stopped listening, `add` fails: shut down.
                        if events.add(map_receiver_event(ev)).is_err()
                            || (is_stats && events.add(video_stats_event()).is_err())
                        {
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
    VIDEO.stop();
}

/// The newest decoded picture that the UI has not fetched yet (RGBA, 4 bytes per pixel, rows
/// without padding), or `None`. Call it after a [`ReceiverUiEvent::FrameReady`].
///
/// The picture is at most the size set by [`set_video_max_size`] (and never larger than
/// 1920 x 1080). Fetching re-arms the `FrameReady` event.
pub async fn take_frame() -> Option<VideoFrame> {
    VIDEO.take().map(|f| VideoFrame {
        seq: f.seq,
        width: f.width,
        height: f.height,
        rgba: f.rgba,
    })
}

/// One decoded picture for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Counts up with every decoded picture (gaps = dropped pictures).
    pub seq: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA bytes, length = width * height * 4.
    pub rgba: Vec<u8>,
}

/// Tell the decoder how large the picture area is in physical pixels. Pictures larger than
/// this are scaled down (aspect ratio kept); smaller ones are not enlarged. Values are
/// clamped to 1..=1920 x 1..=1080. Call it when the window size changes.
#[frb(sync)]
pub fn set_video_max_size(width: u32, height: u32) {
    VIDEO.set_max_size(width, height);
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
    /// Counters of the screen pipeline, sent together with `Streaming` in screen mode.
    ScreenStats {
        /// Times the screen was looked at.
        polls: u64,
        /// Pictures that differed from the previous one.
        changed: u64,
        /// Frames produced by the encoder.
        encoded: u64,
        /// Of those, key frames.
        keyframes: u64,
        /// Encoded bytes so far.
        encoded_bytes: u64,
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

/// Send the screen instead of the test video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenRequest {
    /// X11 display such as `:99`; `None` = `$DISPLAY`.
    pub display: Option<String>,
    /// `CaptureMonitor::id`; `None` = the whole desktop.
    pub monitor_id: Option<String>,
    /// How often the screen is looked at (1..=60).
    pub fps: u32,
    /// Encoder target bit rate in kbit/s.
    pub bitrate_kbps: u32,
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

/// Start a sender: join the room `code` on `server` and send the built-in VP8 test stream,
/// or with `screen` the X11 screen as H.264, for `seconds` seconds (`None` = until stopped).
///
/// An invalid code or an unknown room is reported as [`SenderUiEvent::Error`] and the stream
/// ends (see [`start_receiver`] for why errors are events).
/// `udp_bind` is for development only, see [`start_receiver`].
pub async fn start_sender(
    code: String,
    server: String,
    seconds: Option<u32>,
    udp_bind: Option<String>,
    screen: Option<ScreenRequest>,
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
    let screen_mode = screen.is_some();
    options.screen = screen.map(|r| ScreenOptions {
        display: r.display,
        source_id: r.monitor_id,
        fps: r.fps.clamp(1, 60),
        bitrate_bps: r.bitrate_kbps.clamp(100, 50_000).saturating_mul(1000),
        ..ScreenOptions::default()
    });
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
                        let is_streaming = matches!(ev, SenderEvent::Streaming { .. });
                        if events.add(map_sender_event(ev)).is_err() {
                            core.stop().await;
                            break;
                        }
                        if screen_mode && is_streaming {
                            let st = core.screen_stats();
                            let stats = SenderUiEvent::ScreenStats {
                                polls: st.polls,
                                changed: st.changed,
                                encoded: st.encoded,
                                keyframes: st.keyframes,
                                encoded_bytes: st.encoded_bytes,
                            };
                            if events.add(stats).is_err() {
                                core.stop().await;
                                break;
                            }
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

// ---------------------------------------------------------------------------
// Screen capture capability and the monitor list
// ---------------------------------------------------------------------------

/// Whether this system can share its screen with this version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSupport {
    /// X11 capture works; `CaptureInfo::monitors` is filled.
    Supported,
    /// The session looks like Wayland (and no X11 display was asked for explicitly): only X11
    /// capture exists, PipeWire / portal capture is not implemented.
    Wayland,
    /// Linux without any display (`DISPLAY` and `WAYLAND_DISPLAY` unset).
    NoDisplay,
    /// An operating system without a capture backend yet (Windows, macOS, mobile).
    UnsupportedOs,
    /// X11 is there but the connection failed.
    Failed {
        /// Technical message in English.
        message: String,
    },
}

/// A part of the desktop that can be shared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureMonitor {
    /// Pass this to [`ScreenRequest::monitor_id`].
    pub id: String,
    /// Name from the X server (or "Whole desktop").
    pub name: String,
    /// Left edge in desktop coordinates.
    pub x: i32,
    /// Top edge in desktop coordinates.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Answer of [`capture_info`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureInfo {
    /// Can this system share its screen?
    pub support: CaptureSupport,
    /// The X11 display that was asked (explicit argument or `$DISPLAY`), if any.
    pub display: Option<String>,
    /// "Whole desktop" first, then the monitors. Empty unless `support` is `Supported`.
    pub monitors: Vec<CaptureMonitor>,
}

/// Check whether screen capture works here and list the monitors.
///
/// `display` forces an X11 display such as `:99` (development and tests). Without it, a
/// session that looks like Wayland (`XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` set) is
/// reported as [`CaptureSupport::Wayland`] even if `DISPLAY` points at XWayland: X11 capture
/// would see only X11 windows there. UNCERTAIN: this heuristic was not tried on a real Wayland
/// desktop.
pub async fn capture_info(display: Option<String>) -> CaptureInfo {
    tokio::task::spawn_blocking(move || capture_info_blocking(display))
        .await
        .unwrap_or_else(|_| CaptureInfo {
            support: CaptureSupport::Failed {
                message: "internal error: the capture check panicked".into(),
            },
            display: None,
            monitors: Vec::new(),
        })
}

#[cfg(target_os = "linux")]
fn capture_info_blocking(display: Option<String>) -> CaptureInfo {
    use cc_capture::ScreenSource;
    use cc_capture::x11::{X11Options, X11Source};

    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let effective = display.clone().or_else(|| env("DISPLAY"));
    let info = |support, monitors| CaptureInfo {
        support,
        display: effective.clone(),
        monitors,
    };
    if display.is_none() {
        let wayland = env("XDG_SESSION_TYPE").is_some_and(|t| t.eq_ignore_ascii_case("wayland"))
            || env("WAYLAND_DISPLAY").is_some();
        if wayland {
            return info(CaptureSupport::Wayland, Vec::new());
        }
        if effective.is_none() {
            return info(CaptureSupport::NoDisplay, Vec::new());
        }
    }
    let listed = X11Source::connect(display.as_deref(), X11Options { fps: 15 })
        .and_then(|src| src.list_sources());
    match listed {
        Ok(list) => info(
            CaptureSupport::Supported,
            list.into_iter()
                .map(|s| CaptureMonitor {
                    id: s.id,
                    name: s.name,
                    x: s.x,
                    y: s.y,
                    width: s.width,
                    height: s.height,
                })
                .collect(),
        ),
        Err(e) => info(
            CaptureSupport::Failed {
                message: e.to_string(),
            },
            Vec::new(),
        ),
    }
}

#[cfg(not(target_os = "linux"))]
fn capture_info_blocking(display: Option<String>) -> CaptureInfo {
    CaptureInfo {
        support: CaptureSupport::UnsupportedOs,
        display,
        monitors: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Server addresses
// ---------------------------------------------------------------------------

/// Server addresses the UI offers when the person has not chosen one yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerDefaults {
    /// The built-in list of `cc-discovery` (`DEFAULT_SERVERS`), in order.
    pub servers: Vec<String>,
    /// Address for development on one machine (a local `cometcast-server`).
    pub dev_fallback: String,
}

/// The server addresses to pre-fill.
///
/// UNCERTAIN: `cc-discovery` only has the built-in list so far; the other steps of its lookup
/// ladder (MDM policy, DNS SRV, display announcement) are not implemented, so this returns just
/// the list. The host in the list (`relay.cometcast.de`) does not exist yet, which is why the
/// UI also offers `dev_fallback`.
#[frb(sync)]
pub fn server_defaults() -> ServerDefaults {
    ServerDefaults {
        servers: cc_discovery::DEFAULT_SERVERS
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        dev_fallback: "127.0.0.1:8443".to_owned(),
    }
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
    fn server_defaults_come_from_cc_discovery() {
        let d = server_defaults();
        assert_eq!(d.servers, ["relay.cometcast.de:443"]);
        assert_eq!(d.dev_fallback, "127.0.0.1:8443");
    }

    #[test]
    fn capture_info_without_a_reachable_display_is_not_supported() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        // Display number 9999 does not exist: the answer is Failed (or UnsupportedOs).
        let info = rt.block_on(capture_info(Some(":9999".into())));
        assert!(info.monitors.is_empty());
        assert!(matches!(
            info.support,
            CaptureSupport::Failed { .. } | CaptureSupport::UnsupportedOs
        ));
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
