// SPDX-License-Identifier: GPL-3.0-or-later
//! The presenter session: join the room **once**, present several times (native-apps.md 20.1).
//!
//! [`PresenterSession::join`] connects and joins the room (the host approves once). Inside the
//! joined session [`PresenterSession::present`] starts a presentation on a fresh WebRTC session
//! over the same signalling connection (a screen, host-pushed H.264, or content only) and
//! [`PresenterSession::stop_presenting`] ends only that presentation: the display returns to
//! Idle with the same code, this sender stays in the room, and the next presentation needs no
//! new approval. [`PresenterSession::leave`] leaves the room.
//!
//! How the display learns that a presentation ended while this sender stays: every
//! presentation has a `control` channel next to the media; stopping sends `cmd stop` (the web
//! display closes what it shows), then closes the channel gracefully. The native receiver
//! reports the end of the channel as `PresentationStopped`; a new offer on the same signalling
//! connection does the same. UNCERTAIN: a web display does not make a fresh session for the
//! next offer of a sender that is still in the room (it never needed to), so a second
//! presentation to a web display is not covered here.
//!
//! The display's `page` messages are read from the channel and reported as
//! [`PresenterEvent::Page`]; the connection check code as [`PresenterEvent::ConnectionCode`].

use std::time::Duration;

use cc_proto::{ControlCmd, Role, RoomCode, ServerMsg};
use cc_transport::{
    ConnectionState, JoinProgress, ServerAddr, SessionConfig, SignalingClient, TlsTrust,
    TransportError, ice, join_room,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::content::{ContentSender, ContentSenderOptions, ControlSender};
use crate::{
    ExternalInput, RunEnd, ScreenOptions, SenderCore, SenderEvent, SenderOptions, StartFailed,
};

/// How long a stop waits for `cmd stop` to leave. UNCERTAIN: placeholder.
const STOP_FLUSH: Duration = Duration::from_secs(2);

/// Settings for [`PresenterSession::join`].
#[derive(Debug, Clone)]
pub struct PresenterOptions {
    /// Signalling server as `host:port` (or a full `ws://` URL).
    pub server: String,
    /// Room code from the display.
    pub code: RoomCode,
    /// WebRTC settings of every presentation. If `session.ice_servers` is empty each
    /// presentation asks the server for its STUN/TURN list.
    pub session: SessionConfig,
    /// Extra trusted certificates for `wss://` / `https://`.
    pub trust: TlsTrust,
    /// Name shown on the display when it has to approve the join.
    pub name: Option<String>,
    /// Receives [`JoinProgress::Pending`] when the room waits for the host's approval.
    pub join_progress: Option<mpsc::UnboundedSender<JoinProgress>>,
}

impl PresenterOptions {
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

/// How the server's STUN/TURN list (`/ice-servers`) turned out for this session (spec 6,
/// decision 13.45): only when a connection then fails the apps use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IceState {
    /// A list, none (404) or the caller's own: nothing to say.
    Fine,
    /// The server refused the list (`rate_limited`, `banned`, `forbidden`).
    Refused,
    /// The request failed otherwise (unexpected status, no answer, a bad answer).
    Failed,
}

impl IceState {
    fn of(e: &TransportError) -> Self {
        match e {
            TransportError::Server { code, .. }
                if matches!(code.as_str(), "rate_limited" | "banned" | "forbidden") =>
            {
                Self::Refused
            }
            _ => Self::Failed,
        }
    }

    /// The code of a failed WebRTC connection: `ice_refused` or `ice_failed` when the list
    /// was the problem, else the code as it is.
    fn refine(self, code: String) -> String {
        match (code.as_str(), self) {
            ("peer_failed", Self::Refused) => "ice_refused".into(),
            ("peer_failed", Self::Failed) => "ice_failed".into(),
            _ => code,
        }
    }
}

/// What to present.
#[derive(Debug, Clone)]
pub enum Presentation {
    /// Content (images, PDFs, links): a data channel without media.
    Content,
    /// Media: the screen (`screen`), H.264 pushed by the host (`external`) or, with neither,
    /// the built-in test stream. The control channel opens next to the video.
    Media {
        /// Capture the screen.
        screen: Option<ScreenOptions>,
        /// H.264 access units the host pushes.
        external: Option<ExternalInput>,
        /// Stop by itself after this long.
        duration: Option<Duration>,
    },
}

/// Why the session ended (terminal): [`PresenterEvent::Ended`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The host removed this sender (`removed_by_host`).
    RemovedByHost,
    /// The display left the room: the room is gone (`display_left`).
    DisplayLeft,
}

impl EndReason {
    /// The code the apps use.
    pub fn code(self) -> &'static str {
        match self {
            Self::RemovedByHost => "removed_by_host",
            Self::DisplayLeft => "display_left",
        }
    }
}

/// What a [`PresenterSession`] reports after the join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenterEvent {
    /// The WebRTC connection of the running presentation changed state.
    State(ConnectionState),
    /// The connection check code of the running presentation (`"123 456"`), once per connection.
    ConnectionCode(String),
    /// The display shows this PDF page of that many (decision 13.31).
    Page {
        /// The shown page, from 1.
        page: u32,
        /// The page count, from 1.
        pages: u32,
    },
    /// Frames sent so far, about once per second while streaming media.
    Streaming {
        /// Frames written to the track.
        frames_sent: u64,
    },
    /// The presentation is connected (media flows, or the channel is open).
    Presenting,
    /// The presentation ended and the session is still joined: on request, by the duration, or
    /// because the system ended the capture.
    PresentingStopped,
    /// The session is over: the host removed this sender or the display left. Terminal.
    Ended(EndReason),
    /// Something failed. Terminal. `code` is a protocol code (`room_locked`, ...) or a local one
    /// (`connection_lost`, `peer_failed`, `capture_unavailable`, `unknown`); `message` is for
    /// logs only.
    Error {
        /// The code the apps map.
        code: String,
        /// English text for logs.
        message: String,
    },
}

enum Cmd {
    Present {
        what: Presentation,
        reply: oneshot::Sender<Result<ControlSender, TransportError>>,
    },
    StopPresenting,
    Leave,
}

/// A joined session. See the module docs.
pub struct PresenterSession {
    room_name: Option<String>,
    cmds: mpsc::UnboundedSender<Cmd>,
    events: mpsc::UnboundedReceiver<PresenterEvent>,
    task: JoinHandle<()>,
}

impl PresenterSession {
    /// Connect and join the room. Waits for the host's approval where the room asks for it
    /// (progress on [`PresenterOptions::join_progress`]); dropping the future withdraws the
    /// request. Fails with the server's error (`room_not_found`, `join_denied`, ...).
    pub async fn join(options: PresenterOptions) -> Result<Self, TransportError> {
        let mut sig =
            SignalingClient::connect_with(&options.server, Role::Sender, &options.trust).await?;
        let room_name = join_room(
            &mut sig,
            options.code.clone(),
            options.name.clone(),
            options.join_progress.as_ref(),
        )
        .await?;
        // Ask for the STUN/TURN list once per session; every presentation uses the outcome.
        let mut options = options;
        let mut ice_state = IceState::Fine;
        if options.session.ice_servers.is_empty() {
            let addr = ServerAddr::parse(&options.server)?;
            match ice::fetch_ice_servers(&addr, &options.trust).await {
                Ok(list) => options.session.ice_servers = list,
                Err(e) => {
                    eprintln!(
                        "cometcast: GET /ice-servers {}; connecting without STUN/TURN servers",
                        ice::describe_ice_failure(&e)
                    );
                    ice_state = IceState::of(&e);
                }
            }
        }
        let (cmds_tx, cmds_rx) = mpsc::unbounded_channel();
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(sig, options, ice_state, cmds_rx, events_tx));
        Ok(Self {
            room_name: room_name.clone(),
            cmds: cmds_tx,
            events: events_rx,
            task,
        })
    }

    /// The name the display gave its room, if any (decision 49: lenient).
    pub fn room_name(&self) -> Option<&str> {
        self.room_name.as_deref()
    }

    /// Start a presentation. Returns after the offer was sent with the handle of the
    /// presentation's control channel (wait for [`ControlSender::wait_ready`] before sending
    /// content). Fails when a presentation runs already, or when it cannot start (for example
    /// the capture is refused: then the session stays joined).
    pub async fn present(&self, what: Presentation) -> Result<ControlSender, TransportError> {
        let (reply, answer) = oneshot::channel();
        self.cmds
            .send(Cmd::Present { what, reply })
            .map_err(|_| TransportError::Closed)?;
        answer.await.map_err(|_| TransportError::Closed)?
    }

    /// End the running presentation only. The session stays joined; the end is reported as
    /// [`PresenterEvent::PresentingStopped`].
    pub fn stop_presenting(&self) {
        let _ = self.cmds.send(Cmd::StopPresenting);
    }

    /// Next event, or `None` after the session ended and all events were read.
    pub async fn next_event(&mut self) -> Option<PresenterEvent> {
        self.events.recv().await
    }

    /// Leave the room (a running presentation ends with it).
    pub async fn leave(mut self) {
        let _ = self.cmds.send(Cmd::Leave);
        let _ = (&mut self.task).await;
    }
}

impl Drop for PresenterSession {
    fn drop(&mut self) {
        let _ = self.cmds.send(Cmd::Leave);
    }
}

/// The running presentation.
struct Running {
    core: Core,
    control: ControlSender,
    pages: JoinHandle<()>,
    presenting: bool,
}

enum Core {
    Media(SenderCore),
    Content(ContentSender),
}

impl Core {
    async fn next_event(&mut self) -> Option<SenderEvent> {
        match self {
            Self::Media(c) => c.next_event().await,
            Self::Content(c) => c.next_event().await,
        }
    }

    fn request_stop(&mut self) {
        match self {
            Self::Media(c) => c.request_stop(),
            Self::Content(c) => c.request_stop(),
        }
    }

    async fn join(self) -> (Option<SignalingClient>, RunEnd) {
        match self {
            Self::Media(c) => c.join().await,
            Self::Content(c) => c.join().await,
        }
    }
}

async fn start(
    sig: SignalingClient,
    options: &PresenterOptions,
    what: Presentation,
) -> Result<Running, StartFailed> {
    match what {
        Presentation::Content => {
            let mut o = ContentSenderOptions::new(options.server.clone(), options.code.clone());
            o.session = options.session.clone();
            o.trust = options.trust.clone();
            o.skip_ice_fetch = true;
            let core = ContentSender::start_joined(sig, None, o).await?;
            let control = core.control();
            Ok(Running::new(Core::Content(core), control))
        }
        Presentation::Media {
            screen,
            external,
            duration,
        } => {
            let mut o = SenderOptions::new(options.server.clone(), options.code.clone());
            o.session = options.session.clone();
            o.trust = options.trust.clone();
            o.skip_ice_fetch = true;
            o.screen = screen;
            o.external = external;
            o.duration = duration;
            o.control = true;
            let core = SenderCore::start_joined(sig, None, o).await?;
            let control = core.control().expect("the control channel was asked for");
            Ok(Running::new(Core::Media(core), control))
        }
    }
}

impl Running {
    fn new(core: Core, control: ControlSender) -> Self {
        Self {
            core,
            control,
            pages: tokio::spawn(std::future::ready(())),
            presenting: false,
        }
    }
}

enum Next {
    Cmd(Option<Cmd>),
    Idle(Option<ServerMsg>),
    Running(Option<SenderEvent>),
    Page(Option<(u32, u32)>),
}

async fn run(
    sig: SignalingClient,
    options: PresenterOptions,
    ice_state: IceState,
    mut cmds: mpsc::UnboundedReceiver<Cmd>,
    out: mpsc::UnboundedSender<PresenterEvent>,
) {
    let emit = |e: PresenterEvent| {
        let _ = out.send(e);
    };
    // Between presentations this task holds the signalling connection itself.
    let mut sig: Option<SignalingClient> = Some(sig);
    let mut running: Option<Running> = None;
    let (page_tx, mut page_rx) = mpsc::unbounded_channel::<(u32, u32)>();
    let mut stopping = false;
    loop {
        let next = tokio::select! {
            c = cmds.recv() => Next::Cmd(c),
            m = async {
                // While a presentation runs it holds the connection (`sig` is `None`).
                match sig.as_mut() {
                    Some(s) => s.recv().await,
                    None => std::future::pending().await,
                }
            } => Next::Idle(m),
            e = async {
                match running.as_mut() {
                    Some(r) => r.core.next_event().await,
                    None => std::future::pending().await,
                }
            } => Next::Running(e),
            p = page_rx.recv() => Next::Page(p),
        };
        match next {
            Next::Cmd(None | Some(Cmd::Leave)) => break,
            Next::Cmd(Some(Cmd::Present { what, reply })) => {
                let Some(s) = sig.take().filter(|_| running.is_none()) else {
                    let _ = reply.send(Err(TransportError::Invalid(
                        "a presentation is running already".into(),
                    )));
                    continue;
                };
                match start(s, &options, what).await {
                    Ok(mut r) => {
                        let control = r.control.clone();
                        let tx = page_tx.clone();
                        let reader = control.clone();
                        r.pages = tokio::spawn(async move {
                            while let Some(p) = reader.recv_page().await {
                                let _ = tx.send(p);
                            }
                        });
                        stopping = false;
                        running = Some(r);
                        let _ = reply.send(Ok(control));
                    }
                    Err(StartFailed { sig: s, error }) => {
                        // The session stays joined; the caller decides what the error means.
                        sig = Some(s);
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Next::Cmd(Some(Cmd::StopPresenting)) => {
                if let Some(r) = running.as_mut()
                    && !stopping
                {
                    stopping = true;
                    // The web display closes what it shows on `cmd stop`; the native one
                    // sees the channel end.
                    let _ = r.control.send_cmd(ControlCmd::Stop).await;
                    let _ = r.control.flush(STOP_FLUSH).await;
                    r.core.request_stop();
                }
            }
            Next::Page(Some((page, pages))) => emit(PresenterEvent::Page { page, pages }),
            Next::Page(None) => {}
            Next::Running(Some(ev)) => match ev {
                SenderEvent::State(s) => {
                    if s == ConnectionState::Connected
                        && let Some(r) = running.as_mut()
                        && !r.presenting
                    {
                        r.presenting = true;
                        emit(PresenterEvent::Presenting);
                    }
                    emit(PresenterEvent::State(s));
                }
                SenderEvent::ConnectionCode(code) => emit(PresenterEvent::ConnectionCode(code)),
                SenderEvent::Streaming { frames_sent } => {
                    emit(PresenterEvent::Streaming { frames_sent });
                }
                // The end itself arrives with the join below.
                SenderEvent::Finished { .. }
                | SenderEvent::PeerLeft
                | SenderEvent::RemovedByHost
                | SenderEvent::Error(_) => {}
            },
            Next::Running(None) => {
                // The presentation's task ended: take the signalling connection back.
                let Some(r) = running.take() else { continue };
                r.pages.abort();
                let (back, end) = r.core.join().await;
                stopping = false;
                match end {
                    RunEnd::Stopped { .. } => {
                        emit(PresenterEvent::PresentingStopped);
                        match back {
                            Some(s) => sig = Some(s),
                            None => {
                                emit(error_event("unknown", "the sender task panicked"));
                                break;
                            }
                        }
                    }
                    RunEnd::PeerLeft => {
                        emit(PresenterEvent::Ended(EndReason::DisplayLeft));
                        break;
                    }
                    RunEnd::RemovedByHost => {
                        emit(PresenterEvent::Ended(EndReason::RemovedByHost));
                        break;
                    }
                    RunEnd::Failed { code, message } => {
                        emit(PresenterEvent::Error {
                            code: ice_state.refine(code),
                            message,
                        });
                        break;
                    }
                }
            }
            Next::Idle(msg) => match msg {
                None => {
                    emit(error_event(
                        "connection_lost",
                        "signalling connection closed",
                    ));
                    break;
                }
                Some(ServerMsg::PeerLeft) => {
                    emit(PresenterEvent::Ended(EndReason::DisplayLeft));
                    break;
                }
                Some(ServerMsg::Error { code, .. }) if code == "removed_by_host" => {
                    emit(PresenterEvent::Ended(EndReason::RemovedByHost));
                    break;
                }
                Some(ServerMsg::Error { code, message }) => {
                    emit(PresenterEvent::Error { code, message });
                    break;
                }
                // Candidates and answers of a finished presentation, and the like.
                Some(_) => {}
            },
        }
    }
    if let Some(mut r) = running.take() {
        r.pages.abort();
        r.core.request_stop();
        let _ = r.core.join().await;
    }
    // Dropping the signalling connection leaves the room.
    drop(sig);
}

fn error_event(code: &str, message: &str) -> PresenterEvent {
    PresenterEvent::Error {
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_connection_after_a_bad_ice_list_gets_the_ice_code() {
        let refused = IceState::of(&TransportError::Server {
            code: "rate_limited".into(),
            message: String::new(),
        });
        assert_eq!(refused, IceState::Refused);
        assert_eq!(refused.refine("peer_failed".into()), "ice_refused");
        let failed = IceState::of(&TransportError::Protocol(
            "unexpected HTTP status 500".into(),
        ));
        assert_eq!(failed, IceState::Failed);
        assert_eq!(failed.refine("peer_failed".into()), "ice_failed");
        // Other failures are not about the list; a good list changes nothing.
        assert_eq!(failed.refine("connection_lost".into()), "connection_lost");
        assert_eq!(IceState::Fine.refine("peer_failed".into()), "peer_failed");
        // A full server is an unexpected status for the list, not a refusal (as in the web client).
        let full = IceState::of(&TransportError::Server {
            code: "server_full".into(),
            message: String::new(),
        });
        assert_eq!(full, IceState::Failed);
    }
}
