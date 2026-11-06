// SPDX-License-Identifier: GPL-3.0-or-later
//! Status and events for the UI.

use crate::{StreamHandle, policy::RejectReason};

/// Status of the Miracast receiver, for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MiracastStatus {
    /// Not started.
    #[default]
    Off,
    /// Being set up.
    Starting,
    /// Visible to senders, waiting.
    Listening,
    /// A sender asked to connect; waiting for the person's decision.
    ConnectionPending,
    /// A sender is connected.
    Connected,
    /// Windows reports that receiving is disabled (policy) or temporarily unavailable.
    Unavailable,
    /// This system cannot receive Miracast (no support, or not Windows).
    Unsupported,
    /// Setup or operation failed; see the error event.
    Failed,
}

/// Identifies one connection inside a running receiver. Not a Windows value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub u64);

/// Things the receiver reports. Delivered through a channel; may arrive on any thread.
#[derive(Debug, Clone)]
pub enum MiracastEvent {
    /// The receiver is visible to senders.
    Listening,
    /// The overall status changed.
    StatusChanged(MiracastStatus),
    /// A foreign device wants to connect. Answer with `MiracastReceiver::respond`.
    ConnectionRequested {
        /// Handle for the answer.
        id: ConnectionId,
        /// Name the sender announced.
        device_name: String,
        /// PIN that must be shown to the person, if Windows gave one.
        // UNCERTAIN: whether Windows or our app shows it, and when it is non-empty.
        pin: Option<String>,
    },
    /// A connection was accepted.
    Connected {
        /// Connection handle.
        id: ConnectionId,
        /// Sender name.
        device_name: String,
    },
    /// The incoming stream is available. Rendering it is NOT done in this crate.
    StreamReady {
        /// Connection handle.
        id: ConnectionId,
        /// Opaque handle to the Windows `MediaSource`.
        stream: StreamHandle,
    },
    /// A connection ended (sender left, rejected, timeout, or we closed it).
    Disconnected {
        /// Connection handle.
        id: ConnectionId,
    },
    /// A foreign connection was turned away without asking the person. Informational: it
    /// never changes the status (a nearby second sender must not disturb an active receiver).
    ConnectionRejected {
        /// Why it was rejected.
        reason: RejectReason,
        /// The cleaned sender name, if it had a usable one.
        device_name: Option<String>,
    },
    /// One connection failed (for example the confirmation could not be completed). The
    /// receiver itself keeps running, so this does not change the status.
    ConnectionFailed {
        /// The connection, if it had been registered.
        id: Option<ConnectionId>,
        /// What failed.
        message: String,
    },
    /// The receiver itself failed; the status becomes [`MiracastStatus::Failed`].
    Error(String),
}

/// The status that follows an event (pure function, used by the UI and tests).
#[must_use]
pub fn status_after(current: MiracastStatus, event: &MiracastEvent) -> MiracastStatus {
    use MiracastStatus as S;
    match event {
        MiracastEvent::Listening => S::Listening,
        MiracastEvent::StatusChanged(s) => *s,
        MiracastEvent::ConnectionRequested { .. } => S::ConnectionPending,
        MiracastEvent::Connected { .. } | MiracastEvent::StreamReady { .. } => S::Connected,
        // Only a live connection falls back to listening. Off, Starting, Unavailable,
        // Unsupported and Failed are not revived by a late disconnect.
        MiracastEvent::Disconnected { .. } => match current {
            S::Connected | S::ConnectionPending => S::Listening,
            other => other,
        },
        MiracastEvent::ConnectionRejected { .. } | MiracastEvent::ConnectionFailed { .. } => {
            current
        }
        MiracastEvent::Error(_) => S::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_follows_events() {
        let id = ConnectionId(1);
        let mut s = MiracastStatus::Starting;
        s = status_after(s, &MiracastEvent::Listening);
        assert_eq!(s, MiracastStatus::Listening);
        s = status_after(
            s,
            &MiracastEvent::ConnectionRequested {
                id,
                device_name: "L".into(),
                pin: None,
            },
        );
        assert_eq!(s, MiracastStatus::ConnectionPending);
        s = status_after(
            s,
            &MiracastEvent::Connected {
                id,
                device_name: "L".into(),
            },
        );
        assert_eq!(s, MiracastStatus::Connected);
        s = status_after(s, &MiracastEvent::Disconnected { id });
        assert_eq!(s, MiracastStatus::Listening);
        s = status_after(s, &MiracastEvent::Error("x".into()));
        assert_eq!(s, MiracastStatus::Failed);
        // A late disconnect does not revive a failed receiver.
        s = status_after(s, &MiracastEvent::Disconnected { id });
        assert_eq!(s, MiracastStatus::Failed);
    }

    #[test]
    fn informational_events_keep_the_status() {
        use MiracastStatus as S;
        let rejected = MiracastEvent::ConnectionRejected {
            reason: RejectReason::Busy,
            device_name: Some("Other".into()),
        };
        let failed = MiracastEvent::ConnectionFailed {
            id: None,
            message: "x".into(),
        };
        for s in [
            S::Listening,
            S::ConnectionPending,
            S::Connected,
            S::Unavailable,
            S::Failed,
        ] {
            assert_eq!(status_after(s, &rejected), s);
            assert_eq!(status_after(s, &failed), s);
        }
    }

    #[test]
    fn disconnected_only_recovers_live_connections() {
        use MiracastStatus as S;
        let d = MiracastEvent::Disconnected {
            id: ConnectionId(1),
        };
        assert_eq!(status_after(S::Connected, &d), S::Listening);
        assert_eq!(status_after(S::ConnectionPending, &d), S::Listening);
        for s in [
            S::Off,
            S::Starting,
            S::Unavailable,
            S::Unsupported,
            S::Failed,
            S::Listening,
        ] {
            assert_eq!(status_after(s, &d), s);
        }
    }
}
