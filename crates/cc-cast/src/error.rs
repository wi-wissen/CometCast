// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors of the Cast sender.

use std::time::Duration;

/// Make text that came from the network safe to print on a terminal: control characters
/// (ESC, newlines, C1 controls, ...) and bidirectional-override characters are replaced by an
/// escape like `\u{1b}`, and the result is cut to at most `max_chars` characters (a cut is
/// marked with `...`).
pub fn sanitize_display(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (n, c) in s.chars().enumerate() {
        if n >= max_chars {
            out.push_str("...");
            break;
        }
        let bidi = matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
        if c.is_control() || bidi {
            out.push_str(&format!("\\u{{{:x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

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

#[cfg(test)]
mod tests {
    use super::sanitize_display;

    #[test]
    fn control_characters_are_escaped_and_length_is_capped() {
        assert_eq!(sanitize_display("Living Room", 64), "Living Room");
        assert_eq!(
            sanitize_display("a\x1b[2Jb\nc\u{202e}d\u{85}", 64),
            "a\\u{1b}[2Jb\\u{a}c\\u{202e}d\\u{85}"
        );
        assert_eq!(sanitize_display("abcdef", 3), "abc...");
        assert_eq!(sanitize_display("abc", 3), "abc");
        // Counted in characters, never cut inside one.
        assert_eq!(sanitize_display("ééé", 2), "éé...");
    }
}
