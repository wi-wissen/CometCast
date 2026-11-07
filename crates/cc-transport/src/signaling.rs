// SPDX-License-Identifier: GPL-3.0-or-later
//! Signalling client: talks to the Comet Cast server over a WebSocket.
//!
//! Same protocol as the web app (docs/protocol.md, version 2). The message types
//! come from `cc-proto`. A small background task owns the socket; this struct only
//! holds two channels, so it is easy to use from `select!` loops.

use std::time::Duration;

use cc_proto::{ClientMsg, PROTO_VERSION, Role, RoomCode, ServerMsg};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Message;

use crate::error::TransportError;
use crate::net::{ServerAddr, TlsTrust, connect_io};

/// UNCERTAIN: 10 s is a placeholder for connect plus hello; not measured on slow networks.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Turn the user's server text into the WebSocket URL.
///
/// Accepted: `host:port`, `host:port/ws`, `ws://host:port[/ws]`, `wss://host[:port][/ws]`,
/// and `http://` / `https://` (read as `ws://` / `wss://`). An empty path becomes `/ws`; any
/// other path is kept. A missing port means 80 (`ws`) or 443 (`wss`).
pub fn ws_url(server: &str) -> Result<String, TransportError> {
    Ok(ServerAddr::parse(server)?.ws_url())
}

/// A connected, handshaken signalling connection.
pub struct SignalingClient {
    tx: mpsc::Sender<ClientMsg>,
    rx: mpsc::Receiver<ServerMsg>,
    // Kept only so the task handle lives here; the task ends by itself (see `connect`).
    _task: JoinHandle<()>,
}

impl SignalingClient {
    /// Connect, send `hello` with the given role and wait for the server's `hello`.
    ///
    /// `server` is `host:port` or a full WebSocket URL (`wss://` uses TLS with the system
    /// trust store). Use [`SignalingClient::connect_with`] to trust more certificates.
    pub async fn connect(server: &str, role: Role) -> Result<Self, TransportError> {
        Self::connect_with(server, role, &TlsTrust::system()).await
    }

    /// Like [`SignalingClient::connect`] with explicit TLS trust (for example a self-signed
    /// development server).
    pub async fn connect_with(
        server: &str,
        role: Role,
        trust: &TlsTrust,
    ) -> Result<Self, TransportError> {
        let addr = ServerAddr::parse(server)?;
        let url = addr.ws_url();
        let handshake = async {
            let io = connect_io(&addr, trust).await?;
            // The scheme in `url` is only used for the request line; TLS is already done.
            let (mut ws, _) = client_async(url.as_str(), io)
                .await
                .map_err(|e| TransportError::Connect(e.to_string()))?;
            let hello = ClientMsg::Hello {
                proto_version: PROTO_VERSION,
                role,
            };
            send_json(&mut ws, &hello).await?;
            // The first answer must be `hello` (or an `error`).
            loop {
                match ws.next().await {
                    Some(Ok(Message::Text(t))) => {
                        return match serde_json::from_str::<ServerMsg>(t.as_str()) {
                            Ok(ServerMsg::Hello { proto_version })
                                if proto_version == PROTO_VERSION =>
                            {
                                Ok(ws)
                            }
                            Ok(ServerMsg::Error { code, message }) => {
                                Err(TransportError::Server { code, message })
                            }
                            other => Err(TransportError::Protocol(format!(
                                "expected hello, got {other:?}"
                            ))),
                        };
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(_)) | None => return Err(TransportError::Closed),
                    Some(Err(e)) => return Err(TransportError::Connect(e.to_string())),
                }
            }
        };
        let mut ws = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake)
            .await
            .map_err(|_| TransportError::Timeout("signalling handshake"))??;

        // UNCERTAIN: channel sizes are placeholders; signalling traffic is a few messages.
        let (out_tx, mut out_rx) = mpsc::channel::<ClientMsg>(64);
        let (in_tx, in_rx) = mpsc::channel::<ServerMsg>(64);
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    out = out_rx.recv() => match out {
                        Some(msg) => {
                            if send_json(&mut ws, &msg).await.is_err() {
                                break;
                            }
                        }
                        // The client was dropped.
                        None => break,
                    },
                    inc = ws.next() => match inc {
                        Some(Ok(Message::Text(t))) => {
                            // UNCERTAIN: messages this version cannot parse are skipped, so a
                            // newer server cannot break an older client.
                            if let Ok(msg) = serde_json::from_str::<ServerMsg>(t.as_str())
                                && in_tx.send(msg).await.is_err()
                            {
                                break;
                            }
                        }
                        Some(Ok(Message::Ping(p))) => {
                            let _ = ws.send(Message::Pong(p)).await;
                        }
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                        Some(Ok(_)) => {}
                    },
                }
            }
            // Best effort: tell the server we are leaving. Then `in_tx` is dropped, which
            // makes `recv()` return None.
            let _ = ws.close(None).await;
        });
        Ok(Self {
            tx: out_tx,
            rx: in_rx,
            _task: task,
        })
    }

    /// Queue a message for the server.
    pub async fn send(&self, msg: ClientMsg) -> Result<(), TransportError> {
        self.tx.send(msg).await.map_err(|_| TransportError::Closed)
    }

    /// Next message from the server, or `None` when the connection has ended.
    pub async fn recv(&mut self) -> Option<ServerMsg> {
        self.rx.recv().await
    }
}

/// What a sender learns while its join waits for the display device's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinProgress {
    /// The room needs approval: the host has to allow the join (server gives up after
    /// `timeout_secs`).
    Pending {
        /// Seconds until the server answers `approval_timeout`.
        timeout_secs: u64,
        /// Two-digit check number; the display shows the same number on the request.
        check: u8,
    },
}

/// UNCERTAIN: 10 s is a placeholder for the server's first answer to `join_room`.
const JOIN_ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
/// Extra time on top of the server's approval timeout before the client gives up itself.
const APPROVAL_SLACK: Duration = Duration::from_secs(15);

/// Join a room as a sender and wait until the server says `joined`.
///
/// In a room that needs approval the server first answers `join_pending`; this is reported
/// on `progress` and the wait continues until the host decides (`joined` or an error such as
/// `join_denied`, `room_locked`, `approval_timeout`). Dropping the returned future (or the
/// whole [`SignalingClient`]) cancels the request: the server then tells the host.
pub async fn join_room(
    sig: &mut SignalingClient,
    code: RoomCode,
    name: Option<String>,
    progress: Option<&mpsc::UnboundedSender<JoinProgress>>,
) -> Result<(), TransportError> {
    sig.send(ClientMsg::JoinRoom { code, name }).await?;
    let mut deadline = tokio::time::Instant::now() + JOIN_ANSWER_TIMEOUT;
    loop {
        let msg = tokio::time::timeout_at(deadline, sig.recv())
            .await
            .map_err(|_| TransportError::Timeout("waiting for joined"))?;
        match msg {
            Some(ServerMsg::Joined) => return Ok(()),
            Some(ServerMsg::JoinPending {
                timeout_secs,
                check,
            }) => {
                if let Some(p) = progress {
                    let _ = p.send(JoinProgress::Pending {
                        timeout_secs,
                        check,
                    });
                }
                deadline = tokio::time::Instant::now()
                    + Duration::from_secs(timeout_secs)
                    + APPROVAL_SLACK;
            }
            Some(ServerMsg::Error { code, message }) => {
                return Err(TransportError::Server { code, message });
            }
            Some(other) => {
                return Err(TransportError::Protocol(format!(
                    "expected joined, got {other:?}"
                )));
            }
            None => return Err(TransportError::Closed),
        }
    }
}

async fn send_json<S>(ws: &mut S, msg: &ClientMsg) -> Result<(), TransportError>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let text = serde_json::to_string(msg).map_err(|e| TransportError::Protocol(e.to_string()))?;
    ws.send(Message::text(text))
        .await
        .map_err(|e| TransportError::Connect(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_forms() {
        let ok = |i: &str| ws_url(i).unwrap();
        assert_eq!(ok("127.0.0.1:8443"), "ws://127.0.0.1:8443/ws");
        assert_eq!(ok("example.org:1/"), "ws://example.org:1/ws");
        assert_eq!(ok("example.org:1/ws"), "ws://example.org:1/ws");
        assert_eq!(ok("ws://x:1"), "ws://x:1/ws");
        assert_eq!(ok("ws://x:1/ws"), "ws://x:1/ws");
        assert_eq!(ok("http://x:1"), "ws://x:1/ws");
        assert_eq!(ok("ws://x:1/other"), "ws://x:1/other");
        assert_eq!(ok("wss://x:9"), "wss://x:9/ws");
        assert_eq!(ok("https://x"), "wss://x:443/ws");
    }

    #[test]
    fn ws_url_rejects_junk() {
        assert!(ws_url("ftp://x").is_err());
        assert!(ws_url("").is_err());
    }
}
