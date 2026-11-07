// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen capture behind one trait ([`ScreenSource`]). Each desktop system gets its own
//! backend. Mobile capture is written natively in Swift/Kotlin and does not use this crate.
//!
//! What exists:
//!
//! * [`x11::X11Source`]: Linux/X11 through the pure-Rust `x11rb` crate (core `GetImage`,
//!   plus XDAMAGE as a cheap "nothing changed" hint when the server has it, plus RandR for the
//!   monitor list). Unchanged pictures are reported as [`Capture::Unchanged`], so the caller
//!   does not encode them (docs/measurement-points.md, A10). Whole desktop, no picker.
//! * **Picker backends** (the system shows its own dialog; this crate draws no UI), opened with
//!   [`open_picker`]:
//!   [`wayland`] (xdg-desktop-portal ScreenCast + PipeWire, cargo feature `wayland`),
//!   `windows_wgc` (Windows.Graphics.Capture + `GraphicsCapturePicker`, Windows only) and
//!   `macos_sck` (ScreenCaptureKit + `SCContentSharingPicker`, macOS 14+, macOS only).
//!   A cancelled or refused picker is [`CaptureError::Denied`].
//! * [`unsupported`]: stubs that only return [`CaptureError::Unsupported`], used where a
//!   backend was not compiled in.
//!
//! Not done: the mouse pointer is not sent separately yet (A10 wants it as its own message;
//! the picker backends ask the system to draw it into the picture where they can); no audio;
//! no dirty rectangles, only "whole picture changed or not".
//!
//! See docs/capture.md.

pub mod change;
pub mod convert;
pub mod sck_logic;
pub mod session;
pub mod unsupported;
pub mod wayland;
pub mod winlogic;
pub mod x11;

#[cfg(target_os = "macos")]
pub mod macos_sck;
#[cfg(target_os = "windows")]
pub mod windows_wgc;

#[cfg(any(test, feature = "test-support"))]
pub mod testutil;

pub use change::ChangeDetector;

/// A screen or window that can be captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSource {
    /// Id that is only meaningful to the backend.
    pub id: String,
    /// Name shown to the user.
    pub name: String,
    /// Left edge in desktop coordinates.
    pub x: i32,
    /// Top edge in desktop coordinates.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// One captured picture in BGRA byte order (4 bytes per pixel, rows without padding).
/// The alpha byte is always 255.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel bytes, length = width * height * 4.
    pub bgra: Vec<u8>,
}

/// Result of asking a source for its next picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capture {
    /// The picture differs from the last one that was returned.
    Frame(RawFrame),
    /// The screen did not change since the last returned frame. Nothing should be encoded.
    Unchanged,
    /// Capture has ended (source gone or stopped).
    Ended,
}

/// Why capture failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    /// This backend does not exist (yet) on this system.
    #[error("screen capture is not supported here: {0}")]
    Unsupported(&'static str),
    /// Talking to the display server failed.
    #[error("display server error: {0}")]
    Display(String),
    /// The display uses a pixel layout this code does not handle.
    #[error("unsupported pixel format: {0}")]
    PixelFormat(String),
    /// The source id is unknown.
    #[error("no such capture source: {0}")]
    NoSuchSource(String),
    /// `next_frame` was called before `start`.
    #[error("capture was not started")]
    NotStarted,
    /// The person cancelled the system picker, or the system refused the permission. The apps
    /// show no error for this (it is the app code `capture_denied`).
    #[error("screen capture was cancelled or refused")]
    Denied,
    /// macOS before 14: the system picker does not exist (app code `capture_os_old`).
    #[error("screen capture needs macOS 14 or newer")]
    OsTooOld,
    /// Linux/Wayland without the ScreenCast desktop portal or without libpipewire (app code
    /// `capture_no_portal`).
    #[error("no screen sharing for apps on this desktop: {0}")]
    NoPortal(&'static str),
}

/// Id of the only source a picker backend lists: "whatever the person picks in the system
/// picker". `start` with it opens the picker.
pub const PICKER_SOURCE_ID: &str = "system-picker";

/// Settings for [`open_picker`].
#[derive(Debug, Clone)]
pub struct PickerOptions {
    /// How often the screen is looked at.
    pub fps: u32,
    /// Linux/Wayland: hand a stored portal restore token to the portal, which then may skip
    /// its picker. Off, and nothing turns it on yet (open design question H1 in
    /// docs/design/implementation-questions-flutter.md).
    pub use_restore_token: bool,
    /// Windows: the window handle (HWND as integer) that owns the picker dialog. `None` uses the
    /// program's own main window (the Flutter window, decision N55), and only a program without
    /// one the foreground window. Ignored elsewhere.
    pub owner_window: Option<u64>,
    /// X11 display name for the fallback (`None` = `$DISPLAY`).
    pub x11_display: Option<String>,
}

impl Default for PickerOptions {
    fn default() -> Self {
        Self {
            fps: 15,
            use_restore_token: false,
            owner_window: None,
            x11_display: None,
        }
    }
}

/// A source of screen pictures. Implemented per operating system.
///
/// Calls block: `next_frame` waits for the next frame time. Run it on its own thread.
pub trait ScreenSource {
    /// List what can be captured right now.
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError>;
    /// Start capturing the given source.
    fn start(&mut self, source: &CaptureSource) -> Result<(), CaptureError>;
    /// Wait for the next frame time and return a new picture, `Unchanged`, or `Ended`.
    fn next_frame(&mut self) -> Result<Capture, CaptureError>;
    /// Stop capturing.
    fn stop(&mut self);
}

/// Which Linux backend [`open_picker`] uses. Pure, so the order is tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPlan {
    /// Desktop portal + PipeWire (system picker).
    Portal,
    /// X11 whole desktop (no picker).
    X11,
    /// A Wayland session where the portal (or libpipewire) is missing: `capture_no_portal`.
    NoPortal(&'static str),
    /// Nothing can capture here.
    Unavailable(&'static str),
}

/// The order on Linux: the portal first (on Wayland sessions and on X11 sessions that have
/// one), then X11 whole-desktop capture, but only when the session is not Wayland (XWayland
/// would show an empty or partial picture). `portal_ok` is only called when the `wayland`
/// feature is compiled in (`wayland_built`).
pub fn plan_linux(
    wayland_session: bool,
    have_x: bool,
    wayland_built: bool,
    portal_ok: impl FnOnce() -> bool,
) -> LinuxPlan {
    if wayland_built && portal_ok() {
        return LinuxPlan::Portal;
    }
    if !wayland_session && have_x {
        return LinuxPlan::X11;
    }
    if wayland_session && !wayland_built {
        return LinuxPlan::NoPortal(
            "Wayland session, but this build has no portal/PipeWire capture (feature `wayland`)",
        );
    }
    if wayland_session {
        return LinuxPlan::NoPortal("no ScreenCast desktop portal on this Wayland session");
    }
    LinuxPlan::Unavailable("neither DISPLAY nor WAYLAND_DISPLAY is set")
}

/// True for a Wayland session (`XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` set).
#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland"))
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Open the capture backend for the "system picker" request of the apps.
///
/// * Linux: [`plan_linux`] (portal, then X11 on X11 sessions).
/// * Windows: `GraphicsCapturePicker`.
/// * macOS: `SCContentSharingPicker` (macOS 14 or newer).
///
/// The returned source lists one entry; `start` opens the system picker and blocks until the
/// person chooses or cancels (cancel is [`CaptureError::Denied`]). Call it from a worker
/// thread, never from a UI thread.
pub fn open_picker(opts: &PickerOptions) -> Result<Box<dyn ScreenSource + Send>, CaptureError> {
    #[cfg(target_os = "linux")]
    {
        let have_x = opts.x11_display.is_some() || std::env::var_os("DISPLAY").is_some();
        #[cfg(feature = "wayland")]
        let (built, mut portal_source): (
            bool,
            Option<Result<wayland::WaylandSource, CaptureError>>,
        ) = (true, None);
        #[cfg(not(feature = "wayland"))]
        let built = false;
        let plan = plan_linux(is_wayland_session(), have_x, built, || {
            #[cfg(feature = "wayland")]
            {
                let r = wayland::WaylandSource::connect(opts);
                let ok = r.is_ok();
                portal_source = Some(r);
                ok
            }
            #[cfg(not(feature = "wayland"))]
            {
                false
            }
        });
        match plan {
            #[cfg(feature = "wayland")]
            LinuxPlan::Portal => match portal_source {
                Some(Ok(s)) => Ok(Box::new(s)),
                Some(Err(e)) => Err(e),
                None => Err(CaptureError::Display("portal was not opened".into())),
            },
            #[cfg(not(feature = "wayland"))]
            LinuxPlan::Portal => Err(CaptureError::NoPortal(
                "no portal/PipeWire capture in this build",
            )),
            LinuxPlan::X11 => Ok(Box::new(x11::X11Source::connect(
                opts.x11_display.as_deref(),
                x11::X11Options { fps: opts.fps },
            )?)),
            LinuxPlan::NoPortal(why) => Err(CaptureError::NoPortal(why)),
            LinuxPlan::Unavailable(why) => Err(CaptureError::Unsupported(why)),
        }
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows_wgc::WgcSource::new(opts)?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos_sck::SckSource::new(opts)?))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = opts;
        Err(CaptureError::Unsupported("no capture backend for this OS"))
    }
}

/// Open the capture backend for this system without a picker: the whole X11 desktop (the
/// settings of `--screen` and of `present_screen(Some(..))`).
///
/// `display` is an X11 display name such as `:99`; `None` uses `$DISPLAY`. On Windows and macOS
/// there is no picker-less backend; use [`open_picker`].
pub fn open_default(
    display: Option<&str>,
    fps: u32,
) -> Result<Box<dyn ScreenSource + Send>, CaptureError> {
    #[cfg(target_os = "linux")]
    {
        let have_x = display.is_some() || std::env::var_os("DISPLAY").is_some();
        if have_x {
            return Ok(Box::new(x11::X11Source::connect(
                display,
                x11::X11Options { fps },
            )?));
        }
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Err(CaptureError::Unsupported(
                "Wayland has no picker-less capture; use the system picker",
            ));
        }
        Err(CaptureError::Display(
            "neither DISPLAY nor WAYLAND_DISPLAY is set".into(),
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (display, fps);
        Err(CaptureError::Unsupported(
            "no picker-less capture on this system; use the system picker",
        ))
    }
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    #[test]
    fn portal_first_when_built_and_present() {
        assert_eq!(plan_linux(true, true, true, || true), LinuxPlan::Portal);
        assert_eq!(plan_linux(false, true, true, || true), LinuxPlan::Portal);
    }

    #[test]
    fn x11_session_falls_back_to_x11() {
        assert_eq!(plan_linux(false, true, true, || false), LinuxPlan::X11);
        assert_eq!(
            plan_linux(false, true, false, || panic!("not asked")),
            LinuxPlan::X11
        );
    }

    #[test]
    fn wayland_session_never_uses_xwayland() {
        assert!(matches!(
            plan_linux(true, true, true, || false),
            LinuxPlan::NoPortal(m) if m.contains("portal")
        ));
        assert!(matches!(
            plan_linux(true, true, false, || panic!("not asked")),
            LinuxPlan::NoPortal(m) if m.contains("feature")
        ));
    }

    #[test]
    fn no_display_at_all() {
        assert!(matches!(
            plan_linux(false, false, true, || false),
            LinuxPlan::Unavailable(_)
        ));
    }
}
