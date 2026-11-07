// SPDX-License-Identifier: GPL-3.0-or-later
//! [`WaylandSource`]: the `ScreenSource` of the portal + PipeWire backend.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::runtime::Runtime;

use super::Shared;
use super::negotiate::{self, StreamInfo};
use super::portal::{self, PickOptions, Portal, Probe};
use super::pw::PwThread;
use super::restore::RestoreStore;
use crate::session::{Event, Pacer, Phase, Taken};
use crate::{
    Capture, CaptureError, CaptureSource, ChangeDetector, PICKER_SOURCE_ID, PickerOptions,
    RawFrame, ScreenSource,
};

/// How long `start` waits for the first picture after the person picked something.
/// UNCERTAIN: placeholder; compositors that need a moment to set up a window capture may take
/// longer than a monitor.
pub const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);

/// Screen capture through the desktop portal (system picker) and PipeWire.
pub struct WaylandSource {
    rt: Runtime,
    sc: ashpd::desktop::screencast::Screencast,
    probe: Probe,
    opts: PickerOptions,
    shared: Arc<Shared>,
    portal: Option<Portal>,
    pw: Option<PwThread>,
    pacer: Pacer,
    detector: ChangeDetector,
    /// The first picture, taken in `start`, returned by the first `next_frame`.
    first: Option<RawFrame>,
    info: Option<StreamInfo>,
}

impl std::fmt::Debug for WaylandSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaylandSource").finish_non_exhaustive()
    }
}

impl WaylandSource {
    /// Connect to the portal and check that it can capture monitors or windows. Shows
    /// nothing. Fails with `Unsupported` where there is no portal.
    pub fn connect(opts: &PickerOptions) -> Result<Self, CaptureError> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("cometcast-portal")
            .build()
            .map_err(|e| CaptureError::Display(format!("cannot start the portal runtime: {e}")))?;
        let (sc, probe) = rt.block_on(portal::connect(opts.app_id.as_deref()))?;
        Ok(Self {
            rt,
            sc,
            probe,
            opts: opts.clone(),
            shared: Arc::new(Shared::new()),
            portal: None,
            pw: None,
            pacer: Pacer::new(opts.fps),
            detector: ChangeDetector::new(),
            first: None,
            info: None,
        })
    }

    /// True if a ScreenCast portal answers on the session bus. Used to choose between the
    /// portal and the X11 fallback.
    pub fn portal_available() -> bool {
        Self::connect(&PickerOptions::default()).is_ok()
    }

    fn store(&self) -> Option<RestoreStore> {
        self.opts
            .use_restore_token
            .then(RestoreStore::default_location)
            .flatten()
    }
}

impl ScreenSource for WaylandSource {
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
        Ok(vec![CaptureSource {
            id: PICKER_SOURCE_ID.into(),
            name: "system picker".into(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }])
    }

    fn start(&mut self, _source: &CaptureSource) -> Result<(), CaptureError> {
        if self.portal.is_some() {
            return Err(CaptureError::Display("capture is already running".into()));
        }
        let store = self.store();
        let pick_opts = PickOptions {
            restore_token: store.as_ref().and_then(RestoreStore::load),
            persist: store.is_some(),
        };
        self.shared = Arc::new(Shared::new());
        let portal = match self.rt.block_on(portal::pick(
            &self.sc,
            &self.probe,
            &pick_opts,
            &self.shared,
        )) {
            Ok(p) => p,
            Err(e) => {
                // A refused stored token must not stick.
                if let (Some(s), Some(_)) = (&store, &pick_opts.restore_token) {
                    s.clear();
                }
                return Err(e);
            }
        };
        if let (Some(s), Some(t)) = (&store, &portal.restore_token) {
            let _ = s.save(t);
        }
        let Some(info) = negotiate::pick_stream(&portal.streams).cloned() else {
            return Err(CaptureError::Denied);
        };
        self.shared.event(Event::Picked);
        let pw = PwThread::spawn(
            portal.fd.try_clone().map_err(|e| {
                CaptureError::Display(format!("cannot duplicate the PipeWire fd: {e}"))
            })?,
            info.node_id,
            Arc::clone(&self.shared),
        )?;
        self.pw = Some(pw);
        self.portal = Some(portal);
        self.info = Some(info);
        match self.shared.mailbox.take_timeout(FIRST_FRAME_TIMEOUT) {
            Taken::Item(f) => {
                self.first = Some(f);
                Ok(())
            }
            Taken::Empty => {
                self.stop();
                Err(CaptureError::Display(
                    "no picture arrived from the compositor".into(),
                ))
            }
            Taken::Closed => {
                let err =
                    self.shared.phase().start_error().unwrap_or_else(|| {
                        CaptureError::Display("the stream ended at once".into())
                    });
                self.stop();
                Err(err)
            }
        }
    }

    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        if self.portal.is_none() {
            return Err(CaptureError::NotStarted);
        }
        let now = Instant::now();
        let wait = self.pacer.wait(now);
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        self.pacer.looked(Instant::now());
        let item = match self.first.take() {
            Some(f) => Taken::Item(f),
            None => self.shared.mailbox.take_timeout(Duration::ZERO),
        };
        match item {
            Taken::Item(f) => {
                if self.detector.update(f.width, f.height, &f.bgra) {
                    Ok(Capture::Frame(f))
                } else {
                    Ok(Capture::Unchanged)
                }
            }
            Taken::Empty => Ok(Capture::Unchanged),
            Taken::Closed => match self.shared.phase() {
                Phase::Failed(e) => Err(CaptureError::Display(e)),
                _ => Ok(Capture::Ended),
            },
        }
    }

    fn stop(&mut self) {
        self.shared.event(Event::StopRequested);
        if let Some(mut pw) = self.pw.take() {
            pw.stop();
        }
        if let Some(p) = self.portal.take() {
            let _ = self.rt.block_on(p.session.close());
        }
        self.first = None;
    }
}

impl Drop for WaylandSource {
    fn drop(&mut self) {
        self.stop();
    }
}
