// SPDX-License-Identifier: GPL-3.0-or-later
//! The `control` data channel: ordered, reliable, carries JSON text messages and binary
//! chunks between sender and receiver (format: docs/web-m2.md, logic in `cc-files`).
//!
//! * The sender creates the channel before the offer ([`Session::create_control_channel`]),
//!   the receiver gets it as [`SessionEvent::ControlChannel`] when the peer's channel
//!   arrives. Channels with another label are ignored.
//! * Incoming messages go through a **bounded** queue ([`CONTROL_INBOX_CAPACITY`] messages).
//!   When the application reads slowly, the internal reader task waits, the library's
//!   receive buffer fills and SCTP flow control slows the peer down; nothing grows without
//!   limit.
//! * Outgoing data is paced: the peer connection is built with a send-buffer limit
//!   ([`SEND_BUFFER_LIMIT`]); [`ControlChannel::send_text`] and [`ControlChannel::send_binary`]
//!   wait while that much is queued. The channel also reports `bufferedamountlow` at
//!   [`LOW_WATER`] / high at [`HIGH_WATER`] (same values as the web client).
//!
//! UNCERTAIN: the thresholds are the web client's guesses (docs/web-m2.md), unmeasured on the
//! native side.
//!
//! [`Session::create_control_channel`]: crate::Session::create_control_channel
//! [`SessionEvent::ControlChannel`]: crate::SessionEvent::ControlChannel

use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use tokio::sync::{Mutex, mpsc, watch};
use webrtc::data_channel::{DataChannel, DataChannelEvent};

use crate::error::TransportError;

/// Label of the data channel (same as the web client).
pub const CONTROL_LABEL: &str = "control";

/// Largest message the library accepts (16 KiB), and the largest binary message we send.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Messages that may wait for the application. 64 x 16 KiB = 1 MiB at most.
/// UNCERTAIN: placeholder, unmeasured.
pub const CONTROL_INBOX_CAPACITY: usize = 64;

/// Sending waits while more than this is queued in the data channel (1 MiB, as the web client).
pub const SEND_BUFFER_LIMIT: usize = 1024 * 1024;
/// `bufferedamountlow` threshold (256 KiB, as the web client).
pub const LOW_WATER: u32 = 256 * 1024;
/// `bufferedamounthigh` threshold (1 MiB, as the web client).
pub const HIGH_WATER: u32 = 1024 * 1024;

/// One message from the peer.
#[derive(Clone, PartialEq, Eq)]
pub enum ControlData {
    /// A text message (JSON).
    Text(String),
    /// A binary message.
    Binary(Bytes),
}

impl std::fmt::Debug for ControlData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(t) => write!(f, "Text({} bytes)", t.len()),
            Self::Binary(b) => write!(f, "Binary({} bytes)", b.len()),
        }
    }
}

struct Inner {
    dc: Arc<dyn DataChannel>,
    inbox: Mutex<mpsc::Receiver<ControlData>>,
    open: watch::Receiver<bool>,
    /// Becomes `true` when the channel closed (also when the connection was lost).
    closed: watch::Receiver<bool>,
}

/// Handle on the `control` data channel. Cheap to clone; all clones share the channel.
#[derive(Clone)]
pub struct ControlChannel {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ControlChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlChannel")
            .field("open", &*self.inner.open.borrow())
            .field("closed", &*self.inner.closed.borrow())
            .finish()
    }
}

impl PartialEq for ControlChannel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}
impl Eq for ControlChannel {}

impl ControlChannel {
    /// Wrap a data channel and start the task that reads its events.
    pub(crate) async fn new(dc: Arc<dyn DataChannel>) -> Self {
        // Failures here (channel already gone) show up as a closed channel later.
        let _ = dc.set_buffered_amount_low_threshold(LOW_WATER).await;
        let _ = dc.set_buffered_amount_high_threshold(HIGH_WATER).await;
        let (tx, rx) = mpsc::channel(CONTROL_INBOX_CAPACITY);
        let (open_tx, open_rx) = watch::channel(false);
        let (closed_tx, closed_rx) = watch::channel(false);
        let reader = Arc::clone(&dc);
        tokio::spawn(async move {
            while let Some(event) = reader.poll().await {
                match event {
                    DataChannelEvent::OnOpen => {
                        let _ = open_tx.send(true);
                    }
                    DataChannelEvent::OnMessage(msg) => {
                        let data = if msg.is_string {
                            match String::from_utf8(msg.data.to_vec()) {
                                Ok(t) => ControlData::Text(t),
                                // Marked as text but not valid UTF-8: drop it.
                                Err(_) => continue,
                            }
                        } else {
                            ControlData::Binary(msg.data.freeze())
                        };
                        // Waits when the application is slow (back-pressure). Stops when
                        // the application dropped the channel.
                        if tx.send(data).await.is_err() {
                            break;
                        }
                    }
                    DataChannelEvent::OnClose => break,
                    _ => {}
                }
            }
            let _ = closed_tx.send(true);
        });
        Self {
            inner: Arc::new(Inner {
                dc,
                inbox: Mutex::new(rx),
                open: open_rx,
                closed: closed_rx,
            }),
        }
    }

    /// Wait until the channel is open (or fail after `timeout` / when it closed).
    pub async fn wait_open(&self, timeout: Duration) -> Result<(), TransportError> {
        let mut open = self.inner.open.clone();
        let mut closed = self.inner.closed.clone();
        let wait = async {
            loop {
                if *open.borrow_and_update() {
                    return Ok(());
                }
                if *closed.borrow_and_update() {
                    return Err(TransportError::Closed);
                }
                tokio::select! {
                    r = open.changed() => if r.is_err() { return Err(TransportError::Closed) },
                    r = closed.changed() => if r.is_err() { return Err(TransportError::Closed) },
                }
            }
        };
        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| TransportError::Timeout("waiting for the control channel to open"))?
    }

    /// `true` once the channel is closed.
    pub fn is_closed(&self) -> bool {
        *self.inner.closed.borrow()
    }

    /// Send a text message. Waits while the send buffer is full. Messages above 16 KiB are
    /// refused here; the format allows 8192 characters (docs/web-m2.md).
    pub async fn send_text(&self, text: &str) -> Result<(), TransportError> {
        if text.len() > MAX_MESSAGE_BYTES {
            return Err(TransportError::Invalid("control message too large".into()));
        }
        self.inner.dc.send_text(text).await?;
        Ok(())
    }

    /// Send a binary message of at most [`MAX_MESSAGE_BYTES`]. Waits while the send buffer is full.
    pub async fn send_binary(&self, data: &[u8]) -> Result<(), TransportError> {
        if data.len() > MAX_MESSAGE_BYTES {
            return Err(TransportError::Invalid("control message too large".into()));
        }
        self.inner.dc.send(BytesMut::from(data)).await?;
        Ok(())
    }

    /// Next message from the peer, or `None` after the channel closed and the queue is empty.
    pub async fn recv(&self) -> Option<ControlData> {
        self.inner.inbox.lock().await.recv().await
    }

    /// Wait until everything queued for sending was released by SCTP (acknowledged), at most
    /// `timeout`. Returns `false` on timeout or when the channel closed first. Call this
    /// before closing the session, otherwise the tail of a transfer can be lost.
    pub async fn flush(&self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match self.inner.dc.outstanding_bytes().await {
                Ok(0) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
            if self.is_closed() || tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Close the channel.
    pub async fn close(&self) {
        let _ = self.inner.dc.close().await;
    }

    /// Close the channel and wait (at most `timeout`) until the close was completed with the
    /// peer. The close is ordered after the data already sent, so a browser delivers every
    /// message to its application before it sees the channel close; closing the whole
    /// connection at once can cut off the end of a transfer.
    pub async fn close_graceful(&self, timeout: Duration) {
        let _ = self.inner.dc.close().await;
        let mut closed = self.inner.closed.clone();
        let _ = tokio::time::timeout(timeout, async {
            while !*closed.borrow_and_update() {
                if closed.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
    }
}
