// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli docs`: look at a PDF and render one page to a picture (docs/documents.md).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use cc_docs::{
    Document, Isolation, Limits, WORKER_ENV, WorkerSpec, encode_png, encode_webp_lossless,
    worker_main,
};

/// Text for `--help` of this command (added to the main usage text).
pub const USAGE: &str = "\
docs (see docs/documents.md):
  cometcast-cli docs info <file.pdf>
  cometcast-cli docs render <file.pdf> [--page N] [--width PX] --out <out.png|out.webp>
    --page N     page number, first page is 1 (default 1)
    --width PX   picture width in pixels (default 1920); the height follows the page shape
    --out FILE   .png or .webp (lossless); required
    --force      replace --out if it exists (never through a symbolic link)
  The PDF is rendered in a child process of this program with memory and time limits
  (a PDF from outside is treated as hostile). Encrypted files that need a password are
  refused. Limits: docs/documents.md.";

/// What `docs` was asked to do.
pub enum DocsArgs {
    /// `docs info FILE`.
    Info { file: PathBuf },
    /// `docs render FILE ...`.
    Render {
        file: PathBuf,
        page: usize,
        width: u32,
        out: PathBuf,
        force: bool,
    },
}

/// Parse the arguments after `docs`. `Ok(None)` means help was asked for.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<DocsArgs>, String> {
    let sub = args
        .next()
        .ok_or("docs needs a subcommand: info or render")?;
    match sub.as_str() {
        "--help" | "-h" | "help" => Ok(None),
        "info" => {
            let file = args.next().ok_or("docs info needs a file")?;
            if file == "--help" || file == "-h" {
                return Ok(None);
            }
            if let Some(extra) = args.next() {
                return Err(format!("unexpected argument {extra:?} for docs info"));
            }
            Ok(Some(DocsArgs::Info { file: file.into() }))
        }
        "render" => {
            let mut file = None;
            let mut page = 1usize;
            let mut width = 1920u32;
            let mut out = None;
            let mut force = false;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--help" | "-h" => return Ok(None),
                    "--page" => {
                        let v = args.next().ok_or("--page needs a value")?;
                        page = v
                            .parse::<usize>()
                            .ok()
                            .filter(|n| *n >= 1)
                            .ok_or(format!("bad --page value {v:?} (1 or more)"))?;
                    }
                    "--width" => {
                        let v = args.next().ok_or("--width needs a value")?;
                        width = v
                            .parse::<u32>()
                            .ok()
                            .filter(|n| *n >= 1)
                            .ok_or(format!("bad --width value {v:?} (1 or more)"))?;
                    }
                    "--out" => out = Some(args.next().ok_or("--out needs a value")?),
                    "--force" => force = true,
                    other if other.starts_with('-') => {
                        return Err(format!("unknown argument {other:?} for docs render"));
                    }
                    other => {
                        if file.replace(other.to_string()).is_some() {
                            return Err("docs render takes one file".into());
                        }
                    }
                }
            }
            Ok(Some(DocsArgs::Render {
                file: file.ok_or("docs render needs a file")?.into(),
                page,
                width,
                out: out.ok_or("docs render needs --out FILE")?.into(),
                force,
            }))
        }
        other => Err(format!("unknown docs subcommand {other:?}")),
    }
}

/// Where the renderer runs: the program named by `COMETCAST_DOCS_WORKER` if set, otherwise
/// this program itself with the hidden argument `docs-worker`.
fn isolation() -> Result<Isolation, String> {
    if std::env::var_os(WORKER_ENV).is_some_and(|v| !v.is_empty()) {
        return Ok(Isolation::Auto);
    }
    WorkerSpec::current_exe_with_arg("docs-worker")
        .map(Isolation::Process)
        .map_err(|e| format!("cannot find this program to start the render worker: {e}"))
}

/// Read a PDF without trusting the file system: only regular files, never more than the
/// size limit (a device such as `/dev/zero` reports size 0 and never ends).
fn read_limited(file: &Path, max: usize) -> Result<Vec<u8>, String> {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", file.display());
    let f = std::fs::File::open(file).map_err(|e| err(&e))?;
    let meta = f.metadata().map_err(|e| err(&e))?;
    if !meta.is_file() {
        return Err(err(&"not a regular file"));
    }
    if meta.len() > max as u64 {
        return Err(err(&format!("too large (limit {max} bytes)")));
    }
    let mut bytes = Vec::new();
    f.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| err(&e))?;
    if bytes.len() > max {
        return Err(err(&format!("too large (limit {max} bytes)")));
    }
    Ok(bytes)
}

fn open(file: &Path) -> Result<(Document, usize), String> {
    let limits = Limits::default();
    let bytes = read_limited(file, limits.max_file_bytes)?;
    let size = bytes.len();
    let doc = Document::open_with_options(bytes, limits, isolation()?)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    Ok((doc, size))
}

/// Write the output file: a new file only (`O_EXCL`, which never follows a symbolic link),
/// unless `force`. With `force` the data goes to a **new temporary file in the same
/// directory** and is renamed over the target, so a symbolic link that appears at the target
/// between the check and the write is replaced as a link and never written through, and a
/// reader never sees half a file. An existing symbolic link is refused up front anyway.
fn write_output(path: &Path, data: &[u8], force: bool) -> Result<(), String> {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    if !force {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    err(&"already exists (use --force to replace it)")
                } else {
                    err(&e)
                }
            })?;
        return f.write_all(data).map_err(|e| err(&e));
    }
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(err(&"is a symbolic link, not overwritten"));
    }
    replace_file(path, data).map_err(|e| err(&e))
}

/// Temporary file next to `path`, then `rename` over it.
fn replace_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".tmp-{}-{nanos}", std::process::id()));
    let tmp = dir.join(tmp_name);
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The hidden `docs-worker` command: the render worker process of `cc-docs`. It reads one
/// request on stdin and writes one reply on stdout; it is started by `docs info` and
/// `docs render`, not by hand.
pub fn worker() -> ExitCode {
    ExitCode::from(u8::try_from(worker_main()).unwrap_or(1))
}

/// Run `docs`.
pub fn run(a: DocsArgs) -> ExitCode {
    match run_inner(a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_inner(a: DocsArgs) -> Result<(), String> {
    match a {
        DocsArgs::Info { file } => {
            let (doc, size) = open(&file)?;
            println!("file: {} ({size} bytes)", file.display());
            println!("pages: {}", doc.page_count());
            const SHOW: usize = 20;
            for i in 0..doc.page_count().min(SHOW) {
                if let Ok(s) = doc.page_size(i) {
                    println!("page {}: {} x {} pt", i + 1, s.width_pt, s.height_pt);
                }
            }
            if doc.page_count() > SHOW {
                println!("... ({} more pages)", doc.page_count() - SHOW);
            }
            Ok(())
        }
        DocsArgs::Render {
            file,
            page,
            width,
            out,
            force,
        } => {
            let ext = out
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(ext.as_deref(), Some("png" | "webp")) {
                return Err("--out must end in .png or .webp".into());
            }
            // Fail on an existing output before the (slow) render, not after.
            if !force && out.exists() {
                return Err(format!(
                    "{}: already exists (use --force to replace it)",
                    out.display()
                ));
            }
            let (doc, _) = open(&file)?;
            if page > doc.page_count() {
                return Err(format!(
                    "page {page} is out of range (the document has {} pages)",
                    doc.page_count()
                ));
            }
            let started = Instant::now();
            let img = doc
                .render_page(page - 1, width)
                .map_err(|e| e.to_string())?;
            let took = started.elapsed();
            let bytes = if ext.as_deref() == Some("webp") {
                encode_webp_lossless(&img)
            } else {
                encode_png(&img)
            }
            .map_err(|e| e.to_string())?;
            write_output(&out, &bytes, force)?;
            println!(
                "page {page} of {}: {}x{} px in {:.1} ms, wrote {} ({} bytes)",
                doc.page_count(),
                img.width,
                img.height,
                took.as_secs_f64() * 1e3,
                out.display(),
                bytes.len()
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Option<DocsArgs>, String> {
        parse(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_render() {
        match p(&[
            "render", "a.pdf", "--page", "3", "--width", "800", "--out", "o.png",
        ]) {
            Ok(Some(DocsArgs::Render { page, width, .. })) => assert_eq!((page, width), (3, 800)),
            _ => panic!("not parsed"),
        }
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(p(&[]).is_err());
        assert!(p(&["render", "a.pdf"]).is_err());
        assert!(p(&["render", "a.pdf", "--page", "0", "--out", "o.png"]).is_err());
        assert!(p(&["render", "a.pdf", "--width", "x", "--out", "o.png"]).is_err());
        assert!(p(&["render", "a.pdf", "--out", "o.png", "--force"]).is_ok());
        assert!(p(&["info"]).is_err());
        assert!(p(&["info", "a", "b"]).is_err());
        assert!(p(&["bogus"]).is_err());
        assert!(matches!(p(&["--help"]), Ok(None)));
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-docs-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn output_is_never_overwritten_without_force() {
        let d = scratch("out");
        let f = d.join("o.png");
        write_output(&f, b"one", false).unwrap();
        let e = write_output(&f, b"two", false).unwrap_err();
        assert!(e.contains("--force"), "{e}");
        assert_eq!(std::fs::read(&f).unwrap(), b"one");
        write_output(&f, b"three", true).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"three");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn output_never_follows_a_symbolic_link() {
        let d = scratch("link");
        let victim = d.join("victim.txt");
        std::fs::write(&victim, b"precious").unwrap();
        let link = d.join("o.png");
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        assert!(write_output(&link, b"x", false).is_err());
        assert!(write_output(&link, b"x", true).is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// The race the review found: the target becomes a symbolic link after the check. The
    /// rename replaces the link itself; the file it pointed to is not touched.
    #[cfg(unix)]
    #[test]
    fn forced_replacement_swaps_a_late_symbolic_link_instead_of_writing_through_it() {
        let d = scratch("late-link");
        let victim = d.join("victim.txt");
        std::fs::write(&victim, b"precious").unwrap();
        let target = d.join("o.png");
        std::os::unix::fs::symlink(&victim, &target).unwrap();
        // `replace_file` is what runs after the symlink check.
        replace_file(&target, b"new picture").unwrap();
        assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
        assert_eq!(std::fs::read(&target).unwrap(), b"new picture");
        assert!(
            !std::fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        // No temporary file is left behind, also not after a failure.
        assert!(replace_file(&d.join("no-such-dir").join("o.png"), b"x").is_err());
        let left: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.contains(".tmp-"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn input_is_read_with_a_size_bound() {
        let d = scratch("in");
        let f = d.join("a.pdf");
        std::fs::write(&f, vec![b'x'; 100]).unwrap();
        assert_eq!(read_limited(&f, 100).unwrap().len(), 100);
        assert!(read_limited(&f, 99).unwrap_err().contains("too large"));
        assert!(read_limited(&d, 100).unwrap_err().contains("regular"));
        assert!(read_limited(&d.join("missing"), 100).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn devices_are_refused_not_read() {
        // `docs info /dev/zero` used to read until memory ran out.
        let e = read_limited(Path::new("/dev/zero"), 1 << 20).unwrap_err();
        assert!(e.contains("regular"), "{e}");
    }
}
