// SPDX-License-Identifier: GPL-3.0-or-later
//! Cheap checks that run before rendering. They catch the common memory bombs early with a
//! clear error. **They never replace the process sandbox**: a hostile file can hide a bomb
//! from them (inline images, streams that only hayro decodes, object streams).
//!
//! * [`raw_page_precheck`] works on the raw bytes before any parsing (parent side).
//! * [`scan`] walks the parsed objects (worker side).

use std::io::Read;

use hayro::hayro_syntax::object::Object;
use hayro::hayro_syntax::object::dict::keys;
use hayro::hayro_syntax::{Filter, Pdf};

use crate::document::Limits;
use crate::error::DocError;

/// How much of the scan to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanLevel {
    /// Object count, filter chains and image sizes only.
    Structure,
    /// Also inflate every `FlateDecode` stream (bounded) to find decompression bombs.
    Full,
}

/// Count `/Count N` and `/Type /Page` (not `/Pages`) in the raw bytes. A file that claims more
/// pages than `max_pages` is refused before a parser sees it (a parse of a million tiny pages
/// took 12 s and 900 MB). Page objects hidden in compressed object streams are not seen here;
/// the worker's own check and the process limits cover those.
pub(crate) fn raw_page_precheck(bytes: &[u8], max_pages: usize) -> Result<(), DocError> {
    let mut page_objects = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        // Jump to the next '/'.
        let Some(off) = bytes[i..].iter().position(|&b| b == b'/') else {
            break;
        };
        i += off + 1;
        let rest = &bytes[i..];
        if let Some(num) = rest.strip_prefix(b"Count") {
            if let Some(n) = parse_number(num).filter(|n| *n > max_pages as u64) {
                return Err(DocError::TooManyPages {
                    count: usize::try_from(n).unwrap_or(usize::MAX),
                    max: max_pages,
                });
            }
        } else if let Some(after) = rest.strip_prefix(b"Type") {
            let after = skip_ws(after);
            if let Some(tail) = after.strip_prefix(b"/Page")
                && !tail.starts_with(b"s")
            {
                page_objects += 1;
                if page_objects > max_pages {
                    return Err(DocError::TooManyPages {
                        count: page_objects,
                        max: max_pages,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Refuse a file whose objects nest `<<` / `[` deeper than `max_depth` outside stream bodies.
/// hayro needs seconds (debug build: 12 s for 100 000 levels) to walk such an object, and
/// the object walk of [`scan`] would pay that for every file. This is a small lexer: it skips
/// comments, strings, hex strings and stream bodies. Objects inside compressed object streams
/// are not seen here; the worker's time limit covers those.
pub(crate) fn raw_nesting_precheck(bytes: &[u8], max_depth: usize) -> Result<(), DocError> {
    let too_deep = || DocError::Rejected("objects are nested too deeply");
    let n = bytes.len();
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < n {
        match bytes[i] {
            b'%' => {
                while i < n && bytes[i] != b'\n' && bytes[i] != b'\r' {
                    i += 1;
                }
            }
            b'(' => {
                let mut parens = 1usize;
                i += 1;
                while i < n && parens > 0 {
                    match bytes[i] {
                        b'\\' => i += 1,
                        b'(' => parens += 1,
                        b')' => parens -= 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
            b'<' if bytes.get(i + 1) == Some(&b'<') => {
                depth += 1;
                if depth > max_depth {
                    return Err(too_deep());
                }
                i += 2;
            }
            b'<' => {
                while i < n && bytes[i] != b'>' {
                    i += 1;
                }
                i += 1;
            }
            b'>' if bytes.get(i + 1) == Some(&b'>') => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            b'[' => {
                depth += 1;
                if depth > max_depth {
                    return Err(too_deep());
                }
                i += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b's' if bytes[i..].starts_with(b"stream")
                && (i == 0 || !bytes[i - 1].is_ascii_alphabetic())
                && matches!(bytes.get(i + 6), Some(b'\r' | b'\n')) =>
            {
                // Skip the body up to "endstream".
                i += 6;
                loop {
                    let Some(off) = bytes[i..].iter().position(|&b| b == b'e') else {
                        return Ok(());
                    };
                    i += off;
                    if bytes[i..].starts_with(b"endstream") {
                        i += 9;
                        break;
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    Ok(())
}

fn skip_ws(mut b: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = b {
        if matches!(first, b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c) {
            b = rest;
        } else {
            break;
        }
    }
    b
}

fn parse_number(b: &[u8]) -> Option<u64> {
    let b = skip_ws(b);
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    // More than 18 digits is certainly over any limit.
    if digits > 18 {
        return Some(u64::MAX);
    }
    std::str::from_utf8(&b[..digits]).ok()?.parse().ok()
}

/// Bytes a zlib stream inflates to, counted up to `cap + 1`. A damaged stream counts what it
/// produced before the error.
fn inflated_len(data: &[u8], cap: u64) -> u64 {
    let mut dec = flate2::read::ZlibDecoder::new(data);
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    while total <= cap {
        match dec.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n as u64,
        }
    }
    total
}

/// Walk the objects of a parsed document and refuse the known bomb shapes.
pub(crate) fn scan(pdf: &Pdf, limits: &Limits, level: ScanLevel) -> Result<(), DocError> {
    if pdf.len() > limits.max_objects {
        return Err(DocError::Rejected("too many objects"));
    }
    let mut total_inflated = 0u64;
    for obj in pdf.objects() {
        let Object::Stream(s) = obj else { continue };
        let dict = s.dict();
        let dim = |a, b| {
            dict.get::<u32>(a)
                .or_else(|| dict.get::<u32>(b))
                .map_or(0, u64::from)
        };
        let (w, h) = (dim(keys::WIDTH, keys::W), dim(keys::HEIGHT, keys::H));
        if w.saturating_mul(h) > limits.max_image_pixels {
            return Err(DocError::Rejected("image is too large"));
        }
        let filters = s.filters();
        if filters.len() > limits.max_filter_chain {
            return Err(DocError::Rejected("filter chain is too long"));
        }
        if level == ScanLevel::Full
            && limits.inflate_scan
            && filters.first() == Some(&Filter::FlateDecode)
        {
            let raw = s.raw_data();
            let left = limits
                .max_total_inflate_bytes
                .saturating_sub(total_inflated);
            let cap = limits.max_stream_inflate_bytes.min(left);
            let n = inflated_len(&raw, cap);
            if n > limits.max_stream_inflate_bytes {
                return Err(DocError::Rejected("a stream inflates to too much data"));
            }
            total_inflated += n;
            if total_inflated > limits.max_total_inflate_bytes {
                return Err(DocError::Rejected("the streams inflate to too much data"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precheck_counts_pages_and_ignores_pages_node() {
        let few = b"<< /Type /Pages /Kids [3 0 R] /Count 2 >> << /Type /Page >> << /Type/Page >>";
        assert!(raw_page_precheck(few, 5).is_ok());
        assert!(matches!(
            raw_page_precheck(few, 1),
            Err(DocError::TooManyPages { .. })
        ));
        let big = b"<< /Type /Pages /Count 99999999999999999999999 >>";
        assert!(matches!(
            raw_page_precheck(big, 5000),
            Err(DocError::TooManyPages { .. })
        ));
        assert!(raw_page_precheck(b"/Count", 1).is_ok());
        assert!(raw_page_precheck(b"/", 1).is_ok());
    }

    #[test]
    fn nesting_precheck_counts_depth_outside_streams_and_strings() {
        let deep = format!("<< /A {}1{} >>", "<< /A ".repeat(300), " >>".repeat(300));
        assert!(raw_nesting_precheck(deep.as_bytes(), 256).is_err());
        assert!(raw_nesting_precheck(deep.as_bytes(), 1000).is_ok());
        // Brackets inside a stream body, a string, a hex string and a comment do not count.
        let calm = format!(
            "<< /A ({}) /B <3c3c> >>\n%{}\nstream\n{}\nendstream\n",
            "[".repeat(1000),
            "[".repeat(1000),
            "[".repeat(1000)
        );
        assert!(raw_nesting_precheck(calm.as_bytes(), 8).is_ok());
        assert!(raw_nesting_precheck(b"", 8).is_ok());
        assert!(raw_nesting_precheck(b"stream", 8).is_ok());
    }
}
