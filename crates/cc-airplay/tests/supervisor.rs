// SPDX-License-Identifier: GPL-3.0-or-later
//! Supervisor tests with a FAKE uxplay script (tests/fixtures/fake-uxplay.sh), plus one
//! optional smoke test against the real UxPlay (only if COMETCAST_REAL_UXPLAY=1).
//! Unix only: the tests use shell scripts and process ids.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use cc_airplay::{
    AirplayConfig, AirplayEvent, Backoff, ConfirmHook, ConfirmRequest, Decision, Supervisor,
    SupervisorOptions,
};

const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake-uxplay.sh");

/// Make a small launcher script that sets the fake's mode and starts it.
fn launcher(test: &str, mode: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cc-airplay-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let counter = dir.join("counter");
    let script = dir.join("uxplay");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nFAKE_UXPLAY_MODE={mode} FAKE_UXPLAY_COUNTER={} exec bash {FAKE} \"$@\"\n",
            counter.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    (script, dir)
}

fn config(path: PathBuf) -> AirplayConfig {
    AirplayConfig {
        uxplay_path: path,
        fullscreen: false,
        ..AirplayConfig::default()
    }
}

fn fast_options() -> SupervisorOptions {
    SupervisorOptions {
        stop_timeout: Duration::from_millis(500),
        backoff: Backoff::new(
            Duration::from_millis(100),
            Duration::from_millis(400),
            Duration::from_secs(60),
            5,
        ),
        confirm: None,
    }
}

/// Collect events until `pred` says an event is the one we wait for (or time runs out).
fn collect_until(
    rx: &Receiver<AirplayEvent>,
    timeout: Duration,
    mut pred: impl FnMut(&AirplayEvent) -> bool,
) -> Vec<AirplayEvent> {
    let end = Instant::now() + timeout;
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

/// True if the process is gone and was reaped (not a zombie).
fn is_reaped(pid: u32) -> bool {
    use nix::errno::Errno;
    use nix::sys::wait::{WaitPidFlag, waitpid};
    use nix::unistd::Pid;
    // A zombie child would be returned here. After reaping, the answer is ECHILD.
    let waited = waitpid(Pid::from_raw(pid as i32), Some(WaitPidFlag::WNOHANG));
    waited == Err(Errno::ECHILD) && !std::path::Path::new(&format!("/proc/{pid}")).exists()
}

fn stop_and_get_exit(sup: &mut Supervisor, rx: &Receiver<AirplayEvent>) -> Vec<AirplayEvent> {
    sup.stop();
    let mut all = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        all.push(ev);
    }
    all
}

#[test]
fn starts_extracts_events_and_stops_cleanly() {
    let (path, dir) = launcher("normal", "normal");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, Duration::from_secs(5), |e| {
        *e == AirplayEvent::ClientDisconnected
    });
    assert!(matches!(seen[0], AirplayEvent::Starting { attempt: 1 }));
    let pos = |want: &AirplayEvent| seen.iter().position(|e| e == want).unwrap();
    let ready = pos(&AirplayEvent::ServerStarted);
    let pin = pos(&AirplayEvent::PinShown { pin: "9876".into() });
    let conn = pos(&AirplayEvent::ClientConnected {
        open_connections: 1,
    });
    assert!(ready < pin && pin < conn);
    assert!(seen.contains(&AirplayEvent::PairingRequested));

    let t = Instant::now();
    let rest = stop_and_get_exit(&mut sup, &rx);
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "stop took {:?}",
        t.elapsed()
    );
    let Some(AirplayEvent::ProcessExited {
        pid,
        expected,
        code,
    }) = rest
        .iter()
        .find(|e| matches!(e, AirplayEvent::ProcessExited { .. }))
        .cloned()
    else {
        panic!("no ProcessExited in {rest:?}");
    };
    assert!(expected);
    assert_eq!(code, Some(0)); // the fake exits 0 on SIGTERM
    assert_eq!(rest.last(), Some(&AirplayEvent::Stopped));
    assert!(
        is_reaped(pid.unwrap()),
        "process {pid:?} is a zombie or still alive"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unexpected_exit_restarts_with_backoff() {
    let (path, dir) = launcher("crash", "crash-once");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, Duration::from_secs(8), |e| {
        matches!(e, AirplayEvent::PinShown { .. })
    });
    let died = seen
        .iter()
        .find_map(|e| match e {
            AirplayEvent::ProcessExited {
                expected: false,
                code,
                pid,
            } => Some((*code, *pid)),
            _ => None,
        })
        .expect("no unexpected exit seen");
    assert_eq!(died.0, Some(3));
    assert!(is_reaped(died.1.unwrap()));
    assert!(seen.contains(&AirplayEvent::Restarting {
        attempt: 1,
        delay: Duration::from_millis(100)
    }));
    assert!(seen.contains(&AirplayEvent::Starting { attempt: 2 }));
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn gives_up_after_too_many_failures() {
    let (path, dir) = launcher("always", "always-crash");
    let mut opts = fast_options();
    opts.backoff = Backoff::new(
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::from_secs(60),
        2,
    );
    let (mut sup, rx) = Supervisor::start(config(path), opts).unwrap();
    let seen = collect_until(&rx, Duration::from_secs(8), |e| *e == AirplayEvent::Stopped);
    assert!(seen.iter().any(|e| matches!(e, AirplayEvent::Failed(_))));
    let starts = seen
        .iter()
        .filter(|e| matches!(e, AirplayEvent::Starting { .. }))
        .count();
    assert_eq!(starts, 3); // first start plus two restarts
    assert!(
        seen.iter()
            .any(|e| matches!(e, AirplayEvent::Error(m) if m.contains("fake failure")))
    );
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn missing_program_is_reported() {
    let cfg = config(PathBuf::from("/nonexistent/uxplay-does-not-exist"));
    let (mut sup, rx) = Supervisor::start(cfg, fast_options()).unwrap();
    let seen = collect_until(&rx, Duration::from_secs(5), |e| *e == AirplayEvent::Stopped);
    assert!(
        seen.iter().any(|e| matches!(e, AirplayEvent::Failed(_))),
        "{seen:?}"
    );
    sup.stop();
}

#[test]
fn kills_a_process_that_ignores_sigterm() {
    let (path, dir) = launcher("stubborn", "ignore-term");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, Duration::from_secs(5), |e| {
        *e == AirplayEvent::ClientDisconnected
    });
    let t = Instant::now();
    let rest = stop_and_get_exit(&mut sup, &rx);
    let took = t.elapsed();
    assert!(
        took >= Duration::from_millis(400),
        "was not given the timeout: {took:?}"
    );
    assert!(took < Duration::from_secs(3), "stop took {took:?}");
    let Some(AirplayEvent::ProcessExited { pid, .. }) = rest
        .iter()
        .find(|e| matches!(e, AirplayEvent::ProcessExited { .. }))
        .cloned()
    else {
        panic!("no ProcessExited in {rest:?}");
    };
    assert!(is_reaped(pid.unwrap()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn planned_restart_starts_again_at_once() {
    let (path, dir) = launcher("restart", "normal");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, Duration::from_secs(5), |e| {
        *e == AirplayEvent::ServerStarted
    });
    sup.restart();
    let seen = collect_until(&rx, Duration::from_secs(5), |e| {
        matches!(e, AirplayEvent::Starting { attempt: 2 })
    });
    assert!(
        seen.iter()
            .any(|e| matches!(e, AirplayEvent::ProcessExited { expected: true, .. }))
    );
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AirplayEvent::Restarting { .. }))
    );
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

struct DenyAll;
impl ConfirmHook for DenyAll {
    fn confirm(&self, _r: &ConfirmRequest) -> Decision {
        Decision::Deny
    }
}

#[test]
fn denied_pin_restarts_uxplay() {
    let (path, dir) = launcher("deny", "normal");
    let mut opts = fast_options();
    opts.confirm = Some(Arc::new(DenyAll));
    let (mut sup, rx) = Supervisor::start(config(path), opts).unwrap();
    let seen = collect_until(&rx, Duration::from_secs(8), |e| {
        matches!(e, AirplayEvent::Starting { attempt: 2 })
    });
    assert!(seen.contains(&AirplayEvent::Denied));
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn dropping_the_supervisor_stops_the_process() {
    let (path, dir) = launcher("drop", "normal");
    let (sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, Duration::from_secs(5), |e| {
        *e == AirplayEvent::ServerStarted
    });
    drop(sup);
    let rest: Vec<_> = rx.try_iter().collect();
    assert_eq!(rest.last(), Some(&AirplayEvent::Stopped));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn long_lines_and_junk_do_not_break_the_reader() {
    let (path, dir) = launcher("chatty", "chatty");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, Duration::from_secs(5), |e| {
        matches!(e, AirplayEvent::PinShown { .. })
    });
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

/// Real UxPlay, only when asked for: `COMETCAST_REAL_UXPLAY=1 cargo test -p cc-airplay
/// real_uxplay -- --nocapture`. Needs the `uxplay` program, GStreamer plugins, a running
/// Avahi/DNS-SD service and an X display (for example Xvfb with DISPLAY set).
/// The test plays the part of an iPhone with one raw request ("PAIR-PIN-START") so that
/// UxPlay prints a PIN.
#[test]
fn real_uxplay_shows_pin_through_the_pty() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    if std::env::var("COMETCAST_REAL_UXPLAY").as_deref() != Ok("1") {
        eprintln!("skipped: set COMETCAST_REAL_UXPLAY=1 to run against the real uxplay");
        return;
    }
    let cfg = AirplayConfig {
        name: "CC smoke test".into(),
        fullscreen: false,
        video_sink: Some("fakesink".into()),
        audio_off: true,
        ..AirplayConfig::default()
    };
    let (mut sup, rx) = Supervisor::start(cfg, SupervisorOptions::default()).unwrap();
    collect_until(&rx, Duration::from_secs(10), |e| {
        *e == AirplayEvent::ServerStarted
    });
    let asked = Instant::now();
    let mut s = TcpStream::connect("127.0.0.1:7000").unwrap();
    s.write_all(b"POST /pair-pin-start RTSP/1.0\r\nCSeq: 0\r\nContent-Length: 0\r\n\r\n")
        .unwrap();
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 200];
    let _ = s.read(&mut buf);
    let seen = collect_until(&rx, Duration::from_secs(5), |e| {
        matches!(e, AirplayEvent::PinShown { .. })
    });
    let delay = asked.elapsed();
    eprintln!("PIN event after {delay:?}: {seen:?}");
    assert!(
        delay < Duration::from_secs(4),
        "PIN arrived late: {delay:?}"
    );
    let rest = stop_and_get_exit(&mut sup, &rx);
    eprintln!("after stop: {rest:?}");
    let Some(AirplayEvent::ProcessExited { pid, .. }) = rest
        .iter()
        .find(|e| matches!(e, AirplayEvent::ProcessExited { .. }))
        .cloned()
    else {
        panic!("no ProcessExited in {rest:?}");
    };
    assert!(is_reaped(pid.unwrap()));
}
