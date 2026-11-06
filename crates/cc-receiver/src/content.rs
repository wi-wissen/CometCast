// SPDX-License-Identifier: GPL-3.0-or-later
//! The task that connects the `control` data channel to `cc-files`.
//!
//! It reads messages from the channel one by one and feeds a [`ContentReceiver`]. Chunks are
//! handled inline (they only copy bytes); text messages go to the blocking thread pool,
//! because opening a PDF or rendering a page waits for the render process of `cc-docs`.
//! Events leave through a bounded queue, and the channel is not read while that queue is
//! full, so a slow application slows the sender down instead of using more memory.
//! When the receiver blocks itself (too many protocol violations) the channel is closed.

use cc_files::{ContentError, ContentEvent, ContentOptions, ContentReceiver};
use cc_transport::{ControlChannel, ControlData};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Events that may wait for the application. Page pictures can be several MB each.
/// UNCERTAIN: placeholder, unmeasured.
pub(crate) const EVENT_QUEUE: usize = 8;

/// Start the task. It ends when the channel closed and everything was handled, when the
/// application dropped the event receiver, or when the receiver blocked itself.
pub(crate) fn spawn(
    channel: ControlChannel,
    opts: ContentOptions,
    mut widths: mpsc::UnboundedReceiver<u32>,
    events: mpsc::Sender<ContentEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = Some(ContentReceiver::new(opts.clone()));
        loop {
            let evs = tokio::select! {
                data = channel.recv() => match data {
                    None => break,
                    Some(ControlData::Binary(b)) => match rx.as_mut() {
                        Some(r) => r.handle_binary(&b),
                        None => Vec::new(),
                    },
                    Some(ControlData::Text(t)) => {
                        blocking(&mut rx, &opts, move |r| r.handle_text(&t)).await
                    }
                },
                Some(w) = widths.recv() => {
                    blocking(&mut rx, &opts, move |r| r.set_target_width(w)).await
                }
            };
            for e in evs {
                if events.send(e).await.is_err() {
                    channel.close().await;
                    return;
                }
            }
            if rx.as_ref().is_some_and(ContentReceiver::is_blocked) {
                channel.close().await;
                break;
            }
        }
    })
}

/// Run `f` on the blocking pool with the receiver moved in and out. If `f` panics (it
/// should not), the receiver is rebuilt from the options and an error event is reported.
async fn blocking<F>(
    rx: &mut Option<ContentReceiver>,
    opts: &ContentOptions,
    f: F,
) -> Vec<ContentEvent>
where
    F: FnOnce(&mut ContentReceiver) -> Vec<ContentEvent> + Send + 'static,
{
    let Some(mut r) = rx.take() else {
        return Vec::new();
    };
    match tokio::task::spawn_blocking(move || {
        let evs = f(&mut r);
        (r, evs)
    })
    .await
    {
        Ok((r, evs)) => {
            *rx = Some(r);
            evs
        }
        Err(_) => {
            *rx = Some(ContentReceiver::new(opts.clone()));
            vec![ContentEvent::Error {
                error: ContentError::RenderFailed,
                detail: "internal error while handling content; state was reset".into(),
            }]
        }
    }
}
