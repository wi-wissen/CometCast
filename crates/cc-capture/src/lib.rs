// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen capture behind one trait. Each desktop system gets its own backend later
//! (Windows, macOS, Linux via the desktop portal). Mobile capture is written natively
//! in Swift/Kotlin and does not use this crate.
//!
//! This crate only defines the trait; no backend exists yet.

/// A screen or window that can be captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSource {
    /// Id that is only meaningful to the backend.
    pub id: String,
    /// Name shown to the user.
    pub name: String,
}

/// One captured picture in BGRA byte order (4 bytes per pixel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel bytes, length = width * height * 4.
    pub bgra: Vec<u8>,
}

/// A source of screen pictures. Implemented per operating system.
pub trait ScreenSource {
    /// Error type of this backend.
    type Error;

    /// List what can be captured right now.
    fn list_sources(&self) -> Result<Vec<CaptureSource>, Self::Error>;
    /// Start capturing the given source.
    fn start(&mut self, source: &CaptureSource) -> Result<(), Self::Error>;
    /// Get the next picture, or None if capture has ended.
    fn next_frame(&mut self) -> Result<Option<RawFrame>, Self::Error>;
    /// Stop capturing.
    fn stop(&mut self);
}
