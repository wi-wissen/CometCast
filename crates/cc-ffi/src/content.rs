// SPDX-License-Identifier: GPL-3.0-or-later
//! Content on the receiving side: images, PDF pages and links that the sender pushes over the
//! `control` data channel (docs/web-m2.md, `cc-files`), and the helpers both directions share
//! (where the PDF worker is, link checks).
//!
//! Pictures use the same pull model as the video frames (`video.rs`): the event stream only
//! carries a tiny `ContentFrameReady`, the bytes wait in a mailbox with **one** slot and Dart
//! fetches them with `take_content_frame`. Why: a `StreamSink` queues without limit, and an
//! image of 10 MiB or a page of up to 127 MiB must not pile up when the UI is slow.
//!
//! Bounds and copies (UNCERTAIN: computed from the code, not measured):
//!
//! * `cc-receiver` hands the pictures over as shared `Blob`s; its own event queue is bounded
//!   by count and bytes (docs/transport.md).
//! * This mailbox keeps one `Blob` (no copy). A newer picture replaces it; the older one is
//!   dropped and counted. `take_content_frame` copies once into a `Vec<u8>` for the bridge,
//!   and the bridge copies the bytes once more into the Dart heap (the same as for video
//!   frames). A page therefore exists up to three times for a short moment.
//! * Anything that is not a picture (link, stop, new document) empties the mailbox, so a stale
//!   page can no longer be fetched.
//!
//! Generations: every receiver start/stop bumps a generation. A task of an older receiver can
//! no longer publish into or clear the mailbox of a newer one.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use cc_docs::{Isolation, WORKER_ENV, WORKER_FILE_NAME, WorkerSpec};
use cc_files::{Blob, ContentError, ContentEvent};

use crate::api::{ContentFrame, LinkCheck, ReceiverUiEvent, UiContentError};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means another thread panicked; the data is still usable.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

// ---------------------------------------------------------------------------
// The mailbox

/// What waits in the mailbox. The bytes are shared with the receiver core (no copy here).
#[derive(Debug, Clone)]
pub(crate) enum Stored {
    Image {
        name: String,
        mime: &'static str,
        bytes: Blob,
    },
    Page {
        page: u32,
        width: u32,
        height: u32,
        rgba: Blob,
    },
}

#[derive(Debug, Default)]
struct Inner {
    generation: u64,
    latest: Option<Stored>,
    /// A `ContentFrameReady` was sent and `take` has not been called since.
    notified: bool,
}

/// One slot, newest wins.
pub(crate) struct Mailbox {
    inner: Mutex<Inner>,
    /// Pictures replaced before Dart fetched them.
    replaced: AtomicU64,
}

pub(crate) static CONTENT: Mailbox = Mailbox::new();

impl Mailbox {
    pub(crate) const fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                generation: 0,
                latest: None,
                notified: false,
            }),
            replaced: AtomicU64::new(0),
        }
    }

    /// Start a new generation (receiver start or stop) and empty the slot.
    pub(crate) fn reset(&self) -> u64 {
        let mut g = lock(&self.inner);
        g.generation += 1;
        g.latest = None;
        g.notified = false;
        g.generation
    }

    /// Put a picture into the slot. Returns `true` if the caller must send a
    /// `ContentFrameReady` (none is outstanding). A stale generation publishes nothing.
    pub(crate) fn publish(&self, generation: u64, frame: Stored) -> bool {
        let mut g = lock(&self.inner);
        if g.generation != generation {
            return false;
        }
        if g.latest.replace(frame).is_some() {
            self.replaced.fetch_add(1, Ordering::Relaxed);
        }
        !std::mem::replace(&mut g.notified, true)
    }

    /// Throw away what waits (the content it belonged to is gone).
    pub(crate) fn clear(&self, generation: u64) {
        let mut g = lock(&self.inner);
        if g.generation == generation {
            g.latest = None;
            g.notified = false;
        }
    }

    /// Take the waiting picture; re-arms the `ContentFrameReady` event.
    pub(crate) fn take(&self) -> Option<Stored> {
        let mut g = lock(&self.inner);
        g.notified = false;
        g.latest.take()
    }

    /// Pictures that were replaced before they were fetched.
    #[cfg(test)]
    pub(crate) fn replaced(&self) -> u64 {
        self.replaced.load(Ordering::Relaxed)
    }
}

impl Stored {
    /// The bridge type. Copies the bytes once (see the module documentation).
    pub(crate) fn into_frame(self) -> ContentFrame {
        match self {
            Self::Image { name, mime, bytes } => ContentFrame::Image {
                name,
                mime: mime.to_owned(),
                bytes: bytes.to_vec(),
            },
            Self::Page {
                page,
                width,
                height,
                rgba,
            } => ContentFrame::Page {
                page,
                width,
                height,
                rgba: rgba.to_vec(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Events from cc-receiver to the UI

fn map_error(e: ContentError, worker_found: bool) -> UiContentError {
    match e {
        ContentError::BadMessage => UiContentError::BadMessage,
        ContentError::UnexpectedChunk => UiContentError::UnexpectedChunk,
        ContentError::TooLarge => UiContentError::TooLarge,
        ContentError::SizeMismatch => UiContentError::SizeMismatch,
        ContentError::NoTransfer => UiContentError::NoTransfer,
        ContentError::BadChunk => UiContentError::BadChunk,
        ContentError::Interrupted => UiContentError::Interrupted,
        // Without a worker program every PDF fails the same way; say so instead of blaming
        // the file. UNCERTAIN: a worker that exists but cannot be started (permissions, a
        // broken file) is still reported as a rejected document: `cc-receiver` only gives
        // an English detail text for it, and this layer does not match on text.
        ContentError::DocumentRejected | ContentError::RenderFailed if !worker_found => {
            UiContentError::WorkerUnavailable
        }
        ContentError::DocumentRejected => UiContentError::DocumentRejected,
        ContentError::RenderFailed => UiContentError::RenderFailed,
        ContentError::TooManyErrors => UiContentError::TooManyErrors,
        ContentError::TransferTimeout => UiContentError::TransferTimeout,
    }
}

/// Turn a content event of the core into the small event for Dart and update the mailbox.
/// Text that came from the sender (the `detail` of an error) never reaches Dart. Names and
/// links were cleaned and validated by `cc-files` already.
pub(crate) fn map_content_event(
    generation: u64,
    worker_found: bool,
    ev: ContentEvent,
) -> Option<ReceiverUiEvent> {
    Some(match ev {
        ContentEvent::TransferStarted {
            document,
            name,
            size,
        } => ReceiverUiEvent::ContentTransferStarted {
            document,
            name,
            size,
        },
        ContentEvent::TransferProgress { received, total } => {
            ReceiverUiEvent::ContentTransferProgress { received, total }
        }
        ContentEvent::ImageReceived { name, mime, bytes } => {
            let stored = Stored::Image {
                name,
                mime: mime.as_str(),
                bytes,
            };
            if CONTENT.publish(generation, stored) {
                ReceiverUiEvent::ContentFrameReady
            } else {
                return None;
            }
        }
        ContentEvent::LinkRequested { url } => {
            CONTENT.clear(generation);
            // `cc-files` validated the URL; a missing host would be a bug, so show nothing.
            let host = host_of(&url)?;
            ReceiverUiEvent::ContentLink { url, host }
        }
        ContentEvent::DocumentOpened { name, pages } => {
            CONTENT.clear(generation);
            ReceiverUiEvent::ContentDocumentOpened { name, pages }
        }
        ContentEvent::PageRendered {
            page,
            width,
            height,
            rgba,
        } => {
            let stored = Stored::Page {
                page,
                width,
                height,
                rgba,
            };
            if CONTENT.publish(generation, stored) {
                ReceiverUiEvent::ContentFrameReady
            } else {
                return None;
            }
        }
        ContentEvent::ContentStopped => {
            CONTENT.clear(generation);
            ReceiverUiEvent::ContentStopped
        }
        ContentEvent::Error { error, detail: _ } => ReceiverUiEvent::ContentError {
            error: map_error(error, worker_found),
        },
    })
}

// ---------------------------------------------------------------------------
// The width pages are rendered at

/// Last width the UI reported (physical pixels), used when a receiver starts.
static LAST_WIDTH: AtomicU32 = AtomicU32::new(0);

/// The running receiver's channel for width changes.
static WIDTH_TX: Mutex<Option<(u64, tokio::sync::mpsc::UnboundedSender<u32>)>> = Mutex::new(None);

/// Remember the width and pass it to the running receiver, if any.
pub(crate) fn set_target_width(width: u32) {
    LAST_WIDTH.store(width, Ordering::Relaxed);
    if let Some((_, tx)) = lock(&WIDTH_TX).as_ref() {
        let _ = tx.send(width);
    }
}

/// The width to start a receiver with, or `None` for the library default.
pub(crate) fn initial_width() -> Option<u32> {
    match LAST_WIDTH.load(Ordering::Relaxed) {
        0 => None,
        w => Some(w),
    }
}

/// Install the width channel of the receiver started with `generation`. The last width is
/// sent at once: it may have changed while the receiver was connecting.
pub(crate) fn set_width_channel(generation: u64, tx: tokio::sync::mpsc::UnboundedSender<u32>) {
    if let Some(w) = initial_width() {
        let _ = tx.send(w);
    }
    *lock(&WIDTH_TX) = Some((generation, tx));
}

/// Remove the width channel if it still belongs to `generation` (a newer receiver may have
/// installed its own).
pub(crate) fn clear_width_channel(generation: u64) {
    let mut g = lock(&WIDTH_TX);
    if g.as_ref().is_some_and(|(g_, _)| *g_ == generation) {
        *g = None;
    }
}

// ---------------------------------------------------------------------------
// Where the PDF worker is

static WORKER_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// How PDFs are rendered or inspected, and whether a worker program was found.
#[derive(Debug, Clone)]
pub(crate) struct Worker {
    pub(crate) isolation: Isolation,
    /// A worker program exists. When `false` every PDF fails with "worker unavailable"; there
    /// is **no** silent fallback to the unprotected in-process engine.
    pub(crate) found: bool,
}

/// Set (or with `None` remove) an explicit worker program. Used by tests and by hosts whose
/// layout differs from "next to the executable".
pub(crate) fn set_worker_override(path: Option<PathBuf>) {
    *lock(&WORKER_OVERRIDE) = path;
}

/// Decide where the worker is. Order:
///
/// 1. the explicit path from [`set_worker_override`];
/// 2. the environment variable `COMETCAST_DOCS_WORKER`: left to `cc-docs`, which refuses a
///    file that others could have replaced (docs/documents.md);
/// 3. `cc-docs-worker` in the directory of the running executable, as an **explicit**
///    `WorkerSpec` (this is where `flutter build linux` puts it, see docs/flutter-bridge.md);
/// 4. `cc-docs`' own discovery (also finds the Cargo test layout).
pub(crate) fn worker() -> Worker {
    if let Some(p) = lock(&WORKER_OVERRIDE).clone() {
        return override_worker(p);
    }
    if std::env::var_os(WORKER_ENV).is_some_and(|v| !v.is_empty()) {
        return auto_worker();
    }
    if let Some(p) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join(WORKER_FILE_NAME)))
        .filter(|p| p.is_file())
    {
        return Worker {
            isolation: Isolation::Process(WorkerSpec::new(p)),
            found: true,
        };
    }
    auto_worker()
}

/// A worker given by path (by the app): the same trust check as the Office worker and the
/// environment variable (absolute, owned by this user or root, not writable by others, in a
/// directory others cannot write). A path that fails it counts as "no worker".
fn override_worker(p: PathBuf) -> Worker {
    let found = cc_docs::check_worker_file(&p).is_ok();
    with_found(Isolation::Process(WorkerSpec::new(p)), found)
}

/// `cc-docs`' own discovery, checked (an untrusted `COMETCAST_DOCS_WORKER` counts as not found).
fn auto_worker() -> Worker {
    with_found(Isolation::Auto, WorkerSpec::discover_checked().is_ok())
}

/// A program that does not exist: with no worker, a PDF must fail, never reach the in-process
/// engine (`cc-docs` maps `Auto` and `InProcess` to in-process on iOS, tvOS and wasm).
const NO_WORKER_PROGRAM: &str = "/nonexistent/cc-docs-worker-not-found";

/// Build a [`Worker`]; without a worker the isolation is replaced by one that cannot start, so
/// no caller can render a PDF unsandboxed by accident.
fn with_found(isolation: Isolation, found: bool) -> Worker {
    Worker {
        isolation: if found {
            isolation
        } else {
            Isolation::Process(WorkerSpec::new(PathBuf::from(NO_WORKER_PROGRAM)))
        },
        found,
    }
}

/// The worker program that would be used now, for display and diagnostics.
pub(crate) fn worker_path() -> Option<String> {
    let w = worker();
    if !w.found {
        return None;
    }
    let spec = match w.isolation {
        Isolation::Process(spec) => Some(spec),
        _ => WorkerSpec::discover(),
    };
    spec.map(|s| s.program.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Links

/// The host of a validated URL as the WHATWG parser writes it: an internationalised name
/// appears in its punycode (`xn--`) form, so a look-alike name is visible as such.
pub(crate) fn host_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    u.host_str().filter(|h| !h.is_empty()).map(str::to_owned)
}

/// The rules of `cc-files` (never duplicated in Dart), plus the host for display.
pub(crate) fn check_link(input: &str) -> LinkCheck {
    match cc_files::validate_link(input).and_then(|u| host_of(&u).map(|h| (u, h))) {
        Some((url, host)) => LinkCheck::Valid { url, host },
        None => LinkCheck::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_worker_path_from_the_app_gets_the_same_trust_check() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let w = d.path().join("w");
        std::fs::write(&w, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&w, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(override_worker(w.clone()).found);
        // Missing, relative, group/other writable, in a world-writable directory: refused.
        assert!(!override_worker(d.path().join("missing")).found);
        assert!(!override_worker(PathBuf::from("w")).found);
        std::fs::set_permissions(&w, std::fs::Permissions::from_mode(0o757)).unwrap();
        assert!(!override_worker(w.clone()).found);
        std::fs::set_permissions(&w, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(!override_worker(w.clone()).found);
        std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        // A refused worker cannot start: no silent in-process fallback.
        match override_worker(d.path().join("missing")).isolation {
            Isolation::Process(spec) => {
                assert_eq!(spec.program, std::path::Path::new(NO_WORKER_PROGRAM))
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    fn img(n: usize) -> Stored {
        Stored::Image {
            name: "a.png".into(),
            mime: "image/png",
            bytes: Blob::new(vec![7; n]),
        }
    }

    #[test]
    fn one_ready_event_until_the_picture_is_taken_and_newest_wins() {
        let mb = Mailbox::new();
        let g = mb.reset();
        assert!(mb.publish(g, img(10)), "first picture: send the event");
        assert!(
            !mb.publish(g, img(20)),
            "second one: the event is outstanding"
        );
        assert_eq!(mb.replaced(), 1);
        match mb.take() {
            Some(Stored::Image { bytes, .. }) => assert_eq!(bytes.len(), 20),
            other => panic!("unexpected {other:?}"),
        }
        assert!(mb.take().is_none());
        assert!(mb.publish(g, img(5)), "taking re-arms the event");
    }

    #[test]
    fn a_stale_generation_cannot_publish_or_clear() {
        let mb = Mailbox::new();
        let old = mb.reset();
        let new = mb.reset();
        assert!(!mb.publish(old, img(1)));
        assert!(mb.take().is_none());
        assert!(mb.publish(new, img(2)));
        mb.clear(old);
        assert!(mb.take().is_some(), "an old generation does not clear");
        assert!(mb.publish(new, img(2)));
        mb.clear(new);
        assert!(mb.take().is_none());
    }

    #[test]
    fn a_reset_drops_the_waiting_picture() {
        let mb = Mailbox::new();
        let g = mb.reset();
        assert!(mb.publish(g, img(3)));
        mb.reset();
        assert!(mb.take().is_none());
    }

    #[test]
    fn links_are_checked_by_cc_files_and_the_host_is_punycode() {
        match check_link("https://bücher.example/a?b=1") {
            LinkCheck::Valid { url, host } => {
                assert_eq!(host, "xn--bcher-kva.example");
                assert!(url.starts_with("https://xn--bcher-kva.example/"));
            }
            LinkCheck::Invalid => panic!("must be valid"),
        }
        for bad in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "ftp://x.example/",
            "https://user:pw@x.example/",
            "https://x.example/a b",
            "",
            "https://",
        ] {
            assert!(matches!(check_link(bad), LinkCheck::Invalid), "{bad}");
        }
    }

    #[test]
    fn no_worker_never_means_in_process() {
        for iso in [Isolation::Auto, Isolation::InProcess] {
            let w = with_found(iso, false);
            assert!(!w.found);
            assert!(matches!(w.isolation, Isolation::Process(_)));
        }
        assert!(matches!(
            with_found(Isolation::Auto, true).isolation,
            Isolation::Auto
        ));
    }

    #[test]
    fn errors_are_codes_and_a_missing_worker_is_named() {
        assert_eq!(
            map_error(ContentError::DocumentRejected, true),
            UiContentError::DocumentRejected
        );
        assert_eq!(
            map_error(ContentError::DocumentRejected, false),
            UiContentError::WorkerUnavailable
        );
        assert_eq!(
            map_error(ContentError::TooLarge, false),
            UiContentError::TooLarge
        );
    }

    #[test]
    fn the_event_for_a_link_clears_the_mailbox_and_hides_error_text() {
        let g = CONTENT.reset();
        let page = ContentEvent::PageRendered {
            page: 1,
            width: 1,
            height: 1,
            rgba: Blob::new(vec![0; 4]),
        };
        assert!(matches!(
            map_content_event(g, true, page),
            Some(ReceiverUiEvent::ContentFrameReady)
        ));
        let link = ContentEvent::LinkRequested {
            url: "https://example.org/".into(),
        };
        assert!(matches!(
            map_content_event(g, true, link),
            Some(ReceiverUiEvent::ContentLink { .. })
        ));
        assert!(CONTENT.take().is_none());
        let err = ContentEvent::Error {
            error: ContentError::BadChunk,
            detail: "secret text of the sender".into(),
        };
        let mapped = map_content_event(g, true, err).expect("event");
        assert!(!format!("{mapped:?}").contains("secret"));
        CONTENT.reset();
    }
}
