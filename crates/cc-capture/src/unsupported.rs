// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture backends that do not exist yet. Each one implements [`ScreenSource`] and answers
//! every call with [`CaptureError::Unsupported`]; their crates are deliberately not pulled in.

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
    /// Linux/Wayland through the desktop portal and PipeWire.
    ///
    /// UNCERTAIN: the plan names the xdg-desktop-portal ScreenCast interface plus PipeWire as
    /// the way; how well it works on school-PC desktops (GNOME, KDE, wlroots) is not tested.
    WaylandPipeWire,
    "Wayland capture (desktop portal + PipeWire) is not implemented"
);

stub!(
    /// Windows.
    ///
    /// UNCERTAIN: the plan mentions DXGI desktop duplication; not looked at yet.
    Windows,
    "Windows capture is not implemented"
);

stub!(
    /// macOS.
    ///
    /// UNCERTAIN: the plan mentions ScreenCaptureKit; not looked at yet, and it needs a
    /// screen-recording permission prompt that has not been designed.
    MacOs,
    "macOS capture is not implemented"
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
