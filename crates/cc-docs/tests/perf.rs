// SPDX-License-Identifier: GPL-3.0-or-later
//! Prints render times of the test PDFs at 1920 px width (no assertions on time: the numbers
//! depend on the machine). Run with `cargo test -p cc-docs --test perf -- --nocapture`, once
//! in the dev profile and once with `--release`. The results are recorded in docs/documents.md.

use std::time::Instant;

use cc_docs::Document;

#[test]
fn render_times_at_1920_px() {
    let files = [
        "minimal.pdf",
        "classroom.pdf",
        "rotated_pages.pdf",
        "two_pages_one_rotated.pdf",
        "encrypted_empty_user_password.pdf",
    ];
    println!(
        "profile: {}",
        if cfg!(debug_assertions) {
            "debug (dev)"
        } else {
            "release"
        }
    );
    for name in files {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name);
        let bytes = std::fs::read(path).unwrap();
        let t = Instant::now();
        let d = Document::open(bytes).unwrap();
        let open = t.elapsed();
        for i in 0..d.page_count() {
            // First render includes font and cache set-up; then the median of 5 further renders.
            let t = Instant::now();
            let img = d.render_page(i, 1920).unwrap();
            let first = t.elapsed();
            let mut times: Vec<_> = (0..5)
                .map(|_| {
                    let t = Instant::now();
                    d.render_page(i, 1920).unwrap();
                    t.elapsed()
                })
                .collect();
            times.sort();
            println!(
                "{name} page {} -> {}x{}: open {:.1} ms, first {:.1} ms, median of 5 {:.1} ms",
                i + 1,
                img.width,
                img.height,
                open.as_secs_f64() * 1e3,
                first.as_secs_f64() * 1e3,
                times[2].as_secs_f64() * 1e3,
            );
        }
    }
}
