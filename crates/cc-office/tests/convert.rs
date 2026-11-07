// SPDX-License-Identifier: GPL-3.0-or-later
//! Conversion through the real worker process, with generated fixtures.
//!
#![allow(missing_docs)]

mod common {
    pub mod zipwriter;
}
use common::zipwriter as zw;

use std::path::PathBuf;
use std::time::Duration;

use cc_office::{
    Limits, OfficeError, OfficeFormat, Options, WorkerSpec, convert_bytes, convert_file,
};

fn worker() -> WorkerSpec {
    WorkerSpec::new(env!("CARGO_BIN_EXE_cc-office-worker"))
}

fn opts() -> Options {
    Options {
        worker: Some(worker()),
        limits: Limits {
            timeout: Duration::from_secs(90),
            ..Limits::default()
        },
    }
}

fn assert_pdf(c: &cc_office::Converted) {
    assert!(c.pdf.starts_with(b"%PDF-"));
    assert!(c.pdf.len() > 500, "suspiciously small PDF: {}", c.pdf.len());
}

fn fixtures() -> Vec<(OfficeFormat, Vec<u8>)> {
    vec![
        (OfficeFormat::Docx, zw::docx("Hello classroom")),
        (OfficeFormat::Xlsx, zw::xlsx("Hello sheet")),
        (OfficeFormat::Pptx, zw::pptx("Hello slide")),
    ]
}

#[cfg(feature = "engine-rust")]
#[test]
fn rust_engine_converts_the_generated_fixtures() {
    for (f, bytes) in fixtures() {
        let r = convert_bytes(&bytes, f, &opts());
        let c = match r {
            Ok(c) => c,
            Err(e) => panic!("{f:?}: {e}"),
        };
        assert_pdf(&c);
        eprintln!(
            "rust engine {f:?}: {} bytes in {:?}, protections {:?}, warnings {:?}",
            c.pdf.len(),
            c.elapsed,
            c.protections,
            c.warnings
        );
        // Resource limits are mandatory in the worker, so they must have been reported.
        #[cfg(unix)]
        for p in ["rlimit-cpu", "rlimit-core", "rlimit-fsize"] {
            assert!(c.protections.iter().any(|x| x == p), "{p} missing");
        }
    }
}

#[test]
fn convert_file_reads_from_disk_and_checks_the_extension() {
    let dir = std::env::temp_dir().join(format!("cc-office-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p: PathBuf = dir.join("Klassenarbeit with spaces.DOCX");
    std::fs::write(&p, zw::docx("x")).unwrap();
    // A .docx that is really an xlsx is refused before any engine runs.
    let wrong = dir.join("wrong.docx");
    std::fs::write(&wrong, zw::xlsx("x")).unwrap();
    assert!(matches!(
        convert_file(&wrong, &opts()),
        Err(OfficeError::BadContainer(_))
    ));
    let c = convert_file(&p, &opts()).expect("convert");
    assert_pdf(&c);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_worker_is_an_error_not_a_panic() {
    let o = Options {
        worker: Some(WorkerSpec::new("/nonexistent/cc-office-worker")),
        ..Options::default()
    };
    assert!(matches!(
        convert_bytes(&zw::docx("x"), OfficeFormat::Docx, &o),
        Err(OfficeError::WorkerUnavailable(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_hanging_worker_is_killed_at_the_deadline_with_its_children() {
    // A fake "worker" that ignores its arguments, starts a grandchild (a background `sleep`,
    // in the same process group), records the grandchild's pid in a file and sleeps itself.
    let dir = std::env::temp_dir().join(format!("cc-office-hang-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("hang.sh");
    let pidfile = dir.join("grandchild.pid");
    let _ = std::fs::remove_file(&pidfile);
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nsleep 600 &\necho $! > '{}.tmp'\nmv '{}.tmp' '{}'\nsleep 600\n",
            pidfile.display(),
            pidfile.display(),
            pidfile.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let o = Options {
        worker: Some(WorkerSpec::new(&script)),
        limits: Limits {
            timeout: Duration::from_secs(1),
            ..Limits::default()
        },
    };
    let t = std::time::Instant::now();
    let r = convert_bytes(&zw::docx("x"), OfficeFormat::Docx, &o);
    assert!(matches!(r, Err(OfficeError::Timeout(_))), "{r:?}");
    assert!(
        t.elapsed() < Duration::from_secs(10),
        "took {:?}",
        t.elapsed()
    );
    // The grandchild must be gone too. Zombies count as gone (nobody may have reaped the
    // orphan yet); it is given a moment to die.
    let pid: i32 = std::fs::read_to_string(&pidfile)
        .expect("the fake worker never recorded its grandchild")
        .trim()
        .parse()
        .expect("pid file content");
    let mut alive = process_alive(pid);
    for _ in 0..50 {
        if !alive {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        alive = process_alive(pid);
    }
    if alive {
        // Do not leave a 600 s sleep behind when the assertion fails.
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
    }
    assert!(!alive, "grandchild {pid} survived the group kill");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether a process exists and is not a zombie.
#[cfg(unix)]
fn process_alive(pid: i32) -> bool {
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        // "pid (comm) S ...": the state follows the last ')'.
        return stat
            .rsplit_once(") ")
            .and_then(|(_, rest)| rest.chars().next())
            .is_some_and(|state| state != 'Z' && state != 'X');
    }
    // No /proc (macOS): signal 0 only tests for existence.
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

#[cfg(unix)]
#[test]
fn a_worker_that_writes_a_fake_or_huge_result_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("cc-office-fake-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The fake worker finds its work dir in the `--work` argument (after the 8th word).
    let mk = |name: &str, body: &str| {
        let s = dir.join(name);
        let script = format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = \"--work\" ] && W=\"$2\"; shift; done\n{body}\n"
        );
        std::fs::write(&s, script).unwrap();
        std::fs::set_permissions(&s, std::fs::Permissions::from_mode(0o755)).unwrap();
        s
    };
    let run = |s: &PathBuf, cap: u64| {
        let o = Options {
            worker: Some(WorkerSpec::new(s)),
            limits: Limits {
                max_output_bytes: cap,
                ..Limits::default()
            },
        };
        convert_bytes(&zw::docx("x"), OfficeFormat::Docx, &o)
    };
    let not_pdf = mk("notpdf.sh", "echo hello > \"$W/out/input.pdf\"");
    assert!(matches!(
        run(&not_pdf, 1 << 20),
        Err(OfficeError::BadOutput(_))
    ));
    let nothing = mk("nothing.sh", "true");
    assert!(matches!(
        run(&nothing, 1 << 20),
        Err(OfficeError::BadOutput(_))
    ));
    let big = mk(
        "big.sh",
        "printf '%%PDF-1.7\\n' > \"$W/out/input.pdf\"; head -c 5000 /dev/zero >> \"$W/out/input.pdf\"; printf '%%%%EOF' >> \"$W/out/input.pdf\"",
    );
    assert!(matches!(run(&big, 1000), Err(OfficeError::BadOutput(m)) if m.contains("larger")));
    let failing = mk("fail.sh", "echo boom >&2; exit 3");
    assert!(
        matches!(run(&failing, 1 << 20), Err(OfficeError::ConversionFailed(m)) if m.contains("boom"))
    );
    // Terminal escape sequences in warnings and in stderr never reach the caller.
    let noisy_ok = mk(
        "noisy.sh",
        "printf 'warning:\\033[31mred\\033]0;title\\007 text\\n'; printf 'warning:\\2331mC1\\n'; printf 'protection:rlimit-cpu\\nprotection:\\033[2J\\n'; printf '%%PDF-1.7\\n%%%%EOF\\n' > \"$W/out/input.pdf\"",
    );
    let c = run(&noisy_ok, 1 << 20).expect("noisy but valid worker");
    assert!(!c.warnings.is_empty());
    for w in &c.warnings {
        assert!(
            !w.chars().any(char::is_control),
            "control character in {w:?}"
        );
    }
    assert_eq!(c.warnings[0], "[31mred]0;title text");
    assert_eq!(c.protections, vec!["rlimit-cpu".to_string()]);
    let noisy_fail = mk("noisyfail.sh", "printf 'bo\\033[2Jom\\007\\n' >&2; exit 3");
    match run(&noisy_fail, 1 << 20) {
        Err(OfficeError::ConversionFailed(m)) => {
            assert!(
                !m.chars().any(char::is_control),
                "control character in {m:?}"
            );
            assert!(m.contains("bo[2Jom"), "{m:?}");
        }
        other => panic!("expected ConversionFailed, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hostile_files_never_reach_an_engine() {
    // Even with a worker that does not exist: the container check fails first, so the error
    // is about the container and not about the worker.
    let o = Options {
        worker: Some(WorkerSpec::new("/nonexistent/cc-office-worker")),
        limits: Limits {
            max_entry_bytes: 4 * 1024 * 1024,
            ..Limits::default()
        },
    };
    let bomb = zw::build(&[
        zw::Entry::deflated("[Content_Types].xml", b"<Types/>"),
        zw::Entry::deflated("word/document.xml", &vec![0u8; 6 * 1024 * 1024]),
    ]);
    assert!(matches!(
        convert_bytes(&bomb, OfficeFormat::Docx, &o),
        Err(OfficeError::TooMuchData(_))
    ));
    let traversal = zw::build(&[
        zw::Entry::deflated("[Content_Types].xml", b"<Types/>"),
        zw::Entry::deflated("word/document.xml", b"<w/>"),
        zw::Entry::stored("../../etc/cron.d/x", b"x"),
    ]);
    assert!(matches!(
        convert_bytes(&traversal, OfficeFormat::Docx, &o),
        Err(OfficeError::BadContainer(_))
    ));
    let corrupt = &zw::docx("x")[..40];
    assert!(matches!(
        convert_bytes(corrupt, OfficeFormat::Docx, &o),
        Err(OfficeError::BadContainer(_))
    ));
}

#[test]
fn opendocument_and_legacy_files_are_refused_by_type() {
    let dir = std::env::temp_dir().join(format!("cc-office-odf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a.odt", "a.odp", "a.ods", "a.doc", "a.ppt", "a.xls"] {
        let p = dir.join(name);
        std::fs::write(&p, zw::docx("x")).unwrap();
        assert!(
            matches!(
                convert_file(&p, &opts()),
                Err(OfficeError::UnsupportedFormat)
            ),
            "{name}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
