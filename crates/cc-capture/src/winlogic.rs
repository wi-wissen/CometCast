// SPDX-License-Identifier: GPL-3.0-or-later
//! The parts of the Windows backend that do not need Windows: which error codes of the picker
//! mean "the person said no", and how a changing capture size is followed. Compiled and tested
//! on every system; `windows_wgc` uses it.

use crate::CaptureError;

/// `E_POINTER`: what the Windows Runtime binding reports when `PickSingleItemAsync` completes
/// with no item (the person closed the picker).
pub const E_POINTER: i32 = 0x8000_4003_u32 as i32;
/// `E_ABORT`.
pub const E_ABORT: i32 = 0x8000_4004_u32 as i32;
/// `E_ACCESSDENIED`.
pub const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
/// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`.
pub const E_CANCELLED: i32 = 0x8007_04C7_u32 as i32;
/// `S_OK`: a failed call that carries no error code (an empty result).
pub const S_OK: i32 = 0;

/// Map the HRESULT of a failed picker call. Cancelling, an empty result and access denied are
/// [`CaptureError::Denied`]; everything else is a display error with the code in hex.
///
/// UNCERTAIN: which of these a given Windows build returns when the picker is closed was not
/// observed (no Windows machine); the list is the set that is plausible from the API contract.
pub fn map_pick_error(hresult: i32, message: &str) -> CaptureError {
    match hresult {
        E_POINTER | E_ABORT | E_ACCESSDENIED | E_CANCELLED | S_OK => CaptureError::Denied,
        code => CaptureError::Display(format!("{message} (0x{:08X})", code as u32)),
    }
}

/// Follows the size of the captured item: a window can be resized while it is shared, and the
/// frame pool must then be recreated with the new size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeTracker {
    current: (u32, u32),
}

impl SizeTracker {
    /// Start with the size at the time of picking.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            current: (width, height),
        }
    }

    /// The size the frame pool was built for.
    pub fn current(&self) -> (u32, u32) {
        self.current
    }

    /// Report the content size of a new frame. Returns `Some(new_size)` when the pool must be
    /// recreated (the size changed and is usable), `None` otherwise. A zero size (minimised
    /// window) never triggers a recreate.
    pub fn observe(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 || (width, height) == self.current {
            return None;
        }
        self.current = (width, height);
        Some(self.current)
    }
}

/// The crop to apply when the texture is larger than the content (a frame pool keeps its
/// size while the window got smaller). `None` when the texture matches the content.
pub fn content_crop(texture: (u32, u32), content: (u32, u32)) -> Option<crate::convert::Crop> {
    let (cw, ch) = (content.0.min(texture.0), content.1.min(texture.1));
    if cw == 0 || ch == 0 || (cw, ch) == texture {
        return None;
    }
    Some(crate::convert::Crop {
        x: 0,
        y: 0,
        width: cw,
        height: ch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_like_codes_are_denied() {
        for c in [E_POINTER, E_ABORT, E_ACCESSDENIED, E_CANCELLED, S_OK] {
            assert_eq!(map_pick_error(c, "x"), CaptureError::Denied);
        }
        assert!(matches!(
            map_pick_error(0x8000_4005_u32 as i32, "picker failed"),
            CaptureError::Display(m) if m.contains("0x80004005")
        ));
    }

    #[test]
    fn size_changes_recreate_the_pool_once() {
        let mut t = SizeTracker::new(1280, 720);
        assert_eq!(t.observe(1280, 720), None);
        assert_eq!(t.observe(1920, 1080), Some((1920, 1080)));
        assert_eq!(t.observe(1920, 1080), None);
        assert_eq!(t.observe(0, 0), None);
        assert_eq!(t.observe(800, 0), None);
        assert_eq!(t.current(), (1920, 1080));
    }

    #[test]
    fn crop_only_when_content_is_smaller() {
        assert_eq!(content_crop((1920, 1080), (1920, 1080)), None);
        let c = content_crop((1920, 1080), (800, 600)).unwrap();
        assert_eq!((c.width, c.height), (800, 600));
        // Content larger than the texture is clamped to the texture.
        assert_eq!(content_crop((100, 100), (200, 200)), None);
        assert_eq!(content_crop((100, 100), (0, 50)), None);
    }
}
