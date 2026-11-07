// SPDX-License-Identifier: GPL-3.0-or-later
//! The plug-in library against the real library (crates/cc-capture-pw). Runs only when
//! `COMETCAST_CAPTURE_PW_LIB` names a built `libcc_capture_pw.so`:
//!
//! ```text
//! cargo build -p cc-capture-pw --features plugin
//! COMETCAST_CAPTURE_PW_LIB=target/debug/libcc_capture_pw.so cargo test -p cc-capture --test pwplugin
//! ```
//!
//! Without a ScreenCast portal on the session bus (CI, containers) the library loads and answers
//! `NoPortal`; with one, `connect` succeeds and shows nothing.

#![cfg(target_os = "linux")]

use cc_capture::pwplugin::{LIBRARY_ENV, PluginSource};
use cc_capture::{CaptureError, PickerOptions, ScreenSource};

#[test]
fn the_real_library_loads_and_reports_the_portal() {
    let Ok(path) = std::env::var(LIBRARY_ENV) else {
        eprintln!("skipped: {LIBRARY_ENV} is not set");
        return;
    };
    let opts = PickerOptions {
        app_id: Some(cc_capture::APP_ID.into()),
        ..PickerOptions::default()
    };
    match PluginSource::connect_from(&[path.into()], &opts) {
        Ok(src) => {
            // A portal answered: the source lists the system picker; dropping it closes the
            // handle without ever showing a dialog.
            assert_eq!(src.list_sources().unwrap().len(), 1);
        }
        Err(CaptureError::NoPortal(_)) => {}
        Err(other) => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn a_library_that_is_not_there_is_no_portal() {
    let r = PluginSource::connect_from(
        &["/nonexistent/libcc_capture_pw.so".into()],
        &PickerOptions::default(),
    );
    assert!(matches!(r, Err(CaptureError::NoPortal(_))));
}
