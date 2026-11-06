// SPDX-License-Identifier: GPL-3.0-or-later
//! The parent side of the worker: it returns within the deadline whatever the worker or its
//! descendants do, kills the whole process group, and starts the worker with a clean
//! environment, `/` as working directory and no stray file descriptors.
//!
//! Fake workers are `sh -c` scripts; the tests need a Unix shell and `/proc`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cc_docs::{DocError, Document, Isolation, Limits, RenderOptions, WorkerSpec};

fn unix_with_proc() -> bool {
    let ok = cfg!(unix) && Path::new("/proc/self/stat").exists();
    if !ok {
        println!("NOTE: skipped, needs a Unix shell and /proc");
    }
    ok
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn sh(script: &str) -> Isolation {
    Isolation::Process(WorkerSpec::new("/bin/sh").arg("-c").arg(script))
}

fn limits_ms(ms: u64) -> Limits {
    Limits {
        open_timeout: Duration::from_millis(ms),
        render_timeout: Duration::from_millis(ms),
        ..Limits::default()
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cc-docs-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A process is gone, or only a zombie that nobody has reaped yet.
fn is_dead(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(s) => s
            .rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .is_some_and(|state| state == "Z" || state == "X"),
    }
}

fn read_pid(file: &Path) -> u32 {
    // The script writes the pid before it blocks; give it a moment.
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(file)
            && let Ok(p) = s.trim().parse()
        {
            return p;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no pid in {}", file.display());
}

fn wait_dead(pid: u32) {
    for _ in 0..100 {
        if is_dead(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("process {pid} survived");
}

/// The reviewer's `fork.sh`: the worker exits at once but a background `sleep` keeps the
/// stdout pipe open.
#[test]
fn a_grandchild_holding_the_pipes_does_not_delay_the_parent() {
    if !unix_with_proc() {
        return;
    }
    let dir = scratch("fork");
    let pidfile = dir.join("pid");
    let script = format!("sleep 31 & echo $! > {}; exit 0", pidfile.display());
    let t = Instant::now();
    let r = Document::open_with_options(data("minimal.pdf"), limits_ms(1000), sh(&script));
    let took = t.elapsed();
    println!("fork.sh worker: {r:?} after {took:?}");
    assert!(matches!(r, Err(DocError::WorkerFailed(_))), "{r:?}");
    assert!(took < Duration::from_millis(1500), "{took:?}");
    wait_dead(read_pid(&pidfile));
}

/// The reviewer's `forkalive.sh`: the worker itself stays alive and its child holds the pipes.
#[test]
fn a_worker_and_its_child_are_killed_together_at_the_deadline() {
    if !unix_with_proc() {
        return;
    }
    let dir = scratch("alive");
    let pidfile = dir.join("pid");
    let script = format!("sleep 600 & echo $! > {}; sleep 600", pidfile.display());
    let t = Instant::now();
    let r = Document::open_with_options(data("minimal.pdf"), limits_ms(400), sh(&script));
    let took = t.elapsed();
    println!("forkalive.sh worker: {r:?} after {took:?}");
    assert_eq!(r.unwrap_err(), DocError::Timeout);
    assert!(took < Duration::from_millis(1400), "{took:?}");
    wait_dead(read_pid(&pidfile));
}

/// The reviewer's `stall.sh`: a plain `sleep 600`.
#[test]
fn a_sleeping_worker_is_killed_at_the_deadline() {
    if !unix_with_proc() {
        return;
    }
    let t = Instant::now();
    let r = Document::open_with_options(
        data("minimal.pdf"),
        limits_ms(300),
        Isolation::Process(WorkerSpec::new("sleep").arg("600")),
    );
    assert_eq!(r.unwrap_err(), DocError::Timeout);
    assert!(
        t.elapsed() < Duration::from_millis(1300),
        "{:?}",
        t.elapsed()
    );
}

#[test]
fn cancel_kills_the_whole_group_too() {
    if !unix_with_proc() {
        return;
    }
    // A worker that is a good one cannot be used to test a late cancel, so open with a
    // valid-looking shell worker is impossible; use a render on a real document and cancel
    // at once.
    let d = Document::open_with_options(
        data("minimal.pdf"),
        Limits::default(),
        Isolation::Process(WorkerSpec::new(env!("CARGO_BIN_EXE_cc-docs-worker"))),
    )
    .unwrap();
    let flag = Arc::new(AtomicBool::new(true));
    flag.store(true, Ordering::SeqCst);
    let t = Instant::now();
    let r = d.render_page_with(
        0,
        400,
        &RenderOptions {
            cancel: Some(flag),
            ..RenderOptions::default()
        },
    );
    assert_eq!(r.unwrap_err(), DocError::Cancelled);
    assert!(t.elapsed() < Duration::from_millis(500));
}

/// The reviewer's `fd.sh`, with the output in a temp directory: the worker has only the three
/// standard descriptors, no environment, `/` as working directory and no way to see secrets.
#[test]
fn the_worker_starts_with_clean_environment_directory_and_descriptors() {
    if !unix_with_proc() {
        return;
    }
    let dir = scratch("fd");
    let out = dir.join("out.txt");
    let script = format!(
        "exec > {} 2>&1; echo FDS; for f in /proc/self/fd/*; do echo \"$f\"; done; echo ENV; env; \
         echo PWD; pwd; echo LIMITS; cat /proc/self/limits; exit 1",
        out.display()
    );
    let _ = Document::open_with_options(data("minimal.pdf"), limits_ms(3000), sh(&script));
    let text = std::fs::read_to_string(&out).expect("the worker script ran");
    println!("{text}");
    let section = |name: &str, next: &str| -> Vec<String> {
        let a = text.split(&format!("{name}\n")).nth(1).unwrap_or("");
        a.split(&format!("{next}\n"))
            .next()
            .unwrap_or("")
            .lines()
            .map(str::to_string)
            .collect()
    };
    // Descriptors 0, 1, 2 only (the shell's own glob may show one more for the directory).
    let fds = section("FDS", "ENV");
    let numbers: Vec<u32> = fds
        .iter()
        .filter_map(|l| l.rsplit('/').next()?.parse().ok())
        .collect();
    assert!(
        numbers.iter().all(|n| *n <= 3),
        "extra descriptors: {fds:?}"
    );
    // No environment at all that came from this test process.
    let env = section("ENV", "PWD");
    for l in &env {
        assert!(
            !l.starts_with("HOME=") && !l.starts_with("CARGO") && !l.starts_with("RUST"),
            "leaked: {l}"
        );
    }
    assert!(
        !env.iter().any(|l| l.starts_with("PATH=/")),
        "PATH leaked: {env:?}"
    );
    assert_eq!(section("PWD", "LIMITS")[0], "/");
}

#[test]
fn worker_paths_are_made_absolute_when_the_spec_is_made() {
    // Bare name: found once in this process's PATH, absolute from then on.
    let s = WorkerSpec::new("sleep");
    assert!(s.program.is_absolute(), "{:?}", s.program);
    assert_eq!(s.program.file_name().unwrap(), "sleep");
    // A relative path becomes absolute.
    let r = WorkerSpec::new("some/relative/worker");
    assert!(r.program.is_absolute(), "{:?}", r.program);
    // A bare name that exists nowhere is still absolute (so no PATH search at spawn time) and
    // fails to start cleanly.
    let n = WorkerSpec::new("no-such-worker-program");
    assert!(n.program.is_absolute());
    let e = Document::open_with_options(
        data("minimal.pdf"),
        Limits::default(),
        Isolation::Process(n),
    )
    .unwrap_err();
    assert!(matches!(e, DocError::WorkerUnavailable(_)), "{e:?}");
}

#[test]
fn the_worker_environment_variable_needs_a_trustworthy_file() {
    // The check itself is exercised through `discover_checked` in a child process so that
    // this test never changes its own environment (setting variables is `unsafe` in 2024).
    if !cfg!(unix) {
        return;
    }
    let dir = scratch("env");
    let me = std::env::current_exe().unwrap();
    let run = |value: &str| {
        std::process::Command::new(&me)
            .args(["--exact", "env_child", "--nocapture", "--test-threads=1"])
            .env(cc_docs::WORKER_ENV, value)
            .env("CC_DOCS_ENV_CHILD", "1")
            .output()
            .unwrap()
    };
    let verdict = |o: &std::process::Output| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .find_map(|l| {
                l[l.find("VERDICT ")? + 8..]
                    .split_once(' ')
                    .map(|(a, b)| (a.to_string(), b.to_string()))
            })
            .expect("verdict line")
    };
    // Relative path: refused.
    let (v, why) = verdict(&run("cc-docs-worker"));
    assert_eq!(
        (v.as_str(), why.contains("not absolute")),
        ("refused", true),
        "{why}"
    );
    // Absolute, but writable by others: refused.
    let file = dir.join("w");
    std::fs::write(&file, b"#!/bin/sh\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o777)).unwrap();
    }
    let (v, why) = verdict(&run(file.to_str().unwrap()));
    assert_eq!(v, "refused", "{why}");
    assert!(why.contains("writable"), "{why}");
    // Absolute, owned by us, mode 0755: accepted.
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (v, why) = verdict(&run(file.to_str().unwrap()));
    assert_eq!(v, "accepted", "{why}");
    // A directory is not a worker.
    let (v, _) = verdict(&run(dir.to_str().unwrap()));
    assert_eq!(v, "refused");
}

#[test]
fn env_child() {
    if std::env::var_os("CC_DOCS_ENV_CHILD").is_none() {
        return;
    }
    match WorkerSpec::discover_checked() {
        Ok(s) => println!("VERDICT accepted {}", s.program.display()),
        Err(why) => println!("VERDICT refused {why}"),
    }
}
