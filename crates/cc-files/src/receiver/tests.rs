// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the receiver state machine: normal transfers, limits and hostile sequences.
//! PDFs are the generated test files of `cc-docs`. Rendering uses `Isolation::InProcess`
//! here because these tests have no worker program; the files are our own and trusted. The
//! sandboxed path is covered by the end-to-end tests in `cc-receiver` and the interop script.

use super::*;
use crate::outgoing::{Outgoing, cmd_message, document_transfer, image_transfer, link_message};

const MINIMAL: &[u8] = include_bytes!("../../../cc-docs/tests/data/minimal.pdf");
const SAMPLE_PDF: &[u8] = include_bytes!("../../../cc-docs/tests/data/slides.pdf");
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
    // The data of the refused transfer is swallowed without further violations.
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    assert_eq!(r.violations(), 1);
    assert!(r.handle_text(r#"{"kind":"image_end"}"#).is_empty());
    // Afterwards chunks without a transfer are violations again.
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
    // A refused document is reported as such, and does not leave an old one on screen.
    assert_eq!(
        errors(&tiny_docs.handle_text(&doc_start(1001))),
        [ContentError::DocumentRejected]
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
        [ContentError::DocumentRejected]
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
    // The one chunk the refused transfer announced is swallowed; a second one is too much.
    assert!(r.handle_binary(&[0; 1000]).is_empty());
    assert_eq!(
        errors(&r.handle_binary(&[0; 1000])),
        [ContentError::TooLarge]
    );
}

#[test]
fn wrong_mimes_and_junk_are_refused() {
    let mut r = rx();
    for (t, code) in [
        (
            r#"{"kind":"image","name":"a","mime":"text/html","size":5}"#,
            ContentError::BadMessage,
        ),
        (
            r#"{"kind":"image","name":"a","mime":"image/svg+xml","size":5}"#,
            ContentError::BadMessage,
        ),
        (
            r#"{"kind":"document","name":"a","mime":"image/png","size":5}"#,
            ContentError::DocumentRejected,
        ),
        (
            r#"{"kind":"document","name":"a","mime":"application/zip","size":5}"#,
            ContentError::DocumentRejected,
        ),
        (r#"{"kind":"nonsense"}"#, ContentError::BadMessage),
        ("not json", ContentError::BadMessage),
        ("", ContentError::BadMessage),
        (r#"{"kind":"image"}"#, ContentError::BadMessage),
    ] {
        assert_eq!(errors(&r.handle_text(t)), [code], "{t}");
        assert!(!r.transfer_active());
    }
}

#[test]
fn a_pdf_is_opened_rendered_and_driven_by_commands() {
    let mut r = rx();
    let ev = feed(&mut r, document_transfer("sample.pdf", SAMPLE_PDF).unwrap());
    assert!(errors(&ev).is_empty(), "{ev:?}");
    let opened = ev.iter().find_map(|e| match e {
        ContentEvent::DocumentOpened { name, pages } => Some((name.clone(), *pages)),
        _ => None,
    });
    assert_eq!(opened, Some(("sample.pdf".into(), 3)));
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
    // A document that cannot be opened costs a child process, so it counts as a violation
    // (the three above); it does not block a receiver that has budget left.
    assert_eq!(r.violations(), 3);
    assert!(!r.is_blocked());
}

#[test]
fn a_new_document_or_image_replaces_the_shown_one() {
    let mut r = rx();
    feed(&mut r, document_transfer("a.pdf", SAMPLE_PDF).unwrap());
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
    let ev = feed(&mut r, document_transfer("c.pdf", SAMPLE_PDF).unwrap());
    assert_eq!(errors(&ev), [ContentError::DocumentRejected], "{ev:?}");
}

#[test]
fn event_debug_does_not_dump_bytes() {
    let b = Blob::new(vec![7; 100_000]);
    let s = format!("{b:?}");
    assert!(s.len() < 40, "{s}");
}

// ---------------------------------------------------------------------------------------
// Flood control, preemption, refused transfers, decay, timeouts, open budget.
// ---------------------------------------------------------------------------------------

use std::time::{Duration, Instant};

/// Run every job that is due, like the task of `cc-receiver` but without waiting.
/// Returns the events and the number of jobs.
fn drain(r: &mut ContentReceiver) -> (Vec<ContentEvent>, usize) {
    let mut out = Vec::new();
    let mut jobs = 0;
    while let Next::Work(w) = r.next_action(Instant::now(), false) {
        jobs += 1;
        out.extend(r.finish_work(w.run()));
    }
    (out, jobs)
}

fn open_sample() -> ContentReceiver {
    let mut r = rx();
    let ev = feed(&mut r, document_transfer("c.pdf", SAMPLE_PDF).unwrap());
    assert!(errors(&ev).is_empty(), "{ev:?}");
    r
}

#[test]
fn a_flood_of_page_commands_costs_one_render() {
    let mut r = open_sample();
    let started = Instant::now();
    let mut events = Vec::new();
    for i in 0..1000 {
        let cmd = if i % 2 == 0 {
            ControlCmd::NextPage
        } else {
            ControlCmd::PrevPage
        };
        events.extend(r.feed_text(&cmd_message(cmd)));
    }
    assert!(events.is_empty(), "feeding never renders: {events:?}");
    let (ev, jobs) = drain(&mut r);
    assert!(jobs <= 1, "{jobs} renders");
    // 1000 alternating commands end on the page they started from.
    assert_eq!(r.document_state(), Some((1, 3)));
    assert!(ev.len() <= 1, "{ev:?}");
    // A flood in one direction renders only the net target.
    for _ in 0..1000 {
        r.feed_text(&cmd_message(ControlCmd::NextPage));
    }
    let (ev, jobs) = drain(&mut r);
    assert_eq!(jobs, 1);
    assert!(
        matches!(&ev[..], [ContentEvent::PageRendered { page: 3, .. }]),
        "{ev:?}"
    );
    assert_eq!(r.violations(), 0);
    assert!(started.elapsed() < Duration::from_secs(5), "bounded time");
}

#[test]
fn stop_wins_over_a_running_render() {
    let mut r = open_sample();
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    let Next::Work(job) = r.next_action(Instant::now(), false) else {
        panic!("a render is due")
    };
    // The job runs "elsewhere" while the next message arrives.
    let ev = r.feed_text(&cmd_message(ControlCmd::Stop));
    assert_eq!(ev, [ContentEvent::ContentStopped]);
    // Its picture is stale and is dropped.
    assert!(r.finish_work(job.run()).is_empty());
    assert_eq!(r.document_state(), None);
    // Page commands after stop find nothing to turn and nothing is rendered.
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    assert_eq!(drain(&mut r).1, 0);
}

#[test]
fn a_newer_page_while_a_render_runs_is_rendered_after_it() {
    // A fast render is allowed to finish; its picture is still delivered.
    let mut r = open_sample();
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    let Next::Work(job) = r.next_action(Instant::now(), false) else {
        panic!()
    };
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    let ev = r.finish_work(job.run());
    assert!(
        matches!(&ev[..], [ContentEvent::PageRendered { page: 2, .. }]),
        "{ev:?}"
    );
    let (ev, _) = drain(&mut r);
    assert!(
        matches!(&ev[..], [ContentEvent::PageRendered { page: 3, .. }]),
        "{ev:?}"
    );

    // With supersede_after = 0 a stale render is cancelled. (In process the renderer cannot
    // be interrupted, so its result is a normal picture, which is fine to deliver.)
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        supersede_after: Duration::ZERO,
        ..ContentOptions::default()
    });
    feed(&mut r, document_transfer("c.pdf", SAMPLE_PDF).unwrap());
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    let Next::Work(job) = r.next_action(Instant::now(), false) else {
        panic!()
    };
    std::thread::sleep(Duration::from_millis(5));
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    let _ = r.finish_work(job.run());
    let (ev, _) = drain(&mut r);
    assert!(
        matches!(ev.last(), Some(ContentEvent::PageRendered { page: 3, .. })),
        "{ev:?}"
    );
}

#[test]
fn renders_keep_a_minimum_interval() {
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        min_render_interval: Duration::from_secs(3600),
        ..ContentOptions::default()
    });
    feed(&mut r, document_transfer("c.pdf", SAMPLE_PDF).unwrap());
    let t0 = Instant::now();
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    // The first render of the document just ran; the next one has to wait.
    assert!(matches!(r.next_action(t0, true), Next::Wait(_)));
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    assert!(matches!(r.next_action(t0, true), Next::Wait(_)));
    assert!(matches!(
        r.next_action(t0 + Duration::from_secs(3601), true),
        Next::Work(_)
    ));
}

#[test]
fn a_refused_transfer_is_one_violation_and_the_channel_stays_usable() {
    // The reviewer's scenario: the receiver takes 1 MiB, the sender honestly sends 8 MiB.
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        max_document_bytes: 1024 * 1024,
        ..ContentOptions::default()
    });
    let mut pdf = b"%PDF-1.4\n".to_vec();
    pdf.resize(8 * 1024 * 1024, b' ');
    let ev = feed(&mut r, document_transfer("big.pdf", &pdf).unwrap());
    assert_eq!(errors(&ev), [ContentError::DocumentRejected], "{ev:?}");
    assert_eq!(r.violations(), 1, "the whole refused transfer counts once");
    assert!(!r.is_blocked());
    // A valid link, and a small PDF, still work.
    let ev = r.handle_text(&link_message("https://example.org/").unwrap());
    assert!(
        matches!(&ev[..], [ContentEvent::LinkRequested { .. }]),
        "{ev:?}"
    );
    let ev = feed(&mut r, document_transfer("m.pdf", MINIMAL).unwrap());
    assert!(errors(&ev).is_empty(), "{ev:?}");
    assert_eq!(r.document_state(), Some((1, 1)));
    // Refused transfers that keep coming still add up and block the receiver.
    let mut blocked = false;
    for _ in 0..100 {
        feed(
            &mut r,
            document_transfer("big.pdf", &pdf[..2 * 1024 * 1024]).unwrap(),
        );
        blocked |= r.is_blocked();
    }
    assert!(blocked);
}

#[test]
fn refused_data_is_bounded_by_the_announced_size() {
    let mut r = rx();
    // Announces 2 chunks of an image that is not allowed.
    r.handle_text(r#"{"kind":"image","name":"a","mime":"image/svg+xml","size":32768}"#);
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    assert_eq!(
        errors(&r.handle_binary(&[0; CHUNK_SIZE])),
        [ContentError::TooLarge]
    );
    // Another message ends the skipping too (the sender gave up on the transfer).
    r.handle_text(r#"{"kind":"image","name":"a","mime":"image/svg+xml","size":32768}"#);
    assert!(r.handle_binary(&[0; CHUNK_SIZE]).is_empty());
    let ev = r.handle_text(&link_message("https://example.org/").unwrap());
    assert!(matches!(&ev[..], [ContentEvent::LinkRequested { .. }]));
    assert_eq!(
        errors(&r.handle_binary(&[0; CHUNK_SIZE])),
        [ContentError::UnexpectedChunk]
    );
    // An absurd announcement is not believed: its chunks are violations again.
    r.handle_text(&format!(
        r#"{{"kind":"image","name":"a","mime":"image/svg+xml","size":{}}}"#,
        MAX_SKIPPED_BYTES + 1
    ));
    assert_eq!(
        errors(&r.handle_binary(&[0; CHUNK_SIZE])),
        [ContentError::UnexpectedChunk]
    );
}

#[test]
fn violations_decay_over_time() {
    let mut r = ContentReceiver::new(ContentOptions {
        max_violations: 5,
        violation_decay: Duration::from_millis(40),
        ..ContentOptions::default()
    });
    for _ in 0..4 {
        r.handle_text("junk");
    }
    assert_eq!(r.violations(), 4);
    std::thread::sleep(Duration::from_millis(250));
    r.handle_text("junk");
    assert_eq!(r.violations(), 1, "the old ones were forgotten");
    // A slow trickle never blocks...
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(60));
        r.handle_text("junk");
    }
    assert!(!r.is_blocked());
    // ...a burst does.
    for _ in 0..10 {
        r.handle_text("junk");
    }
    assert!(r.is_blocked());
}

#[test]
fn a_stalled_transfer_is_dropped_with_an_event() {
    let mut r = ContentReceiver::new(ContentOptions {
        transfer_idle_timeout: Duration::from_millis(30),
        ..ContentOptions::default()
    });
    assert_eq!(r.idle_deadline(), None);
    r.handle_text(&image_start(50_000));
    r.handle_binary(&[0; CHUNK_SIZE]);
    assert!(r.idle_deadline().is_some());
    assert!(r.tick(Instant::now()).is_empty(), "not yet");
    std::thread::sleep(Duration::from_millis(60));
    let ev = r.tick(Instant::now());
    assert_eq!(errors(&ev), [ContentError::TransferTimeout]);
    assert!(!r.transfer_active());
    assert_eq!(r.violations(), 0, "a stall is not a violation");
    assert_eq!(r.idle_deadline(), None);
    // The next transfer works.
    let ev = feed(
        &mut r,
        image_transfer("i.png", ImageMime::Png, &[1; 10]).unwrap(),
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, ContentEvent::ImageReceived { .. }))
    );
}

#[test]
fn opening_documents_is_budgeted() {
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        open_burst: 2,
        open_refill: Duration::from_secs(3600),
        ..ContentOptions::default()
    });
    for _ in 0..2 {
        let ev = feed(&mut r, document_transfer("m.pdf", MINIMAL).unwrap());
        assert!(errors(&ev).is_empty(), "{ev:?}");
    }
    let ev = feed(&mut r, document_transfer("m.pdf", MINIMAL).unwrap());
    assert_eq!(errors(&ev), [ContentError::DocumentRejected], "{ev:?}");
    assert_eq!(r.document_state(), None, "the old document is gone too");
    assert_eq!(r.violations(), 1);
}

#[test]
fn a_refused_document_clears_the_old_one() {
    let mut r = open_sample();
    assert!(r.document_state().is_some());
    let ev = r.handle_text(&doc_start(MAX_DOCUMENT_BYTES + 1));
    assert_eq!(errors(&ev), [ContentError::DocumentRejected]);
    assert_eq!(r.document_state(), None);
}

#[test]
fn a_panicking_job_keeps_the_violation_state() {
    let mut r = open_sample();
    for _ in 0..5 {
        r.handle_text("junk");
    }
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    let Next::Work(job) = r.next_action(Instant::now(), false) else {
        panic!()
    };
    let id = job.id();
    drop(job); // the thread died with it
    let ev = r.work_panicked(id);
    assert_eq!(errors(&ev), [ContentError::RenderFailed]);
    assert_eq!(r.violations(), 5, "nothing was reset");
    // The receiver goes on: the page is asked for again.
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    assert_eq!(drain(&mut r).1, 1);
}

#[test]
fn page_commands_that_arrive_while_a_document_opens_apply_to_it() {
    let mut r = rx();
    // Feed the whole transfer without running the open.
    for item in document_transfer("c.pdf", SAMPLE_PDF).unwrap() {
        match item {
            Outgoing::Text(t) => assert!(errors(&r.feed_text(&t)).is_empty()),
            Outgoing::Binary(b) => assert!(r.feed_binary(b).is_empty()),
        }
    }
    // The sender does not wait for the open: next, go to 3, back one.
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    r.feed_text(&cmd_message(ControlCmd::GoToPage { page: 3 }));
    r.feed_text(&cmd_message(ControlCmd::PrevPage));
    let (ev, jobs) = drain(&mut r);
    assert_eq!(jobs, 2, "one open, one render");
    assert!(
        matches!(&ev[0], ContentEvent::DocumentOpened { pages: 3, .. }),
        "{ev:?}"
    );
    assert!(
        matches!(&ev[1..], [ContentEvent::PageRendered { page: 2, .. }]),
        "{ev:?}"
    );
    // Stop while it opens wins: nothing is shown afterwards.
    for item in document_transfer("c.pdf", SAMPLE_PDF).unwrap() {
        match item {
            Outgoing::Text(t) => drop(r.feed_text(&t)),
            Outgoing::Binary(b) => drop(r.feed_binary(b)),
        }
    }
    r.feed_text(&cmd_message(ControlCmd::NextPage));
    assert_eq!(
        r.feed_text(&cmd_message(ControlCmd::Stop)),
        [ContentEvent::ContentStopped]
    );
    let (ev, _) = drain(&mut r);
    assert!(ev.is_empty(), "{ev:?}");
    assert_eq!(r.document_state(), None);
}

#[test]
fn progress_is_throttled_and_ends_with_the_whole_transfer() {
    let data = vec![7u8; CHUNK_SIZE * 40];
    let mut r = ContentReceiver::new(ContentOptions {
        isolation: Isolation::InProcess,
        progress: true,
        ..ContentOptions::default()
    });
    let ev = feed(
        &mut r,
        image_transfer("a.png", ImageMime::Png, &data).unwrap(),
    );
    let progress: Vec<(u64, u64)> = ev
        .iter()
        .filter_map(|e| match e {
            ContentEvent::TransferProgress { received, total } => Some((*received, *total)),
            _ => None,
        })
        .collect();
    // All chunks arrive within a few microseconds: the first one, and the complete one.
    assert_eq!(progress.len(), 2, "{progress:?}");
    assert_eq!(progress[0].1, data.len() as u64);
    assert_eq!(progress[1], (data.len() as u64, data.len() as u64));
    let started = ev
        .iter()
        .position(|e| matches!(e, ContentEvent::TransferStarted { .. }))
        .unwrap();
    let done = ev
        .iter()
        .position(|e| matches!(e, ContentEvent::ImageReceived { .. }))
        .unwrap();
    assert!(started < done);
    assert!(
        ev[started..done]
            .iter()
            .filter(|e| matches!(e, ContentEvent::TransferProgress { .. }))
            .count()
            == 2
    );
}

#[test]
fn a_page_message_is_ignored_without_a_violation() {
    let mut r = rx();
    let ev = r.handle_text(&crate::outgoing::page_message(1, 3));
    assert!(ev.is_empty(), "{ev:?}");
    assert_eq!(r.violations(), 0);
}
