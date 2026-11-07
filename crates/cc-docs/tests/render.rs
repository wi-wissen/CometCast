// SPDX-License-Identifier: GPL-3.0-or-later
//! Renders the generated test PDFs (tests/data, see PROVENANCE.md) and checks properties.
//!
//! Set `CC_DOCS_DUMP_DIR=/some/dir` to also write every rendered page as PNG for looking at.

use cc_docs::{
    DocError, Document, DocumentSession, Limits, RenderedPage, SessionEvent, encode_png,
    encode_webp_lossless, validate_webp,
};
use cc_proto::ControlCmd;

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
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open(name: &str) -> Document {
    open_pdf(data(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn dump(name: &str, page: usize, img: &RenderedPage) {
    if let Ok(dir) = std::env::var("CC_DOCS_DUMP_DIR") {
        let f = std::path::Path::new(&dir).join(format!("{name}-p{page}.png"));
        std::fs::write(f, encode_png(img).unwrap()).unwrap();
    }
}

/// Pixel is within `tol` per channel of `rgb` (and opaque).
fn assert_color(img: &RenderedPage, x: u32, y: u32, rgb: [u8; 3], tol: u8) {
    let p = img.pixel(x, y).expect("pixel inside picture");
    let ok = (0..3).all(|i| p[i].abs_diff(rgb[i]) <= tol) && p[3] == 255;
    assert!(ok, "pixel ({x},{y}) is {p:?}, expected about {rgb:?}");
}

fn non_white(img: &RenderedPage) -> usize {
    img.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] < 250 || p[1] < 250 || p[2] < 250)
        .count()
}

#[test]
fn minimal_page_has_two_colour_halves_and_text() {
    let d = open("minimal.pdf");
    assert_eq!(d.page_count(), 1);
    let s = d.page_size(0).unwrap();
    assert_eq!((s.width_pt, s.height_pt), (200.0, 100.0));
    let img = d.render_page(0, 400).unwrap();
    dump("minimal", 0, &img);
    assert_eq!((img.width, img.height), (400, 200));
    assert_eq!(img.rgba.len(), 400 * 200 * 4);
    assert_color(&img, 100, 150, [255, 0, 0], 2);
    assert_color(&img, 300, 150, [0, 0, 255], 2);
    // The text "Hi" is black on red at the top left: some dark pixels exist there.
    let dark = (0..60u32)
        .flat_map(|y| (0..80u32).map(move |x| (x, y)))
        .filter(|&(x, y)| img.pixel(x, y).is_some_and(|p| p[0] < 100))
        .count();
    assert!(dark > 20, "text pixels: {dark}");
}

#[test]
fn render_scales_to_the_requested_width() {
    let d = open("minimal.pdf");
    for w in [1u32, 7, 100, 1920] {
        let img = d.render_page(0, w).unwrap();
        assert_eq!(img.width, w);
        assert_eq!(img.height, (f64::from(w) / 2.0).round().max(1.0) as u32);
        assert_eq!(img.rgba.len(), img.width as usize * img.height as usize * 4);
    }
}

#[test]
fn damaged_file_without_xref_is_repaired_by_scanning() {
    let d = open("no_xref.pdf");
    let img = d.render_page(0, 400).unwrap();
    assert_color(&img, 100, 150, [255, 0, 0], 2);
    assert_color(&img, 300, 150, [0, 0, 255], 2);
}

#[test]
fn rotate_90_swaps_the_page_size_and_moves_content() {
    let d = open("two_pages_one_rotated.pdf");
    assert_eq!(d.page_count(), 2);
    let s0 = d.page_size(0).unwrap();
    let s1 = d.page_size(1).unwrap();
    assert_eq!((s0.width_pt, s0.height_pt), (200.0, 100.0));
    assert_eq!((s1.width_pt, s1.height_pt), (100.0, 200.0));
    let a = d.render_page(0, 200).unwrap();
    assert_color(&a, 100, 50, [255, 255, 0], 2);
    let b = d.render_page(1, 100).unwrap();
    dump("two_pages", 1, &b);
    assert_eq!((b.width, b.height), (100, 200));
    // A red box was at the top left of the unrotated page; 90 degrees clockwise puts it
    // at the top right.
    assert_color(&b, 90, 10, [255, 0, 0], 2);
    assert_color(&b, 10, 10, [0, 153, 0], 2);
    assert_color(&b, 50, 150, [0, 153, 0], 2);
}

#[test]
fn four_rotations_put_the_red_box_in_four_corners() {
    let d = open("rotated_pages.pdf");
    assert_eq!(d.page_count(), 4);
    let sizes: Vec<_> = (0..4)
        .map(|i| {
            let s = d.page_size(i).unwrap();
            (s.width_pt, s.height_pt)
        })
        .collect();
    assert_eq!(
        sizes,
        [
            (200.0, 300.0),
            (300.0, 200.0),
            (200.0, 300.0),
            (300.0, 200.0)
        ]
    );
    // (x, y) of a point inside the red box at width 200 px for upright pages and 300 px for
    // sideways pages (1 pixel = 1 pt): top left, top right, bottom right, bottom left.
    let cases = [
        (0, 200, (10, 10)),
        (1, 300, (290, 10)),
        (2, 200, (190, 290)),
        (3, 300, (10, 190)),
    ];
    for (i, w, (x, y)) in cases {
        let img = d.render_page(i, w).unwrap();
        dump("rotated", i, &img);
        assert_color(&img, x, y, [255, 0, 0], 2);
        // The opposite area is white.
        assert_color(
            &img,
            img.width - 1 - x,
            img.height - 1 - y,
            [255, 255, 255],
            0,
        );
    }
}

#[test]
fn slides_page1_text_and_vector_graphics() {
    let d = open("slides.pdf");
    assert_eq!(d.page_count(), 3);
    let img = d.render_page(0, 595).unwrap();
    dump("slides", 0, &img);
    assert_eq!(img.height, 842);
    // Page height is 841.89 pt; y of a PDF point is counted from the bottom.
    assert_color(&img, 150, 467, [0, 153, 0], 3); // green box
    assert_color(&img, 400, 467, [255, 217, 0], 3); // yellow circle centre
    assert_color(&img, 20, 20, [255, 255, 255], 0);
    assert!(non_white(&img) > 5000, "page looks blank");
}

#[test]
fn slides_page2_embedded_jpeg() {
    let d = open("slides.pdf");
    let img = d.render_page(1, 595).unwrap();
    dump("slides", 1, &img);
    // JPEG at x 100..420, y 500..660 pt: left half orange, right half teal (JPEG tolerance).
    assert_color(&img, 200, 262, [255, 128, 0], 14);
    assert_color(&img, 320, 262, [0, 128, 128], 14);
    assert_color(&img, 50, 262, [255, 255, 255], 0);
}

#[test]
fn slides_page3_embedded_font_and_gradient() {
    let d = open("slides.pdf");
    let img = d.render_page(2, 595).unwrap();
    dump("slides", 2, &img);
    // Gradient from red (left) to blue (right) inside x 50..450, y 400..500 pt.
    let left = img.pixel(60, 392).unwrap();
    let right = img.pixel(440, 392).unwrap();
    assert!(left[0] > 200 && left[2] < 60, "left {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "right {right:?}");
    // Text line at the top is drawn (dark pixels in y 40..80).
    let dark = (40..80u32)
        .flat_map(|y| (40..400u32).map(move |x| (x, y)))
        .filter(|&(x, y)| img.pixel(x, y).is_some_and(|p| p[0] < 100 && p[1] < 100))
        .count();
    assert!(dark > 300, "text pixels: {dark}");
}

#[test]
fn encrypted_with_empty_user_password_opens() {
    let d = open("encrypted_empty_user_password.pdf");
    let img = d.render_page(0, 595).unwrap();
    assert_color(&img, 200, 342, [0, 0, 255], 2);
}

#[test]
fn password_protected_file_is_refused_with_a_clear_error() {
    let e = open_pdf(data("encrypted_password.pdf")).unwrap_err();
    assert_eq!(e, DocError::PasswordProtected);
    assert_eq!(e.to_string(), "the PDF is password protected");
}

#[test]
fn thumbnails_fit_the_box() {
    let d = open("slides.pdf");
    let t = d.thumbnail(0, 160).unwrap();
    assert_eq!((t.width, t.height), (113, 160));
    let m = open("minimal.pdf").thumbnail(0, 160).unwrap();
    assert_eq!((m.width, m.height), (160, 80));
    assert_eq!(d.thumbnail(0, 0).unwrap_err(), DocError::InvalidWidth);
}

/// A one-page PDF with the given page dictionary entries.
fn page_with(extra: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R {extra} /Contents 4 0 R /Resources << >> >>"),
        "<< /Length 23 >>\nstream\n1 0 0 rg 0 0 50 50 re f\nendstream".to_string(),
    ];
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
    }
    let x = out.len();
    out.extend(b"xref\n0 5\n0000000000 65535 f \n");
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").bytes());
    out
}

/// Review finding: a 10 x 10 000 pt page with `max_side` 64 gave a 1 x 1000 picture.
#[test]
fn thumbnails_of_extreme_shapes_still_fit_the_box() {
    for (media, max_side) in [
        ("[0 0 10 10000]", 64u32),
        ("[0 0 10000 10]", 64),
        ("[0 0 1 100000]", 16),
        ("[0 0 100 100]", 1),
        ("[0 0 7 300]", 100),
    ] {
        let d = open_pdf(page_with(&format!("/MediaBox {media}"))).unwrap();
        let t = d.thumbnail(0, max_side).unwrap();
        println!("{media} in {max_side}: {} x {}", t.width, t.height);
        assert!(t.width >= 1 && t.height >= 1);
        assert!(
            t.width <= max_side && t.height <= max_side,
            "{media}: {} x {}",
            t.width,
            t.height
        );
        // The long side uses the whole box.
        assert_eq!(t.width.max(t.height), max_side, "{media}");
        assert_eq!(t.rgba.len(), t.width as usize * t.height as usize * 4);
    }
}

/// `/UserUnit` (PDF 1.6) scales a page's unit; hayro ignores it, so the size stays in
/// points. Documented in `docs/documents.md` and `PageSize`.
#[test]
fn user_unit_is_not_applied() {
    let d = open_pdf(page_with("/MediaBox [0 0 100 100] /UserUnit 100")).unwrap();
    let s = d.page_size(0).unwrap();
    assert_eq!((s.width_pt, s.height_pt), (100.0, 100.0));
    assert_eq!(d.render_page(0, 200).unwrap().height, 200);
}

#[test]
fn notes_are_none_for_pdf() {
    assert_eq!(open("minimal.pdf").notes(0), None);
}

#[test]
fn page_index_and_width_errors() {
    let d = open("minimal.pdf");
    assert!(matches!(
        d.render_page(1, 100),
        Err(DocError::PageOutOfRange { index: 1, count: 1 })
    ));
    assert_eq!(d.render_page(0, 0).unwrap_err(), DocError::InvalidWidth);
    assert!(matches!(
        d.render_page(0, 100_000),
        Err(DocError::OutputTooLarge { .. })
    ));
    assert!(matches!(
        d.page_size(5),
        Err(DocError::PageOutOfRange { .. })
    ));
}

#[test]
fn output_limits_are_enforced_before_rendering() {
    let limits = Limits {
        max_output_side: 1000,
        max_output_pixels: 300_000,
        ..Limits::default()
    };
    let d = open_pdf_limits(data("minimal.pdf"), limits).unwrap();
    // 1000 x 500 = 500 000 pixels > 300 000
    assert!(matches!(
        d.render_page(0, 1000),
        Err(DocError::OutputTooLarge { .. })
    ));
    assert!(matches!(
        d.render_page(0, 1001),
        Err(DocError::OutputTooLarge { .. })
    ));
    assert!(d.render_page(0, 600).is_ok()); // 600 x 300 = 180 000
}

#[test]
fn encoders_round_trip() {
    let d = open("slides.pdf");
    let img = d.render_page(0, 300).unwrap();
    let png_bytes = encode_png(&img).unwrap();
    assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");
    let webp = encode_webp_lossless(&img).unwrap();
    assert_eq!(&webp[..4], b"RIFF");
    let info = validate_webp(&webp, 10_000_000).unwrap();
    assert_eq!((info.width, info.height), (img.width, img.height));
    // Decode again and compare pixels: lossless means identical.
    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(&webp)).unwrap();
    let mut buf = vec![0u8; dec.output_buffer_size().unwrap()];
    dec.read_image(&mut buf).unwrap();
    if dec.has_alpha() {
        assert_eq!(buf, img.rgba);
    } else {
        let rgb: Vec<u8> = img
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        assert_eq!(buf, rgb);
    }
    println!(
        "sizes at 300 px: png {} bytes, lossless webp {} bytes",
        png_bytes.len(),
        webp.len()
    );
}

#[test]
fn webp_validation_rejects_bad_input() {
    assert!(validate_webp(b"", 1000).is_err());
    assert!(validate_webp(b"RIFF....WEBP", 1000).is_err());
    let img = open("minimal.pdf").render_page(0, 100).unwrap();
    let webp = encode_webp_lossless(&img).unwrap();
    assert!(validate_webp(&webp, 100).is_err()); // 100 x 50 = 5000 pixels > 100
    let bad = RenderedPage {
        width: 10,
        height: 10,
        rgba: vec![0; 3],
    };
    assert!(encode_png(&bad).is_err());
    assert!(encode_webp_lossless(&bad).is_err());
}

#[test]
fn session_drives_a_real_document() {
    let d = open("slides.pdf");
    let mut s = DocumentSession::new(u32::try_from(d.page_count()).unwrap()).unwrap();
    assert_eq!(
        s.apply(&ControlCmd::GoToPage { page: 99 }),
        SessionEvent::PageChanged(3)
    );
    let img = d.render_page(s.current_index(), 200).unwrap();
    assert_eq!(img.width, 200);
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
