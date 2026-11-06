// SPDX-License-Identifier: GPL-3.0-or-later
//! Hook for testing a hardware encoder before it is trusted. Trait only: no hardware encoder
//! exists in Comet Cast yet.
//!
//! Idea (docs/measurement-points.md, A11): a hardware encoder is first tried in a separate
//! child process with a time limit, so that a crashing or hanging driver cannot take the
//! sender down. The result is stored together with a fingerprint of the graphics hardware and
//! driver and is discarded when the fingerprint changes. If the encoder fails while running,
//! the sender falls back to the software encoder ([`crate::H264Encoder`]).
//!
//! UNCERTAIN: nothing here has been tried on real hardware. The 30 s limit and the 1280x720
//! test size below are placeholders, not measured values.

use std::time::Duration;

/// Result of one probe run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// The encoder produced a decodable key frame and delta frames.
    Works,
    /// The encoder exists but failed; the text is for the log.
    Failed(String),
    /// No such encoder on this machine.
    Unavailable,
    /// The child process did not finish in time (and was killed).
    TimedOut,
}

/// How a platform backend exposes its hardware encoder test.
pub trait HardwareEncoderProbe {
    /// Short name of the encoder for logs, e.g. a platform API name.
    fn name(&self) -> &str;

    /// Text that changes when the graphics hardware or driver changes. A stored result is only
    /// valid for the fingerprint it was made with.
    fn environment_fingerprint(&self) -> String;

    /// Run the test in a child process and wait at most `limit`.
    fn probe(&self, width: u32, height: u32, limit: Duration) -> ProbeOutcome;
}

/// Placeholder test size and time limit (see the UNCERTAIN note in the module docs).
pub const DEFAULT_PROBE_SIZE: (u32, u32) = (1280, 720);
/// Placeholder time limit for a probe.
pub const DEFAULT_PROBE_LIMIT: Duration = Duration::from_secs(30);

/// The probe used where no hardware encoder is implemented: always `Unavailable`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoHardwareEncoder;

impl HardwareEncoderProbe for NoHardwareEncoder {
    fn name(&self) -> &str {
        "none"
    }
    fn environment_fingerprint(&self) -> String {
        String::new()
    }
    fn probe(&self, _width: u32, _height: u32, _limit: Duration) -> ProbeOutcome {
        ProbeOutcome::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_reports_unavailable() {
        let p = NoHardwareEncoder;
        let (w, h) = DEFAULT_PROBE_SIZE;
        assert_eq!(
            p.probe(w, h, DEFAULT_PROBE_LIMIT),
            ProbeOutcome::Unavailable
        );
    }
}
