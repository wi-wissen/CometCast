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
//! Events leave through a queue that is bounded by **count and by bytes**
//! ([`EVENT_QUEUE`], [`EVENT_QUEUE_BYTES`]), and the channel is not read while it is full, so
//! a slow application slows the sender down instead of using more memory. A rendered page
//! that is still waiting when a newer one arrives is dropped from the queue: the application
//! only ever needs the latest picture. When the receiver blocks itself (too many protocol
//! violations) the channel is closed.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use cc_files::{ContentEvent, ContentOptions, ContentReceiver, Next, WorkDone};
use cc_transport::{ControlChannel, ControlData};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

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
        let superseding = matches!(event, ContentEvent::PageRendered { .. });
        loop {
            {
                let mut st = self.0.lock();
                if st.consumer_gone {
                    return Err(());
                }
                if superseding {
                    let mut freed = 0;
                    st.items.retain(|(e, w)| {
                        let stale = matches!(e, ContentEvent::PageRendered { .. });
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

/// Start the task. It ends when the channel closed and everything was handled, when the
/// application dropped the event receiver, or when the receiver blocked itself.
pub(crate) fn spawn(
    channel: ControlChannel,
    opts: ContentOptions,
    mut widths: mpsc::UnboundedReceiver<u32>,
    events: EventSink,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = ContentReceiver::new(opts);
        let mut running: Option<(u64, JoinHandle<WorkDone>)> = None;
        let mut closed = false;
        loop {
            // Start the next job, if one is due.
            let mut wake = None;
            if running.is_none() {
                match rx.next_action(Instant::now(), true) {
                    Next::Work(w) => {
                        let id = w.id();
                        running = Some((id, tokio::task::spawn_blocking(move || w.run())));
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
                        Some((_, h)) => h.await,
                        None => std::future::pending().await,
                    }
                } => {
                    let id = running.take().map_or(0, |(id, _)| id);
                    evs = match res {
                        Ok(done) => rx.finish_work(done),
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
                if events.push(e).await.is_err() {
                    channel.close().await;
                    return;
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
    async fn the_writer_learns_that_the_reader_is_gone() {
        let (tx, rx) = event_queue();
        drop(rx);
        assert!(tx.push(ContentEvent::ContentStopped).await.is_err());
    }
}
