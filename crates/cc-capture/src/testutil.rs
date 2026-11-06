// SPDX-License-Identifier: GPL-3.0-or-later
//! Test helpers: start a private `Xvfb` and draw a window of a known colour with a tiny X11
//! client (no xterm needed). Only compiled for tests and with the `test-support` feature.
//!
//! If `Xvfb` is not installed, [`TestXServer::start`] returns `None` and prints a loud note;
//! tests then skip. Install it with `apt-get install xvfb` to run them.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _, CreateWindowAux, WindowClass};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// A running `Xvfb` that is stopped when dropped.
pub struct TestXServer {
    child: Child,
    /// Display name such as `:101`.
    pub display: String,
}

impl TestXServer {
    /// Start `Xvfb` with the given screen size (24-bit colour). Returns `None` if the program
    /// is missing or does not start; a note goes to stderr.
    pub fn start(width: u32, height: u32) -> Option<Self> {
        // `-displayfd 1`: Xvfb picks a free display number and prints it to stdout.
        let spawned = Command::new("Xvfb")
            .args([
                "-displayfd",
                "1",
                "-screen",
                "0",
                &format!("{width}x{height}x24"),
                "-nolisten",
                "tcp",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                eprintln!("NOTE: SKIPPING X11 TEST, cannot start Xvfb ({e}); install package xvfb");
                return None;
            }
        };
        let mut line = String::new();
        if let Some(out) = child.stdout.take() {
            let _ = BufReader::new(out).read_line(&mut line);
        }
        let number = line.trim();
        if number.is_empty() || number.parse::<u32>().is_err() {
            eprintln!("NOTE: SKIPPING X11 TEST, Xvfb did not report a display number");
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        Some(Self {
            child,
            display: format!(":{number}"),
        })
    }
}

impl Drop for TestXServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A top-level window filled with one colour. The X server paints the window background
/// itself, so no event loop is needed; the window lives as long as this value.
pub struct TestWindow {
    conn: RustConnection,
    window: xproto::Window,
}

impl TestWindow {
    /// Create and map a window at `(x, y)` with the given size and RGB colour.
    pub fn open(
        display: &str,
        rect: (i16, i16, u16, u16),
        rgb: [u8; 3],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (conn, screen_num) = x11rb::connect(Some(display))?;
        let root = conn.setup().roots[screen_num].root;
        let window = conn.generate_id()?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixel(pixel(rgb)),
        )?;
        conn.map_window(window)?;
        conn.sync()?;
        Ok(Self { conn, window })
    }

    /// Move the window.
    pub fn move_to(&self, x: i16, y: i16) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.configure_window(
            self.window,
            &xproto::ConfigureWindowAux::new()
                .x(i32::from(x))
                .y(i32::from(y)),
        )?;
        self.conn.sync()?;
        Ok(())
    }

    /// Repaint the whole window in another colour.
    pub fn set_colour(&self, rgb: [u8; 3]) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.change_window_attributes(
            self.window,
            &xproto::ChangeWindowAttributesAux::new().background_pixel(pixel(rgb)),
        )?;
        self.conn.clear_area(false, self.window, 0, 0, 0, 0)?;
        self.conn.sync()?;
        Ok(())
    }
}

fn pixel(rgb: [u8; 3]) -> u32 {
    (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2])
}
