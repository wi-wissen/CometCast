// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen capture on X11 with the pure-Rust `x11rb` crate.
//!
//! How a frame is made: wait for the next frame time; if the server has the DAMAGE extension
//! and no damage was reported since the last frame, report [`Capture::Unchanged`] without
//! reading any pixels; otherwise read the picture with the core `GetImage` request and compare
//! its hash with the previous one ([`ChangeDetector`]).
//!
//! Limits (see docs/capture.md): `GetImage` copies the pixels through the X connection (the
//! MIT-SHM extension would be faster but needs `unsafe` shared memory, which this workspace
//! forbids); the mouse pointer is not in the picture; only 24/32-bit TrueColor little-endian
//! BGRX roots are handled; windows that use compositing tricks or other screens of a
//! multi-screen (Zaphod) setup are not considered.

use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::damage::{self, ConnectionExt as _};
use x11rb::protocol::randr::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ConnectionExt as _, ImageFormat};
use x11rb::rust_connection::RustConnection;

use crate::{Capture, CaptureError, CaptureSource, ChangeDetector, RawFrame, ScreenSource};

/// Settings of [`X11Source`].
#[derive(Debug, Clone, Copy)]
pub struct X11Options {
    /// Target frames per second (how often the screen is looked at).
    pub fps: u32,
}

impl Default for X11Options {
    fn default() -> Self {
        Self { fps: 15 }
    }
}

/// Id of the source that covers the whole desktop (the X root window).
pub const ROOT_SOURCE_ID: &str = "root";

struct Running {
    source: CaptureSource,
    detector: ChangeDetector,
    damage: Option<damage::Damage>,
    /// True until the first frame was delivered.
    first: bool,
    next_at: Instant,
}

/// Screen capture on X11.
pub struct X11Source {
    conn: RustConnection,
    root: xproto::Window,
    options: X11Options,
    running: Option<Running>,
}

impl std::fmt::Debug for X11Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X11Source")
            .field("root", &self.root)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

fn disp<E: std::fmt::Display>(e: E) -> CaptureError {
    CaptureError::Display(e.to_string())
}

impl X11Source {
    /// Connect to an X server. `display` is e.g. `":99"`; `None` uses `$DISPLAY`.
    /// Also checks that the root window has a pixel layout this code can read.
    pub fn connect(display: Option<&str>, options: X11Options) -> Result<Self, CaptureError> {
        let (conn, screen_num) = x11rb::connect(display).map_err(disp)?;
        let setup = conn.setup();
        let screen = setup
            .roots
            .get(screen_num)
            .ok_or_else(|| CaptureError::Display("screen number out of range".into()))?;
        let root = screen.root;
        check_pixel_layout(&conn, screen)?;
        Ok(Self {
            conn,
            root,
            options,
            running: None,
        })
    }

    fn root_geometry(&self) -> Result<(i32, i32, u32, u32), CaptureError> {
        let g = self
            .conn
            .get_geometry(self.root)
            .map_err(disp)?
            .reply()
            .map_err(disp)?;
        Ok((0, 0, u32::from(g.width), u32::from(g.height)))
    }

    /// True if the running capture uses the DAMAGE extension as a "nothing changed" hint.
    pub fn uses_damage(&self) -> bool {
        self.running.as_ref().is_some_and(|r| r.damage.is_some())
    }

    fn has_extension(&self, name: &'static str) -> bool {
        matches!(self.conn.extension_information(name), Ok(Some(_)))
    }

    fn monitors(&self) -> Vec<CaptureSource> {
        if !self.has_extension(randr::X11_EXTENSION_NAME) {
            return Vec::new();
        }
        let Ok(cookie) = self.conn.randr_get_monitors(self.root, true) else {
            return Vec::new();
        };
        let Ok(reply) = cookie.reply() else {
            return Vec::new();
        };
        reply
            .monitors
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let name = self
                    .conn
                    .get_atom_name(m.name)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .map(|r| String::from_utf8_lossy(&r.name).into_owned())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("Monitor {}", i + 1));
                CaptureSource {
                    id: format!("monitor:{i}"),
                    name,
                    x: i32::from(m.x),
                    y: i32::from(m.y),
                    width: u32::from(m.width),
                    height: u32::from(m.height),
                }
            })
            .collect()
    }

    /// Set up DAMAGE on the root window. `None` if the server has no such extension.
    fn start_damage(&self) -> Option<damage::Damage> {
        if !self.has_extension(damage::X11_EXTENSION_NAME) {
            return None;
        }
        self.conn.damage_query_version(1, 1).ok()?.reply().ok()?;
        let id = self.conn.generate_id().ok()?;
        self.conn
            .damage_create(id, self.root, damage::ReportLevel::NON_EMPTY)
            .ok()?
            .check()
            .ok()?;
        Some(id)
    }

    /// Drain pending events; true if a damage notification was among them.
    fn damage_seen(&self) -> Result<bool, CaptureError> {
        let mut seen = false;
        while let Some(ev) = self.conn.poll_for_event().map_err(disp)? {
            if matches!(ev, Event::DamageNotify(_)) {
                seen = true;
            }
        }
        Ok(seen)
    }
}

/// The picture must be: depth 24 or 32, 32 bits per pixel, no row padding beyond 4 bytes, little
/// endian image byte order, red/green/blue masks of the usual BGRX layout.
fn check_pixel_layout(conn: &RustConnection, screen: &xproto::Screen) -> Result<(), CaptureError> {
    let setup = conn.setup();
    let depth = screen.root_depth;
    let fmt = setup
        .pixmap_formats
        .iter()
        .find(|f| f.depth == depth)
        .ok_or_else(|| CaptureError::PixelFormat(format!("no pixmap format for depth {depth}")))?;
    if fmt.bits_per_pixel != 32 || fmt.scanline_pad != 32 {
        return Err(CaptureError::PixelFormat(format!(
            "depth {depth} uses {} bits per pixel, pad {}; only 32/32 is handled",
            fmt.bits_per_pixel, fmt.scanline_pad
        )));
    }
    if setup.image_byte_order != xproto::ImageOrder::LSB_FIRST {
        return Err(CaptureError::PixelFormat("big-endian image order".into()));
    }
    let visual = screen
        .allowed_depths
        .iter()
        .flat_map(|d| d.visuals.iter())
        .find(|v| v.visual_id == screen.root_visual)
        .ok_or_else(|| CaptureError::PixelFormat("root visual not found".into()))?;
    if (visual.red_mask, visual.green_mask, visual.blue_mask) != (0xff_0000, 0x00_ff00, 0x00_00ff) {
        return Err(CaptureError::PixelFormat(format!(
            "colour masks {:#x}/{:#x}/{:#x} are not the BGRX layout",
            visual.red_mask, visual.green_mask, visual.blue_mask
        )));
    }
    Ok(())
}

impl ScreenSource for X11Source {
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
        let (x, y, w, h) = self.root_geometry()?;
        let mut out = vec![CaptureSource {
            id: ROOT_SOURCE_ID.into(),
            name: "Whole desktop".into(),
            x,
            y,
            width: w,
            height: h,
        }];
        out.extend(self.monitors());
        Ok(out)
    }

    fn start(&mut self, source: &CaptureSource) -> Result<(), CaptureError> {
        // Look the source up again so a stale geometry cannot be used.
        let known = self
            .list_sources()?
            .into_iter()
            .find(|s| s.id == source.id)
            .ok_or_else(|| CaptureError::NoSuchSource(source.id.clone()))?;
        let damage = self.start_damage();
        self.running = Some(Running {
            source: known,
            detector: ChangeDetector::new(),
            damage,
            first: true,
            next_at: Instant::now(),
        });
        Ok(())
    }

    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        let Some(run) = self.running.as_ref() else {
            return Err(CaptureError::NotStarted);
        };
        // Wait for the frame time. The deadline moves by whole intervals, so a slow capture
        // does not make the following ones faster to "catch up".
        let interval = Duration::from_secs(1) / self.options.fps.max(1);
        let now = Instant::now();
        let next_at = run.next_at;
        if next_at > now {
            std::thread::sleep(next_at - now);
        }
        let after = Instant::now().max(next_at);
        let (damage_id, first, mut source) = {
            let run = self.running.as_mut().ok_or(CaptureError::NotStarted)?;
            run.next_at = after + interval;
            (run.damage, run.first, run.source.clone())
        };

        let damaged = self.damage_seen()?;
        if damage_id.is_some() && !first && !damaged {
            // The server says nothing changed: no pixels are read.
            return Ok(Capture::Unchanged);
        }
        if let Some(id) = damage_id {
            // Re-arm before reading, so a change that happens during the read is reported.
            self.conn
                .damage_subtract(id, x11rb::NONE, x11rb::NONE)
                .map_err(disp)?;
        }

        if source.id == ROOT_SOURCE_ID {
            // The root can change size (RandR); follow it.
            let (x, y, w, h) = self.root_geometry()?;
            source.x = x;
            source.y = y;
            source.width = w;
            source.height = h;
        }
        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                i16::try_from(source.x).map_err(disp)?,
                i16::try_from(source.y).map_err(disp)?,
                u16::try_from(source.width).map_err(disp)?,
                u16::try_from(source.height).map_err(disp)?,
                !0,
            )
            .map_err(disp)?
            .reply()
            .map_err(disp)?;
        let expected = source.width as usize * source.height as usize * 4;
        if reply.data.len() != expected {
            return Err(CaptureError::PixelFormat(format!(
                "GetImage returned {} bytes, expected {expected}",
                reply.data.len()
            )));
        }
        let mut bgra = reply.data;
        // The fourth byte is padding with unspecified content; make it opaque.
        for px in bgra.chunks_exact_mut(4) {
            px[3] = 255;
        }
        let run = self.running.as_mut().ok_or(CaptureError::NotStarted)?;
        run.first = false;
        if !run.detector.update(source.width, source.height, &bgra) {
            return Ok(Capture::Unchanged);
        }
        Ok(Capture::Frame(RawFrame {
            width: source.width,
            height: source.height,
            bgra,
        }))
    }

    fn stop(&mut self) {
        if let Some(run) = self.running.take()
            && let Some(id) = run.damage
        {
            let _ = self.conn.damage_destroy(id);
            let _ = self.conn.flush();
        }
    }
}

impl Drop for X11Source {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{TestWindow, TestXServer};

    fn pixel_at(f: &RawFrame, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * f.width + x) * 4) as usize;
        [f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]]
    }

    fn started(server: &TestXServer, fps: u32) -> X11Source {
        let mut src = X11Source::connect(Some(&server.display), X11Options { fps }).unwrap();
        let root = src.list_sources().unwrap().remove(0);
        src.start(&root).unwrap();
        src
    }

    #[test]
    fn lists_the_desktop_and_monitors() {
        let Some(server) = TestXServer::start(640, 480) else {
            return;
        };
        let src = X11Source::connect(Some(&server.display), X11Options::default()).unwrap();
        let list = src.list_sources().unwrap();
        assert_eq!(list[0].id, ROOT_SOURCE_ID);
        assert_eq!((list[0].width, list[0].height), (640, 480));
        // UNCERTAIN: RandR monitor names/count differ between servers; here only the shape is
        // checked.
        for m in &list[1..] {
            assert!(m.id.starts_with("monitor:") && m.width > 0);
        }
    }

    #[test]
    fn captures_a_known_colour_and_reports_unchanged_until_it_changes() {
        let Some(server) = TestXServer::start(640, 480) else {
            return;
        };
        let win = TestWindow::open(&server.display, (100, 50, 200, 100), [200, 30, 40]).unwrap();
        let mut src = started(&server, 50);
        // Xvfb has DAMAGE; if this ever fails the hash alone still gives correct results.
        assert!(src.uses_damage(), "DAMAGE extension not in use");

        let Capture::Frame(first) = src.next_frame().unwrap() else {
            panic!("first call must deliver a frame");
        };
        assert_eq!((first.width, first.height), (640, 480));
        assert_eq!(first.bgra.len(), 640 * 480 * 4);
        assert_eq!(
            pixel_at(&first, 150, 80),
            [200, 30, 40],
            "inside the window"
        );
        assert_eq!(
            pixel_at(&first, 10, 10),
            [0, 0, 0],
            "outside is the black root"
        );
        assert_eq!(first.bgra[3], 255, "alpha is opaque");

        // Nothing happens on screen: only Unchanged, over many frame times.
        for _ in 0..10 {
            assert_eq!(src.next_frame().unwrap(), Capture::Unchanged);
        }

        win.set_colour([20, 160, 220]).unwrap();
        // The change may need one frame time to show up; look at the next few calls.
        let mut got = None;
        for _ in 0..10 {
            if let Capture::Frame(f) = src.next_frame().unwrap() {
                got = Some(f);
                break;
            }
        }
        let f = got.expect("the colour change must produce a frame");
        assert_eq!(pixel_at(&f, 150, 80), [20, 160, 220]);
        assert_eq!(src.next_frame().unwrap(), Capture::Unchanged);
        src.stop();
    }

    #[test]
    fn the_frame_rate_is_paced() {
        let Some(server) = TestXServer::start(320, 240) else {
            return;
        };
        let mut src = started(&server, 20);
        let t = Instant::now();
        for _ in 0..11 {
            let _ = src.next_frame().unwrap();
        }
        // 11 calls at 20 fps: the first is immediate, ten more take about 500 ms.
        let ms = t.elapsed().as_millis();
        assert!((400..900).contains(&ms), "took {ms} ms");
    }

    #[test]
    fn next_frame_before_start_and_unknown_source_fail() {
        let Some(server) = TestXServer::start(320, 240) else {
            return;
        };
        let mut src = X11Source::connect(Some(&server.display), X11Options::default()).unwrap();
        assert_eq!(src.next_frame(), Err(CaptureError::NotStarted));
        let bogus = CaptureSource {
            id: "monitor:99".into(),
            name: String::new(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        assert!(matches!(
            src.start(&bogus),
            Err(CaptureError::NoSuchSource(_))
        ));
    }

    #[test]
    fn connecting_to_nothing_is_an_error() {
        assert!(X11Source::connect(Some(":9999"), X11Options::default()).is_err());
    }
}
