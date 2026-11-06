// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen capture behind one trait ([`ScreenSource`]). Each desktop system gets its own
//! backend. Mobile capture is written natively in Swift/Kotlin and does not use this crate.
//!
//! What exists:
//!
//! * [`x11::X11Source`]: Linux/X11 through the pure-Rust `x11rb` crate (core `GetImage`,
//!   plus XDAMAGE as a cheap "nothing changed" hint when the server has it, plus RandR for the
//!   monitor list). Unchanged pictures are reported as [`Capture::Unchanged`], so the caller
//!   does not encode them (docs/measurement-points.md, A10).
//! * [`unsupported`]: stubs for Wayland/PipeWire, Windows and macOS. They only return
//!   [`CaptureError::Unsupported`].
//!
//! Not done: the mouse pointer is not part of the captured picture and is not sent
//! separately yet (A10 wants it as its own message); no audio; no dirty rectangles, only
//! "whole picture changed or not".
//!
//! See docs/capture.md.

pub mod change;
pub mod unsupported;
pub mod x11;

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

/// Open the capture backend for this system.
///
/// `display` is an X11 display name such as `:99`; `None` uses `$DISPLAY`.
///
/// UNCERTAIN: on a Linux system with Wayland only, the right answer is the desktop portal and
/// PipeWire, which is not implemented; this function then returns `Unsupported`.
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
            return Err(unsupported::WaylandPipeWire.error());
        }
        Err(CaptureError::Display(
            "neither DISPLAY nor WAYLAND_DISPLAY is set".into(),
        ))
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (display, fps);
        Err(unsupported::Windows.error())
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (display, fps);
        Err(unsupported::MacOs.error())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (display, fps);
        Err(CaptureError::Unsupported("no capture backend for this OS"))
    }
}
