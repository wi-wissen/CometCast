// SPDX-License-Identifier: GPL-3.0-or-later
//! Test load for the X11 capture: draws something that keeps changing on an X display.
//!
//! `cargo run -p cc-capture --features test-support --example xanim -- :99 box 30`
//!
//! Modes: `box` moves a small coloured window around (a small part of the screen changes),
//! `flash` repaints a full-screen window in a new colour every step (the whole screen changes).
//! The last argument is the number of steps per second (default 30).

use std::time::{Duration, Instant};

use cc_capture::testutil::TestWindow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let display = args
        .next()
        .ok_or("usage: xanim DISPLAY box|flash [STEPS_PER_SECOND]")?;
    let mode = args.next().unwrap_or_else(|| "box".into());
    let rate: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(30);
    let step = Duration::from_micros(1_000_000 / rate.max(1));
    let started = Instant::now();
    let mut i: u32 = 0;
    match mode.as_str() {
        "flash" => {
            let w = TestWindow::open(&display, (0, 0, 1280, 720), [0, 0, 0])?;
            loop {
                i = i.wrapping_add(1);
                w.set_colour([(i * 37) as u8, (i * 91) as u8, (i * 53) as u8])?;
                std::thread::sleep(step);
            }
        }
        _ => {
            let w = TestWindow::open(&display, (0, 0, 200, 120), [250, 200, 20])?;
            loop {
                i = i.wrapping_add(1);
                let x = ((i * 7) % 1000) as i16;
                let y = ((i * 5) % 560) as i16;
                w.move_to(x, y)?;
                if started.elapsed() > Duration::from_secs(3600) {
                    return Ok(());
                }
                std::thread::sleep(step);
            }
        }
    }
}
