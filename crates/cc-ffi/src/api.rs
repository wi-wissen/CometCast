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
//! * Sender: the built-in VP8 test stream, or ([`ScreenRequest`]) the X11 screen as H.264, or
//!   ([`start_encoded_sender`]) H.264 access units that the host pushes with
//!   [`push_encoded_frame`] (Android: `MediaCodec` output, see docs/android.md).
//!   [`capture_info`] says whether screen capture works on this system and lists the monitors.
//! * [`server_defaults`]: the server addresses the UI pre-fills (policy file, built-in list);
//!   [`discover_servers`] also asks DNS-SRV and display announcements.
//!
//! Only one receiver and one sender can run at a time. Starting a new one stops the old one.
//!
//! After changing this file, run the code generator again (docs/flutter-bridge.md).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cc_proto::{JoinMode, RoomCode, RoomCodeError};
use cc_receiver::{ContentOptions, ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{
    ContentSender, ContentSenderOptions, ExternalInput, PushOutcome, ScreenOptions, SenderCore,
    SenderEvent, SenderOptions,
};
use cc_transport::{ConnectionState, JoinProgress, MediaSnapshot, TransportError};
use flutter_rust_bridge::frb;
use tokio::sync::oneshot;

use crate::content::{self, CONTENT};
use crate::content_sender;
use crate::encoded;
use crate::frb_generated::StreamSink;
use crate::nearby;
use crate::office;
use crate::presenter::{self, PCmd};
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

/// Who may join a room (docs/protocol.md, "Join approval").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiJoinMode {
    /// Anybody who knows the code joins at once.
    Open,
    /// Every join waits for an explicit allow or deny on this device.
    Confirm,
    /// Nobody can join.
    Locked,
}

impl From<JoinMode> for UiJoinMode {
    fn from(m: JoinMode) -> Self {
        match m {
            JoinMode::Open => Self::Open,
            JoinMode::Confirm => Self::Confirm,
            JoinMode::Locked => Self::Locked,
        }
    }
}

impl From<UiJoinMode> for JoinMode {
    fn from(m: UiJoinMode) -> Self {
        match m {
            UiJoinMode::Open => Self::Open,
            UiJoinMode::Confirm => Self::Confirm,
            UiJoinMode::Locked => Self::Locked,
        }
    }
}

/// What the receiver tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverUiEvent {
    /// The room exists; the sender must enter this code. Always the first event.
    RoomCode {
        /// The room code, uppercase.
        code: String,
        /// The join mode the room has.
        join_mode: UiJoinMode,
    },
    /// A sender joined the room (it is allowed to present).
    PeerJoined {
        /// Cleaned display name of the sender (plain text, at most 40 characters).
        name: String,
        /// The join came with a hand-over token this display had issued (the iOS broadcast
        /// extension, native-apps.md 20.5) and was let in without a dialog: show no
        /// "connected" notice, the person was approved before.
        handover: bool,
    },
    /// A presenter's hand-over token is stored (the display answered `handover_ok`). The
    /// presenter leaving now is not a leave for the host (no notice) until the token is used
    /// ([`ReceiverUiEvent::PeerJoined`] with `handover`) or runs out
    /// ([`ReceiverUiEvent::HandoverExpired`]).
    HandoverStored {
        /// The presenter's name.
        name: String,
    },
    /// A hand-over token ran out unused (60 s): back to normal.
    HandoverExpired,
    /// Join mode `confirm`: somebody waits for a decision ([`decide_join`]). The name is
    /// untrusted text that the server cleaned: show it as plain text.
    JoinRequest {
        /// Request id for [`decide_join`].
        id: u64,
        /// Cleaned display name.
        name: String,
        /// Two-digit check number; the sender's waiting screen shows the same number.
        check: u8,
    },
    /// A waiting request ended: `left`, `cancelled`, `timeout`, `locked` or `room_full`.
    JoinRequestCancelled {
        /// Request id.
        id: u64,
        /// Why.
        reason: String,
    },
    /// The join mode in force now (confirmed by the server).
    JoinModeChanged {
        /// The mode.
        mode: UiJoinMode,
    },
    /// A sender was added to the room's block list ("No and block", or "Remove and block").
    Blocked {
        /// Block id for [`unblock`].
        id: u64,
        /// Cleaned display name (for the list only).
        name: String,
    },
    /// An entry left the block list.
    Unblocked {
        /// Block id.
        id: u64,
    },
    /// The room session was ended by [`end_room`]. The stream ends after this; start a new
    /// receiver for a new room (a new code).
    RoomEnded,
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
    /// Pass-through mode ([`set_video_passthrough`]): encoded H.264 access units wait; call
    /// [`take_encoded_video`]. Sent again only after that call.
    EncodedReady,
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
    /// A transfer began (only with content enabled). For a "receiving ..." note; progress
    /// follows as [`ReceiverUiEvent::ContentTransferProgress`].
    ContentTransferStarted {
        /// `true` for a PDF, `false` for an image.
        document: bool,
        /// File name, already cleaned of control and invisible characters.
        name: String,
        /// Announced size in bytes.
        size: u64,
    },
    /// Bytes of the running transfer received so far, at most one per whole percent and four
    /// per second, between `ContentTransferStarted` and the picture (the last one has
    /// `received == total`).
    ContentTransferProgress {
        /// Bytes received.
        received: u64,
        /// Announced size in bytes.
        total: u64,
    },
    /// An image or a PDF page waits: call [`take_content_frame`]. Sent again only after
    /// `take_content_frame` was called.
    ContentFrameReady,
    /// A PDF was accepted. Pages are rendered next and announced with `ContentFrameReady`.
    ContentDocumentOpened {
        /// File name, cleaned.
        name: String,
        /// Number of pages (at least 1).
        pages: u32,
    },
    /// The sender asks to open a link. **Nothing is opened by the core or by this layer**:
    /// the UI shows the host and opens the link only after a click. The URL passed the
    /// `validate_link` rules.
    ContentLink {
        /// The normalised URL (ASCII, http or https).
        url: String,
        /// Host in its punycode form (an internationalised name shows as `xn--...`).
        host: String,
    },
    /// The sender stopped the content: whatever is shown should disappear.
    ContentStopped,
    /// A transfer was refused or failed. Only a code: text of the sender never reaches the UI.
    ContentError {
        /// What happened.
        error: UiContentError,
    },
    /// The sender left or was removed. **The room stays**: the stream goes on, the code, the
    /// join mode, the block list and the queue are the same, and the next sender can join.
    PeerLeft,
    /// The sender stopped its presentation but stays in the room: show Idle without a leave
    /// notice. The next presentation of the same sender needs no new approval.
    PresentationStopped,
    /// The connection check code of the sender's connection, `"123 456"`, once per
    /// connection; the presenter shows the same digits (`cc_transport::sas`).
    ConnectionCode {
        /// The six digits with a space in the middle.
        sas: String,
    },
    /// Something failed. The stream ends after this.
    Error {
        /// The code the UI maps: a protocol code (`room_locked`, `server_full`, ...) or
        /// `connection_lost`, `peer_failed`, `unknown`.
        code: String,
        /// Technical message in English (for logs only).
        message: String,
    },
}

/// Why received content could not be shown. The UI translates these codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiContentError {
    /// A message that is malformed, unknown or refused (too large, type not allowed).
    BadMessage,
    /// Data without a transfer.
    UnexpectedChunk,
    /// More data than announced.
    TooLarge,
    /// Fewer bytes than announced.
    SizeMismatch,
    /// An end message without a transfer.
    NoTransfer,
    /// A chunk that breaks the size rule.
    BadChunk,
    /// A new transfer began before the previous one ended.
    Interrupted,
    /// The PDF was refused (damaged, password, too many pages, ...).
    DocumentRejected,
    /// A page could not be rendered.
    RenderFailed,
    /// Too many protocol violations; further input is ignored.
    TooManyErrors,
    /// A transfer stalled and was dropped.
    TransferTimeout,
    /// No PDF render worker program was found, so no PDF can be shown.
    WorkerUnavailable,
}

/// A picture of received content, fetched with [`take_content_frame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentFrame {
    /// An image file as it was sent (PNG, JPEG, WebP or GIF; the UI decodes it).
    Image {
        /// File name, cleaned.
        name: String,
        /// `image/png`, `image/jpeg`, `image/webp` or `image/gif`.
        mime: String,
        /// The file's bytes (at most 10 MiB).
        bytes: Vec<u8>,
    },
    /// One rendered PDF page.
    Page {
        /// Page number, 1-based.
        page: u32,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// RGBA, rows without padding, opaque white background.
        rgba: Vec<u8>,
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
        ReceiverEvent::PeerJoined { name, handover } => {
            ReceiverUiEvent::PeerJoined { name, handover }
        }
        ReceiverEvent::HandoverStored { name } => ReceiverUiEvent::HandoverStored { name },
        ReceiverEvent::HandoverExpired => ReceiverUiEvent::HandoverExpired,
        ReceiverEvent::JoinRequest { id, name, check } => {
            ReceiverUiEvent::JoinRequest { id, name, check }
        }
        ReceiverEvent::JoinRequestCancelled { id, reason } => {
            ReceiverUiEvent::JoinRequestCancelled { id, reason }
        }
        ReceiverEvent::JoinModeChanged(m) => ReceiverUiEvent::JoinModeChanged { mode: m.into() },
        ReceiverEvent::Blocked { id, name } => ReceiverUiEvent::Blocked { id, name },
        ReceiverEvent::Unblocked { id } => ReceiverUiEvent::Unblocked { id },
        ReceiverEvent::RoomEnded => ReceiverUiEvent::RoomEnded,
        ReceiverEvent::State(s) => ReceiverUiEvent::ConnectionState { state: s.into() },
        ReceiverEvent::VideoTrack { mime, codec } => ReceiverUiEvent::VideoTrack {
            mime,
            // Debug of the enum gives "Vp8", "H264", ... so lowercase it.
            codec: codec.map(|c| format!("{c:?}").to_lowercase()),
        },
        ReceiverEvent::Stats(s) => stats_event(s),
        ReceiverEvent::PeerLeft => ReceiverUiEvent::PeerLeft,
        ReceiverEvent::PresentationStopped => ReceiverUiEvent::PresentationStopped,
        ReceiverEvent::ConnectionCode(sas) => ReceiverUiEvent::ConnectionCode { sas },
        ReceiverEvent::Error(message) => ReceiverUiEvent::Error {
            code: cc_receiver::error_code(&message),
            message,
        },
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

/// Commands of the UI to the running receiver (join approval).
enum ReceiverCmd {
    SetMode(JoinMode),
    Decide { id: Option<u64>, allow: bool },
    DenyAndBlock(u64),
    Unblock(u64),
    EndRoom,
    RemovePeer { block: bool },
}

/// The command channel of the running receiver (replaced at every start).
static RECEIVER_CMDS: Mutex<Option<tokio::sync::mpsc::UnboundedSender<ReceiverCmd>>> =
    Mutex::new(None);

fn send_receiver_cmd(cmd: ReceiverCmd) -> bool {
    RECEIVER_CMDS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .is_some_and(|tx| tx.send(cmd).is_ok())
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
///
/// `room_name` is the name of the display as senders see it ("Meeting room Elbe"): it goes to the
/// server with `create_room` (the server cleans it and cuts it at 32 characters); `None` or an
/// empty name = no room name.
///
/// The room outlives a presenter: after [`ReceiverUiEvent::PeerLeft`] the stream stays open and
/// the same code is valid for the next sender (it ends on [`end_room`], an error or
/// [`stop_receiver`]).
///
/// With `content` the receiver also accepts images, PDFs, links and page commands on the
/// `control` data channel (docs/web-m2.md) and reports them as the `Content...` events; without
/// it the channel is refused. PDFs render in a resource-limited child process (docs/documents.md);
/// see [`docs_worker_path`] for how it is found.
pub async fn start_receiver(
    server: String,
    udp_bind: Option<String>,
    content: bool,
    join_mode: UiJoinMode,
    room_name: Option<String>,
    events: StreamSink<ReceiverUiEvent>,
) {
    let generation = RECEIVER.begin();
    *RECEIVER_CMDS.lock().unwrap_or_else(|p| p.into_inner()) = None;
    let video_generation = VIDEO.reset();
    let content_generation = CONTENT.reset();
    let worker = content::worker();
    let announce_server = server.clone();
    let mut options = ReceiverOptions::new(server);
    options.join_mode = join_mode.into();
    options.room_name = room_name.filter(|n| !n.trim().is_empty());
    let announce_room_name = options.room_name.clone();
    if let Some(b) = udp_bind {
        options.session.udp_bind = vec![b];
    }
    if content {
        let mut co = ContentOptions {
            isolation: worker.isolation.clone(),
            ..ContentOptions::default()
        };
        if let Some(w) = content::initial_width() {
            co.target_width = w;
        }
        options.content = Some(co);
    }
    // The decoder thread gets a copy of every incoming video packet. If the start fails below,
    // `options` is dropped, the channel closes and the thread ends.
    let frame_sink = events.clone();
    options.session.video_tap = video::spawn_decoder(video_generation, move |encoded| {
        let _ = frame_sink.add(if encoded {
            ReceiverUiEvent::EncodedReady
        } else {
            ReceiverUiEvent::FrameReady
        });
    });
    let mut core = match ReceiverCore::start(options).await {
        Ok(core) => core,
        Err(e) => {
            let _ = events.add(ReceiverUiEvent::Error {
                code: e.code(),
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
                join_mode: core.join_mode().into(),
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
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ReceiverCmd>();
    *RECEIVER_CMDS.lock().unwrap_or_else(|p| p.into_inner()) = Some(cmd_tx);
    // From here on the display announces its code (when the UI allows it, spec 14.8); the guard
    // withdraws the announcement whenever this task ends, for whatever reason.
    nearby::receiver_started(
        generation,
        announce_server,
        announce_room_name,
        core.room_code().as_str().to_owned(),
        core.join_mode(),
    );
    let announce_guard = nearby::ReceiverGuard::new(generation);
    let panic_sink = events.clone();
    let mut content_events = core.take_content_events();
    let (width_tx, mut width_rx) = tokio::sync::mpsc::unbounded_channel::<u32>();
    content::set_width_channel(content_generation, width_tx);
    spawn_guarded(
        async move {
            let _announce_guard = announce_guard;
            // The decoder thread asks for key frames through a flag; this timer sends them.
            let mut keyframe_tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop_rx => {
                        core.stop().await;
                        break;
                    }
                    // Content first: the core waits for it to be read (docs/transport.md).
                    cev = async {
                        match content_events.as_mut() {
                            Some(c) => c.next().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        match cev {
                            None => content_events = None,
                            Some(cev) => {
                                if let Some(ui) = content::map_content_event(content_generation, worker.found, cev)
                                    && events.add(ui).is_err()
                                {
                                    core.stop().await;
                                    break;
                                }
                            }
                        }
                    }
                    Some(w) = width_rx.recv() => core.set_content_target_width(w),
                    Some(cmd) = cmd_rx.recv() => match cmd {
                        ReceiverCmd::SetMode(m) => {
                            // Locked takes the code out of the announcement at once, without
                            // waiting for the server's confirmation.
                            nearby::receiver_mode(generation, m);
                            core.set_join_mode(m);
                        }
                        ReceiverCmd::Decide { id: Some(id), allow } => core.decide_join(id, allow),
                        ReceiverCmd::Decide { id: None, .. } => core.deny_all_joins(),
                        ReceiverCmd::DenyAndBlock(id) => core.deny_and_block_join(id),
                        ReceiverCmd::Unblock(id) => core.unblock(id),
                        ReceiverCmd::EndRoom => core.end_room(),
                        ReceiverCmd::RemovePeer { block } => core.remove_peer(block),
                    },
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
                        if let ReceiverEvent::JoinModeChanged(m) = &ev {
                            nearby::receiver_mode(generation, *m);
                        }
                        let is_stats = matches!(ev, ReceiverEvent::Stats(_));
                        let left = matches!(ev, ReceiverEvent::PeerLeft);
                        // If Dart stopped listening, `add` fails: shut down.
                        if events.add(map_receiver_event(ev)).is_err()
                            || (is_stats && events.add(video_stats_event()).is_err())
                        {
                            core.stop().await;
                            break;
                        }
                        if left
                            && let Some(c) = content_events.as_mut()
                        {
                            // The UI knows that the sender left. Content that arrived just
                            // before is still queued: hand it over, but stay stoppable.
                            let mut alive = true;
                            let mut stopped = false;
                            loop {
                                tokio::select! {
                                    biased;
                                    _ = &mut stop_rx => {
                                        stopped = true;
                                        break;
                                    }
                                    r = tokio::time::timeout(CONTENT_DRAIN, c.next()) => {
                                        let Ok(Some(cev)) = r else { break };
                                        if let Some(ui) = content::map_content_event(content_generation, worker.found, cev)
                                            && events.add(ui).is_err()
                                        {
                                            alive = false;
                                            break;
                                        }
                                    }
                                }
                            }
                            if stopped || !alive {
                                core.stop().await;
                                break;
                            }
                        }
                    }
                }
            }
            content::clear_width_channel(content_generation);
            // `events` is dropped here, which ends the Dart stream.
        },
        move || {
            let _ = panic_sink.add(ReceiverUiEvent::Error {
                code: "unknown".into(),
                message: "internal error: the receiver task panicked".into(),
            });
        },
    );
}

/// How long to wait for the next queued content event after the sender left.
const CONTENT_DRAIN: Duration = Duration::from_secs(2);

/// Stop the running receiver (does nothing if none runs). The Dart stream then ends.
pub fn stop_receiver() {
    *RECEIVER_CMDS.lock().unwrap_or_else(|p| p.into_inner()) = None;
    RECEIVER.stop();
    VIDEO.stop();
    CONTENT.reset();
}

/// Change who may join the running receiver's room, at any time. `Locked` also refuses
/// everybody who waits. The server confirms with [`ReceiverUiEvent::JoinModeChanged`].
/// Returns `false` if no receiver runs.
#[frb(sync)]
pub fn set_join_mode(mode: UiJoinMode) -> bool {
    send_receiver_cmd(ReceiverCmd::SetMode(mode.into()))
}

/// Allow or deny a waiting request (`id` from [`ReceiverUiEvent::JoinRequest`]). `id: None`
/// with `allow: false` denies everybody who waits (`None` with `true` does nothing).
/// Returns `false` if no receiver runs.
#[frb(sync)]
pub fn decide_join(id: Option<u64>, allow: bool) -> bool {
    if id.is_none() && allow {
        return false;
    }
    send_receiver_cmd(ReceiverCmd::Decide { id, allow })
}

/// Remove the connected sender (end its presentation); with `block` also block its address
/// bucket for the rest of this room session. The receiver reports `PeerLeft` and stays in the
/// room with the same code. Returns `false` if no receiver runs.
#[frb(sync)]
pub fn remove_peer(block: bool) -> bool {
    send_receiver_cmd(ReceiverCmd::RemovePeer { block })
}

/// Deny the waiting request `id` and block the sender's address bucket for the rest of this room
/// session: it gets the same `join_denied` as any denied sender and is not asked again; the
/// host sees [`ReceiverUiEvent::Blocked`]. Returns `false` if no receiver runs.
#[frb(sync)]
pub fn deny_and_block_join(id: u64) -> bool {
    send_receiver_cmd(ReceiverCmd::DenyAndBlock(id))
}

/// Take an entry off the block list (`id` from [`ReceiverUiEvent::Blocked`]). Returns `false`
/// if no receiver runs.
#[frb(sync)]
pub fn unblock(id: u64) -> bool {
    send_receiver_cmd(ReceiverCmd::Unblock(id))
}

/// End the room session: the connected sender and everybody who waits are dropped, the code is
/// gone and the stream ends with [`ReceiverUiEvent::RoomEnded`]. Start a new receiver for a new
/// room. Returns `false` if no receiver runs.
#[frb(sync)]
pub fn end_room() -> bool {
    send_receiver_cmd(ReceiverCmd::EndRoom)
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

/// One undecoded H.264 access unit for a hardware decoder of the host (pass-through mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedVideoUnit {
    /// Annex B bytes (start codes), SPS and PPS included on key frames.
    pub data: Vec<u8>,
    /// The unit is an IDR frame.
    pub is_key: bool,
    /// Presentation time in microseconds (RTP clock converted; wraps with the 32 bit RTP time).
    pub pts_us: u64,
}

/// Use the host's own decoder (Android `MediaCodec`) instead of the software decoder: the
/// receiver then does not decode, it queues the H.264 access units and sends
/// [`ReceiverUiEvent::EncodedReady`]; fetch them with [`take_encoded_video`]. Call it before
/// [`start_receiver`] (the choice is kept across receivers); switching while a receiver runs
/// works too (a key frame is requested). `false` goes back to the software decoder.
#[frb(sync)]
pub fn set_video_passthrough(on: bool) {
    VIDEO.set_passthrough(on);
}

/// Ask the sender for a key frame (rate limited to one per second). For a host decoder that lost
/// its place (a new surface, a decoder restart).
#[frb(sync)]
pub fn request_video_keyframe() {
    VIDEO.request_keyframe();
}

/// The encoded access units that wait in pass-through mode, oldest first (at most 8 are ever
/// queued; if the host is too slow the queue is dropped and a key frame requested). Call it
/// after [`ReceiverUiEvent::EncodedReady`]; fetching re-arms that event.
pub async fn take_encoded_video() -> Vec<EncodedVideoUnit> {
    VIDEO
        .take_encoded()
        .into_iter()
        .map(|u| EncodedVideoUnit {
            data: u.data,
            is_key: u.is_key,
            pts_us: u.pts_us,
        })
        .collect()
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
    /// The room needs the host's approval: show a waiting screen with the check number (the
    /// display shows the same number). Nothing is sent until `Joined`.
    WaitingForApproval {
        /// Seconds until the server gives up.
        timeout_secs: u32,
        /// Two-digit check number.
        check: u8,
    },
    /// The join was refused: `join_denied`, `room_locked` or `approval_timeout`. The stream
    /// ends after this.
    JoinRefused {
        /// Server code.
        code: String,
    },
    /// The host removed this sender from the room. The stream ends after this.
    RemovedByHost,
    /// The room was joined and the offer sent. Always the first event unless the room asked
    /// for approval first.
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
    /// Counters of the external H.264 input, sent together with `Streaming` in that mode.
    EncodedStats {
        /// Access units handed to [`push_encoded_frame`].
        pushed: u64,
        /// Units written to the transport.
        sent: u64,
        /// Units dropped (no peer yet, queue full, waiting for a key frame, no SPS/PPS,
        /// replaced by a newer key frame, invalid).
        dropped: u64,
        /// Key frames written to the transport.
        keyframes: u64,
        /// Key frame requests (PLI, new peer, drop policy).
        keyframe_requests: u64,
        /// Bytes written to the transport.
        bytes: u64,
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

/// The server code of a refused join that the UI shows with its own text.
fn join_refusal(e: &TransportError) -> Option<String> {
    match e {
        TransportError::Server { code, .. }
            if matches!(
                code.as_str(),
                "join_denied" | "room_locked" | "approval_timeout"
            ) =>
        {
            Some(code.clone())
        }
        _ => None,
    }
}

/// The event for the old sender API; the connection check code only exists in the presenter
/// session ([`start_presenter`]).
fn map_sender_event(e: SenderEvent) -> Option<SenderUiEvent> {
    Some(match e {
        SenderEvent::State(s) => SenderUiEvent::ConnectionState { state: s.into() },
        SenderEvent::ConnectionCode(_) => return None,
        SenderEvent::Streaming { frames_sent } => SenderUiEvent::Streaming { frames_sent },
        SenderEvent::Finished { frames_sent } => SenderUiEvent::Finished { frames_sent },
        SenderEvent::PeerLeft => SenderUiEvent::PeerLeft,
        SenderEvent::RemovedByHost => SenderUiEvent::RemovedByHost,
        SenderEvent::Error(message) => SenderUiEvent::Error { message },
    })
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
    name: Option<String>,
    events: StreamSink<SenderUiEvent>,
) {
    run_sender(code, server, seconds, udp_bind, screen, None, name, events).await;
}

/// Start a sender whose video the host pushes: join the room `code` on `server` and send the
/// H.264 access units given to [`push_encoded_frame`] (until stopped). Meant for platforms whose
/// own encoder makes the H.264 (Android `MediaCodec`).
///
/// Same event rules as [`start_sender`]; the first event is [`SenderUiEvent::Joined`]. Frames
/// pushed before the WebRTC connection is up are refused ([`PushResult::NotStreaming`]); when it
/// comes up the host is asked for a key frame ([`keyframe_requested`]). The counters arrive once
/// per second as [`SenderUiEvent::EncodedStats`].
pub async fn start_encoded_sender(
    code: String,
    server: String,
    udp_bind: Option<String>,
    name: Option<String>,
    events: StreamSink<SenderUiEvent>,
) {
    let input = ExternalInput::default();
    run_sender(
        code,
        server,
        None,
        udp_bind,
        None,
        Some(input),
        name,
        events,
    )
    .await;
}

/// Parse the room code and only then install the encoded input (closing the previous one). An
/// invalid code installs nothing and leaves no input from an earlier session behind either.
fn accept_code_and_input(
    code: &str,
    external: Option<ExternalInput>,
) -> Result<RoomCode, RoomCodeError> {
    match RoomCode::parse(code) {
        Ok(code) => {
            encoded::replace(external);
            Ok(code)
        }
        Err(e) => {
            encoded::replace(None);
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_sender(
    code: String,
    server: String,
    seconds: Option<u32>,
    udp_bind: Option<String>,
    screen: Option<ScreenRequest>,
    external: Option<ExternalInput>,
    name: Option<String>,
    events: StreamSink<SenderUiEvent>,
) {
    let generation = SENDER.begin();
    let code = match accept_code_and_input(&code, external.clone()) {
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
    let encoded_mode = external.is_some();
    options.external = external.clone();
    options.screen = screen.map(|r| ScreenOptions {
        display: r.display,
        source_id: r.monitor_id,
        fps: r.fps.clamp(1, 60),
        bitrate_bps: r.bitrate_kbps.clamp(100, 50_000).saturating_mul(1000),
        ..ScreenOptions::default()
    });
    options.name = name;
    // Stoppable while the join waits for the host's decision.
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    SENDER.store(stop_tx);
    let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel();
    options.join_progress = Some(ptx);
    let notice_sink = events.clone();
    tokio::spawn(async move {
        while let Some(JoinProgress::Pending {
            timeout_secs,
            check,
            ..
        }) = prx.recv().await
        {
            let _ = notice_sink.add(SenderUiEvent::WaitingForApproval {
                timeout_secs: u32::try_from(timeout_secs).unwrap_or(u32::MAX),
                check,
            });
        }
    });
    let started = tokio::select! {
        r = SenderCore::start(options) => Some(r),
        _ = &mut stop_rx => None,
    };
    let mut core = match started {
        // Stopped (cancelled) while connecting or waiting: dropping the join request
        // withdraws it on the server.
        None => {
            encoded::clear_if(external.as_ref());
            return;
        }
        Some(Ok(core)) => core,
        Some(Err(e)) => {
            let _ = events.add(match join_refusal(&e) {
                Some(code) => SenderUiEvent::JoinRefused { code },
                None => SenderUiEvent::Error {
                    message: e.to_string(),
                },
            });
            encoded::clear_if(external.as_ref());
            return;
        }
    };
    if !SENDER.is_current(generation) || events.add(SenderUiEvent::Joined).is_err() {
        core.stop().await;
        encoded::clear_if(external.as_ref());
        return;
    }
    if !SENDER.is_current(generation) {
        core.stop().await;
        encoded::clear_if(external.as_ref());
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
                        if let Some(ui) = map_sender_event(ev)
                            && events.add(ui).is_err()
                        {
                            core.stop().await;
                            break;
                        }
                        if encoded_mode && is_streaming {
                            let st = core.external_stats();
                            let stats = SenderUiEvent::EncodedStats {
                                pushed: st.pushed,
                                sent: st.sent,
                                dropped: st.dropped() + st.invalid,
                                keyframes: st.keyframes_sent,
                                keyframe_requests: st.keyframe_requests,
                                bytes: st.bytes_sent,
                            };
                            if events.add(stats).is_err() {
                                core.stop().await;
                                break;
                            }
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
            encoded::clear_if(external.as_ref());
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
    encoded::replace(None);
    content_sender::clear_cmd_channel();
    presenter::clear();
}

// ---------------------------------------------------------------------------
// Host-encoded video (Android MediaCodec): push access units, read key frame requests
// ---------------------------------------------------------------------------

/// What happened to an access unit given to [`push_encoded_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushResult {
    /// Queued for sending.
    Queued,
    /// No sender started with [`start_encoded_sender`] is running (or it ended).
    NoSender,
    /// The peer is not connected yet: dropped. Wait for [`keyframe_requested`].
    NotStreaming,
    /// The bounded queue was full: dropped; the next frames are dropped until a key frame.
    QueueFull,
    /// A delta frame while a key frame is awaited after a drop: dropped.
    WaitingForKeyFrame,
    /// A key frame without SPS/PPS and none known yet: dropped. Give the codec config
    /// ([`push_codec_config`]) before the first frame.
    MissingParameterSets,
    /// Not H.264 in Annex B or AVCC form, empty, or too large: dropped.
    Invalid,
}

impl From<PushOutcome> for PushResult {
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

/// Give one H.264 access unit to the sender started with [`start_encoded_sender`].
///
/// `data` is Annex B (start codes) or AVCC (length-prefixed NAL units; both are detected) and is
/// **copied**: Dart keeps its buffer. `is_key` is the encoder's key frame flag (an IDR slice in
/// the data counts too). `pts_us` is the presentation time in microseconds (negative = unknown);
/// it sets the RTP time step. The call never blocks: the queue is bounded (3 units) and a unit
/// that does not fit is dropped and counted, see [`PushResult`] and
/// [`SenderUiEvent::EncodedStats`]. Key frames without SPS/PPS get the ones from
/// [`push_codec_config`] (or seen earlier in the stream) in front.
#[frb(sync)]
pub fn push_encoded_frame(data: Vec<u8>, is_key: bool, pts_us: i64) -> PushResult {
    match encoded::current() {
        Some(input) => input.push(&data, is_key, u64::try_from(pts_us).ok()).into(),
        None => PushResult::NoSender,
    }
}

/// Give the codec configuration (the H.264 SPS/PPS): Annex B, or an `avcC` record. Returns
/// `false` if there is no sender or `data` contains neither an SPS nor a PPS. Copied, like
/// [`push_encoded_frame`].
#[frb(sync)]
pub fn push_codec_config(data: Vec<u8>) -> bool {
    encoded::current().is_some_and(|input| input.set_config(&data))
}

/// True once per key frame request (the peer connected, the peer lost a picture, or frames were
/// dropped): the host should make its encoder produce a sync frame now. Poll it (every 100 ms is
/// plenty; requests are rate limited to one per second). `false` if no encoded sender runs.
#[frb(sync)]
pub fn keyframe_requested() -> bool {
    encoded::current().is_some_and(|input| input.keyframe_requested())
}

// ---------------------------------------------------------------------------
// Screen capture capability and the monitor list
// ---------------------------------------------------------------------------

/// Whether this system can share its screen with this version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSupport {
    /// Capture works; `CaptureInfo::monitors` is filled. On Linux it is X11 capture; on Android
    /// it means "built for Android": the Kotlin capture service of the app does the capture and
    /// its H.264 is pushed with [`push_encoded_frame`]; the single monitor has the id
    /// [`ANDROID_PROJECTION_ID`]. UNCERTAIN: not run on a device, and some Android TV devices
    /// may have no screen capture at all.
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

/// The id of the single entry in the monitor list on Android: the screen that the system's
/// screen capture consent dialog lets the person pick. The Rust core cannot list monitors there.
pub const ANDROID_PROJECTION_ID: &str = "android-projection";

/// What [`capture_info`] answers on Android. Compiled everywhere so a test can check it.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn android_capture_info() -> CaptureInfo {
    CaptureInfo {
        support: CaptureSupport::Supported,
        display: None,
        monitors: vec![CaptureMonitor {
            id: ANDROID_PROJECTION_ID.to_owned(),
            name: "Screen".to_owned(),
            x: 0,
            y: 0,
            // Unknown here: the size is decided by the capture service of the app.
            width: 0,
            height: 0,
        }],
    }
}

/// The id of the single entry in the monitor list on iOS and iPadOS: the screen that the
/// ReplayKit broadcast picker records. The Rust core cannot capture there; the Broadcast Upload
/// Extension (`apps/flutter/ios/BroadcastExtension`, docs/ios.md) does.
pub const IOS_BROADCAST_ID: &str = "ios-broadcast";

/// What [`capture_info`] answers on iOS. Compiled everywhere so a test can check it.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn ios_capture_info() -> CaptureInfo {
    CaptureInfo {
        support: CaptureSupport::Supported,
        display: None,
        monitors: vec![CaptureMonitor {
            id: IOS_BROADCAST_ID.to_owned(),
            name: "Screen".to_owned(),
            x: 0,
            y: 0,
            // Unknown here: decided by the broadcast extension.
            width: 0,
            height: 0,
        }],
    }
}

#[cfg(target_os = "android")]
fn capture_info_blocking(_display: Option<String>) -> CaptureInfo {
    android_capture_info()
}

#[cfg(target_os = "ios")]
fn capture_info_blocking(_display: Option<String>) -> CaptureInfo {
    ios_capture_info()
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

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "ios")))]
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

/// One server offered to the person, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCandidate {
    /// `[scheme://]host:port`, as the server field wants it.
    pub address: String,
    /// Where it came from: `mdm`, `dns-srv`, `display-announcement` or `default-list`.
    pub source: String,
    /// For `display-announcement`: the IPv4 address the announcement resolved to, as text. The
    /// display shows it as `idle.altAddress` (and the QR code uses it) when the announced
    /// server is a host name (the in-house box, decision N40).
    pub ip: Option<String>,
}

/// What went wrong while looking for servers, as a code. The UI shows a translated sentence
/// for each code and never the raw text of `cc-discovery` (it may contain network content).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscoveryWarning {
    /// The policy file of the administrator exists but cannot be used: no server is offered.
    PolicyUnusable,
    /// The policy forbids other servers but names none.
    PolicyNamesNoServer,
    /// The DNS-SRV lookup did not answer in time.
    DnsTimeout,
    /// The DNS-SRV lookup failed or is not available.
    DnsFailed,
    /// The network says that there is no server here (SRV target `.`).
    DnsServiceUnavailable,
    /// Some DNS records were ignored because they did not pass the checks.
    EntriesIgnored,
    /// Listening for display announcements failed.
    AnnouncementFailed,
    /// Any other note.
    Other,
}

/// Map one English warning text of `cc-discovery` to a code.
///
/// UNCERTAIN: `cc-discovery` has no codes yet, so this matches the start of its texts. A text
/// that changes there falls into `Other` (a general note in the UI), never into raw text; the
/// tests below build real warnings with `cc-discovery` to notice such a change.
fn classify_warning(text: &str) -> DiscoveryWarning {
    if text.starts_with("the policy file is unusable") {
        DiscoveryWarning::PolicyUnusable
    } else if text.starts_with("the policy forbids") {
        DiscoveryWarning::PolicyNamesNoServer
    } else if text.starts_with("DNS-SRV lookup for") && text.ends_with("timed out") {
        DiscoveryWarning::DnsTimeout
    } else if text.starts_with("DNS-SRV") && text.ends_with("says the service is not available") {
        DiscoveryWarning::DnsServiceUnavailable
    } else if text.contains("DNS-SRV record(s)") && text.ends_with("were refused") {
        DiscoveryWarning::EntriesIgnored
    } else if text.starts_with("DNS-SRV") {
        DiscoveryWarning::DnsFailed
    } else if text.starts_with("announcement browse failed") {
        DiscoveryWarning::AnnouncementFailed
    } else {
        DiscoveryWarning::Other
    }
}

/// The codes of `warnings`, each once, in order.
fn classify_warnings(warnings: &[String]) -> Vec<DiscoveryWarning> {
    let mut out = Vec::new();
    for w in warnings {
        let c = classify_warning(w);
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

fn candidates(report: &cc_discovery::DiscoveryReport) -> Vec<ServerCandidate> {
    report
        .servers
        .iter()
        .map(|s| ServerCandidate {
            address: s.address.clone(),
            source: s.source.as_str().to_owned(),
            ip: s.ip.map(|ip| ip.to_string()),
        })
        .collect()
}

/// May announcements of display devices be used? Not if a policy forbids them or other servers,
/// or if the policy file is unusable (fail closed).
pub(crate) fn announcements_allowed(report: &cc_discovery::DiscoveryReport) -> bool {
    use cc_discovery::PolicyState;
    match &report.policy {
        PolicyState::Absent => true,
        PolicyState::Active(p) => !p.forbid_announcement && !p.forbid_user_override,
        PolicyState::Invalid(_) => false,
    }
}

/// Server addresses the UI offers when the person has not chosen one yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerDefaults {
    /// The servers `cc-discovery` knows without asking the network, best first: the servers of
    /// an MDM policy, then the built-in list (`DEFAULT_SERVERS`) unless the policy forbids it.
    pub servers: Vec<ServerCandidate>,
    /// A policy forbids other servers, or the policy file is unusable: the server field must
    /// be read-only and only `servers` may be used.
    pub locked: bool,
    /// Display announcements may be used (see [`ServerDiscovery::announcements_allowed`]).
    pub announcements_allowed: bool,
    /// Notes about the policy file, as codes.
    pub notices: Vec<DiscoveryWarning>,
    /// Address for development on one machine (a local `cometcast-server`).
    pub dev_fallback: String,
}

/// The development fallback, offered last (not when `locked`).
const DEV_FALLBACK: &str = "127.0.0.1:8443";

fn server_defaults_with(opts: &cc_discovery::DiscoverOptions) -> ServerDefaults {
    let r = cc_discovery::discover_servers_offline(opts);
    ServerDefaults {
        servers: candidates(&r),
        locked: r.locked,
        announcements_allowed: announcements_allowed(&r),
        notices: classify_warnings(&r.warnings),
        dev_fallback: DEV_FALLBACK.to_owned(),
    }
}

/// The server addresses to pre-fill, found without network access (fast): the policy file of
/// the operating system (MDM) first, then the built-in list. DNS-SRV and announcements need the
/// network and are in [`discover_servers`].
///
/// UNCERTAIN: the host in the built-in list (`relay.cometcast.de`) does not exist yet, which is
/// why the UI also offers `dev_fallback` (unless `locked`).
#[frb(sync)]
pub fn server_defaults() -> ServerDefaults {
    server_defaults_with(&cc_discovery::DiscoverOptions::default())
}

/// The result of asking all sources for servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerDiscovery {
    /// Best first (MDM, DNS-SRV, announcement, built-in list).
    pub servers: Vec<ServerCandidate>,
    /// A policy forbids other servers, or the policy file is unusable.
    pub locked: bool,
    /// Display announcements may be used: the UI may offer the receivers it finds nearby as a
    /// way to pick a server. False under a policy that forbids announcements or other servers.
    pub announcements_allowed: bool,
    /// Notes about sources that failed, as codes the UI translates.
    pub notices: Vec<DiscoveryWarning>,
    /// The same notes as English text for logs. Not for the UI: it may contain network content.
    pub warnings: Vec<String>,
}

/// Ask all sources for servers: policy file, DNS-SRV and announcements by display devices (it
/// listens on the network for a moment, so this takes some seconds; the time limits are those
/// of `cc_discovery::DiscoverOptions`). Never fails; sources that do not work leave a warning.
pub async fn discover_servers() -> ServerDiscovery {
    discover_servers_with(&cc_discovery::DiscoverOptions::default()).await
}

async fn discover_servers_with(opts: &cc_discovery::DiscoverOptions) -> ServerDiscovery {
    let r = cc_discovery::discover_servers(opts).await;
    ServerDiscovery {
        servers: candidates(&r),
        locked: r.locked,
        announcements_allowed: announcements_allowed(&r),
        notices: classify_warnings(&r.warnings),
        warnings: r.warnings,
    }
}

// ---------------------------------------------------------------------------
// Announcing this receiver, and listing receivers nearby
// ---------------------------------------------------------------------------

/// Result of [`start_advertising`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvertiseOutcome {
    /// The receiver is announced on the local network.
    Started,
    /// The name or the server was refused by the checks of `cc-discovery` (for example a
    /// loopback or public IP address as server).
    Refused,
    /// The network could not be used (no multicast, no interface).
    Failed,
    /// A stop or a newer start came first; nothing is announced.
    Superseded,
}

struct AdvertiseSlot {
    generation: AtomicU64,
    current: Mutex<Option<cc_discovery::Advertisement>>,
}

static ADVERTISING: AdvertiseSlot = AdvertiseSlot {
    generation: AtomicU64::new(0),
    current: Mutex::new(None),
};

impl AdvertiseSlot {
    /// Withdraw the running announcement and invalidate starts that are still being set up.
    fn begin(&self) -> u64 {
        let g = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        // Dropping the advertisement withdraws the service; do it outside the lock.
        let old = self
            .current
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        drop(old);
        g
    }

    /// Keep `ad` if no stop or newer start happened; otherwise it is dropped (withdrawn).
    fn store(&self, generation: u64, ad: cc_discovery::Advertisement) -> bool {
        let mut slot = self.current.lock().unwrap_or_else(|p| p.into_inner());
        if self.generation.load(Ordering::SeqCst) != generation {
            return false;
        }
        *slot = Some(ad);
        true
    }
}

/// Announce this receiver on the local network (DNS-SD `_cometcast._tcp`) with the friendly
/// `name` and, if given, its preferred server (`host:port`, or with a scheme). The room code is
/// never announced. A running announcement is replaced. Call [`stop_advertising`] when the
/// receiver view closes.
pub async fn start_advertising(name: String, preferred_server: Option<String>) -> AdvertiseOutcome {
    let generation = ADVERTISING.begin();
    let opts = cc_discovery::AdvertiseOptions {
        instance_name: name,
        // UNCERTAIN: receivers do not listen for senders yet; the port only has to exist.
        port: 9,
        preferred_server,
        host_label: None,
    };
    let built = tokio::task::spawn_blocking(move || cc_discovery::advertise_receiver(&opts)).await;
    match built {
        Ok(Ok(ad)) => {
            if ADVERTISING.store(generation, ad) {
                AdvertiseOutcome::Started
            } else {
                AdvertiseOutcome::Superseded
            }
        }
        Ok(Err(e)) => classify_advertise_error(&e.0),
        Err(_) => AdvertiseOutcome::Failed,
    }
}

/// `cc-discovery` reports input errors and daemon errors with the same type; the daemon ones
/// come from `mdns-sd`. UNCERTAIN: this is a guess from the message text.
fn classify_advertise_error(text: &str) -> AdvertiseOutcome {
    const INPUT_ERRORS: [&str; 3] = ["instance name", "server cannot be announced", "host label"];
    if INPUT_ERRORS.iter().any(|p| text.contains(p)) || text.contains("address") {
        AdvertiseOutcome::Refused
    } else {
        AdvertiseOutcome::Failed
    }
}

/// Withdraw the announcement of [`start_advertising`] (does nothing if none runs) and cancel a
/// start that is still being set up.
pub fn stop_advertising() {
    ADVERTISING.begin();
}

/// A change in the receivers found on the local network by [`watch_receivers`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearbyReceiverEvent {
    /// A receiver appeared or changed. All text is cleaned and bounded by `cc-discovery`.
    Found {
        /// The DNS-SD instance name; identifies the receiver in `Lost`.
        id: String,
        /// Friendly name of the display device.
        name: String,
        /// The server it prefers (`[scheme://]host:port`), if it announced a valid one. It is
        /// not trusted: the person decides. The room code is never part of an announcement.
        server: Option<String>,
    },
    /// A receiver went away.
    Lost {
        /// The instance name of the receiver.
        id: String,
    },
    /// Listening is not possible here (no multicast); the stream ends after this.
    Unavailable,
}

static WATCH: Slot = Slot::new();

/// List the receivers on the local network (mDNS) until [`stop_watching_receivers`] is called or
/// Dart stops listening. A newer call replaces the running one.
pub async fn watch_receivers(events: StreamSink<NearbyReceiverEvent>) {
    let generation = WATCH.begin();
    let mut browser = match cc_discovery::discover_receivers() {
        Ok(b) => b,
        Err(_) => {
            let _ = events.add(NearbyReceiverEvent::Unavailable);
            return;
        }
    };
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    WATCH.store(stop_tx);
    // A stop between `begin` and `store` found an empty slot: check once more.
    if !WATCH.is_current(generation) {
        return;
    }
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                ev = browser.next() => {
                    let Some(ev) = ev else { break };
                    let out = match ev {
                        cc_discovery::ReceiverEvent::Found(r) => NearbyReceiverEvent::Found {
                            id: r.instance,
                            name: r.name,
                            server: r.announced_server,
                        },
                        cc_discovery::ReceiverEvent::Lost(id) => NearbyReceiverEvent::Lost { id },
                    };
                    // Dart stopped listening: stop browsing.
                    if events.add(out).is_err() {
                        break;
                    }
                }
            }
        }
        // `browser` is dropped here: the browse stops. `events` is dropped: the stream ends.
    });
}

/// Stop [`watch_receivers`] (does nothing if none runs).
pub fn stop_watching_receivers() {
    WATCH.stop();
}

// ---------------------------------------------------------------------------
// Screens on this network (spec 14): the display announces, the presenter app lists
// ---------------------------------------------------------------------------

/// Whether the display announces itself now (result of [`set_display_announcing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnounceStatus {
    /// A record with the current code is published (or will be as soon as the room exists).
    Announcing,
    /// The UI said the display must not announce (setup, offline, background).
    Off,
    /// Allowed, but no room exists yet (the announcement starts with the room).
    WaitingForRoom,
    /// The administrator's policy forbids announcements (or the policy file is unusable).
    ForbiddenByPolicy,
    /// The instance id is not 8 to 32 lower-case hex digits.
    BadId,
}

/// Tell the core whether the display may announce itself on the local network.
///
/// `active` is true while the display shows Idle or Presenting in the foreground and false in
/// setup, offline, in the background and on every other page; the announcement is withdrawn
/// (goodbye) when it turns false. `instance_id` is the stable random id of this installation
/// (8 to 32 lower-case hex digits, created and persisted by the app, new after a data reset).
/// What is announced (server, room name, join mode, the current code; no code while Locked)
/// is taken from the running receiver ([`start_receiver`]), so it cannot differ from what the
/// screen shows, and it is updated at once when the mode changes. A policy that forbids
/// announcements wins over `active`.
pub async fn set_display_announcing(active: bool, instance_id: String) -> AnnounceStatus {
    match nearby::set_enabled(active, instance_id) {
        nearby::Wanted::Yes => AnnounceStatus::Announcing,
        nearby::Wanted::NotEnabled => AnnounceStatus::Off,
        nearby::Wanted::NoReceiver => AnnounceStatus::WaitingForRoom,
        nearby::Wanted::Forbidden => AnnounceStatus::ForbiddenByPolicy,
        nearby::Wanted::BadId => AnnounceStatus::BadId,
    }
}

/// Who may join a screen found nearby.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearbyMode {
    /// Connects at once.
    Open,
    /// The person at the screen has to approve.
    Approval,
    /// Locked right now (no code is announced).
    Locked,
}

impl From<cc_discovery::ScreenMode> for NearbyMode {
    fn from(m: cc_discovery::ScreenMode) -> Self {
        match m {
            cc_discovery::ScreenMode::Open => Self::Open,
            cc_discovery::ScreenMode::Approval => Self::Approval,
            cc_discovery::ScreenMode::Locked => Self::Locked,
        }
    }
}

/// One screen on the network, checked and cleaned by `cc-discovery` (docs/discovery.md,
/// section 3). Nothing in it is trusted: in particular the server is only a suggestion until
/// the person has confirmed it (spec 14.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearbyScreenInfo {
    /// The stable instance id of the screen; rows are merged by it.
    pub id: String,
    /// The room name, cleaned, at most 32 characters; empty when the screen has none.
    pub name: String,
    /// The announced server with scheme (`ws://host:port` or `wss://host:port`).
    pub server: String,
    /// Who may join.
    pub mode: NearbyMode,
    /// The current code (6 characters); `None` exactly when the mode is `Locked`.
    pub code: Option<String>,
}

impl From<cc_discovery::NearbyScreen> for NearbyScreenInfo {
    fn from(s: cc_discovery::NearbyScreen) -> Self {
        Self {
            id: s.id,
            name: s.name,
            server: s.server,
            mode: s.mode.into(),
            code: s.code,
        }
    }
}

/// The state of the system permission to search the local network (iOS and iPadOS: Local
/// Network; macOS 15 and later may ask the same way). Reported by the native bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearbyPermission {
    /// The platform needs a permission and the person was not asked yet.
    NotAsked,
    /// Searching is allowed (or the platform needs no permission).
    Granted,
    /// The person refused (Apple: `kDNSServiceErr_PolicyDenied`).
    Denied,
}

/// What a search for screens on the network reports ([`watch_nearby_screens`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearbyUiEvent {
    /// The search runs. The first event of every stream.
    SearchStarted,
    /// The search ended (the last event after [`stop_nearby_screens`]).
    SearchStopped,
    /// A screen appeared. It goes to the end of the list.
    Added {
        /// The screen.
        screen: NearbyScreenInfo,
    },
    /// A screen changed (code, mode, name or server); it keeps its place.
    Updated {
        /// The screen.
        screen: NearbyScreenInfo,
    },
    /// A screen left: goodbye, no refresh for 30 s, or it announced something unusable.
    Removed {
        /// The id of the screen.
        id: String,
    },
    /// The state of the system permission changed (from the native bridge).
    Permission {
        /// The new state.
        state: NearbyPermission,
    },
    /// Searching is not possible here (no multicast, no interface); the stream ends.
    Unavailable,
    /// The administrator's policy forbids announcements (or locks the server): there is no
    /// search; the stream ends.
    Forbidden,
}

static NEARBY: Slot = Slot::new();

/// Search for screens on the local network until [`stop_nearby_screens`] is called or Dart
/// stops listening. A newer call replaces the running search.
///
/// `native` is true on platforms whose own browse API finds the screens (Android
/// `NsdManager`, iOS and iPadOS `NWBrowser`): the bridge then reports what it sees with
/// [`nearby_native_seen`], [`nearby_native_gone`] and [`nearby_native_permission`], and this
/// function only checks and keeps the list. With `native` false (Linux, Windows, macOS) the
/// core browses itself (`mdns-sd`).
///
/// Rules of the list (spec 14.3, 14.8; code in `cc_discovery::nearby`): rows are keyed by the
/// instance id, new rows come at the end, a row is removed on goodbye or after 30 s without
/// refresh, at most 32 rows, every field is cleaned and checked (server like any announced
/// server, code of 6 characters from the code alphabet, other layout versions dropped). The
/// search never picks a screen and never saves a server.
pub async fn watch_nearby_screens(native: bool, events: StreamSink<NearbyUiEvent>) {
    let generation = NEARBY.begin();
    let allowed = announcements_allowed(&cc_discovery::discover_servers_offline(
        &cc_discovery::DiscoverOptions::default(),
    ));
    if !allowed {
        let _ = events.add(NearbyUiEvent::Forbidden);
        return;
    }
    let mut search = if native {
        cc_discovery::NearbySearch::new()
    } else {
        match cc_discovery::NearbySearch::with_mdns() {
            Ok(s) => s,
            Err(_) => {
                let _ = events.add(NearbyUiEvent::Unavailable);
                return;
            }
        }
    };
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    NEARBY.store(stop_tx);
    // A stop between `begin` and `store` found an empty slot: check once more.
    if !NEARBY.is_current(generation) {
        return;
    }
    nearby::set_feed(generation, search.sink(), events.clone());
    if events.add(NearbyUiEvent::SearchStarted).is_err() {
        nearby::clear_feed(generation);
        return;
    }
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    let _ = events.add(NearbyUiEvent::SearchStopped);
                    break;
                }
                c = search.next() => {
                    let Some(c) = c else { break };
                    let out = match c {
                        cc_discovery::NearbyChange::Added(s) => NearbyUiEvent::Added { screen: s.into() },
                        cc_discovery::NearbyChange::Updated(s) => NearbyUiEvent::Updated { screen: s.into() },
                        cc_discovery::NearbyChange::Removed(id) => NearbyUiEvent::Removed { id },
                    };
                    if events.add(out).is_err() {
                        break;
                    }
                }
            }
        }
        nearby::clear_feed(generation);
        // `search` is dropped here: the browse stops. `events` is dropped: the stream ends.
    });
}

/// Stop [`watch_nearby_screens`] (does nothing if none runs). The stream ends with
/// [`NearbyUiEvent::SearchStopped`].
pub fn stop_nearby_screens() {
    NEARBY.stop();
}

/// A native bridge saw (or saw again) a `_cometcast._tcp` service. `instance` is the service
/// instance name, `addresses` the resolved IP addresses as text (may be empty: Apple's browser
/// does not resolve), `txt` the TXT entries. Everything is checked here. Ignored when no
/// search runs.
#[frb(sync)]
pub fn nearby_native_seen(instance: String, addresses: Vec<String>, txt: Vec<(String, String)>) {
    nearby::native_seen(instance, addresses, txt);
}

/// A native bridge lost a service (goodbye or the platform dropped it).
#[frb(sync)]
pub fn nearby_native_gone(instance: String) {
    nearby::native_gone(instance);
}

/// The native browse restarted: forget every row (the bridge reports what it sees again).
#[frb(sync)]
pub fn nearby_native_reset() {
    nearby::native_reset();
}

/// The native bridge reports the state of the system permission.
#[frb(sync)]
pub fn nearby_native_permission(state: NearbyPermission) {
    nearby::native_event(NearbyUiEvent::Permission { state });
}

/// Does this machine have a local network (a non-loopback interface with a private address)?
/// Used by the computers' pages for the state "No local network"; phones and tablets ask the
/// platform instead (`ConnectivityManager`, `NWPathMonitor`).
#[frb(sync)]
pub fn local_network_available() -> bool {
    cc_discovery::local_network_available()
}

// ---------------------------------------------------------------------------
// Content: pictures of received content, link check, PDF worker
// ---------------------------------------------------------------------------

/// The image or PDF page that waits, or `None`. Call it after a
/// [`ReceiverUiEvent::ContentFrameReady`]. Fetching re-arms that event; a newer picture
/// replaces an unfetched one (only the latest matters), and a link, a stop or a new document
/// empties the slot. The bytes are copied once here and once by the bridge.
pub async fn take_content_frame() -> Option<ContentFrame> {
    CONTENT.take().map(content::Stored::into_frame)
}

/// Tell the receiver how wide pages are rendered, in physical pixels (the width of the view
/// times the device pixel ratio). The shown page is rendered again at the new width and
/// arrives as a `ContentFrameReady`. Call it when the view is resized. The core clamps the
/// value to its output limits.
#[frb(sync)]
pub fn set_content_target_width(width: u32) {
    content::set_target_width(width);
}

/// The result of [`validate_link`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCheck {
    /// An absolute http or https link without user name or password, no control or
    /// whitespace characters, at most 2048 characters as written and as normalised.
    Valid {
        /// The normalised URL (ASCII), the one to open.
        url: String,
        /// Host in its punycode form.
        host: String,
    },
    /// Not acceptable.
    Invalid,
}

/// The one place where link rules live (`cc-files`, the same rules that the receiver applies
/// to a received link): the Dart side asks here and does not repeat the rules. Call it
/// before sending a link and again right before opening one.
#[frb(sync)]
pub fn validate_link(input: String) -> LinkCheck {
    content::check_link(&input)
}

/// Use this program to render PDFs (an absolute path) instead of looking for one; `None` goes
/// back to the default search. For tests and for hosts whose layout differs. Only pass a path
/// the app itself controls. It must pass the same trust check as the Office worker (`cc-docs::check_worker_file`: absolute,
/// owned by this user or root, not writable by others); otherwise there is no worker.
#[frb(sync)]
pub fn set_docs_worker(path: Option<String>) {
    content::set_worker_override(path.map(std::path::PathBuf::from));
}

/// Use this program to convert Office files instead of looking for one; it must be an
/// **absolute** path (a bare or relative name is not looked up: there is then no worker) and
/// pass the ownership and permission check of `cc-docs` (owned by this user or root, not
/// writable by group or others, in a directory not writable by others). `None` goes back to
/// the default search, whose result is checked the same way. For tests and for hosts whose
/// layout differs.
#[frb(sync)]
pub fn set_office_worker(path: Option<String>) {
    office::set_worker_override(path.map(std::path::PathBuf::from));
}

/// The Office conversion worker program (`cc-office-worker`) that would be used now, or
/// `None` when none was found (then Office files are refused with
/// [`UiSendRejection::OfficeUnavailable`]). Search order: [`set_office_worker`]; a file
/// `cc-office-worker` in the directory of the running executable (or one directory up from a
/// Cargo `deps` directory). No environment variable and no `PATH` search.
#[frb(sync)]
pub fn office_worker_path() -> Option<String> {
    office::worker_path()
}

/// The PDF render worker program that would be used now, or `None` when none was found (then
/// received PDFs fail with [`UiContentError::WorkerUnavailable`], and nothing falls back to
/// the unprotected in-process engine). Search order: [`set_docs_worker`]; the environment
/// variable `COMETCAST_DOCS_WORKER` (only if `cc-docs` trusts the file); `cc-docs-worker` in
/// the directory of the running executable; `cc-docs`' own discovery.
#[frb(sync)]
pub fn docs_worker_path() -> Option<String> {
    content::worker_path()
}

// ---------------------------------------------------------------------------
// Content sender: an image, a PDF or a link, then page commands
// ---------------------------------------------------------------------------

/// What the person wants to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentSource {
    /// A file on this computer: an image (`.png .jpg .jpeg .webp .gif`, at most 10 MiB), a
    /// PDF (`.pdf`, at most 64 MiB) or an Office file (`.docx .pptx .xlsx`, at most 50 MiB,
    /// converted to a PDF here first; event `Converting`). Rust reads it; the bytes do not
    /// cross the bridge.
    File {
        /// Path of the file.
        path: String,
    },
    /// A web link; Rust validates it ([`validate_link`] rules).
    Link {
        /// The link as typed.
        url: String,
    },
}

/// The kind of content that is being sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiContentKind {
    /// An image.
    Image,
    /// A PDF document.
    Pdf,
    /// A link.
    Link,
}

/// Why something cannot be sent. Found before anything is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSendRejection {
    /// The file cannot be opened or read.
    Unreadable,
    /// The path is not a regular file (a folder, a device).
    NotARegularFile,
    /// The file has no bytes.
    Empty,
    /// Larger than the limit (10 MiB image, 64 MiB PDF).
    TooLarge,
    /// Not one of the allowed image types or PDF (by extension and by content).
    UnsupportedType,
    /// Named like a PDF but without the PDF header.
    NotPdf,
    /// The PDF cannot be shown by any receiver (password, damaged, too many pages, ...).
    DocumentUnusable,
    /// Not an acceptable http(s) link.
    BadLink,
    /// An Office file (`.docx .pptx .xlsx`) cannot be converted on this system: the conversion
    /// worker program is missing or the build has no conversion engine.
    OfficeUnavailable,
    /// An old binary Office file or one protected by a password (save it as `.docx`, `.pptx`
    /// or `.xlsx` without a password).
    OfficeEncrypted,
    /// The Office file is damaged, refused by the safety checks (container, size after
    /// unpacking), could not be converted or took too long (120 s).
    OfficeFailed,
}

/// What the content sender tells the UI. Order: `Prepared`, `Joined`, `Connected`,
/// `Progress`..., `Sent`; a problem ends the stream with `Rejected`, `Error`, `PeerLeft` or
/// `Stopped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentSenderUiEvent {
    /// The file or link was read and checked. Always the first event.
    Prepared {
        /// File name, or the host of a link.
        name: String,
        /// Image, PDF or link.
        kind: UiContentKind,
        /// Size in bytes (the length of the URL for a link).
        size: u64,
        /// Pages of a PDF, if the render worker could tell. `None` = unknown.
        pages: Option<u32>,
        /// Host of a link in its punycode form.
        host: Option<String>,
    },
    /// An Office file is being converted to a PDF (takes seconds, up to 120 s; runs in a
    /// child process). Comes before `Prepared` and only for `.docx .pptx .xlsx`; a failure
    /// ends the stream with `Rejected`.
    Converting {
        /// The file name as picked.
        name: String,
    },
    /// The room needs the host's approval: show a waiting screen with the check number.
    /// Nothing is sent until `Joined`.
    WaitingForApproval {
        /// Seconds until the server gives up.
        timeout_secs: u32,
        /// Two-digit check number (the display shows the same number).
        check: u8,
    },
    /// The join was refused: `join_denied`, `room_locked` or `approval_timeout`. The stream
    /// ends after this.
    JoinRefused {
        /// Server code.
        code: String,
    },
    /// The host removed this sender. The stream ends after this.
    RemovedByHost,
    /// The room was joined and the offer sent.
    Joined,
    /// The WebRTC connection state changed.
    ConnectionState {
        /// New state.
        state: UiConnectionState,
    },
    /// The data channel is open; the transfer starts.
    Connected,
    /// Bytes handed to the data channel (at most one report per percent). The channel buffers
    /// up to about 1 MiB, so the receiver may be that far behind.
    Progress {
        /// Bytes so far.
        sent: u64,
        /// Bytes in total.
        total: u64,
    },
    /// Everything was delivered (the transport confirmed it). Commands can follow.
    Sent,
    /// A stop command was delivered; the sender ended. The stream ends after this.
    Stopped,
    /// The receiver left. The stream ends after this.
    PeerLeft,
    /// The content was refused before connecting. The stream ends after this.
    Rejected {
        /// Why.
        reason: UiSendRejection,
    },
    /// Something failed (unknown room, connection). The stream ends after this.
    Error {
        /// Technical message in English.
        message: String,
    },
}

/// A command for the presenter view of a PDF, sent to the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiContentCmd {
    /// Next page.
    NextPage,
    /// Previous page.
    PrevPage,
    /// Go to a page (the first page is 1; the receiver clamps to the page count).
    GoToPage {
        /// Page number, 1-based.
        page: u32,
    },
    /// Stop showing the content; the sender ends after the command left.
    Stop,
}

/// Send an image, a PDF or a link: check it (events: `Prepared` or `Rejected`), join the room
/// `code` on `server`, wait for the data channel, send with progress, and then forward
/// [`send_content_cmd`] commands until stopped. Replaces a running sender.
///
/// Errors are events, not return values (see [`start_receiver`]). `udp_bind` is for
/// development only.
pub async fn start_content_sender(
    code: String,
    server: String,
    udp_bind: Option<String>,
    source: ContentSource,
    name: Option<String>,
    events: StreamSink<ContentSenderUiEvent>,
) {
    let generation = SENDER.begin();
    encoded::replace(None);
    content_sender::clear_cmd_channel();
    let code = match RoomCode::parse(&code) {
        Ok(code) => code,
        Err(e) => {
            let _ = events.add(ContentSenderUiEvent::Error {
                message: e.to_string(),
            });
            return;
        }
    };
    let worker = content::worker();
    // An Office file takes a while to convert: say so first. A person who stops now is
    // noticed after the conversion (the child process runs to its end or its deadline; its
    // result is dropped).
    if let ContentSource::File { path } = &source {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if office::is_office_name(&name)
            && events
                .add(ContentSenderUiEvent::Converting { name })
                .is_err()
        {
            return;
        }
    }
    // One Office conversion at a time; a request that was superseded while it waited is
    // dropped without converting.
    let permit = match &source {
        ContentSource::File { path } if office::is_office_name(path) => {
            let p = office::GATE.acquire().await;
            if !SENDER.is_current(generation) {
                return;
            }
            Some(p)
        }
        _ => None,
    };
    let office_options = office::options();
    let prepared = match tokio::task::spawn_blocking(move || {
        // Held until the conversion (the child process) has ended, even if this task is
        // dropped meanwhile.
        let _permit = permit;
        content_sender::prepare(&source, &worker, &office_options)
    })
    .await
    {
        Ok(Ok(p)) => p,
        Ok(Err(reason)) => {
            let _ = events.add(ContentSenderUiEvent::Rejected { reason });
            return;
        }
        Err(_) => {
            let _ = events.add(ContentSenderUiEvent::Error {
                message: "internal error: reading the content failed".into(),
            });
            return;
        }
    };
    if !SENDER.is_current(generation)
        || events
            .add(ContentSenderUiEvent::Prepared {
                name: prepared.name.clone(),
                kind: prepared.kind,
                size: prepared.size,
                pages: prepared.pages,
                host: prepared.host.clone(),
            })
            .is_err()
    {
        return;
    }
    let mut options = ContentSenderOptions::new(server, code);
    if let Some(b) = udp_bind {
        options.session.udp_bind = vec![b];
    }
    options.name = name;
    // Stoppable while the join waits for the host's decision.
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    SENDER.store(stop_tx);
    let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel();
    options.join_progress = Some(ptx);
    let notice_sink = events.clone();
    tokio::spawn(async move {
        while let Some(JoinProgress::Pending {
            timeout_secs,
            check,
            ..
        }) = prx.recv().await
        {
            let _ = notice_sink.add(ContentSenderUiEvent::WaitingForApproval {
                timeout_secs: u32::try_from(timeout_secs).unwrap_or(u32::MAX),
                check,
            });
        }
    });
    let started = tokio::select! {
        r = ContentSender::start(options) => Some(r),
        _ = &mut stop_rx => None,
    };
    let sender = match started {
        None => return,
        Some(Ok(s)) => s,
        Some(Err(e)) => {
            let _ = events.add(match join_refusal(&e) {
                Some(code) => ContentSenderUiEvent::JoinRefused { code },
                None => ContentSenderUiEvent::Error {
                    message: e.to_string(),
                },
            });
            return;
        }
    };
    if !SENDER.is_current(generation) || events.add(ContentSenderUiEvent::Joined).is_err() {
        sender.stop().await;
        return;
    }
    if !SENDER.is_current(generation) {
        sender.stop().await;
        return;
    }
    let (token, cmds) = content_sender::new_cmd_channel();
    let panic_sink = events.clone();
    spawn_guarded(
        content_sender::drive(sender, prepared, events, stop_rx, cmds, token),
        move || {
            let _ = panic_sink.add(ContentSenderUiEvent::Error {
                message: "internal error: the content sender task panicked".into(),
            });
        },
    );
}

/// Queue a presenter command for the running content sender, or for the running presenter
/// session ([`start_presenter`]; there `Stop` ends the presentation and keeps the join).
/// Returns `false` if nothing runs (or the content has not been sent yet), the command queue
/// (64) is full, or the page is 0. Commands are not applied locally: the page that is shown
/// is the receiver's business, the sender has no renderer.
#[frb(sync)]
pub fn send_content_cmd(cmd: UiContentCmd) -> bool {
    if presenter::running() {
        return presenter::cmd_from_ui(cmd).is_some_and(presenter::send);
    }
    content_sender::send_cmd(cmd)
}

// ---------------------------------------------------------------------------
// Presenter session: join once, present several times (native-apps.md 20.1)
// ---------------------------------------------------------------------------

/// What the presenter session tells the UI.
///
/// Order: `WaitingForApproval` (only where the room asks), `Joined`, then per presentation
/// `Presenting`, `ConnectionCode`, (`Page`, `Progress`...), `PresentingStopped`. The stream ends
/// after `Ended` and after every `Error` except the ones of a presentation that could not
/// start (`capture_denied`, `capture_unavailable`: the session stays joined, the person is back
/// at "What to show"). `Error.code` is a protocol code (`room_not_found`, `join_denied`,
/// `room_locked`, `room_locked_waiting`, `approval_timeout`, `removed_by_host`, `room_full`,
/// `rate_limited`, `banned`, `server_full`, `proto_mismatch`, `bad_hello`, `bad_message`,
/// `forbidden`, `altcha_*`, `wrong_role`, `not_in_room`) or a local one: `connection_lost`
/// (could not reach or lost the server, timeouts included), `peer_failed` (WebRTC failed),
/// `ice_refused` / `ice_failed`, `capture_denied`, `capture_unavailable`, `unknown`.
/// The apps map the code, never the English `message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenterUiEvent {
    /// The room needs the host's approval: show the waiting page with the check number (the
    /// display shows the same number). Nothing is presented until `Joined`.
    WaitingForApproval {
        /// Seconds until the server gives up.
        timeout_secs: u32,
        /// Two-digit check number.
        check: u8,
        /// The name the display gave its room, if any (decision 49: lenient).
        room_name: Option<String>,
    },
    /// The room was joined (the host approved once, or the room is open).
    Joined {
        /// The name the display gave its room, if any.
        room_name: Option<String>,
    },
    /// The connection check code of the running presentation, `"123 456"`, once per connection;
    /// the display shows the same digits.
    ConnectionCode {
        /// The six digits with a space in the middle.
        sas: String,
    },
    /// The display shows this PDF page of that many.
    Page {
        /// The shown page, from 1.
        page: u32,
        /// The page count, from 1.
        pages: u32,
    },
    /// An Office file is being converted to a PDF (takes seconds; child process). Comes before
    /// `ContentPrepared`; a failure is `ContentRejected`.
    Converting {
        /// The file name as picked.
        name: String,
    },
    /// The file or link was read and checked.
    ContentPrepared {
        /// File name, or the host of a link.
        name: String,
        /// Image, PDF or link.
        kind: UiContentKind,
        /// Size in bytes.
        size: u64,
        /// Pages of a PDF, if known.
        pages: Option<u32>,
        /// Host of a link in its punycode form.
        host: Option<String>,
    },
    /// The content was refused before anything was sent. The session stays joined.
    ContentRejected {
        /// Why.
        reason: UiSendRejection,
    },
    /// Bytes handed to the data channel, at most one report per percent.
    Progress {
        /// Bytes so far.
        sent: u64,
        /// Bytes in total.
        total: u64,
    },
    /// The content was delivered (the transport confirmed it). Page commands can follow.
    ContentSent,
    /// The answer to [`request_handover`]: the display holds the token (`ok`), or it did not
    /// answer in time (an old display, a web display, a lost connection). Either way the app
    /// goes on: with the token the extension is let in without a dialog, without it the host
    /// is asked as for any join.
    HandoverResult {
        /// The display answered `handover_ok`.
        ok: bool,
    },
    /// The presentation is connected: media flows, or the channel is open.
    Presenting,
    /// The presentation ended and the session is still joined (on request, or because the system
    /// ended the capture). The display is Idle with the same code.
    PresentingStopped,
    /// The session is over: `removed_by_host` (the host removed this sender) or `display_left`
    /// (the room is gone). The stream ends after this.
    Ended {
        /// `removed_by_host` or `display_left`.
        code: String,
    },
    /// Something failed; see the type's documentation for the codes and for which ones end the
    /// stream.
    Error {
        /// The code the app maps.
        code: String,
        /// Technical message in English (logs only).
        message: String,
    },
}

/// Join the room `code` on `server` **once** (on "Connect"): the host approves once where the
/// room asks for it (event `WaitingForApproval`, then `Joined`). Then call [`present_screen`],
/// [`present_encoded`] or [`present_content`]; [`stop_presenting`] ends only the presentation,
/// [`stop_presenter`] leaves. Replaces a running sender or presenter session.
///
/// Errors are events, not return values (see [`start_receiver`]); a code that is not a room
/// code is reported as `room_not_found`. `handover` is a single-use token that the display issued
/// to this person earlier ([`request_handover`], native-apps.md 20.5): the display admits the
/// join without a dialog; `None` (or a token the display does not know) is an ordinary join.
/// `udp_bind` is for development only.
pub async fn start_presenter(
    code: String,
    server: String,
    name: Option<String>,
    handover: Option<String>,
    udp_bind: Option<String>,
    events: StreamSink<PresenterUiEvent>,
) {
    let generation = SENDER.begin();
    encoded::replace(None);
    content_sender::clear_cmd_channel();
    presenter::clear();
    let code = match RoomCode::parse(&code) {
        Ok(code) => code,
        Err(e) => {
            let _ = events.add(PresenterUiEvent::Error {
                code: "room_not_found".into(),
                message: e.to_string(),
            });
            return;
        }
    };
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    SENDER.store(stop_tx);
    let join = presenter::Join {
        code,
        server,
        name,
        handover,
        udp_bind,
    };
    let Some(session) = presenter::join(join, &events, &mut stop_rx).await else {
        return;
    };
    if !SENDER.is_current(generation)
        || events
            .add(PresenterUiEvent::Joined {
                room_name: session.room_name().map(str::to_owned),
            })
            .is_err()
    {
        session.leave().await;
        return;
    }
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    presenter::install(cmd_tx);
    let panic_sink = events.clone();
    spawn_guarded(
        presenter::drive(session, events, stop_rx, cmd_rx),
        move || {
            let _ = panic_sink.add(PresenterUiEvent::Error {
                code: "unknown".into(),
                message: "internal error: the presenter task panicked".into(),
            });
        },
    );
}

/// Leave the room (Disconnect; withdraw while waiting; the app ends). Does nothing if no
/// presenter session runs. The Dart stream then ends.
pub fn stop_presenter() {
    SENDER.stop();
    presenter::clear();
    encoded::replace(None);
}

/// Share the screen in the joined presenter session. `Some(request)`: the X11 screen with these
/// settings. `None`: the system picker of the desktop (native-apps.md 20.3): the xdg-desktop-portal
/// ScreenCast picker on Linux (build feature `wayland`; X11 whole desktop where no portal
/// exists), `GraphicsCapturePicker` on Windows, `SCContentSharingPicker` on macOS 14+. A
/// cancelled or refused capture is `capture_denied`, a system without a usable backend
/// `capture_unavailable`; neither ends the session. Returns `false` if no presenter session
/// runs. The picker blocks a worker thread while it is open, not the app.
#[frb(sync)]
pub fn present_screen(screen: Option<ScreenRequest>) -> bool {
    presenter::send(PCmd::Screen(screen))
}

/// Share H.264 that the host pushes with [`push_encoded_frame`] (Android `MediaCodec`, iOS
/// broadcast) in the joined presenter session. Returns `false` if no presenter session runs.
#[frb(sync)]
pub fn present_encoded() -> bool {
    presenter::send(PCmd::Encoded)
}

/// Show a file or a link in the joined presenter session: check it (events `Converting`,
/// `ContentPrepared` or `ContentRejected`), open a fresh connection inside the session, send it
/// with `Progress`, then `ContentSent`; [`send_content_cmd`] turns pages. Returns `false` if no
/// presenter session runs.
#[frb(sync)]
pub fn present_content(source: ContentSource) -> bool {
    presenter::send(PCmd::Content(source))
}

/// iOS hand-over (native-apps.md 20.5, step 1): ask the display to hold the single-use `token`
/// (128 random bits as base64url, 1 to 64 characters of `A-Z a-z 0-9 - _`, made by the app) for
/// the broadcast extension, which joins with it. The answer is the event `HandoverResult`; then
/// leave with [`stop_presenter`]. Opens a quiet content-only connection if nothing is
/// presented. Returns `false` if no presenter session runs or the token is not valid.
#[frb(sync)]
pub fn request_handover(token: String) -> bool {
    cc_proto::sanitize_handover(Some(&token)).is_some() && presenter::send(PCmd::Handover(token))
}

/// End the running presentation only: the display goes Idle with the same code and the
/// session stays joined (`PresentingStopped`); the next presentation needs no new approval.
/// Returns `false` if no presenter session runs.
#[frb(sync)]
pub fn stop_presenting() -> bool {
    presenter::send(PCmd::StopPresenting)
}

// ---------------------------------------------------------------------------
// H.264 codec (Cisco OpenH264), downloaded on first use in release builds
// ---------------------------------------------------------------------------

/// Whether the H.264 codec can be used, for the UI. See `cc-codec` and docs/capture.md.
#[derive(Debug, Clone, PartialEq)]
pub enum UiCodecState {
    /// Nothing to do: the codec is built in, or the library is on the disk.
    Ready,
    /// The library is missing. [`ensure_codec`] downloads it (after the user agreed).
    Missing,
    /// A download runs; `progress` is 0.0 to 1.0.
    Downloading {
        /// Fraction done.
        progress: f64,
    },
    /// The last download or import failed; `reason` is technical and in English.
    Failed {
        /// What went wrong.
        reason: String,
    },
    /// This build or platform has no H.264 codec to offer.
    Unsupported {
        /// Why.
        reason: String,
    },
}

/// What the UI shows before it asks to download the codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCodecInfo {
    /// Exact file name the library must have (for the administrator directory).
    pub file_name: String,
    /// OpenH264 version.
    pub version: String,
    /// Host the download comes from.
    pub host: String,
    /// Approximate download size in bytes.
    pub download_bytes: u32,
    /// The administrator allows the download without asking (`COMETCAST_CODEC_AUTO=1`).
    pub auto_download: bool,
    /// The administrator directory (`COMETCAST_CODEC_DIR`), if set.
    pub admin_dir: Option<String>,
}

#[cfg(all(feature = "h264-libloading", not(feature = "h264-source")))]
fn ui_codec_state(s: cc_codec::CodecStatus) -> UiCodecState {
    use cc_codec::CodecStatus as S;
    match s {
        S::Ready { .. } => UiCodecState::Ready,
        S::Missing => UiCodecState::Missing,
        S::Downloading { progress } => UiCodecState::Downloading { progress },
        S::Failed { reason } => UiCodecState::Failed { reason },
        S::Unsupported { reason } => UiCodecState::Unsupported { reason },
    }
}

/// Current state of the H.264 codec. Cheap; the UI polls it while a download runs.
#[frb(sync)]
pub fn codec_state() -> UiCodecState {
    codec_state_impl()
}

#[cfg(feature = "h264-source")]
fn codec_state_impl() -> UiCodecState {
    UiCodecState::Ready
}

#[cfg(all(feature = "h264-libloading", not(feature = "h264-source")))]
fn codec_state_impl() -> UiCodecState {
    ui_codec_state(cc_codec::codec_status())
}

#[cfg(not(feature = "h264"))]
fn codec_state_impl() -> UiCodecState {
    UiCodecState::Unsupported {
        reason: "this build has no H.264 support".into(),
    }
}

/// Facts for the consent text; `None` if this build never downloads (codec built in, no H.264,
/// or a platform without a library).
#[frb(sync)]
pub fn codec_info() -> Option<UiCodecInfo> {
    codec_info_impl()
}

#[cfg(all(feature = "h264-libloading", not(feature = "h264-source")))]
fn codec_info_impl() -> Option<UiCodecInfo> {
    let i = cc_codec::codec_info()?;
    Some(UiCodecInfo {
        file_name: i.file_name,
        version: i.version,
        host: i.host,
        download_bytes: u32::try_from(i.download_bytes).unwrap_or(u32::MAX),
        auto_download: i.auto_download,
        admin_dir: i.admin_dir,
    })
}

#[cfg(not(all(feature = "h264-libloading", not(feature = "h264-source"))))]
fn codec_info_impl() -> Option<UiCodecInfo> {
    None
}

/// Make the codec available: import the administrator's file or download from Cisco. Call it
/// only after the user agreed (or when [`UiCodecInfo::auto_download`] is true). Returns the
/// state when done; while it runs, [`codec_state`] reports the progress.
pub async fn ensure_codec() -> UiCodecState {
    #[cfg(all(feature = "h264-libloading", not(feature = "h264-source")))]
    {
        tokio::task::spawn_blocking(|| ui_codec_state(cc_codec::ensure_codec()))
            .await
            .unwrap_or_else(|_| UiCodecState::Failed {
                reason: "the download task stopped unexpectedly".into(),
            })
    }
    #[cfg(not(all(feature = "h264-libloading", not(feature = "h264-source"))))]
    {
        codec_state_impl()
    }
}

/// Where the codec is stored on Android: the app-private directory the app passes in
/// (`getFilesDir()`). Other platforms use their per-user data directory and ignore this.
#[frb(sync)]
pub fn set_codec_dir(path: Option<String>) {
    #[cfg(all(feature = "h264-libloading", not(feature = "h264-source")))]
    cc_codec::set_data_dir(path.map(std::path::PathBuf::from));
    #[cfg(not(all(feature = "h264-libloading", not(feature = "h264-source"))))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hand-over request is refused before anything is queued when the token is not a
    /// valid one or no presenter session runs (native-apps.md 20.5).
    #[test]
    fn a_handover_request_needs_a_valid_token_and_a_session() {
        presenter::clear();
        assert!(
            !request_handover("AbCdEfGhIjKlMnOpQrStUv".into()),
            "no session"
        );
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        presenter::install(tx);
        for bad in ["", "a b", "a+b", &"x".repeat(65)] {
            assert!(!request_handover(bad.into()), "{bad:?}");
        }
        assert!(rx.try_recv().is_err(), "nothing was queued for a bad token");
        assert!(request_handover("AbCd-_09".into()));
        assert!(matches!(rx.try_recv(), Ok(PCmd::Handover(t)) if t == "AbCd-_09"));
        presenter::clear();
    }

    /// Both features on (cargo unifies them in a workspace build): the backend is the source
    /// build, nothing is missing, so the UI must not ask for a download.
    #[cfg(all(feature = "h264-libloading", feature = "h264-source"))]
    #[test]
    fn source_backend_is_ready_and_needs_no_download() {
        assert_eq!(codec_state(), UiCodecState::Ready);
        assert!(codec_info().is_none());
    }

    #[cfg(all(feature = "h264-libloading", not(feature = "h264-source")))]
    #[test]
    fn codec_status_maps_to_the_ui_state() {
        use cc_codec::CodecStatus as S;
        assert_eq!(ui_codec_state(S::Missing), UiCodecState::Missing);
        assert_eq!(
            ui_codec_state(S::Ready { path: "/x".into() }),
            UiCodecState::Ready
        );
        assert_eq!(
            ui_codec_state(S::Downloading { progress: 0.5 }),
            UiCodecState::Downloading { progress: 0.5 }
        );
        assert_eq!(
            ui_codec_state(S::Failed { reason: "r".into() }),
            UiCodecState::Failed { reason: "r".into() }
        );
    }

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

    /// Tests that touch the process-wide encoded-input slot run one at a time.
    static SLOT_TESTS: Mutex<()> = Mutex::new(());

    #[test]
    fn invalid_code_installs_no_encoded_input_and_clears_the_old_one() {
        let _g = SLOT_TESTS.lock().unwrap_or_else(|p| p.into_inner());
        let old = ExternalInput::default();
        encoded::replace(Some(old.clone()));
        let new = ExternalInput::default();
        assert!(accept_code_and_input("nope", Some(new.clone())).is_err());
        assert!(encoded::current().is_none(), "nothing stale is left");
        assert!(!new.same_as(&old));
        assert_eq!(
            new.push(&[0, 0, 0, 1, 0x65, 1], true, None),
            cc_sender::PushOutcome::NotStreaming,
            "the new input was not closed or installed by the failed start"
        );
        // A valid code installs the input.
        assert!(accept_code_and_input("K7M2QX", Some(new.clone())).is_ok());
        assert!(encoded::current().is_some_and(|c| c.same_as(&new)));
        encoded::replace(None);
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

    fn no_policy() -> cc_discovery::DiscoverOptions {
        // Explicit empty location list, so that a policy file on the machine running the test
        // does not matter.
        cc_discovery::DiscoverOptions {
            policy_locations: Some(Vec::new()),
            dns_srv: false,
            mdns: false,
            ..cc_discovery::DiscoverOptions::default()
        }
    }

    #[cfg(unix)]
    fn policy_options(
        json: &[u8],
        mode: u32,
    ) -> (tempfile::TempDir, cc_discovery::DiscoverOptions) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp dir");
        let p = dir.path().join("policy.json");
        std::fs::write(&p, json).expect("write");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).expect("chmod");
        let opts = cc_discovery::DiscoverOptions {
            policy_locations: Some(vec![cc_discovery::policy::explicit_location(&p)]),
            dns_srv: false,
            mdns: false,
            ..cc_discovery::DiscoverOptions::default()
        };
        (dir, opts)
    }

    fn addresses(list: &[ServerCandidate]) -> Vec<(&str, &str)> {
        list.iter()
            .map(|c| (c.address.as_str(), c.source.as_str()))
            .collect()
    }

    #[test]
    fn server_defaults_come_from_cc_discovery() {
        let d = server_defaults_with(&no_policy());
        assert_eq!(
            addresses(&d.servers),
            [("relay.cometcast.de:443", "default-list")]
        );
        assert!(!d.locked);
        assert!(d.announcements_allowed);
        assert!(d.notices.is_empty());
        assert_eq!(d.dev_fallback, "127.0.0.1:8443");
        assert_eq!(server_defaults().dev_fallback, "127.0.0.1:8443");
    }

    #[cfg(unix)]
    #[test]
    fn server_defaults_put_the_policy_first() {
        let (_dir, opts) = policy_options(
            br#"{"servers":["wss://mdm.example:443"],"forbid_default_list":true}"#,
            0o644,
        );
        let d = server_defaults_with(&opts);
        assert_eq!(addresses(&d.servers), [("wss://mdm.example:443", "mdm")]);
        assert!(!d.locked);
    }

    #[cfg(unix)]
    #[test]
    fn a_policy_that_forbids_override_locks_and_blocks_announcements() {
        let (_dir, opts) = policy_options(
            br#"{"servers":["wss://mdm.example:443"],"forbid_user_override":true}"#,
            0o644,
        );
        let d = server_defaults_with(&opts);
        assert!(d.locked);
        assert!(!d.announcements_allowed);
        assert_eq!(addresses(&d.servers), [("wss://mdm.example:443", "mdm")]);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let r = rt.block_on(discover_servers_with(&opts));
        assert!(r.locked);
        assert_eq!(addresses(&r.servers), [("wss://mdm.example:443", "mdm")]);
    }

    #[cfg(unix)]
    #[test]
    fn an_unusable_policy_fails_closed_with_a_code_not_with_text() {
        // Writable by others: unusable.
        let (_dir, opts) = policy_options(br#"{"servers":["a.example:1"]}"#, 0o666);
        let d = server_defaults_with(&opts);
        assert!(d.locked);
        assert!(d.servers.is_empty());
        assert!(!d.announcements_allowed);
        assert_eq!(d.notices, [DiscoveryWarning::PolicyUnusable]);
    }

    #[cfg(unix)]
    #[test]
    fn a_locked_policy_without_servers_is_reported() {
        let (_dir, opts) = policy_options(br#"{"forbid_user_override":true}"#, 0o644);
        let d = server_defaults_with(&opts);
        assert!(d.locked);
        assert!(d.servers.is_empty());
        assert_eq!(d.notices, [DiscoveryWarning::PolicyNamesNoServer]);
    }

    #[test]
    fn discovery_without_network_sources_returns_the_default_list() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let r = rt.block_on(discover_servers_with(&no_policy()));
        assert_eq!(
            addresses(&r.servers),
            [("relay.cometcast.de:443", "default-list")]
        );
        assert!(!r.locked);
        assert!(r.announcements_allowed);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn warning_texts_map_to_codes() {
        let w = |s: &str| classify_warning(s);
        assert_eq!(
            w("the policy file is unusable (x); no server is offered"),
            DiscoveryWarning::PolicyUnusable
        );
        assert_eq!(
            w("the policy forbids other servers but lists none"),
            DiscoveryWarning::PolicyNamesNoServer
        );
        assert_eq!(
            w("DNS-SRV lookup for _cometcast._tcp.x.example timed out"),
            DiscoveryWarning::DnsTimeout
        );
        assert_eq!(
            w("DNS-SRV lookup for _cometcast._tcp.x.example: boom"),
            DiscoveryWarning::DnsFailed
        );
        assert_eq!(
            w("DNS-SRV is not available: no resolv.conf"),
            DiscoveryWarning::DnsFailed
        );
        assert_eq!(
            w("DNS-SRV _cometcast._tcp.x.example says the service is not available"),
            DiscoveryWarning::DnsServiceUnavailable
        );
        assert_eq!(
            w("2 DNS-SRV record(s) for _cometcast._tcp.x.example were refused"),
            DiscoveryWarning::EntriesIgnored
        );
        assert_eq!(
            w("announcement browse failed: nope"),
            DiscoveryWarning::AnnouncementFailed
        );
        // Anything else, including hostile text, is only a code.
        assert_eq!(w("<script>alert(1)</script>"), DiscoveryWarning::Other);
        let all = classify_warnings(&["a".into(), "b".into(), "DNS-SRV x".into()]);
        assert_eq!(all, [DiscoveryWarning::Other, DiscoveryWarning::DnsFailed]);
    }

    #[test]
    fn real_merge_warnings_keep_their_codes() {
        // Built with cc-discovery itself, so a changed text there fails here.
        let invalid = cc_discovery::PolicyState::Invalid("x".into());
        let m = cc_discovery::merge(&invalid, &[], &[], &[]);
        assert_eq!(
            classify_warnings(&m.warnings),
            [DiscoveryWarning::PolicyUnusable]
        );
    }

    #[test]
    fn advertise_errors_are_sorted_into_refused_and_failed() {
        assert_eq!(
            classify_advertise_error("server cannot be announced: loopback"),
            AdvertiseOutcome::Refused
        );
        assert_eq!(
            classify_advertise_error("instance name is empty or too long"),
            AdvertiseOutcome::Refused
        );
        assert_eq!(
            classify_advertise_error("no interface found"),
            AdvertiseOutcome::Failed
        );
    }

    #[test]
    fn advertising_refuses_a_loopback_server_and_stop_is_harmless() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        stop_advertising();
        let out = rt.block_on(start_advertising(
            "Test receiver".into(),
            Some("127.0.0.1:8443".into()),
        ));
        assert_eq!(out, AdvertiseOutcome::Refused);
        let out = rt.block_on(start_advertising(String::new(), None));
        assert_eq!(out, AdvertiseOutcome::Refused);
        stop_advertising();
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
    fn android_capture_info_lists_the_projection_only() {
        let info = android_capture_info();
        assert_eq!(info.support, CaptureSupport::Supported);
        assert_eq!(info.monitors.len(), 1);
        assert_eq!(info.monitors[0].id, ANDROID_PROJECTION_ID);
        // The Dart side (lib/core/android_capture.dart) compares with this literal.
        assert_eq!(ANDROID_PROJECTION_ID, "android-projection");
    }

    #[test]
    fn ios_capture_info_lists_the_broadcast_only() {
        let info = ios_capture_info();
        assert_eq!(info.support, CaptureSupport::Supported);
        assert_eq!(info.monitors.len(), 1);
        assert_eq!(info.monitors[0].id, IOS_BROADCAST_ID);
        // The Dart side (lib/core/ios_broadcast.dart) compares with this literal.
        assert_eq!(IOS_BROADCAST_ID, "ios-broadcast");
    }

    #[test]
    fn push_without_a_sender_is_refused() {
        let _g = SLOT_TESTS.lock().unwrap_or_else(|p| p.into_inner());
        encoded::replace(None);
        assert_eq!(
            push_encoded_frame(vec![0, 0, 0, 1, 0x65, 1], true, 0),
            PushResult::NoSender
        );
        assert!(!push_codec_config(vec![0, 0, 0, 1, 0x67, 1]));
        assert!(!keyframe_requested());
    }

    #[test]
    fn push_reaches_the_current_input_and_stop_clears_it() {
        let _g = SLOT_TESTS.lock().unwrap_or_else(|p| p.into_inner());
        let input = ExternalInput::default();
        encoded::replace(Some(input.clone()));
        // No peer yet: refused, counted by the core.
        assert_eq!(
            push_encoded_frame(vec![0, 0, 0, 1, 0x65, 1], true, 5),
            PushResult::NotStreaming
        );
        assert_eq!(
            push_encoded_frame(vec![1, 2], true, -1),
            PushResult::Invalid
        );
        assert!(input.request_keyframe());
        assert!(keyframe_requested());
        assert!(!keyframe_requested(), "reading clears the request");
        stop_sender();
        assert_eq!(
            push_encoded_frame(vec![0, 0, 0, 1, 0x65, 1], true, 5),
            PushResult::NoSender
        );
        assert_eq!(
            input.push(&[0, 0, 0, 1, 0x65, 1], true, None),
            PushOutcome::Closed,
            "the closed input refuses late pushes"
        );
    }

    #[test]
    fn the_new_receiver_events_are_mapped() {
        assert_eq!(
            map_receiver_event(ReceiverEvent::ConnectionCode("123 456".into())),
            ReceiverUiEvent::ConnectionCode {
                sas: "123 456".into()
            }
        );
        assert_eq!(
            map_receiver_event(ReceiverEvent::PresentationStopped),
            ReceiverUiEvent::PresentationStopped
        );
        assert_eq!(
            map_receiver_event(ReceiverEvent::Error(
                "server error room_full: the room is full".into()
            )),
            ReceiverUiEvent::Error {
                code: "room_full".into(),
                message: "server error room_full: the room is full".into()
            }
        );
        assert!(matches!(
            map_receiver_event(ReceiverEvent::Error("signalling connection closed".into())),
            ReceiverUiEvent::Error { code, .. } if code == "connection_lost"
        ));
    }

    #[test]
    fn sender_events_are_mapped() {
        assert_eq!(
            map_sender_event(SenderEvent::Finished { frames_sent: 9 }),
            Some(SenderUiEvent::Finished { frames_sent: 9 })
        );
        assert_eq!(
            map_sender_event(SenderEvent::Error("x".into())),
            Some(SenderUiEvent::Error {
                message: "x".into()
            })
        );
        assert_eq!(
            map_sender_event(SenderEvent::ConnectionCode("123 456".into())),
            None
        );
    }
}
