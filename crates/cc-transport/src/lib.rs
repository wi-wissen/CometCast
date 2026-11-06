// SPDX-License-Identifier: GPL-3.0-or-later
//! Connection routes between sender and receiver.
//!
//! Idea ("route ladder"): try a direct WebRTC connection first; if that fails, go through
//! the internal server (TURN relay / SFU); if that is not possible either, use the
//! receiver's own Wi-Fi network.
//!
//! UNCERTAIN: how often each route is needed in real networks is not measured yet
//! (see docs/measurement-points.md). The order below is the plan's assumption.

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
