// SPDX-License-Identifier: GPL-3.0-or-later
//! Binary property list helpers (AirPlay request and answer bodies).

use std::io::Cursor;

use plist::{Dictionary, Value};

use crate::error::{Result, SenderError, clean};

/// Plists we accept from a receiver are at most this big.
pub const MAX_PLIST_BYTES: usize = 1024 * 1024;

/// Deepest container nesting we accept (real AirPlay bodies are 3 to 5 levels deep).
///
/// The `plist` crate parses recursively and has no depth limit of its own: about 60,000
/// nested arrays (400 KB) overflow the stack of the process, which no code can catch. So
/// [`decode_dict`] first checks the nesting and the declared counts without recursion
/// (`check_binary`, `check_xml`) and only then calls the crate, on a thread with a big
/// stack as a second line of defence.
pub const MAX_DEPTH: usize = 32;

/// Stack size of the thread that runs the `plist` crate.
const PARSE_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Content type of binary plists.
pub const BPLIST_TYPE: &str = "application/x-apple-binary-plist";

/// Encode a dictionary as a binary plist.
pub fn encode(dict: Dictionary) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    Value::Dictionary(dict)
        .to_writer_binary(&mut out)
        .map_err(|e| SenderError::Input(format!("cannot encode plist: {e}")))?;
    Ok(out)
}

/// Decode a binary or XML plist whose top level is a dictionary.
pub fn decode_dict(data: &[u8]) -> Result<Dictionary> {
    if data.len() > MAX_PLIST_BYTES {
        return Err(SenderError::TooLarge("property list"));
    }
    if data.starts_with(b"bplist00") {
        check_binary(data)?;
    } else {
        check_xml(data)?;
    }
    // Defence in depth: a big stack, and a panic inside the crate becomes an error.
    let parsed = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(PARSE_STACK_BYTES)
            .spawn_scoped(scope, || Value::from_reader(Cursor::new(data)))
            .map_err(|e| SenderError::Io(format!("cannot start the plist parser: {e}")))?
            .join()
            .map_err(|_| SenderError::Protocol("the property list parser failed".into()))
    })?;
    match parsed {
        Ok(Value::Dictionary(d)) => Ok(d),
        Ok(_) => Err(SenderError::Protocol(
            "property list is not a dictionary".into(),
        )),
        Err(e) => Err(SenderError::Protocol(format!(
            "bad property list: {}",
            clean(&e.to_string(), 120)
        ))),
    }
}

fn bad(what: &str) -> SenderError {
    SenderError::Protocol(format!("bad property list: {what}"))
}

/// Big-endian unsigned integer of 1 to 8 bytes.
fn be(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |a, b| (a << 8) | u64::from(*b))
}

/// Walk a `bplist00` without recursion: checks the trailer, that every offset and every
/// declared length lies inside the data, that no container is reached twice (this rules
/// out cycles and exponential sharing) and that the nesting is at most [`MAX_DEPTH`].
fn check_binary(data: &[u8]) -> Result<()> {
    if data.len() < 8 + 1 + 32 {
        return Err(bad("binary list too short"));
    }
    let trailer = &data[data.len() - 32..];
    let off_size = trailer[6] as usize;
    let ref_size = trailer[7] as usize;
    let count = be(&trailer[8..16]);
    let top = be(&trailer[16..24]);
    let table = be(&trailer[24..32]);
    if !(1..=8).contains(&off_size) || !(1..=8).contains(&ref_size) {
        return Err(bad("bad integer sizes"));
    }
    let end_of_objects = (data.len() - 32) as u64;
    // The offset table has `count` entries and must fit; this bounds `count` by the data.
    let table_len = count
        .checked_mul(off_size as u64)
        .ok_or_else(|| bad("object count"))?;
    if count == 0
        || top >= count
        || table < 9
        || table.checked_add(table_len).is_none_or(|e| e > end_of_objects)
    {
        return Err(bad("bad offset table"));
    }
    let (count, table) = (count as usize, table as usize);
    // Offset of object `i`, checked to be inside the object area.
    let offset_of = |i: u64| -> Result<usize> {
        if i >= count as u64 {
            return Err(bad("object reference out of range"));
        }
        let at = table + i as usize * off_size;
        let o = be(&data[at..at + off_size]);
        if o < 8 || o >= table as u64 {
            return Err(bad("object offset out of range"));
        }
        Ok(o as usize)
    };
    let mut seen = vec![false; count];
    let mut stack: Vec<(u64, usize)> = vec![(top, 1)];
    while let Some((idx, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(bad("nesting too deep"));
        }
        let pos = offset_of(idx)?;
        let marker = data[pos];
        let (kind, low) = (marker >> 4, (marker & 0x0F) as usize);
        // Length of the object and where its payload starts.
        let (len, payload) = if matches!(kind, 0x4 | 0x5 | 0x6 | 0xA | 0xC | 0xD) && low == 0x0F {
            let m = *data.get(pos + 1).ok_or_else(|| bad("truncated length"))?;
            if m >> 4 != 0x1 || (m & 0x0F) > 3 {
                return Err(bad("bad length marker"));
            }
            let n = 1usize << (m & 0x0F);
            let raw = data
                .get(pos + 2..pos + 2 + n)
                .ok_or_else(|| bad("truncated length"))?;
            (be(raw), pos + 2 + n)
        } else {
            (low as u64, pos + 1)
        };
        // Bytes the payload needs, as `u64` so that a huge declared length cannot overflow.
        let (unit, elements): (u64, u64) = match kind {
            0x0 => (0, 0),
            0x1 | 0x2 => {
                if low > 4 {
                    return Err(bad("bad number size"));
                }
                (1u64 << low, 1)
            }
            0x3 => (8, 1),
            0x4 | 0x5 => (1, len),
            0x6 => (2, len),
            0x8 => (1, low as u64 + 1),
            0xA | 0xC => (ref_size as u64, len),
            0xD => (
                ref_size as u64,
                len.checked_mul(2).ok_or_else(|| bad("length"))?,
            ),
            _ => return Err(bad("unknown object type")),
        };
        let need = unit.checked_mul(elements).ok_or_else(|| bad("length"))?;
        if (payload as u64).saturating_add(need) > table as u64 {
            return Err(bad("object runs past the data"));
        }
        if matches!(kind, 0xA | 0xC | 0xD) {
            if std::mem::replace(&mut seen[idx as usize], true) {
                return Err(bad("a container is used twice"));
            }
            let refs = &data[payload..payload + need as usize];
            for r in refs.chunks_exact(ref_size) {
                stack.push((be(r), depth + 1));
            }
        }
    }
    Ok(())
}

/// Scan an XML plist for the deepest nesting of `<dict>` and `<array>` without building
/// anything. Comments, CDATA sections and processing instructions are skipped the way a
/// parser skips them, so text inside them cannot hide a tag.
fn check_xml(data: &[u8]) -> Result<()> {
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < data.len() {
        if data[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &data[i..];
        let skip = |open: &[u8], close: &[u8]| -> Option<usize> {
            rest.starts_with(open).then(|| {
                rest.windows(close.len())
                    .position(|w| w == close)
                    .map_or(rest.len(), |p| p + close.len())
            })
        };
        if let Some(n) = skip(b"<!--", b"-->")
            .or_else(|| skip(b"<![CDATA[", b"]]>"))
            .or_else(|| skip(b"<?", b"?>"))
        {
            i += n;
            continue;
        }
        // An ordinary tag or a declaration: up to the `>` that is not inside quotes.
        let mut j = 1usize;
        let mut quote = 0u8;
        while j < rest.len() {
            let c = rest[j];
            if quote != 0 {
                if c == quote {
                    quote = 0;
                }
            } else if c == b'"' || c == b'\'' {
                quote = c;
            } else if c == b'>' {
                break;
            }
            j += 1;
        }
        let tag = &rest[1..j.min(rest.len())];
        let closing = tag.first() == Some(&b'/');
        let self_closing = tag.last() == Some(&b'/');
        let name_start = usize::from(closing);
        let name_end = tag[name_start..]
            .iter()
            .position(|c| c.is_ascii_whitespace() || *c == b'/')
            .map_or(tag.len(), |p| p + name_start);
        let name = &tag[name_start..name_end];
        if (name == b"dict" || name == b"array") && !self_closing {
            if closing {
                depth = depth.saturating_sub(1);
            } else {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(bad("nesting too deep"));
                }
            }
        }
        i += j + 1;
    }
    Ok(())
}

/// A byte value of a dictionary.
pub fn data<'a>(d: &'a Dictionary, key: &str) -> Option<&'a [u8]> {
    match d.get(key) {
        Some(Value::Data(b)) => Some(b.as_slice()),
        _ => None,
    }
}

/// A real number or integer value as `f64`.
pub fn number(d: &Dictionary, key: &str) -> Option<f64> {
    match d.get(key) {
        Some(Value::Real(r)) => Some(*r),
        Some(Value::Integer(i)) => i.as_signed().map(|v| v as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut d = Dictionary::new();
        d.insert("pk".into(), Value::Data(vec![1, 2, 3]));
        d.insert("duration".into(), Value::Real(12.5));
        let back = decode_dict(&encode(d).unwrap()).unwrap();
        assert_eq!(data(&back, "pk"), Some(&[1u8, 2, 3][..]));
        assert_eq!(number(&back, "duration"), Some(12.5));
        assert_eq!(number(&back, "pk"), None);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode_dict(b"bplist00garbage").is_err());
        assert!(decode_dict(b"").is_err());
        assert!(decode_dict(&vec![0u8; MAX_PLIST_BYTES + 1]).is_err());
        let mut arr = Vec::new();
        Value::Array(vec![]).to_writer_binary(&mut arr).unwrap();
        assert!(decode_dict(&arr).is_err());
    }

    /// The reviewer's probe: `depth` nested one-element arrays, as a `bplist00`.
    fn bplist_nested(depth: usize) -> Vec<u8> {
        let n = depth + 1;
        let refsz: usize = if n < 256 {
            1
        } else if n < 65536 {
            2
        } else {
            4
        };
        let mut out = b"bplist00".to_vec();
        let mut offs = Vec::new();
        for i in 0..depth {
            offs.push(out.len());
            out.push(0xA1);
            out.extend(&((i + 1) as u32).to_be_bytes()[4 - refsz..]);
        }
        offs.push(out.len());
        out.extend([0x10, 0x00]);
        let table = out.len();
        for o in &offs {
            out.extend((*o as u32).to_be_bytes());
        }
        out.extend([0u8; 6]);
        out.push(4);
        out.push(refsz as u8);
        out.extend((n as u64).to_be_bytes());
        out.extend(0u64.to_be_bytes());
        out.extend((table as u64).to_be_bytes());
        out
    }

    #[test]
    fn deeply_nested_binary_plist_is_rejected_not_a_stack_overflow() {
        let d = bplist_nested(60_000);
        assert!(d.len() < MAX_PLIST_BYTES);
        assert!(decode_dict(&d).is_err());
        let err = |d: &[u8]| decode_dict(d).unwrap_err().to_string();
        assert!(err(&bplist_nested(MAX_DEPTH + 5)).contains("too deep"));
        // Shallow nesting passes the check (the top level is an array, hence still an error).
        assert!(!err(&bplist_nested(5)).contains("too deep"));
    }

    #[test]
    fn deeply_nested_xml_plist_is_rejected() {
        let n = 60_000;
        let mut s = String::from("<?xml version=\"1.0\"?><plist version=\"1.0\">");
        s.push_str(&"<array>".repeat(n));
        s.push_str(&"</array>".repeat(n));
        s.push_str("</plist>");
        assert!(s.len() < MAX_PLIST_BYTES);
        let e = decode_dict(s.as_bytes()).unwrap_err().to_string();
        assert!(e.contains("too deep"), "{e}");
        // Tags hidden in comments or CDATA do not change the count.
        let hidden = format!(
            "<plist><!-- {} --><dict><key>a</key><string><![CDATA[{}]]></string></dict></plist>",
            "</dict>".repeat(100),
            "<dict>".repeat(100)
        );
        assert!(decode_dict(hidden.as_bytes()).is_ok());
    }

    #[test]
    fn xml_dictionary_still_decodes() {
        let xml = b"<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>a</key><array><dict/><array/></array><key>d</key><real>2.5</real></dict></plist>";
        let d = decode_dict(xml).unwrap();
        assert_eq!(number(&d, "d"), Some(2.5));
    }

    fn trailer(out: &mut Vec<u8>, count: u64, table: usize) {
        out.extend([0u8; 6]);
        out.extend([4, 1]);
        out.extend(count.to_be_bytes());
        out.extend(0u64.to_be_bytes());
        out.extend((table as u64).to_be_bytes());
    }

    #[test]
    fn hostile_binary_plists_are_errors() {
        // A declared array length of 2^40 in a tiny file.
        let mut big = b"bplist00".to_vec();
        let off0 = big.len();
        big.push(0xAF);
        big.push(0x13);
        big.extend(0x0000_00FF_FFFF_FFFFu64.to_be_bytes());
        let table = big.len();
        big.extend((off0 as u32).to_be_bytes());
        trailer(&mut big, 1, table);
        assert!(decode_dict(&big).is_err());
        // An array that contains itself.
        let mut cyc = b"bplist00".to_vec();
        cyc.extend([0xA1, 0x00]);
        let table = cyc.len();
        cyc.extend(8u32.to_be_bytes());
        trailer(&mut cyc, 1, table);
        assert!(decode_dict(&cyc).is_err());
        // A huge object count.
        let mut many = cyc.clone();
        let l = many.len();
        many[l - 24..l - 16].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(decode_dict(&many).is_err());
    }
}
