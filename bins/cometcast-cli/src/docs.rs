// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli docs`: look at a PDF and render one page to a picture (docs/documents.md).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use cc_docs::{Document, encode_png, encode_webp_lossless};

/// Text for `--help` of this command (added to the main usage text).
pub const USAGE: &str = "\
docs (see docs/documents.md):
  cometcast-cli docs info <file.pdf>
  cometcast-cli docs render <file.pdf> [--page N] [--width PX] --out <out.png|out.webp>
    --page N     page number, first page is 1 (default 1)
    --width PX   picture width in pixels (default 1920); the height follows the page shape
    --out FILE   .png or .webp (lossless); required
  Encrypted files that need a password are refused. Limits: docs/documents.md.";

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
            }))
        }
        other => Err(format!("unknown docs subcommand {other:?}")),
    }
}

fn open(file: &PathBuf) -> Result<(Document, usize), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let size = bytes.len();
    let doc = Document::open(bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok((doc, size))
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
        } => {
            let ext = out
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(ext.as_deref(), Some("png" | "webp")) {
                return Err("--out must end in .png or .webp".into());
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
            std::fs::write(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
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
        assert!(p(&["info"]).is_err());
        assert!(p(&["info", "a", "b"]).is_err());
        assert!(p(&["bogus"]).is_err());
        assert!(matches!(p(&["--help"]), Ok(None)));
    }
}
