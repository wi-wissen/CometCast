// SPDX-License-Identifier: GPL-3.0-or-later
//! Pre-scan findings of the second security review: incremental updates, fake `stream`
//! tokens, dimensions over `u32::MAX`, bit depth and colour components, inline images.
//! All THROUGH THE WORKER PROCESS where the scan runs.

use std::io::Write;

use cc_docs::{DocError, Document, Isolation, Limits, WorkerSpec};

fn worker() -> Isolation {
    Isolation::Process(WorkerSpec::new(env!("CARGO_BIN_EXE_cc-docs-worker")))
}

fn open(bytes: Vec<u8>, limits: Limits) -> Result<Document, DocError> {
    Document::open_with_options(bytes, limits, worker())
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

fn page_pdf(resources: &str, content: &[u8], extra: &[Vec<u8>]) -> Vec<u8> {
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << {resources} >> >>"
        )
        .into_bytes(),
        stream("", content),
    ];
    objs.extend(extra.iter().cloned());
    build_pdf(&objs)
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn assert_rejected(r: Result<Document, DocError>, what: &str) {
    assert!(
        matches!(r, Err(DocError::Rejected(_))),
        "{what}: expected Rejected, got {r:?}"
    );
}

/// A file with `n` pages and an incremental update that writes every page object again.
fn incremental_pdf(n: usize) -> Vec<u8> {
    let kids: String = (0..n).map(|i| format!("{} 0 R ", i + 3)).collect();
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {n} >>").into_bytes(),
    ];
    for _ in 0..n {
        objs.push(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec());
    }
    let mut out = build_pdf(&objs);
    let first_xref = {
        let t = String::from_utf8_lossy(&out).into_owned();
        let k = t.rfind("startxref\n").unwrap() + "startxref\n".len();
        t[k..].lines().next().unwrap().parse::<usize>().unwrap()
    };
    // The update: new copies of all page objects, a new cross-reference section with /Prev.
    let mut offsets = Vec::new();
    for i in 0..n {
        offsets.push(out.len());
        out.extend(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n",
                i + 3
            )
            .bytes(),
        );
    }
    let xref2 = out.len();
    out.extend(format!("xref\n3 {n}\n").bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Prev {first_xref} >>\nstartxref\n{xref2}\n%%EOF\n",
            n + 3
        )
        .bytes(),
    );
    out
}

#[test]
fn an_incremental_update_does_not_double_the_page_count() {
    // 6 pages, written twice: 12 `/Type /Page` tokens. A limit of 6 pages must accept it.
    let limits = Limits {
        max_pages: 6,
        ..Limits::default()
    };
    let d = open(incremental_pdf(6), limits).expect("opens: the pages are counted once");
    assert_eq!(d.page_count(), 6);
    // The update is what is shown (200 x 200 pt).
    assert_eq!(d.page_size(0).unwrap().width_pt, 200.0);
    // And the limit still bites on the real number.
    let tight = Limits {
        max_pages: 5,
        ..Limits::default()
    };
    assert!(matches!(
        open(incremental_pdf(6), tight),
        Err(DocError::TooManyPages { .. })
    ));
}

#[test]
fn a_big_count_in_an_outline_is_not_a_page_claim() {
    // `/Count 100000` on an outline root; one page.
    let pdf = build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec(),
        b"<< /Type /Outlines /Count 100000 >>".to_vec(),
    ]);
    assert_eq!(open(pdf, Limits::default()).unwrap().page_count(), 1);
}

#[test]
fn a_lying_page_count_cannot_make_a_good_file_fail_or_a_bad_one_pass() {
    // /Count says 9 million but the tree holds one page: one page.
    let pdf = build_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 9000000 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec(),
    ]);
    assert_eq!(open(pdf, Limits::default()).unwrap().page_count(), 1);
}

#[test]
fn a_fake_stream_token_does_not_hide_deep_nesting() {
    let mut deep = Vec::new();
    for _ in 0..1000 {
        deep.extend(b"<< /A ");
    }
    deep.extend(b"1");
    for _ in 0..1000 {
        deep.extend(b" >>");
    }
    // The object starts with a nested dictionary that contains the word "stream" followed by
    // a line break; the deep part follows, and an "endstream" later closes the fake body.
    let mut obj = b"<< /X << /Note stream\n /Y 1 >> /Deep ".to_vec();
    obj.extend(&deep);
    obj.extend(b" >> endstream");
    let pdf = page_pdf("/X 5 0 R", b"", &[obj]);
    assert_rejected(open(pdf, Limits::default()), "fake stream token");
}

#[test]
fn width_over_u32_max_is_too_large_not_zero() {
    // 5e9 x 1: before, `Width` that does not fit u32 counted as 0 and the product was 0.
    let img = stream(
        "/Type /XObject /Subtype /Image /Width 5000000000 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
        b"x",
    );
    let pdf = page_pdf(
        "/XObject << /Im 5 0 R >>",
        b"q 100 0 0 100 0 0 cm /Im Do Q",
        &[img],
    );
    assert_rejected(open(pdf, Limits::default()), "width 5e9");
    // A negative size and a real number that is huge.
    for (w, h) in [("-1", "5"), ("1000000000000.5", "1"), ("4294967296", "1")] {
        let img = stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8"
            ),
            b"x",
        );
        let pdf = page_pdf("/XObject << /Im 5 0 R >>", b"q /Im Do Q", &[img]);
        let r = open(pdf, Limits::default());
        // A parser that refuses the file instead is also fine.
        assert!(
            matches!(r, Err(DocError::Rejected(_) | DocError::Invalid)),
            "{w} x {h}: {r:?}"
        );
    }
}

#[test]
fn bit_depth_and_components_count_in_the_size_estimate() {
    // 8000 x 8000 is exactly the area limit. 8 bit gray is fine, 16 bit CMYK is 512 MiB.
    let make = |cs: &str, bpc: u32| {
        let img = stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width 8000 /Height 8000 /ColorSpace /{cs} /BitsPerComponent {bpc} /Filter /FlateDecode"
            ),
            &zlib(&[0u8; 64]),
        );
        page_pdf(
            "/XObject << /Im 5 0 R >>",
            b"q 100 0 0 100 0 0 cm /Im Do Q",
            &[img],
        )
    };
    assert_rejected(
        open(make("DeviceCMYK", 16), Limits::default()),
        "16 bit CMYK",
    );
    // Allowed by area and bytes (8 bit CMYK = 256 MiB = the stream limit): no pre-scan error
    // (the tiny body then renders as a blank or failed image, which is not the scan's concern).
    let r = open(make("DeviceCMYK", 8), Limits::default());
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn an_inline_image_with_a_huge_declared_size_is_found() {
    let content = b"q BI /W 100000 /H 100000 /BPC 8 /CS /G ID abcd EI Q";
    // Plain content stream.
    let plain = page_pdf("", content, &[]);
    assert_rejected(open(plain, Limits::default()), "inline image, plain stream");
    // The same inside a FlateDecode content stream.
    let packed = {
        let objs = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
            stream("/Filter /FlateDecode", &zlib(content)),
        ];
        build_pdf(&objs)
    };
    assert_rejected(
        open(packed, Limits::default()),
        "inline image, flate stream",
    );
    // A small inline image is fine.
    let small = page_pdf(
        "",
        b"q 10 0 0 10 0 0 cm BI /W 2 /H 2 /BPC 8 /CS /G ID abcd EI Q",
        &[],
    );
    assert!(open(small, Limits::default()).is_ok());
}
