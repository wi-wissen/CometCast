// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type shared by the signalling client and the WebRTC session.

/// Everything that can go wrong in this crate.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Could not open or keep the WebSocket to the signalling server.
    #[error("signalling connection failed: {0}")]
    Connect(String),
    /// The server did not behave as docs/protocol.md says (wrong or missing message).
    #[error("unexpected signalling message: {0}")]
    Protocol(String),
    /// The server sent an `error` message.
    #[error("server error {code}: {message}")]
    Server {
        /// Machine-readable code, e.g. `room_not_found`.
        code: String,
        /// Human-readable text from the server.
        message: String,
    },
    /// The signalling connection ended.
    #[error("signalling connection closed")]
    Closed,
    /// Something timed out.
    #[error("timeout: {0}")]
    Timeout(&'static str),
    /// The WebRTC library reported an error.
    #[error("webrtc error: {0}")]
    WebRtc(String),
    /// A request that this version does not implement (e.g. a relay route).
    #[error("not supported yet: {0}")]
    Unsupported(&'static str),
    /// Bad input (SDP, candidate JSON, address).
    #[error("invalid input: {0}")]
    Invalid(String),
}

impl From<webrtc::error::Error> for TransportError {
    fn from(e: webrtc::error::Error) -> Self {
        TransportError::WebRtc(e.to_string())
    }
}
