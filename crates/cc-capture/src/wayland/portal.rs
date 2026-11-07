// SPDX-License-Identifier: GPL-3.0-or-later
//! The ScreenCast portal through `ashpd` (MIT). All calls are async; the backend owns a small
//! tokio runtime and blocks on it, because `ScreenSource` is a blocking trait.

use std::os::fd::OwnedFd;
use std::sync::Arc;

use ashpd::desktop::PersistMode;
use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, StartCastOptions,
};
use ashpd::desktop::{CreateSessionOptions, Session};

use futures_util::StreamExt;

use super::Shared;
use super::negotiate::{self, StreamInfo};
use crate::CaptureError;
use crate::session::Event;

/// What the portal offers.
#[derive(Debug, Clone, Copy)]
pub(super) struct Probe {
    pub(super) cursor_modes: u32,
    pub(super) source_types: u32,
}

/// Options of one picker run.
#[derive(Debug, Clone, Default)]
pub(super) struct PickOptions {
    /// A stored restore token to hand to the portal (skips its picker if it accepts it).
    pub(super) restore_token: Option<String>,
    /// Ask the portal for a new token to store.
    pub(super) persist: bool,
}

/// A running portal session.
pub(super) struct Portal {
    pub(super) session: Arc<Session<Screencast>>,
    pub(super) streams: Vec<StreamInfo>,
    pub(super) fd: OwnedFd,
    pub(super) restore_token: Option<String>,
}

/// Map an ashpd error to the error of this crate. A cancelled or otherwise refused request is
/// [`CaptureError::Denied`]; a missing portal service is `Unsupported`.
pub(super) fn map_error(e: &ashpd::Error) -> CaptureError {
    use ashpd::Error as E;
    match e {
        // UNCERTAIN: `ResponseError::Other` is treated like a cancel (no error page), because
        // the designer's text says refusal is `capture_denied`; backends differ in what they
        // answer when the person closes their dialog.
        E::Response(_) => CaptureError::Denied,
        E::PortalNotFound(_) => {
            CaptureError::Unsupported("no desktop portal with the ScreenCast interface")
        }
        E::RequiresVersion(..) => {
            CaptureError::Unsupported("the desktop portal ScreenCast interface is too old")
        }
        other => CaptureError::Display(format!("desktop portal: {other}")),
    }
}

/// Connect to the portal and ask what it offers. Fails with `Unsupported` when there is no
/// session bus or no ScreenCast portal.
pub(super) async fn connect() -> Result<(Screencast, Probe), CaptureError> {
    let sc = Screencast::new().await.map_err(|e| match map_error(&e) {
        CaptureError::Display(_) => {
            CaptureError::Unsupported("no desktop portal on the session bus")
        }
        other => other,
    })?;
    // Properties are optional in old interface versions; a failure means "unknown" (0).
    let cursor_modes = sc
        .available_cursor_modes()
        .await
        .map(|m| m.bits())
        .unwrap_or(0);
    let source_types = sc
        .available_source_types()
        .await
        .map(|m| m.bits())
        .unwrap_or(0);
    negotiate::choose_source_types(source_types)?;
    Ok((
        sc,
        Probe {
            cursor_modes,
            source_types,
        },
    ))
}

/// Create a session, show the picker (inside `Start`) and open the PipeWire remote. While the
/// picker is open the caller's `shared` is in `Picking`. The `Closed` signal of the session
/// (compositor ended it) is forwarded to `shared` for the whole life of the session.
pub(super) async fn pick(
    sc: &Screencast,
    probe: &Probe,
    opts: &PickOptions,
    shared: &Arc<Shared>,
) -> Result<Portal, CaptureError> {
    let session = sc
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|e| map_error(&e))?;

    let session = Arc::new(session);
    // Forward "the compositor closed the session" for the whole life of the session. The
    // subscription is made inside the task (the stream borrows the session).
    let watched = Arc::clone(&session);
    let sh = Arc::clone(shared);
    tokio::spawn(async move {
        let Ok(stream) = watched.receive_closed().await else {
            return;
        };
        let mut stream = std::pin::pin!(stream);
        if stream.next().await.is_some() {
            sh.event(Event::SystemStopped);
        }
    });

    let types = SourceType::from_bits_truncate(negotiate::choose_source_types(probe.source_types)?);
    let mut select = SelectSourcesOptions::default()
        .set_multiple(false)
        .set_sources(types)
        .set_persist_mode(if opts.persist {
            PersistMode::ExplicitlyRevoked
        } else {
            PersistMode::DoNot
        });
    if let Some(mode) = negotiate::choose_cursor_mode(probe.cursor_modes) {
        select = select.set_cursor_mode(CursorMode::from_bits_truncate(mode));
    }
    if let Some(t) = opts.restore_token.as_deref() {
        select = select.set_restore_token(t);
    }
    sc.select_sources(&session, select)
        .await
        .map_err(|e| map_error(&e))?
        .response()
        .map_err(|e| map_error(&e))?;

    shared.event(Event::PickerShown);
    let started = sc
        .start(&session, None, StartCastOptions::default())
        .await
        .map_err(|e| map_error(&e))?
        .response();
    let started = match started {
        Ok(s) => s,
        Err(e) => {
            let _ = session.close().await;
            return Err(map_error(&e));
        }
    };
    let streams: Vec<StreamInfo> = started
        .streams()
        .iter()
        .map(|s| StreamInfo {
            node_id: s.pipe_wire_node_id(),
            position: s.position(),
            logical_size: s.size(),
            is_window: s.source_type() == Some(SourceType::WINDOW),
        })
        .collect();
    if negotiate::pick_stream(&streams).is_none() {
        let _ = session.close().await;
        // The person confirmed an empty selection: same as cancelling.
        return Err(CaptureError::Denied);
    }
    let restore_token = started.restore_token().map(str::to_owned);
    let fd = sc
        .open_pipe_wire_remote(&session, Default::default())
        .await
        .map_err(|e| map_error(&e))?;
    Ok(Portal {
        session,
        streams,
        fd,
        restore_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashpd::desktop::ResponseError;

    #[test]
    fn cancelled_and_refused_responses_are_denied() {
        assert_eq!(
            map_error(&ashpd::Error::Response(ResponseError::Cancelled)),
            CaptureError::Denied
        );
        assert_eq!(
            map_error(&ashpd::Error::Response(ResponseError::Other)),
            CaptureError::Denied
        );
    }

    #[test]
    fn old_interface_is_unsupported() {
        assert!(matches!(
            map_error(&ashpd::Error::RequiresVersion(4, 2)),
            CaptureError::Unsupported(_)
        ));
        assert!(matches!(
            map_error(&ashpd::Error::NoResponse),
            CaptureError::Display(_)
        ));
    }
}
