// SPDX-License-Identifier: GPL-3.0-or-later
//! Receiver core: creates a room, answers the sender's offer and reports incoming video.
//!
//! This is the M1 slice (native peer for the signalling protocol). There is no decoding
//! or rendering yet: the core only counts RTP packets, bytes, frames and key frames. Later
//! parts (queue, toolbar state, deletion rule) arrive with the apps.
//!
//! On a server that requires proof-of-work (open mode) the core fetches and solves the
//! ALTCHA puzzle by itself when the server answers `altcha_required`.
//!
//! Content: if [`ReceiverOptions::content`] is set, the `control` data channel of the sender
//! is accepted and its images, links, PDF documents and page commands arrive as
//! [`ContentEvent`]s from [`ReceiverCore::take_content_events`] (docs/web-m2.md, `cc-files`).
//! Links are only reported, never opened. Without the option the channel is closed.
//!
//! The room outlives a presenter: when the sender leaves (or is removed with
//! [`ReceiverCore::remove_peer`]) the core reports [`ReceiverEvent::PeerLeft`], tears down the
//! WebRTC session and the content task, makes a fresh session and waits for the next sender in
//! the same room. Code, join mode, block list and request queue stay. A sender that only stops
//! presenting (it closes its control channel, or starts a new offer, but stays in the room) is
//! reported as [`ReceiverEvent::PresentationStopped`]; the session is made fresh as well. So
//! is a presenter whose connection fails (an offer that cannot be answered, WebRTC `failed`,
//! no connection in time): a joiner must not be able to end the room.
//! Pause (docs/design/pause.md): the core holds the two pause flags of the live screen
//! ([`PauseState`]): `presenter` (set by the presenter's `cmd pause` / `cmd resume`) and
//! `display` (set with [`ReceiverCore::set_display_paused`]). Every change is reported as
//! [`ReceiverEvent::PauseChanged`] and sent to the presenter as `pause_state`, once more when
//! the control channel arrives (both `false`). Pause only applies while a video track is
//! shown; both flags are cleared when the presentation ends (stop, failure, peer left, new
//! offer). The core never treats missing frames as a failure: it has no frame watchdog.
//!
//! The core stops on [`ReceiverEvent::RoomEnded`], an error (the signalling connection, a
//! server error that closes the connection) or [`ReceiverCore::stop`]. A server error that
//! leaves the connection open (for example `no_such_request`) is a
//! [`ReceiverEvent::ServerNotice`] and the room stays.
//!
//! Use: [`ReceiverCore::start`] -> show [`ReceiverCore::room_code`] -> loop on
//! [`ReceiverCore::next_event`].

mod content;
mod device_memory;
mod handover;

use std::sync::Arc;
use std::time::{Duration, Instant};

pub use cc_files::{Blob, ContentError, ContentEvent, ContentOptions, ImageMime, PauseState};
pub use cc_proto::JoinMode;
use cc_proto::{ClientMsg, Role, RoomCode, ServerMsg};
use cc_transport::device::{unix_ms, verify_join};
use cc_transport::{
    ConnectionState, IceServerConfig, MediaSnapshot, ServerAddr, Session, SessionConfig,
    SessionEvent, SignalingClient, TlsTrust, TransportError, VideoCodec, altcha, ice,
};
pub use device_memory::{DeviceMemory, MAX_PAIRS as DEVICE_MEMORY_MAX_PAIRS, is_generated_name};
pub use handover::{HANDOVER_TTL, HandoverStore};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// How long we wait for the server to hand out a room code.
/// UNCERTAIN: placeholder, not measured.
const ROOM_TIMEOUT: Duration = Duration::from_secs(10);

/// How long after the sender's offer arrived the WebRTC connection may take to become connected.
/// UNCERTAIN: 30 s is a placeholder; real values depend on networks and are not measured.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Settings for [`ReceiverCore::start`].
#[derive(Debug, Clone)]
pub struct ReceiverOptions {
    /// Signalling server as `host:port` or a `ws://`, `wss://`, `http://`, `https://` URL.
    pub server: String,
    /// WebRTC settings (local bind address, codecs). If `session.ice_servers` is empty the
    /// core asks the server for its STUN/TURN list (`/ice-servers`).
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://` (default: system store only).
    pub trust: TlsTrust,
    /// Accept content (images, links, PDFs, page commands) on the `control` data channel.
    /// `None` (default): the channel is closed. The application must then read
    /// [`ReceiverCore::take_content_events`]; when it does not, the sender is slowed down.
    pub content: Option<ContentOptions>,
    /// Who may join the room from the start (docs/protocol.md, "Join approval"). Default
    /// `open`. Can be changed at any time with [`ReceiverCore::set_join_mode`].
    pub join_mode: JoinMode,
    /// Name of the room, shown to senders on their waiting and "connected" pages. The
    /// server cleans it and cuts it at 32 characters. `None` = no room name.
    pub room_name: Option<String>,
    /// How long a hand-over token of a presenter stays valid (native-apps.md 20.5). Default
    /// [`HANDOVER_TTL`] (60 s); shorter values are for tests.
    pub handover_ttl: Duration,
    /// What the display remembers about names and devices (from the app's settings, see
    /// [`DeviceMemory::from_json`]). Default empty. Changes come back as
    /// [`ReceiverEvent::DeviceMemoryChanged`].
    pub device_memory: DeviceMemory,
}

impl ReceiverOptions {
    /// Options for the given server with default WebRTC settings.
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
            content: None,
            join_mode: JoinMode::Open,
            room_name: None,
            handover_ttl: HANDOVER_TTL,
            device_memory: DeviceMemory::new(),
        }
    }
}

/// The content events of a [`ReceiverCore`] (from [`ReceiverCore::take_content_events`]).
///
/// The queue behind it is bounded by **count (16) and by bytes (32 MiB of pictures)**: while
/// the application does not read, the sender is slowed down. A single event is always
/// admitted into an empty queue, so a large page (up to the output limit of `cc-docs`,
/// 7680 x 4320 pixels = 127 MiB of RGBA by default) can wait alone. A `PageRendered` that is
/// still waiting when a newer one arrives is dropped: only the latest picture matters.
///
/// Keep reading this stream **while** you read [`ReceiverCore::next_event`] (for example in
/// one `select!`, content first), and after [`ReceiverEvent::PeerLeft`] read it until it
/// returns `None`: content that arrived just before the sender left is still queued then.
/// UNCERTAIN: the sizes are placeholders, unmeasured.
pub struct ContentEvents {
    rx: content::EventStream,
}

impl ContentEvents {
    /// Next event, or `None` after the core has stopped and all were read.
    pub async fn next(&mut self) -> Option<ContentEvent> {
        self.rx.next().await
    }
}

/// How long the core waits, after the sender left, for the content that was still in the
/// queue to be handed to the application. UNCERTAIN: placeholder.
const CONTENT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// What the receiver reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    /// A sender joined the room (it is a normal peer from now on).
    PeerJoined {
        /// Sanitised display name of the sender.
        name: String,
        /// The join came with a hand-over token this display had issued and was let in
        /// without a dialog (native-apps.md 20.5): the person was approved before, so the app
        /// shows no "connected" notice.
        handover: bool,
        /// The presenter's device id when it proved its key for this room (open mode: with
        /// the join; confirm mode: with the request the host allowed), else `None`.
        device: Option<String>,
        /// `device` is set and this name was used here before, but only by other devices
        /// (open mode only; docs/design/presenter-identity.md). A hint, not a warning.
        other_device: bool,
    },
    /// The display's memory of names and devices changed (an allowed request, or a verified
    /// join in open mode). The app stores the text with its settings and gives it back in
    /// [`ReceiverOptions::device_memory`] next time.
    DeviceMemoryChanged(String),
    /// A presenter asked this display to hold a hand-over token and the display stored it
    /// (and answered `handover_ok`). If the presenter now leaves, that is not a leave for the
    /// host (no notice) until the token is used or runs out ([`ReceiverEvent::HandoverExpired`]).
    HandoverStored {
        /// The presenter's name.
        name: String,
    },
    /// A hand-over token ran out unused (60 s), or was withdrawn because the host removed the
    /// presenter, denied and blocked a request or locked the room.
    HandoverExpired,
    /// Join mode `confirm`: somebody waits for a decision. Answer with
    /// [`ReceiverCore::decide_join`]. The name is untrusted text (cleaned by the server):
    /// show it as plain text only.
    JoinRequest {
        /// Request id for `decide_join`.
        id: u64,
        /// Sanitised display name.
        name: String,
        /// Two-digit check number; the sender's waiting screen shows the same number.
        check: u8,
        /// The presenter's device id when its proof for this room verified, else `None`.
        device: Option<String>,
        /// `device` is set and this name was used here before, but only by other devices:
        /// the dialog adds "This name was used here before by a different device."
        other_device: bool,
    },
    /// A waiting request is gone: `left`, `cancelled`, `timeout`, `locked` or `room_full`.
    JoinRequestCancelled {
        /// Request id.
        id: u64,
        /// Why.
        reason: String,
    },
    /// The join mode now in force (confirmed by the server after `set_join_mode`).
    JoinModeChanged(JoinMode),
    /// An entry was added to the room's block list (docs/protocol.md, "Join approval").
    Blocked {
        /// Block id for [`ReceiverCore::unblock`].
        id: u64,
        /// Cleaned display name of the blocked sender (for the list only).
        name: String,
    },
    /// An entry left the block list.
    Unblocked {
        /// Block id.
        id: u64,
    },
    /// The room session was ended with [`ReceiverCore::end_room`]. The core stops after this
    /// event; start a new core for a new room (a new code).
    RoomEnded,
    /// WebRTC connection state changed.
    State(ConnectionState),
    /// The sender's video track arrived.
    VideoTrack {
        /// MIME type, e.g. `video/VP8`.
        mime: String,
        /// Parsed codec, if known.
        codec: Option<VideoCodec>,
    },
    /// Counters of incoming video, sent about once per second after the track arrived.
    Stats(MediaSnapshot),
    /// The sender left (or was removed). The room stays: the core is ready for the next sender
    /// with the same code, join mode, block list and queue.
    PeerLeft,
    /// A pause flag changed (docs/design/pause.md). The picture is frozen while either flag is
    /// set; the application keeps showing the last picture. The presenter has been told with
    /// `pause_state`. Both flags are cleared (one event) when the presentation ends.
    PauseChanged {
        /// The presenter paused its screen (it stops sending video).
        presenter: bool,
        /// The display's own flag ([`ReceiverCore::set_display_paused`]).
        display: bool,
    },
    /// The sender's presentation ended but the sender is still in the room: it closed its
    /// control channel or began a new offer, or its connection could not be made (an offer
    /// that cannot be answered, WebRTC `failed`, no connection 30 s after the offer).
    /// Whatever is shown should go; the next presentation of the same sender needs no new
    /// approval. The room stays.
    PresentationStopped,
    /// The connection check code (`"123 456"`) of the sender's connection, once per connection,
    /// as soon as it is connected. Identical to the one the sender shows (`cc_transport::sas`).
    ConnectionCode(String),
    /// The server refused one of the host's commands, or a relay message, with an error that
    /// does **not** end the room (docs/protocol.md, "Error codes": the connection stays
    /// open), e.g. `no_such_request` when the host decided on a request that was cancelled
    /// at the same moment, `block_list_full`, `not_in_room`, `rate_limited`. The room, its
    /// code and the connected sender stay; the app may show a short note. If the server
    /// closes the connection after the error, [`ReceiverEvent::Error`] follows.
    ServerNotice {
        /// The protocol error code.
        code: String,
        /// The server's message in English (for logs only).
        message: String,
    },
    /// Something failed that ends the room (the signalling connection, a server error that
    /// closes the connection, no new session). The core stops after this event. A failure
    /// of the presenter's own connection is not an error: it is
    /// [`ReceiverEvent::PresentationStopped`].
    Error(String),
}

/// Does a server `error` with this code end the room? Only the codes after which the server
/// closes the connection do (docs/protocol.md, "Error codes", column "closes": `bad_hello`,
/// `proto_mismatch`, `banned`). Every other code answers one message and the connection and
/// the room stay; when the server does close the connection after such an error (a
/// connection that sends too fast gets `rate_limited` and is closed), the closed connection
/// itself ends the room. So a joiner that makes a host command fail at the right moment (for
/// example by cancelling its request while the host decides) cannot end the room.
fn server_error_ends_room(code: &str) -> bool {
    matches!(
        code,
        "bad_hello" | "proto_mismatch" | cc_proto::error_codes::BANNED
    )
}

/// The code of a [`ReceiverEvent::Error`] message, for the apps (native-apps.md 20.2): the
/// protocol code of a `server error <code>: ...`, `connection_lost` when the signalling
/// connection ended, `peer_failed` when no WebRTC session could be made, `unknown` otherwise.
/// The messages are the ones made in this file; the text itself is for logs only. (A failed
/// connection of a presenter is no error any more but [`ReceiverEvent::PresentationStopped`];
/// its old messages still map to `peer_failed`.)
pub fn error_code(message: &str) -> String {
    if let Some(rest) = message.strip_prefix("server error ")
        && let Some((code, _)) = rest.split_once(':')
    {
        return code.to_owned();
    }
    if message.starts_with("signalling connection") {
        "connection_lost".into()
    } else if message.starts_with("WebRTC connection failed")
        || message.starts_with("no WebRTC connection")
        || message.starts_with("cannot answer offer")
        || message.starts_with("cannot start a session")
    {
        "peer_failed".into()
    } else {
        "unknown".into()
    }
}

/// The server's answer to `create_room`.
enum RoomReply {
    Created(RoomCode, JoinMode),
    AltchaRequired,
}

/// Wait for the answer to `create_room`.
async fn next_room_reply(sig: &mut SignalingClient) -> Result<RoomReply, TransportError> {
    tokio::time::timeout(ROOM_TIMEOUT, async {
        match sig.recv().await {
            Some(ServerMsg::RoomCreated { code, join_mode }) => {
                Ok(RoomReply::Created(code, join_mode))
            }
            Some(ServerMsg::Error { code, .. }) if code == "altcha_required" => {
                Ok(RoomReply::AltchaRequired)
            }
            Some(ServerMsg::Error { code, message }) => {
                Err(TransportError::Server { code, message })
            }
            Some(other) => Err(TransportError::Protocol(format!(
                "expected room_created, got {other:?}"
            ))),
            None => Err(TransportError::Closed),
        }
    })
    .await
    .map_err(|_| TransportError::Timeout("waiting for room_created"))?
}

/// A running receiver.
pub struct ReceiverCore {
    code: RoomCode,
    join_mode: JoinMode,
    commands: mpsc::UnboundedSender<ClientMsg>,
    ice_servers: Vec<IceServerConfig>,
    /// The current session (replaced when a sender left); see [`Self::session`].
    session: Arc<std::sync::Mutex<Arc<Session>>>,
    events: mpsc::UnboundedReceiver<ReceiverEvent>,
    content_events: Option<content::EventStream>,
    content_width: mpsc::UnboundedSender<u32>,
    display_pause: mpsc::UnboundedSender<bool>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl ReceiverCore {
    /// Connect to the server, create a room and start answering. Returns once the room code is known.
    pub async fn start(options: ReceiverOptions) -> Result<Self, TransportError> {
        let addr = ServerAddr::parse(&options.server)?;
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Receiver, &options.trust).await?;
        sig.send(ClientMsg::CreateRoom {
            altcha: None,
            join_mode: Some(options.join_mode),
            room_name: options.room_name.clone(),
        })
        .await?;
        let (code, join_mode) = match next_room_reply(&mut sig).await? {
            RoomReply::Created(code, mode) => (code, mode),
            // Open servers always want a solved puzzle (docs/server.md). The connection
            // stays open after `altcha_required`, so we ask again on the same one.
            RoomReply::AltchaRequired => {
                let payload = altcha::fetch_and_solve(&addr, &options.trust).await?;
                sig.send(ClientMsg::CreateRoom {
                    altcha: Some(payload),
                    join_mode: Some(options.join_mode),
                    room_name: options.room_name.clone(),
                })
                .await?;
                match next_room_reply(&mut sig).await? {
                    RoomReply::Created(code, mode) => (code, mode),
                    RoomReply::AltchaRequired => {
                        return Err(TransportError::Server {
                            code: "altcha_required".into(),
                            message: "the server still wants a puzzle solution".into(),
                        });
                    }
                }
            }
        };

        let mut session_config = options.session;
        if session_config.ice_servers.is_empty() {
            // Best effort: no list (404, or a failed request) means no STUN/TURN.
            session_config.ice_servers =
                ice::fetch_ice_servers_or_none(&addr, &options.trust).await;
        }
        let ice_servers = session_config.ice_servers.clone();
        let (session, session_events) = Session::new(session_config.clone()).await?;
        let session = Arc::new(std::sync::Mutex::new(Arc::new(session)));
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (width_tx, width_rx) = mpsc::unbounded_channel();
        let (pause_tx, pause_rx) = mpsc::unbounded_channel();
        let (content_tx, content_rx) = content::event_queue();
        let content = options.content.map(|mut o| {
            // The apps show a progress for incoming files.
            o.progress = true;
            ContentSetup {
                options: o,
                widths: width_rx,
                events: Arc::new(content_tx),
                room: content::RoomContent::new(),
            }
        });
        let has_content = content.is_some();
        let handover = Arc::new(HandoverStore::new(options.handover_ttl));
        let task = tokio::spawn(run(
            sig,
            Arc::clone(&session),
            session_config,
            session_events,
            events_tx,
            stop_rx,
            cmd_rx,
            pause_rx,
            content,
            handover,
            code.to_string(),
            options.device_memory,
        ));
        Ok(Self {
            code,
            join_mode,
            commands: cmd_tx,
            ice_servers,
            session,
            events: events_rx,
            content_events: has_content.then_some(content_rx),
            content_width: width_tx,
            display_pause: pause_tx,
            stop: Some(stop_tx),
            task,
        })
    }

    /// The STUN/TURN servers this receiver uses (from the server's `/ice-servers`, or what the
    /// caller configured). Empty = none.
    pub fn ice_servers(&self) -> &[IceServerConfig] {
        &self.ice_servers
    }

    /// The code the sender must enter.
    pub fn room_code(&self) -> &RoomCode {
        &self.code
    }

    /// The join mode the room was created with (changes are reported as
    /// [`ReceiverEvent::JoinModeChanged`]).
    pub fn join_mode(&self) -> JoinMode {
        self.join_mode
    }

    /// Change who may join, at any time. Switching to `locked` also refuses everybody who
    /// waits. The server confirms with [`ReceiverEvent::JoinModeChanged`].
    pub fn set_join_mode(&self, mode: JoinMode) {
        let _ = self
            .commands
            .send(ClientMsg::SetJoinMode { join_mode: mode });
    }

    /// Allow or deny a waiting request (`id` from [`ReceiverEvent::JoinRequest`]).
    pub fn decide_join(&self, id: u64, allow: bool) {
        let _ = self.commands.send(ClientMsg::JoinDecision {
            id: Some(id),
            allow,
            block: false,
        });
    }

    /// Deny a waiting request and block the sender's address bucket for the rest of this room
    /// session (it then gets `join_denied` at once, and the host is not asked again). All
    /// hand-over tokens are withdrawn as well: after a block, every join asks the host.
    pub fn deny_and_block_join(&self, id: u64) {
        let _ = self.commands.send(ClientMsg::JoinDecision {
            id: Some(id),
            allow: false,
            block: true,
        });
    }

    /// Take an entry off the block list (`id` from [`ReceiverEvent::Blocked`]).
    pub fn unblock(&self, id: u64) {
        let _ = self.commands.send(ClientMsg::Unblock { id });
    }

    /// End the room session: everybody is dropped and the code is gone. The core then stops
    /// ([`ReceiverEvent::RoomEnded`]); the app starts a new core for a new room.
    pub fn end_room(&self) {
        let _ = self.commands.send(ClientMsg::EndRoom);
    }

    /// Deny everybody who waits.
    pub fn deny_all_joins(&self) {
        let _ = self.commands.send(ClientMsg::JoinDecision {
            id: None,
            allow: false,
            block: false,
        });
    }

    /// Remove the connected sender (end its presentation); with `block` also block its address
    /// bucket for the rest of this room session. The core reports [`ReceiverEvent::PeerLeft`]
    /// and waits for the next sender in the same room. A hand-over token the sender left with
    /// is withdrawn ([`ReceiverEvent::HandoverExpired`]): its next join asks the host again.
    pub fn remove_peer(&self, block: bool) {
        let _ = self.commands.send(ClientMsg::RemovePeer { block });
    }

    /// Ask the sender for a new key frame (picture loss indication), for example after a
    /// decoding error. Fails if no video track has arrived yet.
    pub async fn request_keyframe(&self) -> Result<(), TransportError> {
        self.session().request_keyframe().await
    }

    /// Current video counters (of the current sender's connection).
    pub fn stats(&self) -> MediaSnapshot {
        self.session().stats()
    }

    fn session(&self) -> Arc<Session> {
        Arc::clone(&self.session.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Next event, or `None` after the core has stopped and all events were read.
    pub async fn next_event(&mut self) -> Option<ReceiverEvent> {
        self.events.recv().await
    }

    /// Take the stream of content events (once; `None` if [`ReceiverOptions::content`] was not
    /// set or the stream was taken already). It is separate from [`Self::next_event`] so that
    /// an application can wait for both at the same time.
    pub fn take_content_events(&mut self) -> Option<ContentEvents> {
        self.content_events.take().map(|rx| ContentEvents { rx })
    }

    /// Set the width in pixels that document pages are rendered at (the size of the window
    /// or screen). The shown page is rendered again; the result arrives as a
    /// `PageRendered` event. The value is clamped to the output limits of `cc-docs`.
    pub fn set_content_target_width(&self, width: u32) {
        let _ = self.content_width.send(width);
    }

    /// Set or clear the **display's own** pause flag (docs/design/pause.md): the display keeps
    /// the last picture while the presenter goes on sending. Only offered while a screen is
    /// shown: without a video track the call is ignored. The change arrives as
    /// [`ReceiverEvent::PauseChanged`]; the presenter is told with `pause_state`. The
    /// presenter's flag cannot be changed from here.
    pub fn set_display_paused(&self, paused: bool) {
        let _ = self.display_pause.send(paused);
    }

    /// Stop and close everything.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for ReceiverCore {
    fn drop(&mut self) {
        // Ask the background task to finish: it closes the WebRTC session and the
        // signalling connection itself. (It is not aborted, so the cleanup runs.)
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }
}

/// What the background loop needs to run the content task.
struct ContentSetup {
    options: ContentOptions,
    widths: mpsc::UnboundedReceiver<u32>,
    events: Arc<content::EventSink>,
    /// Shared by the content tasks of all presentations in this room: the violation count,
    /// the document-open budget and the one document job that may run.
    room: content::RoomContent,
}

/// What belongs to the connection of one sender and starts over with the next one.
#[derive(Default)]
struct Conn {
    have_track: bool,
    connected: bool,
    offer_at: Option<tokio::time::Instant>,
}

/// A join that was let in with a hand-over token and whose `peer_joined` is still to come:
/// the request id and the name. The server answers an allowed request with `peer_joined`, or
/// with `join_request_cancelled` for the same id (the joiner left, the room was full); only
/// that join is reported as a hand-over, never a later ordinary join of the same name.
#[derive(Default)]
struct TokenAdmission(Option<(u64, String)>);

impl TokenAdmission {
    fn admitted(&mut self, id: u64, name: String) {
        self.0 = Some((id, name));
    }

    fn cancelled(&mut self, id: u64) {
        if self.0.as_ref().is_some_and(|(i, _)| *i == id) {
            self.0 = None;
        }
    }

    /// Was the join of `name` that just arrived the one let in with a token?
    fn joined(&mut self, name: &str) -> bool {
        self.0.take().is_some_and(|(_, n)| n == name)
    }
}

/// The verified devices of join requests (docs/design/presenter-identity.md): while a request
/// waits (by request id), and after the host allowed it until the server's `peer_joined` shows
/// that the presenter is in (only then the pair goes into the memory). Both lists are small:
/// the server holds at most 8 waiting requests per room.
#[derive(Default)]
struct RequestDevices {
    waiting: Vec<(u64, String, String)>,
    allowed: Vec<(String, String)>,
}

impl RequestDevices {
    const CAP: usize = 16;

    fn requested(&mut self, id: u64, name: &str, device: &str) {
        if self.waiting.len() >= Self::CAP {
            self.waiting.remove(0);
        }
        self.waiting.push((id, name.to_owned(), device.to_owned()));
    }

    /// The host (or a token) decided on request `id`.
    fn decided(&mut self, id: u64, allow: bool) {
        let Some(p) = self.waiting.iter().position(|(i, _, _)| *i == id) else {
            return;
        };
        let (_, name, device) = self.waiting.remove(p);
        if allow {
            if self.allowed.len() >= Self::CAP {
                self.allowed.remove(0);
            }
            self.allowed.push((name, device));
        }
    }

    fn cancelled(&mut self, id: u64) {
        self.waiting.retain(|(i, _, _)| *i != id);
    }

    /// A presenter named `name` joined after an allowed request: its device, if the request
    /// had a verified one. The other allowed requests are void (one sender per room).
    fn joined(&mut self, name: &str) -> Option<String> {
        let found = self
            .allowed
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d.clone());
        self.allowed.clear();
        found
    }
}

/// The pause flags of the running presentation and the way to tell the presenter.
#[derive(Default)]
struct PauseCtl {
    state: PauseState,
    /// Texts for the presenter, sent by the content task on its control channel.
    down: Option<mpsc::UnboundedSender<String>>,
}

impl PauseCtl {
    fn changed(&self, emit: &impl Fn(ReceiverEvent)) {
        emit(ReceiverEvent::PauseChanged {
            presenter: self.state.presenter,
            display: self.state.display,
        });
        self.tell();
    }

    /// Tell the presenter the flags (`pause_state`).
    fn tell(&self) {
        if let Some(d) = &self.down {
            let _ = d.send(self.state.message());
        }
    }

    fn set_presenter(&mut self, paused: bool, emit: &impl Fn(ReceiverEvent)) {
        if self.state.set_presenter(paused) {
            self.changed(emit);
        }
    }

    fn set_display(&mut self, paused: bool, emit: &impl Fn(ReceiverEvent)) {
        if self.state.set_display(paused) {
            self.changed(emit);
        }
    }

    /// The presentation ended: both flags are cleared and the channel is forgotten.
    fn end(&mut self, emit: &impl Fn(ReceiverEvent)) {
        if self.state.clear() {
            self.changed(emit);
        }
        self.down = None;
    }
}

/// End the content task of a presentation that is over (the presenter stays in the room).
fn stop_presentation(
    content_task: &mut Option<JoinHandle<()>>,
    task_widths: &mut Option<mpsc::UnboundedSender<u32>>,
) {
    if let Some(t) = content_task.take() {
        t.abort();
    }
    *task_widths = None;
}

/// Close the session of a sender that is gone and make a fresh one for the next sender.
/// The new session is also published for [`ReceiverCore::stats`] and the key frame requests.
async fn fresh_session(
    old: &Session,
    config: &SessionConfig,
    shared: &std::sync::Mutex<Arc<Session>>,
) -> Result<(Arc<Session>, mpsc::UnboundedReceiver<SessionEvent>), TransportError> {
    let _ = old.close().await;
    let (session, events) = Session::new(config.clone()).await?;
    let session = Arc::new(session);
    *shared.lock().unwrap_or_else(|p| p.into_inner()) = Arc::clone(&session);
    Ok((session, events))
}

/// The background loop: signalling messages in, session events in, receiver events out.
#[allow(clippy::too_many_arguments)]
async fn run(
    mut sig: SignalingClient,
    shared_session: Arc<std::sync::Mutex<Arc<Session>>>,
    session_config: SessionConfig,
    mut session_events: mpsc::UnboundedReceiver<SessionEvent>,
    out: mpsc::UnboundedSender<ReceiverEvent>,
    mut stop: oneshot::Receiver<()>,
    mut commands: mpsc::UnboundedReceiver<ClientMsg>,
    mut display_pause: mpsc::UnboundedReceiver<bool>,
    mut content: Option<ContentSetup>,
    handover: Arc<HandoverStore>,
    code: String,
    mut memory: DeviceMemory,
) {
    let mut session = Arc::clone(&shared_session.lock().unwrap_or_else(|p| p.into_inner()));
    let mut content_task: Option<tokio::task::JoinHandle<()>> = None;
    // Width changes go to the running content task; the last one also applies to the next.
    let mut task_widths: Option<mpsc::UnboundedSender<u32>> = None;
    let mut target_width: Option<u32> = None;
    let mut widths_open = true;
    let emit = |e: ReceiverEvent| {
        let _ = out.send(e);
    };
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut conn = Conn::default();
    // The name of the connected presenter (the content task needs it for hand-over tokens) and
    // the join that was let in with a token and whose `peer_joined` is still to come.
    let mut peer_name: Option<String> = None;
    let mut admitted_by_token = TokenAdmission::default();
    // Verified devices of the waiting requests (by request id) and of the requests that were
    // allowed and whose `peer_joined` is still to come (docs/design/presenter-identity.md).
    let mut request_devices = RequestDevices::default();
    // The presenter the host asked to remove, until its `peer_left` arrives.
    let mut removed_peer: Option<String> = None;
    let (hand_tx, mut hand_rx) = mpsc::unbounded_channel::<content::TaskMsg>();
    // The pause flags of the live screen and the way to tell the presenter (the content task
    // of the running presentation sends the texts on its control channel).
    let mut pause = PauseCtl::default();
    let mut display_pause_open = true;
    // Why the loop ended: stop, a fatal error or the end of the room all close the session below.
    loop {
        // The sender is gone or its presentation ended: forget its connection and start
        // afresh. (`reset` is set by the arms below; the new session is made after the select.)
        let mut reset = false;
        tokio::select! {
            _ = &mut stop => break,
            cmd = commands.recv() => {
                // The core itself holds the sender of this channel, so `None` cannot happen
                // before it is dropped (which stops the loop too).
                if let Some(cmd) = &cmd {
                    // The host turned against somebody: hand-over tokens must not let them
                    // straight back in (an ordinary leave keeps them, see `PeerLeft`).
                    let withdrawn = match cmd {
                        ClientMsg::RemovePeer { .. } => {
                            removed_peer = peer_name.clone();
                            peer_name.as_deref().map_or(0, |n| handover.remove_for(n))
                        }
                        ClientMsg::JoinDecision { block: true, .. } => handover.clear(),
                        _ => 0,
                    };
                    if let ClientMsg::JoinDecision { id: Some(id), allow, .. } = cmd {
                        request_devices.decided(*id, *allow);
                    }
                    if withdrawn > 0 {
                        emit(ReceiverEvent::HandoverExpired);
                    }
                }
                if let Some(cmd) = cmd
                    && sig.send(cmd).await.is_err()
                {
                    emit(ReceiverEvent::Error("signalling connection closed".into()));
                    break;
                }
            }
            w = async {
                match content.as_mut() {
                    Some(c) if widths_open => c.widths.recv().await,
                    _ => std::future::pending().await,
                }
            } => match w {
                Some(w) => {
                    target_width = Some(w);
                    if let Some(tx) = &task_widths {
                        let _ = tx.send(w);
                    }
                }
                None => widths_open = false,
            },
            _ = async {
                match content_task.as_mut() {
                    Some(t) => { let _ = t.await; }
                    None => std::future::pending().await,
                }
            } => {
                // The control channel of the sender ended while it is still in the room (a
                // leaving sender is told by the server: `PeerLeft` below): its presentation
                // is over.
                content_task = None;
                task_widths = None;
                pause.end(&emit);
                emit(ReceiverEvent::PresentationStopped);
                reset = true;
            }
            Some(m) = hand_rx.recv() => match m {
                content::TaskMsg::Event(e) => emit(e),
                // Only while a screen is shown (a video track arrived).
                content::TaskMsg::PresenterPause(p) => {
                    pause.set_presenter(p && conn.have_track, &emit);
                }
            },
            d = async {
                if display_pause_open {
                    display_pause.recv().await
                } else {
                    std::future::pending().await
                }
            } => match d {
                Some(p) => pause.set_display(p && conn.have_track, &emit),
                None => display_pause_open = false,
            },
            msg = sig.recv() => match msg {
                None => {
                    emit(ReceiverEvent::Error("signalling connection closed".into()));
                    break;
                }
                Some(ServerMsg::PeerJoined { name, handover: token, device }) => {
                    // A join that needed no decision (join mode open) carries its token here;
                    // one that was allowed by us below was checked when the request came.
                    let by_token = admitted_by_token.joined(&name)
                        || token.is_some_and(|t| handover.take(&t, &name, Instant::now()));
                    peer_name = Some(name.clone());
                    // Open mode: the proof came with the join. Confirm mode: the device of the
                    // request the host allowed. The note is for open mode only; in confirm mode
                    // the dialog showed it.
                    let from_join = device
                        .as_ref()
                        .and_then(|d| verify_join(d, &code, unix_ms()).ok());
                    let other_device = from_join
                        .as_deref()
                        .is_some_and(|id| memory.used_by_other_device(&name, id));
                    let device = from_join.or_else(|| request_devices.joined(&name));
                    if let Some(id) = &device
                        && memory.remember(&name, id)
                    {
                        emit(ReceiverEvent::DeviceMemoryChanged(memory.to_json()));
                    }
                    emit(ReceiverEvent::PeerJoined { name, handover: by_token, device, other_device });
                }
                Some(ServerMsg::JoinRequest { id, name, check, handover: token, device }) => {
                    // Only a proof that verifies for this room counts; anything else is shown
                    // as before, with no device note (never as "verified").
                    let device = device.and_then(|d| verify_join(&d, &code, unix_ms()).ok());
                    let other_device = device
                        .as_deref()
                        .is_some_and(|d| memory.used_by_other_device(&name, d));
                    if let Some(d) = &device {
                        request_devices.requested(id, &name, d);
                    }
                    // A token this display issued to this person: let them in without asking.
                    // Anything else (no token, expired, used, another name, another room) is
                    // an ordinary request for the host.
                    if token.is_some_and(|t| handover.take(&t, &name, Instant::now())) {
                        admitted_by_token.admitted(id, name);
                        request_devices.decided(id, true);
                        let _ = sig
                            .send(ClientMsg::JoinDecision { id: Some(id), allow: true, block: false })
                            .await;
                    } else {
                        emit(ReceiverEvent::JoinRequest { id, name, check, device, other_device });
                    }
                }
                Some(ServerMsg::JoinRequestCancelled { id, reason }) => {
                    admitted_by_token.cancelled(id);
                    request_devices.cancelled(id);
                    emit(ReceiverEvent::JoinRequestCancelled { id, reason })
                }
                Some(ServerMsg::JoinModeChanged { join_mode }) => {
                    emit(ReceiverEvent::JoinModeChanged(join_mode));
                    // Nobody may join a locked room, a hand-over join included. (`confirm`
                    // keeps the tokens: they stand for an approval the host already gave.)
                    if join_mode == JoinMode::Locked && handover.clear() > 0 {
                        emit(ReceiverEvent::HandoverExpired);
                    }
                }
                Some(ServerMsg::Blocked { id, name }) => emit(ReceiverEvent::Blocked { id, name }),
                Some(ServerMsg::Unblocked { id }) => emit(ReceiverEvent::Unblocked { id }),
                Some(ServerMsg::RoomEnded) => {
                    emit(ReceiverEvent::RoomEnded);
                    break;
                }
                Some(ServerMsg::Offer { sdp }) => {
                    if conn.offer_at.is_some() {
                        // A second offer in a connection that had one: the sender begins a
                        // new presentation. The old one is over; start on a fresh session.
                        stop_presentation(&mut content_task, &mut task_widths);
                        pause.end(&emit);
                        emit(ReceiverEvent::PresentationStopped);
                        match fresh_session(&session, &session_config, &shared_session).await {
                            Ok((s, ev)) => {
                                session = s;
                                session_events = ev;
                                conn = Conn::default();
                            }
                            Err(e) => {
                                emit(ReceiverEvent::Error(format!("cannot start a session: {e}")));
                                break;
                            }
                        }
                    }
                    conn.offer_at.get_or_insert_with(tokio::time::Instant::now);
                    match session.accept_offer(sdp).await {
                        Ok(answer) => {
                            if sig.send(ClientMsg::Answer { sdp: answer }).await.is_err() {
                                emit(ReceiverEvent::Error("signalling connection closed".into()));
                                break;
                            }
                        }
                        // An offer we cannot answer is the presenter's: it ends this
                        // presentation, not the room (any joiner could send one).
                        Err(_) => {
                            stop_presentation(&mut content_task, &mut task_widths);
                            pause.end(&emit);
                            emit(ReceiverEvent::PresentationStopped);
                            reset = true;
                        }
                    }
                }
                Some(ServerMsg::IceCandidate { candidate }) => {
                    // A bad candidate is not fatal: others may still work.
                    let _ = session.add_remote_candidate(&candidate).await;
                }
                Some(ServerMsg::PeerLeft) => {
                    // Content that is still on its way is handed over first.
                    if let Some(mut t) = content_task.take()
                        && tokio::time::timeout(CONTENT_DRAIN_TIMEOUT, &mut t).await.is_err()
                    {
                        t.abort();
                    }
                    task_widths = None;
                    peer_name = None;
                    // A presenter the host removed: a token its content task stored after
                    // `remove_peer` was sent is withdrawn too (the task has ended above).
                    if let Some(n) = removed_peer.take()
                        && handover.remove_for(&n) > 0
                    {
                        emit(ReceiverEvent::HandoverExpired);
                    }
                    pause.end(&emit);
                    emit(ReceiverEvent::PeerLeft);
                    reset = true;
                }
                Some(ServerMsg::Error { code, message }) => {
                    if server_error_ends_room(&code) {
                        emit(ReceiverEvent::Error(format!("server error {code}: {message}")));
                        break;
                    }
                    emit(ReceiverEvent::ServerNotice { code, message });
                }
                // hello/room_created/joined/answer/control: nothing to do here yet.
                Some(_) => {}
            },
            ev = session_events.recv() => match ev {
                None => break,
                Some(SessionEvent::LocalIceCandidate(c)) => {
                    let _ = sig.send(ClientMsg::IceCandidate { candidate: c }).await;
                }
                Some(SessionEvent::State(s)) => {
                    emit(ReceiverEvent::State(s));
                    if s == ConnectionState::Connected && !conn.connected {
                        conn.connected = true;
                        if let Some(code) = session.connection_code() {
                            emit(ReceiverEvent::ConnectionCode(code));
                        }
                    }
                    if s == ConnectionState::Failed {
                        // The presenter's connection failed: its presentation is over, the
                        // room stays (the presenter may offer again, or the next one comes).
                        stop_presentation(&mut content_task, &mut task_widths);
                        pause.end(&emit);
                        emit(ReceiverEvent::PresentationStopped);
                        reset = true;
                    }
                }
                Some(SessionEvent::ControlChannel(channel)) => match content.as_ref() {
                    Some(c) => {
                        let mut options = c.options.clone();
                        if let Some(w) = target_width {
                            options.target_width = w;
                        }
                        let (wtx, wrx) = mpsc::unbounded_channel();
                        task_widths = Some(wtx);
                        // The presenter learns the pause flags as soon as the channel is
                        // there (both `false` unless the display paused meanwhile).
                        let (down_tx, down_rx) = mpsc::unbounded_channel();
                        pause.down = Some(down_tx);
                        pause.tell();
                        content_task = Some(content::spawn(
                            channel,
                            options,
                            wrx,
                            Arc::clone(&c.events),
                            &c.room,
                            peer_name.clone().map(|name| content::HandoverHook {
                                store: Arc::clone(&handover),
                                name,
                                notify: hand_tx.clone(),
                            }),
                            content::PauseLink {
                                up: hand_tx.clone(),
                                down: down_rx,
                            },
                        ));
                    }
                    // Content is not wanted: refuse the channel.
                    None => channel.close().await,
                },
                Some(SessionEvent::VideoTrack { mime, codec }) => {
                    conn.have_track = true;
                    emit(ReceiverEvent::VideoTrack { mime, codec });
                }
            },
            _ = tick.tick() => {
                if handover.expire(Instant::now()) > 0 {
                    emit(ReceiverEvent::HandoverExpired);
                }
                if !conn.connected && conn.offer_at.is_some_and(|t| t.elapsed() > CONNECT_TIMEOUT) {
                    // No connection after the presenter's offer: as a failed connection.
                    stop_presentation(&mut content_task, &mut task_widths);
                    pause.end(&emit);
                    emit(ReceiverEvent::PresentationStopped);
                    reset = true;
                } else if conn.have_track {
                    emit(ReceiverEvent::Stats(session.stats()));
                }
            }
        }
        if reset {
            pause.end(&emit);
            match fresh_session(&session, &session_config, &shared_session).await {
                Ok((s, ev)) => {
                    session = s;
                    session_events = ev;
                    conn = Conn::default();
                }
                Err(e) => {
                    emit(ReceiverEvent::Error(format!("cannot start a session: {e}")));
                    break;
                }
            }
        }
    }
    if let Some(t) = content_task {
        t.abort();
    }
    let _ = session.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_read_from_the_messages_of_this_crate() {
        assert_eq!(
            error_code("server error room_full: the room is full"),
            "room_full"
        );
        assert_eq!(
            error_code("signalling connection closed"),
            "connection_lost"
        );
        assert_eq!(error_code("WebRTC connection failed"), "peer_failed");
        assert_eq!(
            error_code("no WebRTC connection 30 s after the offer arrived"),
            "peer_failed"
        );
        assert_eq!(error_code("cannot answer offer: x"), "peer_failed");
        assert_eq!(error_code("something else"), "unknown");
    }

    #[test]
    fn only_server_errors_that_close_the_connection_end_the_room() {
        for code in ["bad_hello", "proto_mismatch", "banned"] {
            assert!(server_error_ends_room(code), "{code}");
        }
        for code in [
            "no_such_request",
            "no_such_block",
            "block_list_full",
            "not_in_room",
            "rate_limited",
            "wrong_role",
            "bad_message",
            "open_rooms_disabled",
            "join_pending",
            "something_new",
        ] {
            assert!(!server_error_ends_room(code), "{code}");
        }
    }

    #[test]
    fn a_cancelled_token_join_does_not_mark_a_later_join_as_a_handover() {
        let mut a = TokenAdmission::default();
        a.admitted(7, "Ada".into());
        // Another request going away does not matter.
        a.cancelled(8);
        assert!(a.joined("Ada"), "the admitted join itself");
        assert!(!a.joined("Ada"), "used once");

        a.admitted(9, "Ada".into());
        a.cancelled(9);
        assert!(!a.joined("Ada"), "a later ordinary join is no hand-over");

        a.admitted(10, "Ada".into());
        assert!(!a.joined("Ben"));
        assert!(!a.joined("Ada"), "cleared by the join that did arrive");
    }
}
