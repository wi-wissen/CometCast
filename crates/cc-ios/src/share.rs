// SPDX-License-Identifier: GPL-3.0-or-later
//! The engine of the iOS/iPadOS **share extension** (native-apps.md 21.3, decision N82): the
//! sheet joins the room on its own connection, shows one item (an image, a PDF or a link) and
//! stays open as the remote; closing it stops and leaves.
//!
//! The same shape as the broadcast engine ([`crate::engine`]): one process-wide session, a
//! small tokio runtime, **polling** instead of callbacks. The Swift side calls
//! [`start`], [`present`], [`command`], [`poll`] and [`stop`]; every event is one JSON line
//! with the shapes of `PresenterUiEvent` of the Flutter app (`"event"` names it):
//!
//! | `event` | other fields |
//! | --- | --- |
//! | `waiting` | `check`, `timeout_secs`, `room_name` |
//! | `joined` | `room_name` |
//! | `connection_code` | `sas` |
//! | `page` | `page`, `pages` |
//! | `content_prepared` | `name`, `kind` (`image`, `pdf`, `link`), `size`, `host` |
//! | `content_rejected` | `reason` (`unreadable`, `not_a_regular_file`, `empty`, `too_large`, `unsupported_type`, `not_pdf`, `bad_link`) |
//! | `progress` | `sent`, `total` |
//! | `content_sent` | |
//! | `presenting` | |
//! | `stopped` | (the presentation ended, the session is still joined) |
//! | `ended` | `code` (`removed_by_host`, `display_left`) |
//! | `error` | `code`, `message` (logs only) |
//!
//! No Office files (N81: no child processes on iOS) and no PDF page count (no render worker; the
//! display reports the page count itself). Memory: the file is read once, at most 10 MiB for an
//! image and 64 MiB for a PDF; the 64 MiB PDF is the measurement item of docs/ios.md.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cc_files::{
    ImageMime, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, Outgoing, document_transfer, image_transfer,
    link_message,
};
use cc_proto::{ControlCmd, RoomCode};
use cc_sender::content::READY_TIMEOUT;
use cc_sender::{
    ControlSender, JoinProgress, Presentation, PresenterEvent, PresenterOptions, PresenterSession,
};
use serde_json::{Value, json};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::engine::ErrorCode;
use crate::log::Log;

/// Events that wait for [`poll`] at most; older ones are dropped (the sheet polls several times a
/// second, so this is only a bound against a sheet that stopped polling).
const MAX_QUEUED_EVENTS: usize = 256;

/// How long [`stop`] waits for the session to close.
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

/// What [`start`] needs.
#[derive(Debug, Clone, Default)]
pub struct ShareConfig {
    /// The room code.
    pub room_code: String,
    /// Relay (signalling server) as `host:port` or a `ws://` / `wss://` URL.
    pub relay_url: String,
    /// The person's name for the display's request (`None` = "Guest N").
    pub name: Option<String>,
    /// Directory for the log file (the App Group container). `None` = no log.
    pub log_dir: Option<PathBuf>,
    /// Tests only: bind the WebRTC UDP socket to this local address.
    pub udp_bind: Option<String>,
}

/// What the sheet shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A file in the App Group container (image or PDF).
    File(PathBuf),
    /// A web link.
    Link(String),
}

/// A page command of the remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Previous page.
    Prev,
    /// Next page.
    Next,
}

enum Cmd {
    Present(Item),
    Page(Command),
}

struct Running {
    runtime: Runtime,
    cmds: mpsc::UnboundedSender<Cmd>,
    events: Arc<Mutex<VecDeque<String>>>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    log: Arc<Log>,
}

static CURRENT: Mutex<Option<Running>> = Mutex::new(None);

fn slot() -> MutexGuard<'static, Option<Running>> {
    CURRENT.lock().unwrap_or_else(|p| p.into_inner())
}

/// Start the session: join the room in the background (events arrive through [`poll`]). Only the
/// code and the relay are checked here.
pub fn start(config: ShareConfig) -> Result<(), ErrorCode> {
    let code = RoomCode::parse(&config.room_code).map_err(|_| ErrorCode::InvalidRoomCode)?;
    if config.relay_url.trim().is_empty() {
        return Err(ErrorCode::InvalidRelay);
    }
    let mut guard = slot();
    if guard.as_ref().is_some_and(|r| r.task.is_finished())
        && let Some(old) = guard.take()
    {
        old.runtime.shutdown_background();
    }
    if guard.is_some() {
        return Err(ErrorCode::AlreadyRunning);
    }
    let log = Arc::new(Log::open(config.log_dir.clone()));
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        // 2 MiB (the broadcast engine uses 512 KiB): the session task and the content sender are
        // large futures and overflowed 512 KiB in a debug build. Stack is address space, only
        // the touched part counts against the extension's memory limit (spike 4, docs/ios.md).
        .thread_stack_size(2 * 1024 * 1024)
        .thread_name("cc-ios-share")
        .enable_all()
        .build()
        .map_err(|e| {
            log.line(&format!("share: runtime failed: {e}"));
            ErrorCode::Internal
        })?;
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (stop_tx, stop_rx) = oneshot::channel();
    log.line(&format!("share: start, relay {}", config.relay_url));
    let task = runtime.spawn(run(
        config,
        code,
        Arc::clone(&events),
        Arc::clone(&log),
        cmd_rx,
        stop_rx,
    ));
    *guard = Some(Running {
        runtime,
        cmds: cmd_tx,
        events,
        stop: Some(stop_tx),
        task,
        log,
    });
    Ok(())
}

/// Show an item (after `joined`). `false` when no session runs.
pub fn present(item: Item) -> bool {
    slot()
        .as_ref()
        .is_some_and(|r| r.cmds.send(Cmd::Present(item)).is_ok())
}

/// A page command for the shown PDF. `false` when no session runs.
pub fn command(c: Command) -> bool {
    slot()
        .as_ref()
        .is_some_and(|r| r.cmds.send(Cmd::Page(c)).is_ok())
}

/// The oldest event that waits, as one JSON line, or `None`.
pub fn poll() -> Option<String> {
    let guard = slot();
    let r = guard.as_ref()?;
    r.events
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .pop_front()
}

/// [`poll`] into a C buffer: the event as UTF-8 plus a NUL byte. Returns the length of the text
/// (more than 0), 0 when no event waits, or the **negative** size the buffer needs when the next
/// event does not fit (the event stays queued: call again with a bigger buffer).
pub fn poll_into(buf: &mut [u8]) -> i32 {
    let guard = slot();
    let Some(r) = guard.as_ref() else { return 0 };
    let mut q = r.events.lock().unwrap_or_else(|p| p.into_inner());
    let Some(front) = q.front() else { return 0 };
    let n = front.len();
    if n + 1 > buf.len() {
        return -i32::try_from(n + 1).unwrap_or(i32::MAX);
    }
    buf[..n].copy_from_slice(front.as_bytes());
    buf[n] = 0;
    q.pop_front();
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// Stop the item and leave the room ("Stop and close", and a sheet that is swiped away). Blocks
/// for a few seconds at most; safe when nothing runs. Never call it from the thread that polls
/// while another thread is inside [`poll`].
pub fn stop() {
    let running = slot().take();
    let Some(mut running) = running else { return };
    running.log.line("share: stop");
    if let Some(tx) = running.stop.take() {
        let _ = tx.send(());
    }
    let task = running.task;
    let _ = running
        .runtime
        .block_on(async { tokio::time::timeout(STOP_TIMEOUT, task).await });
    running.runtime.shutdown_timeout(Duration::from_secs(1));
}

// ---------------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------------

type Queue = Arc<Mutex<VecDeque<String>>>;

fn push(queue: &Queue, log: &Log, ev: &Value) {
    let line = ev.to_string();
    log.line(&format!("share event: {}", ev["event"]));
    let mut q = queue.lock().unwrap_or_else(|p| p.into_inner());
    if q.len() >= MAX_QUEUED_EVENTS {
        q.pop_front();
    }
    q.push_back(line);
}

async fn run(
    config: ShareConfig,
    code: RoomCode,
    queue: Queue,
    log: Arc<Log>,
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    mut stop: oneshot::Receiver<()>,
) {
    let mut options = PresenterOptions::new(config.relay_url.clone(), code);
    if let Some(b) = config.udp_bind.clone() {
        options.session.udp_bind = vec![b];
    }
    options.name = config.name.clone().filter(|n| !n.trim().is_empty());
    let (ptx, mut prx) = mpsc::unbounded_channel();
    options.join_progress = Some(ptx);
    let waiting_queue = Arc::clone(&queue);
    let waiting_log = Arc::clone(&log);
    tokio::spawn(async move {
        while let Some(JoinProgress::Pending {
            timeout_secs,
            check,
            room_name,
        }) = prx.recv().await
        {
            push(
                &waiting_queue,
                &waiting_log,
                &json!({"event": "waiting", "check": check, "timeout_secs": timeout_secs, "room_name": room_name}),
            );
        }
    });
    let joined = tokio::select! {
        r = PresenterSession::join(options) => r,
        _ = &mut stop => return,
    };
    let mut session = match joined {
        Ok(s) => s,
        Err(e) => {
            push(
                &queue,
                &log,
                &json!({"event": "error", "code": e.code(), "message": e.to_string()}),
            );
            return;
        }
    };
    push(
        &queue,
        &log,
        &json!({"event": "joined", "room_name": session.room_name()}),
    );
    let mut control: Option<ControlSender> = None;
    let mut send_job: Option<JoinHandle<()>> = None;
    let (job_tx, mut job_rx) = mpsc::unbounded_channel::<Result<Prepared, &'static str>>();
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    Cmd::Present(item) => {
                        if let Some(j) = send_job.take() { j.abort(); }
                        let tx = job_tx.clone();
                        tokio::task::spawn_blocking(move || {
                            let _ = tx.send(prepare(&item));
                        });
                    }
                    Cmd::Page(c) => {
                        if let Some(ctl) = control.clone() {
                            let cmd = match c {
                                Command::Prev => ControlCmd::PrevPage,
                                Command::Next => ControlCmd::NextPage,
                            };
                            tokio::spawn(async move { let _ = ctl.send_cmd(cmd).await; });
                        }
                    }
                }
            }
            job = job_rx.recv() => {
                let Some(job) = job else { continue };
                let prepared = match job {
                    Ok(p) => p,
                    Err(reason) => {
                        push(&queue, &log, &json!({"event": "content_rejected", "reason": reason}));
                        continue;
                    }
                };
                push(&queue, &log, &json!({
                    "event": "content_prepared", "name": prepared.name, "kind": prepared.kind,
                    "size": prepared.size, "host": prepared.host,
                }));
                match session.present(Presentation::Content).await {
                    Ok(ctl) => {
                        control = Some(ctl.clone());
                        let (q, l) = (Arc::clone(&queue), Arc::clone(&log));
                        send_job = Some(tokio::spawn(send(ctl, prepared, q, l)));
                    }
                    Err(e) => push(&queue, &log, &json!({
                        "event": "error", "code": e.code(), "message": e.to_string(),
                    })),
                }
            }
            ev = session.next_event() => {
                let Some(ev) = ev else { break };
                let (out, last) = match ev {
                    PresenterEvent::Presenting => (Some(json!({"event": "presenting"})), false),
                    PresenterEvent::PresentingStopped => {
                        if let Some(j) = send_job.take() { j.abort(); }
                        control = None;
                        (Some(json!({"event": "stopped"})), false)
                    }
                    PresenterEvent::ConnectionCode(sas) => {
                        (Some(json!({"event": "connection_code", "sas": sas})), false)
                    }
                    PresenterEvent::Page { page, pages } => {
                        (Some(json!({"event": "page", "page": page, "pages": pages})), false)
                    }
                    PresenterEvent::Ended(reason) => {
                        (Some(json!({"event": "ended", "code": reason.code()})), true)
                    }
                    PresenterEvent::Error { code, message } => {
                        (Some(json!({"event": "error", "code": code, "message": message})), true)
                    }
                    PresenterEvent::State(_) | PresenterEvent::Streaming { .. } => (None, false),
                };
                if let Some(out) = out { push(&queue, &log, &out); }
                if last { break; }
            }
        }
    }
    if let Some(j) = send_job.take() {
        j.abort();
    }
    session.leave().await;
}

// ---------------------------------------------------------------------------
// Content
// ---------------------------------------------------------------------------

enum Payload {
    Image(ImageMime, Vec<u8>),
    Pdf(Vec<u8>),
    Link,
}

struct Prepared {
    name: String,
    kind: &'static str,
    size: u64,
    host: Option<String>,
    url: Option<String>,
    payload: Payload,
}

/// Image type from the first bytes (the extension is not trusted).
fn sniff_image(b: &[u8]) -> Option<ImageMime> {
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

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, &'static str> {
    let meta = std::fs::metadata(path).map_err(|_| "unreadable")?;
    if !meta.is_file() {
        return Err("not_a_regular_file");
    }
    if meta.len() > limit {
        return Err("too_large");
    }
    let file = std::fs::File::open(path).map_err(|_| "unreadable")?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreadable")?;
    if bytes.len() as u64 > limit {
        return Err("too_large");
    }
    if bytes.is_empty() {
        return Err("empty");
    }
    Ok(bytes)
}

fn host_of(url: &str) -> Option<String> {
    // The normalised link is `scheme://host[:port]/...`.
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if host.starts_with('[') {
        host.split(']').next().map(|h| format!("{h}]"))?
    } else {
        host.split(':').next()?.to_owned()
    };
    (!host.is_empty()).then_some(host)
}

/// Read and check an item. Blocking.
fn prepare(item: &Item) -> Result<Prepared, &'static str> {
    match item {
        Item::Link(url) => {
            let url = cc_files::validate_link(url).ok_or("bad_link")?;
            let host = host_of(&url).ok_or("bad_link")?;
            link_message(&url).map_err(|_| "bad_link")?;
            Ok(Prepared {
                name: host.clone(),
                kind: "link",
                size: url.len() as u64,
                host: Some(host),
                url: Some(url),
                payload: Payload::Link,
            })
        }
        Item::File(path) => {
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            if name.to_ascii_lowercase().ends_with(".pdf") {
                let bytes = read_limited(path, MAX_DOCUMENT_BYTES)?;
                document_transfer(&name, &bytes).map_err(|_| "not_pdf")?;
                Ok(Prepared {
                    name,
                    kind: "pdf",
                    size: bytes.len() as u64,
                    host: None,
                    url: None,
                    payload: Payload::Pdf(bytes),
                })
            } else if ImageMime::from_extension(&name).is_some() {
                let bytes = read_limited(path, MAX_IMAGE_BYTES)?;
                let mime = sniff_image(&bytes).ok_or("unsupported_type")?;
                image_transfer(&name, mime, &bytes).map_err(|_| "too_large")?;
                Ok(Prepared {
                    name,
                    kind: "image",
                    size: bytes.len() as u64,
                    host: None,
                    url: None,
                    payload: Payload::Image(mime, bytes),
                })
            } else {
                // Office files and everything else: not on iOS (decision N81).
                Err("unsupported_type")
            }
        }
    }
}

/// Wait for the channel, send, report progress (at most one per percent) and `content_sent`.
async fn send(ctl: ControlSender, prepared: Prepared, queue: Queue, log: Arc<Log>) {
    let fail = |m: String| {
        push(
            &queue,
            &log,
            &json!({"event": "error", "code": "peer_failed", "message": m}),
        );
    };
    if let Err(e) = ctl.wait_ready(READY_TIMEOUT).await {
        push(
            &queue,
            &log,
            &json!({"event": "error", "code": e.code(), "message": e.to_string()}),
        );
        return;
    }
    let total = prepared.size;
    let chan = ctl.channel();
    let mut sent = 0u64;
    let mut last_pct = u64::MAX;
    let items: Box<dyn Iterator<Item = Outgoing<'_>> + Send> = match &prepared.payload {
        Payload::Image(mime, bytes) => match image_transfer(&prepared.name, *mime, bytes) {
            Ok(i) => Box::new(i),
            Err(e) => return fail(e.to_string()),
        },
        Payload::Pdf(bytes) => match document_transfer(&prepared.name, bytes) {
            Ok(i) => Box::new(i),
            Err(e) => return fail(e.to_string()),
        },
        Payload::Link => {
            let url = prepared.url.as_deref().unwrap_or_default();
            let sent_ok = match link_message(url) {
                Ok(m) => chan.send_text(&m).await.map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            if let Err(m) = sent_ok {
                return fail(m);
            }
            return finish(&ctl, &queue, &log, total).await;
        }
    };
    for item in items {
        let r = match item {
            Outgoing::Text(t) => chan.send_text(&t).await.map_err(|e| e.to_string()),
            Outgoing::Binary(b) => {
                let r = chan.send_binary(b).await.map_err(|e| e.to_string());
                sent += b.len() as u64;
                let pct = sent * 100 / total.max(1);
                if pct != last_pct {
                    last_pct = pct;
                    push(
                        &queue,
                        &log,
                        &json!({"event": "progress", "sent": sent, "total": total}),
                    );
                }
                r
            }
        };
        if let Err(m) = r {
            return fail(m);
        }
    }
    finish(&ctl, &queue, &log, total).await;
}

async fn finish(ctl: &ControlSender, queue: &Queue, log: &Log, total: u64) {
    push(
        queue,
        log,
        &json!({"event": "progress", "sent": total, "total": total}),
    );
    if ctl.finish().await {
        push(queue, log, &json!({"event": "content_sent"}));
    } else {
        push(
            queue,
            log,
            &json!({"event": "error", "code": "peer_failed",
                    "message": "the data did not reach the receiver in time"}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(&[0; 32]);
        v
    }

    #[test]
    fn an_image_a_pdf_and_a_link_are_prepared() {
        let d = tempfile::tempdir().unwrap();
        let i = d.path().join("a.png");
        std::fs::write(&i, png()).unwrap();
        let p = prepare(&Item::File(i)).ok().unwrap();
        assert_eq!((p.kind, p.name.as_str()), ("image", "a.png"));

        let f = d.path().join("b.pdf");
        std::fs::write(&f, b"%PDF-1.7\n%%EOF\n").unwrap();
        assert_eq!(prepare(&Item::File(f)).ok().unwrap().kind, "pdf");

        let l = prepare(&Item::Link("https://Example.org/a".into()))
            .ok()
            .unwrap();
        assert_eq!(l.kind, "link");
        assert_eq!(l.host.as_deref(), Some("example.org"));
    }

    #[test]
    fn refusals_have_names() {
        let d = tempfile::tempdir().unwrap();
        let reason = |item| prepare(&item).err().unwrap();
        assert_eq!(
            reason(Item::File(d.path().join("missing.png"))),
            "unreadable"
        );
        assert_eq!(
            reason(Item::File(d.path().to_path_buf())),
            "unsupported_type"
        );
        let empty = d.path().join("e.png");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(reason(Item::File(empty)), "empty");
        let fake = d.path().join("f.png");
        std::fs::write(&fake, b"not a png at all").unwrap();
        assert_eq!(reason(Item::File(fake)), "unsupported_type");
        let notpdf = d.path().join("n.pdf");
        std::fs::write(&notpdf, b"hello").unwrap();
        assert_eq!(reason(Item::File(notpdf)), "not_pdf");
        // Office and everything else: not on iOS (N81).
        let docx = d.path().join("c.docx");
        std::fs::write(&docx, b"PK").unwrap();
        assert_eq!(reason(Item::File(docx)), "unsupported_type");
        assert_eq!(reason(Item::Link("ftp://example.org".into())), "bad_link");
        assert_eq!(
            reason(Item::Link("https://u:p@example.org/".into())),
            "bad_link"
        );
    }

    #[test]
    fn the_size_limits_are_the_spec_ones() {
        assert_eq!(MAX_IMAGE_BYTES, 10 * 1024 * 1024);
        assert_eq!(MAX_DOCUMENT_BYTES, 64 * 1024 * 1024);
        let d = tempfile::tempdir().unwrap();
        let big = d.path().join("big.png");
        let f = std::fs::File::create(&big).unwrap();
        f.set_len(MAX_IMAGE_BYTES + 1).unwrap();
        assert_eq!(prepare(&Item::File(big)).err(), Some("too_large"));
    }

    #[test]
    fn hosts_of_links() {
        assert_eq!(
            host_of("https://example.org:8080/x?y#z").as_deref(),
            Some("example.org")
        );
        assert_eq!(host_of("http://[::1]:80/").as_deref(), Some("[::1]"));
        assert_eq!(host_of("https:///x"), None);
    }

    #[test]
    fn events_queue_in_order_and_are_bounded() {
        let q: Queue = Arc::new(Mutex::new(VecDeque::new()));
        let log = Log::open(None);
        for i in 0..(MAX_QUEUED_EVENTS + 5) {
            push(
                &q,
                &log,
                &json!({"event": "progress", "sent": i, "total": 1}),
            );
        }
        let q = q.lock().unwrap();
        assert_eq!(q.len(), MAX_QUEUED_EVENTS);
        assert!(q.front().unwrap().contains("\"sent\":5"));
    }

    #[test]
    fn nothing_runs_without_start() {
        stop();
        assert!(poll().is_none());
        assert!(!present(Item::Link("https://example.org".into())));
        assert!(!command(Command::Next));
        assert_eq!(
            start(ShareConfig {
                room_code: "bad".into(),
                relay_url: "x:1".into(),
                ..ShareConfig::default()
            }),
            Err(ErrorCode::InvalidRoomCode)
        );
    }
}
