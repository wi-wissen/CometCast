// SPDX-License-Identifier: GPL-3.0-or-later
//! Review finding: a 725 byte file (`pattern_self.pdf`, a tiling pattern that uses itself)
//! made four renders time out, and with the old global cap of abandoned threads `open`
//! returned `Busy` for every later document, forever.
//!
//! Two paths are checked: the child process (nothing is abandoned, nothing piles up) and the
//! in-process engine (a timed-out document is poisoned, so one bad file costs one thread).
//! The in-process case leaves a spinning thread behind, so this test has its own process.

use std::time::{Duration, Instant};

use cc_docs::{DocError, Document, Isolation, Limits, RenderOptions, WorkerSpec};

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

fn stream(dict: &str, data: &str) -> Vec<u8> {
    let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend(data.bytes());
    v.extend(b"\nendstream");
    v
}

/// The reviewer's `pattern_self.pdf` (gen2.py): a tiling pattern whose resources name itself.
fn pattern_self_pdf() -> Vec<u8> {
    build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /Pattern << /P 5 0 R >> >> >>".to_vec(),
        stream("", "/Pattern cs /P scn 0 0 100 100 re f"),
        stream(
            "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 5 5] /XStep 5 /YStep 5 /Resources << /Pattern << /P 5 0 R >> >>",
            "/Pattern cs /P scn 0 0 5 5 re f",
        ),
    ])
}

/// The pattern costs time in proportion to the pixels: slow at this width (seconds in a
/// debug build, fast at 400).
const WIDTH: u32 = 3840;

fn good() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/minimal.pdf"
    ))
    .unwrap()
}

/// Tests run in parallel and start many workers; the default global cap (4) is for
/// applications. (The cap itself is tested in `worker_cap.rs`.)
fn lift_cap() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| cc_docs::set_max_concurrent_workers(64));
}

fn limits() -> Limits {
    lift_cap();
    Limits {
        open_timeout: Duration::from_millis(800),
        render_timeout: Duration::from_millis(800),
        ..Limits::default()
    }
}

/// Both engines in one test, in this order: the process path first (leaves nothing behind).
#[test]
fn a_hostile_file_cannot_make_open_return_busy() {
    let bomb = pattern_self_pdf();
    println!("pattern_self.pdf: {} bytes", bomb.len());
    let worker = Isolation::Process(WorkerSpec::new(env!("CARGO_BIN_EXE_cc-docs-worker")));

    // Child process: any number of tries, each ends at the deadline or with an error.
    for i in 0..6 {
        let t = Instant::now();
        let r = Document::open_with_options(bomb.clone(), limits(), worker.clone())
            .and_then(|d| d.render_page(0, WIDTH).map(|_| ()));
        println!("process try {i}: {r:?} in {:?}", t.elapsed());
        assert!(
            !matches!(r, Err(DocError::Busy | DocError::Poisoned)),
            "{r:?}"
        );
        assert!(t.elapsed() < Duration::from_secs(5));
    }
    assert_eq!(cc_docs::abandoned_renders(), 0);
    assert!(Document::open_with_options(good(), limits(), worker).is_ok());

    // In process: the first timeout poisons the file; later opens say so (not Busy).
    let mut timed_out = false;
    for i in 0..6 {
        let r = Document::open_with_options(bomb.clone(), limits(), Isolation::InProcess).and_then(
            |d| {
                d.render_page_with(
                    0,
                    WIDTH,
                    &RenderOptions {
                        timeout: Some(Duration::from_millis(500)),
                        cancel: None,
                    },
                )
                .map(|_| ())
            },
        );
        println!(
            "in-process try {i}: {r:?}, abandoned {}",
            cc_docs::abandoned_renders()
        );
        match r {
            Err(DocError::Timeout) => {
                assert!(!timed_out, "second timeout: the file was not poisoned");
                timed_out = true;
            }
            Err(DocError::Poisoned) => assert!(timed_out),
            Err(DocError::Busy) => panic!("Busy: a single file filled the cap"),
            // UNCERTAIN: a fast machine (release build) may finish the render in time.
            Ok(()) | Err(_) => {}
        }
        assert!(cc_docs::abandoned_renders() <= 1);
    }
    // An unrelated good document still opens and renders.
    let d = Document::open_with_options(good(), limits(), Isolation::InProcess).unwrap();
    assert!(d.render_page(0, 100).is_ok());
}
