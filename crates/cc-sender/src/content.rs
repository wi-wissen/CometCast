// SPDX-License-Identifier: GPL-3.0-or-later
//! Content sending over the `control` data channel: images, PDF documents, links and page
//! commands (message format: docs/web-m2.md; logic: `cc-files`).
//!
//! * [`ControlSender`] wraps an open channel. Sending a transfer waits whenever the channel's
//!   send buffer is full (back-pressure from `cc-transport`), so memory stays bounded.
//! * [`ContentSender`] is a content-only sender (no video): joins the room, offers a session
//!   that has only the data channel, and exposes a [`ControlSender`]. The web client has the
//!   same mode ("Send file or link").
//! * [`SenderCore`](crate::SenderCore) can also carry a channel next to the video
//!   ([`SenderOptions::control`](crate::SenderOptions::control)).

use std::sync::Arc;
use std::time::Duration;

use cc_files::{
    ImageMime, Outgoing, SendError, cmd_message, document_transfer, image_transfer, link_message,
};
use cc_proto::{ClientMsg, ControlCmd, Role, RoomCode, ServerMsg};
use cc_transport::JoinProgress;
use cc_transport::{
    ConnectionState, ControlChannel, ServerAddr, Session, SessionConfig, SessionEvent,
    SignalingClient, TlsTrust, TransportError, ice, join_room,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::SenderEvent;

/// How long [`ControlSender::wait_ready`] callers typically wait for the channel to open.
/// UNCERTAIN: placeholder, not measured.
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long closing the channel may wait for the peer. UNCERTAIN: placeholder.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

/// How long [`ControlSender::finish`] waits for the data to leave. UNCERTAIN: placeholder.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(30);

/// Why content could not be sent.
#[derive(Debug, thiserror::Error)]
pub enum ContentSendError {
    /// The content itself was refused before anything was sent (empty, too large, not a PDF,
    /// bad link).
    #[error("{0}")]
    Content(#[from] SendError),
    /// The channel failed.
    #[error("{0}")]
    Transport(#[from] TransportError),
}

/// Sends content on an open `control` channel. Cheap to clone.
#[derive(Debug, Clone)]
pub struct ControlSender {
    channel: ControlChannel,
}

impl ControlSender {
    /// Wrap a channel (from [`Session::create_control_channel`]).
    pub fn new(channel: ControlChannel) -> Self {
        Self { channel }
    }

    /// The channel itself.
    pub fn channel(&self) -> &ControlChannel {
        &self.channel
    }

    /// Wait until the channel is open (the peer is connected).
    pub async fn wait_ready(&self, timeout: Duration) -> Result<(), TransportError> {
        self.channel.wait_open(timeout).await
    }

    async fn run<'a>(
        &self,
        items: impl Iterator<Item = Outgoing<'a>>,
    ) -> Result<(), ContentSendError> {
        for item in items {
            match item {
                Outgoing::Text(t) => self.channel.send_text(&t).await?,
                Outgoing::Binary(b) => self.channel.send_binary(b).await?,
            }
        }
        Ok(())
    }

    /// Send an image (PNG, JPEG, WebP or GIF, at most 10 MiB). The bytes are not decoded here.
    pub async fn send_image(
        &self,
        name: &str,
        mime: ImageMime,
        bytes: &[u8],
    ) -> Result<(), ContentSendError> {
        self.run(image_transfer(name, mime, bytes)?).await
    }

    /// Send a PDF document. The receiver renders it and shows the first page; use
    /// [`Self::send_cmd`] to turn pages.
    pub async fn send_document(&self, name: &str, bytes: &[u8]) -> Result<(), ContentSendError> {
        self.run(document_transfer(name, bytes)?).await
    }

    /// Send a link. Only `http` and `https` are allowed. The receiver's person decides
    /// whether to open it.
    pub async fn send_link(&self, url: &str) -> Result<(), ContentSendError> {
        let m = link_message(url)?;
        self.channel.send_text(&m).await?;
        Ok(())
    }

    /// Send a remote-control command (next/previous page, go to page, stop).
    pub async fn send_cmd(&self, cmd: ControlCmd) -> Result<(), ContentSendError> {
        self.channel.send_text(&cmd_message(cmd)).await?;
        Ok(())
    }

    /// Wait until everything sent has left (acknowledged by the peer's transport), at most
    /// `timeout`. `false` on timeout or when the channel closed.
    pub async fn flush(&self, timeout: Duration) -> bool {
        self.channel.flush(timeout).await
    }

    /// [`Self::flush`] with the default timeout. Call it before stopping a content-only
    /// sender, otherwise the end of a transfer can be lost.
    pub async fn finish(&self) -> bool {
        self.flush(FLUSH_TIMEOUT).await
    }
}

/// Settings for [`ContentSender::start`].
#[derive(Debug, Clone)]
pub struct ContentSenderOptions {
    /// Signalling server as `host:port` (or a full `ws://` URL).
    pub server: String,
    /// Room code from the receiver.
    pub code: RoomCode,
    /// WebRTC settings. If `session.ice_servers` is empty the core asks the server for its list.
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://`.
    pub trust: TlsTrust,
    /// Name shown on the display device when it has to approve the join.
    pub name: Option<String>,
    /// Receives [`JoinProgress::Pending`] when the room waits for the host's approval.
    pub join_progress: Option<mpsc::UnboundedSender<JoinProgress>>,
}

impl ContentSenderOptions {
    /// Options for the given server and code with default settings.
    pub fn new(server: impl Into<String>, code: RoomCode) -> Self {
        Self {
            server: server.into(),
            code,
            session: SessionConfig::default(),
            trust: TlsTrust::system(),
            name: None,
            join_progress: None,
        }
    }
}

/// A content-only sender: a WebRTC session with a data channel and no media.
pub struct ContentSender {
    room_name: Option<String>,
    control: ControlSender,
    events: mpsc::UnboundedReceiver<SenderEvent>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl ContentSender {
    /// Join the room and send the offer. Returns after the offer was sent; a wrong code fails
    /// here with `room_not_found`. Then call [`ControlSender::wait_ready`] before sending.
    pub async fn start(mut options: ContentSenderOptions) -> Result<Self, TransportError> {
        let addr = ServerAddr::parse(&options.server)?;
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Sender, &options.trust).await?;
        let room_name = join_room(
            &mut sig,
            options.code.clone(),
            options.name.clone(),
            options.join_progress.as_ref(),
        )
        .await?;

        // No video codecs at all: the offer has only the application section.
        options.session.video_codecs = Some(Vec::new());
        if options.session.ice_servers.is_empty() {
            options.session.ice_servers =
                ice::fetch_ice_servers_or_none(&addr, &options.trust).await;
        }
        let (session, session_events) = Session::new(options.session).await?;
        let channel = session.create_control_channel().await?;
        let offer = session.create_offer().await?;
        sig.send(ClientMsg::Offer { sdp: offer }).await?;

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let task = tokio::spawn(run(
            sig,
            Arc::new(session),
            channel.clone(),
            session_events,
            events_tx,
            stop_rx,
        ));
        Ok(Self {
            room_name,
            control: ControlSender::new(channel),
            events: events_rx,
            stop: Some(stop_tx),
            task,
        })
    }

    /// The name the display device gave its room (`joined.room_name`); `None` when it gave
    /// none or the server is older.
    pub fn room_name(&self) -> Option<&str> {
        self.room_name.as_deref()
    }

    /// The handle to send content with.
    pub fn control(&self) -> ControlSender {
        self.control.clone()
    }

    /// Next event, or `None` after the sender has stopped and all events were read.
    pub async fn next_event(&mut self) -> Option<SenderEvent> {
        self.events.recv().await
    }

    /// Stop and close everything. Call [`ControlSender::finish`] first if content was sent.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for ContentSender {
    fn drop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
    }
}

async fn run(
    mut sig: SignalingClient,
    session: Arc<Session>,
    channel: ControlChannel,
    mut session_events: mpsc::UnboundedReceiver<SessionEvent>,
    out: mpsc::UnboundedSender<SenderEvent>,
    mut stop: oneshot::Receiver<()>,
) {
    let emit = |e: SenderEvent| {
        let _ = out.send(e);
    };
    loop {
        tokio::select! {
            _ = &mut stop => break,
            msg = sig.recv() => match msg {
                None => {
                    emit(SenderEvent::Error("signalling connection closed".into()));
                    break;
                }
                Some(ServerMsg::Answer { sdp }) => {
                    if let Err(e) = session.accept_answer(sdp).await {
                        emit(SenderEvent::Error(format!("cannot use answer: {e}")));
                        break;
                    }
                }
                Some(ServerMsg::IceCandidate { candidate }) => {
                    let _ = session.add_remote_candidate(&candidate).await;
                }
                Some(ServerMsg::PeerLeft) => {
                    emit(SenderEvent::PeerLeft);
                    break;
                }
                Some(ServerMsg::Error { code, .. }) if code == "removed_by_host" => {
                    emit(SenderEvent::RemovedByHost);
                    break;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(SenderEvent::Error(format!("server error {code}: {message}")));
                    break;
                }
                Some(_) => {}
            },
            ev = session_events.recv() => match ev {
                None => break,
                Some(SessionEvent::LocalIceCandidate(c)) => {
                    let _ = sig.send(ClientMsg::IceCandidate { candidate: c }).await;
                }
                Some(SessionEvent::State(s)) => {
                    emit(SenderEvent::State(s));
                    if s == ConnectionState::Failed {
                        emit(SenderEvent::Error("WebRTC connection failed".into()));
                        break;
                    }
                }
                Some(_) => {}
            },
        }
    }
    // Closing the channel first lets the peer deliver the last messages before the connection
    // goes away.
    channel.close_graceful(CLOSE_TIMEOUT).await;
    let _ = session.close().await;
}
