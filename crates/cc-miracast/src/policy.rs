// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast's own connection rules (independent of Windows, testable on Linux).

use std::time::Duration;

/// Longest receiver display name we accept.
// UNCERTAIN: 32 is a conservative guess (the Wi-Fi Display device name field is short).
// The real limit is checked by Windows (`FriendlyNameTooLong`); not measured.
pub const MAX_NAME_CHARS: usize = 32;

/// Default and hard maximum of simultaneous sessions.
// UNCERTAIN: one session is a product decision (like AirPlay in `cc-airplay`), not an API limit.
pub const DEFAULT_MAX_SESSIONS: usize = 1;

/// How long a connection request waits for a person before it is rejected.
// UNCERTAIN: 30 seconds is a guess; the Windows timeout for a pending connection is not known.
pub const CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

/// What to do with a new foreign connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Ask the person at the receiver. The only way to get a connection accepted.
    AskPerson,
    /// Reject without asking.
    Reject(RejectReason),
}

/// Why a connection is rejected without asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The maximum number of sessions is already in use or waiting for a decision.
    Busy,
    /// The device name is empty (cannot be shown to the person for confirmation).
    NoDeviceName,
}

/// The connection rules. A unit type so it can be named in docs and tests.
pub struct ConnectionPolicy;

impl ConnectionPolicy {
    /// Decide about a new connection.
    ///
    /// * `in_use`: sessions that are active or waiting for a decision.
    /// * `max_sessions`: the configured limit; values above [`DEFAULT_MAX_SESSIONS`] are
    ///   cut down to it ("max one active session").
    /// * `device_name`: name the sender announced.
    ///
    /// There is no input that returns "accept": confirmation is required for every foreign
    /// device.
    #[must_use]
    pub fn evaluate(in_use: usize, max_sessions: usize, device_name: &str) -> Decision {
        let limit = max_sessions.clamp(1, DEFAULT_MAX_SESSIONS);
        if in_use >= limit {
            return Decision::Reject(RejectReason::Busy);
        }
        if device_name.trim().is_empty() {
            return Decision::Reject(RejectReason::NoDeviceName);
        }
        Decision::AskPerson
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_receiver_asks_the_person() {
        assert_eq!(
            ConnectionPolicy::evaluate(0, 1, "Laptop"),
            Decision::AskPerson
        );
    }

    #[test]
    fn second_session_is_rejected() {
        assert_eq!(
            ConnectionPolicy::evaluate(1, 1, "Laptop"),
            Decision::Reject(RejectReason::Busy)
        );
    }

    #[test]
    fn configured_limit_cannot_exceed_one() {
        assert_eq!(
            ConnectionPolicy::evaluate(1, 5, "Laptop"),
            Decision::Reject(RejectReason::Busy)
        );
        // 0 is treated as 1, not as "unlimited" or "always busy".
        assert_eq!(ConnectionPolicy::evaluate(0, 0, "A"), Decision::AskPerson);
    }

    #[test]
    fn nameless_device_is_rejected() {
        assert_eq!(
            ConnectionPolicy::evaluate(0, 1, "  "),
            Decision::Reject(RejectReason::NoDeviceName)
        );
    }

    #[test]
    fn never_accepts_without_asking() {
        for in_use in 0..4 {
            for max in 0..4 {
                for name in ["", "x", "Laptop"] {
                    let d = ConnectionPolicy::evaluate(in_use, max, name);
                    assert!(matches!(d, Decision::AskPerson | Decision::Reject(_)));
                }
            }
        }
    }
}
