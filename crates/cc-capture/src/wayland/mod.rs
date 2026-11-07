// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux/Wayland capture through the xdg-desktop-portal ScreenCast interface and PipeWire.
//!
//! The portal shows the **system's own picker** (screens and windows); this crate never draws
//! one. What is here:
//!
//! * always compiled and unit-tested: [`negotiate`] (what to ask the portal and PipeWire for,
//!   which buffers to use, picture size), [`restore`] (restore tokens in the per-user
//!   directory) and [`Shared`] (the part between the PipeWire thread and `next_frame`);
//! * behind the cargo feature `wayland` (needs `libpipewire-0.3` development files to build):
//!   `portal` (ashpd), `pw` (the PipeWire stream) and [`WaylandSource`].
//!
//! Frame path: PipeWire thread -> [`ingest`] (format conversion to BGRA) -> one-slot
//! [`Mailbox`] -> `next_frame` at the requested frame rate -> change detection.

use crate::CaptureError;
use crate::convert::{self, Crop, PixelFormat};
use crate::session::Event;

pub mod negotiate;
pub mod restore;

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod portal;
#[cfg(all(target_os = "linux", feature = "wayland"))]
mod pw;
#[cfg(all(target_os = "linux", feature = "wayland"))]
mod source;

#[cfg(all(target_os = "linux", feature = "wayland"))]
pub use source::WaylandSource;

use negotiate::{BufferVerdict, judge_buffer};

/// The format PipeWire settled on for the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFormat {
    /// Pixel layout.
    pub format: PixelFormat,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Visible rectangle inside the buffer, if the producer says so.
    pub crop: Option<Crop>,
}

pub use crate::session::Shared;

/// Handle one buffer that PipeWire delivered. `data` is the mapped memory of the first
/// data block (`None` when not mapped), `offset`/`size`/`stride`/`flags` come from its
/// chunk. Empty and corrupted buffers keep the previous picture; a DMA-BUF is fatal.
/// The first good picture moves the session to `Streaming`.
#[allow(clippy::too_many_arguments)]
pub fn ingest(
    shared: &Shared,
    fmt: &StreamFormat,
    data_type: u32,
    data: Option<&[u8]>,
    offset: u32,
    size: u32,
    stride: i32,
    flags: i32,
) {
    match judge_buffer(data_type, data.is_some(), size, flags) {
        BufferVerdict::Use => {}
        BufferVerdict::SkipEmpty | BufferVerdict::SkipCorrupted => return,
        BufferVerdict::UnsupportedDmaBuf => {
            shared.event(Event::Error(
                "the compositor sent DMA-BUF buffers although only shared memory was offered"
                    .into(),
            ));
            return;
        }
        BufferVerdict::Unreadable => {
            shared.event(Event::Error("a buffer could not be mapped".into()));
            return;
        }
    }
    let Some(data) = data else { return };
    let start = offset as usize;
    let end = start.saturating_add(size as usize).min(data.len());
    if start >= end {
        return;
    }
    let stride = if stride > 0 {
        stride as usize
    } else {
        fmt.width as usize * fmt.format.bytes_per_pixel()
    };
    match convert::to_bgra(
        fmt.format,
        &data[start..end],
        fmt.width,
        fmt.height,
        stride,
        fmt.crop,
    ) {
        Ok(frame) => shared.push_frame(frame),
        Err(CaptureError::PixelFormat(m)) => {
            shared.event(Event::Error(format!("bad buffer: {m}")));
        }
        Err(other) => shared.event(Event::Error(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Phase, Taken};
    use negotiate::{CHUNK_CORRUPTED, spa_data};
    use std::time::Duration;

    fn fmt(w: u32, h: u32) -> StreamFormat {
        StreamFormat {
            format: PixelFormat::Bgrx,
            width: w,
            height: h,
            crop: None,
        }
    }

    fn starting() -> Shared {
        let s = Shared::new();
        s.event(Event::PickerShown);
        s.event(Event::Picked);
        s
    }

    #[test]
    fn a_fake_frame_arrives_as_bgra_and_starts_streaming() {
        let s = starting();
        let px = vec![1u8, 2, 3, 0, 4, 5, 6, 0];
        ingest(&s, &fmt(2, 1), spa_data::MEM_FD, Some(&px), 0, 8, 8, 0);
        assert_eq!(s.phase(), Phase::Streaming);
        assert_eq!(s.size(), Some((2, 1)));
        match s.mailbox.take_timeout(Duration::ZERO) {
            Taken::Item(f) => assert_eq!(f.bgra, vec![1, 2, 3, 255, 4, 5, 6, 255]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn empty_and_corrupted_buffers_keep_the_old_picture() {
        let s = starting();
        let px = vec![0u8; 8];
        ingest(&s, &fmt(2, 1), spa_data::MEM_FD, Some(&px), 0, 0, 8, 0);
        ingest(
            &s,
            &fmt(2, 1),
            spa_data::MEM_FD,
            Some(&px),
            0,
            8,
            8,
            CHUNK_CORRUPTED,
        );
        assert_eq!(s.phase(), Phase::Starting);
        assert_eq!(s.mailbox.take_timeout(Duration::ZERO), Taken::Empty);
    }

    #[test]
    fn dmabuf_fails_the_session_and_closes_the_mailbox() {
        let s = starting();
        ingest(&s, &fmt(2, 1), spa_data::DMA_BUF, None, 0, 8, 8, 0);
        assert!(matches!(s.phase(), Phase::Failed(_)));
        assert_eq!(s.mailbox.take_timeout(Duration::ZERO), Taken::Closed);
    }

    #[test]
    fn row_padding_offset_and_zero_stride() {
        let s = starting();
        // Offset 4, tight rows with stride 0 (= width * 4).
        let mut data = vec![9u8; 4];
        data.extend_from_slice(&[1, 2, 3, 0, 4, 5, 6, 0]);
        ingest(&s, &fmt(2, 1), spa_data::MEM_PTR, Some(&data), 4, 8, 0, 0);
        match s.mailbox.take_timeout(Duration::ZERO) {
            Taken::Item(f) => assert_eq!(f.bgra[0], 1),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn size_changes_are_followed() {
        let s = starting();
        let a = vec![0u8; 8];
        ingest(&s, &fmt(2, 1), spa_data::MEM_FD, Some(&a), 0, 8, 8, 0);
        let b = vec![0u8; 16];
        ingest(&s, &fmt(2, 2), spa_data::MEM_FD, Some(&b), 0, 16, 8, 0);
        assert_eq!(s.size(), Some((2, 2)));
        match s.mailbox.take_timeout(Duration::ZERO) {
            Taken::Item(f) => assert_eq!((f.width, f.height), (2, 2)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_buffer_shorter_than_the_format_fails_cleanly() {
        let s = starting();
        let a = vec![0u8; 4];
        ingest(&s, &fmt(2, 2), spa_data::MEM_FD, Some(&a), 0, 4, 8, 0);
        assert!(matches!(s.phase(), Phase::Failed(m) if m.contains("bad buffer")));
    }

    #[test]
    fn session_closed_by_the_compositor_ends_the_stream() {
        let s = starting();
        let px = vec![0u8; 8];
        ingest(&s, &fmt(2, 1), spa_data::MEM_FD, Some(&px), 0, 8, 8, 0);
        s.event(Event::SystemStopped);
        // The last picture is still delivered, then the mailbox reports the end.
        assert!(matches!(
            s.mailbox.take_timeout(Duration::ZERO),
            Taken::Item(_)
        ));
        assert_eq!(s.mailbox.take_timeout(Duration::ZERO), Taken::Closed);
    }
}
