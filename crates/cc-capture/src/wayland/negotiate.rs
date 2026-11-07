// SPDX-License-Identifier: GPL-3.0-or-later
//! The decisions of the Wayland backend that do not need the portal or PipeWire: what to ask
//! the portal for, which pixel formats and buffer types to offer, which buffers to use, and
//! how big the picture is. Kept free of D-Bus and PipeWire types so it is tested with fake
//! values on every system.

use crate::CaptureError;
use crate::CaptureSource;
use crate::convert::{PixelFormat, check_size};

/// `AvailableCursorModes` / `cursor_mode` bits of the ScreenCast portal.
pub mod cursor_bits {
    /// The cursor is not part of the stream.
    pub const HIDDEN: u32 = 1;
    /// The cursor is drawn into the pictures.
    pub const EMBEDDED: u32 = 2;
    /// The cursor comes as metadata (not used here).
    pub const METADATA: u32 = 4;
}

/// `AvailableSourceTypes` / `types` bits of the ScreenCast portal.
pub mod source_bits {
    /// A whole monitor.
    pub const MONITOR: u32 = 1;
    /// One window.
    pub const WINDOW: u32 = 2;
    /// A virtual monitor (never requested here).
    pub const VIRTUAL: u32 = 4;
}

/// The cursor mode to request. `Some(EMBEDDED)` when the portal offers it: the pointer is part
/// of the picture, which is what the viewers need until the pointer is sent as its own message
/// (docs/measurement-points.md, A10). `Some(HIDDEN)` when only that is offered. `None` when the
/// portal did not report modes (interface version 1): the portal's default applies.
pub fn choose_cursor_mode(available: u32) -> Option<u32> {
    if available == 0 {
        None
    } else if available & cursor_bits::EMBEDDED != 0 {
        Some(cursor_bits::EMBEDDED)
    } else if available & cursor_bits::HIDDEN != 0 {
        Some(cursor_bits::HIDDEN)
    } else {
        // Only metadata mode: the pointer would be missing; hidden is not offered, so ask for
        // nothing and let the portal decide.
        None
    }
}

/// The source types to ask for: monitors and windows, as far as the portal offers them.
/// `available == 0` (property missing) asks for both. Nothing usable is an error.
pub fn choose_source_types(available: u32) -> Result<u32, CaptureError> {
    let wanted = source_bits::MONITOR | source_bits::WINDOW;
    if available == 0 {
        return Ok(wanted);
    }
    match available & wanted {
        0 => Err(CaptureError::NoPortal(
            "the desktop portal offers neither monitors nor windows",
        )),
        t => Ok(t),
    }
}

/// Pixel formats offered to PipeWire, best first. The two BGR layouts need no per-pixel work.
pub const OFFERED_FORMATS: [PixelFormat; 6] = [
    PixelFormat::Bgrx,
    PixelFormat::Bgra,
    PixelFormat::Rgbx,
    PixelFormat::Rgba,
    PixelFormat::Bgr,
    PixelFormat::Rgb,
];

/// SPA data types (`spa_data_type`).
pub mod spa_data {
    /// Memory in the process (`SPA_DATA_MemPtr`).
    pub const MEM_PTR: u32 = 1;
    /// Shared memory by file descriptor (`SPA_DATA_MemFd`).
    pub const MEM_FD: u32 = 2;
    /// A DMA-BUF (`SPA_DATA_DmaBuf`).
    pub const DMA_BUF: u32 = 3;
}

/// Bit mask for the `SPA_PARAM_BUFFERS_dataType` property of the buffer parameter: which data
/// types this client can read. DMA-BUF import (EGL/Vulkan) is not implemented, so it is never
/// offered and compositors fall back to shared memory; with `dmabuf_import` true (a later
/// implementation) DMA-BUF is added.
pub fn buffer_type_mask(dmabuf_import: bool) -> u32 {
    let mut m = (1 << spa_data::MEM_PTR) | (1 << spa_data::MEM_FD);
    if dmabuf_import {
        m |= 1 << spa_data::DMA_BUF;
    }
    m
}

/// `SPA_CHUNK_FLAG_CORRUPTED`.
pub const CHUNK_CORRUPTED: i32 = 1;

/// What to do with one dequeued buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferVerdict {
    /// Read the picture from it.
    Use,
    /// No new picture: the compositor sent an empty buffer (for example only the pointer
    /// moved while it is not embedded, or nothing was damaged).
    SkipEmpty,
    /// The producer marked the data corrupted; the previous picture stays.
    SkipCorrupted,
    /// The buffer is a DMA-BUF although only shared memory was offered: the compositor
    /// ignored the negotiation. Fatal for the stream (no way to read it without EGL).
    UnsupportedDmaBuf,
    /// The data cannot be read at all (no mapped pointer).
    Unreadable,
}

/// Decide about a buffer from what PipeWire says about it.
pub fn judge_buffer(
    data_type: u32,
    mapped: bool,
    chunk_size: u32,
    chunk_flags: i32,
) -> BufferVerdict {
    if data_type == spa_data::DMA_BUF {
        return BufferVerdict::UnsupportedDmaBuf;
    }
    if chunk_flags & CHUNK_CORRUPTED != 0 {
        return BufferVerdict::SkipCorrupted;
    }
    if chunk_size == 0 {
        return BufferVerdict::SkipEmpty;
    }
    if !mapped {
        return BufferVerdict::Unreadable;
    }
    BufferVerdict::Use
}

/// What the portal says about one stream (one picked monitor or window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    /// PipeWire node id to connect to.
    pub node_id: u32,
    /// Position in the compositor's logical coordinate space, if given.
    pub position: Option<(i32, i32)>,
    /// Size in logical coordinates, if given (not pixels when fractional scaling is on).
    pub logical_size: Option<(i32, i32)>,
    /// True for a window, false for a monitor or unknown.
    pub is_window: bool,
}

/// The stream to capture. Only one was asked for (`multiple = false`); a compositor that
/// returns several anyway gets its first one used.
pub fn pick_stream(streams: &[StreamInfo]) -> Option<&StreamInfo> {
    streams.first()
}

/// The size of the picture in pixels. The size in the PipeWire format is the truth (it is in
/// pixels, also with HiDPI and fractional scaling, where the portal's logical size is smaller
/// than the buffer). The portal size is only a placeholder until the format is known.
pub fn choose_dims(logical: Option<(i32, i32)>, format: Option<(u32, u32)>) -> Option<(u32, u32)> {
    if let Some((w, h)) = format
        && check_size(w, h).is_ok()
    {
        return Some((w, h));
    }
    let (w, h) = logical?;
    let (w, h) = (u32::try_from(w).ok()?, u32::try_from(h).ok()?);
    check_size(w, h).ok()?;
    Some((w, h))
}

/// The `CaptureSource` for a picked stream: id from the PipeWire node, position and size as
/// far as known (size 0x0 means "not known yet").
pub fn describe_source(info: &StreamInfo, pixels: Option<(u32, u32)>) -> CaptureSource {
    let (width, height) = choose_dims(info.logical_size, pixels).unwrap_or((0, 0));
    let (x, y) = info.position.unwrap_or((0, 0));
    CaptureSource {
        id: format!("wayland-node-{}", info.node_id),
        name: if info.is_window { "window" } else { "screen" }.into(),
        x,
        y,
        width,
        height,
    }
}

/// How the pixel size relates to the logical size, for logs: 1.0 = no scaling, 1.5 = 150%.
/// `None` when either is unknown.
pub fn scale_factor(logical: Option<(i32, i32)>, pixels: Option<(u32, u32)>) -> Option<f64> {
    let (lw, _) = logical?;
    let (pw, _) = pixels?;
    (lw > 0).then(|| f64::from(pw) / f64::from(lw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_mode_prefers_embedded() {
        assert_eq!(choose_cursor_mode(0), None);
        assert_eq!(choose_cursor_mode(1 | 2 | 4), Some(2));
        assert_eq!(choose_cursor_mode(1 | 4), Some(1));
        assert_eq!(choose_cursor_mode(4), None);
    }

    #[test]
    fn source_types_are_monitors_and_windows_only() {
        assert_eq!(choose_source_types(0).unwrap(), 3);
        assert_eq!(choose_source_types(7).unwrap(), 3);
        assert_eq!(choose_source_types(1).unwrap(), 1);
        assert_eq!(choose_source_types(2).unwrap(), 2);
        assert!(choose_source_types(4).is_err());
    }

    #[test]
    fn dmabuf_is_not_offered_until_it_can_be_read() {
        let m = buffer_type_mask(false);
        assert_eq!(m & (1 << spa_data::DMA_BUF), 0);
        assert_ne!(m & (1 << spa_data::MEM_FD), 0);
        assert_ne!(m & (1 << spa_data::MEM_PTR), 0);
        assert_ne!(buffer_type_mask(true) & (1 << spa_data::DMA_BUF), 0);
    }

    #[test]
    fn buffer_verdicts() {
        use BufferVerdict::*;
        assert_eq!(judge_buffer(spa_data::MEM_FD, true, 100, 0), Use);
        assert_eq!(judge_buffer(spa_data::MEM_PTR, true, 100, 0), Use);
        assert_eq!(judge_buffer(spa_data::MEM_FD, true, 0, 0), SkipEmpty);
        assert_eq!(
            judge_buffer(spa_data::MEM_FD, true, 100, CHUNK_CORRUPTED),
            SkipCorrupted
        );
        assert_eq!(
            judge_buffer(spa_data::DMA_BUF, false, 100, 0),
            UnsupportedDmaBuf
        );
        assert_eq!(judge_buffer(spa_data::MEM_FD, false, 100, 0), Unreadable);
    }

    #[test]
    fn pixel_size_beats_logical_size() {
        // 150% fractional scaling: 1280x720 logical, 1920x1080 pixels.
        assert_eq!(
            choose_dims(Some((1280, 720)), Some((1920, 1080))),
            Some((1920, 1080))
        );
        assert_eq!(choose_dims(Some((1280, 720)), None), Some((1280, 720)));
        assert_eq!(choose_dims(None, None), None);
        assert_eq!(choose_dims(Some((-1, 5)), None), None);
        // A bad format size falls back to the portal size.
        assert_eq!(
            choose_dims(Some((800, 600)), Some((0, 0))),
            Some((800, 600))
        );
        assert_eq!(
            scale_factor(Some((1280, 720)), Some((1920, 1080))),
            Some(1.5)
        );
        assert_eq!(scale_factor(None, Some((1, 1))), None);
    }

    #[test]
    fn first_stream_wins_and_is_described() {
        let a = StreamInfo {
            node_id: 42,
            position: Some((1920, 0)),
            logical_size: Some((1920, 1080)),
            is_window: false,
        };
        let b = StreamInfo {
            node_id: 43,
            position: None,
            logical_size: None,
            is_window: true,
        };
        let both = [a.clone(), b];
        assert_eq!(pick_stream(&both), Some(&a));
        assert_eq!(pick_stream(&[]), None);
        let s = describe_source(&a, Some((3840, 2160)));
        assert_eq!(s.id, "wayland-node-42");
        assert_eq!((s.x, s.y, s.width, s.height), (1920, 0, 3840, 2160));
        let w = describe_source(
            &StreamInfo {
                node_id: 7,
                position: None,
                logical_size: None,
                is_window: true,
            },
            None,
        );
        assert_eq!((w.width, w.height), (0, 0));
        assert_eq!(w.name, "window");
    }
}
