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
//! Video: the Windows backend plays the stream with a `MediaPlayer` in frame-server mode and
//! publishes BGRA pictures into a one-slot mailbox ([`frame`]); the UI pulls them with
//! [`MiracastReceiver::take_frame`] (through the C interface of `cc-miracast-ffi` in the
//! Flutter app). The mailbox, the scaler, the connection engine and the diagnosis are platform
//! independent and unit-tested on Linux; the Windows calls are compile-checked only.

pub mod config;
mod deadline;
pub mod diagnosis;
mod engine;
pub mod frame;
pub mod policy;
pub mod status;

#[cfg(windows)]
mod player;
#[cfg(windows)]
mod windows_backend;

pub use config::{ConfigError, MiracastConfig, PairingPolicy};
pub use diagnosis::{
    AdapterSupport, FeatureState, Issue, Probe, WIRELESS_DISPLAY_CAPABILITY, diagnose_start_error,
    diagnose_status,
};
pub use frame::{FrameCounters, FrameMailbox, VideoFrame};
pub use policy::{
    CONFIRM_TIMEOUT, ConnectionPolicy, DEFAULT_MAX_SESSIONS, Decision, MAX_DEVICE_NAME_CHARS,
    MAX_NAME_CHARS, REJECT_REPORT_INTERVAL, RejectLimiter, RejectReason, sanitize_device_name,
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
/// The receiver itself plays the stream (see [`frame`]); the handle only proves that a stream
/// exists and keeps the source alive while it is in an event.
#[derive(Clone)]
pub struct StreamHandle {
    #[cfg(windows)]
    #[allow(dead_code)] // held so the source stays alive
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

    /// End the active connection (the "Disconnect" button) but keep listening.
    pub fn disconnect(&self) {
        #[cfg(windows)]
        self.backend.disconnect();
    }

    /// Take the newest picture of the stream (BGRA), if a new one is there.
    #[must_use]
    pub fn take_frame(&self) -> Option<VideoFrame> {
        #[cfg(windows)]
        {
            self.backend.take_frame()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Size of the pending picture, without taking it.
    #[must_use]
    pub fn peek_frame_size(&self) -> Option<(u32, u32)> {
        #[cfg(windows)]
        {
            self.backend.peek_frame_size()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    /// Counters of the picture path.
    #[must_use]
    pub fn frame_counters(&self) -> FrameCounters {
        #[cfg(windows)]
        {
            self.backend.frame_counters()
        }
        #[cfg(not(windows))]
        {
            FrameCounters::default()
        }
    }

    /// Largest picture size handed out; larger pictures are scaled down. Default 1920 x 1080.
    pub fn set_max_frame_size(&self, width: u32, height: u32) {
        #[cfg(windows)]
        self.backend.set_max_frame_size(width, height);
        #[cfg(not(windows))]
        let _ = (width, height);
    }

    /// Mute or unmute the audio of the stream.
    pub fn set_muted(&self, muted: bool) {
        #[cfg(windows)]
        self.backend.set_muted(muted);
        #[cfg(not(windows))]
        let _ = muted;
    }

    /// Stop listening and disconnect everything (also done on drop).
    #[allow(clippy::needless_pass_by_value)]
    pub fn stop(self) {
        // `self` is dropped here; on Windows its `Drop` does the cleanup.
    }
}

/// Look at the system without starting anything: is the optional feature there, does Windows
/// report support for the Wi-Fi adapter. On other systems everything is "unknown".
#[must_use]
pub fn probe() -> Probe {
    #[cfg(windows)]
    {
        windows_backend::probe()
    }
    #[cfg(not(windows))]
    {
        Probe::default()
    }
}

/// What the UI and the C interface need from a receiver. Implemented by [`MiracastReceiver`]
/// and by fakes in tests, so the layers above can be tested without Windows.
pub trait ReceiverBackend: Send {
    /// Current status.
    fn status(&self) -> MiracastStatus;
    /// Answer a connection request.
    fn respond(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError>;
    /// End the active connection, keep listening.
    fn disconnect(&self);
    /// Take the newest picture.
    fn take_frame(&self) -> Option<VideoFrame>;
    /// Size of the pending picture.
    fn peek_frame_size(&self) -> Option<(u32, u32)>;
    /// Counters of the picture path.
    fn frame_counters(&self) -> FrameCounters;
    /// Largest picture size handed out.
    fn set_max_frame_size(&self, width: u32, height: u32);
    /// Mute or unmute.
    fn set_muted(&self, muted: bool);
}

impl ReceiverBackend for MiracastReceiver {
    fn status(&self) -> MiracastStatus {
        MiracastReceiver::status(self)
    }
    fn respond(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        MiracastReceiver::respond(self, id, accept)
    }
    fn disconnect(&self) {
        MiracastReceiver::disconnect(self);
    }
    fn take_frame(&self) -> Option<VideoFrame> {
        MiracastReceiver::take_frame(self)
    }
    fn peek_frame_size(&self) -> Option<(u32, u32)> {
        MiracastReceiver::peek_frame_size(self)
    }
    fn frame_counters(&self) -> FrameCounters {
        MiracastReceiver::frame_counters(self)
    }
    fn set_max_frame_size(&self, width: u32, height: u32) {
        MiracastReceiver::set_max_frame_size(self, width, height);
    }
    fn set_muted(&self, muted: bool) {
        MiracastReceiver::set_muted(self, muted);
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
