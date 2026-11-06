// SPDX-License-Identifier: GPL-3.0-or-later
//! The Cast channel after the TLS handshake: framed reading and writing in two tasks, and the
//! heartbeat (`urn:x-cast:com.google.cast.tp.heartbeat`, `PING` / `PONG`).
//!
//! The namespace and sender/receiver ids (`sender-0`, `receiver-0`) are from
//! `cast/common/channel/message_util.h` of Open Screen (commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef`, BSD-3-Clause, Copyright 2019 The Chromium
//! Authors).
//!
//! UNCERTAIN: the heartbeat is NOT implemented in the Open Screen sender code that was read
//! (a search of `cast/common/channel` and `cast/standalone_sender` found no PING handling).
//! The 5 second PING interval, the `{"type":"PING"}` / `{"type":"PONG"}` bodies and the
//! 15 second silence timeout come from the project plan and from public descriptions of the
//! Cast v2 protocol, and were never tried against a device.

use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::error::CastError;
use crate::framing;
use crate::proto::CastMessage;

/// Heartbeat namespace.
pub const HEARTBEAT_NS: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
/// Connection namespace (`CONNECT` / `CLOSE`).
pub const CONNECTION_NS: &str = "urn:x-cast:com.google.cast.tp.connection";
/// Receiver namespace (`GET_STATUS`, `LAUNCH`, `RECEIVER_STATUS`, ...).
pub const RECEIVER_NS: &str = "urn:x-cast:com.google.cast.receiver";
/// Streaming negotiation namespace (`OFFER` / `ANSWER`).
pub const WEBRTC_NS: &str = "urn:x-cast:com.google.cast.webrtc";
/// Id used by the sender platform.
pub const PLATFORM_SENDER: &str = "sender-0";
/// Id of the receiver platform.
pub const PLATFORM_RECEIVER: &str = "receiver-0";

/// Heartbeat timing.
#[derive(Debug, Clone, Copy)]
pub struct Heartbeat {
    /// How often a `PING` is sent (plan: every 5 s).
    pub interval: Duration,
    /// Silence after which the connection counts as dead.
    pub timeout: Duration,
}

impl Default for Heartbeat {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            timeout: Duration::from_secs(15),
        }
    }
}

/// Build a JSON message.
pub fn json_message(source: &str, destination: &str, namespace: &str, body: &Value) -> CastMessage {
    CastMessage::text(source, destination, namespace, body.to_string())
}

/// A running Cast channel.
pub struct Connection {
    tx: mpsc::Sender<CastMessage>,
    rx: mpsc::Receiver<Result<CastMessage, CastError>>,
    tasks: Vec<JoinHandle<()>>,
}

impl Connection {
    /// Start the reader, writer and heartbeat tasks on an established stream.
    pub fn start<S>(stream: S, heartbeat: Heartbeat) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (mut rd, mut wr) = tokio::io::split(stream);
        let (tx, mut out_rx) = mpsc::channel::<CastMessage>(64);
        let (in_tx, rx) = mpsc::channel::<Result<CastMessage, CastError>>(64);

        let writer = tokio::spawn(async move {
            while let Some(m) = out_rx.recv().await {
                if framing::write_message(&mut wr, &m).await.is_err() {
                    break;
                }
            }
            let _ = wr.shutdown().await;
        });

        let pong_tx = tx.clone();
        let reader = tokio::spawn(async move {
            loop {
                match tokio::time::timeout(heartbeat.timeout, framing::read_message(&mut rd)).await
                {
                    Err(_) => {
                        let _ = in_tx
                            .send(Err(CastError::Timeout(
                                heartbeat.timeout,
                                "any message (heartbeat)",
                            )))
                            .await;
                        break;
                    }
                    Ok(Err(e)) => {
                        let e = match e {
                            CastError::Io(io) if io.kind() == std::io::ErrorKind::UnexpectedEof => {
                                CastError::Closed
                            }
                            other => other,
                        };
                        let _ = in_tx.send(Err(e)).await;
                        break;
                    }
                    Ok(Ok(m)) if m.namespace == HEARTBEAT_NS => {
                        let ty = m
                            .text_payload()
                            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                            .and_then(|v| v.get("type").and_then(Value::as_str).map(str::to_owned));
                        if ty.as_deref() == Some("PING") {
                            let pong = json_message(
                                PLATFORM_SENDER,
                                PLATFORM_RECEIVER,
                                HEARTBEAT_NS,
                                &serde_json::json!({"type": "PONG"}),
                            );
                            let _ = pong_tx.send(pong).await;
                        }
                    }
                    Ok(Ok(m)) => {
                        if in_tx.send(Ok(m)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let ping_tx = tx.clone();
        let pinger = tokio::spawn(async move {
            // The first PING is one interval away: the caller queues the virtual-connection
            // CONNECT right after `start`, and a PING must not overtake it.
            let mut tick = tokio::time::interval_at(
                tokio::time::Instant::now() + heartbeat.interval,
                heartbeat.interval,
            );
            loop {
                tick.tick().await;
                let ping = json_message(
                    PLATFORM_SENDER,
                    PLATFORM_RECEIVER,
                    HEARTBEAT_NS,
                    &serde_json::json!({"type": "PING"}),
                );
                if ping_tx.send(ping).await.is_err() {
                    break;
                }
            }
        });

        Self {
            tx,
            rx,
            tasks: vec![writer, reader, pinger],
        }
    }

    /// Queue a message for sending.
    pub async fn send(&self, message: CastMessage) -> Result<(), CastError> {
        self.tx.send(message).await.map_err(|_| CastError::Closed)
    }

    /// Next non-heartbeat message.
    pub async fn recv(&mut self) -> Result<CastMessage, CastError> {
        self.rx.recv().await.unwrap_or(Err(CastError::Closed))
    }

    /// Wait up to `timeout` for the first message for which `pick` returns `Some`. Other
    /// messages are dropped (UNCERTAIN: a real receiver may send status messages that matter
    /// later; this sender only needs the replies it waits for).
    pub async fn recv_matching<T>(
        &mut self,
        timeout: Duration,
        what: &'static str,
        mut pick: impl FnMut(&CastMessage) -> Option<T>,
    ) -> Result<T, CastError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let next = tokio::time::timeout_at(deadline, self.recv())
                .await
                .map_err(|_| CastError::Timeout(timeout, what))??;
            if let Some(v) = pick(&next) {
                return Ok(v);
            }
        }
    }

    /// A handle that can send messages from other tasks.
    pub fn sender(&self) -> mpsc::Sender<CastMessage> {
        self.tx.clone()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn answers_ping_with_pong_and_hides_heartbeat() {
        let (a, mut b) = tokio::io::duplex(4096);
        let mut conn = Connection::start(
            a,
            Heartbeat {
                interval: Duration::from_secs(3600),
                timeout: Duration::from_secs(5),
            },
        );
        // Peer sends a PING and then a normal message.
        let ping = json_message(
            PLATFORM_RECEIVER,
            PLATFORM_SENDER,
            HEARTBEAT_NS,
            &serde_json::json!({"type":"PING"}),
        );
        framing::write_message(&mut b, &ping).await.unwrap();
        let hello = json_message(
            "receiver-0",
            "sender-0",
            RECEIVER_NS,
            &serde_json::json!({"type":"X"}),
        );
        framing::write_message(&mut b, &hello).await.unwrap();
        let got = conn.recv().await.unwrap();
        assert_eq!(got.namespace, RECEIVER_NS);
        let m = framing::read_message(&mut b).await.unwrap();
        assert!(m.text_payload().unwrap().contains("PONG"));
    }

    #[tokio::test]
    async fn the_first_ping_never_overtakes_a_message_queued_right_after_start() {
        let (a, mut b) = tokio::io::duplex(4096);
        let conn = Connection::start(
            a,
            Heartbeat {
                interval: Duration::from_millis(150),
                timeout: Duration::from_secs(5),
            },
        );
        let connect = json_message(
            PLATFORM_SENDER,
            PLATFORM_RECEIVER,
            CONNECTION_NS,
            &serde_json::json!({"type":"CONNECT"}),
        );
        conn.send(connect).await.unwrap();
        let first = framing::read_message(&mut b).await.unwrap();
        assert_eq!(first.namespace, CONNECTION_NS);
        let second = framing::read_message(&mut b).await.unwrap();
        assert_eq!(second.namespace, HEARTBEAT_NS);
        assert!(second.text_payload().unwrap().contains("PING"));
    }

    #[tokio::test]
    async fn silence_times_out() {
        let (a, _b) = tokio::io::duplex(4096);
        let mut conn = Connection::start(
            a,
            Heartbeat {
                interval: Duration::from_secs(3600),
                timeout: Duration::from_millis(100),
            },
        );
        assert!(matches!(conn.recv().await, Err(CastError::Timeout(..))));
    }
}
