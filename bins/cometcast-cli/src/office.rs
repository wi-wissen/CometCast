// SPDX-License-Identifier: GPL-3.0-or-later
//! Office documents to PDF on the sender side (docs/documents.md, "Office documents").
//!
//! * `office convert FILE --out FILE.pdf` converts one file;
//! * `send-file` calls [`convert_for_send`] for `.docx .pptx .xlsx`.
//!
//! The conversion runs in a resource-limited child process of this program: it starts itself
//! with the hidden argument `office-worker` (decided in `main` before any async runtime
//! exists, like `docs-worker`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cc_office::{OfficeFormat, Options, WorkerSpec};

/// Text for `--help` of this command (added to the main usage text).
pub const USAGE: &str = "\
office (see docs/documents.md):
  cometcast-cli office convert <file> --out <file.pdf> [--force]
  Converts .docx .pptx .xlsx to PDF on this machine with the built-in pure-Rust converter, in a
  child process with time, memory and size limits. send-file does the same before sending,
  so the receiver only ever gets a PDF. OpenDocument (.odt .odp .ods) and old binary
  formats (.doc .ppt .xls) are not supported. The child process is limited, not sandboxed.
  Fidelity on real documents has not been measured. The command warns when the limits or
  the network block were not in effect.";

/// What `office` was asked to do.
pub enum OfficeArgs {
    /// `office convert`.
    Convert {
        file: PathBuf,
        out: PathBuf,
        force: bool,
    },
}

/// Parse the arguments after `office`. `Ok(None)` means help was asked for.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<OfficeArgs>, String> {
    match args
        .next()
        .ok_or("office needs a subcommand: convert")?
        .as_str()
    {
        "--help" | "-h" | "help" => Ok(None),
        "convert" => {
            let (mut file, mut out, mut force) = (None, None, false);
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--out" => out = Some(args.next().ok_or("--out needs a value")?),
                    "--force" => force = true,
                    "--help" | "-h" => return Ok(None),
                    s if s.starts_with("--") => return Err(format!("unknown option {s:?}")),
                    _ if file.is_none() => file = Some(a),
                    _ => return Err(format!("unexpected argument {a:?}")),
                }
            }
            Ok(Some(OfficeArgs::Convert {
                file: file.ok_or("office convert needs a file")?.into(),
                out: out.ok_or("office convert needs --out")?.into(),
                force,
            }))
        }
        other => Err(format!("unknown office subcommand {other:?}")),
    }
}

/// The hidden `office-worker` command: the conversion worker process (everything after the
/// command name is the worker's own argument list).
pub fn worker() -> ExitCode {
    let code = cc_office::worker_main_args(std::env::args_os().skip(2));
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Whether the file name is an Office document that `send-file` converts.
pub fn is_office(name: &str) -> bool {
    OfficeFormat::from_name(name).is_some()
}

fn options() -> Result<Options, String> {
    let spec = WorkerSpec::current_exe_with_arg("office-worker")
        .map_err(|e| format!("cannot find this program to start the conversion worker: {e}"))?;
    Ok(Options {
        worker: Some(spec),
        ..Options::default()
    })
}

/// Convert for sending. Returns the name to send (`<stem>.pdf`) and the PDF bytes (at most
/// the transfer limit for documents). Blocking: call it from a blocking context.
pub fn convert_for_send(path: &Path) -> Result<(String, Vec<u8>), String> {
    let conv = cc_office::convert_file(path, &options()?).map_err(|e| e.to_string())?;
    if conv.pdf.len() as u64 > cc_files::MAX_DOCUMENT_BYTES {
        return Err(format!(
            "the converted PDF is {} bytes, more than the {} byte limit for documents",
            conv.pdf.len(),
            cc_files::MAX_DOCUMENT_BYTES
        ));
    }
    println!(
        "Converted to PDF in {:.1} s ({} bytes).",
        conv.elapsed.as_secs_f32(),
        conv.pdf.len()
    );
    for w in conv.warnings.iter().take(10) {
        println!("Conversion warning: {w}");
    }
    for line in protection_notes(&conv) {
        eprintln!("{line}");
    }
    let stem = path.file_stem().map_or_else(
        || "document".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );
    Ok((format!("{stem}.pdf"), conv.pdf))
}

/// Lines to show about the isolation the conversion really had: what was in effect, and a
/// warning when something expected was not (no Landlock on an old kernel, a platform without
/// limits).
fn protection_notes(conv: &cc_office::Converted) -> Vec<String> {
    let mut v = Vec::new();
    if conv.protections.is_empty() {
        v.push(
            "Warning: no resource limits or network block were in effect for this conversion \
             (only the time limit); see docs/documents.md."
                .to_string(),
        );
    } else {
        v.push(format!(
            "Protections in effect: {}.",
            conv.protections.join(", ")
        ));
        let missing = conv.missing_protections();
        if !missing.is_empty() {
            v.push(format!(
                "Warning: not in effect: {}. The conversion ran with less isolation than \
                 documented (for example Landlock needs Linux 6.7 or newer).",
                missing.join(", ")
            ));
        }
    }
    v
}

/// Run `office`.
pub fn run(a: OfficeArgs) -> ExitCode {
    match run_inner(a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_inner(a: OfficeArgs) -> Result<(), String> {
    match a {
        OfficeArgs::Convert { file, out, force } => {
            let (_, pdf) = convert_for_send(&file)?;
            write_output(&out, &pdf, force)
        }
    }
}

/// Create the output new (`O_EXCL`); with `force`, write a new temporary file next to it and
/// rename it over the target, so a symbolic link is replaced and never written through.
fn write_output(out: &Path, pdf: &[u8], force: bool) -> Result<(), String> {
    let err = |e: std::io::Error| format!("cannot write {}: {e}", out.display());
    if !force {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out)
            .map_err(err)?;
        return f.write_all(pdf).and_then(|()| f.flush()).map_err(err);
    }
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = dir.join(format!(".cometcast-office-{}.tmp", std::process::id()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(err)?;
    let r = f
        .write_all(pdf)
        .and_then(|()| f.flush())
        .and_then(|()| std::fs::rename(&tmp, out));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Option<OfficeArgs>, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_convert() {
        let Ok(Some(OfficeArgs::Convert { force, .. })) = p("convert a.docx --out a.pdf --force")
        else {
            panic!("did not parse");
        };
        assert!(force);
        assert!(p("convert a.docx").is_err());
        assert!(p("convert --out a.pdf").is_err());
        assert!(p("convert a.docx --out a.pdf --engine rust").is_err());
        assert!(p("convert a.docx b.docx --out a.pdf").is_err());
        assert!(p("frobnicate").is_err());
        assert!(p("engines").is_err());
        assert!(matches!(p("help"), Ok(None)));
    }

    #[test]
    fn office_names() {
        assert!(is_office("a.DOCX") && is_office("b.xlsx"));
        assert!(!is_office("a.pdf") && !is_office("a.doc") && !is_office("b.ods"));
    }
}
