// SPDX-License-Identifier: GPL-3.0-or-later
//! Connection routes between sender and receiver.
//!
//! Idea ("route ladder"): try a direct WebRTC connection first; if that fails, go through
//! the internal server (TURN relay / SFU); if that is not possible either, use the
//! receiver's own Wi-Fi network.
//!
//! UNCERTAIN: how often each route is needed in real networks is not measured yet
//! (see docs/measurement-points.md). The order below is the plan's assumption.
//!
//! Today only [`Route::Direct`] is implemented as a route, but a [`Session`] can use the
//! STUN/TURN servers the signalling server hands out (`/ice-servers`, see [`ice`]).
//! This crate holds the signalling client ([`SignalingClient`]) and the WebRTC
//! [`Session`] wrapper that sender and receiver cores build on. See docs/transport.md.

pub mod altcha;
mod control;
mod error;
pub mod ice;
mod keyframe;
pub mod net;
mod rtp;
pub mod sas;
mod session;
mod signaling;

pub use control::{
    CONTROL_INBOX_CAPACITY, CONTROL_LABEL, ControlChannel, ControlData, HIGH_WATER, LOW_WATER,
    MAX_MESSAGE_BYTES, SEND_BUFFER_LIMIT,
};
pub use error::TransportError;
pub use ice::IceServerConfig;
pub use net::{ServerAddr, TlsTrust};
pub use rtp::{MediaSnapshot, MediaStats, VideoCodec, starts_key_frame};
pub use session::{
    ConnectionState, Session, SessionConfig, SessionEvent, VIDEO_TAP_CAPACITY, VideoPacket,
    VideoSender, VideoTap,
};
pub use signaling::{JoinProgress, SignalingClient, join_room, join_room_with, ws_url};

/// The ways a sender can reach a receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Direct WebRTC connection between the two devices.
    Direct,
    /// Via the internal server (TURN relay or SFU).
    ServerRelay,
    /// The sender joins the receiver's own Wi-Fi network.
    ReceiverWifi,
}

impl Route {
    /// All routes, most preferred first.
    pub fn preference_order() -> [Route; 3] {
        [Route::Direct, Route::ServerRelay, Route::ReceiverWifi]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_order_is_direct_then_server_then_wifi() {
        assert_eq!(
            Route::preference_order(),
            [Route::Direct, Route::ServerRelay, Route::ReceiverWifi]
        );
    }
}
