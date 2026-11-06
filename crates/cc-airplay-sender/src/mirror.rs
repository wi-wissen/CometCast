// SPDX-License-Identifier: GPL-3.0-or-later
//! The place where real AirPlay screen mirroring would plug in, and nothing else.
//!
//! Real AirPlay screen mirroring needs the FairPlay handshake (`fp-setup`) before the
//! receiver accepts a mirroring stream. The project owner forbids taking over FairPlay
//! material: no `fp-setup`, no FairPlay tables or keys, no code or data from AirParrot,
//! AirServer or the AGPL RustDesk code, no key or certificate extraction, no DRM
//! circumvention. This crate therefore only defines the trait and one type that always says
//! "unsupported". The working alternative is the HLS path (`crate::screen`).

use crate::discovery::Device;

/// Why mirroring does not work.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MirroringError {
    /// There is no implementation, on purpose.
    #[error(
        "AirPlay screen mirroring is not supported: it needs the FairPlay handshake (fp-setup), \
         and this project does not implement or call FairPlay. Use the HLS screen stream \
         (`cometcast-cli airplay-send screen`) instead; it has a delay of several seconds."
    )]
    Unsupported,
}

/// Something that could start and stop real AirPlay screen mirroring to a receiver.
///
/// There is no implementation in this repository.
pub trait MirroringProvider {
    /// Start mirroring the screen to `device`.
    fn start(&mut self, device: &Device) -> Result<(), MirroringError>;
    /// Stop mirroring.
    fn stop(&mut self) -> Result<(), MirroringError>;
}

/// The only "implementation": always refuses and says why.
#[derive(Debug, Default, Clone, Copy)]
pub struct Unsupported;

impl MirroringProvider for Unsupported {
    fn start(&mut self, _device: &Device) -> Result<(), MirroringError> {
        Err(MirroringError::Unsupported)
    }

    fn stop(&mut self) -> Result<(), MirroringError> {
        Err(MirroringError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_says_why() {
        let d = Device::from_address("10.0.0.1").unwrap();
        let mut p = Unsupported;
        let e = p.start(&d).unwrap_err();
        assert!(e.to_string().contains("FairPlay"));
        assert!(e.to_string().contains("fp-setup"));
        assert_eq!(p.stop(), Err(MirroringError::Unsupported));
    }
}
