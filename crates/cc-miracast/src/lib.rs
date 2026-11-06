// SPDX-License-Identifier: GPL-3.0-or-later
//! Miracast receiving on Windows 11 through the documented Windows Runtime API
//! `Windows.Media.Miracast.MiracastReceiver` (milestone M5, assumption A4).
//!
//! * Platform independent: [`MiracastConfig`], [`MiracastStatus`], [`MiracastEvent`],
//!   [`ConnectionPolicy`]. These are unit-tested on Linux.
//! * Windows only (`#[cfg(windows)]`): the backend using the `windows` crate.
//!   On every other target [`MiracastReceiver::start`] returns [`MiracastError::Unsupported`].
//!
//! HONEST STATE: the Windows backend is compile-checked only. It has never been run, on no
//! Windows machine. Every statement about runtime behaviour is UNCERTAIN until the checklist
//! in `docs/miracast.md` has been done on real hardware.
//!
//! Not done here: showing the incoming video. The crate hands out an opaque [`StreamHandle`]
//! (the Windows `MediaSource`); the Flutter app would need a Windows video surface for it,
//! which does not exist yet.

pub mod config;
pub mod policy;
pub mod status;

#[cfg(windows)]
mod windows_backend;

pub use config::{ConfigError, MiracastConfig, PairingPolicy};
pub use policy::{
    CONFIRM_TIMEOUT, ConnectionPolicy, DEFAULT_MAX_SESSIONS, Decision, MAX_NAME_CHARS, RejectReason,
};
pub use status::{ConnectionId, MiracastEvent, MiracastStatus, status_after};

use std::sync::mpsc::Receiver;

/// Errors of the receiver.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MiracastError {
    /// Not Windows, or Windows reports that Miracast receiving is not supported.
    #[error("Miracast receiving is only available on Windows")]
    Unsupported,
    /// The settings are invalid.
    #[error("invalid settings: {0}")]
    Config(#[from] ConfigError),
    /// No connection with this id is waiting for an answer.
    #[error("unknown or already answered connection")]
    UnknownConnection,
    /// A Windows call failed.
    #[error("Windows call failed: {0}")]
    Windows(String),
}

/// Opaque handle to the incoming stream (on Windows a `Windows.Media.Core.MediaSource`).
///
/// Keep it alive as long as the stream is used. Nothing in this crate renders it.
#[derive(Clone)]
pub struct StreamHandle {
    #[cfg(windows)]
    #[allow(dead_code)] // held so the stream stays alive; rendering is not implemented
    pub(crate) source: windows::Media::Core::MediaSource,
}

impl std::fmt::Debug for StreamHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StreamHandle(..)")
    }
}

/// A running Miracast receiver. Dropping it stops listening and closes all connections.
pub struct MiracastReceiver {
    #[cfg(windows)]
    backend: windows_backend::Backend,
}

impl MiracastReceiver {
    /// Start listening. Events arrive on the returned channel (any thread may send them).
    ///
    /// On non-Windows targets this returns [`MiracastError::Unsupported`].
    #[cfg(windows)]
    pub fn start(config: MiracastConfig) -> Result<(Self, Receiver<MiracastEvent>), MiracastError> {
        config.validate()?;
        let (backend, rx) = windows_backend::Backend::start(&config)?;
        Ok((Self { backend }, rx))
    }

    /// Start listening (not available on this target).
    #[cfg(not(windows))]
    pub fn start(config: MiracastConfig) -> Result<(Self, Receiver<MiracastEvent>), MiracastError> {
        config.validate()?;
        Err(MiracastError::Unsupported)
    }

    /// Answer a [`MiracastEvent::ConnectionRequested`]: accept or reject.
    pub fn respond(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        #[cfg(windows)]
        {
            self.backend.respond(id, accept)
        }
        #[cfg(not(windows))]
        {
            let _ = (id, accept);
            Err(MiracastError::Unsupported)
        }
    }

    /// Current status.
    #[must_use]
    pub fn status(&self) -> MiracastStatus {
        #[cfg(windows)]
        {
            self.backend.status()
        }
        #[cfg(not(windows))]
        {
            MiracastStatus::Unsupported
        }
    }

    /// Stop listening and disconnect everything (also done on drop).
    #[allow(clippy::needless_pass_by_value)]
    pub fn stop(self) {
        // `self` is dropped here; on Windows its `Drop` does the cleanup.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn non_windows_is_unsupported() {
        let r = MiracastReceiver::start(MiracastConfig::default());
        assert!(matches!(r, Err(MiracastError::Unsupported)));
    }

    #[test]
    fn invalid_config_is_reported_first() {
        let c = MiracastConfig {
            display_name: String::new(),
            ..Default::default()
        };
        assert!(matches!(
            MiracastReceiver::start(c),
            Err(MiracastError::Config(ConfigError::EmptyName))
        ));
    }
}
