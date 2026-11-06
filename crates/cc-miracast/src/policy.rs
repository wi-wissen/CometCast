// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast's own connection rules (independent of Windows, testable on Linux).

use std::time::{Duration, Instant};

/// Longest receiver display name we accept.
// UNCERTAIN: 32 is a conservative guess (the Wi-Fi Display device name field is short).
// The real limit is checked by Windows (`FriendlyNameTooLong`); not measured.
pub const MAX_NAME_CHARS: usize = 32;

/// Longest foreign device name that is passed on to the person (after cleaning), in characters.
// UNCERTAIN: 64 is a product choice; the Wi-Fi Display name limit was not measured.
pub const MAX_DEVICE_NAME_CHARS: usize = 64;

/// Minimum time between two reports of the same rejection reason.
pub const REJECT_REPORT_INTERVAL: Duration = Duration::from_secs(5);

/// Default and hard maximum of simultaneous sessions.
// UNCERTAIN: one session is a product decision (like AirPlay in `cc-airplay`), not an API limit.
pub const DEFAULT_MAX_SESSIONS: usize = 1;

/// How long a connection request waits for a person before it is rejected.
// UNCERTAIN: 30 seconds is a guess; the Windows timeout for a pending connection is not known.
pub const CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

/// What to do with a new foreign connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Ask the person at the receiver. The only way to get a connection accepted.
    AskPerson {
        /// The sender name after [`sanitize_device_name`]; the only form that may be shown.
        device_name: String,
    },
    /// Reject without asking.
    Reject(RejectReason),
}

/// Why a connection is rejected without asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The maximum number of sessions is already in use or waiting for a decision.
    Busy,
    /// The device name is empty after cleaning (cannot be shown to the person).
    NoDeviceName,
    /// Windows gave no `Deferral`, so the connection cannot be held for a confirmation.
    ConfirmationUnavailable,
}

/// Is this character invisible or able to change how surrounding text is drawn?
/// Covers format characters (Unicode category Cf: bidi controls, zero-width characters,
/// soft hyphen, word joiner, tags, ...) and a few invisible fillers that are not Cf.
fn is_invisible_or_format(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{115F}' | '\u{1160}' | '\u{17B4}'
        | '\u{17B5}' | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{2800}' | '\u{3164}'
        | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}' | '\u{FFA0}' | '\u{FFF9}'..='\u{FFFB}'
        | '\u{E0000}'..='\u{E0FFF}')
}

/// Clean a sender-announced device name before it is shown to a person.
///
/// A foreign device chooses this text freely, so it must not be able to fake the look of
/// the confirmation line. Removes control, format, bidi and zero-width characters, turns
/// any run of white space (including newlines and tabs) into one space, trims, and cuts the
/// result to [`MAX_DEVICE_NAME_CHARS`] characters. Returns `None` when nothing visible is
/// left.
#[must_use]
pub fn sanitize_device_name(raw: &str) -> Option<String> {
    let kept: String = raw
        .chars()
        .filter_map(|c| {
            if c.is_whitespace() {
                Some(' ')
            } else if c.is_control() || is_invisible_or_format(c) {
                None
            } else {
                Some(c)
            }
        })
        .collect();
    let collapsed = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(MAX_DEVICE_NAME_CHARS).collect();
    let cut = cut.trim_end();
    if cut.is_empty() {
        None
    } else {
        Some(cut.to_string())
    }
}

/// Limits how often the same rejection reason is reported, so a nearby sender that keeps
/// retrying cannot flood the event channel.
#[derive(Debug, Default)]
pub struct RejectLimiter {
    last: Option<(RejectReason, Instant)>,
}

impl RejectLimiter {
    /// Should a rejection with this reason be reported now?
    pub fn allow(&mut self, reason: RejectReason, now: Instant) -> bool {
        if let Some((r, at)) = self.last
            && r == reason
            && now.saturating_duration_since(at) < REJECT_REPORT_INTERVAL
        {
            return false;
        }
        self.last = Some((reason, now));
        true
    }
}

/// The connection rules. A unit type so it can be named in docs and tests.
pub struct ConnectionPolicy;

impl ConnectionPolicy {
    /// Decide about a new connection.
    ///
    /// * `in_use`: sessions that are active or waiting for a decision.
    /// * `max_sessions`: the configured limit; values above [`DEFAULT_MAX_SESSIONS`] are
    ///   cut down to it ("max one active session").
    /// * `device_name`: name the sender announced (raw; it is cleaned here).
    ///
    /// There is no input that returns "accept": confirmation is required for every foreign
    /// device (assuming the `Deferral` really keeps the connection waiting, which is
    /// checklist item 8 and UNCERTAIN).
    #[must_use]
    pub fn evaluate(in_use: usize, max_sessions: usize, device_name: &str) -> Decision {
        let limit = max_sessions.clamp(1, DEFAULT_MAX_SESSIONS);
        if in_use >= limit {
            return Decision::Reject(RejectReason::Busy);
        }
        match sanitize_device_name(device_name) {
            Some(device_name) => Decision::AskPerson { device_name },
            None => Decision::Reject(RejectReason::NoDeviceName),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_receiver_asks_the_person() {
        assert_eq!(
            ConnectionPolicy::evaluate(0, 1, "Laptop"),
            Decision::AskPerson {
                device_name: "Laptop".into()
            }
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
        assert!(matches!(
            ConnectionPolicy::evaluate(0, 0, "A"),
            Decision::AskPerson { .. }
        ));
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
                    assert!(matches!(
                        d,
                        Decision::AskPerson { .. } | Decision::Reject(_)
                    ));
                }
            }
        }
    }

    #[test]
    fn sanitize_strips_bidi_and_zero_width() {
        // U+202E (right-to-left override) must not survive.
        assert_eq!(
            sanitize_device_name("Bob\u{202E}gpj.exe").as_deref(),
            Some("Bobgpj.exe")
        );
        assert_eq!(
            sanitize_device_name("a\u{200B}b\u{2066}c\u{FEFF}").as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn sanitize_rejects_names_that_are_empty_after_cleaning() {
        assert_eq!(sanitize_device_name("\u{200B}\u{200D}\u{202E}"), None);
        assert_eq!(sanitize_device_name(" \t\n "), None);
        assert_eq!(sanitize_device_name("\u{0}\u{7}"), None);
        assert_eq!(
            ConnectionPolicy::evaluate(0, 1, "\u{200B}"),
            Decision::Reject(RejectReason::NoDeviceName)
        );
    }

    #[test]
    fn sanitize_collapses_newlines_and_whitespace() {
        assert_eq!(
            sanitize_device_name("  Laptop\r\n\nAccept?   [y]\t").as_deref(),
            Some("Laptop Accept? [y]")
        );
    }

    #[test]
    fn sanitize_truncates_long_names() {
        let long = "x".repeat(10_000);
        let out = sanitize_device_name(&long).unwrap();
        assert_eq!(out.chars().count(), MAX_DEVICE_NAME_CHARS);
        // Multi-byte characters are cut on character boundaries, no trailing space is left.
        let wide = format!(
            "{} {}",
            "ä".repeat(MAX_DEVICE_NAME_CHARS - 1),
            "y".repeat(50)
        );
        let out = sanitize_device_name(&wide).unwrap();
        assert!(out.chars().count() <= MAX_DEVICE_NAME_CHARS);
        assert!(!out.ends_with(' '));
    }

    #[test]
    fn busy_is_decided_before_the_name() {
        assert_eq!(
            ConnectionPolicy::evaluate(1, 1, "\u{200B}"),
            Decision::Reject(RejectReason::Busy)
        );
    }

    #[test]
    fn reject_limiter_suppresses_repeats_of_the_same_reason() {
        let mut l = RejectLimiter::default();
        let t0 = Instant::now();
        assert!(l.allow(RejectReason::Busy, t0));
        assert!(!l.allow(RejectReason::Busy, t0 + Duration::from_secs(1)));
        assert!(l.allow(RejectReason::NoDeviceName, t0 + Duration::from_secs(1)));
        assert!(l.allow(
            RejectReason::NoDeviceName,
            t0 + Duration::from_secs(1) + REJECT_REPORT_INTERVAL
        ));
    }
}
