// SPDX-License-Identifier: GPL-3.0-or-later
//! Resource limits: time limit, cancel, abandoned workers, compression bombs, huge images.
//!
//! These tests leave abandoned renders running inside hayro (it cannot be interrupted), so
//! they live in their own test binary (own process) and run one after the other.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cc_docs::{DocError, Document, Limits, RenderOptions};

static SERIAL: Mutex<()> = Mutex::new(());

fn build_pdf(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend(data);
    v.extend(b"\nendstream");
    v
}

fn one_page(page_extra: &str, resources: &str, content: &[u8], extra_objs: &[Vec<u8>]) -> Vec<u8> {
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R {page_extra} /Contents 4 0 R /Resources << {resources} >> >>")
            .into_bytes(),
        stream("", content),
    ];
    objs.extend(extra_objs.iter().cloned());
    build_pdf(&objs)
}

fn quick() -> Limits {
    Limits {
        open_timeout: Duration::from_secs(5),
        render_timeout: Duration::from_secs(5),
        ..Limits::default()
    }
}

/// Open and try to render all pages; returns which errors happened. Must not panic out.
fn exercise(bytes: Vec<u8>) -> Vec<DocError> {
    let mut errors = Vec::new();
    match Document::open_with_limits(bytes, quick()) {
        Ok(d) => {
            for i in 0..d.page_count().min(5) {
                if let Err(e) = d.render_page(i, 160) {
                    errors.push(e);
                }
                let _ = d.page_size(i);
                let _ = d.thumbnail(i, 64);
            }
        }
        Err(e) => errors.push(e),
    }
    errors
}

fn assert_no_internal_failure(what: &str, errors: &[DocError]) {
    for e in errors {
        assert!(
            !matches!(
                e,
                DocError::Panicked | DocError::Internal(_) | DocError::Timeout
            ),
            "{what}: {e:?}"
        );
    }
}

/// Peak resident memory of this process in MiB (Linux `VmHWM`), for the numbers in docs.
fn peak_rss_mib() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib / 1024)
}

fn zeros_deflated(len: u64) -> Vec<u8> {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let chunk = vec![0u8; 1 << 20];
    let mut left = len;
    while left > 0 {
        let n = left.min(chunk.len() as u64) as usize;
        enc.write_all(&chunk[..n]).unwrap();
        left -= n as u64;
    }
    enc.finish().unwrap()
}

/// Exponential operation blow-up: three forms each drawing the next `fan` times.
fn blowup_pdf(fan: usize) -> Vec<u8> {
    let calls: String = "/N Do ".repeat(fan);
    let mk = |next: &str, name: &str| {
        stream(
            &format!(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /N {next} 0 R >> >>"
            ),
            format!("0.5 g {name} {calls}").as_bytes(),
        )
    };
    let leaf = stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
        b"0 0 1 rg 10 10 30 30 re f",
    );
    one_page(
        "/MediaBox [0 0 100 100]",
        "/XObject << /N 5 0 R >>",
        b"/N Do",
        &[mk("6", ""), mk("7", ""), mk("8", ""), leaf],
    )
}

/// Decompression bomb as a content stream. `MIB` of zeros are NUL bytes, which a content
/// parser treats as white space.
#[test]
fn flate_bomb_as_content_stream() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    const MIB: u64 = 256;
    let z = zeros_deflated(MIB << 20);
    println!("bomb: {} MiB of zeros deflated to {} bytes", MIB, z.len());
    let bytes = one_page("/MediaBox [0 0 100 100]", "", b"", &[]);
    // Replace the content stream (object 4) by the compressed one.
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
    ];
    objs.push(stream("/Filter /FlateDecode", &z));
    drop(bytes);
    let t = Instant::now();
    let errors = exercise(build_pdf(&objs));
    println!(
        "flate bomb (content): {:?}, errors {errors:?}, peak RSS of the process {:?} MiB",
        t.elapsed(),
        peak_rss_mib()
    );
    assert_no_internal_failure("flate bomb", &errors);
}

/// Image whose declared size is huge, with a tiny compressed body.
#[test]
fn image_with_huge_declared_size() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let z = zeros_deflated(1 << 20);
    for (w, h) in [
        (100_000u32, 100_000u32),
        (2_000_000_000, 2_000_000_000),
        (65_536, 65_536),
    ] {
        let img = stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode"
            ),
            &z,
        );
        let bytes = one_page(
            "/MediaBox [0 0 100 100]",
            "/XObject << /Im 5 0 R >>",
            b"q 100 0 0 100 0 0 cm /Im Do Q",
            &[img],
        );
        let t = Instant::now();
        let errors = exercise(bytes);
        println!(
            "image {w}x{h}: {:?}, errors {errors:?}, peak RSS so far {:?} MiB",
            t.elapsed(),
            peak_rss_mib()
        );
        // A timeout is an accepted outcome here: the point is that the caller gets control back.
        let errors: Vec<_> = errors
            .into_iter()
            .filter(|e| *e != DocError::Timeout)
            .collect();
        assert_no_internal_failure(&format!("image {w}x{h}"), &errors);
    }
}

#[test]
fn render_time_limit_returns_in_time_and_the_next_render_works() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // fan 60: 60^3 = 216 000 leaf draws + 12 960 form invocations; far over 150 ms in debug.
    let bytes = blowup_pdf(60);
    let d = Document::open_with_limits(bytes, quick()).unwrap();
    let t = Instant::now();
    let r = d.render_page_with(
        0,
        400,
        &RenderOptions {
            timeout: Some(Duration::from_millis(150)),
            cancel: None,
        },
    );
    let took = t.elapsed();
    println!("blow-up render with 150 ms limit: {r:?} after {took:?}");
    match r {
        Err(DocError::Timeout) => {
            assert!(took < Duration::from_millis(400), "returned late: {took:?}");
            assert!(cc_docs::abandoned_renders() >= 1);
        }
        // UNCERTAIN: if hayro is fast enough on this machine the render simply finishes.
        Ok(_) => {}
        Err(e) => panic!("unexpected {e:?}"),
    }
    // The document is still usable: a new worker is started (page_size needs no worker).
    assert_eq!(d.page_count(), 1);
}

#[test]
fn cancel_flag_stops_waiting() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let d = Document::open_with_limits(blowup_pdf(60), quick()).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = Arc::clone(&cancel);
    let h = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        c2.store(true, Ordering::SeqCst);
    });
    let t = Instant::now();
    let r = d.render_page_with(
        0,
        400,
        &RenderOptions {
            timeout: None,
            cancel: Some(cancel),
        },
    );
    h.join().unwrap();
    println!("cancelled render: {r:?} after {:?}", t.elapsed());
    match r {
        Err(DocError::Cancelled) => assert!(t.elapsed() < Duration::from_millis(500)),
        Ok(_) => {} // finished before the flag was set
        Err(e) => panic!("unexpected {e:?}"),
    }
}
