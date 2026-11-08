// SPDX-License-Identifier: GPL-3.0-or-later
//! The task that connects the `control` data channel to `cc-files`.
//!
//! It reads messages from the channel and feeds a [`ContentReceiver`]. Feeding never blocks
//! (chunks only copy bytes, a page command only changes the wanted page), so the task keeps
//! reading while a PDF is opened or a page is rendered: those run as [`Work`] on the
//! blocking thread pool, one job at a time, and `stop` cancels a running render. Messages
//! that piled up are read in one go before the next job starts, so a flood of page commands
//! costs one render (and renders keep a minimum interval, see `cc-files`).
//!
//! A presenter can end a presentation and start a new one at any time (a new offer, leave and
//! join again). So the limits are kept **per room**, not per presentation ([`RoomContent`]):
//! the violation count and the document-open budget carry over to the next presentation, and
//! one job runs at a time in the whole room. A task that is aborted while its job runs sets
//! the job's cancel flag (the render process is killed), and the job keeps the room's job
//! place until it has really ended, so the next presentation's first job waits for it.
//!
//! Events leave through a queue that is bounded by **count and by bytes**
//! ([`EVENT_QUEUE`], [`EVENT_QUEUE_BYTES`]), and the channel is not read while it is full, so
//! a slow application slows the sender down instead of using more memory. A rendered page
//! that is still waiting when a newer one arrives is dropped from the queue: the application
//! only ever needs the latest picture. When the receiver blocks itself (too many protocol
//! violations) the channel is closed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use cc_files::{
    ContentError, ContentEvent, ContentOptions, ContentReceiver, Next, SharedBudget, WorkDone,
    handover_ok_message, page_message,
};
use cc_transport::{ControlChannel, ControlData};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc};
use tokio::task::JoinHandle;

/// What the content tasks of one room share (one task per presentation, one after the other;
/// an aborted task's job may still be running when the next task starts).
#[derive(Clone)]
pub(crate) struct RoomContent {
    /// Violations and document-open budget.
    budget: SharedBudget,
    /// One place: the document job (open or render) that may run in this room.
    jobs: Arc<Semaphore>,
}

impl RoomContent {
    pub(crate) fn new() -> Self {
        Self {
            budget: SharedBudget::new(),
            jobs: Arc::new(Semaphore::new(1)),
        }
    }

    /// Wait for the room's job place (held by a job of an earlier presentation that has not
    /// ended yet).
    async fn job_place(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.jobs).acquire_owned().await.ok()
    }
}

/// Sets a job's cancel flag when dropped: the content task was aborted (or ended) while the
/// job ran, so nobody waits for its result any more.
struct CancelOnDrop(Option<Arc<AtomicBool>>);

impl CancelOnDrop {
    /// The job ended on its own: nothing to cancel.
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(c) = &self.0 {
            c.store(true, Ordering::SeqCst);
        }
    }
}

/// A job on the blocking pool: its id, its handle (which gives the room's job place back
/// with the result) and its cancel flag.
type Running = (
    u64,
    JoinHandle<(WorkDone, OwnedSemaphorePermit)>,
    CancelOnDrop,
);

/// Events that may wait for the application (count).
/// UNCERTAIN: placeholder, unmeasured.
pub(crate) const EVENT_QUEUE: usize = 16;

/// Events that may wait for the application (bytes of pictures; an event of its own size
/// is always admitted into an empty queue, so one page of up to the output limit of
/// `cc-docs`, about 127 MiB by default, can wait on top of nothing).
/// UNCERTAIN: placeholder, unmeasured.
pub(crate) const EVENT_QUEUE_BYTES: usize = 32 * 1024 * 1024;

/// Bookkeeping size of an event without pictures.
const EVENT_OVERHEAD: usize = 256;

/// Messages read in one go before the next job is started.
const READ_BATCH: usize = 512;

fn weight(e: &ContentEvent) -> usize {
    EVENT_OVERHEAD
        + match e {
            ContentEvent::ImageReceived { bytes, .. } => bytes.len(),
            ContentEvent::PageRendered { rgba, .. } => rgba.len(),
            _ => 0,
        }
}

struct State {
    items: VecDeque<(ContentEvent, usize)>,
    bytes: usize,
    producer_gone: bool,
    consumer_gone: bool,
}

struct Shared {
    state: Mutex<State>,
    readable: Notify,
    writable: Notify,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The writing end, owned by the content task.
pub(crate) struct EventSink(Arc<Shared>);

/// The reading end, behind [`ContentEvents`](crate::ContentEvents).
pub(crate) struct EventStream(Arc<Shared>);

/// A queue of events, bounded by count and bytes.
pub(crate) fn event_queue() -> (EventSink, EventStream) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            items: VecDeque::new(),
            bytes: 0,
            producer_gone: false,
            consumer_gone: false,
        }),
        readable: Notify::new(),
        writable: Notify::new(),
    });
    (EventSink(Arc::clone(&shared)), EventStream(shared))
}

impl EventSink {
    /// Queue an event, waiting while the queue is full. `Err` when the reader is gone.
    /// A queued `PageRendered` is replaced by a newer one.
    pub(crate) async fn push(&self, event: ContentEvent) -> Result<(), ()> {
        let w = weight(&event);
        let superseding = matches!(
            event,
            ContentEvent::PageRendered { .. } | ContentEvent::TransferProgress { .. }
        );
        let progress = matches!(event, ContentEvent::TransferProgress { .. });
        loop {
            {
                let mut st = self.0.lock();
                if st.consumer_gone {
                    return Err(());
                }
                if superseding {
                    let mut freed = 0;
                    st.items.retain(|(e, w)| {
                        // A newer picture replaces an older one, a newer progress an older
                        // progress.
                        let stale = if progress {
                            matches!(e, ContentEvent::TransferProgress { .. })
                        } else {
                            matches!(e, ContentEvent::PageRendered { .. })
                        };
                        if stale {
                            freed += w;
                        }
                        !stale
                    });
                    st.bytes -= freed;
                }
                let fits = st.items.is_empty()
                    || (st.items.len() < EVENT_QUEUE && st.bytes + w <= EVENT_QUEUE_BYTES);
                if fits {
                    st.bytes += w;
                    st.items.push_back((event, w));
                    drop(st);
                    self.0.readable.notify_one();
                    return Ok(());
                }
            }
            self.0.writable.notified().await;
        }
    }
}

impl Drop for EventSink {
    fn drop(&mut self) {
        self.0.lock().producer_gone = true;
        self.0.readable.notify_one();
    }
}

impl EventStream {
    /// Next event, or `None` once the task has ended and everything was read.
    pub(crate) async fn next(&mut self) -> Option<ContentEvent> {
        loop {
            {
                let mut st = self.0.lock();
                if let Some((e, w)) = st.items.pop_front() {
                    st.bytes -= w;
                    drop(st);
                    self.0.writable.notify_one();
                    return Some(e);
                }
                if st.producer_gone {
                    return None;
                }
            }
            self.0.readable.notified().await;
        }
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        self.0.lock().consumer_gone = true;
        self.0.writable.notify_one();
    }
}

fn feed(rx: &mut ContentReceiver, data: ControlData, evs: &mut Vec<ContentEvent>) {
    match data {
        ControlData::Binary(b) => evs.extend(rx.feed_binary(&b)),
        ControlData::Text(t) => evs.extend(rx.feed_text(&t)),
    }
}

/// What the content task needs to answer a presenter's hand-over request (native-apps.md 20.5).
pub(crate) struct HandoverHook {
    /// The display's tokens.
    pub(crate) store: Arc<crate::HandoverStore>,
    /// Name of the presenter whose channel this is (as the display knows it).
    pub(crate) name: String,
    /// Where "a token was stored" is reported to the receiver loop.
    pub(crate) notify: mpsc::UnboundedSender<crate::ReceiverEvent>,
}

/// Start the task. It ends when the channel closed and everything was handled, when the
/// application dropped the event receiver, or when the receiver blocked itself.
pub(crate) fn spawn(
    channel: ControlChannel,
    opts: ContentOptions,
    mut widths: mpsc::UnboundedReceiver<u32>,
    events: Arc<EventSink>,
    room: &RoomContent,
    hook: Option<HandoverHook>,
) -> JoinHandle<()> {
    let room = room.clone();
    tokio::spawn(async move {
        let mut rx = ContentReceiver::with_budget(opts, room.budget.clone());
        if rx.is_blocked() {
            // Too many violations in this room, not yet forgotten: no new presentation.
            let _ = events
                .push(ContentEvent::Error {
                    error: ContentError::TooManyErrors,
                    detail: "too many protocol violations in this room; try again later".into(),
                })
                .await;
            channel.close().await;
            return;
        }
        // The room's job place: a job of the previous presentation may still be ending.
        // While the task holds it no other job runs; a job takes it along and gives it back
        // with its result (or, if this task is aborted meanwhile, when it really ended).
        let Some(place) = room.job_place().await else {
            return;
        };
        let mut place = Some(place);
        // What the presenter was last told (decision 58): the page and the page count.
        let mut told: Option<(u32, u32)> = None;
        let mut running: Option<Running> = None;
        let mut closed = false;
        loop {
            // Start the next job, if one is due.
            let mut wake = None;
            if running.is_none() {
                if place.is_none() {
                    // The last job's thread panicked and dropped the place: take it again.
                    match room.job_place().await {
                        Some(p) => place = Some(p),
                        None => return,
                    }
                }
                match rx.next_action(Instant::now(), true) {
                    Next::Work(w) => {
                        let id = w.id();
                        let cancel = CancelOnDrop(Some(w.cancel_flag()));
                        let p = place.take().expect("the job place is held here");
                        let h = tokio::task::spawn_blocking(move || (w.run(), p));
                        running = Some((id, h, cancel));
                    }
                    Next::Wait(t) => wake = Some(t),
                    Next::Idle => {}
                }
            }
            if closed && running.is_none() && wake.is_none() {
                break; // the channel is closed and all its content was handled
            }
            let deadline = [wake, rx.idle_deadline()].into_iter().flatten().min();

            let mut evs = Vec::new();
            tokio::select! {
                biased;
                res = async {
                    match running.as_mut() {
                        Some((_, h, _)) => h.await,
                        None => std::future::pending().await,
                    }
                } => {
                    let id = running.take().map_or(0, |(id, _, mut cancel)| {
                        cancel.disarm();
                        id
                    });
                    evs = match res {
                        Ok((done, p)) => {
                            place = Some(p);
                            rx.finish_work(done)
                        }
                        // The thread panicked (it should not). The receiver keeps its state.
                        Err(_) => rx.work_panicked(id),
                    };
                }
                () = async {
                    match deadline {
                        Some(d) => tokio::time::sleep_until(tokio::time::Instant::from_std(d)).await,
                        None => std::future::pending().await,
                    }
                } => {
                    evs = rx.tick(Instant::now());
                }
                Some(w) = widths.recv() => rx.feed_target_width(w),
                data = async {
                    if closed {
                        std::future::pending().await
                    } else {
                        channel.recv().await
                    }
                } => match data {
                    None => closed = true,
                    Some(d) => {
                        feed(&mut rx, d, &mut evs);
                        // Take what piled up in the meantime before a job is started.
                        for _ in 0..READ_BATCH {
                            if rx.is_blocked() {
                                break;
                            }
                            match channel.try_recv().await {
                                Some(d) => feed(&mut rx, d, &mut evs),
                                None => break,
                            }
                        }
                    }
                },
            }
            for e in evs {
                // A hand-over request is for the display itself, not for the application:
                // hold the token and answer. Without a known presenter it is not answered.
                if let ContentEvent::HandoverRequested { token } = &e {
                    if let Some(h) = &hook
                        && h.store.insert(token, &h.name, Instant::now())
                    {
                        let _ = channel.send_text(&handover_ok_message()).await;
                        let _ = h.notify.send(crate::ReceiverEvent::HandoverStored {
                            name: h.name.clone(),
                        });
                    }
                    continue;
                }
                if events.push(e).await.is_err() {
                    channel.close().await;
                    return;
                }
            }
            // Tell the presenter the page and the page count once the count is known and on
            // every page change; the next document starts afresh.
            let now_page = rx.document_state();
            if now_page != told {
                told = now_page;
                if let Some((page, pages)) = now_page {
                    // A failed send only means that the channel is closing.
                    let _ = channel.send_text(&page_message(page, pages)).await;
                }
            }
            if rx.is_blocked() {
                channel.close().await;
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_files::Blob;

    fn page(n: u32, bytes: usize) -> ContentEvent {
        ContentEvent::PageRendered {
            page: n,
            width: 1,
            height: 1,
            rgba: Blob::new(vec![0; bytes]),
        }
    }

    fn image(bytes: usize) -> ContentEvent {
        ContentEvent::ImageReceived {
            name: "i".into(),
            mime: cc_files::ImageMime::Png,
            bytes: Blob::new(vec![0; bytes]),
        }
    }

    #[tokio::test]
    async fn a_waiting_page_is_replaced_by_a_newer_one() {
        let (tx, mut rx) = event_queue();
        for n in 1..=100 {
            tx.push(page(n, 1024 * 1024)).await.unwrap();
        }
        // Only the newest picture is still queued: 1 MiB, not 100 MiB.
        assert_eq!(tx.0.lock().items.len(), 1);
        assert!(tx.0.lock().bytes < 2 * 1024 * 1024);
        assert!(matches!(
            rx.next().await,
            Some(ContentEvent::PageRendered { page: 100, .. })
        ));
        drop(tx);
        assert!(rx.next().await.is_none());
    }

    #[tokio::test]
    async fn the_queue_is_bounded_by_bytes_and_blocks_the_writer() {
        let (tx, mut rx) = event_queue();
        // 4 images of 8 MiB fill the 32 MiB budget; the fifth has to wait.
        for _ in 0..3 {
            tx.push(image(8 * 1024 * 1024)).await.unwrap();
        }
        let tx = Arc::new(tx);
        let t2 = Arc::clone(&tx);
        let blocked = tokio::spawn(async move {
            t2.push(image(8 * 1024 * 1024)).await.unwrap();
            t2.push(image(8 * 1024 * 1024)).await.unwrap();
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(!blocked.is_finished(), "the writer waits for the reader");
        assert!(tx.0.lock().bytes <= EVENT_QUEUE_BYTES);
        for _ in 0..5 {
            assert!(matches!(
                rx.next().await,
                Some(ContentEvent::ImageReceived { .. })
            ));
        }
        blocked.await.unwrap();
    }

    #[tokio::test]
    async fn one_event_larger_than_the_budget_is_admitted_alone() {
        let (tx, mut rx) = event_queue();
        tx.push(page(1, EVENT_QUEUE_BYTES * 2)).await.unwrap();
        let tx = Arc::new(tx);
        let t2 = Arc::clone(&tx);
        let second = tokio::spawn(async move { t2.push(ContentEvent::ContentStopped).await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!second.is_finished());
        assert!(matches!(
            rx.next().await,
            Some(ContentEvent::PageRendered { .. })
        ));
        second.await.unwrap().unwrap();
        assert!(matches!(
            rx.next().await,
            Some(ContentEvent::ContentStopped)
        ));
    }

    #[tokio::test]
    async fn an_aborted_task_cancels_its_job_and_the_job_keeps_the_room_place() {
        let room = RoomContent::new();
        let place = room.job_place().await.unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let cancel = CancelOnDrop(Some(Arc::clone(&flag)));
        // A task as the content task: its job holds the room's place on the blocking pool.
        let task = tokio::spawn(async move {
            let _cancel = cancel;
            let job = tokio::task::spawn_blocking(move || {
                let _place = place;
                let _ = wait.recv();
            });
            let _ = job.await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        task.abort();
        let _ = task.await;
        assert!(flag.load(Ordering::SeqCst), "the job was told to stop");
        assert_eq!(
            room.jobs.available_permits(),
            0,
            "the job still runs: the next presentation must wait for it"
        );
        release.send(()).unwrap();
        let _place = tokio::time::timeout(std::time::Duration::from_secs(5), room.job_place())
            .await
            .expect("the place is free once the job ended")
            .unwrap();
    }

    #[tokio::test]
    async fn the_writer_learns_that_the_reader_is_gone() {
        let (tx, rx) = event_queue();
        drop(rx);
        assert!(tx.push(ContentEvent::ContentStopped).await.is_err());
    }
}
