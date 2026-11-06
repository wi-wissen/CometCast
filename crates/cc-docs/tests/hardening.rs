// SPDX-License-Identifier: GPL-3.0-or-later
//! What the worker really does to itself, and what the parent does around it.
//!
//! The in-worker checks run in a **child copy of this test program** that calls the same
//! function the worker calls (`enter_worker_sandbox_for_tests`) and then tries things the
//! renderer must not be able to do. Parts that the kernel does not offer (Landlock, seccomp)
//! print a note and skip.

use std::process::Command;
use std::time::Duration;

use cc_docs::{DocError, Document, Isolation, Limits, Protection, WorkerSpec};

const CHILD_ENV: &str = "CC_DOCS_HARDENING_CHILD";

fn worker() -> Isolation {
    Isolation::Process(WorkerSpec::new(env!("CARGO_BIN_EXE_cc-docs-worker")))
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn open_good(limits: Limits) -> Result<Document, DocError> {
    Document::open_with_options(data("classroom.pdf"), limits, worker())
}

/// The child half: runs only when the parent test started this program for it.
#[test]
fn probe_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let applied = cc_docs::enter_worker_sandbox_for_tests(&Limits::default())
        .map(|v| v.iter().map(|p| p.name()).collect::<Vec<_>>().join(","))
        .unwrap_or_else(|e| format!("ERR {e}"));
    let verdict = |ok: bool| if ok { "ALLOWED" } else { "denied" };
    // Plain `println!` goes to fd 1, which the filter allows.
    println!("PROBE applied={applied}");
    println!(
        "PROBE open={}",
        verdict(std::fs::File::open("/etc/passwd").is_ok())
    );
    println!("PROBE readdir={}", verdict(std::fs::read_dir("/").is_ok()));
    println!(
        "PROBE create={}",
        verdict(std::fs::File::create("/tmp/cc-docs-probe-file").is_ok())
    );
    println!(
        "PROBE tcp={}",
        verdict(std::net::TcpStream::connect(("127.0.0.1", 9)).is_ok())
    );
    println!(
        "PROBE tcp_listen={}",
        verdict(std::net::TcpListener::bind(("127.0.0.1", 0)).is_ok())
    );
    println!(
        "PROBE udp={}",
        verdict(std::net::UdpSocket::bind(("127.0.0.1", 0)).is_ok())
    );
    println!(
        "PROBE fork_exec={}",
        verdict(Command::new("/bin/true").spawn().is_ok())
    );
    println!(
        "PROBE thread={}",
        verdict(std::thread::spawn(|| 40 + 2).join().is_ok_and(|v| v == 42))
    );
    // Memory still works.
    let big = vec![1u8; 32 << 20];
    println!("PROBE alloc={}", verdict(big.iter().all(|b| *b == 1)));
}

fn run_probe() -> std::collections::HashMap<String, String> {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "probe_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .output()
        .expect("start the probe");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    println!("probe output:\n{text}");
    assert!(
        out.status.success(),
        "probe failed: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    text.lines()
        .filter_map(|l| l[l.find("PROBE ")? + 6..].split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn inside_the_worker_files_sockets_and_processes_are_closed() {
    if !cfg!(target_os = "linux") {
        println!("NOTE: skipped, the in-worker sandbox is Linux only");
        return;
    }
    let p = run_probe();
    let applied = &p["applied"];
    println!("applied protections: {applied}");
    // The resource limits are mandatory.
    for must in [
        "memory-limit",
        "cpu-limit",
        "no-core-dumps",
        "no-file-writes",
        "fd-limit",
    ] {
        assert!(applied.contains(must), "{must} missing in {applied}");
    }
    assert_eq!(p["alloc"], "ALLOWED");
    assert_eq!(p["thread"], "ALLOWED", "threads must still work");
    if applied.contains("landlock") {
        for k in ["open", "readdir", "create"] {
            assert_eq!(p[k], "denied", "{k} under Landlock");
        }
    } else {
        println!("NOTE: Landlock is not available here; file access is not checked");
    }
    if applied.contains("landlock-network") {
        assert_eq!(p["tcp"], "denied");
        assert_eq!(p["tcp_listen"], "denied");
    }
    if applied.contains("seccomp") {
        for k in ["tcp", "tcp_listen", "udp", "fork_exec", "open", "create"] {
            assert_eq!(p[k], "denied", "{k} under seccomp");
        }
    } else {
        println!("NOTE: seccomp is not available here; sockets and fork are not checked");
    }
}

#[test]
fn protections_are_reported_to_the_caller() {
    if !cfg!(unix) {
        println!("NOTE: skipped");
        return;
    }
    let d = open_good(Limits::default()).expect("opens");
    let got = d.protections();
    let names: Vec<_> = got.iter().map(|p| p.name()).collect();
    println!("protections the worker reported: {names:?}");
    assert!(d.is_isolated());
    for p in [
        Protection::ProcessGroupKill,
        Protection::CleanEnvironment,
        Protection::CpuLimit,
        Protection::NoCoreDumps,
        Protection::NoFileWrites,
        Protection::FdLimit,
    ] {
        assert!(got.contains(&p), "{p:?} missing in {names:?}");
    }
    if cfg!(any(target_os = "linux", target_os = "android")) {
        assert!(got.contains(&Protection::MemoryLimit));
    }
    // A render under the full sandbox still works and reports the same.
    let img = d.render_page(0, 400).expect("renders under the sandbox");
    assert_eq!(img.width, 400);
    // (The memory watchdog bit depends on whether the parent got to poll before the exit.)
    let strip = |v: Vec<Protection>| -> Vec<Protection> {
        v.into_iter()
            .filter(|p| *p != Protection::MemoryWatchdog)
            .collect()
    };
    assert_eq!(strip(d.protections()), strip(got));
    // The in-process engine has none.
    let t =
        Document::open_with_options(data("minimal.pdf"), Limits::default(), Isolation::InProcess)
            .unwrap();
    assert!(!t.is_isolated());
    assert!(t.protections().is_empty());
}

#[test]
fn require_os_sandbox_follows_what_is_available() {
    if !cfg!(unix) {
        return;
    }
    let normal = open_good(Limits::default()).unwrap();
    let both = normal.protections().contains(&Protection::Landlock)
        && normal.protections().contains(&Protection::Seccomp);
    let strict = Limits {
        require_os_sandbox: true,
        ..Limits::default()
    };
    match open_good(strict) {
        Ok(d) => {
            assert!(both, "strict mode opened although not both are available");
            assert!(d.protections().contains(&Protection::Seccomp));
        }
        Err(e) => {
            assert!(
                !both,
                "strict mode refused although both are available: {e:?}"
            );
            assert!(matches!(e, DocError::SandboxUnavailable(_)), "{e:?}");
            println!("NOTE: Landlock and seccomp are not both available here; strict mode refused");
        }
    }
}

/// The worker is fed hostile structure; the sandbox must not break normal parsing of any of
/// the generated files (a denied syscall would show as a failed render).
#[test]
fn all_test_files_still_render_under_the_sandbox() {
    for name in ["minimal.pdf", "classroom.pdf", "rotated_pages.pdf"] {
        let d = Document::open_with_options(data(name), Limits::default(), worker())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        for i in 0..d.page_count() {
            let img = d
                .render_page(i, 800)
                .unwrap_or_else(|e| panic!("{name} p{i}: {e}"));
            assert_eq!(img.width, 800);
        }
    }
    let _ = Duration::from_secs(0);
}
