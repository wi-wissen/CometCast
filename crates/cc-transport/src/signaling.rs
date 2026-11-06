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

/// Turn `host:port` (or a full `ws://` / `wss://` URL) into the WebSocket URL of the server.
pub fn ws_url(server: &str) -> String {
    if server.starts_with("ws://") || server.starts_with("wss://") {
        server.to_string()
    } else {
        format!("ws://{}/ws", server.trim_end_matches('/'))
    }
}

/// A connected, handshaken signalling connection.
pub struct SignalingClient {
    tx: mpsc::Sender<ClientMsg>,
    rx: mpsc::Receiver<ServerMsg>,
    task: JoinHandle<()>,
}

impl SignalingClient {
    /// Connect, send `hello` with the given role and wait for the server's `hello`.
    ///
    /// `server` is `host:port` or a full WebSocket URL. There is no TLS yet
    /// (docs/protocol.md), so plain `ws://` is what works today.
    pub async fn connect(server: &str, role: Role) -> Result<Self, TransportError> {
        let url = ws_url(server);
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
                        // The client was dropped: close politely.
                        None => {
                            let _ = ws.close(None).await;
                            break;
                        }
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
            // Dropping `in_tx` makes `recv()` return None.
        });
        Ok(Self {
            tx: out_tx,
            rx: in_rx,
            task,
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

impl Drop for SignalingClient {
    fn drop(&mut self) {
        // The task would also stop once `tx` is dropped; abort makes it immediate.
        self.task.abort();
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
    fn ws_url_from_host_port_and_full_url() {
        assert_eq!(ws_url("127.0.0.1:8443"), "ws://127.0.0.1:8443/ws");
        assert_eq!(ws_url("example.org:1/"), "ws://example.org:1/ws");
        assert_eq!(ws_url("ws://x:1/ws"), "ws://x:1/ws");
    }
}
