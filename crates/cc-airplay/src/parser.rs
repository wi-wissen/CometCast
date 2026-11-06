// SPDX-License-Identifier: GPL-3.0-or-later
//! Reads single lines of UxPlay output and recognises the few we care about.
//!
//! Rules:
//! * Tolerant: a line we do not know is ignored (`None`), never an error.
//! * Output is only used for the display and for status. It is never a security decision:
//!   whether a device may connect is decided inside UxPlay (PIN check), not by this text.
//! * Which lines were seen for real: see the tests. Lines marked UNCERTAIN below were
//!   recalled or guessed, not observed with a real iPhone, and may differ per UxPlay version.

/// A thing we recognised in one output line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recognized {
    /// UxPlay opened its server sockets and is ready.
    /// Real line (1.68): `Initialized server socket(s)`.
    ServerReady,
    /// A device asked to pair and UxPlay shows a PIN.
    /// Real line (1.68): `*** CLIENT MUST NOW ENTER PIN = "8227" AS AIRPLAY PASSWORD`.
    PinShown {
        /// Exactly four ASCII digits.
        pin: String,
    },
    /// A device asked to pair (before the PIN line).
    /// Real line (1.68): `client sent PAIR-PIN-START request`.
    PairingRequested,
    /// Number of open network connections.
    /// Real line (1.68): `Open connections: 1`. Note: this counts every TCP connection,
    /// including short probes that never mirror anything (the real log shows 1 then 0).
    OpenConnections(u32),
    /// UxPlay reports an error (`*** ERROR: ...`, or a missing GStreamer plugin).
    /// Real lines (1.68): `*** ERROR: No DNS-SD Server found (...)`,
    /// `Required gstreamer plugin 'app' not found`.
    Error(String),
    /// UxPlay is shutting down by itself. Real line (1.68): `Stopping...`.
    Stopping,
    /// The stream seems to have stopped (client silent).
    /// UNCERTAIN: the exact text is not known. UxPlay has the option `-reset n`
    /// ("Reset after 3n seconds client silence"), so we look for words like "silence" or
    /// "timed out" or "timeout". Not seen in a real run.
    StreamStalled,
}

/// Longest message text we keep for an error (keeps events small).
const MAX_TEXT: usize = 300;

fn short(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

/// Look at one line (without the line break). Returns `None` for lines we ignore.
pub fn parse_line(line: &str) -> Option<Recognized> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if line == "Initialized server socket(s)" {
        return Some(Recognized::ServerReady);
    }
    if let Some(rest) = line.split_once("CLIENT MUST NOW ENTER PIN").map(|(_, r)| r) {
        // rest looks like: ` = "8227" AS AIRPLAY PASSWORD`
        let digits = rest.split('"').nth(1)?;
        if digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()) {
            return Some(Recognized::PinShown {
                pin: digits.to_string(),
            });
        }
        // UNCERTAIN: newer versions may use other PIN lengths. We refuse to guess.
        return None;
    }
    if line.contains("PAIR-PIN-START") {
        return Some(Recognized::PairingRequested);
    }
    if let Some(n) = line.strip_prefix("Open connections:") {
        return n.trim().parse().ok().map(Recognized::OpenConnections);
    }
    if line == "Stopping..." {
        return Some(Recognized::Stopping);
    }
    if line.starts_with("*** ERROR") || line.starts_with("Required gstreamer plugin") {
        return Some(Recognized::Error(short(line)));
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("client silence") || lower.contains("timed out") || lower.contains("timeout")
    {
        return Some(Recognized::StreamStalled);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_PIN: &str = include_str!("../tests/fixtures/uxplay-1.68-real-pin.txt");
    const REAL_PLUGINS: &str =
        include_str!("../tests/fixtures/uxplay-1.68-real-missing-plugins.txt");
    const REAL_NO_DNSSD: &str = include_str!("../tests/fixtures/uxplay-1.68-real-no-dnssd.txt");

    fn all(text: &str) -> Vec<Recognized> {
        text.lines().filter_map(parse_line).collect()
    }

    #[test]
    fn real_pin_run() {
        let got = all(REAL_PIN);
        assert_eq!(
            got,
            [
                Recognized::ServerReady,
                Recognized::PairingRequested,
                Recognized::PinShown { pin: "8227".into() },
                Recognized::Stopping,
            ]
        );
    }

    #[test]
    fn ascii_art_banner_is_ignored() {
        // The big digits drawn with characters must never be read as a PIN.
        for l in REAL_PIN.lines().filter(|l| l.contains("d88P")) {
            assert_eq!(parse_line(l), None, "{l:?}");
        }
    }

    #[test]
    fn real_errors() {
        let got = all(REAL_PLUGINS);
        assert_eq!(got.len(), 5, "{got:?}");
        assert!(matches!(&got[0], Recognized::Error(m) if m.contains("'app'")));
        // The "*** ERROR: stopping" line at the end.
        assert!(matches!(got.last(), Some(Recognized::Error(_))));
        let got = all(REAL_NO_DNSSD);
        assert_eq!(got[0], Recognized::ServerReady);
        assert!(matches!(&got[1..], [Recognized::Error(m)] if m.contains("No DNS-SD Server")));
    }

    #[test]
    fn open_connections() {
        assert_eq!(
            parse_line("Open connections: 1"),
            Some(Recognized::OpenConnections(1))
        );
        assert_eq!(
            parse_line("Open connections: 0\r"),
            Some(Recognized::OpenConnections(0))
        );
        assert_eq!(parse_line("Open connections: many"), None);
    }

    #[test]
    fn tolerant_of_junk() {
        for l in [
            "",
            "   ",
            "\u{1b}[31mcolour\u{1b}[0m",
            "using system MAC address 6a:17:00:ed:fc:75",
            "*** WARNING: httpd didn't get response",
            "PIN = \"1234\"", // no UxPlay marker: not a PIN line
            "ENTER PIN = \"12\"",
            "\0\0\0",
            "\u{fffd}\u{fffd}",
        ] {
            assert_eq!(parse_line(l), None, "{l:?}");
        }
    }

    #[test]
    fn pin_must_be_four_digits() {
        let l = |p: &str| format!("*** CLIENT MUST NOW ENTER PIN = \"{p}\" AS AIRPLAY PASSWORD");
        assert!(
            matches!(parse_line(&l("0042")), Some(Recognized::PinShown { pin }) if pin == "0042")
        );
        for p in ["", "123", "12345", "12a4", "١٢٣٤"] {
            assert_eq!(parse_line(&l(p)), None, "{p:?}");
        }
        // Broken line without a closing quote.
        assert_eq!(parse_line("*** CLIENT MUST NOW ENTER PIN = \"12"), None);
    }

    #[test]
    fn stall_wording_is_a_guess() {
        // UNCERTAIN wording, see the doc comment on `Recognized::StreamStalled`.
        assert_eq!(
            parse_line("reset: 15 seconds of client silence"),
            Some(Recognized::StreamStalled)
        );
    }

    #[test]
    fn very_long_error_is_cut() {
        let l = format!("*** ERROR: {}", "x".repeat(5000));
        assert!(
            matches!(parse_line(&l), Some(Recognized::Error(m)) if m.chars().count() == MAX_TEXT)
        );
    }
}
