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

impl TransportError {
    /// The code the apps map to an error page: the protocol code of a server error
    /// (`room_not_found`, `join_denied`, `altcha_required`, ...), `connection_lost` when the
    /// server could not be reached or the connection ended or timed out, `peer_failed` for
    /// WebRTC errors, `unknown` for the rest. The text is for logs only.
    pub fn code(&self) -> String {
        match self {
            Self::Server { code, .. } => code.clone(),
            Self::Connect(_) | Self::Protocol(_) | Self::Closed | Self::Timeout(_) => {
                "connection_lost".into()
            }
            Self::WebRtc(_) => "peer_failed".into(),
            Self::Unsupported(_) | Self::Invalid(_) => "unknown".into(),
        }
    }
}

impl From<webrtc::error::Error> for TransportError {
    fn from(e: webrtc::error::Error) -> Self {
        TransportError::WebRtc(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_follow_the_error_kind() {
        let server = TransportError::Server {
            code: "room_locked".into(),
            message: "x".into(),
        };
        assert_eq!(server.code(), "room_locked");
        for e in [
            TransportError::Connect("x".into()),
            TransportError::Protocol("x".into()),
            TransportError::Closed,
            TransportError::Timeout("x"),
        ] {
            assert_eq!(e.code(), "connection_lost", "{e}");
        }
        assert_eq!(TransportError::WebRtc("x".into()).code(), "peer_failed");
        assert_eq!(TransportError::Invalid("x".into()).code(), "unknown");
    }
}
