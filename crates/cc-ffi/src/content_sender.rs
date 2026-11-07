// SPDX-License-Identifier: GPL-3.0-or-later
//! The sending side of content for the Flutter app: an image, a PDF or a link goes over the
//! `control` data channel (`cc-sender`'s `ContentSender`), with a progress report, and then
//! page commands can follow (the presenter controls).
//!
//! The file is read **here**, not in Dart: the bytes never cross the bridge, only the path
//! does, and every rule (type, size, PDF header, link) is checked by Rust before anything is
//! connected. Memory: the file is held once (at most 10 MiB for an image, 64 MiB for a PDF);
//! for a PDF a second copy exists briefly while the page count is read in the render worker
//! (docs/documents.md) and is dropped before sending starts.
//!
//! Progress counts bytes handed to the data channel. The channel itself holds up to about
//! 1 MiB (docs/transport.md), so the bar can be that far ahead of what the receiver has;
//! `Sent` is only reported after the transport confirmed delivery (flush).

use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cc_files::wire::MAX_PAGE_NUMBER;
use cc_files::{
    ImageMime, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, Outgoing, document_transfer, image_transfer,
    link_message,
};
use cc_proto::ControlCmd;
use cc_sender::content::READY_TIMEOUT;
use cc_sender::{ContentSender, ControlSender, SenderEvent};
use tokio::sync::{mpsc, oneshot};

use crate::api::{
    ContentSenderUiEvent, ContentSource, UiContentCmd, UiContentKind, UiSendRejection,
};
use crate::content::{Worker, host_of};
use crate::frb_generated::StreamSink;

/// Commands waiting for the sender task (the presenter's keys).
/// UNCERTAIN: placeholder.
const CMD_QUEUE: usize = 64;

/// How long a stop command may take to leave before the sender closes. UNCERTAIN: placeholder.
const STOP_FLUSH: Duration = Duration::from_secs(5);

/// The command channel of the running content sender, with the token of the start it belongs to.
static CMD_TX: Mutex<Option<(u64, mpsc::Sender<ControlCmd>)>> = Mutex::new(None);
static CMD_TOKEN: AtomicU64 = AtomicU64::new(0);

fn cmd_lock() -> std::sync::MutexGuard<'static, Option<(u64, mpsc::Sender<ControlCmd>)>> {
    CMD_TX.lock().unwrap_or_else(|p| p.into_inner())
}

/// A new command channel for a starting sender (replaces the old one).
pub(crate) fn new_cmd_channel() -> (u64, mpsc::Receiver<ControlCmd>) {
    let (tx, rx) = mpsc::channel(CMD_QUEUE);
    let token = CMD_TOKEN.fetch_add(1, Ordering::Relaxed) + 1;
    *cmd_lock() = Some((token, tx));
    (token, rx)
}

/// Remove the command channel if it still belongs to the start with `token` (a newer start
/// may have replaced it).
fn drop_cmd_channel(token: u64) {
    let mut g = cmd_lock();
    if g.as_ref().is_some_and(|(t, _)| *t == token) {
        *g = None;
    }
}

/// Remove the command channel (the sender was stopped).
pub(crate) fn clear_cmd_channel() {
    *cmd_lock() = None;
}

/// Queue a command for the running content sender. `false` when none runs, the queue is full
/// or the command is not valid (page 0).
pub(crate) fn send_cmd(cmd: UiContentCmd) -> bool {
    let cmd = match cmd {
        UiContentCmd::NextPage => ControlCmd::NextPage,
        UiContentCmd::PrevPage => ControlCmd::PrevPage,
        // The receiver counts page 0 and pages above the limit as protocol violations.
        UiContentCmd::GoToPage { page } if (1..=MAX_PAGE_NUMBER).contains(&page) => {
            ControlCmd::GoToPage { page }
        }
        UiContentCmd::GoToPage { .. } => return false,
        UiContentCmd::Stop => ControlCmd::Stop,
    };
    cmd_lock()
        .as_ref()
        .is_some_and(|(_, tx)| tx.try_send(cmd).is_ok())
}

// ---------------------------------------------------------------------------
// Preparing: read and check, before anything is connected

/// What is sent.
pub(crate) enum Payload {
    Image(ImageMime, Vec<u8>),
    Pdf(Vec<u8>),
    Link,
}

/// A checked item, ready to send.
pub(crate) struct Prepared {
    pub(crate) name: String,
    pub(crate) kind: UiContentKind,
    pub(crate) size: u64,
    /// Pages of a PDF, when the render worker could tell.
    pub(crate) pages: Option<u32>,
    /// Host of a link (punycode form).
    pub(crate) host: Option<String>,
    /// The normalised URL of a link.
    pub(crate) url: Option<String>,
    pub(crate) payload: Payload,
}

/// Image type from the first bytes (the extension of a file name is not trusted).
pub(crate) fn sniff_image(b: &[u8]) -> Option<ImageMime> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(ImageMime::Png)
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageMime::Jpeg)
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some(ImageMime::Gif)
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(ImageMime::Webp)
    } else {
        None
    }
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, UiSendRejection> {
    // `metadata` follows symbolic links, so a link to a device is seen as a device.
    let meta = std::fs::metadata(path).map_err(|_| UiSendRejection::Unreadable)?;
    if !meta.is_file() {
        return Err(UiSendRejection::NotARegularFile);
    }
    if meta.len() > limit {
        return Err(UiSendRejection::TooLarge);
    }
    let file = std::fs::File::open(path).map_err(|_| UiSendRejection::Unreadable)?;
    let mut bytes = Vec::new();
    // At most `limit + 1` bytes: a file that grows while it is read cannot make this read
    // without bound.
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| UiSendRejection::Unreadable)?;
    if bytes.len() as u64 > limit {
        return Err(UiSendRejection::TooLarge);
    }
    if bytes.is_empty() {
        return Err(UiSendRejection::Empty);
    }
    Ok(bytes)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Read and check what the person picked. Blocking (file access, and for a PDF the render
/// worker): run it on a blocking thread.
pub(crate) fn prepare(
    source: &ContentSource,
    worker: &Worker,
) -> Result<Prepared, UiSendRejection> {
    match source {
        ContentSource::Link { url } => {
            let msg_ok = link_message(url).is_ok();
            let url = cc_files::validate_link(url).ok_or(UiSendRejection::BadLink)?;
            let host = host_of(&url).ok_or(UiSendRejection::BadLink)?;
            if !msg_ok {
                return Err(UiSendRejection::BadLink);
            }
            Ok(Prepared {
                name: host.clone(),
                kind: UiContentKind::Link,
                size: url.len() as u64,
                pages: None,
                host: Some(host),
                url: Some(url),
                payload: Payload::Link,
            })
        }
        ContentSource::File { path } => {
            let p = Path::new(path);
            let name = file_name(p);
            if name.to_ascii_lowercase().ends_with(".pdf") {
                let bytes = read_limited(p, MAX_DOCUMENT_BYTES)?;
                document_transfer(&name, &bytes).map_err(|_| UiSendRejection::NotPdf)?;
                let pages = pdf_pages(&bytes, worker)?;
                Ok(Prepared {
                    name,
                    kind: UiContentKind::Pdf,
                    size: bytes.len() as u64,
                    pages,
                    host: None,
                    url: None,
                    payload: Payload::Pdf(bytes),
                })
            } else if ImageMime::from_extension(&name).is_some() {
                let bytes = read_limited(p, MAX_IMAGE_BYTES)?;
                let mime = sniff_image(&bytes).ok_or(UiSendRejection::UnsupportedType)?;
                image_transfer(&name, mime, &bytes).map_err(|_| UiSendRejection::TooLarge)?;
                Ok(Prepared {
                    name,
                    kind: UiContentKind::Image,
                    size: bytes.len() as u64,
                    pages: None,
                    host: None,
                    url: None,
                    payload: Payload::Image(mime, bytes),
                })
            } else {
                Err(UiSendRejection::UnsupportedType)
            }
        }
    }
}

/// Number of pages, asked from the render worker. `Ok(None)`: the worker could not say (not
/// found, timeout, ...), which does not stop the sending: the receiver decides. An error: the
/// PDF cannot be shown by any receiver (password, damaged, ...).
fn pdf_pages(bytes: &[u8], worker: &Worker) -> Result<Option<u32>, UiSendRejection> {
    use cc_docs::{DocError, Document, Limits};
    if !worker.found {
        return Ok(None);
    }
    match Document::open_with_options(bytes.to_vec(), Limits::default(), worker.isolation.clone()) {
        Ok(doc) => Ok(u32::try_from(doc.page_count()).ok()),
        Err(
            DocError::PasswordProtected
            | DocError::UnsupportedEncryption(_)
            | DocError::Invalid
            | DocError::NoPages
            | DocError::TooManyPages { .. }
            | DocError::Rejected(_)
            | DocError::NotPdf
            | DocError::Empty,
        ) => Err(UiSendRejection::DocumentUnusable),
        Err(_) => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Running

/// Why a guarded wait ended early.
enum Interrupt {
    Stop,
    PeerLeft,
    Error(String),
}

/// Run `fut` while forwarding the sender's own events; a stop request, the receiver leaving
/// or an error end it early.
async fn guarded<T>(
    fut: impl std::future::Future<Output = T>,
    sender: &mut ContentSender,
    stop: &mut oneshot::Receiver<()>,
    events: &StreamSink<ContentSenderUiEvent>,
) -> Result<T, Interrupt> {
    tokio::pin!(fut);
    loop {
        tokio::select! {
            biased;
            _ = &mut *stop => return Err(Interrupt::Stop),
            r = &mut fut => return Ok(r),
            ev = sender.next_event() => match ev {
                Some(SenderEvent::State(s)) => {
                    if events.add(ContentSenderUiEvent::ConnectionState { state: s.into() }).is_err() {
                        return Err(Interrupt::Stop);
                    }
                }
                Some(SenderEvent::PeerLeft) => return Err(Interrupt::PeerLeft),
                Some(SenderEvent::Error(m)) => return Err(Interrupt::Error(m)),
                Some(_) => {}
                None => return Err(Interrupt::Error("the connection ended".into())),
            },
        }
    }
}

async fn send_payload(
    ctl: &ControlSender,
    prepared: &Prepared,
    events: &StreamSink<ContentSenderUiEvent>,
) -> Result<(), String> {
    let total = prepared.size;
    let chan = ctl.channel();
    let mut sent = 0u64;
    let mut last_pct = u64::MAX;
    let items: Box<dyn Iterator<Item = Outgoing<'_>> + Send> = match &prepared.payload {
        Payload::Image(mime, bytes) => {
            Box::new(image_transfer(&prepared.name, *mime, bytes).map_err(|e| e.to_string())?)
        }
        Payload::Pdf(bytes) => {
            Box::new(document_transfer(&prepared.name, bytes).map_err(|e| e.to_string())?)
        }
        Payload::Link => {
            let url = prepared.url.as_deref().unwrap_or_default();
            let m = link_message(url).map_err(|e| e.to_string())?;
            chan.send_text(&m).await.map_err(|e| e.to_string())?;
            return finish(ctl, events, total).await;
        }
    };
    for item in items {
        match item {
            Outgoing::Text(t) => chan.send_text(&t).await.map_err(|e| e.to_string())?,
            Outgoing::Binary(b) => {
                chan.send_binary(b).await.map_err(|e| e.to_string())?;
                sent += b.len() as u64;
                // One report per percent at most.
                let pct = sent * 100 / total.max(1);
                if pct != last_pct {
                    last_pct = pct;
                    if events
                        .add(ContentSenderUiEvent::Progress { sent, total })
                        .is_err()
                    {
                        return Err("the app stopped listening".into());
                    }
                }
            }
        }
    }
    finish(ctl, events, total).await
}

async fn finish(
    ctl: &ControlSender,
    events: &StreamSink<ContentSenderUiEvent>,
    total: u64,
) -> Result<(), String> {
    let _ = events.add(ContentSenderUiEvent::Progress { sent: total, total });
    if ctl.finish().await {
        Ok(())
    } else {
        Err("the data did not reach the receiver in time".into())
    }
}

/// The whole life of a content sender after it joined the room: wait for the channel, send,
/// then pass on the presenter's commands until the person stops, the receiver leaves or
/// something fails. Ends the sender.
pub(crate) async fn drive(
    mut sender: ContentSender,
    prepared: Prepared,
    events: StreamSink<ContentSenderUiEvent>,
    mut stop: oneshot::Receiver<()>,
    mut cmds: mpsc::Receiver<ControlCmd>,
    token: u64,
) {
    let ctl = sender.control();
    let end = async {
        match guarded(ctl.wait_ready(READY_TIMEOUT), &mut sender, &mut stop, &events).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Some(ContentSenderUiEvent::Error { message: e.to_string() }),
            Err(i) => return interrupt_event(i),
        }
        if events.add(ContentSenderUiEvent::Connected).is_err() {
            return None;
        }
        match guarded(send_payload(&ctl, &prepared, &events), &mut sender, &mut stop, &events)
            .await
        {
            Ok(Ok(())) => {}
            Ok(Err(message)) => return Some(ContentSenderUiEvent::Error { message }),
            Err(i) => return interrupt_event(i),
        }
        if events.add(ContentSenderUiEvent::Sent).is_err() {
            return None;
        }
        // The presenter's commands. The sender stays connected: a receiver drops the shown
        // content when the sender leaves.
        loop {
            let cmd = tokio::select! {
                biased;
                _ = &mut stop => return None,
                c = cmds.recv() => c,
                ev = sender.next_event() => match ev {
                    Some(SenderEvent::State(s)) => {
                        if events.add(ContentSenderUiEvent::ConnectionState { state: s.into() }).is_err() {
                            return None;
                        }
                        continue;
                    }
                    Some(SenderEvent::PeerLeft) => return Some(ContentSenderUiEvent::PeerLeft),
                    Some(SenderEvent::Error(message)) => {
                        return Some(ContentSenderUiEvent::Error { message });
                    }
                    Some(_) => continue,
                    None => return Some(ContentSenderUiEvent::Error {
                        message: "the connection ended".into(),
                    }),
                },
            };
            let cmd = cmd?;
            let is_stop = cmd == ControlCmd::Stop;
            match guarded(ctl.send_cmd(cmd), &mut sender, &mut stop, &events).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Some(ContentSenderUiEvent::Error { message: e.to_string() }),
                Err(i) => return interrupt_event(i),
            }
            if is_stop {
                let _ = ctl.flush(STOP_FLUSH).await;
                return Some(ContentSenderUiEvent::Stopped);
            }
        }
    }
    .await;
    if let Some(e) = end {
        let _ = events.add(e);
    }
    drop_cmd_channel(token);
    sender.stop().await;
}

fn interrupt_event(i: Interrupt) -> Option<ContentSenderUiEvent> {
    match i {
        Interrupt::Stop => None,
        Interrupt::PeerLeft => Some(ContentSenderUiEvent::PeerLeft),
        Interrupt::Error(message) => Some(ContentSenderUiEvent::Error { message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_worker() -> Worker {
        Worker {
            isolation: cc_docs::Isolation::Auto,
            found: false,
        }
    }

    fn file(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p.to_string_lossy().into_owned()
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    #[test]
    fn images_are_checked_by_content_not_by_name() {
        let d = tempfile::tempdir().unwrap();
        let ok = file(d.path(), "a.png", PNG);
        let p = prepare(&ContentSource::File { path: ok }, &no_worker()).unwrap();
        assert_eq!(p.kind, UiContentKind::Image);
        assert_eq!(p.name, "a.png");
        // A JPEG named .png is sent with the type of its bytes.
        let jpg = file(d.path(), "b.png", &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]);
        match prepare(&ContentSource::File { path: jpg }, &no_worker())
            .unwrap()
            .payload
        {
            Payload::Image(m, _) => assert_eq!(m, ImageMime::Jpeg),
            _ => panic!("image expected"),
        }
        // Text named .png is refused.
        let bad = file(d.path(), "c.png", b"hello world, not a picture");
        assert_eq!(
            prepare(&ContentSource::File { path: bad }, &no_worker()).err(),
            Some(UiSendRejection::UnsupportedType)
        );
        // An unknown extension is refused (SVG can carry script).
        let svg = file(d.path(), "d.svg", b"<svg/>");
        assert_eq!(
            prepare(&ContentSource::File { path: svg }, &no_worker()).err(),
            Some(UiSendRejection::UnsupportedType)
        );
    }

    #[test]
    fn files_that_cannot_be_sent_are_named() {
        let d = tempfile::tempdir().unwrap();
        let empty = file(d.path(), "e.png", b"");
        assert_eq!(
            prepare(&ContentSource::File { path: empty }, &no_worker()).err(),
            Some(UiSendRejection::Empty)
        );
        let missing = d.path().join("nope.png").to_string_lossy().into_owned();
        assert_eq!(
            prepare(&ContentSource::File { path: missing }, &no_worker()).err(),
            Some(UiSendRejection::Unreadable)
        );
        let dir = d.path().to_string_lossy().into_owned() + "/sub.png";
        std::fs::create_dir(&dir).unwrap();
        assert_eq!(
            prepare(&ContentSource::File { path: dir }, &no_worker()).err(),
            Some(UiSendRejection::NotARegularFile)
        );
        let big = vec![0u8; usize::try_from(MAX_IMAGE_BYTES).unwrap() + 1];
        let big = file(d.path(), "big.png", &big);
        assert_eq!(
            prepare(&ContentSource::File { path: big }, &no_worker()).err(),
            Some(UiSendRejection::TooLarge)
        );
        let notpdf = file(d.path(), "x.pdf", b"this is not a pdf");
        assert_eq!(
            prepare(&ContentSource::File { path: notpdf }, &no_worker()).err(),
            Some(UiSendRejection::NotPdf)
        );
    }

    #[test]
    fn a_pdf_without_a_worker_is_still_sendable_without_a_page_count() {
        let d = tempfile::tempdir().unwrap();
        let pdf = file(d.path(), "x.pdf", b"%PDF-1.4\n% not parsed here\n");
        let p = prepare(&ContentSource::File { path: pdf }, &no_worker()).unwrap();
        assert_eq!(p.kind, UiContentKind::Pdf);
        assert_eq!(p.pages, None);
    }

    #[test]
    fn links_are_validated_here_and_the_host_is_returned() {
        let p = prepare(
            &ContentSource::Link {
                url: " https://Example.org/a ".into(),
            },
            &no_worker(),
        )
        .unwrap();
        assert_eq!(p.kind, UiContentKind::Link);
        assert_eq!(p.host.as_deref(), Some("example.org"));
        assert_eq!(p.url.as_deref(), Some("https://example.org/a"));
        for bad in [
            "javascript:alert(1)",
            "ftp://a.example/",
            "http://u:p@a.example/",
            "x",
        ] {
            assert_eq!(
                prepare(&ContentSource::Link { url: bad.into() }, &no_worker()).err(),
                Some(UiSendRejection::BadLink),
                "{bad}"
            );
        }
    }

    #[test]
    fn commands_need_a_running_sender_and_a_valid_page() {
        clear_cmd_channel();
        assert!(!send_cmd(UiContentCmd::NextPage));
        let (_token, mut rx) = new_cmd_channel();
        assert!(send_cmd(UiContentCmd::NextPage));
        assert!(send_cmd(UiContentCmd::GoToPage { page: 3 }));
        assert!(!send_cmd(UiContentCmd::GoToPage { page: 0 }));
        assert!(!send_cmd(UiContentCmd::GoToPage {
            page: MAX_PAGE_NUMBER + 1
        }));
        assert!(send_cmd(UiContentCmd::GoToPage {
            page: MAX_PAGE_NUMBER
        }));
        assert_eq!(rx.try_recv().unwrap(), ControlCmd::NextPage);
        assert_eq!(rx.try_recv().unwrap(), ControlCmd::GoToPage { page: 3 });
        assert_eq!(
            rx.try_recv().unwrap(),
            ControlCmd::GoToPage {
                page: MAX_PAGE_NUMBER
            }
        );
        // The queue is bounded: a flood is refused, not stored.
        let accepted = (0..CMD_QUEUE * 2)
            .filter(|_| send_cmd(UiContentCmd::PrevPage))
            .count();
        assert_eq!(accepted, CMD_QUEUE);
        clear_cmd_channel();
    }
}
