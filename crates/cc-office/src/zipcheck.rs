// SPDX-License-Identifier: GPL-3.0-or-later
//! Pre-flight check of the ZIP container of an Office file, before any engine sees it.
//!
//! This is our **own small reader** of the central directory (no ZIP library): it only looks
//! at headers and counts bytes, it never extracts anything to disk. It refuses:
//!
//! * too many entries, names that are absolute, contain `..`, a backslash, a NUL byte or any
//!   control character, a drive letter, or that are duplicated after Unicode lower-casing
//!   (a name that two readers could resolve differently);
//! * encrypted entries; ZIP64 (a ZIP64 locator before the end record, a ZIP64 extra field, or
//!   a field set to its ZIP64 marker value); multi-disk archives; compression methods other
//!   than stored and deflate; the Info-ZIP Unicode path extra field (`0x7075`), which lets a
//!   reader replace the name we checked;
//! * a container whose end-of-central-directory record is not the very end of the file (data
//!   after it, or a comment length that does not match), whose central directory does not
//!   end exactly where that record starts, or whose entries do not fill it exactly;
//! * entries whose local header disagrees with the central directory (name, compression
//!   method, flags, sizes and CRC; with a data descriptor the descriptor is compared
//!   instead), entries that overlap each other, and entries that reach into the central
//!   directory;
//! * entries that are too large or compress too well (checked on the **declared** size and
//!   then on the **really inflated** size, which must equal the declared one, so a header
//!   that lies is caught);
//! * a container that does not match the extension (the main part of the format is missing).
//!
//! What is **not** checked: the CRC-32 of the data is compared between headers but not
//! recomputed, bytes between entries (gaps) are tolerated, and the file comment is not
//! interpreted. This is a screen, not a proof of safety: the engine that parses the XML
//! afterwards is the real attack surface, and runs in a limited child process (see `runner`).

use std::collections::HashSet;
use std::io::Read;

use crate::error::{Limits, OfficeError};
use crate::format::OfficeFormat;

const EOCD_SIG: u32 = 0x0605_4b50;
const EOCD_LEN: usize = 22;
const ZIP64_LOCATOR_SIG: u32 = 0x0706_4b50;
const CEN_SIG: u32 = 0x0201_4b50;
const LOC_SIG: u32 = 0x0403_4b50;
const DESC_SIG: u32 = 0x0807_4b50;
const MAX_NAME: usize = 512;
const FLAG_ENCRYPTED: u16 = 1;
const FLAG_DESCRIPTOR: u16 = 8;
const FLAG_STRONG: u16 = 0x40;

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
    flags: u16,
    method: u16,
    crc: u32,
    csize: u64,
    usize_: u64,
    local_offset: usize,
    /// Start of the compressed data; set by `check_layout`.
    data_start: usize,
}

fn le16(b: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    b.get(at..end).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    b.get(at..end)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn bad(msg: impl Into<String>) -> OfficeError {
    OfficeError::BadContainer(msg.into())
}

/// Checked addition (a 32-bit build must not wrap around on a hostile offset).
fn add(a: usize, b: usize) -> Result<usize, OfficeError> {
    a.checked_add(b)
        .ok_or_else(|| bad("an offset or length overflows"))
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
    let (mut entries, cd_start) = read_central_directory(bytes, limits)?;
    check_names(&entries)?;
    check_layout(bytes, &mut entries, cd_start)?;
    let total = check_sizes_and_inflate(bytes, &entries, limits)?;
    check_main_part(bytes, &entries, format)?;
    Ok(ZipReport {
        entries: entries.len(),
        total_uncompressed: total,
    })
}

/// The end record must be the very end of the file: its comment length has to match the
/// bytes that follow it. Two candidates, or a signature inside the comment, are ambiguous
/// (readers that scan from the other end would pick another record) and refused.
fn find_eocd(bytes: &[u8]) -> Result<usize, OfficeError> {
    let min = bytes.len().saturating_sub(EOCD_LEN + 65_535);
    let mut i = bytes.len().checked_sub(EOCD_LEN);
    let mut found = None;
    let mut saw_sig = false;
    while let Some(p) = i {
        if p < min {
            break;
        }
        if le32(bytes, p) == Some(EOCD_SIG) {
            saw_sig = true;
            let clen = usize::from(le16(bytes, p + 20).unwrap_or(0));
            if p + EOCD_LEN + clen == bytes.len() {
                if found.is_some() {
                    return Err(bad("more than one end-of-central-directory record fits"));
                }
                found = Some(p);
            }
        }
        i = p.checked_sub(1);
    }
    let Some(p) = found else {
        return Err(bad(if saw_sig {
            "data after the end-of-central-directory record, or a wrong comment length"
        } else {
            "no end-of-central-directory record (damaged or cut off)"
        }));
    };
    let comment = &bytes[p + EOCD_LEN..];
    if comment.windows(4).any(|w| w == EOCD_SIG.to_le_bytes()) {
        return Err(bad(
            "the archive comment contains an end-of-central-directory signature",
        ));
    }
    Ok(p)
}

/// Refuse the extra fields we do not handle: ZIP64 sizes (`0x0001`) and the Info-ZIP
/// Unicode path (`0x7075`). A malformed extra area is refused too.
fn check_extra(extra: &[u8]) -> Result<(), OfficeError> {
    let mut at = 0usize;
    while at < extra.len() {
        let (Some(id), Some(len)) = (le16(extra, at), le16(extra, at + 2)) else {
            return Err(bad("a malformed extra field"));
        };
        let end = add(add(at, 4)?, usize::from(len))?;
        if end > extra.len() {
            return Err(bad("a malformed extra field"));
        }
        match id {
            0x0001 => return Err(bad("ZIP64 extra fields are not supported")),
            0x7075 => {
                return Err(bad(
                    "Unicode path extra fields are not supported (they can change the entry name)",
                ));
            }
            _ => {}
        }
        at = end;
    }
    Ok(())
}

/// Returns the entries and where the central directory starts.
fn read_central_directory(
    bytes: &[u8],
    limits: &Limits,
) -> Result<(Vec<CenEntry>, usize), OfficeError> {
    let p = find_eocd(bytes)?;
    if p >= 20 && le32(bytes, p - 20) == Some(ZIP64_LOCATOR_SIG) {
        return Err(bad("ZIP64 containers are not supported"));
    }
    let short = || bad("short end record");
    let disk = le16(bytes, p + 4).ok_or_else(short)?;
    let cd_disk = le16(bytes, p + 6).ok_or_else(short)?;
    let n_disk = le16(bytes, p + 8).ok_or_else(short)?;
    let n = le16(bytes, p + 10).ok_or_else(short)?;
    let cd_size = le32(bytes, p + 12).ok_or_else(short)?;
    let cd_off = le32(bytes, p + 16).ok_or_else(short)?;
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
    let cd_off = usize::try_from(cd_off).map_err(|_| bad("the central directory is too far"))?;
    let cd_size = usize::try_from(cd_size).map_err(|_| bad("the central directory is too big"))?;
    let cd_end = add(cd_off, cd_size)?;
    if cd_end != p {
        return Err(bad(
            "the central directory does not end where the end record starts",
        ));
    }
    let mut out = Vec::with_capacity(n);
    let mut at = cd_off;
    for _ in 0..n {
        if le32(bytes, at) != Some(CEN_SIG) {
            return Err(bad("damaged central directory entry"));
        }
        let get = |o: usize| le16(bytes, add(at, o)?).ok_or_else(|| bad("short central entry"));
        let get32 = |o: usize| le32(bytes, add(at, o)?).ok_or_else(|| bad("short central entry"));
        let flags = get(8)?;
        let method = get(10)?;
        let crc = get32(16)?;
        let csize = get32(20)?;
        let usize_ = get32(24)?;
        let name_len = usize::from(get(28)?);
        let extra_len = usize::from(get(30)?);
        let comment_len = usize::from(get(32)?);
        let entry_disk = get(34)?;
        let local_offset = get32(42)?;
        if flags & (FLAG_ENCRYPTED | FLAG_STRONG) != 0 {
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
        let name_at = add(at, 46)?;
        let extra_at = add(name_at, name_len)?;
        let comment_at = add(extra_at, extra_len)?;
        let next = add(comment_at, comment_len)?;
        if next > cd_end {
            return Err(bad("the central directory is longer than it says"));
        }
        let name_bytes = bytes
            .get(name_at..extra_at)
            .ok_or_else(|| bad("short central entry name"))?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| bad("an entry name is not valid UTF-8"))?
            .to_string();
        check_extra(&bytes[extra_at..comment_at])?;
        out.push(CenEntry {
            name,
            flags,
            method,
            crc,
            csize: u64::from(csize),
            usize_: u64::from(usize_),
            local_offset: usize::try_from(local_offset)
                .map_err(|_| bad("a local header offset is too far"))?,
            data_start: 0,
        });
        at = next;
    }
    if at != cd_end {
        return Err(bad("the central directory is longer than its entries"));
    }
    Ok((out, cd_off))
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
        // Unicode lower-casing, so a case-insensitive file system sees no two equal names.
        if !seen.insert(n.to_lowercase()) {
            return Err(bad(format!("duplicate entry name {n:?}")));
        }
    }
    Ok(())
}

/// Length of the data descriptor at `at` if it agrees with the central entry (12 bytes, or
/// 16 with the optional signature).
fn descriptor_len(bytes: &[u8], at: usize, e: &CenEntry) -> Option<usize> {
    let same = |o: usize| -> Option<bool> {
        let crc = le32(bytes, at.checked_add(o)?)?;
        let cs = le32(bytes, at.checked_add(o)?.checked_add(4)?)?;
        let us = le32(bytes, at.checked_add(o)?.checked_add(8)?)?;
        Some(crc == e.crc && u64::from(cs) == e.csize && u64::from(us) == e.usize_)
    };
    if le32(bytes, at) == Some(DESC_SIG) && same(4) == Some(true) {
        Some(16)
    } else if same(0) == Some(true) {
        Some(12)
    } else {
        None
    }
}

/// Local header agrees with the central entry; data of different entries do not overlap,
/// and none reaches into the central directory.
fn check_layout(
    bytes: &[u8],
    entries: &mut [CenEntry],
    cd_start: usize,
) -> Result<(), OfficeError> {
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(entries.len());
    for e in entries.iter_mut() {
        let o = e.local_offset;
        let short = || bad("short local header");
        if le32(bytes, o) != Some(LOC_SIG) {
            return Err(bad(format!("entry {:?} has no local header", e.name)));
        }
        let lflags = le16(bytes, add(o, 6)?).ok_or_else(short)?;
        let lmethod = le16(bytes, add(o, 8)?).ok_or_else(short)?;
        let lcrc = le32(bytes, add(o, 14)?).ok_or_else(short)?;
        let lcsize = u64::from(le32(bytes, add(o, 18)?).ok_or_else(short)?);
        let lusize = u64::from(le32(bytes, add(o, 22)?).ok_or_else(short)?);
        let name_len = usize::from(le16(bytes, add(o, 26)?).ok_or_else(short)?);
        let extra_len = usize::from(le16(bytes, add(o, 28)?).ok_or_else(short)?);
        let name_at = add(o, 30)?;
        let extra_at = add(name_at, name_len)?;
        let start = add(extra_at, extra_len)?;
        let local_name = bytes.get(name_at..extra_at).ok_or_else(short)?;
        if local_name != e.name.as_bytes() {
            return Err(bad(format!(
                "entry {:?}: local and central names differ",
                e.name
            )));
        }
        check_extra(bytes.get(extra_at..start).ok_or_else(short)?)?;
        if lmethod != e.method {
            return Err(bad(format!(
                "entry {:?}: local and central compression methods differ",
                e.name
            )));
        }
        if lflags != e.flags {
            return Err(bad(format!(
                "entry {:?}: local and central flags differ",
                e.name
            )));
        }
        let csize = usize::try_from(e.csize).map_err(|_| bad("an entry is too big"))?;
        let mut end = add(start, csize)?;
        if e.flags & FLAG_DESCRIPTOR == 0 {
            if lcrc != e.crc || lcsize != e.csize || lusize != e.usize_ {
                return Err(bad(format!(
                    "entry {:?}: local and central sizes or checksum differ",
                    e.name
                )));
            }
        } else {
            // The real values follow the data. The local header carries zeros (or the same).
            let zero_or_same = |l: u64, c: u64| l == 0 || l == c;
            if !zero_or_same(lcsize, e.csize)
                || !zero_or_same(lusize, e.usize_)
                || (lcrc != 0 && lcrc != e.crc)
            {
                return Err(bad(format!(
                    "entry {:?}: local header sizes disagree with the central directory",
                    e.name
                )));
            }
            let dl = descriptor_len(bytes, end, e).ok_or_else(|| {
                bad(format!(
                    "entry {:?}: the data descriptor is missing or disagrees with the central directory",
                    e.name
                ))
            })?;
            end = add(end, dl)?;
        }
        if end > bytes.len() {
            return Err(bad(format!(
                "entry {:?} runs past the end of the file",
                e.name
            )));
        }
        if end > cd_start {
            return Err(bad(format!(
                "entry {:?} overlaps the central directory",
                e.name
            )));
        }
        e.data_start = start;
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
    // Offsets were validated by `check_layout`; an impossible span reads as empty.
    usize::try_from(e.csize)
        .ok()
        .and_then(|n| bytes.get(e.data_start..e.data_start.checked_add(n)?))
        .unwrap_or(&[])
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
        total = total.saturating_add(n as u64);
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
        actual_total = actual_total.saturating_add(real);
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

    fn positions(b: &[u8], sig: u32) -> Vec<usize> {
        let s = sig.to_le_bytes();
        b.windows(4)
            .enumerate()
            .filter(|(_, w)| *w == s)
            .map(|(i, _)| i)
            .collect()
    }
    fn put16(b: &mut [u8], at: usize, v: u16) {
        b[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn put32(b: &mut [u8], at: usize, v: u32) {
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn get32(b: &[u8], at: usize) -> u32 {
        le32(b, at).unwrap()
    }
    /// A small valid .docx-shaped archive (deflated parts).
    fn base() -> Vec<u8> {
        zw::build(&[
            Entry::deflated("[Content_Types].xml", b"<Types/>"),
            Entry::deflated("word/document.xml", b"<w/>"),
        ])
    }
    fn ok(b: &[u8]) {
        check(b, OfficeFormat::Docx, &lim()).unwrap_or_else(|e| panic!("expected ok: {e:?}"));
    }
    fn refused(b: &[u8], needle: &str) {
        expect_container(check(b, OfficeFormat::Docx, &lim()), needle);
    }

    #[test]
    fn overlapping_entries_are_refused() {
        // Entry B's local header (with the right name, method, sizes and CRC) is hidden
        // inside the data of entry A, and B's central entry points at it. Every name and
        // header agrees; only the data overlap.
        let b_entry = Entry::stored("word/document.xml", b"<w/>");
        let single = zw::build(std::slice::from_ref(&b_entry));
        let cd = positions(&single, CEN_SIG)[0];
        let hidden_local = single[..cd].to_vec();
        let a_name = "[Content_Types].xml";
        let mut b = zw::build(&[Entry::stored(a_name, &hidden_local), b_entry]);
        ok(&b); // the control: unpatched, it is a valid archive
        let cens = positions(&b, CEN_SIG);
        assert_eq!(cens.len(), 2);
        put32(&mut b, cens[1] + 42, (30 + a_name.len()) as u32);
        refused(&b, "overlap");
    }

    #[test]
    fn entry_reaching_into_the_central_directory_is_refused() {
        let mut b = zw::build(&[
            Entry::deflated("[Content_Types].xml", b"<Types/>"),
            Entry::stored("word/document.xml", b"<w/>"),
        ]);
        ok(&b);
        let cens = positions(&b, CEN_SIG);
        let locs = positions(&b, LOC_SIG);
        // Stored entry: grow compressed and uncompressed size by 50 in both headers.
        put32(&mut b, cens[1] + 20, 54);
        put32(&mut b, cens[1] + 24, 54);
        put32(&mut b, locs[1] + 18, 54);
        put32(&mut b, locs[1] + 22, 54);
        refused(&b, "central directory");
    }

    #[test]
    fn data_after_the_end_record_and_bad_comment_lengths_are_refused() {
        let good = base();
        ok(&good);
        let mut b = good.clone();
        b.extend_from_slice(b"trailing junk");
        refused(&b, "after the end-of-central-directory");
        // A comment length that promises more than there is.
        let mut b = good.clone();
        let p = positions(&b, EOCD_SIG)[0];
        put16(&mut b, p + 20, 5);
        refused(&b, "comment length");
        // A comment length that is shorter than the bytes that follow.
        let mut b = good.clone();
        b.extend_from_slice(b"hi");
        put16(&mut b, p + 20, 1);
        refused(&b, "comment length");
        // A matching comment is fine.
        let mut b = good.clone();
        b.extend_from_slice(b"hi");
        put16(&mut b, p + 20, 2);
        ok(&b);
        // A second end-record signature inside the comment is ambiguous.
        let mut b = good;
        let mut comment = EOCD_SIG.to_le_bytes().to_vec();
        comment.extend_from_slice(&[0u8; 30]);
        put16(&mut b, p + 20, comment.len() as u16);
        b.extend_from_slice(&comment);
        refused(&b, "comment contains");
    }

    #[test]
    fn central_directory_must_end_at_the_end_record() {
        // Bytes between the central directory and the end record.
        let mut b = base();
        let p = positions(&b, EOCD_SIG)[0];
        b.splice(p..p, [0u8; 3]);
        refused(&b, "does not end where the end record starts");
        // The directory is longer than the entries it lists (count says 1 of 2).
        let mut b = base();
        let p = positions(&b, EOCD_SIG)[0];
        put16(&mut b, p + 8, 1);
        put16(&mut b, p + 10, 1);
        refused(&b, "longer than its entries");
    }

    #[test]
    fn local_and_central_headers_must_agree() {
        let good = base();
        let locs = positions(&good, LOC_SIG);
        let cens = positions(&good, CEN_SIG);
        assert_eq!((locs.len(), cens.len()), (2, 2));
        // Method (deflate in the central entry, stored in the local header).
        let mut b = good.clone();
        put16(&mut b, locs[1] + 8, 0);
        refused(&b, "compression methods differ");
        // Flags.
        let mut b = good.clone();
        put16(&mut b, locs[1] + 6, 0x0800);
        refused(&b, "flags differ");
        // Uncompressed size, compressed size, CRC.
        for off in [22usize, 18, 14] {
            let mut b = good.clone();
            let v = get32(&b, locs[1] + off);
            put32(&mut b, locs[1] + off, v ^ 1);
            refused(&b, "sizes or checksum differ");
        }
    }

    #[test]
    fn data_descriptors_are_handled() {
        let mut d = Entry::deflated("word/document.xml", b"<w/>");
        d.descriptor = true;
        let entries = [Entry::deflated("[Content_Types].xml", b"<Types/>"), d];
        let good = zw::build(&entries);
        ok(&good); // zeros in the local header, real values after the data
        let locs = positions(&good, LOC_SIG);
        let cens = positions(&good, CEN_SIG);
        assert_eq!(get32(&good, locs[1] + 18), 0);
        // A local header that carries the real values as well is accepted.
        let mut b = good.clone();
        for off in [14usize, 18, 22] {
            let v = get32(&b, cens[1] + off + 2);
            put32(&mut b, locs[1] + off, v);
        }
        ok(&b);
        // A local value that is neither zero nor the central one is not.
        let mut b = good.clone();
        put32(&mut b, locs[1] + 22, 12345);
        refused(&b, "local header sizes disagree");
        // A descriptor whose sizes differ from the central directory.
        let mut b = good.clone();
        let desc = positions(&b, DESC_SIG);
        assert_eq!(desc.len(), 1);
        let v = get32(&b, desc[0] + 8);
        put32(&mut b, desc[0] + 8, v + 1);
        refused(&b, "data descriptor");
        // The flag set only in the central directory is a flag mismatch.
        let mut b = good;
        put16(&mut b, locs[1] + 6, 0);
        refused(&b, "flags differ");
    }

    #[test]
    fn info_zip_unicode_path_and_zip64_extra_fields_are_refused() {
        let with_extra = |extra: Vec<u8>| {
            let mut e = Entry::deflated("word/document.xml", b"<w/>");
            e.extra = extra;
            zw::build(&[Entry::deflated("[Content_Types].xml", b"<Types/>"), e])
        };
        // 0x7075: version 1, CRC of the name, replacement name "x".
        let unicode = vec![0x75, 0x70, 6, 0, 1, 0, 0, 0, 0, b'x'];
        refused(&with_extra(unicode.clone()), "Unicode path");
        // 0x0001: ZIP64 sizes.
        let mut z64 = vec![1, 0, 8, 0];
        z64.extend_from_slice(&[0u8; 8]);
        refused(&with_extra(z64), "ZIP64 extra");
        // An unknown, harmless extra field is fine; a malformed one is not.
        ok(&with_extra(vec![0xFE, 0xCA, 2, 0, 9, 9]));
        refused(&with_extra(vec![0xFE, 0xCA, 9, 0, 9]), "malformed extra");
        refused(&with_extra(vec![0xFE, 0xCA, 0]), "malformed extra");
        // Only the *local* header carries 0x7075: the central entry is clean.
        let b = with_extra(unicode);
        let cens = positions(&b, CEN_SIG);
        let at = cens[1] + 46 + "word/document.xml".len();
        assert_eq!(&b[at..at + 2], &[0x75, 0x70]);
        let mut b = b;
        b[at..at + 2].copy_from_slice(&[0xFE, 0xCA]);
        refused(&b, "Unicode path");
    }

    #[test]
    fn zip64_multi_disk_and_unknown_methods_are_refused() {
        let good = base();
        let p = positions(&good, EOCD_SIG)[0];
        let cens = positions(&good, CEN_SIG);
        // ZIP64 locator in front of the end record.
        let mut b = good.clone();
        let mut loc = ZIP64_LOCATOR_SIG.to_le_bytes().to_vec();
        loc.extend_from_slice(&[0u8; 16]);
        b.splice(p..p, loc);
        refused(&b, "ZIP64");
        // ZIP64 marker values in the end record.
        let mut b = good.clone();
        put16(&mut b, p + 8, 0xFFFF);
        put16(&mut b, p + 10, 0xFFFF);
        refused(&b, "ZIP64");
        for off in [12usize, 16] {
            let mut b = good.clone();
            put32(&mut b, p + off, 0xFFFF_FFFF);
            refused(&b, "ZIP64");
        }
        // ZIP64 marker values in a central entry.
        for off in [20usize, 24, 42] {
            let mut b = good.clone();
            put32(&mut b, cens[0] + off, 0xFFFF_FFFF);
            refused(&b, "ZIP64");
        }
        // Multi-disk: disk number, directory disk, per-disk count, entry disk.
        let mut b = good.clone();
        put16(&mut b, p + 4, 1);
        refused(&b, "multi-disk");
        let mut b = good.clone();
        put16(&mut b, p + 6, 1);
        refused(&b, "multi-disk");
        let mut b = good.clone();
        put16(&mut b, p + 8, 1);
        refused(&b, "multi-disk");
        let mut b = good.clone();
        put16(&mut b, cens[1] + 34, 1);
        refused(&b, "multi-disk");
        // Unknown compression methods (bzip2 = 12, LZMA = 14, a made-up 99).
        for m in [12u16, 14, 99] {
            let mut b = good.clone();
            put16(&mut b, cens[1] + 10, m);
            refused(&b, &format!("compression method {m}"));
        }
        // Strong encryption flag.
        let mut b = good;
        put16(&mut b, cens[1] + 8, 0x40);
        refused(&b, "encrypted");
    }

    #[test]
    fn duplicate_names_are_folded_with_unicode_lowercase() {
        // ASCII-only folding would call these two different names.
        let b = zw::build(&[
            Entry::deflated("[Content_Types].xml", b"<Types/>"),
            Entry::deflated("word/document.xml", b"<w/>"),
            Entry::deflated("word/\u{C4}.xml", b"<a/>"),
            Entry::deflated("word/\u{E4}.xml", b"<b/>"),
        ]);
        refused(&b, "duplicate");
    }

    #[test]
    fn a_huge_local_offset_is_refused_without_a_panic() {
        let mut b = base();
        let cens = positions(&b, CEN_SIG);
        put32(&mut b, cens[0] + 42, 0xFFFF_FFF0);
        refused(&b, "no local header");
    }
}
