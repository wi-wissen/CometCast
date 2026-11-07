// SPDX-License-Identifier: GPL-3.0-or-later
//! The parts of the macOS backend that do not need macOS: what a ScreenCaptureKit frame status
//! means, which error codes mean "no" or "the system stopped it", the minimum system version
//! and the capture size. Compiled and tested everywhere; `macos_sck` uses it.

use crate::CaptureError;
use crate::session::Event;

/// `SCContentSharingPicker` needs macOS 14 (Sonoma).
pub const MIN_MACOS_MAJOR: i64 = 14;

/// True if a macOS with this major version has the system content-sharing picker.
pub fn picker_supported(major: i64) -> bool {
    major >= MIN_MACOS_MAJOR
}

/// What to do with a sample buffer, from its `SCFrameStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    /// A new picture: read it.
    Use,
    /// Nothing changed (`Idle`): no new picture, the stream is alive.
    Unchanged,
    /// The source delivers nothing for now (`Blank`, `Suspended`: window minimised, screen
    /// locked).
    Pause,
    /// The stream has started (`Started`): no picture yet.
    Started,
    /// The stream stopped (`Stopped`).
    End,
}

/// Map `SCFrameStatus` (`Complete` 0, `Idle` 1, `Blank` 2, `Suspended` 3, `Started` 4,
/// `Stopped` 5). Unknown values are treated as "no new picture".
pub fn frame_action(status: i64) -> FrameAction {
    match status {
        0 => FrameAction::Use,
        1 => FrameAction::Unchanged,
        2 | 3 => FrameAction::Pause,
        4 => FrameAction::Started,
        5 => FrameAction::End,
        _ => FrameAction::Unchanged,
    }
}

/// `SCStreamErrorUserDeclined`.
pub const ERR_USER_DECLINED: i64 = -3801;
/// `SCStreamErrorMissingEntitlements`.
pub const ERR_MISSING_ENTITLEMENTS: i64 = -3803;
/// `SCStreamErrorUserStopped`: the person pressed stop in the system's sharing indicator.
pub const ERR_USER_STOPPED: i64 = -3817;

/// Map an error from starting the stream (or from the picker) to a [`CaptureError`]. The
/// person declining is [`CaptureError::Denied`].
///
/// UNCERTAIN: the codes are from Apple's `SCStreamError` header as remembered; none was seen
/// on a Mac. Screen Recording permission (TCC) may also show up as `ERR_USER_DECLINED`.
pub fn map_start_error(code: i64, message: &str) -> CaptureError {
    match code {
        ERR_USER_DECLINED => CaptureError::Denied,
        ERR_MISSING_ENTITLEMENTS => CaptureError::Unsupported(
            "ScreenCaptureKit refused: the app lacks the screen capture entitlement",
        ),
        other => CaptureError::Display(format!("{message} ({other})")),
    }
}

/// The state-machine event for `stream:didStopWithError:`. Pressing stop in the system
/// indicator is a normal end; any other error means the source is gone. Both end the session
/// without a failure (the app returns to "What to show").
pub fn stop_event(code: i64) -> Event {
    if code == ERR_USER_STOPPED {
        Event::SystemStopped
    } else {
        Event::SourceClosed
    }
}

/// Pixel size of the capture from the content rectangle (points) and the points-to-pixels
/// scale of the filter. Rounded down to even numbers (4:2:0 video), at least 2.
pub fn capture_size(width_pt: f64, height_pt: f64, scale: f32) -> (usize, usize) {
    let s = if scale.is_finite() && scale > 0.0 {
        f64::from(scale)
    } else {
        1.0
    };
    let even = |v: f64| -> usize {
        let px = (v * s).max(2.0).min(f64::from(crate::convert::MAX_SIDE)) as usize;
        px & !1
    };
    (even(width_pt).max(2), even(height_pt).max(2))
}

/// The minimum frame interval for `fps` as (value, timescale): 1/fps seconds.
pub fn frame_interval(fps: u32) -> (i64, i32) {
    (1, fps.clamp(1, 240) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_statuses() {
        assert_eq!(frame_action(0), FrameAction::Use);
        assert_eq!(frame_action(1), FrameAction::Unchanged);
        assert_eq!(frame_action(2), FrameAction::Pause);
        assert_eq!(frame_action(3), FrameAction::Pause);
        assert_eq!(frame_action(4), FrameAction::Started);
        assert_eq!(frame_action(5), FrameAction::End);
        assert_eq!(frame_action(99), FrameAction::Unchanged);
    }

    #[test]
    fn errors() {
        assert_eq!(
            map_start_error(ERR_USER_DECLINED, "x"),
            CaptureError::Denied
        );
        assert!(matches!(
            map_start_error(ERR_MISSING_ENTITLEMENTS, "x"),
            CaptureError::Unsupported(_)
        ));
        assert!(matches!(
            map_start_error(-1, "start failed"),
            CaptureError::Display(m) if m.contains("-1")
        ));
        assert_eq!(stop_event(ERR_USER_STOPPED), Event::SystemStopped);
        assert_eq!(stop_event(-3808), Event::SourceClosed);
    }

    #[test]
    fn version_gate() {
        assert!(!picker_supported(13));
        assert!(picker_supported(14));
        assert!(picker_supported(26));
    }

    #[test]
    fn sizes_are_even_and_scaled() {
        // A 1512x982 point screen at 2x.
        assert_eq!(capture_size(1512.0, 982.0, 2.0), (3024, 1964));
        // Odd results are rounded down to even.
        assert_eq!(capture_size(101.0, 51.0, 1.0), (100, 50));
        // Bad scale counts as 1; tiny sizes have a floor.
        assert_eq!(capture_size(800.0, 600.0, 0.0), (800, 600));
        assert_eq!(capture_size(0.0, 0.0, f32::NAN), (2, 2));
    }

    #[test]
    fn interval() {
        assert_eq!(frame_interval(30), (1, 30));
        assert_eq!(frame_interval(0), (1, 1));
        assert_eq!(frame_interval(10_000), (1, 240));
    }
}
