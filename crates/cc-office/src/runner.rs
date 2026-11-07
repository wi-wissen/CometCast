// SPDX-License-Identifier: GPL-3.0-or-later
//! The parent side: checks the input, makes a private work directory, starts the worker as a
//! resource-limited child, waits with a deadline, kills the whole process group, and checks
//! the result before it is returned.
//!
//! The work directory is removed when the conversion ends (also on an error or a timeout).
//! If the **parent process itself is killed** (SIGKILL, power loss) the directory stays in
//! the temp directory; the next conversion removes stale `cometcast-office-*` directories
//! of this user that are older than one day, best effort.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::engine::{Concrete, Engine, find_soffice, rust_engine_compiled};
use crate::error::{Limits, OfficeError};
use crate::format::OfficeFormat;
use crate::worker::OUTPUT_NAME;
use crate::zipcheck;

/// How to start the worker: a program and the arguments that come before the worker's own.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    program: PathBuf,
    args: Vec<OsString>,
}

impl WorkerSpec {
    /// A program that calls [`worker_main`](crate::worker_main) (or
    /// [`worker_main_args`](crate::worker_main_args)).
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    /// The worker program (without the leading arguments).
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Add a leading argument (for example the `office-worker` command of `cometcast-cli`).
    #[must_use]
    pub fn arg(mut self, a: impl Into<OsString>) -> Self {
        self.args.push(a.into());
        self
    }

    /// The running program itself, started with `arg` first.
    pub fn current_exe_with_arg(arg: impl Into<OsString>) -> std::io::Result<Self> {
        Ok(Self::new(std::env::current_exe()?).arg(arg))
    }

    /// A file `cc-office-worker` next to the running executable, or one directory up when
    /// the executable is in a `deps` directory (the Cargo test layout). There is no
    /// environment variable and no `PATH` search: what runs is what sits next to us.
    pub fn discover() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        let name = format!("cc-office-worker{}", std::env::consts::EXE_SUFFIX);
        let dir = exe.parent()?;
        let mut cands = vec![dir.join(&name)];
        if dir.file_name().is_some_and(|n| n == "deps") {
            cands.push(dir.parent()?.join(&name));
        }
        cands
            .into_iter()
            .find(|p| p.is_file())
            .and_then(|p| p.canonicalize().ok())
            .map(Self::new)
    }
}

/// What to convert with.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Engine choice.
    pub engine: Engine,
    /// Limits.
    pub limits: Limits,
    /// How to start the worker; `None` = [`WorkerSpec::discover`].
    pub worker: Option<WorkerSpec>,
    /// LibreOffice program; `None` = [`find_soffice`] with no explicit path.
    pub soffice: Option<PathBuf>,
    /// The worker program has the pure-Rust engine even though this build does not (a
    /// parent that is built with `default-features = false` so that the engine's large
    /// dependency tree is not linked into it, and runs a worker that has it). Without this
    /// the parent refuses [`Engine::Rust`] with `NoEngine` before it starts the worker.
    pub rust_engine_in_worker: bool,
}

/// A converted document.
#[derive(Debug, Clone)]
pub struct Converted {
    /// The PDF.
    pub pdf: Vec<u8>,
    /// The engine that produced it.
    pub engine: Concrete,
    /// Warnings the engine reported (the pure-Rust engine lists what it left out).
    pub warnings: Vec<String>,
    /// Protections the worker reported as in effect (`rlimit-*`, `landlock-no-tcp`).
    pub protections: Vec<String>,
    /// Time the conversion took.
    pub elapsed: Duration,
}

impl Converted {
    /// The protections this platform is expected to give (see [`expected_protections`]) that
    /// the worker did **not** report as in effect. Callers should tell the user when this is
    /// not empty: the conversion still ran, but with less isolation than documented.
    pub fn missing_protections(&self) -> Vec<&'static str> {
        expected_protections()
            .iter()
            .copied()
            .filter(|p| !self.protections.iter().any(|x| x == p))
            .collect()
    }
}

/// The protection names a worker on this platform is expected to report: resource limits on
/// Unix (`rlimit-as` on Linux only), and the Landlock TCP block on Linux x86_64 and aarch64
/// (it also needs kernel 6.7+, so it can be missing on a supported platform). Empty on other
/// systems, where the worker applies no limit at all and only the parent's deadline holds.
pub fn expected_protections() -> &'static [&'static str] {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        &[
            "rlimit-cpu",
            "rlimit-core",
            "rlimit-fsize",
            "rlimit-nofile",
            "rlimit-as",
            "landlock-no-tcp",
        ]
    }
    #[cfg(all(
        unix,
        not(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))
    ))]
    {
        &["rlimit-cpu", "rlimit-core", "rlimit-fsize", "rlimit-nofile"]
    }
    #[cfg(not(unix))]
    {
        &[]
    }
}

/// Convert a file. The format comes from the extension; the content must agree.
pub fn convert_file(path: &Path, opts: &Options) -> Result<Converted, OfficeError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or(OfficeError::UnsupportedFormat)?;
    let format = OfficeFormat::from_name(&name).ok_or(OfficeError::UnsupportedFormat)?;
    // Open first, then look at the *open* file (`fstat`): no gap between the check and the
    // use. `O_NONBLOCK` keeps the open of a FIFO from waiting for a writer; a link to a
    // regular file is followed on purpose (the user named it), a link to a device is seen as
    // a device and refused.
    let file = open_input(path).map_err(|e| OfficeError::Io(e.to_string()))?;
    let meta = file
        .metadata()
        .map_err(|e| OfficeError::Io(e.to_string()))?;
    if !meta.is_file() {
        return Err(OfficeError::NotRegularFile);
    }
    let limit = opts.limits.max_input_bytes;
    // At most limit + 1 bytes: a file that grows cannot make this read without bound.
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| OfficeError::Io(e.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(OfficeError::InputTooLarge { limit });
    }
    convert_bytes(&bytes, format, opts)
}

#[cfg(unix)]
fn open_with(path: &Path, extra_flags: u32) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    // `bits()` of the flag set is a plain `c_int` worth of bits on every Unix.
    let flags = (rustix::fs::OFlags::NONBLOCK.bits() | extra_flags) as i32;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)
}

#[cfg(unix)]
fn open_input(path: &Path) -> std::io::Result<std::fs::File> {
    open_with(path, 0)
}

#[cfg(not(unix))]
fn open_input(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

/// Make text from an engine or from the worker's stderr safe to print: every control
/// character (ESC, BEL, C1 controls such as CSI, ...) is dropped, line breaks and tabs become
/// a space, and the bidirectional override characters that can reorder what a terminal shows
/// are dropped as well.
pub(crate) fn sanitize(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\n' | '\r' | '\t' => Some(' '),
            c if c.is_control() => None,
            '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => None,
            c => Some(c),
        })
        .collect()
}

/// Convert bytes of a known format.
pub fn convert_bytes(
    bytes: &[u8],
    format: OfficeFormat,
    opts: &Options,
) -> Result<Converted, OfficeError> {
    let limits = &opts.limits;
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::InputTooLarge {
            limit: limits.max_input_bytes,
        });
    }
    zipcheck::check(bytes, format, limits)?;

    let soffice = if opts.engine == Engine::Rust {
        None
    } else {
        find_soffice(opts.soffice.as_deref())
    };
    let order = opts.engine.order(
        format,
        soffice.is_some(),
        rust_engine_compiled() || opts.rust_engine_in_worker,
    );
    let Some(engine) = order.first().copied() else {
        return Err(OfficeError::NoEngine(no_engine_reason(
            opts.engine,
            format,
            soffice.is_some(),
        )));
    };
    let worker = match &opts.worker {
        Some(w) => w.clone(),
        None => WorkerSpec::discover().ok_or_else(|| {
            OfficeError::WorkerUnavailable(
                "no cc-office-worker next to this program and no explicit worker given".into(),
            )
        })?,
    };
    run_worker(bytes, format, engine, soffice.as_deref(), &worker, limits)
}

fn no_engine_reason(engine: Engine, format: OfficeFormat, have_soffice: bool) -> String {
    match engine {
        Engine::Soffice => "LibreOffice (soffice) was not found; install it or set COMETCAST_SOFFICE to its full path".into(),
        Engine::Rust if !format.rust_engine_supports() => {
            format!("the pure-Rust engine cannot convert .{} files; use LibreOffice", format.extension())
        }
        Engine::Rust => "the pure-Rust engine is not compiled into this build".into(),
        Engine::Auto if !format.rust_engine_supports() && !have_soffice => format!(
            ".{} files need LibreOffice, which was not found; install it or set COMETCAST_SOFFICE",
            format.extension()
        ),
        Engine::Auto => "neither LibreOffice nor the pure-Rust engine is available".into(),
    }
}

/// A private directory, removed again when dropped.
struct WorkDir(PathBuf);

/// Prefix of every work directory (the stale-directory clean-up only touches this prefix).
const WORK_PREFIX: &str = "cometcast-office-";
/// A work directory older than this is a leftover of a crashed run.
const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// Remove work directories under `base` that a crashed or killed run left behind: a real
/// directory (not a link) whose name starts with [`WORK_PREFIX`], owned by this user, and
/// last modified more than `max_age` ago. Best effort; every failure is ignored.
fn remove_stale_work_dirs(base: &Path, max_age: Duration) {
    let Ok(rd) = std::fs::read_dir(base) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in rd.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(WORK_PREFIX) {
            continue;
        }
        // `symlink_metadata`: a link is never followed, so it is neither inspected nor removed.
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != rustix::process::getuid().as_raw() {
                continue;
            }
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age > max_age);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

impl WorkDir {
    fn create() -> Result<Self, OfficeError> {
        Self::create_in(&std::env::temp_dir())
    }

    /// Create the directory under `base`. `base` is made absolute and free of links first:
    /// the worker runs with this directory as its working directory, so a relative `TMPDIR`
    /// would not mean the same thing there. Stale directories of earlier runs are removed
    /// once per process.
    fn create_in(base: &Path) -> Result<Self, OfficeError> {
        let base = std::fs::canonicalize(base)
            .map_err(|e| OfficeError::Io(format!("cannot resolve the temp directory: {e}")))?;
        static CLEANED: std::sync::Once = std::sync::Once::new();
        CLEANED.call_once(|| remove_stale_work_dirs(&base, STALE_AFTER));
        let path = base.join(format!(
            "{WORK_PREFIX}{}-{:016x}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        // `create` (not `create_all`) fails if the name exists: no reuse of someone's directory.
        b.create(&path)
            .map_err(|e| OfficeError::Io(format!("cannot create a work directory: {e}")))?;
        let w = Self(path);
        for d in ["in", "out"] {
            std::fs::create_dir(w.0.join(d)).map_err(|e| OfficeError::Io(e.to_string()))?;
        }
        Ok(w)
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Collect at most `cap` bytes from a pipe, but read it to the end.
fn spawn_reader<R: Read + Send + 'static>(mut r: R, cap: usize) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            if kept.len() < cap {
                kept.extend_from_slice(&buf[..n.min(cap - kept.len())]);
            }
        }
        let _ = tx.send(kept);
    });
    rx
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    use rustix::process::{Pid, Signal, kill_process_group};
    if let Some(pgid) = i32::try_from(pid).ok().and_then(Pid::from_raw) {
        let _ = kill_process_group(pgid, Signal::KILL);
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

fn run_worker(
    bytes: &[u8],
    format: OfficeFormat,
    engine: Concrete,
    soffice: Option<&Path>,
    worker: &WorkerSpec,
    limits: &Limits,
) -> Result<Converted, OfficeError> {
    use std::process::{Command, Stdio};
    let start = Instant::now();
    let work = WorkDir::create()?;
    // The private copy gets our canonical name, never the name the user gave.
    let input = work
        .0
        .join("in")
        .join(format!("input.{}", format.extension()));
    std::fs::write(&input, bytes).map_err(|e| OfficeError::Io(e.to_string()))?;

    let mem = match engine {
        Concrete::Rust => limits.rust_engine_memory_bytes,
        Concrete::Soffice => limits.soffice_memory_bytes,
    };
    let mut c = Command::new(&worker.program);
    c.args(&worker.args)
        .arg("--engine")
        .arg(engine.name())
        .arg("--format")
        .arg(format.extension())
        .arg("--work")
        .arg(&work.0)
        .arg("--timeout")
        .arg(limits.timeout.as_secs().max(1).to_string())
        .arg("--mem")
        .arg(mem.to_string())
        .arg("--fsize")
        .arg(limits.max_output_bytes.to_string());
    if let Some(s) = soffice.filter(|_| engine == Concrete::Soffice) {
        c.arg("--soffice").arg(s);
    }
    c.env_clear()
        .current_dir(&work.0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group: the parent can kill the worker and everything it started.
        c.process_group(0);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        c.env("SystemRoot", root);
    }
    let mut child = c.spawn().map_err(|e| {
        OfficeError::WorkerUnavailable(format!("{}: {e}", worker.program.display()))
    })?;
    let pid = child.id();
    let out_rx = child.stdout.take().map(|s| spawn_reader(s, 64 * 1024));
    let err_rx = child.stderr.take().map(|s| spawn_reader(s, 2 * 1024));

    // The worker has the same timeout itself; the parent's is the one that cannot be
    // bypassed, with a little extra for start-up and clean-up.
    let deadline = start + limits.timeout + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {}
            Err(e) => {
                kill_group(pid);
                let _ = child.kill();
                return Err(OfficeError::ConversionFailed(e.to_string()));
            }
        }
        if Instant::now() >= deadline {
            kill_group(pid);
            let _ = child.kill();
            let _ = child.wait();
            return Err(OfficeError::Timeout(limits.timeout));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Anything the worker left behind (LibreOffice helper processes) dies with the group.
    kill_group(pid);
    let grab = |rx: Option<mpsc::Receiver<Vec<u8>>>| {
        rx.and_then(|r| r.recv_timeout(Duration::from_secs(2)).ok())
            .unwrap_or_default()
    };
    let stdout = String::from_utf8_lossy(&grab(out_rx)).into_owned();
    let stderr = String::from_utf8_lossy(&grab(err_rx)).into_owned();

    if !status.success() {
        // Worker stderr can echo bytes from the document: strip control characters before
        // the text can reach a terminal.
        let detail = sanitize(&stderr);
        let detail: String = detail.trim().chars().take(300).collect();
        return Err(OfficeError::ConversionFailed(if detail.is_empty() {
            format!("the worker ended with {status}")
        } else {
            format!("{detail} ({status})")
        }));
    }
    let mut warnings = Vec::new();
    let mut protections = Vec::new();
    for line in stdout.lines() {
        if let Some(w) = line.strip_prefix("warning:") {
            // Engine warnings quote the document (font names and so on): sanitise them.
            warnings.push(sanitize(w).trim().chars().take(300).collect());
        } else if let Some(p) = line.strip_prefix("protection:") {
            // Protection names are ours: anything that is not a plain name is dropped.
            let p = p.trim();
            if !p.is_empty()
                && p.len() <= 32
                && p.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
            {
                protections.push(p.to_string());
            }
        }
    }
    let pdf = read_output(&work.0.join("out").join(OUTPUT_NAME), limits)?;
    Ok(Converted {
        pdf,
        engine,
        warnings,
        protections,
        elapsed: start.elapsed(),
    })
}

#[cfg(unix)]
fn open_output(path: &Path) -> std::io::Result<std::fs::File> {
    open_with(path, rustix::fs::OFlags::NOFOLLOW.bits())
}

#[cfg(not(unix))]
fn open_output(path: &Path) -> std::io::Result<std::fs::File> {
    // No `O_NOFOLLOW` here: refuse a link by looking at it first (UNCERTAIN: not run on Windows).
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::other("a link"));
    }
    std::fs::File::open(path)
}

/// Read the result: a regular file (not a link), within the cap, that looks like a PDF.
fn read_output(path: &Path, limits: &Limits) -> Result<Vec<u8>, OfficeError> {
    let bad = |m: &str| OfficeError::BadOutput(m.to_string());
    // Open without following a link and without blocking (the engine's directory is not
    // trusted: the name could be a FIFO or a link), then look at the open file.
    let file = open_output(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            bad("the engine produced no PDF")
        } else {
            bad("the result cannot be opened (a link or a special file?)")
        }
    })?;
    let meta = file
        .metadata()
        .map_err(|e| OfficeError::Io(e.to_string()))?;
    if !meta.is_file() {
        return Err(bad("the result is not a regular file"));
    }
    if meta.len() > limits.max_output_bytes {
        return Err(OfficeError::BadOutput(format!(
            "the PDF is larger than {} bytes",
            limits.max_output_bytes
        )));
    }
    let mut buf = Vec::new();
    file.take(limits.max_output_bytes + 1)
        .read_to_end(&mut buf)
        .map_err(|e| OfficeError::Io(e.to_string()))?;
    if buf.len() as u64 > limits.max_output_bytes {
        return Err(bad("the PDF is larger than the cap"));
    }
    if !buf.starts_with(b"%PDF-") {
        return Err(bad("the result does not start with %PDF-"));
    }
    let tail = &buf[buf.len().saturating_sub(2048)..];
    if !tail.windows(5).any(|w| w == b"%%EOF") {
        return Err(bad("the PDF has no end marker (cut off?)"));
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_checks() {
        let dir = WorkDir::create().unwrap();
        let p = dir.0.join("x.pdf");
        let lim = Limits {
            max_output_bytes: 100,
            ..Limits::default()
        };
        std::fs::write(&p, b"%PDF-1.7\n1 0 obj\n%%EOF\n").unwrap();
        assert!(read_output(&p, &lim).is_ok());
        std::fs::write(&p, b"not a pdf").unwrap();
        assert!(matches!(
            read_output(&p, &lim),
            Err(OfficeError::BadOutput(_))
        ));
        std::fs::write(&p, b"%PDF-1.7 no end").unwrap();
        assert!(matches!(
            read_output(&p, &lim),
            Err(OfficeError::BadOutput(_))
        ));
        std::fs::write(&p, vec![b'%'; 101]).unwrap();
        assert!(
            matches!(read_output(&p, &lim), Err(OfficeError::BadOutput(m)) if m.contains("larger"))
        );
        assert!(matches!(
            read_output(&dir.0.join("missing.pdf"), &lim),
            Err(OfficeError::BadOutput(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn output_symlink_is_refused() {
        let dir = WorkDir::create().unwrap();
        let target = dir.0.join("t.pdf");
        std::fs::write(&target, b"%PDF-1.7\n%%EOF\n").unwrap();
        let link = dir.0.join("l.pdf");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(matches!(
            read_output(&link, &Limits::default()),
            Err(OfficeError::BadOutput(_))
        ));
    }

    #[test]
    fn work_dir_is_private_and_removed() {
        let p;
        {
            let d = WorkDir::create().unwrap();
            p = d.0.clone();
            assert!(p.join("in").is_dir() && p.join("out").is_dir());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&p).unwrap().permissions().mode();
                assert_eq!(mode & 0o077, 0, "work dir must not be accessible to others");
            }
        }
        assert!(!p.exists());
    }

    #[test]
    fn input_checks_happen_before_any_engine() {
        let o = Options::default();
        assert!(matches!(
            convert_file(Path::new("/nonexistent/a.pdf"), &o),
            Err(OfficeError::UnsupportedFormat)
        ));
        assert!(matches!(
            convert_file(Path::new("/dev/zero.docx"), &o),
            Err(OfficeError::Io(_))
        ));
        #[cfg(unix)]
        {
            let dir = WorkDir::create().unwrap();
            let d = dir.0.join("dir.docx");
            std::fs::create_dir(&d).unwrap();
            assert!(matches!(
                convert_file(&d, &o),
                Err(OfficeError::NotRegularFile)
            ));
            let big = dir.0.join("big.docx");
            std::fs::write(&big, vec![b'x'; 2000]).unwrap();
            let small = Options {
                limits: Limits {
                    max_input_bytes: 1000,
                    ..Limits::default()
                },
                ..Options::default()
            };
            assert!(matches!(
                convert_file(&big, &small),
                Err(OfficeError::InputTooLarge { limit: 1000 })
            ));
            // A file of the right size that is not a zip never reaches an engine.
            assert!(matches!(
                convert_file(&big, &o),
                Err(OfficeError::BadContainer(_))
            ));
        }
    }

    #[test]
    fn sanitize_strips_every_control_character() {
        let dirty = "a\u{1b}[31mb\u{7}c\u{9b}d\ne\tf\r\u{0}g\u{202e}h\u{1b}]0;title\u{7}";
        let clean = sanitize(dirty);
        assert!(!clean.chars().any(char::is_control), "{clean:?}");
        assert!(!clean.contains('\u{202e}'));
        assert_eq!(clean, "a[31mbcd e f gh]0;title");
        assert_eq!(sanitize("plain text, ünïcode"), "plain text, ünïcode");
    }

    #[test]
    fn work_dir_base_is_made_absolute() {
        // A relative base (what a relative TMPDIR is) must not reach the worker as a relative
        // path: the worker's working directory is the work directory itself.
        let rel = format!("cc-office-reltest-{}", std::process::id());
        std::fs::create_dir(&rel).unwrap();
        let made = WorkDir::create_in(Path::new(&rel));
        let expect_under = std::fs::canonicalize(&rel);
        let path = made.as_ref().ok().map(|w| w.0.clone());
        drop(made);
        let _ = std::fs::remove_dir_all(&rel);
        let path = path.expect("work dir under a relative base");
        assert!(path.is_absolute(), "{path:?}");
        assert!(path.starts_with(expect_under.unwrap()), "{path:?}");
    }

    #[cfg(unix)]
    #[test]
    fn stale_work_dirs_are_removed_and_nothing_else() {
        let scratch = WorkDir::create().unwrap();
        let mk = |name: &str| {
            let d = scratch.0.join(name);
            std::fs::create_dir_all(d.join("in")).unwrap();
            std::fs::write(d.join("in").join("f"), b"x").unwrap();
            d
        };
        let old = mk("cometcast-office-1-old");
        let fresh = mk("cometcast-office-2-fresh");
        let other_name = mk("something-else");
        let victim = mk("victim");
        let link = scratch.0.join("cometcast-office-3-link");
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        let two_days = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
        for d in [&old, &other_name, &victim] {
            std::fs::File::open(d)
                .unwrap()
                .set_modified(two_days)
                .unwrap();
        }
        remove_stale_work_dirs(&scratch.0, STALE_AFTER);
        assert!(!old.exists(), "the old work directory must be gone");
        assert!(fresh.exists(), "a fresh one must stay");
        assert!(
            other_name.exists(),
            "a directory with another name must stay"
        );
        assert!(
            victim.join("in").join("f").exists(),
            "a link must not be followed"
        );
        assert!(link.symlink_metadata().is_ok(), "the link itself stays");
    }

    #[cfg(unix)]
    #[test]
    fn fifos_do_not_block_and_are_refused() {
        use rustix::fs::{CWD, FileType, Mode, mknodat};
        let dir = WorkDir::create().unwrap();
        // As the output file: refused, and the call returns.
        let out = dir.0.join("out.pdf");
        mknodat(CWD, &out, FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
        let (tx, rx) = mpsc::channel();
        let out2 = out.clone();
        std::thread::spawn(move || {
            let _ = tx.send(read_output(&out2, &Limits::default()));
        });
        let r = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("read_output blocked on a FIFO");
        assert!(
            matches!(&r, Err(OfficeError::BadOutput(m)) if m.contains("regular")),
            "{r:?}"
        );
        // As the input file: refused without waiting for a writer.
        let input = dir.0.join("in.docx");
        mknodat(CWD, &input, FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(convert_file(&input, &Options::default()));
        });
        let r = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("convert_file blocked on a FIFO");
        assert!(matches!(r, Err(OfficeError::NotRegularFile)), "{r:?}");
    }

    #[test]
    fn missing_protections_are_listed() {
        let mk = |p: &[&str]| Converted {
            pdf: Vec::new(),
            engine: Concrete::Rust,
            warnings: Vec::new(),
            protections: p.iter().map(|s| (*s).to_string()).collect(),
            elapsed: Duration::ZERO,
        };
        let all = mk(expected_protections());
        assert!(all.missing_protections().is_empty());
        let none = mk(&[]);
        assert_eq!(none.missing_protections(), expected_protections());
        #[cfg(unix)]
        {
            let partial = mk(&["rlimit-cpu"]);
            assert!(!partial.missing_protections().contains(&"rlimit-cpu"));
            assert!(partial.missing_protections().contains(&"rlimit-fsize"));
        }
    }
}
