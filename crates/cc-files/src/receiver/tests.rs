// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the receiver state machine: normal transfers, limits and hostile sequences.
//! PDFs are the generated test files of `cc-docs`. Rendering uses `Isolation::InProcess`
//! here because these tests have no worker program; the files are our own and trusted. The
//! sandboxed path is covered by the end-to-end tests in `cc-receiver` and the interop script.

use super::*;
use crate::outgoing::{Outgoing, cmd_message, document_transfer, image_transfer, link_message};

const MINIMAL: &[u8] = include_bytes!("../../../cc-docs/tests/data/minimal.pdf");
const CLASSROOM: &[u8] = include_bytes!("../../../cc-docs/tests/data/classroom.pdf");
const ENCRYPTED: &[u8] = include_bytes!("../../../cc-docs/tests/data/encrypted_password.pdf");

fn rx() -> ContentReceiver {
    ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        target_width: 400,
        ..ContentOptions::default()
    })
}

/// Feed a whole transfer and collect the events.
fn feed<'a>(rx: &mut ContentReceiver, t: impl Iterator<Item = Outgoing<'a>>) -> Vec<ContentEvent> {
    let mut out = Vec::new();
    for item in t {
        out.extend(match item {
            Outgoing::Text(t) => rx.handle_text(&t),
            Outgoing::Binary(b) => rx.handle_binary(b),
        });
    }
    out
}

fn errors(events: &[ContentEvent]) -> Vec<ContentError> {
    events
        .iter()
        .filter_map(|e| match e {
            ContentEvent::Error { error, .. } => Some(*error),
            _ => None,
        })
        .collect()
}

fn image_start(size: u64) -> String {
    format!(r#"{{"kind":"image","name":"a.png","mime":"image/png","size":{size}}}"#)
}

fn doc_start(size: u64) -> String {
    format!(r#"{{"kind":"document","name":"a.pdf","mime":"application/pdf","size":{size}}}"#)
}

#[test]
fn an_image_arrives_whole() {
    let data: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();
    let mut r = rx();
    let ev = feed(
        &mut r,
        image_transfer("a.png", ImageMime::Png, &data).unwrap(),
    );
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert!(matches!(
        &ev[0],
        ContentEvent::TransferStarted {
            document: false,
            size: 40_000,
            ..
        }
    ));
    let ContentEvent::ImageReceived { name, mime, bytes } = &ev[1] else {
        panic!("{ev:?}")
    };
    assert_eq!((name.as_str(), *mime), ("a.png", ImageMime::Png));
    assert_eq!(&bytes[..], &data[..]);
    assert!(!r.transfer_active());
    assert_eq!(r.violations(), 0);
}

#[test]
fn web_example_sequence_works() {
    // The example of docs/web-m2.md: 40000 bytes = 16384 + 16384 + 7232.
    let mut r = rx();
    assert_eq!(r.handle_text(&image_start(40_000)).len(), 1);
    assert!(r.handle_binary(&[1; 16384]).is_empty());
    assert!(r.handle_binary(&[2; 16384]).is_empty());
    assert!(r.handle_binary(&[3; 7232]).is_empty());
    let ev = r.handle_text(r#"{"kind":"image_end"}"#);
    assert!(matches!(&ev[..], [ContentEvent::ImageReceived { .. }]));
}

#[test]
fn links_are_reported_and_validated() {
    let mut r = rx();
    let ev = r.handle_text(&link_message("https://example.org/x").unwrap());
    assert_eq!(
        ev,
        [ContentEvent::LinkRequested {
            url: "https://example.org/x".into()
        }]
    );
    for bad in [
        r#"{"kind":"link","url":"javascript:alert(1)"}"#,
        r#"{"kind":"link","url":"https://user:pw@example.org"}"#,
        r#"{"kind":"link","url":"https://exa\tmple.org"}"#,
        r#"{"kind":"link","url":"file:///etc/passwd"}"#,
        r#"{"kind":"link","url":5}"#,
        r#"{"kind":"link"}"#,
    ] {
        let ev = r.handle_text(bad);
        assert_eq!(errors(&ev), [ContentError::BadMessage], "{bad}");
        assert!(
            !ev.iter()
                .any(|e| matches!(e, ContentEvent::LinkRequested { .. }))
        );
    }
}

#[test]
fn oversize_announcements_are_refused() {
    let mut r = rx();
    let ev = r.handle_text(&image_start(MAX_IMAGE_BYTES + 1));
    assert_eq!(errors(&ev), [ContentError::BadMessage]);
    assert!(!r.transfer_active());
    // Chunks that follow find no transfer.
    let ev = r.handle_binary(&[0; CHUNK_SIZE]);
    assert_eq!(errors(&ev), [ContentError::UnexpectedChunk]);
    // A receiver with a smaller limit refuses what the format would allow.
    let mut small = ContentReceiver::new(ContentOptions {
        max_image_bytes: 1000,
        ..ContentOptions::default()
    });
    assert!(
        small
            .handle_text(&image_start(1001))
            .iter()
            .any(|e| matches!(e, ContentEvent::Error { .. }))
    );
    assert!(!small.handle_text(&image_start(1000)).is_empty());
    let mut tiny_docs = ContentReceiver::new(ContentOptions {
        max_document_bytes: 1000,
        ..ContentOptions::default()
    });
    assert_eq!(
        errors(&tiny_docs.handle_text(&doc_start(1001))),
        [ContentError::BadMessage]
    );
    // The cc-docs file limit counts too.
    let limits = Limits {
        max_file_bytes: 500,
        ..Limits::default()
    };
    let mut docs = ContentReceiver::new(ContentOptions {
        doc_limits: limits,
        ..ContentOptions::default()
    });
    assert_eq!(
        errors(&docs.handle_text(&doc_start(501))),
        [ContentError::BadMessage]
    );
}

#[test]
fn data_past_the_announced_size_ends_the_transfer() {
    let mut r = rx();
    r.handle_text(&image_start(20_000));
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    // 3616 left; a full chunk is too much.
    let ev = r.handle_binary(&[0; CHUNK_SIZE]);
    assert_eq!(errors(&ev), [ContentError::TooLarge]);
    assert!(!r.transfer_active());
    // The end message of the dropped transfer is a violation too, no image comes out.
    let ev = r.handle_text(r#"{"kind":"image_end"}"#);
    assert_eq!(errors(&ev), [ContentError::NoTransfer]);
}

#[test]
fn the_exact_size_can_not_be_exceeded_by_one_byte() {
    let mut r = rx();
    r.handle_text(&image_start(CHUNK_SIZE as u64));
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    assert_eq!(errors(&r.handle_binary(&[0])), [ContentError::TooLarge]);
}

#[test]
fn tiny_chunk_floods_are_refused() {
    let mut r = rx();
    r.handle_text(&image_start(1_000_000));
    let ev = r.handle_binary(&[0; 10]);
    assert_eq!(errors(&ev), [ContentError::BadChunk]);
    assert!(!r.transfer_active(), "the transfer is dropped");
    // An empty message is no chunk either.
    r.handle_text(&image_start(10));
    assert_eq!(errors(&r.handle_binary(&[])), [ContentError::BadChunk]);
    // A single short message is fine when it completes the transfer.
    r.handle_text(&image_start(10));
    assert!(r.handle_binary(&[0; 10]).is_empty());
}

#[test]
fn a_short_chunk_in_the_middle_is_refused() {
    let mut r = rx();
    r.handle_text(&image_start(3 * CHUNK_SIZE as u64));
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    assert_eq!(
        errors(&r.handle_binary(&[0; CHUNK_SIZE - 1])),
        [ContentError::BadChunk]
    );
}

#[test]
fn truncated_transfers_are_not_delivered() {
    let mut r = rx();
    r.handle_text(&image_start(2 * CHUNK_SIZE as u64));
    r.handle_binary(&[0; CHUNK_SIZE]);
    let ev = r.handle_text(r#"{"kind":"image_end"}"#);
    assert_eq!(errors(&ev), [ContentError::SizeMismatch]);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, ContentEvent::ImageReceived { .. }))
    );
    assert!(!r.transfer_active());
    // Same for a document.
    r.handle_text(&doc_start(100));
    let ev = r.handle_text(r#"{"kind":"document_end"}"#);
    assert_eq!(errors(&ev), [ContentError::SizeMismatch]);
}

#[test]
fn chunks_after_the_end_are_refused() {
    let mut r = rx();
    feed(
        &mut r,
        image_transfer("a.png", ImageMime::Png, &[1; 100]).unwrap(),
    );
    assert_eq!(
        errors(&r.handle_binary(&[1; 100])),
        [ContentError::UnexpectedChunk]
    );
    assert_eq!(
        errors(&r.handle_text(r#"{"kind":"image_end"}"#)),
        [ContentError::NoTransfer]
    );
}

#[test]
fn a_chunk_flood_without_a_transfer_gives_one_event_and_blocks() {
    let mut r = ContentReceiver::new(ContentOptions {
        max_violations: 10,
        ..ContentOptions::default()
    });
    let mut events = 0;
    for _ in 0..1000 {
        events += r.handle_binary(&[0; CHUNK_SIZE]).len();
    }
    assert_eq!(events, 2, "one unexpected_chunk and one too_many_errors");
    assert!(r.is_blocked());
    // Blocked: even valid messages are ignored.
    assert!(
        r.handle_text(&link_message("https://example.org").unwrap())
            .is_empty()
    );
    assert!(r.handle_text(&cmd_message(ControlCmd::Stop)).is_empty());
}

#[test]
fn a_new_transfer_interrupts_the_old_one() {
    let mut r = rx();
    r.handle_text(&image_start(50_000));
    r.handle_binary(&[9; CHUNK_SIZE]);
    // Interleaved: a document starts while the image is open.
    let ev = r.handle_text(&doc_start(MINIMAL.len() as u64));
    assert_eq!(errors(&ev), [ContentError::Interrupted]);
    assert!(
        ev.iter()
            .any(|e| matches!(e, ContentEvent::TransferStarted { document: true, .. }))
    );
    // The image's end message now finds a document transfer: wrong kind, nothing delivered.
    let ev = r.handle_text(r#"{"kind":"image_end"}"#);
    assert_eq!(errors(&ev), [ContentError::NoTransfer]);
    assert!(!r.transfer_active());
}

#[test]
fn end_of_the_wrong_kind_is_refused() {
    let mut r = rx();
    r.handle_text(&doc_start(5));
    r.handle_binary(b"%PDF-");
    let ev = r.handle_text(r#"{"kind":"image_end"}"#);
    assert_eq!(errors(&ev), [ContentError::NoTransfer]);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, ContentEvent::DocumentOpened { .. }))
    );
    assert_eq!(
        errors(&r.handle_text(r#"{"kind":"document_end"}"#)),
        [ContentError::NoTransfer]
    );
}

#[test]
fn a_refused_start_drops_an_open_transfer() {
    let mut r = rx();
    r.handle_text(&image_start(1000));
    let ev = r.handle_text(r#"{"kind":"image","name":"x","mime":"image/svg+xml","size":5}"#);
    assert_eq!(errors(&ev), [ContentError::BadMessage]);
    assert!(!r.transfer_active());
    assert_eq!(
        errors(&r.handle_binary(&[0; 1000])),
        [ContentError::UnexpectedChunk]
    );
}

#[test]
fn wrong_mimes_and_junk_are_refused() {
    let mut r = rx();
    for t in [
        r#"{"kind":"image","name":"a","mime":"text/html","size":5}"#,
        r#"{"kind":"image","name":"a","mime":"image/svg+xml","size":5}"#,
        r#"{"kind":"document","name":"a","mime":"image/png","size":5}"#,
        r#"{"kind":"document","name":"a","mime":"application/zip","size":5}"#,
        r#"{"kind":"nonsense"}"#,
        "not json",
        "",
        r#"{"kind":"image"}"#,
    ] {
        assert_eq!(errors(&r.handle_text(t)), [ContentError::BadMessage], "{t}");
        assert!(!r.transfer_active());
    }
}

#[test]
fn a_pdf_is_opened_rendered_and_driven_by_commands() {
    let mut r = rx();
    let ev = feed(&mut r, document_transfer("class.pdf", CLASSROOM).unwrap());
    assert!(errors(&ev).is_empty(), "{ev:?}");
    let opened = ev.iter().find_map(|e| match e {
        ContentEvent::DocumentOpened { name, pages } => Some((name.clone(), *pages)),
        _ => None,
    });
    assert_eq!(opened, Some(("class.pdf".into(), 3)));
    let Some(ContentEvent::PageRendered {
        page,
        width,
        height,
        rgba,
    }) = ev.last()
    else {
        panic!("{ev:?}")
    };
    assert_eq!((*page, *width), (1, 400));
    assert!((565..=568).contains(height), "A4 at 400 px wide: {height}");
    assert_eq!(rgba.len(), (*width * *height * 4) as usize);
    assert_eq!(r.document_state(), Some((1, 3)));

    let page_of = |ev: &[ContentEvent]| match ev {
        [ContentEvent::PageRendered { page, .. }] => Some(*page),
        _ => None,
    };
    assert_eq!(
        page_of(&r.handle_text(&cmd_message(ControlCmd::NextPage))),
        Some(2)
    );
    assert_eq!(
        page_of(&r.handle_text(&cmd_message(ControlCmd::NextPage))),
        Some(3)
    );
    // At the last page nothing changes, nothing is rendered again.
    assert!(r.handle_text(&cmd_message(ControlCmd::NextPage)).is_empty());
    assert_eq!(
        page_of(&r.handle_text(&cmd_message(ControlCmd::PrevPage))),
        Some(2)
    );
    assert_eq!(
        page_of(&r.handle_text(&cmd_message(ControlCmd::GoToPage { page: 1 }))),
        Some(1)
    );
    // Out of range is clamped by the page state, not refused.
    assert_eq!(
        page_of(&r.handle_text(&cmd_message(ControlCmd::GoToPage { page: 999 }))),
        Some(3)
    );
    assert_eq!(r.violations(), 0);

    assert_eq!(
        r.handle_text(&cmd_message(ControlCmd::Stop)),
        [ContentEvent::ContentStopped]
    );
    assert_eq!(r.document_state(), None);
    assert!(
        r.handle_text(&cmd_message(ControlCmd::NextPage)).is_empty(),
        "nothing to turn"
    );
}

#[test]
fn page_commands_without_a_document_are_ignored() {
    let mut r = rx();
    for c in [
        ControlCmd::NextPage,
        ControlCmd::PrevPage,
        ControlCmd::GoToPage { page: 2 },
    ] {
        assert!(r.handle_text(&cmd_message(c)).is_empty());
    }
    assert_eq!(r.violations(), 0);
    assert_eq!(
        r.handle_text(&cmd_message(ControlCmd::Stop)),
        [ContentEvent::ContentStopped]
    );
}

#[test]
fn stop_cancels_a_transfer() {
    let mut r = rx();
    r.handle_text(&image_start(50_000));
    r.handle_binary(&[0; CHUNK_SIZE]);
    assert_eq!(
        r.handle_text(&cmd_message(ControlCmd::Stop)),
        [ContentEvent::ContentStopped]
    );
    assert!(!r.transfer_active());
}

#[test]
fn a_hostile_pdf_is_rejected_without_panic() {
    let mut r = rx();
    // Not a PDF at all (the sender check is skipped by writing the messages by hand).
    r.handle_text(&doc_start(11));
    r.handle_binary(b"hello world");
    let ev = r.handle_text(r#"{"kind":"document_end"}"#);
    assert_eq!(errors(&ev), [ContentError::DocumentRejected]);
    assert_eq!(r.document_state(), None);
    // A password protected one.
    let ev = feed(&mut r, document_transfer("e.pdf", ENCRYPTED).unwrap());
    assert_eq!(errors(&ev), [ContentError::DocumentRejected]);
    // A PDF header followed by noise.
    let mut junk = b"%PDF-1.7\n".to_vec();
    junk.extend((0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8));
    let ev = feed(&mut r, document_transfer("j.pdf", &junk).unwrap());
    assert_eq!(errors(&ev), [ContentError::DocumentRejected]);
    // Rejected files are not violations of the channel protocol.
    assert_eq!(r.violations(), 0);
    assert!(!r.is_blocked());
}

#[test]
fn a_new_document_or_image_replaces_the_shown_one() {
    let mut r = rx();
    feed(&mut r, document_transfer("a.pdf", CLASSROOM).unwrap());
    assert_eq!(r.document_state(), Some((1, 3)));
    // A bad document closes the old one (nothing stale stays on screen).
    r.handle_text(&doc_start(5));
    r.handle_binary(b"junk!");
    r.handle_text(r#"{"kind":"document_end"}"#);
    assert_eq!(r.document_state(), None);
    feed(&mut r, document_transfer("a.pdf", MINIMAL).unwrap());
    assert_eq!(r.document_state(), Some((1, 1)));
    feed(
        &mut r,
        image_transfer("i.png", ImageMime::Png, &[1; 10]).unwrap(),
    );
    assert_eq!(r.document_state(), None, "an image replaces the document");
}

#[test]
fn the_target_width_is_clamped_and_rerenders() {
    let mut r = rx();
    assert!(
        r.set_target_width(800).is_empty(),
        "no document, nothing to render"
    );
    feed(&mut r, document_transfer("m.pdf", MINIMAL).unwrap());
    let ev = r.set_target_width(300);
    let [ContentEvent::PageRendered { width, height, .. }] = &ev[..] else {
        panic!("{ev:?}")
    };
    assert_eq!((*width, *height), (300, 150));
    // Too small: raised to the minimum. Too big: lowered to the output limit.
    let ev = r.set_target_width(1);
    let [ContentEvent::PageRendered { width, .. }] = &ev[..] else {
        panic!()
    };
    assert_eq!(*width, MIN_RENDER_WIDTH);
    let ev = r.set_target_width(u32::MAX);
    let [ContentEvent::PageRendered { width, height, .. }] = &ev[..] else {
        panic!("{ev:?}")
    };
    assert!(*width <= Limits::default().max_output_side);
    assert!(u64::from(*width) * u64::from(*height) <= Limits::default().max_output_pixels);
}

#[test]
fn rendering_respects_small_output_limits() {
    let limits = Limits {
        max_output_side: 100,
        max_output_pixels: 100 * 100,
        ..Limits::default()
    };
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        doc_limits: limits,
        target_width: 5000,
        ..ContentOptions::default()
    });
    let ev = feed(&mut r, document_transfer("m.pdf", MINIMAL).unwrap());
    let Some(ContentEvent::PageRendered { width, height, .. }) = ev.last() else {
        panic!("{ev:?}")
    };
    assert_eq!((*width, *height), (100, 50));
}

#[test]
fn page_count_limit_comes_from_cc_docs() {
    let limits = Limits {
        max_pages: 2,
        ..Limits::default()
    };
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        doc_limits: limits,
        ..ContentOptions::default()
    });
    let ev = feed(&mut r, document_transfer("c.pdf", CLASSROOM).unwrap());
    assert_eq!(errors(&ev), [ContentError::DocumentRejected], "{ev:?}");
}

#[test]
fn event_debug_does_not_dump_bytes() {
    let b = Blob::new(vec![7; 100_000]);
    let s = format!("{b:?}");
    assert!(s.len() < 40, "{s}");
}
