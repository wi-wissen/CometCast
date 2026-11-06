// SPDX-License-Identifier: GPL-3.0-or-later
//! Signalling client: talks to the Comet Cast server over a WebSocket.
//!
//! Same protocol as the web app (docs/protocol.md, version 1). The message types
//! come from `cc-proto`. A small background task owns the socket; this struct only
//! holds two channels, so it is easy to use from `select!` loops.

use std::time::Duration;

use cc_proto::{ClientMsg, PROTO_VERSION, Role, ServerMsg};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::error::TransportError;

/// UNCERTAIN: 10 s is a placeholder for connect plus hello; not measured on slow networks.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Turn the user's server text into the WebSocket URL.
///
/// Accepted: `host:port`, `host:port/ws`, `ws://host:port[/ws]` and `http://host:port`
/// (treated as `ws://`). An empty path becomes `/ws`; any other path is kept.
/// `wss://` and `https://` are rejected with a clear error: this build has no TLS yet
/// (docs/protocol.md; the tokio-tungstenite TLS feature is not enabled).
pub fn ws_url(server: &str) -> Result<String, TransportError> {
    let server = server.trim();
    let rest = if let Some(r) = server.strip_prefix("ws://") {
        r
    } else if let Some(r) = server.strip_prefix("http://") {
        r
    } else if server.starts_with("wss://") || server.starts_with("https://") {
        return Err(TransportError::Unsupported(
            "TLS (wss:// or https://) is not supported yet; use ws://host:port",
        ));
    } else if server.contains("://") {
        return Err(TransportError::Invalid(format!(
            "unknown URL scheme in {server:?}"
        )));
    } else {
        server
    };
    let (host, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    if host.is_empty() {
        return Err(TransportError::Invalid("empty server address".into()));
    }
    let path = if path.is_empty() || path == "/" {
        "/ws"
    } else {
        path
    };
    Ok(format!("ws://{host}{path}"))
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
    /// `server` is `host:port` or a full WebSocket URL. There is no TLS yet
    /// (docs/protocol.md), so plain `ws://` is what works today.
    pub async fn connect(server: &str, role: Role) -> Result<Self, TransportError> {
        let url = ws_url(server)?;
        let handshake = async {
            let (mut ws, _) = connect_async(&url)
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
    }

    #[test]
    fn ws_url_rejects_tls_and_junk() {
        assert!(matches!(
            ws_url("wss://x"),
            Err(TransportError::Unsupported(_))
        ));
        assert!(matches!(
            ws_url("https://x"),
            Err(TransportError::Unsupported(_))
        ));
        assert!(ws_url("ftp://x").is_err());
        assert!(ws_url("").is_err());
    }
}
