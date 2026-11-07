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
//! reported as [`ReceiverEvent::PresentationStopped`]; the session is made fresh as well.
//! The core stops on [`ReceiverEvent::RoomEnded`], an error or [`ReceiverCore::stop`].
//!
//! Use: [`ReceiverCore::start`] -> show [`ReceiverCore::room_code`] -> loop on
//! [`ReceiverCore::next_event`].

mod content;

use std::sync::Arc;
use std::time::Duration;

pub use cc_files::{Blob, ContentError, ContentEvent, ContentOptions, ImageMime};
pub use cc_proto::JoinMode;
use cc_proto::{ClientMsg, Role, RoomCode, ServerMsg};
use cc_transport::{
    ConnectionState, IceServerConfig, MediaSnapshot, ServerAddr, Session, SessionConfig,
    SessionEvent, SignalingClient, TlsTrust, TransportError, VideoCodec, altcha, ice,
};
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
    },
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
    /// The sender stopped its presentation but is still in the room (it closed its control
    /// channel, or began a new offer). Whatever is shown should go; the next presentation
    /// of the same sender needs no new approval.
    PresentationStopped,
    /// The connection check code (`"123 456"`) of the sender's connection, once per connection,
    /// as soon as it is connected. Identical to the one the sender shows (`cc_transport::sas`).
    ConnectionCode(String),
    /// Something failed. The core stops after this event.
    Error(String),
}

/// The code of a [`ReceiverEvent::Error`] message, for the apps (native-apps.md 20.2): the
/// protocol code of a `server error <code>: ...`, `connection_lost` when the signalling
/// connection ended, `peer_failed` when WebRTC failed, `unknown` otherwise. The messages are
/// the ones made in this file; the text itself is for logs only.
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
        let (content_tx, content_rx) = content::event_queue();
        let content = options.content.map(|mut o| {
            // The apps show a progress for incoming files.
            o.progress = true;
            ContentSetup {
                options: o,
                widths: width_rx,
                events: Arc::new(content_tx),
            }
        });
        let has_content = content.is_some();
        let task = tokio::spawn(run(
            sig,
            Arc::clone(&session),
            session_config,
            session_events,
            events_tx,
            stop_rx,
            cmd_rx,
            content,
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
    /// session (it then gets `join_denied` at once, and the host is not asked again).
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
    /// and waits for the next sender in the same room.
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
}

/// What belongs to the connection of one sender and starts over with the next one.
#[derive(Default)]
struct Conn {
    have_track: bool,
    connected: bool,
    offer_at: Option<tokio::time::Instant>,
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
    mut content: Option<ContentSetup>,
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
                emit(ReceiverEvent::PresentationStopped);
                reset = true;
            }
            msg = sig.recv() => match msg {
                None => {
                    emit(ReceiverEvent::Error("signalling connection closed".into()));
                    break;
                }
                Some(ServerMsg::PeerJoined { name }) => emit(ReceiverEvent::PeerJoined { name }),
                Some(ServerMsg::JoinRequest { id, name, check }) => {
                    emit(ReceiverEvent::JoinRequest { id, name, check })
                }
                Some(ServerMsg::JoinRequestCancelled { id, reason }) => {
                    emit(ReceiverEvent::JoinRequestCancelled { id, reason })
                }
                Some(ServerMsg::JoinModeChanged { join_mode }) => {
                    emit(ReceiverEvent::JoinModeChanged(join_mode))
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
                        if let Some(t) = content_task.take() {
                            t.abort();
                        }
                        task_widths = None;
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
                    let answer = match session.accept_offer(sdp).await {
                        Ok(a) => a,
                        Err(e) => {
                            emit(ReceiverEvent::Error(format!("cannot answer offer: {e}")));
                            break;
                        }
                    };
                    if sig.send(ClientMsg::Answer { sdp: answer }).await.is_err() {
                        emit(ReceiverEvent::Error("signalling connection closed".into()));
                        break;
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
                    emit(ReceiverEvent::PeerLeft);
                    reset = true;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(ReceiverEvent::Error(format!("server error {code}: {message}")));
                    break;
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
                        emit(ReceiverEvent::Error("WebRTC connection failed".into()));
                        break;
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
                        content_task = Some(content::spawn(
                            channel,
                            options,
                            wrx,
                            Arc::clone(&c.events),
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
                if !conn.connected && conn.offer_at.is_some_and(|t| t.elapsed() > CONNECT_TIMEOUT) {
                    emit(ReceiverEvent::Error(format!(
                        "no WebRTC connection {} s after the offer arrived",
                        CONNECT_TIMEOUT.as_secs()
                    )));
                    break;
                }
                if conn.have_track {
                    emit(ReceiverEvent::Stats(session.stats()));
                }
            }
        }
        if reset {
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
}
