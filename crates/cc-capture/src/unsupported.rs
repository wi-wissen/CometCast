// SPDX-License-Identifier: GPL-3.0-or-later
//! Stand-ins for capture backends that are not compiled in. Each one implements [`ScreenSource`]
//! and answers every call with [`CaptureError::Unsupported`]. [`WaylandPipeWire`] is what
//! `open_picker` reports on Linux when the crate was built without the `wayland` feature;
//! [`Windows`] and [`MacOs`] are kept for code that wants a value of the right type on a system
//! whose real backend (`windows_wgc`, `macos_sck`) is only built on that system.

use crate::{Capture, CaptureError, CaptureSource, ScreenSource};

macro_rules! stub {
    ($(#[$doc:meta])* $name:ident, $why:literal) => {
        $(#[$doc])*
        #[derive(Debug, Default, Clone, Copy)]
        pub struct $name;

        impl $name {
            /// The error every method of this backend returns.
            pub fn error(&self) -> CaptureError {
                CaptureError::Unsupported($why)
            }
        }

        impl ScreenSource for $name {
            fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
                Err(self.error())
            }
            fn start(&mut self, _source: &CaptureSource) -> Result<(), CaptureError> {
                Err(self.error())
            }
            fn next_frame(&mut self) -> Result<Capture, CaptureError> {
                Err(self.error())
            }
            fn stop(&mut self) {}
        }
    };
}

stub!(
    /// Linux/Wayland through the desktop portal and PipeWire: the real backend is
    /// `wayland::WaylandSource` (cargo feature `wayland`); this is the answer without it.
    WaylandPipeWire,
    "Wayland capture needs a build with the `wayland` feature (desktop portal + PipeWire)"
);

stub!(
    /// Windows: the real backend is `windows_wgc::WgcSource` (Windows targets only).
    Windows,
    "Windows capture is only built on Windows"
);

stub!(
    /// macOS: the real backend is `macos_sck::SckSource` (macOS targets only).
    MacOs,
    "macOS capture is only built on macOS"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_return_a_clear_error() {
        let mut s = WaylandPipeWire;
        assert!(
            matches!(s.list_sources(), Err(CaptureError::Unsupported(m)) if m.contains("Wayland"))
        );
        assert!(matches!(s.next_frame(), Err(CaptureError::Unsupported(_))));
        assert!(matches!(
            Windows.list_sources(),
            Err(CaptureError::Unsupported(_))
        ));
        assert!(matches!(
            MacOs.list_sources(),
            Err(CaptureError::Unsupported(_))
        ));
    }
}
