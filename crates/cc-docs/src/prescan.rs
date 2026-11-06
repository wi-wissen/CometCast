// SPDX-License-Identifier: GPL-3.0-or-later
//! Cheap checks that run before rendering. They catch the common memory bombs early with a
//! clear error. **They never replace the process limits**: a hostile file can hide a bomb
//! from them (streams that only hayro decodes, object streams, inline images deep inside a
//! very long content stream).
//!
//! * [`raw_page_precheck`] and [`raw_nesting_precheck`] work on the raw bytes before any
//!   parsing (parent side).
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

/// How far back from a `/Type /Page` token the object header (`N G obj`) is looked for.
const OBJ_HEADER_WINDOW: usize = 512;

/// Count the distinct objects that say `/Type /Page` (not `/Pages`) in the raw bytes. A file
/// with more than `max_pages` of them is refused before a parser sees it (a parse of a million
/// tiny pages took 12 s and 900 MB).
///
/// * The same object number counted once: an incremental update repeats the page objects it
///   changes, and a file with 3 revisions of 2 000 pages is still 2 000 pages. A token whose
///   object header is not found within 512 bytes counts on its own.
/// * `/Count` is **not** used: outlines carry `/Count` too and legitimately exceed the page
///   limit, and the number lies anyway. The worker checks the real page count after parsing,
///   inside the process that can be killed.
/// * Page objects hidden in compressed object streams are not seen here; the worker's own
///   check and the process limits cover those.
pub(crate) fn raw_page_precheck(bytes: &[u8], max_pages: usize) -> Result<(), DocError> {
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut anonymous = 0usize;
    // Where the backward search for "N G obj" has already been, and the object it found last.
    let mut scanned_to = 0usize;
    let mut current: Option<u64> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        // Jump to the next '/'.
        let Some(off) = bytes[i..].iter().position(|&b| b == b'/') else {
            break;
        };
        let at = i + off;
        i = at + 1;
        let Some(after) = bytes[i..].strip_prefix(b"Type") else {
            continue;
        };
        let after = skip_ws(after);
        let Some(tail) = after.strip_prefix(b"/Page") else {
            continue;
        };
        if tail.starts_with(b"s") {
            continue;
        }
        // Which object is this in? Search only the bytes not searched for the previous token.
        let from = at.saturating_sub(OBJ_HEADER_WINDOW);
        if from > scanned_to {
            current = None;
        }
        let region_start = from.max(scanned_to);
        if let Some(found) = last_object_header(&bytes[region_start..at]) {
            current = found;
        }
        scanned_to = at;
        match current {
            Some(n) => {
                seen.insert(n);
            }
            None => anonymous += 1,
        }
        let count = seen.len() + anonymous;
        if count > max_pages {
            return Err(DocError::TooManyPages {
                count,
                max: max_pages,
            });
        }
    }
    Ok(())
}

/// The last `obj` keyword in `region`: `Some(Some(n))` for `n g obj`, `Some(None)` for an
/// `endobj` or a header that cannot be read, `None` when there is no such keyword.
fn last_object_header(region: &[u8]) -> Option<Option<u64>> {
    let p = region.windows(3).rposition(|w| w == b"obj")?;
    // "endobj" ends an object: the token is then outside any object we know.
    if p == 0 || !matches!(region[p - 1], b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c) {
        return Some(None);
    }
    let mut j = p;
    let skip_back_ws = |j: &mut usize| {
        while *j > 0 && matches!(region[*j - 1], b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c) {
            *j -= 1;
        }
    };
    let digits_back = |j: &mut usize| {
        let end = *j;
        while *j > 0 && region[*j - 1].is_ascii_digit() {
            *j -= 1;
        }
        (end > *j).then_some((*j, end))
    };
    skip_back_ws(&mut j);
    let Some(_gen) = digits_back(&mut j) else {
        return Some(None);
    };
    skip_back_ws(&mut j);
    let Some((a, b)) = digits_back(&mut j) else {
        return Some(None);
    };
    if b - a > 18 {
        return Some(None);
    }
    Some(std::str::from_utf8(&region[a..b]).ok()?.parse().ok())
}

/// Refuse a file whose objects nest `<<` / `[` deeper than `max_depth` outside stream bodies.
/// hayro needs seconds (debug build: 12 s for 100 000 levels) to walk such an object, and
/// the object walk of [`scan`] would pay that for every file. This is a small lexer: it skips
/// comments, strings, hex strings and stream bodies.
///
/// A `stream` keyword only starts a body to skip when it comes **right after a dictionary
/// that was closed at depth 0** (only white space in between), as in a real stream object. A
/// `stream` token anywhere else (inside a nested object, in a plain object, after a number)
/// is an ordinary word and hides nothing. What remains: this lexer ends a body at the first
/// `endstream`, while hayro trusts `/Length` (and repairs); a file that makes the two
/// disagree can place nesting where this check does not look. That is not closed here:
/// hayro has its own nesting limit (`MAX_OBJECT_NESTING_DEPTH` 512) and the worker has a time
/// limit, a CPU limit and a 16 MiB stack. Objects inside compressed object streams are not
/// seen either.
pub(crate) fn raw_nesting_precheck(bytes: &[u8], max_depth: usize) -> Result<(), DocError> {
    let too_deep = || DocError::Rejected("objects are nested too deeply");
    let n = bytes.len();
    let mut depth = 0usize;
    // End of the last `>>` that brought the depth back to 0.
    let mut closed_at: Option<usize> = None;
    let is_ws = |b: &u8| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c);
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
                closed_at = (depth == 0).then_some(i);
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
            b's' if depth == 0
                && bytes[i..].starts_with(b"stream")
                && matches!(bytes.get(i + 6), Some(b'\r' | b'\n'))
                && closed_at.is_some_and(|e| bytes[e..i].iter().all(is_ws)) =>
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

/// Bytes a zlib stream inflates to, counted up to `cap + 1`. A damaged stream counts what it
/// produced before the error. `peek` sees every chunk that was produced.
fn inflated_len(data: &[u8], cap: u64, mut peek: impl FnMut(&[u8])) -> u64 {
    let mut dec = flate2::read::ZlibDecoder::new(data);
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    while total <= cap {
        match dec.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                peek(&buf[..n]);
                total += n as u64;
            }
        }
    }
    total
}

/// How much of one content stream (and of all together) is searched for inline images. A
/// hostile file can put an inline image after this; the process limits are the backstop.
const INLINE_SCAN_PER_STREAM: u64 = 8 * 1024 * 1024;
const INLINE_SCAN_TOTAL: u64 = 64 * 1024 * 1024;
/// Longest `BI ... ID` header looked at, and the overlap kept between chunks.
const INLINE_HEADER_MAX: usize = 1024;
const INLINE_CARRY: usize = 2 * INLINE_HEADER_MAX;

/// A rough finder of inline images (`BI /W n /H n ... ID`) in content stream bytes that arrive
/// in chunks. It only reads the declared width and height; the data is not decoded.
struct InlineScanner {
    carry: Vec<u8>,
    seen: u64,
    max_pixels: u64,
    too_big: bool,
}

impl InlineScanner {
    fn new(max_pixels: u64) -> Self {
        Self {
            carry: Vec::new(),
            seen: 0,
            max_pixels,
            too_big: false,
        }
    }

    fn feed(&mut self, chunk: &[u8], budget: &mut u64) {
        if self.too_big || self.seen >= INLINE_SCAN_PER_STREAM || *budget == 0 {
            return;
        }
        let take = chunk
            .len()
            .min(usize::try_from(INLINE_SCAN_PER_STREAM - self.seen).unwrap_or(usize::MAX))
            .min(usize::try_from(*budget).unwrap_or(usize::MAX));
        let chunk = &chunk[..take];
        self.seen += take as u64;
        *budget -= take as u64;
        let mut window = std::mem::take(&mut self.carry);
        window.extend_from_slice(chunk);
        self.too_big = inline_image_too_big(&window, self.max_pixels);
        let keep = window.len().saturating_sub(INLINE_CARRY);
        self.carry = window.split_off(keep);
    }
}

fn is_pdf_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0 | 0x0c)
}

/// Does `window` hold an inline image header that declares more than `max_pixels`?
fn inline_image_too_big(window: &[u8], max_pixels: u64) -> bool {
    let mut at = 0usize;
    while let Some(off) = window[at..].windows(2).position(|w| w == b"BI") {
        let p = at + off;
        at = p + 2;
        let before_ok = p == 0 || is_pdf_ws(window[p - 1]) || window[p - 1] == b'>';
        if !before_ok || !window.get(p + 2).is_some_and(|b| is_pdf_ws(*b)) {
            continue;
        }
        let header = &window[p + 2..window.len().min(p + 2 + INLINE_HEADER_MAX)];
        // The header ends at "ID" followed by white space; without it the header is cut.
        let end = header
            .windows(3)
            .position(|w| w[0] == b'I' && w[1] == b'D' && is_pdf_ws(w[2]))
            .unwrap_or(header.len());
        let header = &header[..end];
        let (w, h) = (
            inline_number(header, &[b"/W", b"/Width"]),
            inline_number(header, &[b"/H", b"/Height"]),
        );
        if let (Some(w), Some(h)) = (w, h)
            && w.saturating_mul(h) > max_pixels
        {
            return true;
        }
    }
    false
}

/// The integer after one of `keys` (a name, then white space) in an inline image header.
/// A value that is too long to be sane counts as `u64::MAX`.
fn inline_number(header: &[u8], keys: &[&[u8]]) -> Option<u64> {
    for key in keys {
        let mut from = 0usize;
        while let Some(off) = header[from..].windows(key.len()).position(|w| w == *key) {
            let p = from + off + key.len();
            from = p;
            // The key must end here: "/W" is not "/Width".
            if header
                .get(p)
                .is_some_and(|b| !is_pdf_ws(*b) && !b.is_ascii_digit())
            {
                continue;
            }
            let rest = skip_ws(&header[p..]);
            let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 {
                continue;
            }
            return Some(if digits > 18 {
                u64::MAX
            } else {
                std::str::from_utf8(&rest[..digits])
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(u64::MAX)
            });
        }
    }
    None
}

/// A declared dimension of an image stream: `None` when the entry is absent; a value that is
/// not a readable non-negative number (a 33-bit integer, a negative or a huge real) counts as
/// "too large", not as 0. `/W` and `/H` are only read when they are numbers (`/W` is an
/// array in cross-reference streams).
fn declared_dim(
    dict: &hayro::hayro_syntax::object::Dict<'_>,
    long: &[u8],
    short: &[u8],
) -> Option<u64> {
    let read = |key: &[u8]| -> Option<u64> {
        let v = dict.get::<f64>(key)?;
        (v.is_finite() && v >= 0.0).then_some(if v >= 1.8e19 { u64::MAX } else { v as u64 })
    };
    if dict.contains_key(long) {
        return Some(read(long).unwrap_or(u64::MAX));
    }
    read(short)
}

/// Rough number of bytes the decoded samples of an image take: area x colour components x bits
/// per component. Colour spaces that are not a plain device space count 4 components.
fn image_sample_bytes(dict: &hayro::hayro_syntax::object::Dict<'_>, w: u64, h: u64) -> u64 {
    let mask = dict.get::<bool>(keys::IMAGE_MASK).unwrap_or(false);
    let (components, bits) = if mask {
        (1u64, 1u64)
    } else {
        let comps = match dict.get::<hayro::hayro_syntax::object::Name<'_>>(keys::COLORSPACE) {
            Some(n) => match n.as_ref() {
                b"DeviceGray" | b"G" | b"CalGray" => 1,
                b"DeviceRGB" | b"RGB" | b"CalRGB" => 3,
                _ => 4,
            },
            None => 4,
        };
        // An unreadable depth counts as the largest legal one.
        let bits = dict
            .get::<u32>(keys::BITS_PER_COMPONENT)
            .or_else(|| dict.get::<u32>(keys::BPC))
            .map_or(8, u64::from)
            .clamp(1, 16);
        (comps, bits)
    };
    let total = u128::from(w) * u128::from(h) * u128::from(components) * u128::from(bits) / 8;
    u64::try_from(total).unwrap_or(u64::MAX)
}

/// Walk the objects of a parsed document and refuse the known bomb shapes.
pub(crate) fn scan(pdf: &Pdf, limits: &Limits, level: ScanLevel) -> Result<(), DocError> {
    if pdf.len() > limits.max_objects {
        return Err(DocError::Rejected("too many objects"));
    }
    let mut total_inflated = 0u64;
    let mut inline_budget = INLINE_SCAN_TOTAL;
    for obj in pdf.objects() {
        let Object::Stream(s) = obj else { continue };
        let dict = s.dict();
        let w = declared_dim(dict, b"Width", b"W");
        let h = declared_dim(dict, b"Height", b"H");
        let is_image = dict.contains_key(keys::WIDTH) || dict.contains_key(keys::HEIGHT);
        if let (Some(w), Some(h)) = (w, h) {
            if w.saturating_mul(h) > limits.max_image_pixels {
                return Err(DocError::Rejected("image is too large"));
            }
            // Area is not everything: 16 bit CMYK has eight times the bytes of 8 bit gray. The
            // byte budget is the one of an 8 bit, 4 component image of the allowed area (with
            // the default 64 Mpx: 256 MiB).
            if is_image
                && image_sample_bytes(dict, w, h) > limits.max_image_pixels.saturating_mul(4)
            {
                return Err(DocError::Rejected("image is too large"));
            }
        } else if is_image {
            // Only one of Width and Height: not a usable image, not a bomb either.
        }
        let filters = s.filters();
        if filters.len() > limits.max_filter_chain {
            return Err(DocError::Rejected("filter chain is too long"));
        }
        if level != ScanLevel::Full || !limits.inflate_scan {
            continue;
        }
        // Content streams and forms may hold inline images; look for them roughly.
        let mut inline = (!is_image).then(|| InlineScanner::new(limits.max_image_pixels));
        if filters.first() == Some(&Filter::FlateDecode) {
            let raw = s.raw_data();
            let left = limits
                .max_total_inflate_bytes
                .saturating_sub(total_inflated);
            let cap = limits.max_stream_inflate_bytes.min(left);
            let n = inflated_len(&raw, cap, |chunk| {
                if let Some(sc) = inline.as_mut() {
                    sc.feed(chunk, &mut inline_budget);
                }
            });
            if n > limits.max_stream_inflate_bytes {
                return Err(DocError::Rejected("a stream inflates to too much data"));
            }
            total_inflated += n;
            if total_inflated > limits.max_total_inflate_bytes {
                return Err(DocError::Rejected("the streams inflate to too much data"));
            }
        } else if filters.is_empty()
            && let Some(sc) = inline.as_mut()
        {
            sc.feed(&s.raw_data(), &mut inline_budget);
        }
        if inline.is_some_and(|sc| sc.too_big) {
            return Err(DocError::Rejected("image is too large"));
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
        // `/Count` alone is not a reason any more (outlines have one); the worker checks the
        // real number of pages after parsing.
        let big = b"<< /Type /Pages /Count 99999999999999999999999 >>";
        assert!(raw_page_precheck(big, 5000).is_ok());
        assert!(raw_page_precheck(b"/Count", 1).is_ok());
        assert!(raw_page_precheck(b"/", 1).is_ok());
    }

    /// An incremental update writes the page objects again: same numbers, counted once.
    #[test]
    fn precheck_counts_each_page_object_once_across_revisions() {
        let mut f = Vec::new();
        for revision in 0..3 {
            for n in 3..=7 {
                f.extend(
                    format!("{n} 0 obj\n<< /Type /Page /Parent 2 0 R /R {revision} >>\nendobj\n")
                        .bytes(),
                );
            }
            f.extend(b"xref\n0 1\ntrailer\n<< /Prev 1 >>\nstartxref\n9\n%%EOF\n");
        }
        // 5 page objects in 3 revisions: 15 tokens, 5 pages.
        assert!(raw_page_precheck(&f, 5).is_ok());
        assert!(matches!(
            raw_page_precheck(&f, 4),
            Err(DocError::TooManyPages { count: 5, max: 4 })
        ));
        // Tokens without an object header count one by one.
        let anon = b"<< /Type /Page >> << /Type /Page >> << /Type /Page >>";
        assert!(raw_page_precheck(anon, 3).is_ok());
        assert!(raw_page_precheck(anon, 2).is_err());
        // Many tokens in ONE object are cheap and count once.
        let mut one = b"9 0 obj\n<< ".to_vec();
        for _ in 0..50_000 {
            one.extend(b"/Type /Page ");
        }
        // (They stay attributed to that object; the point is also that this ends fast and does
        // not search backwards quadratically.)
        let t = std::time::Instant::now();
        assert!(raw_page_precheck(&one, 100).is_ok());
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn nesting_precheck_ignores_a_fake_stream_token() {
        // "stream" inside a nested dictionary, then 300 levels of nesting: must still be seen.
        let deep = format!("{}1{}", "<< /A ".repeat(300), " >>".repeat(300));
        let fake = format!("<< /X << /S stream\n >> >> {deep} endstream");
        assert!(raw_nesting_precheck(fake.as_bytes(), 256).is_err());
        // The same after a plain object and after a number.
        let fake2 = format!("5 0 obj stream\n{deep}\nendstream endobj");
        assert!(raw_nesting_precheck(fake2.as_bytes(), 256).is_err());
        // A real stream still hides what is inside its body.
        let real = format!("<< /Length 9 >>\nstream\n{}\nendstream", "[".repeat(1000));
        assert!(raw_nesting_precheck(real.as_bytes(), 8).is_ok());
    }

    #[test]
    fn inline_image_headers_are_found() {
        let max = 64 * 1024 * 1024;
        let big = b"q BI /W 100000 /H 100000 /BPC 8 /CS /G ID xxxx EI Q";
        assert!(inline_image_too_big(big, max));
        let long = b"q BI /Width 100000 /Height 100000 ID \x00 EI Q";
        assert!(inline_image_too_big(long, max));
        let ok = b"q BI /W 100 /H 100 /BPC 8 /CS /G ID xxxx EI Q";
        assert!(!inline_image_too_big(ok, max));
        // Chunk borders: the header is found whatever the split.
        let mut stream = vec![b' '; 5000];
        stream.extend_from_slice(big);
        for split in [1, 4999, 5003, 5010, 5030] {
            let mut sc = InlineScanner::new(max);
            let mut budget = u64::MAX;
            sc.feed(&stream[..split], &mut budget);
            sc.feed(&stream[split..], &mut budget);
            assert!(sc.too_big, "split {split}");
        }
    }

    #[test]
    fn nesting_precheck_counts_depth_outside_streams_and_strings() {
        let deep = format!("<< /A {}1{} >>", "<< /A ".repeat(300), " >>".repeat(300));
        assert!(raw_nesting_precheck(deep.as_bytes(), 256).is_err());
        assert!(raw_nesting_precheck(deep.as_bytes(), 1000).is_ok());
        // Brackets inside a stream body, a string, a hex string and a comment do not count.
        let calm = format!(
            "%{}\n<< /A ({}) /B <3c3c> >>\nstream\n{}\nendstream\n",
            "[".repeat(1000),
            "[".repeat(1000),
            "[".repeat(1000)
        );
        assert!(raw_nesting_precheck(calm.as_bytes(), 8).is_ok());
        assert!(raw_nesting_precheck(b"", 8).is_ok());
        assert!(raw_nesting_precheck(b"stream", 8).is_ok());
    }
}
