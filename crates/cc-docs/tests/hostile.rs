// SPDX-License-Identifier: GPL-3.0-or-later
//! Hostile and damaged input: the library must return an error or a picture, never panic out
//! of the API and never hang past the time limit. Memory is bounded by the render worker
//! process, not by this file: the memory bombs are in `sandbox.rs`.
//!
//! Test files are built in memory; nothing here is a real exploit, only structures that
//! stress a parser (truncation, mutation, deep nesting, cycles, huge numbers, compression
//! ratio, operation blow-up).

use std::time::{Duration, Instant};

use cc_docs::{DocError, Limits};

/// Tests run in parallel and start many workers; the default global cap (4) is for
/// applications. (The cap itself is tested in `worker_cap.rs`.)
fn lift_cap() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| cc_docs::set_max_concurrent_workers(64));
}

fn data(name: &str) -> Vec<u8> {
    lift_cap();
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(p).unwrap()
}

/// A PDF with an xref table from object bodies (1-based, object 1 must be the catalog).
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

/// One page with the given page dictionary entries and content stream.
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

/// Fast limits so a failing test cannot hold the suite for long.
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
    match open_pdf_limits(bytes, quick()) {
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

/// xorshift64: deterministic, no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

const FILES: [&str; 5] = [
    "minimal.pdf",
    "slides.pdf",
    "rotated_pages.pdf",
    "two_pages_one_rotated.pdf",
    "encrypted_empty_user_password.pdf",
];

#[test]
fn before_parsing_checks() {
    assert_eq!(open_pdf(vec![]).unwrap_err(), DocError::Empty);
    assert_eq!(open_pdf(b"hello".to_vec()).unwrap_err(), DocError::NotPdf);
    let big = Limits {
        max_file_bytes: 100,
        ..Limits::default()
    };
    assert!(matches!(
        open_pdf_limits(vec![b'%'; 101], big),
        Err(DocError::TooLarge {
            size: 101,
            max: 100
        })
    ));
    // Not a PDF header in the first 1024 bytes: refused without parsing.
    let mut late = vec![b' '; 2000];
    late.extend(b"%PDF-1.4\n");
    assert_eq!(open_pdf(late).unwrap_err(), DocError::NotPdf);
}

#[test]
fn truncated_files_never_panic() {
    for name in FILES {
        let full = data(name);
        let step = (full.len() / 60).max(1);
        for cut in (0..full.len()).step_by(step) {
            let errors = exercise(full[..cut].to_vec());
            assert_no_internal_failure(&format!("{name} cut at {cut}"), &errors);
        }
    }
}

#[test]
fn mutated_files_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut oks = 0;
    let mut total = 0;
    for name in FILES {
        let full = data(name);
        for _ in 0..80 {
            let mut m = full.clone();
            let flips = 1 + (rng.next() % 8) as usize;
            for _ in 0..flips {
                let pos = (rng.next() as usize) % m.len();
                m[pos] = (rng.next() & 0xFF) as u8;
            }
            let errors = exercise(m);
            assert_no_internal_failure(name, &errors);
            total += 1;
            if errors.is_empty() {
                oks += 1;
            }
        }
    }
    println!("mutated files: {oks} of {total} still rendered fully");
}

#[test]
fn deeply_nested_content_stream() {
    // 200 000 nested arrays inside the content stream.
    let mut c = vec![b'['; 200_000];
    c.extend(vec![b']'; 200_000]);
    c.extend(b" 0 0 1 rg 0 0 50 50 re f");
    let errors = exercise(one_page("/MediaBox [0 0 100 100]", "", &c, &[]));
    assert_no_internal_failure("nested arrays", &errors);
}

#[test]
fn deeply_nested_dictionaries_in_an_object() {
    let mut obj = b"<< /A ".to_vec();
    for _ in 0..100_000 {
        obj.extend(b"<< /A ");
    }
    obj.extend(b"1");
    for _ in 0..100_001 {
        obj.extend(b" >>");
    }
    let bytes = one_page("/MediaBox [0 0 100 100]", "/X 5 0 R", b"", &[obj]);
    assert_no_internal_failure("nested dicts", &exercise(bytes));
}

#[test]
fn page_tree_that_loops_back_to_itself() {
    // Pages node whose Kids contains itself and a page.
    let objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [2 0 R 3 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec(),
    ];
    let t = Instant::now();
    let errors = exercise(build_pdf(&objs));
    assert_no_internal_failure("page tree loop", &errors);
    assert!(t.elapsed() < Duration::from_secs(5));
}

#[test]
fn page_tree_two_nodes_pointing_at_each_other_and_parent_cycles() {
    let objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 /Parent 3 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [2 0 R] /Count 1 /Parent 2 0 R >>".to_vec(),
    ];
    assert_no_internal_failure("mutual loop", &exercise(build_pdf(&objs)));
}

#[test]
fn very_deep_page_tree() {
    // 20 000 nested Pages nodes, a page at the bottom.
    let depth = 20_000usize;
    let mut objs = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()];
    for i in 0..depth {
        let me = i + 2;
        objs.push(format!("<< /Type /Pages /Kids [{} 0 R] /Count 1 >>", me + 1).into_bytes());
    }
    objs.push(b"<< /Type /Page /MediaBox [0 0 100 100] >>".to_vec());
    assert_no_internal_failure("deep page tree", &exercise(build_pdf(&objs)));
}

#[test]
fn page_count_over_the_limit_is_refused() {
    let n = 300;
    let kids: String = (0..n).map(|i| format!("{} 0 R ", i + 3)).collect();
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {n} >>").into_bytes(),
    ];
    for _ in 0..n {
        objs.push(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec());
    }
    let bytes = build_pdf(&objs);
    let small = Limits {
        max_pages: 100,
        ..Limits::default()
    };
    assert!(matches!(
        open_pdf_limits(bytes.clone(), small),
        // The raw pre-check stops counting as soon as the limit is passed.
        Err(DocError::TooManyPages {
            count: 101..=300,
            max: 100
        })
    ));
    assert_eq!(open_pdf(bytes).unwrap().page_count(), 300);
}

#[test]
fn huge_media_box_does_not_allocate_a_huge_picture() {
    for mb in [
        "[0 0 1000000000 1000000000]",
        "[0 0 99999999999999999999999999999999 1]",
        "[0 0 1 99999999999999999999999999999999]",
        "[-1000000000 -1000000000 1000000000 1000000000]",
    ] {
        let bytes = one_page(
            &format!("/MediaBox {mb}"),
            "",
            b"1 0 0 rg 0 0 5000000 5000000 re f",
            &[],
        );
        let t = Instant::now();
        let d = open_pdf_limits(bytes, quick()).unwrap();
        // Fits into the output limits by scaling (or is refused), never a 1e9 pixel buffer.
        match d.render_page(0, 1920) {
            Ok(img) => assert!(u64::from(img.width) * u64::from(img.height) <= 7680 * 4320),
            Err(e) => assert!(
                matches!(
                    e,
                    DocError::OutputTooLarge { .. } | DocError::InvalidPageSize
                ),
                "{mb}: {e:?}"
            ),
        }
        assert!(
            t.elapsed() < Duration::from_secs(5),
            "{mb} took {:?}",
            t.elapsed()
        );
    }
}

#[test]
fn degenerate_media_boxes() {
    for mb in [
        "[0 0 0 0]",
        "[10 10 10 10]",
        "[0 0 -5 -5]",
        "[0 0 0.0000001 100]",
        "[]",
        "[0 0]",
        "(x)",
    ] {
        let bytes = one_page(&format!("/MediaBox {mb}"), "", b"0 g 0 0 5 5 re f", &[]);
        assert_no_internal_failure(mb, &exercise(bytes));
    }
}

#[test]
fn form_xobject_that_draws_itself() {
    let form = stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /F 5 0 R >> >>",
        b"1 0 0 rg 0 0 10 10 re f /F Do /F Do",
    );
    let bytes = one_page(
        "/MediaBox [0 0 100 100]",
        "/XObject << /F 5 0 R >>",
        b"/F Do",
        &[form],
    );
    let t = Instant::now();
    let errors = exercise(bytes);
    println!(
        "self-referencing form: {:?}, errors {errors:?}",
        t.elapsed()
    );
    assert_no_internal_failure("self form", &errors);
}

#[test]
fn garbage_bytes_after_a_valid_header() {
    let mut rng = Rng(12345);
    for _ in 0..50 {
        let mut b = b"%PDF-1.7\n".to_vec();
        for _ in 0..2000 {
            b.push((rng.next() & 0xFF) as u8);
        }
        let errors = exercise(b);
        assert_no_internal_failure("garbage", &errors);
    }
}

/// Always the worker built for this test run, never discovery next to the test executable.
fn explicit_worker() -> cc_docs::Isolation {
    cc_docs::Isolation::Process(cc_docs::WorkerSpec::new(env!(
        "CARGO_BIN_EXE_cc-docs-worker"
    )))
}

#[allow(dead_code)]
fn open_pdf(bytes: Vec<u8>) -> Result<cc_docs::Document, cc_docs::DocError> {
    open_pdf_limits(bytes, cc_docs::Limits::default())
}

fn open_pdf_limits(
    bytes: Vec<u8>,
    limits: cc_docs::Limits,
) -> Result<cc_docs::Document, cc_docs::DocError> {
    cc_docs::Document::open_with_options(bytes, limits, explicit_worker())
}
