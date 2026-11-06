// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors of the Cast sender.

use std::time::Duration;

/// Why a Cast operation failed.
#[derive(Debug, thiserror::Error)]
pub enum CastError {
    /// Network or TLS I/O failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// The peer sent something that breaks the protocol.
    #[error("protocol error: {0}")]
    Protocol(String),
    /// Device authentication failed or could not be done (see `docs/cast.md`).
    #[error("device authentication failed: {0}")]
    DeviceAuth(String),
    /// The receiver answered `LAUNCH` with `LAUNCH_ERROR` (measurement point A5).
    #[error("the receiver refused to launch the mirroring app: {reason}")]
    LaunchError {
        /// The `reason` field of the message, or `UNKNOWN`.
        reason: String,
    },
    /// The receiver answered the OFFER with an error.
    #[error("the receiver rejected the OFFER: {0}")]
    OfferRejected(String),
    /// The receiver sent `INVALID_REQUEST`.
    #[error("the receiver reported INVALID_REQUEST: {0}")]
    InvalidRequest(String),
    /// Nothing came back in time.
    #[error("timeout after {0:?} while waiting for {1}")]
    Timeout(Duration, &'static str),
    /// The connection was closed by the receiver.
    #[error("the Cast connection was closed")]
    Closed,
    /// Capture or encoding failed.
    #[error("media error: {0}")]
    Media(String),
    /// Device discovery failed.
    #[error("discovery error: {0}")]
    Discovery(String),
}
