// SPDX-License-Identifier: GPL-3.0-or-later
//! The display's AirPlay receiver (`AirplayReceiver`) against the FAKE uxplay script: admission
//! by join mode (Open, Approval on, Locked), Comet Cast first, the mirroring signal.
//! Unix only: the tests use shell scripts.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use cc_airplay::{
    AirplayConfig, AirplayReceiver, Backoff, ReceiveEvent, ReceiveOptions, SupervisorOptions,
};

/// Generous: `cargo test --workspace` runs many busy test binaries at once.
const WAIT: Duration = Duration::from_secs(60);

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake-uxplay.sh");

fn launcher(test: &str, mode: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cc-airplay-rx-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("uxplay");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nFAKE_UXPLAY_MODE={mode} FAKE_UXPLAY_COUNTER={} FAKE_UXPLAY_HELPER_PID={} exec bash {FAKE} \"$@\"\n",
            dir.join("counter").display(),
            dir.join("helper.pid").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    (script, dir)
}

fn options(path: PathBuf, ask_first: bool) -> ReceiveOptions {
    let mut o = ReceiveOptions::new(AirplayConfig {
        uxplay_path: path,
        fullscreen: false,
        ..AirplayConfig::default()
    });
    o.ask_first = ask_first;
    o.supervisor = SupervisorOptions {
        stop_timeout: Duration::from_millis(500),
        reader_grace: Duration::from_secs(30),
        backoff: Backoff::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(60),
            5,
        ),
        confirm: None,
    };
    o
}

fn wait_for(
    rx: &Receiver<ReceiveEvent>,
    mut pred: impl FnMut(&ReceiveEvent) -> bool,
) -> Vec<ReceiveEvent> {
    let end = Instant::now() + WAIT;
    let mut seen = Vec::new();
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(ev) => {
                let done = pred(&ev);
                seen.push(ev);
                if done {
                    return seen;
                }
            }
            Err(_) => break,
        }
    }
    panic!("timed out; saw only {seen:?}");
}

fn is_pin(e: &ReceiveEvent) -> bool {
    matches!(e, ReceiveEvent::Pin { .. })
}

#[test]
fn open_shows_the_pin_at_once_and_reports_the_mirroring() {
    let (path, dir) = launcher("open", "mirror");
    let (rx_core, rx) = AirplayReceiver::start(options(path, false)).unwrap();
    let seen = wait_for(&rx, |e| *e == ReceiveEvent::Mirroring { active: true });
    assert!(seen.contains(&ReceiveEvent::Advertising), "{seen:?}");
    assert!(
        !seen.contains(&ReceiveEvent::Request),
        "Open asks no question: {seen:?}"
    );
    assert!(seen.contains(&ReceiveEvent::Pin { pin: "9876".into() }));
    assert!(rx_core.mirroring());
    // The PIN dialog is closed again before the picture (the probe connection closed).
    assert!(seen.contains(&ReceiveEvent::PinDone), "{seen:?}");
    rx_core.stop();
    let end = wait_for(&rx, |e| *e == ReceiveEvent::Stopped);
    assert!(
        end.contains(&ReceiveEvent::Mirroring { active: false }),
        "{end:?}"
    );
    assert!(!rx_core.mirroring());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn approval_on_asks_before_the_pin_and_a_yes_shows_it() {
    let (path, dir) = launcher("approve-yes", "normal");
    let (core, rx) = AirplayReceiver::start(options(path, true)).unwrap();
    let seen = wait_for(&rx, |e| *e == ReceiveEvent::Request);
    assert!(
        !seen.iter().any(is_pin),
        "no PIN before the answer: {seen:?}"
    );
    assert!(core.answer(true));
    assert!(!core.answer(true), "a request is answered once");
    let seen = wait_for(&rx, is_pin);
    assert_eq!(seen.last(), Some(&ReceiveEvent::Pin { pin: "9876".into() }));
    core.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn approval_on_a_no_closes_the_dialog_and_restarts_uxplay() {
    let (path, dir) = launcher("approve-no", "normal");
    let (core, rx) = AirplayReceiver::start(options(path, true)).unwrap();
    wait_for(&rx, |e| *e == ReceiveEvent::Request);
    assert!(core.answer(false));
    let seen = wait_for(&rx, |e| *e == ReceiveEvent::PinDone);
    assert!(!seen.iter().any(is_pin), "{seen:?}");
    // UxPlay was restarted (that drops the pairing): it is offered again.
    let again = wait_for(&rx, |e| *e == ReceiveEvent::Advertising);
    assert!(!again.iter().any(is_pin), "{again:?}");
    core.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_request_that_nobody_answers_is_refused() {
    let (path, dir) = launcher("approve-timeout", "normal");
    let mut o = options(path, true);
    o.request_timeout = Duration::from_millis(300);
    let (core, rx) = AirplayReceiver::start(o).unwrap();
    wait_for(&rx, |e| *e == ReceiveEvent::Request);
    let seen = wait_for(&rx, |e| *e == ReceiveEvent::PinDone);
    assert!(!seen.iter().any(is_pin), "{seen:?}");
    core.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn switching_to_open_applies_to_the_next_device() {
    let (path, dir) = launcher("mode-change", "normal");
    let (core, rx) = AirplayReceiver::start(options(path, true)).unwrap();
    wait_for(&rx, |e| *e == ReceiveEvent::Request);
    core.set_ask_first(false);
    core.answer(false);
    // The restarted UxPlay pairs again: now without a question.
    let seen = wait_for(&rx, is_pin);
    assert!(
        !seen
            .iter()
            .skip_while(|e| **e != ReceiveEvent::PinDone)
            .any(|e| *e == ReceiveEvent::Request),
        "{seen:?}"
    );
    core.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn comet_cast_first_refuses_without_a_request() {
    let (path, dir) = launcher("refusing", "normal");
    let (core, rx) = AirplayReceiver::start(options(path, false)).unwrap();
    core.set_refusing(true);
    // Refused: UxPlay is restarted at least once and no PIN or request is ever shown.
    let mut offered = 0;
    let seen = wait_for(&rx, |e| {
        if *e == ReceiveEvent::Advertising {
            offered += 1;
        }
        offered >= 2
    });
    assert!(
        !seen
            .iter()
            .any(|e| is_pin(e) || *e == ReceiveEvent::Request),
        "{seen:?}"
    );
    // Presenting is over: the next device gets the PIN.
    core.set_refusing(false);
    wait_for(&rx, is_pin);
    core.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn locked_stops_the_service_and_unlocking_starts_it_again() {
    let (path, dir) = launcher("locked", "normal");
    let (core, rx) = AirplayReceiver::start(options(path, false)).unwrap();
    wait_for(&rx, |e| *e == ReceiveEvent::Advertising);
    core.set_locked(true);
    let seen = wait_for(&rx, |e| *e == ReceiveEvent::NotAnnouncing);
    assert!(!seen.contains(&ReceiveEvent::Request), "{seen:?}");
    core.set_locked(false);
    wait_for(&rx, |e| *e == ReceiveEvent::Advertising);
    core.stop();
    wait_for(&rx, |e| *e == ReceiveEvent::Stopped);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn locked_keeps_a_running_mirror_until_it_ends() {
    let (path, dir) = launcher("locked-mirror", "mirror");
    let (core, rx) = AirplayReceiver::start(options(path, false)).unwrap();
    wait_for(&rx, |e| *e == ReceiveEvent::Mirroring { active: true });
    core.set_locked(true);
    // Not announcing is NOT reported while the picture runs ...
    std::thread::sleep(Duration::from_millis(400));
    while let Ok(e) = rx.try_recv() {
        assert_ne!(
            e,
            ReceiveEvent::NotAnnouncing,
            "stopped under a running mirror"
        );
    }
    assert!(core.mirroring());
    // ... and the end of the picture (the device stops, here the app ends it) stops the service.
    core.end_session();
    let seen = wait_for(&rx, |e| *e == ReceiveEvent::NotAnnouncing);
    assert!(
        seen.contains(&ReceiveEvent::Mirroring { active: false }),
        "{seen:?}"
    );
    core.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_missing_program_is_a_failure_code() {
    let (core, rx) = AirplayReceiver::start(options(
        PathBuf::from("/nonexistent/uxplay-does-not-exist"),
        false,
    ))
    .unwrap();
    let seen = wait_for(&rx, |e| matches!(e, ReceiveEvent::Failed { .. }));
    assert_eq!(
        seen.last(),
        Some(&ReceiveEvent::Failed {
            code: "not_installed".into()
        })
    );
    core.stop();
}

#[test]
fn stop_ends_the_stream() {
    let (path, dir) = launcher("stop", "normal");
    let (core, rx) = AirplayReceiver::start(options(path, false)).unwrap();
    wait_for(&rx, |e| *e == ReceiveEvent::Advertising);
    core.stop();
    core.stop();
    wait_for(&rx, |e| *e == ReceiveEvent::Stopped);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_installed_uxplay_prints_the_lines_the_signal_needs() {
    // Only where UxPlay is installed (the CI image has the Ubuntu package 1.68).
    let Some(path) = cc_airplay::locate_uxplay() else {
        eprintln!("skipped: no uxplay on this machine");
        return;
    };
    assert!(
        cc_airplay::has_mirroring_signal(&path),
        "{} has no mirroring signal: AirPlay would not be offered",
        path.display()
    );
}
