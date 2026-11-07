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

/// Deadline for every wait on an event. Waiting is done on events, so it ends as soon as the
/// event arrives; the deadline is only the point where a lost event becomes a failure. It is
/// generous because `cargo test --workspace` runs many busy test binaries at once.
const WAIT: Duration = Duration::from_secs(60);

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
            "#!/bin/sh\nFAKE_UXPLAY_MODE={mode} FAKE_UXPLAY_COUNTER={} FAKE_UXPLAY_HELPER_PID={} exec bash {FAKE} \"$@\"\n",
            counter.display(),
            dir.join("helper.pid").display()
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
        // Long: the limit is only reached when the output thread is stuck.
        reader_grace: Duration::from_secs(30),
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
    // Long stop timeout: a fake that obeys SIGTERM must end long before it, so the check
    // below ("did not need the kill") does not depend on how fast the machine is.
    let mut opts = fast_options();
    opts.stop_timeout = Duration::from_secs(30);
    let (mut sup, rx) = Supervisor::start(config(path), opts).unwrap();
    let seen = collect_until(&rx, WAIT, |e| *e == AirplayEvent::ClientDisconnected);
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
        t.elapsed() < Duration::from_secs(30),
        "stop took {:?}",
        t.elapsed()
    );
    let Some(AirplayEvent::ProcessExited {
        pid,
        expected,
        code,
        signal,
    }) = rest
        .iter()
        .find(|e| matches!(e, AirplayEvent::ProcessExited { .. }))
        .cloned()
    else {
        panic!("no ProcessExited in {rest:?}");
    };
    assert!(expected);
    assert_eq!(code, Some(0)); // the fake exits 0 on SIGTERM
    assert_eq!(signal, None);
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
    let seen = collect_until(&rx, WAIT, |e| matches!(e, AirplayEvent::PinShown { .. }));
    let died = seen
        .iter()
        .find_map(|e| match e {
            AirplayEvent::ProcessExited {
                expected: false,
                code,
                pid,
                ..
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
    let seen = collect_until(&rx, WAIT, |e| *e == AirplayEvent::Stopped);
    assert!(seen.iter().any(|e| matches!(e, AirplayEvent::Failed(_))));
    let starts = seen
        .iter()
        .filter(|e| matches!(e, AirplayEvent::Starting { .. }))
        .count();
    assert_eq!(starts, 2); // max_failures = 2: the 2nd failure in a row is the last
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
    let seen = collect_until(&rx, WAIT, |e| *e == AirplayEvent::Stopped);
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
    collect_until(&rx, WAIT, |e| *e == AirplayEvent::ClientDisconnected);
    let t = Instant::now();
    let rest = stop_and_get_exit(&mut sup, &rx);
    let took = t.elapsed();
    assert!(
        took >= Duration::from_millis(400),
        "was not given the timeout: {took:?}"
    );
    assert!(took < Duration::from_secs(30), "stop took {took:?}");
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
    collect_until(&rx, WAIT, |e| *e == AirplayEvent::ServerStarted);
    sup.restart();
    let seen = collect_until(&rx, WAIT, |e| {
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
    let seen = collect_until(&rx, WAIT, |e| {
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
    collect_until(&rx, WAIT, |e| *e == AirplayEvent::ServerStarted);
    drop(sup);
    let rest: Vec<_> = rx.try_iter().collect();
    assert_eq!(rest.last(), Some(&AirplayEvent::Stopped));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn long_lines_and_junk_do_not_break_the_reader() {
    let (path, dir) = launcher("chatty", "chatty");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, WAIT, |e| matches!(e, AirplayEvent::PinShown { .. }));
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

/// Pids written by the fake's background helpers, one per line.
fn helper_pids(dir: &std::path::Path) -> Vec<u32> {
    fs::read_to_string(dir.join("helper.pid"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

/// True if the process does not exist or is only a zombie of someone else's child
/// (an orphan that its new parent has not collected yet is dead for our purpose).
fn is_dead(pid: u32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => stat
            .rsplit(')')
            .next()
            .is_some_and(|rest| rest.trim_start().starts_with('Z')),
    }
}

fn wait_dead(pid: u32) -> bool {
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end {
        if is_dead(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn helper_processes_do_not_survive_a_stop() {
    let (path, dir) = launcher("helper", "helper");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, WAIT, |e| *e == AirplayEvent::ServerStarted);
    let pids = helper_pids(&dir);
    assert_eq!(pids.len(), 1, "fake did not start its helper");
    assert!(!is_dead(pids[0]), "helper should be alive while running");
    sup.stop();
    assert!(wait_dead(pids[0]), "helper {} survived the stop", pids[0]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn helper_processes_do_not_survive_a_crash() {
    let (path, dir) = launcher("helpercrash", "helper-crash");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    collect_until(&rx, WAIT, |e| matches!(e, AirplayEvent::Restarting { .. }));
    let first = helper_pids(&dir)[0];
    assert!(wait_dead(first), "helper {first} survived the crash");
    sup.stop();
    for pid in helper_pids(&dir) {
        assert!(wait_dead(pid), "helper {pid} survived");
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_pin_line_without_pairing_line_is_not_believed() {
    let (path, dir) = launcher("spoof", "spoof");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, WAIT, |e| matches!(e, AirplayEvent::PinShown { .. }));
    // The first PIN line (1111) came before any pairing line and must be dropped.
    let pins: Vec<_> = seen
        .iter()
        .filter_map(|e| match e {
            AirplayEvent::PinShown { pin } => Some(pin.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(pins, ["9876"]);
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

/// A dialog that counts its calls and stays open until released.
struct SlowHook {
    calls: std::sync::atomic::AtomicUsize,
    release: std::sync::Mutex<Receiver<Decision>>,
}
impl ConfirmHook for SlowHook {
    fn confirm(&self, _r: &ConfirmRequest) -> Decision {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.release
            .lock()
            .unwrap()
            .recv_timeout(WAIT)
            .unwrap_or(Decision::Accept)
    }
}

fn slow_hook() -> (Arc<SlowHook>, std::sync::mpsc::Sender<Decision>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let hook = Arc::new(SlowHook {
        calls: std::sync::atomic::AtomicUsize::new(0),
        release: std::sync::Mutex::new(rx),
    });
    (hook, tx)
}

#[test]
fn only_one_confirm_dialog_is_open_at_a_time() {
    let (path, dir) = launcher("twopins", "twopins");
    let (hook, release) = slow_hook();
    let mut opts = fast_options();
    opts.confirm = Some(hook.clone());
    let (mut sup, rx) = Supervisor::start(config(path), opts).unwrap();
    // Wait for both PINs (9876 and 5432).
    let mut pins = 0;
    collect_until(&rx, WAIT, |e| {
        if matches!(e, AirplayEvent::PinShown { .. }) {
            pins += 1;
        }
        pins == 2
    });
    assert_eq!(hook.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    release.send(Decision::Accept).unwrap();
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn no_dialog_and_no_restart_while_a_session_is_active() {
    let (path, dir) = launcher("active", "normal");
    let (hook, _release) = slow_hook();
    let mut opts = fast_options();
    opts.confirm = Some(hook.clone());
    let (mut sup, rx) = Supervisor::start(config(path), opts).unwrap();
    sup.set_session_active(true);
    // The fake prints "Open connections: 0" after the PIN, so once ClientDisconnected is
    // seen the reader has handled the PIN line and nothing of it can still be on its way.
    collect_until(&rx, WAIT, |e| *e == AirplayEvent::ClientDisconnected);
    std::thread::sleep(Duration::from_millis(300)); // extra grace for the negative check
    assert_eq!(hook.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!rx.try_iter().any(|e| matches!(
        e,
        AirplayEvent::Denied | AirplayEvent::Starting { attempt: 2 }
    )));
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn deny_after_a_session_started_is_ignored() {
    let (path, dir) = launcher("lateden", "normal");
    let (hook, release) = slow_hook();
    let mut opts = fast_options();
    opts.confirm = Some(hook.clone());
    let (mut sup, rx) = Supervisor::start(config(path), opts).unwrap();
    collect_until(&rx, WAIT, |e| matches!(e, AirplayEvent::PinShown { .. }));
    sup.set_session_active(true); // dialog is open, now a session starts
    release.send(Decision::Deny).unwrap();
    let seen = collect_until(&rx, WAIT, |e| *e == AirplayEvent::DenyIgnored);
    assert!(!seen.contains(&AirplayEvent::Denied));
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !rx.try_iter()
            .any(|e| matches!(e, AirplayEvent::Starting { attempt: 2 }))
    );
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn death_by_signal_is_reported_as_signal() {
    let (path, dir) = launcher("sigkill", "self-kill");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, WAIT, |e| {
        matches!(e, AirplayEvent::ProcessExited { .. })
    });
    let Some(AirplayEvent::ProcessExited { code, signal, .. }) = seen.last() else {
        panic!("{seen:?}");
    };
    assert_eq!(*code, None);
    assert!(signal.is_some());
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}

// --- the trusted "mirroring" signal (native-apps.md 21.4) ---------------------------------

#[test]
fn mirroring_start_comes_from_uxplays_line_after_a_pin() {
    let (path, dir) = launcher("mirror", "mirror");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, WAIT, |e| {
        *e == AirplayEvent::MirroringChanged { active: true }
    });
    let pos = |want: &AirplayEvent| seen.iter().position(|e| e == want).unwrap();
    assert!(
        pos(&AirplayEvent::PinShown { pin: "9876".into() })
            < pos(&AirplayEvent::MirroringChanged { active: true })
    );
    // The process ends (stop): the mirroring ends with it, before the exit is reported.
    let rest = stop_and_get_exit(&mut sup, &rx);
    let end = rest
        .iter()
        .position(|e| *e == AirplayEvent::MirroringChanged { active: false })
        .expect("no end of the mirroring");
    let exit = rest
        .iter()
        .position(|e| matches!(e, AirplayEvent::ProcessExited { .. }))
        .unwrap();
    assert!(end < exit, "{rest:?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mirroring_ends_when_the_last_connection_closes() {
    let (path, dir) = launcher("mirror-ends", "mirror-ends");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, WAIT, |e| {
        *e == AirplayEvent::MirroringChanged { active: false }
    });
    let started = seen
        .iter()
        .filter(|e| **e == AirplayEvent::MirroringChanged { active: true })
        .count();
    assert_eq!(started, 1, "{seen:?}");
    sup.stop();
    // No second end when the process stops afterwards.
    let mut more = Vec::new();
    while let Ok(e) = rx.try_recv() {
        more.push(e);
    }
    assert!(
        !more.contains(&AirplayEvent::MirroringChanged { active: false }),
        "{more:?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_mirror_line_without_a_pin_is_not_believed() {
    let (path, dir) = launcher("mirror-spoof", "mirror-spoof");
    let (mut sup, rx) = Supervisor::start(config(path), fast_options()).unwrap();
    let seen = collect_until(&rx, WAIT, |e| *e == AirplayEvent::ClientDisconnected);
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AirplayEvent::MirroringChanged { .. })),
        "{seen:?}"
    );
    sup.stop();
    let _ = fs::remove_dir_all(dir);
}
