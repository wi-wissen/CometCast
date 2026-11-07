// SPDX-License-Identifier: GPL-3.0-or-later
//! `send-file`, `send-link` and the printing of content events for `receive`.
//!
//! * `send-file <CODE> <FILE>`: sends an image (PNG, JPEG, WebP, GIF) or a PDF over the
//!   `control` data channel (docs/web-m2.md) to the receiver with that room code, optionally
//!   followed by page commands (`--then next,prev,go:3,stop`), and stays connected until Ctrl-C
//!   (or `--seconds N`): a receiver drops the shown content when the sender leaves.
//! * `send-link <CODE> <URL>`: sends an http(s) link.
//! * `receive` prints one `Content:` line per event (see [`print_event`]). A link is only
//!   printed, never opened.
//!
//! PDFs are rendered by the receiver in a child process: this program starts itself with the
//! hidden argument `docs-worker` (docs/documents.md).

use std::process::ExitCode;
use std::time::Duration;

use cc_docs::{Isolation, WorkerSpec};
use cc_proto::{ControlCmd, RoomCode};
use cc_receiver::{ContentEvent, ContentOptions, ImageMime};
use cc_sender::{ContentSender, ContentSenderOptions, SenderEvent};
use cc_transport::TlsTrust;

pub const USAGE: &str = "\
  cometcast-cli send-file <CODE> <FILE> [--then CMDS] [--seconds N] [--server HOST:PORT] [--bind ADDR] [--ca-file PEM]
  cometcast-cli send-link <CODE> <URL> [--seconds N] [--server HOST:PORT] [--bind ADDR] [--ca-file PEM]

send-file sends an image (.png .jpg .jpeg .webp .gif, at most 10 MiB) or a PDF (.pdf, at most
64 MiB) to the receiver with that room code. An Office document (.docx .pptx .xlsx) is first converted to PDF on this machine, in a child process with limits, and
the PDF is sent: the receiver never gets an Office file (see `office --help`, docs/documents.md). --then takes commands separated by commas, sent
after the file: next, prev, go:N (N from 1 to 1000000), stop (e.g. --then next,go:3,stop). The program stays
connected until Ctrl-C, because receivers drop the content when the sender leaves; --seconds N
leaves N seconds after sending (0 = at once); send-link takes --seconds too. `receive` prints what
arrives as 'Content:' lines; a link is printed and never opened. See docs/web-m2.md.";

/// What to send.
pub enum What {
    File { path: String, then: Vec<ControlCmd> },
    Link(String),
}

pub struct Cmd {
    stay: Option<u64>,
    code: RoomCode,
    what: What,
    server: String,
    bind: Option<String>,
    ca_file: Option<String>,
}

/// Parse the arguments after `send-file` (`link == false`) or `send-link`.
pub fn parse(link: bool, mut args: impl Iterator<Item = String>) -> Result<Cmd, String> {
    let mut server = "127.0.0.1:8443".to_string();
    let mut bind = None;
    let mut ca_file = None;
    let mut then = Vec::new();
    let mut stay = None;
    let mut positional = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--server" => server = args.next().ok_or("--server needs a value")?,
            "--bind" => bind = Some(args.next().ok_or("--bind needs a value")?),
            "--ca-file" => ca_file = Some(args.next().ok_or("--ca-file needs a value")?),
            "--seconds" => {
                let v = args.next().ok_or("--seconds needs a value")?;
                stay = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("bad --seconds value {v:?}"))?,
                );
            }
            "--then" if !link => then = parse_then(&args.next().ok_or("--then needs a value")?)?,
            s if s.starts_with("--") => return Err(format!("unknown option {s:?}")),
            _ => positional.push(a),
        }
    }
    let [code, target] = positional.as_slice() else {
        return Err(if link {
            "send-link needs a room code and a URL"
        } else {
            "send-file needs a room code and a file"
        }
        .into());
    };
    let code = RoomCode::parse(code).map_err(|e| format!("bad room code: {e}"))?;
    let what = if link {
        What::Link(target.clone())
    } else {
        What::File {
            path: target.clone(),
            then,
        }
    };
    Ok(Cmd {
        stay,
        code,
        what,
        server,
        bind,
        ca_file,
    })
}

fn parse_then(s: &str) -> Result<Vec<ControlCmd>, String> {
    s.split(',')
        .map(|c| match c.trim() {
            "next" => Ok(ControlCmd::NextPage),
            "prev" => Ok(ControlCmd::PrevPage),
            "stop" => Ok(ControlCmd::Stop),
            other => other
                .strip_prefix("go:")
                .and_then(|n| n.parse::<u32>().ok())
                // The receiver refuses page 0 and pages above the limit as a protocol violation.
                .filter(|n| (1..=cc_files::wire::MAX_PAGE_NUMBER).contains(n))
                .map(|page| ControlCmd::GoToPage { page })
                .ok_or_else(|| {
                    format!(
                        "bad command {other:?} in --then (go:N needs N from 1 to {})",
                        cc_files::wire::MAX_PAGE_NUMBER
                    )
                }),
        })
        .collect()
}

/// Content options for `receive`: PDFs render in a child process of this program.
pub fn options() -> Result<ContentOptions, String> {
    let isolation = if std::env::var_os(cc_docs::WORKER_ENV).is_some_and(|v| !v.is_empty()) {
        Isolation::Auto
    } else {
        WorkerSpec::current_exe_with_arg("docs-worker")
            .map(Isolation::Process)
            .map_err(|e| format!("cannot find this program to start the render worker: {e}"))?
    };
    Ok(ContentOptions {
        isolation,
        ..ContentOptions::default()
    })
}

/// FNV-1a, 64 bit: a short fingerprint so that tests can compare received bytes.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Print one event as a `Content:` line.
pub fn print_event(ev: &ContentEvent) {
    match ev {
        ContentEvent::TransferStarted {
            document,
            name,
            size,
        } => println!(
            "Content: transfer {} name={name:?} size={size}",
            if *document { "document" } else { "image" }
        ),
        ContentEvent::ImageReceived { name, mime, bytes } => println!(
            "Content: image name={name:?} mime={} bytes={} fnv1a64={:016x}",
            mime.as_str(),
            bytes.len(),
            fnv1a(bytes)
        ),
        ContentEvent::LinkRequested { url } => {
            println!("Content: link url={url} (not opened)");
        }
        ContentEvent::DocumentOpened { name, pages } => {
            println!("Content: document name={name:?} pages={pages}");
        }
        ContentEvent::PageRendered {
            page,
            width,
            height,
            ..
        } => println!("Content: page={page} width={width} height={height}"),
        ContentEvent::ContentStopped => println!("Content: stopped"),
        ContentEvent::Error { error, detail } => {
            println!("Content: error code={} detail={detail:?}", error.code());
        }
    }
}

#[derive(Debug)]
enum FileKind {
    Pdf,
    Image(ImageMime),
}

/// Read the file to send: the extension and the file type are checked first, then at most
/// `limit + 1` bytes are read, so a file that grows, a device (`/dev/zero` reports length 0)
/// or a FIFO cannot make this program read without bound.
fn read_file(path: &str) -> Result<(String, FileKind, Vec<u8>), String> {
    use std::io::Read;
    let name = std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
    let (kind, limit) = if name.to_ascii_lowercase().ends_with(".pdf") {
        (FileKind::Pdf, cc_files::MAX_DOCUMENT_BYTES)
    } else if let Some(mime) = ImageMime::from_extension(&name) {
        (FileKind::Image(mime), cc_files::MAX_IMAGE_BYTES)
    } else {
        return Err("only .png .jpg .jpeg .webp .gif, .pdf and Office files (.docx .pptx .xlsx) can be sent".into());
    };
    // `metadata` follows symbolic links, so a link to a device is seen as a device.
    let meta = std::fs::metadata(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{path} is not a regular file"));
    }
    let file = std::fs::File::open(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read {path}: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{path} is too large (at most {limit} bytes)"));
    }
    Ok((name, kind, bytes))
}

/// After the sender left: print the content events that are still queued, until the stream
/// ends (the content task finished) or nothing comes for a short time. Returns `true` if any
/// was printed. Without this, content that arrived just before `PeerLeft` could be lost when
/// the program exits.
pub async fn drain_events(events: &mut cc_receiver::ContentEvents) -> bool {
    let mut any = false;
    while let Ok(Some(ev)) = tokio::time::timeout(Duration::from_secs(2), events.next()).await {
        any = true;
        print_event(&ev);
    }
    any
}

pub async fn run(cmd: Cmd) -> ExitCode {
    match send(cmd).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn send(cmd: Cmd) -> Result<(), String> {
    // Read and check the file before connecting.
    enum Payload {
        Image(String, ImageMime, Vec<u8>),
        Pdf(String, Vec<u8>),
        Link(String),
    }
    let payload = match &cmd.what {
        What::Link(u) => Payload::Link(u.clone()),
        What::File { path, .. } if crate::office::is_office(path) => {
            // Conversion starts a child process and can take a while: not on an async thread.
            let p = std::path::PathBuf::from(path);
            let (name, bytes) =
                tokio::task::spawn_blocking(move || crate::office::convert_for_send(&p))
                    .await
                    .map_err(|e| format!("conversion task failed: {e}"))??;
            Payload::Pdf(name, bytes)
        }
        What::File { path, .. } => match read_file(path)? {
            (name, FileKind::Pdf, bytes) => Payload::Pdf(name, bytes),
            (name, FileKind::Image(mime), bytes) => Payload::Image(name, mime, bytes),
        },
    };

    let mut trust = TlsTrust::system();
    if let Some(p) = &cmd.ca_file {
        let pem = std::fs::read(p).map_err(|e| format!("cannot read --ca-file {p}: {e}"))?;
        trust
            .add_pem(&pem)
            .map_err(|e| format!("--ca-file {p}: {e}"))?;
    }
    let mut opts = ContentSenderOptions::new(cmd.server, cmd.code);
    opts.trust = trust;
    if let Some(b) = cmd.bind {
        opts.session.udp_bind = vec![b];
    }
    let mut sender = ContentSender::start(opts)
        .await
        .map_err(|e| e.to_string())?;
    let ctl = sender.control();
    // Wait for the connection, but report a failure of the signalling or the connection.
    let ready = tokio::select! {
        r = ctl.wait_ready(Duration::from_secs(30)) => r.map_err(|e| e.to_string()),
        err = async {
            while let Some(ev) = sender.next_event().await {
                match ev {
                    SenderEvent::Error(e) => return e,
                    SenderEvent::PeerLeft => return "the receiver left".to_string(),
                    _ => {}
                }
            }
            "stopped".to_string()
        } => Err(err),
    };
    ready?;
    println!("Connected.");
    let result = async {
        match &payload {
            Payload::Image(name, mime, bytes) => ctl.send_image(name, *mime, bytes).await,
            Payload::Pdf(name, bytes) => ctl.send_document(name, bytes).await,
            Payload::Link(url) => ctl.send_link(url).await,
        }
        .map_err(|e| e.to_string())?;
        if let What::File { then, .. } = &cmd.what {
            for c in then {
                // Give the receiver time to render the page before the next command.
                tokio::time::sleep(Duration::from_millis(500)).await;
                ctl.send_cmd(c.clone()).await.map_err(|e| e.to_string())?;
            }
        }
        if ctl.finish().await {
            Ok(())
        } else {
            Err("the data did not reach the receiver in time".to_string())
        }
    }
    .await;
    if result.is_ok() {
        println!("Sent.");
        stay(&mut sender, cmd.stay).await;
    }
    sender.stop().await;
    result
}

/// Stay connected: `seconds` seconds, or until Ctrl-C, or until the receiver leaves.
async fn stay(sender: &mut ContentSender, seconds: Option<u64>) {
    let limit = async {
        match seconds {
            Some(s) => tokio::time::sleep(Duration::from_secs(s)).await,
            None => std::future::pending().await,
        }
    };
    let gone = async {
        while let Some(ev) = sender.next_event().await {
            if matches!(ev, SenderEvent::PeerLeft | SenderEvent::Error(_)) {
                return;
            }
        }
    };
    tokio::select! {
        () = limit => {}
        () = gone => println!("The receiver left."),
        _ = tokio::signal::ctrl_c() => println!("Interrupted, closing."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> impl Iterator<Item = String> {
        s.split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_send_file_with_commands() {
        let c = parse(
            false,
            args("ABCDEF a.pdf --then next,go:3,prev,stop --seconds 2"),
        )
        .unwrap();
        assert_eq!(c.stay, Some(2));
        let What::File { path, then, .. } = c.what else {
            panic!()
        };
        assert_eq!(path, "a.pdf");
        assert_eq!(
            then,
            [
                ControlCmd::NextPage,
                ControlCmd::GoToPage { page: 3 },
                ControlCmd::PrevPage,
                ControlCmd::Stop
            ]
        );
    }

    #[test]
    fn parses_send_link_and_rejects_bad_input() {
        let c = parse(true, args("ABCDEF https://example.org/")).unwrap();
        assert!(matches!(c.what, What::Link(u) if u == "https://example.org/"));
        assert!(parse(true, args("ABCDEF https://example.org/ --then next")).is_err());
        assert!(parse(false, args("ABCDEF")).is_err());
        assert!(parse(false, args("abc a.png")).is_err());
        assert!(parse(false, args("ABCDEF a.png --then fly")).is_err());
        assert!(parse(false, args("ABCDEF a.png --then go:x")).is_err());
        assert!(parse(false, args("ABCDEF a.png --bogus")).is_err());
    }

    #[test]
    fn send_link_takes_seconds_but_not_then() {
        let c = parse(true, args("ABCDEF https://example.org/ --seconds 3")).unwrap();
        assert_eq!(c.stay, Some(3));
        assert!(parse(true, args("ABCDEF https://example.org/ --seconds x")).is_err());
    }

    #[test]
    fn page_numbers_in_then_must_be_in_range() {
        for bad in ["go:0", "go:1000001", "go:-1", "go:4294967296", "next,go:0"] {
            assert!(parse_then(bad).is_err(), "{bad}");
        }
        assert!(parse_then("go:1,go:1000000").is_ok());
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cometcast-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn files_are_read_with_a_bound() {
        let ok = temp("ok.png");
        std::fs::write(&ok, [1u8; 100]).unwrap();
        let (name, _, bytes) = read_file(ok.to_str().unwrap()).unwrap();
        assert_eq!((name.as_str(), bytes.len()), ("ok.png", 100));
        // Wrong extension: refused before the file is looked at.
        assert!(
            read_file("/nonexistent/x.txt")
                .unwrap_err()
                .contains("only .png")
        );
        // A directory with a good name is not a file.
        let dir = temp("dir.pdf");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            read_file(dir.to_str().unwrap())
                .unwrap_err()
                .contains("not a regular file")
        );
        // Too large, found by the bounded read (a sparse file; the limit is 10 MiB for images).
        let big = temp("big.png");
        let f = std::fs::File::create(&big).unwrap();
        f.set_len(cc_files::MAX_IMAGE_BYTES + 1).unwrap();
        assert!(
            read_file(big.to_str().unwrap())
                .unwrap_err()
                .contains("too large")
        );
        f.set_len(cc_files::MAX_IMAGE_BYTES).unwrap();
        assert!(read_file(big.to_str().unwrap()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_device_behind_a_link_is_not_read() {
        let link = temp("zero.pdf");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        assert!(
            read_file(link.to_str().unwrap())
                .unwrap_err()
                .contains("not a regular file")
        );
    }

    #[test]
    fn fingerprint_is_fnv1a() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
