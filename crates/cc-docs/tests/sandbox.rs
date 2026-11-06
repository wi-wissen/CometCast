// SPDX-License-Identifier: GPL-3.0-or-later
//! The render worker process: memory bombs run THROUGH THE CHILD PATH with a memory limit.
//!
//! The hostile files come from an independent review (see `tests/data/PROVENANCE.md`); big
//! ones are generated at test time, nothing multi-megabyte is committed. Each test checks
//! three things: the call returns an error, the memory seen from the worker stays inside the
//! limit (and this test process stays small), and a good render still works afterwards.
//!
//! The tests need `sh` and `ulimit -v` (Linux, Android). Where they are missing the tests
//! print a note and return; macOS relies on the memory watchdog (`ps`), which these tests do
//! not exercise.

use std::io::Write;
use std::time::{Duration, Instant};

use cc_docs::{DocError, Document, Isolation, Limits, WorkerSpec};

fn worker() -> Isolation {
    Isolation::Process(WorkerSpec::new(env!("CARGO_BIN_EXE_cc-docs-worker")))
}

/// `false` (with a note on stdout) where `sh` or `ulimit -v` is missing.
fn have_sandbox() -> bool {
    let ok =
        cfg!(any(target_os = "linux", target_os = "android")) && cc_docs::worker_limits_available();
    if !ok {
        println!("NOTE: skipped, `sh` with `ulimit -v` is not available on this platform");
    }
    ok
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn own_peak_kib() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
}

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

/// A zlib stream of `mib` MiB of zeros, made in milliseconds: one raw-deflate chunk that ends
/// on a byte boundary (sync flush) is repeated, then a final empty block and the Adler-32
/// (zeros: `a = 1`, `b = length mod 65521`) close it.
fn zeros_zlib(mib: u64) -> Vec<u8> {
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(&vec![0u8; 1 << 20]).unwrap();
    enc.flush().unwrap();
    let chunk = enc.get_ref().clone();
    let mut out = vec![0x78, 0x9c];
    for _ in 0..mib {
        out.extend_from_slice(&chunk);
    }
    out.extend_from_slice(&[0x03, 0x00]);
    let total = mib << 20;
    let adler = ((total % 65521) as u32) << 16 | 1;
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

const IMG_DO: &[u8] = b"q 100 0 0 100 0 0 cm /Im Do Q";

fn image_pdf(w: u64, h: u64, cs: &str, body: &[u8]) -> Vec<u8> {
    one_page(
        "/MediaBox [0 0 100 100]",
        "/XObject << /Im 5 0 R >>",
        IMG_DO,
        &[stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /{cs} /BitsPerComponent 8 /Filter /FlateDecode"
            ),
            body,
        )],
    )
}

/// 2 GiB of zeros as a content stream (2 MB file).
fn flate_bomb_pdf() -> Vec<u8> {
    build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
        stream("/Filter /FlateDecode", &zeros_zlib(2048)),
    ])
}

/// Declared 2e9 x 2e9 gray image with a tiny body (hayro allocates `W * H` up front).
fn declared_huge_image_pdf() -> Vec<u8> {
    image_pdf(2_000_000_000, 2_000_000_000, "DeviceGray", &zeros_zlib(1))
}

/// Consistent 20000 x 20000 RGB image: 1.2 GB decoded from a 1.2 MB body.
fn rgb_20k_image_pdf() -> Vec<u8> {
    image_pdf(20_000, 20_000, "DeviceRGB", &zeros_zlib(1144))
}

/// Limits for the "sandbox only" tests: the pre-scan is switched off so the process limit
/// alone has to hold.
fn sandbox_only(mem_mib: u64) -> Limits {
    Limits {
        max_worker_memory_bytes: mem_mib << 20,
        max_image_pixels: u64::MAX,
        inflate_scan: false,
        open_timeout: Duration::from_secs(20),
        render_timeout: Duration::from_secs(20),
        ..Limits::default()
    }
}

/// A good render must still work after a failure.
fn assert_good_render_works() {
    let good = Document::open_with_options(data("classroom.pdf"), Limits::default(), worker())
        .expect("good document opens");
    let img = good.render_page(0, 640).expect("good render works");
    assert_eq!(img.width, 640);
    assert!(good.last_worker_peak_kib().is_some_and(|k| k > 0));
}

fn is_bounded_failure(e: &DocError) -> bool {
    matches!(
        e,
        DocError::WorkerFailed(_)
            | DocError::MemoryLimit
            | DocError::Timeout
            | DocError::Rejected(_)
    )
}

fn run_bomb(name: &str, bytes: Vec<u8>, mem_mib: u64) {
    let limits = sandbox_only(mem_mib);
    let t = Instant::now();
    let d = Document::open_with_options(bytes, limits, worker()).expect("opens (structure only)");
    let r = d.render_page(0, 400);
    let took = t.elapsed();
    let peak = d.last_worker_peak_kib().unwrap_or(0);
    println!(
        "{name}: limit {mem_mib} MiB -> {:?} in {took:?}; worker peak seen {} MiB; this process peak {} MiB",
        r.as_ref().map(|p| (p.width, p.height)),
        peak / 1024,
        own_peak_kib() / 1024
    );
    // hayro may refuse the bomb itself and draw a blank page; that is also bounded.
    if let Err(e) = r {
        assert!(is_bounded_failure(&e), "{name}: {e:?}");
    }
    // The worker never went far over its limit (VmHWM is read every 10 ms; address space is
    // capped by `ulimit -v`, so RSS cannot pass the limit).
    assert!(
        peak <= (mem_mib << 10) + (mem_mib << 10) / 4,
        "{name}: peak {peak} KiB"
    );
    assert!(took < Duration::from_secs(30), "{name} took {took:?}");
    // The bomb did not run in this process.
    assert!(
        own_peak_kib() < 600 * 1024,
        "parent grew to {} KiB",
        own_peak_kib()
    );
    assert_good_render_works();
}

#[test]
fn flate_bomb_2_gib_content_stream_is_stopped_by_the_memory_limit() {
    if !have_sandbox() {
        return;
    }
    let bytes = flate_bomb_pdf();
    println!("flate bomb file: {} bytes", bytes.len());
    run_bomb("flate bomb 2 GiB", bytes, 512);
}

#[test]
fn declared_2e9_by_2e9_image_is_stopped_by_the_memory_limit() {
    if !have_sandbox() {
        return;
    }
    run_bomb("image 2e9 x 2e9", declared_huge_image_pdf(), 512);
}

#[test]
fn consistent_20000_by_20000_rgb_image_is_stopped_by_the_memory_limit() {
    if !have_sandbox() {
        return;
    }
    let bytes = rgb_20k_image_pdf();
    println!("20k rgb file: {} bytes", bytes.len());
    run_bomb("image 20000 x 20000 RGB", bytes, 512);
}

#[test]
fn the_prescan_rejects_the_same_bombs_with_a_clear_error() {
    // The default limits: the scan runs inside the worker (also needs `sh` for the limits).
    if !have_sandbox() {
        return;
    }
    for (name, bytes) in [
        ("flate bomb", flate_bomb_pdf()),
        ("2e9 image", declared_huge_image_pdf()),
        ("20k rgb", rgb_20k_image_pdf()),
    ] {
        let t = Instant::now();
        let r = Document::open_with_options(bytes, Limits::default(), worker());
        println!("{name}: {:?} in {:?}", r.as_ref().map(|_| ()), t.elapsed());
        assert!(matches!(r, Err(DocError::Rejected(_))), "{name}: {r:?}");
        assert!(t.elapsed() < Duration::from_secs(10));
    }
    assert_good_render_works();
}

#[test]
fn a_64_mpx_image_is_accepted_and_its_peak_memory_fits_the_default_limit() {
    if !have_sandbox() {
        return;
    }
    // 8000 x 8000 RGB = 64 Mpx, the largest image the default limit lets through. The body
    // is 192 MB of zeros (1.5 MB deflated). The square page is drawn at 3840 x 3840 (measured by hand at 5760 x 5760, the output pixel
    // cap: worker peak 396 MiB, 26 s in a debug build; at 3840: 272 MiB).
    let bytes = image_pdf(8000, 8000, "DeviceRGB", &zeros_zlib(184));
    // Debug builds are slow: give the render a long deadline (this test is about memory).
    let limits = Limits {
        render_timeout: Duration::from_secs(240),
        ..Limits::default()
    };
    let d = Document::open_with_options(bytes, limits, worker()).expect("opens");
    let t = Instant::now();
    let r = d.render_page(0, 3840);
    println!(
        "64 Mpx RGB image on a 3840 px page: {:?} in {:?}, worker peak {} MiB",
        r.as_ref().map(|p| (p.width, p.height)),
        t.elapsed(),
        d.last_worker_peak_kib().unwrap_or(0) / 1024
    );
    // Either a picture or a bounded failure, never a hang or an unbounded peak.
    if let Err(e) = &r {
        assert!(is_bounded_failure(e), "{e:?}");
    }
    let peak = d.last_worker_peak_kib().unwrap_or(0);
    assert!(peak <= Limits::default().max_worker_memory_bytes / 1024);
}

#[test]
fn the_memory_limit_is_real_and_the_lowest_working_value_is_documented() {
    if !have_sandbox() {
        return;
    }
    // A 3840 px page needs about 75 MB resident (about 130 MiB of address space with the
    // 16 MiB thread stack). 64 MiB must fail, 192 MiB must work.
    let tight = Limits {
        max_worker_memory_bytes: 64 << 20,
        ..Limits::default()
    };
    let d = Document::open_with_options(data("minimal.pdf"), tight, worker());
    match d {
        Ok(d) => {
            let r = d.render_page(0, 3840);
            assert!(
                matches!(r, Err(DocError::WorkerFailed(_) | DocError::MemoryLimit)),
                "{r:?}"
            );
        }
        Err(e) => assert!(is_bounded_failure(&e), "{e:?}"),
    }
    let roomy = Limits {
        max_worker_memory_bytes: 192 << 20,
        ..Limits::default()
    };
    let d = Document::open_with_options(data("minimal.pdf"), roomy, worker()).unwrap();
    assert_eq!(d.render_page(0, 3840).unwrap().width, 3840);
    let small = Limits {
        max_worker_memory_bytes: 128 << 20,
        ..Limits::default()
    };
    let d = Document::open_with_options(data("classroom.pdf"), small, worker()).unwrap();
    assert_eq!(d.render_page(0, 1920).unwrap().width, 1920);
}

/// 1 000 000 tiny pages: 12 s and 900 MB when parsed; refused on the raw bytes now.
#[test]
fn a_million_pages_are_refused_before_parsing() {
    let n = 1_000_000usize;
    let mut kids = String::with_capacity(n * 9);
    for i in 0..n {
        kids.push_str(&(i + 3).to_string());
        kids.push_str(" 0 R ");
    }
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        // A lying /Count: the page objects below are counted instead.
        format!("<< /Type /Pages /Kids [{kids}] /Count 1 >>").into_bytes(),
    ];
    for _ in 0..n {
        objs.push(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".to_vec());
    }
    let bytes = build_pdf(&objs);
    drop(objs);
    println!("million pages file: {} bytes", bytes.len());
    let t = Instant::now();
    let r = Document::open_with_options(bytes, Limits::default(), Isolation::InProcess);
    println!("refused after {:?}", t.elapsed());
    assert!(matches!(r, Err(DocError::TooManyPages { .. })), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(20));
}

#[test]
fn nesting_bomb_is_refused_before_parsing() {
    let mut obj = b"<< /A ".to_vec();
    for _ in 0..100_000 {
        obj.extend(b"<< /A ");
    }
    obj.extend(b"1");
    for _ in 0..100_001 {
        obj.extend(b" >>");
    }
    let bytes = one_page("/MediaBox [0 0 100 100]", "/X 5 0 R", b"", &[obj]);
    let t = Instant::now();
    let r = Document::open_with_options(bytes, Limits::default(), Isolation::InProcess);
    assert!(matches!(r, Err(DocError::Rejected(_))), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(5));
}

/// A fake worker that sleeps: the wall-clock kill must not wait for it.
#[test]
fn a_stuck_worker_is_killed_at_the_deadline() {
    if !have_sandbox() {
        return;
    }
    let sleeper = Isolation::Process(WorkerSpec::new("sleep").arg("30"));
    let limits = Limits {
        open_timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let t = Instant::now();
    let r = Document::open_with_options(data("minimal.pdf"), limits, sleeper);
    assert_eq!(r.unwrap_err(), DocError::Timeout);
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
}

/// The parent does not believe the worker: wrong answers are errors, not allocations.
#[test]
fn bad_workers_give_errors() {
    if !have_sandbox() {
        return;
    }
    let sh = |script: &str| Isolation::Process(WorkerSpec::new("sh").arg("-c").arg(script));
    let open = |iso| Document::open_with_options(data("minimal.pdf"), Limits::default(), iso);
    // Garbage on stdout.
    let r = open(sh("echo garbage; exit 0"));
    assert!(matches!(r, Err(DocError::WorkerFailed(_))), "{r:?}");
    // A valid head that claims 2^40 pages (octal escapes: "CCDR", version 1, kind 0).
    let lie = r"printf 'CCDR\001\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000\377\000\000\000\000\000\000\000\000\000\000\000\000'";
    let r = open(sh(lie));
    assert!(matches!(r, Err(DocError::WorkerFailed(_))), "{r:?}");
    // No such program.
    let r = open(Isolation::Process(WorkerSpec::new(
        "/nonexistent/cc-docs-worker",
    )));
    assert!(matches!(r, Err(DocError::WorkerUnavailable(_))), "{r:?}");
    // Crash without an answer.
    let r = open(sh("kill -SEGV $$"));
    assert!(matches!(r, Err(DocError::WorkerFailed(_))), "{r:?}");
}

#[test]
fn page_claim_checks_run_before_a_process_is_started() {
    // The program does not exist; the error must still be the page claim, not "unavailable".
    let bytes = one_page("/MediaBox [0 0 10 10]", "", b"", &[]);
    let limits = Limits {
        max_pages: 0,
        ..Limits::default()
    };
    let r = Document::open_with_options(
        bytes,
        limits,
        Isolation::Process(WorkerSpec::new("/nonexistent/cc-docs-worker")),
    );
    assert!(matches!(r, Err(DocError::TooManyPages { .. })), "{r:?}");
}
