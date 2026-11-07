// SPDX-License-Identifier: GPL-3.0-or-later
//! Pre-flight check of the ZIP container of an Office file, before any engine sees it.
//!
//! This is our **own small reader** of the central directory (no ZIP library): it only looks
//! at headers and counts bytes, it never extracts anything to disk. It refuses:
//!
//! * too many entries, names that are absolute, contain `..`, a backslash, a NUL byte, a
//!   drive letter, or are duplicated (a name that two readers could resolve differently);
//! * encrypted entries, ZIP64, multi-disk archives and compression methods other than
//!   stored and deflate;
//! * entries whose local header disagrees with the central directory, and entries whose
//!   data overlap (overlapping-entry bombs);
//! * entries that are too large or compress too well (checked on the **declared** size and
//!   then on the **really inflated** size, which must equal the declared one, so a header
//!   that lies is caught);
//! * a container that does not match the extension (the main part of the format is missing).
//!
//! This is a screen, not a proof of safety: the engine that parses the XML afterwards is the
//! real attack surface, and runs in a limited child process (see `runner`).

use std::collections::HashSet;
use std::io::Read;

use crate::error::{Limits, OfficeError};
use crate::format::OfficeFormat;

const EOCD_SIG: u32 = 0x0605_4b50;
const CEN_SIG: u32 = 0x0201_4b50;
const LOC_SIG: u32 = 0x0403_4b50;
const MAX_NAME: usize = 512;

/// What the check learned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipReport {
    /// Number of entries.
    pub entries: usize,
    /// Sum of the uncompressed sizes (verified by inflating).
    pub total_uncompressed: u64,
}

#[derive(Debug)]
struct CenEntry {
    name: String,
    method: u16,
    csize: u64,
    usize_: u64,
    local_offset: usize,
}

fn le16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn bad(msg: impl Into<String>) -> OfficeError {
    OfficeError::BadContainer(msg.into())
}

/// Check one container against the limits and the expected format.
pub fn check(
    bytes: &[u8],
    format: OfficeFormat,
    limits: &Limits,
) -> Result<ZipReport, OfficeError> {
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return Err(OfficeError::LegacyOrEncrypted);
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(bad("not a ZIP file (no local header at the start)"));
    }
    let entries = read_central_directory(bytes, limits)?;
    check_names(&entries)?;
    check_layout(bytes, &entries)?;
    let total = check_sizes_and_inflate(bytes, &entries, limits)?;
    check_main_part(bytes, &entries, format)?;
    Ok(ZipReport {
        entries: entries.len(),
        total_uncompressed: total,
    })
}

fn read_central_directory(bytes: &[u8], limits: &Limits) -> Result<Vec<CenEntry>, OfficeError> {
    // The end record is within the last 22 + 65535 bytes.
    let min = bytes.len().saturating_sub(22 + 65_535);
    let mut eocd = None;
    let mut i = bytes.len().checked_sub(22);
    while let Some(p) = i {
        if p < min {
            break;
        }
        if le32(bytes, p) == Some(EOCD_SIG) {
            eocd = Some(p);
            break;
        }
        i = p.checked_sub(1);
    }
    let p = eocd.ok_or_else(|| bad("no end-of-central-directory record (damaged or cut off)"))?;
    let disk = le16(bytes, p + 4).ok_or_else(|| bad("short end record"))?;
    let cd_disk = le16(bytes, p + 6).ok_or_else(|| bad("short end record"))?;
    let n_disk = le16(bytes, p + 8).ok_or_else(|| bad("short end record"))?;
    let n = le16(bytes, p + 10).ok_or_else(|| bad("short end record"))?;
    let cd_size = le32(bytes, p + 12).ok_or_else(|| bad("short end record"))?;
    let cd_off = le32(bytes, p + 16).ok_or_else(|| bad("short end record"))?;
    if disk != 0 || cd_disk != 0 || n_disk != n {
        return Err(bad("multi-disk archives are not supported"));
    }
    if n == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_off == 0xFFFF_FFFF {
        return Err(bad("ZIP64 containers are not supported"));
    }
    let n = usize::from(n);
    if n == 0 {
        return Err(bad("the container is empty"));
    }
    if n > limits.max_zip_entries {
        return Err(OfficeError::TooMuchData(format!(
            "{n} entries (limit {})",
            limits.max_zip_entries
        )));
    }
    let cd_off = cd_off as usize;
    let cd_end = cd_off
        .checked_add(cd_size as usize)
        .filter(|e| *e <= p)
        .ok_or_else(|| bad("the central directory lies outside the file"))?;
    let mut out = Vec::with_capacity(n);
    let mut at = cd_off;
    for _ in 0..n {
        if le32(bytes, at) != Some(CEN_SIG) {
            return Err(bad("damaged central directory entry"));
        }
        let get = |o: usize| le16(bytes, at + o).ok_or_else(|| bad("short central entry"));
        let get32 = |o: usize| le32(bytes, at + o).ok_or_else(|| bad("short central entry"));
        let flags = get(8)?;
        let method = get(10)?;
        let csize = get32(20)?;
        let usize_ = get32(24)?;
        let name_len = usize::from(get(28)?);
        let extra_len = usize::from(get(30)?);
        let comment_len = usize::from(get(32)?);
        let entry_disk = get(34)?;
        let local_offset = get32(42)?;
        if flags & 1 != 0 || flags & 0x40 != 0 {
            return Err(bad("encrypted entries are not supported"));
        }
        if entry_disk != 0 {
            return Err(bad("multi-disk archives are not supported"));
        }
        if csize == 0xFFFF_FFFF || usize_ == 0xFFFF_FFFF || local_offset == 0xFFFF_FFFF {
            return Err(bad("ZIP64 entries are not supported"));
        }
        if method != 0 && method != 8 {
            return Err(bad(format!("compression method {method} is not supported")));
        }
        if name_len == 0 || name_len > MAX_NAME {
            return Err(bad("an entry name is empty or too long"));
        }
        let name_bytes = bytes
            .get(at + 46..at + 46 + name_len)
            .ok_or_else(|| bad("short central entry name"))?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| bad("an entry name is not valid UTF-8"))?
            .to_string();
        out.push(CenEntry {
            name,
            method,
            csize: u64::from(csize),
            usize_: u64::from(usize_),
            local_offset: local_offset as usize,
        });
        at = at
            .checked_add(46 + name_len + extra_len + comment_len)
            .filter(|a| *a <= cd_end)
            .ok_or_else(|| bad("the central directory is longer than it says"))?;
    }
    Ok(out)
}

/// Names that no reader may resolve outside its own directory, and no duplicates.
fn check_names(entries: &[CenEntry]) -> Result<(), OfficeError> {
    let mut seen = HashSet::new();
    for e in entries {
        let n = e.name.as_str();
        let drive =
            n.len() >= 2 && n.as_bytes()[1] == b':' && n.as_bytes()[0].is_ascii_alphabetic();
        if n.contains('\0')
            || n.contains('\\')
            || n.starts_with('/')
            || drive
            || n.split('/').any(|c| c == "..")
            || n.chars().any(|c| c.is_control())
        {
            return Err(bad(format!("unsafe entry name {n:?}")));
        }
        if !seen.insert(n.to_ascii_lowercase()) {
            return Err(bad(format!("duplicate entry name {n:?}")));
        }
    }
    Ok(())
}

/// Local header agrees with the central entry; data of different entries do not overlap.
fn check_layout(bytes: &[u8], entries: &[CenEntry]) -> Result<(), OfficeError> {
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(entries.len());
    for e in entries {
        let o = e.local_offset;
        if le32(bytes, o) != Some(LOC_SIG) {
            return Err(bad(format!("entry {:?} has no local header", e.name)));
        }
        let name_len = usize::from(le16(bytes, o + 26).ok_or_else(|| bad("short local header"))?);
        let extra_len = usize::from(le16(bytes, o + 28).ok_or_else(|| bad("short local header"))?);
        let local_name = bytes
            .get(o + 30..o + 30 + name_len)
            .ok_or_else(|| bad("short local header"))?;
        if local_name != e.name.as_bytes() {
            return Err(bad(format!(
                "entry {:?}: local and central names differ",
                e.name
            )));
        }
        let start = o + 30 + name_len + extra_len;
        let end = start
            .checked_add(e.csize as usize)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| bad(format!("entry {:?} runs past the end of the file", e.name)))?;
        spans.push((o, end));
    }
    spans.sort_unstable();
    for w in spans.windows(2) {
        if w[1].0 < w[0].1 {
            return Err(bad("entries overlap"));
        }
    }
    Ok(())
}

fn data_of<'a>(bytes: &'a [u8], e: &CenEntry) -> &'a [u8] {
    // Offsets were validated by `check_layout`.
    let o = e.local_offset;
    let name_len = usize::from(le16(bytes, o + 26).unwrap_or(0));
    let extra_len = usize::from(le16(bytes, o + 28).unwrap_or(0));
    let start = o + 30 + name_len + extra_len;
    &bytes[start..start + e.csize as usize]
}

/// Inflate (and discard) the entry; returns the real size, stops past `cap`.
fn real_size(raw: &[u8], method: u16, cap: u64) -> Result<u64, OfficeError> {
    let mut reader: Box<dyn Read + '_> = if method == 0 {
        Box::new(raw)
    } else {
        Box::new(flate2::read::DeflateDecoder::new(raw))
    };
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| bad(format!("damaged compressed data: {e}")))?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        if total > cap {
            return Ok(total);
        }
    }
}

fn check_sizes_and_inflate(
    bytes: &[u8],
    entries: &[CenEntry],
    limits: &Limits,
) -> Result<u64, OfficeError> {
    let mut declared_total = 0u64;
    for e in entries {
        if e.usize_ > limits.max_entry_bytes {
            return Err(OfficeError::TooMuchData(format!(
                "entry {:?} declares {} bytes (limit {})",
                e.name, e.usize_, limits.max_entry_bytes
            )));
        }
        if e.method == 0 && e.csize != e.usize_ {
            return Err(bad(format!("entry {:?}: stored sizes differ", e.name)));
        }
        if e.usize_ > 1024 * 1024 && e.usize_ / e.csize.max(1) > limits.max_ratio {
            return Err(OfficeError::TooMuchData(format!(
                "entry {:?} compresses more than {}:1",
                e.name, limits.max_ratio
            )));
        }
        declared_total = declared_total.saturating_add(e.usize_);
        if declared_total > limits.max_total_uncompressed_bytes {
            return Err(OfficeError::TooMuchData(format!(
                "all entries together declare more than {} bytes",
                limits.max_total_uncompressed_bytes
            )));
        }
    }
    // The declared numbers are only claims: inflate every entry (bounded by what it
    // claims plus one byte) and compare.
    let mut actual_total = 0u64;
    for e in entries {
        let real = real_size(data_of(bytes, e), e.method, e.usize_)?;
        if real != e.usize_ {
            return Err(bad(format!(
                "entry {:?}: declared {} bytes but contains {}{}",
                e.name,
                e.usize_,
                real,
                if real > e.usize_ { " or more" } else { "" }
            )));
        }
        actual_total += real;
    }
    Ok(actual_total)
}

fn check_main_part(
    bytes: &[u8],
    entries: &[CenEntry],
    format: OfficeFormat,
) -> Result<(), OfficeError> {
    if let Some(part) = format.main_part() {
        let has_types = entries.iter().any(|e| e.name == "[Content_Types].xml");
        let has_main = entries.iter().any(|e| e.name == part);
        if !has_types || !has_main {
            return Err(bad(format!(
                "the content does not match .{} (missing [Content_Types].xml or {part})",
                format.extension()
            )));
        }
    }
    if let Some(mime) = format.odf_mimetype() {
        let m = entries.iter().find(|e| e.name == "mimetype");
        let ok = m.is_some_and(|e| e.method == 0 && data_of(bytes, e) == mime.as_bytes());
        if !ok {
            return Err(bad(format!(
                "the content does not match .{} (missing or wrong mimetype entry)",
                format.extension()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/common/zipwriter.rs"]
mod zipwriter;

#[cfg(test)]
mod tests {
    use super::zipwriter::{self as zw, Entry};
    use super::*;

    fn lim() -> Limits {
        Limits {
            max_entry_bytes: 8 * 1024 * 1024,
            max_total_uncompressed_bytes: 12 * 1024 * 1024,
            max_zip_entries: 50,
            ..Limits::default()
        }
    }

    fn expect_container(r: Result<ZipReport, OfficeError>, needle: &str) {
        match r {
            Err(OfficeError::BadContainer(m)) if m.contains(needle) => {}
            other => panic!("expected BadContainer containing {needle:?}, got {other:?}"),
        }
    }

    #[test]
    fn accepts_the_generated_fixtures() {
        for (b, f) in [
            (zw::docx("hi"), OfficeFormat::Docx),
            (zw::xlsx("hi"), OfficeFormat::Xlsx),
            (zw::pptx("hi"), OfficeFormat::Pptx),
        ] {
            let r = check(&b, f, &lim()).expect("fixture must pass");
            assert!(r.entries >= 3);
        }
    }

    #[test]
    fn accepts_an_opendocument_shape() {
        let b = zw::build(&[
            Entry::stored("mimetype", b"application/vnd.oasis.opendocument.text"),
            Entry::deflated("content.xml", b"<x/>"),
        ]);
        check(&b, OfficeFormat::Odt, &lim()).expect("odt shape");
        // The same file under another extension is a mismatch.
        expect_container(check(&b, OfficeFormat::Ods, &lim()), "does not match");
    }

    #[test]
    fn extension_must_match_content() {
        expect_container(
            check(&zw::docx("x"), OfficeFormat::Xlsx, &lim()),
            "does not match",
        );
    }

    #[test]
    fn zip_bomb_is_refused_by_ratio_and_size() {
        // 6 MiB of zeros deflates to a few KiB: ratio far above 200 and above 1 MiB.
        let zeros = vec![0u8; 6 * 1024 * 1024];
        let b = zw::build(&[
            Entry::deflated("[Content_Types].xml", b"<Types/>"),
            Entry::deflated("word/document.xml", &zeros),
        ]);
        assert!(matches!(
            check(&b, OfficeFormat::Docx, &lim()),
            Err(OfficeError::TooMuchData(m)) if m.contains("compresses more than")
        ));
        // Bigger than the per-entry cap.
        let big = vec![0u8; 9 * 1024 * 1024];
        let b = zw::build(&[
            Entry::deflated("[Content_Types].xml", b"<Types/>"),
            Entry::deflated("word/document.xml", &big),
        ]);
        assert!(matches!(
            check(&b, OfficeFormat::Docx, &lim()),
            Err(OfficeError::TooMuchData(_))
        ));
    }

    #[test]
    fn total_size_cap_is_enforced() {
        // Three entries of 5 MiB of noise-ish data (stored): 15 MiB > 12 MiB total.
        let data: Vec<u8> = (0..5 * 1024 * 1024u32)
            .map(|i| (i * 31 % 251) as u8)
            .collect();
        let b = zw::build(&[
            Entry::stored("[Content_Types].xml", &data),
            Entry::stored("word/document.xml", &data),
            Entry::stored("word/x.bin", &data),
        ]);
        assert!(matches!(
            check(&b, OfficeFormat::Docx, &lim()),
            Err(OfficeError::TooMuchData(m)) if m.contains("together")
        ));
    }

    #[test]
    fn lying_header_is_caught_by_inflating() {
        // Header says 10 bytes, the data inflate to 4 MiB (ratio check is bypassed by the lie).
        let zeros = vec![0u8; 4 * 1024 * 1024];
        let mut e = Entry::deflated("word/document.xml", &zeros);
        e.lie_usize = Some(10);
        let b = zw::build(&[Entry::deflated("[Content_Types].xml", b"<Types/>"), e]);
        expect_container(check(&b, OfficeFormat::Docx, &lim()), "declared 10 bytes");
    }

    #[test]
    fn too_many_entries() {
        let mut v = vec![Entry::deflated("[Content_Types].xml", b"<Types/>")];
        for i in 0..60 {
            v.push(Entry::stored(&format!("word/p{i}.xml"), b"x"));
        }
        let b = zw::build(&v);
        assert!(matches!(
            check(&b, OfficeFormat::Docx, &lim()),
            Err(OfficeError::TooMuchData(m)) if m.contains("entries")
        ));
    }

    #[test]
    fn path_traversal_and_odd_names_are_refused() {
        for name in [
            "../evil.xml",
            "word/../../evil.xml",
            "/etc/passwd",
            "C:/win/evil.xml",
            "word\\document.xml",
            "a\0b.xml",
            "x/\u{1}y",
        ] {
            let mut entries = vec![
                Entry::deflated("[Content_Types].xml", b"<Types/>"),
                Entry::deflated("word/document.xml", b"<w/>"),
            ];
            entries.push(Entry::stored(name, b"x"));
            let v = zw::build(&entries);
            expect_container(check(&v, OfficeFormat::Docx, &lim()), "unsafe entry name");
        }
    }

    #[test]
    fn duplicate_names_are_refused() {
        let b = zw::build(&[
            Entry::deflated("[Content_Types].xml", b"<Types/>"),
            Entry::deflated("word/document.xml", b"<w/>"),
            Entry::deflated("WORD/Document.xml", b"<evil/>"),
        ]);
        expect_container(check(&b, OfficeFormat::Docx, &lim()), "duplicate");
    }

    #[test]
    fn encrypted_and_unknown_method_are_refused() {
        let mut e = Entry::stored("word/document.xml", b"<w/>");
        e.flags = 1;
        let b = zw::build(&[Entry::stored("[Content_Types].xml", b"<Types/>"), e]);
        expect_container(check(&b, OfficeFormat::Docx, &lim()), "encrypted");
    }

    #[test]
    fn corrupt_files_are_refused_not_panicked_on() {
        let good = zw::docx("hello");
        // Not a zip at all.
        expect_container(
            check(b"hello world", OfficeFormat::Docx, &lim()),
            "not a ZIP",
        );
        expect_container(check(b"", OfficeFormat::Docx, &lim()), "not a ZIP");
        // Cut off at every length: never a panic, always an error.
        for cut in 0..good.len() {
            assert!(
                check(&good[..cut], OfficeFormat::Docx, &lim()).is_err(),
                "truncation at {cut} was accepted"
            );
        }
        // Flip every byte once: never a panic (may pass or fail).
        for i in 0..good.len() {
            let mut b = good.clone();
            b[i] ^= 0xFF;
            let _ = check(&b, OfficeFormat::Docx, &lim());
        }
        // Garbage in the middle of the deflate data.
        let mut b = good.clone();
        let mid = b.len() / 3;
        for x in &mut b[mid..mid + 20] {
            *x = 0xAA;
        }
        let _ = check(&b, OfficeFormat::Docx, &lim());
    }

    #[test]
    fn old_binary_office_files_get_a_helpful_error() {
        let mut ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        ole.extend_from_slice(&[0u8; 100]);
        assert!(matches!(
            check(&ole, OfficeFormat::Docx, &lim()),
            Err(OfficeError::LegacyOrEncrypted)
        ));
    }

    #[test]
    fn overlapping_entries_are_refused() {
        // Make the second local header offset point into the first entry's data.
        let mut b = zw::build(&[
            Entry::stored("[Content_Types].xml", &[b'a'; 200]),
            Entry::stored("word/document.xml", &[b'b'; 200]),
        ]);
        // Find the central directory entry of the second file and point it at offset 0.
        let sig = CEN_SIG.to_le_bytes();
        let second = b
            .windows(4)
            .enumerate()
            .filter(|(_, w)| *w == sig)
            .nth(1)
            .map(|(i, _)| i)
            .unwrap();
        b[second + 42..second + 46].copy_from_slice(&0u32.to_le_bytes());
        let r = check(&b, OfficeFormat::Docx, &lim());
        assert!(r.is_err(), "{r:?}");
    }
}
