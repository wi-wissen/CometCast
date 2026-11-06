// SPDX-License-Identifier: GPL-3.0-or-later
//! The one external H.264 input of the running encoded sender (see `api::start_encoded_sender`).
//! A process-wide slot, like `VIDEO` in `video.rs`: the Dart side calls free functions.

use std::sync::Mutex;

use cc_sender::ExternalInput;

static CURRENT: Mutex<Option<ExternalInput>> = Mutex::new(None);

fn slot() -> std::sync::MutexGuard<'static, Option<ExternalInput>> {
    // A poisoned lock only means another thread panicked; the data is still usable.
    CURRENT.lock().unwrap_or_else(|p| p.into_inner())
}

/// The input that pushes go to, if an encoded sender is running.
pub(crate) fn current() -> Option<ExternalInput> {
    slot().clone()
}

/// Make `next` the current input and close the previous one. `None` = no encoded sender.
pub(crate) fn replace(next: Option<ExternalInput>) {
    let old = std::mem::replace(&mut *slot(), next);
    if let Some(old) = old {
        old.close();
    }
}

/// Close and clear the slot if it still holds `input` (a newer sender may have replaced it).
pub(crate) fn clear_if(input: Option<&ExternalInput>) {
    let Some(input) = input else { return };
    input.close();
    let mut guard = slot();
    if guard.as_ref().is_some_and(|cur| cur.same_as(input)) {
        *guard = None;
    }
}
