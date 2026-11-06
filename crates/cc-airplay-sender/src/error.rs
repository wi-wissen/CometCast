// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the AirPlay sender.

/// Everything that can go wrong. The text never contains secrets (PIN, keys).
#[derive(Debug, thiserror::Error)]
pub enum SenderError {
    /// Network input/output failed.
    #[error("network error: {0}")]
    Io(String),
    /// A step did not finish in time.
    #[error("timeout: {0}")]
    Timeout(&'static str),
    /// The receiver sent something that does not follow the protocol (or a hostile answer).
    #[error("protocol error: {0}")]
    Protocol(String),
    /// The receiver answered with an HTTP error status.
    #[error("the receiver answered {status} to {what}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// The request, for example `POST /play`.
        what: String,
    },
    /// Pairing or authentication failed (wrong PIN, bad proof, ...).
    #[error("authentication failed: {0}")]
    Auth(String),
    /// A size limit was hit (answers from the receiver are untrusted).
    #[error("too large: {0}")]
    TooLarge(&'static str),
    /// Not possible with this receiver or not implemented. The text says which.
    #[error("not supported: {0}")]
    Unsupported(String),
    /// Looking for receivers failed.
    #[error("discovery: {0}")]
    Discovery(String),
    /// Reading or writing the pairing store failed.
    #[error("pairing store: {0}")]
    Store(String),
    /// Bad argument from the caller.
    #[error("invalid input: {0}")]
    Input(String),
}

impl From<std::io::Error> for SenderError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                SenderError::Timeout("network operation")
            }
            _ => SenderError::Io(e.to_string()),
        }
    }
}

/// Result alias of this crate.
pub type Result<T> = std::result::Result<T, SenderError>;
