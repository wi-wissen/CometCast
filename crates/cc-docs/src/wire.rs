// SPDX-License-Identifier: GPL-3.0-or-later
//! The byte format between the parent and the render worker process.
//!
//! Request (parent to worker stdin), little endian:
//! `"CCDW" ver:u8 op:u8 index:u64 width:u32 height:u32 <limits: 13 x u64> pdf_len:u64 pdf...`
//!
//! Reply (worker stdout): a fixed 40 byte head
//! `"CCDR" ver:u8 kind:u8 code:u16 peak_kib:u64 x:u64 y:u64 z:u64` and then a payload whose
//! length follows from the head (`kind` 0: `x` pages as `f32,f32`; `kind` 1: `x*y*4` RGBA
//! bytes; `kind` 2: error `code` with parameters, no payload). The parent never trusts the
//! worker: sizes are checked against what it asked for before anything is allocated.

use crate::document::Limits;
use crate::error::DocError;

pub(crate) const REQ_MAGIC: &[u8; 4] = b"CCDW";
pub(crate) const REPLY_MAGIC: &[u8; 4] = b"CCDR";
pub(crate) const VERSION: u8 = 2;
pub(crate) const OP_INFO: u8 = 1;
pub(crate) const OP_RENDER: u8 = 2;
pub(crate) const REPLY_HEAD: usize = 40;
const LIMIT_FIELDS: usize = 13;
pub(crate) const REQ_HEAD: usize = 4 + 1 + 1 + 8 + 4 + 4 + LIMIT_FIELDS * 8 + 8;

pub(crate) const KIND_INFO: u8 = 0;
pub(crate) const KIND_PAGE: u8 = 1;
pub(crate) const KIND_ERROR: u8 = 2;

/// A decoded request head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Request {
    /// CPU seconds the worker may use (`RLIMIT_CPU`).
    pub cpu_secs: u64,
    pub op: u8,
    pub index: u64,
    pub width: u32,
    pub height: u32,
    pub limits: Limits,
    pub pdf_len: u64,
}

pub(crate) fn encode_request(
    op: u8,
    index: u64,
    width: u32,
    height: u32,
    limits: &Limits,
    cpu_secs: u64,
    pdf_len: u64,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(REQ_HEAD);
    v.extend_from_slice(REQ_MAGIC);
    v.push(VERSION);
    v.push(op);
    v.extend_from_slice(&index.to_le_bytes());
    v.extend_from_slice(&width.to_le_bytes());
    v.extend_from_slice(&height.to_le_bytes());
    for f in [
        limits.max_file_bytes as u64,
        limits.max_pages as u64,
        u64::from(limits.max_output_side),
        limits.max_output_pixels,
        limits.max_image_pixels,
        limits.max_stream_inflate_bytes,
        limits.max_total_inflate_bytes,
        limits.max_objects as u64,
        limits.max_filter_chain as u64,
        u64::from(limits.inflate_scan),
        limits.max_worker_memory_bytes,
        cpu_secs,
        u64::from(limits.require_os_sandbox),
    ] {
        v.extend_from_slice(&f.to_le_bytes());
    }
    v.extend_from_slice(&pdf_len.to_le_bytes());
    debug_assert_eq!(v.len(), REQ_HEAD);
    v
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(a)
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[at..at + 4]);
    u32::from_le_bytes(a)
}

/// Decode a request head. Values that do not fit are clamped; the worker re-checks them.
pub(crate) fn decode_request(b: &[u8; REQ_HEAD]) -> Result<Request, DocError> {
    if &b[0..4] != REQ_MAGIC || b[4] != VERSION || !matches!(b[5], OP_INFO | OP_RENDER) {
        return Err(DocError::Internal("bad request"));
    }
    let l = 22; // offset after magic, version, op, index, width, height
    let f = |i: usize| u64_at(b, l + i * 8);
    let us = |v: u64| usize::try_from(v).unwrap_or(usize::MAX);
    let limits = Limits {
        max_file_bytes: us(f(0)),
        max_pages: us(f(1)),
        max_output_side: u32::try_from(f(2)).unwrap_or(u32::MAX),
        max_output_pixels: f(3),
        max_image_pixels: f(4),
        max_stream_inflate_bytes: f(5),
        max_total_inflate_bytes: f(6),
        max_objects: us(f(7)),
        max_filter_chain: us(f(8)),
        inflate_scan: f(9) != 0,
        max_worker_memory_bytes: f(10),
        require_os_sandbox: f(12) != 0,
        // The timeouts are enforced by the parent; the worker only gets `cpu_secs`.
        ..Limits::default()
    };
    Ok(Request {
        cpu_secs: f(11),
        op: b[5],
        index: u64_at(b, 6),
        width: u32_at(b, 14),
        height: u32_at(b, 18),
        limits,
        pdf_len: u64_at(b, l + LIMIT_FIELDS * 8),
    })
}

/// A decoded reply head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReplyHead {
    pub kind: u8,
    pub code: u16,
    pub peak_kib: u64,
    pub x: u64,
    pub y: u64,
    pub z: u64,
}

pub(crate) fn encode_head(h: &ReplyHead) -> [u8; REPLY_HEAD] {
    let mut v = [0u8; REPLY_HEAD];
    v[0..4].copy_from_slice(REPLY_MAGIC);
    v[4] = VERSION;
    v[5] = h.kind;
    v[6..8].copy_from_slice(&h.code.to_le_bytes());
    v[8..16].copy_from_slice(&h.peak_kib.to_le_bytes());
    v[16..24].copy_from_slice(&h.x.to_le_bytes());
    v[24..32].copy_from_slice(&h.y.to_le_bytes());
    v[32..40].copy_from_slice(&h.z.to_le_bytes());
    v
}

pub(crate) fn decode_head(b: &[u8; REPLY_HEAD]) -> Option<ReplyHead> {
    if &b[0..4] != REPLY_MAGIC || b[4] != VERSION || b[5] > KIND_ERROR {
        return None;
    }
    Some(ReplyHead {
        kind: b[5],
        code: u16::from_le_bytes([b[6], b[7]]),
        peak_kib: u64_at(b, 8),
        x: u64_at(b, 16),
        y: u64_at(b, 24),
        z: u64_at(b, 32),
    })
}

const REJECT_REASONS: [&str; 5] = [
    "image is too large",
    "a stream inflates to too much data",
    "the streams inflate to too much data",
    "too many objects",
    "filter chain is too long",
];

/// Reasons the worker gives when it cannot restrict itself (the index travels as `x`).
const WORKER_FAILED: [&str; 6] = [
    "the renderer could not restrict itself",
    "core dumps could not be disabled",
    "the file size limit could not be applied",
    "the open files limit could not be applied",
    "the CPU time limit could not be applied",
    "the memory limit could not be applied",
];

/// Error to `(code, x, y)`.
pub(crate) fn error_to_wire(e: &DocError) -> (u16, u64, u64) {
    match e {
        DocError::Invalid => (1, 0, 0),
        DocError::PasswordProtected => (2, 0, 0),
        DocError::UnsupportedEncryption("missing ID entry") => (3, 0, 0),
        DocError::UnsupportedEncryption("invalid encryption dictionary") => (4, 0, 0),
        DocError::UnsupportedEncryption(_) => (5, 0, 0),
        DocError::NoPages => (6, 0, 0),
        DocError::TooManyPages { count, max } => (7, *count as u64, *max as u64),
        DocError::PageOutOfRange { index, count } => (8, *index as u64, *count as u64),
        DocError::InvalidPageSize => (9, 0, 0),
        DocError::Panicked => (10, 0, 0),
        DocError::Rejected(r) => {
            let i = REJECT_REASONS.iter().position(|x| x == r).unwrap_or(0);
            (11, i as u64, 0)
        }
        DocError::TooLarge { size, max } => (12, *size as u64, *max as u64),
        DocError::NotPdf => (14, 0, 0),
        DocError::Empty => (15, 0, 0),
        DocError::OutputTooLarge { width, height } => (16, *width, *height),
        DocError::SandboxUnavailable(_) => (17, 0, 0),
        DocError::WorkerFailed(w) => {
            let i = WORKER_FAILED.iter().position(|x| x == w).unwrap_or(0);
            (18, i as u64, 0)
        }
        _ => (13, 0, 0),
    }
}

/// `(code, x, y)` from the worker to an error.
pub(crate) fn error_from_wire(code: u16, x: u64, y: u64) -> DocError {
    let us = |v: u64| usize::try_from(v).unwrap_or(usize::MAX);
    match code {
        1 => DocError::Invalid,
        2 => DocError::PasswordProtected,
        3 => DocError::UnsupportedEncryption("missing ID entry"),
        4 => DocError::UnsupportedEncryption("invalid encryption dictionary"),
        5 => DocError::UnsupportedEncryption("unsupported algorithm"),
        6 => DocError::NoPages,
        7 => DocError::TooManyPages {
            count: us(x),
            max: us(y),
        },
        8 => DocError::PageOutOfRange {
            index: us(x),
            count: us(y),
        },
        9 => DocError::InvalidPageSize,
        10 => DocError::Panicked,
        11 => DocError::Rejected(
            REJECT_REASONS
                .get(us(x))
                .copied()
                .unwrap_or("refused by the safety scan"),
        ),
        12 => DocError::TooLarge {
            size: us(x),
            max: us(y),
        },
        14 => DocError::NotPdf,
        15 => DocError::Empty,
        16 => DocError::OutputTooLarge {
            width: x,
            height: y,
        },
        17 => {
            DocError::SandboxUnavailable("Landlock and seccomp are required but not both available")
        }
        18 => DocError::WorkerFailed(
            WORKER_FAILED
                .get(us(x))
                .copied()
                .unwrap_or("the renderer could not restrict itself"),
        ),
        _ => DocError::Internal("render worker reported an internal error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips() {
        let l = Limits {
            max_pages: 77,
            max_image_pixels: 12345,
            ..Limits::default()
        };
        let b = encode_request(OP_RENDER, 5, 640, 480, &l, 12, 999);
        let arr: [u8; REQ_HEAD] = b.try_into().unwrap();
        let r = decode_request(&arr).unwrap();
        assert_eq!(
            (r.op, r.index, r.width, r.height, r.pdf_len),
            (OP_RENDER, 5, 640, 480, 999)
        );
        assert_eq!(r.limits.max_pages, 77);
        assert_eq!(r.limits.max_image_pixels, 12345);
        assert_eq!(r.limits.max_objects, l.max_objects);
        assert_eq!(r.cpu_secs, 12);
        assert_eq!(r.limits.max_worker_memory_bytes, l.max_worker_memory_bytes);
        assert!(!r.limits.require_os_sandbox);
        let mut bad = arr;
        bad[0] = b'X';
        assert!(decode_request(&bad).is_err());
    }

    #[test]
    fn head_and_errors_round_trip() {
        let h = ReplyHead {
            kind: KIND_PAGE,
            code: 0,
            peak_kib: 7,
            x: 1,
            y: 2,
            z: 3,
        };
        assert_eq!(decode_head(&encode_head(&h)), Some(h));
        let mut bad = encode_head(&h);
        bad[5] = 9;
        assert_eq!(decode_head(&bad), None);
        for e in [
            DocError::Invalid,
            DocError::TooManyPages { count: 9, max: 3 },
            DocError::Rejected("too many objects"),
            DocError::Panicked,
            DocError::PageOutOfRange { index: 4, count: 2 },
        ] {
            let (c, x, y) = error_to_wire(&e);
            assert_eq!(error_from_wire(c, x, y), e);
        }
    }
}
