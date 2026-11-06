// SPDX-License-Identifier: GPL-3.0-or-later
//! Content over the `control` data channel, native sender to native receiver, against a real
//! cc-server in this process and real UDP on 127.0.0.1: images, links, a PDF with page
//! commands, stop, a slow reader (back-pressure), and a hostile sender.
//!
//! PDFs are rendered in the child process of cc-docs (the worker program `cc-docs-worker`,
//! found next to the test binaries; run `cargo build --workspace --bins` first). If the worker
//! is missing or does not work the tests **fail**: silently rendering in process would test a
//! different, unsandboxed path. Set `COMETCAST_ALLOW_INPROCESS_RENDER=1` to allow the
//! in-process fallback on purpose (for example on a platform without the worker).

use std::time::Duration;

use cc_docs::{Isolation, WorkerSpec};
use cc_proto::ControlCmd;
use cc_receiver::{
    ContentError, ContentEvent, ContentEvents, ContentOptions, ImageMime, ReceiverCore,
    ReceiverEvent, ReceiverOptions,
};
use cc_sender::{ContentSender, ContentSenderOptions, ControlSender, SenderCore, SenderOptions};
use cc_server::{ServerConfig, run};
use cc_transport::SessionConfig;
use tokio::net::TcpListener;

const MINIMAL: &[u8] = include_bytes!("../../cc-docs/tests/data/minimal.pdf");
const CLASSROOM: &[u8] = include_bytes!("../../cc-docs/tests/data/classroom.pdf");

async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(
        listener,
        ServerConfig::default(),
        std::future::pending(),
    ));
    addr.to_string()
}

fn local_session() -> SessionConfig {
    SessionConfig {
        udp_bind: vec!["127.0.0.1:0".into()],
        ..SessionConfig::default()
    }
}

/// A private copy of the render worker, made once per test run. The build directory can be
/// rebuilt (for example by another cargo process) while the tests run, which would break a
/// worker that runs from there.
/// `COMETCAST_ALLOW_INPROCESS_RENDER=1` allows rendering in process when no worker works.
fn allow_inprocess() -> bool {
    std::env::var("COMETCAST_ALLOW_INPROCESS_RENDER").is_ok_and(|v| v == "1")
}

fn private_worker() -> Option<&'static std::path::PathBuf> {
    static COPY: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    COPY.get_or_init(|| {
        let found = WorkerSpec::discover()?;
        let copy = std::env::temp_dir().join(format!("cc-docs-worker-test-{}", std::process::id()));
        std::fs::copy(&found.program, &copy).ok()?;
        // The worker in a shared build directory can come from a different version of
        // cc-docs (another checkout's build); then it cannot talk to this test. Use it only
        // when it works.
        let probe = cc_docs::Document::open_with_options(
            MINIMAL.to_vec(),
            cc_docs::Limits::default(),
            Isolation::Process(WorkerSpec::new(copy.clone())),
        );
        if let Err(e) = probe {
            assert!(
                allow_inprocess(),
                "the found cc-docs-worker does not work ({e}); build the current one with \
                 `cargo build --workspace --bins`, or set COMETCAST_ALLOW_INPROCESS_RENDER=1"
            );
            eprintln!("note: the found cc-docs-worker does not work, rendering in process");
            return None;
        }
        Some(copy)
    })
    .as_ref()
}

fn content_options() -> ContentOptions {
    let isolation = if let Some(copy) = private_worker() {
        Isolation::Process(WorkerSpec::new(copy.clone()))
    } else {
        assert!(
            allow_inprocess(),
            "cc-docs-worker not found; run `cargo build --workspace --bins` first, or set \
             COMETCAST_ALLOW_INPROCESS_RENDER=1 to render in process"
        );
        eprintln!("note: cc-docs-worker not found, rendering in process");
        Isolation::InProcess
    };
    ContentOptions {
        isolation,
        target_width: 300,
        ..ContentOptions::default()
    }
}

async fn start_receiver(server: &str) -> Rx {
    start_receiver_with(server, content_options()).await
}

async fn start_receiver_with(server: &str, content: ContentOptions) -> Rx {
    let mut o = ReceiverOptions::new(server);
    o.session = local_session();
    o.content = Some(content);
    Rx::new(ReceiverCore::start(o).await.expect("receiver start"))
}

async fn start_content_sender(server: &str, receiver: &Rx) -> ContentSender {
    let mut o = ContentSenderOptions::new(server, receiver.core.room_code().clone());
    o.session = local_session();
    let s = ContentSender::start(o).await.expect("sender start");
    s.control()
        .wait_ready(Duration::from_secs(20))
        .await
        .expect("control channel opens");
    s
}

/// A receiver core and its content stream.
struct Rx {
    core: ReceiverCore,
    content: ContentEvents,
}

impl Rx {
    fn new(mut core: ReceiverCore) -> Self {
        let content = core.take_content_events().expect("content enabled");
        Self { core, content }
    }
}

/// Next content event within 20 s, skipping transfer-start notices.
async fn next(rx: &mut Rx) -> ContentEvent {
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                tokio::select! {
                    e = rx.content.next() => return e.expect("content stream ended"),
                    e = rx.core.next_event() => if let Some(ReceiverEvent::Error(m)) = e {
                        panic!("receiver error: {m}");
                    },
                }
            }
        })
        .await
        .expect("timeout waiting for a content event");
        if !matches!(ev, ContentEvent::TransferStarted { .. }) {
            return ev;
        }
    }
}

/// A small valid PNG (1 x 1 pixel).
fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0,
        0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 13, 0x49, 0x44, 0x41,
        0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0, 5, 0, 1, 0xff, 0x89, 0x99, 0x3d,
        0x1d, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn image_link_pdf_commands_and_stop() {
    let server = start_server().await;
    let mut rx = start_receiver(&server).await;
    let tx = start_content_sender(&server, &rx).await;
    let ctl = tx.control();

    // Image, byte for byte.
    let png = tiny_png();
    ctl.send_image("pixel.png", ImageMime::Png, &png)
        .await
        .unwrap();
    match next(&mut rx).await {
        ContentEvent::ImageReceived { name, mime, bytes } => {
            assert_eq!((name.as_str(), mime), ("pixel.png", ImageMime::Png));
            assert_eq!(&bytes[..], &png[..]);
        }
        other => panic!("{other:?}"),
    }

    // Link: reported, validated on both sides.
    ctl.send_link("https://example.org/a?b=1").await.unwrap();
    assert_eq!(
        next(&mut rx).await,
        ContentEvent::LinkRequested {
            url: "https://example.org/a?b=1".into()
        }
    );
    assert!(ctl.send_link("javascript:alert(1)").await.is_err());
    assert!(ctl.send_image("x", ImageMime::Png, &[]).await.is_err());
    assert!(ctl.send_document("x.pdf", b"not a pdf").await.is_err());

    // PDF: opens, first page rendered at the target width, commands turn pages.
    ctl.send_document("class.pdf", CLASSROOM).await.unwrap();
    assert_eq!(
        next(&mut rx).await,
        ContentEvent::DocumentOpened {
            name: "class.pdf".into(),
            pages: 3
        }
    );
    let page = |e: ContentEvent| match e {
        ContentEvent::PageRendered {
            page,
            width,
            height,
            rgba,
        } => {
            assert_eq!(width, 300);
            assert_eq!(rgba.len(), (width * height * 4) as usize);
            assert!(
                rgba.chunks(4).any(|p| p[..3] != [255, 255, 255]),
                "not blank"
            );
            page
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(page(next(&mut rx).await), 1);
    ctl.send_cmd(ControlCmd::NextPage).await.unwrap();
    assert_eq!(page(next(&mut rx).await), 2);
    ctl.send_cmd(ControlCmd::GoToPage { page: 3 })
        .await
        .unwrap();
    assert_eq!(page(next(&mut rx).await), 3);
    ctl.send_cmd(ControlCmd::PrevPage).await.unwrap();
    assert_eq!(page(next(&mut rx).await), 2);
    // A larger window: same page again, wider.
    rx.core.set_content_target_width(300);
    ctl.send_cmd(ControlCmd::GoToPage { page: 1 })
        .await
        .unwrap();
    assert_eq!(page(next(&mut rx).await), 1);

    // A one-page PDF replaces it; stop ends the content.
    ctl.send_document("min.pdf", MINIMAL).await.unwrap();
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::DocumentOpened { pages: 1, .. }
    ));
    assert_eq!(page(next(&mut rx).await), 1);
    ctl.send_cmd(ControlCmd::Stop).await.unwrap();
    assert_eq!(next(&mut rx).await, ContentEvent::ContentStopped);

    // A PDF the renderer refuses is an error event, and the session carries on.
    let mut junk = b"%PDF-1.7\n".to_vec();
    junk.extend((0..9000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8));
    ctl.send_document("junk.pdf", &junk).await.unwrap();
    match next(&mut rx).await {
        ContentEvent::Error { error, .. } => assert_eq!(error, ContentError::DocumentRejected),
        other => panic!("{other:?}"),
    }
    ctl.send_link("http://example.com/").await.unwrap();
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::LinkRequested { .. }
    ));

    assert!(ctl.finish().await);
    tx.stop().await;
    rx.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_reader_holds_the_sender_back_and_loses_nothing() {
    // One transfer is buffered by the receiver whole (up to its size limit), so back-pressure
    // shows with many transfers: 48 images of 1 MiB are far more than the event queue (8),
    // the library queue (256 messages), the inbox (64) and the send buffer (1 MiB) hold.
    const COUNT: u8 = 48;
    let server = start_server().await;
    let mut rx = start_receiver(&server).await;
    let tx = start_content_sender(&server, &rx).await;
    let ctl = tx.control();
    let image = |n: u8| -> Vec<u8> {
        (0..1024 * 1024u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8 ^ n)
            .collect()
    };

    let sender = ctl.clone();
    let send = tokio::spawn(async move {
        for n in 0..COUNT {
            sender
                .send_image("big.png", ImageMime::Png, &image(n))
                .await
                .unwrap();
        }
    });
    // Do not read for a while.
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        !send.is_finished(),
        "the sender must be held back by the full queues"
    );
    for n in 0..COUNT {
        match next(&mut rx).await {
            ContentEvent::ImageReceived { bytes, .. } => {
                assert!(bytes[..] == image(n)[..], "image {n} differs");
            }
            other => panic!("{other:?}"),
        }
    }
    send.await.unwrap();
    assert!(ctl.finish().await);
    tx.stop().await;
    rx.core.stop().await;
}

/// A sender that breaks the rules on purpose, through the raw channel.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hostile_sender_gets_errors_then_is_cut_off() {
    let server = start_server().await;
    let mut rx = start_receiver(&server).await;
    let tx = start_content_sender(&server, &rx).await;
    let ch = tx.control().channel().clone();

    // Oversize announcement, junk, a chunk without transfer, and a transfer that lies.
    ch.send_text(r#"{"kind":"image","name":"x","mime":"image/png","size":99999999999}"#)
        .await
        .unwrap();
    ch.send_text("not json").await.unwrap();
    ch.send_text(r#"{"kind":"image","name":"x","mime":"image/svg+xml","size":5}"#)
        .await
        .unwrap();
    ch.send_text(r#"{"kind":"image","name":"x","mime":"image/png","size":20000}"#)
        .await
        .unwrap();
    ch.send_binary(&[0; 16384]).await.unwrap();
    ch.send_binary(&[0; 16384]).await.unwrap(); // more than announced
    let mut codes = Vec::new();
    for _ in 0..4 {
        match next(&mut rx).await {
            ContentEvent::Error { error, .. } => codes.push(error),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        codes,
        [
            ContentError::BadMessage,
            ContentError::BadMessage,
            ContentError::BadMessage,
            ContentError::TooLarge,
        ]
    );

    // A flood: after enough violations the receiver closes the channel.
    for _ in 0..200 {
        if ch.send_binary(&[0; 16384]).await.is_err() {
            break;
        }
    }
    let mut blocked = false;
    for _ in 0..300 {
        if let ContentEvent::Error {
            error: ContentError::TooManyErrors,
            ..
        } = next(&mut rx).await
        {
            blocked = true;
            break;
        }
    }
    assert!(blocked, "too_many_errors was reported");
    // The receiver closed its side: the sender's channel ends.
    let closed = tokio::time::timeout(Duration::from_secs(10), async {
        while !ch.is_closed() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(closed.is_ok(), "channel was closed by the receiver");
    tx.stop().await;
    rx.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_control_channel_works_next_to_video() {
    let server = start_server().await;
    let mut rx = start_receiver(&server).await;
    let mut o = SenderOptions::new(&server, rx.core.room_code().clone());
    o.session = local_session();
    o.control = true;
    let sender = SenderCore::start(o).await.expect("sender start");
    let ctl: ControlSender = sender.control().expect("control handle");
    ctl.wait_ready(Duration::from_secs(20)).await.unwrap();
    ctl.send_link("https://example.org/").await.unwrap();
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::LinkRequested { .. }
    ));
    // Video flows on the same connection.
    let saw_video = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Some(ReceiverEvent::Stats(s)) = rx.core.next_event().await
                && s.packets > 5
            {
                return;
            }
        }
    })
    .await;
    assert!(
        saw_video.is_ok(),
        "video packets arrived next to the channel"
    );
    sender.stop().await;
    rx.core.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_receiver_without_content_refuses_the_channel() {
    let server = start_server().await;
    let mut o = ReceiverOptions::new(&server);
    o.session = local_session();
    let rx = ReceiverCore::start(o).await.expect("receiver start");
    let mut so = ContentSenderOptions::new(&server, rx.room_code().clone());
    so.session = local_session();
    let tx = ContentSender::start(so).await.expect("sender start");
    // The channel opens at the transport level and is closed by the receiver.
    let _ = tx.control().wait_ready(Duration::from_secs(20)).await;
    let closed = tokio::time::timeout(Duration::from_secs(10), async {
        while !tx.control().channel().is_closed() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(closed.is_ok(), "the receiver closed the channel");
    let _ = tx.control().send_link("https://example.org/").await;
    tx.stop().await;
    rx.stop().await;
}

/// 1000 page commands in a row are merged into a few renders, and `stop` is honoured at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_flood_of_page_commands_is_merged_and_stop_is_prompt() {
    let server = start_server().await;
    let mut rx = start_receiver(&server).await;
    let tx = start_content_sender(&server, &rx).await;
    let ctl = tx.control();
    ctl.send_document("class.pdf", CLASSROOM).await.unwrap();
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::DocumentOpened { pages: 3, .. }
    ));
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::PageRendered { page: 1, .. }
    ));

    let started = std::time::Instant::now();
    for i in 0..1000 {
        let cmd = if i % 2 == 0 {
            ControlCmd::NextPage
        } else {
            ControlCmd::PrevPage
        };
        ctl.send_cmd(cmd).await.unwrap();
    }
    ctl.send_cmd(ControlCmd::Stop).await.unwrap();
    let mut renders = 0;
    loop {
        match next(&mut rx).await {
            ContentEvent::PageRendered { .. } => renders += 1,
            ContentEvent::ContentStopped => break,
            other => panic!("{other:?}"),
        }
    }
    // Before: one render per command (1000 x 100+ ms). Now a handful.
    assert!(renders <= 40, "{renders} renders for 1000 commands");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "stop was queued behind renders: {:?}",
        started.elapsed()
    );
    // The channel is still fine afterwards.
    ctl.send_link("https://example.org/").await.unwrap();
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::LinkRequested { .. }
    ));
    assert!(ctl.finish().await);
    tx.stop().await;
    rx.core.stop().await;
}

/// One refused transfer does not block the channel (the reviewer's scenario: the receiver takes
/// 1 MiB, the sender honestly sends an 8 MiB PDF).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_document_does_not_block_the_channel() {
    let server = start_server().await;
    let mut opts = content_options();
    opts.max_document_bytes = 1024 * 1024;
    let mut rx = start_receiver_with(&server, opts).await;
    let tx = start_content_sender(&server, &rx).await;
    let ctl = tx.control();
    let mut big = b"%PDF-1.4\n".to_vec();
    big.resize(8 * 1024 * 1024, b' ');
    ctl.send_document("big.pdf", &big).await.unwrap();
    match next(&mut rx).await {
        ContentEvent::Error { error, .. } => assert_eq!(error, ContentError::DocumentRejected),
        other => panic!("{other:?}"),
    }
    ctl.send_link("https://example.org/after").await.unwrap();
    // Exactly the one error above: the 512 chunks and the end message made no noise.
    assert_eq!(
        next(&mut rx).await,
        ContentEvent::LinkRequested {
            url: "https://example.org/after".into()
        }
    );
    ctl.send_document("min.pdf", MINIMAL).await.unwrap();
    assert!(matches!(
        next(&mut rx).await,
        ContentEvent::DocumentOpened { pages: 1, .. }
    ));
    assert!(ctl.finish().await);
    tx.stop().await;
    rx.core.stop().await;
}
