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
    /// A device's mirror stream started: UxPlay's mirror thread begins to accept the video
    /// connection. The line is [`MIRRORING_STARTED_LINE`]; its text is a format string of the
    /// UxPlay 1.68 binary (found in the shipped binary, see `docs/airplay.md`; the supervisor
    /// believes it only after a PIN was shown in the same connection).
    MirroringStarted,
    /// The stream may have stopped (client silent). A HINT only.
    /// UNCERTAIN: the exact text is not known. UxPlay has the option `-reset n`
    /// ("Reset after 3n seconds client silence"), so we accept a line that starts with
    /// "reset" and mentions "client silence". Not seen in a real run.
    StreamStalled,
}

/// The line that UxPlay prints when a device starts mirroring (`raop_rtp_mirror.c`, logged when
/// the mirror thread starts). Read from the format strings of the shipped binary; the end of a
/// mirroring is `Open connections: 0` (needs UxPlay's `-d`, which this crate always passes for
/// receiving, see [`crate::AirplayConfig::debug_log`]).
pub const MIRRORING_STARTED_LINE: &str = "raop_rtp_mirror starting mirroring";

/// Longest message text we keep for an error (keeps events small).
const MAX_TEXT: usize = 300;

fn short(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

/// The exact start and end of the real PIN line (UxPlay 1.68).
const PIN_PREFIX: &str = "*** CLIENT MUST NOW ENTER PIN = \"";
const PIN_SUFFIX: &str = "\" AS AIRPLAY PASSWORD";
/// The exact real pairing line (UxPlay 1.68).
const PAIR_LINE: &str = "client sent PAIR-PIN-START request";

/// Look at one line (without the line break). Returns `None` for lines we ignore.
///
/// Every pattern is anchored: the line must START with the known text (and the PIN line
/// must END with the known text). A marker in the middle of a line is ignored, because
/// parts of UxPlay's log can contain text chosen by a device (for example a device name),
/// and such text must not be able to fake a PIN. A device that can inject a whole line
/// (a line break inside its name) is still a risk: this is why the PIN is only a display
/// value, and the supervisor believes a PIN line only right after a pairing line.
pub fn parse_line(line: &str) -> Option<Recognized> {
    // Only the line end is trimmed. Leading spaces mean "not the start of the line".
    let line = line.trim_end();
    if line.is_empty() {
        return None;
    }
    if line == "Initialized server socket(s)" {
        return Some(Recognized::ServerReady);
    }
    if let Some(rest) = line.strip_prefix(PIN_PREFIX) {
        let digits = rest.strip_suffix(PIN_SUFFIX)?;
        if digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()) {
            return Some(Recognized::PinShown {
                pin: digits.to_string(),
            });
        }
        // UNCERTAIN: newer versions may use other PIN lengths. We refuse to guess.
        return None;
    }
    if line == PAIR_LINE {
        return Some(Recognized::PairingRequested);
    }
    if let Some(n) = line.strip_prefix("Open connections: ") {
        if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) {
            return n.parse().ok().map(Recognized::OpenConnections);
        }
        return None;
    }
    if line == MIRRORING_STARTED_LINE {
        return Some(Recognized::MirroringStarted);
    }
    if line == "Stopping..." {
        return Some(Recognized::Stopping);
    }
    if line.starts_with("*** ERROR") || line.starts_with("Required gstreamer plugin") {
        return Some(Recognized::Error(short(line)));
    }
    // Hint only (UNCERTAIN wording, never seen for real): a line that starts with "reset"
    // and mentions client silence.
    if line.starts_with("reset") && line.to_ascii_lowercase().contains("client silence") {
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
    fn mirroring_started_line() {
        assert_eq!(
            parse_line("raop_rtp_mirror starting mirroring"),
            Some(Recognized::MirroringStarted)
        );
        assert_eq!(
            parse_line("raop_rtp_mirror starting mirroring\r"),
            Some(Recognized::MirroringStarted)
        );
        // Anchored: a device name cannot fake it from the middle of a line, and near misses
        // are nothing.
        for l in [
            "Remote: raop_rtp_mirror starting mirroring",
            " raop_rtp_mirror starting mirroring",
            "raop_rtp_mirror starting mirroring now",
            "raop_rtp_mirror starting",
        ] {
            assert_eq!(parse_line(l), None, "{l:?}");
        }
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
            // Spoofed lines: the marker is not at the start / the end does not match.
            "Remote device: *** CLIENT MUST NOW ENTER PIN = \"0000\" AS AIRPLAY PASSWORD",
            " *** CLIENT MUST NOW ENTER PIN = \"0000\" AS AIRPLAY PASSWORD",
            "*** CLIENT MUST NOW ENTER PIN = \"0000\" AS AIRPLAY PASSWORD and more",
            "*** CLIENT MUST NOW ENTER PIN = \"0000\"",
            "name=x client sent PAIR-PIN-START request",
            "client sent PAIR-PIN-START request, honest",
            "request timeout from device",
            "Open connections: 1 extra",
            "Open connections: -1",
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
    fn words_in_the_middle_do_not_make_a_stall() {
        assert_eq!(parse_line("device name: reset client silence"), None);
        assert_eq!(parse_line("connection timeout"), None);
    }

    #[test]
    fn very_long_error_is_cut() {
        let l = format!("*** ERROR: {}", "x".repeat(5000));
        assert!(
            matches!(parse_line(&l), Some(Recognized::Error(m)) if m.chars().count() == MAX_TEXT)
        );
    }
}
