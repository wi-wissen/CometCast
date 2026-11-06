// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast command line tool (developer tooling for now).
//!
//! `receive` creates a room and prints packet statistics of the incoming video
//! (`--decode` also decodes H.264 and prints the picture size and mean colour).
//! `send <CODE>` joins a room and sends a synthetic VP8 test stream, or with `--screen` the
//! real screen (X11 only) as H.264.
//!
//! Note: the output text of this tool is English only. It is a developer tool, not a user
//! interface, so it is not translated (the i18n rules in docs/i18n.md are for the apps).

mod airplay_send;

use std::process::ExitCode;
use std::time::Duration;

mod cast;
mod discover;
mod docs;

use cc_airplay::{AirplayConfig, AirplayEvent, AirplayStatus, Supervisor, SupervisorOptions};
use cc_proto::RoomCode;
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{ScreenOptions, SenderCore, SenderEvent, SenderOptions};
use cc_transport::{IceServerConfig, TlsTrust};

const USAGE: &str = "\
Usage:
  cometcast-cli receive [--server HOST:PORT] [--bind ADDR] [--ca-file PEM] [--decode]
  cometcast-cli send <CODE> [--server HOST:PORT] [--seconds N] [--bind ADDR] [--ca-file PEM]
                    [--screen [--display :N] [--fps N] [--bitrate-kbps N]]
  cometcast-cli airplay [--name NAME] [--uxplay-path PATH] [--video-sink SINK] [--seconds N]
  cometcast-cli airplay-send list | pair | play | photo | screen (details at the end)
  cometcast-cli miracast [--name NAME]
  cometcast-cli cast list [--seconds N]
  cometcast-cli cast send <DEVICE-NAME|IP[:PORT]> [--screen] [--display :N] [--fps N]
                    [--bitrate-kbps N] [--seconds N] [--trust-anchors PEM]
                    [--insecure-skip-device-auth]
  cometcast-cli discover [--seconds N] [--policy-file PATH] [--announce HOST:PORT]
  cometcast-cli docs info|render ... (PDF pages, see `docs --help`)
  cometcast-cli --version

Options:
  --server ADDRESS    Signalling server: HOST:PORT (plain ws://), or ws://, wss://, http://,
                      https:// URL (default 127.0.0.1:8443). Servers that require proof-of-work
                      (open mode) are handled automatically; STUN/TURN servers are fetched
                      from the server when it offers them.
  --ca-file PEM       Also trust the certificate(s) in this PEM file for wss:// / https://
                      (for a self-signed test server; the system store is always used too)
  --seconds N         send: stop after N seconds (default: until Ctrl-C)
  --screen            send: send the screen (X11 only) as H.264 instead of the test pattern
  --display NAME      send --screen: X11 display such as :99 (default: $DISPLAY)
  --fps N             send --screen: how often the screen is looked at (default 15)
  --bitrate-kbps N    send --screen: encoder target bit rate (default 2000)
  --decode            receive: decode H.264 video with OpenH264 and print the picture size and
                      mean colour once per second (proof that the stream decodes; no display)
  --bind ADDR         Local UDP address for WebRTC (default 0.0.0.0:0; use 127.0.0.1:0 for same-machine tests)

  --name NAME         airplay: receiver name shown on iPhone, iPad and Mac (default: Comet Cast)
  --uxplay-path PATH  airplay: the uxplay program (default: uxplay from PATH)
  --video-sink SINK   airplay: GStreamer video sink, e.g. fakesink for a headless test
  (--seconds N also works with airplay: stop after N seconds)
  --name NAME         miracast: receiver name shown on Windows senders (default: Comet Cast)

discover prints the server list (policy, DNS-SRV, announcement, default list) with the source of each
entry, and the receivers seen on the network. --announce advertises a server as a receiver would.
See docs/discovery.md.

Exit codes: 0 ok, 1 error, 2 bad arguments, 3 the peer left before any video was exchanged.

airplay starts UxPlay (a separate GPL program that must be installed) and prints what it
reports: server ready, the PIN of each connection, connects, errors. See docs/airplay.md.

cast (BETA, never tried on a real device) finds Chromecast / Google Cast devices (`cast list`) and
mirrors the X11 screen to one as H.264 (`cast send`). The device certificate chain is checked against
the roots in --trust-anchors PEM (this program embeds none); --insecure-skip-device-auth skips that
check, prints a warning and lets anybody who answers at the address receive the screen. See docs/cast.md.
airplay-send is the AirPlay sender without FairPlay (discovery, pairing, media link, photo, the
screen as an HLS stream). Its commands are listed below; see docs/airplay-sender.md.

miracast (Windows 11 only; compile-checked, never run) listens through the Windows
MiracastReceiver API and prints events. A connection request is answered on stdin: type y or
n and Enter (no answer within 30 seconds = rejected). On other systems it prints a message
and exits with code 2. See docs/miracast.md.

receive prints the room code, then one 'stats' line per second. It exits when the sender
leaves or on Ctrl-C. send streams a built-in VP8 test pattern, or with --screen the real screen as H.264 (no
mouse pointer, no audio, no Wayland; see docs/capture.md).";

const DEFAULT_SERVER: &str = "127.0.0.1:8443";

enum Command {
    Receive {
        common: Common,
        decode: bool,
    },
    Send {
        code: RoomCode,
        seconds: Option<u64>,
        common: Common,
        screen: Option<ScreenArgs>,
    },
    Airplay(AirplayArgs),
    Cast(cast::CastCmd),
    Discover(discover::DiscoverArgs),
    Docs(docs::DocsArgs),
    AirplaySend(airplay_send::Sub),
    Miracast {
        name: Option<String>,
    },
    Version,
    Help,
    DocsHelp,
}

struct AirplayArgs {
    name: Option<String>,
    uxplay_path: Option<String>,
    video_sink: Option<String>,
    seconds: Option<u64>,
}

fn parse_airplay(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut out = AirplayArgs {
        name: None,
        uxplay_path: None,
        video_sink: None,
        seconds: None,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--name" => out.name = Some(args.next().ok_or("--name needs a value")?),
            "--uxplay-path" => {
                out.uxplay_path = Some(args.next().ok_or("--uxplay-path needs a value")?);
            }
            "--video-sink" => {
                out.video_sink = Some(args.next().ok_or("--video-sink needs a value")?);
            }
            "--seconds" => {
                let v = args.next().ok_or("--seconds needs a value")?;
                match v.parse::<u64>() {
                    Ok(n) if n > 0 => out.seconds = Some(n),
                    _ => return Err(format!("bad --seconds value {v:?}")),
                }
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unexpected argument {other:?} for airplay")),
        }
    }
    Ok(Command::Airplay(out))
}

struct ScreenArgs {
    display: Option<String>,
    fps: u32,
    bitrate_kbps: u32,
}

fn parse_miracast(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut name = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--name" => name = Some(args.next().ok_or("--name needs a value")?),
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unexpected argument {other:?} for miracast")),
        }
    }
    Ok(Command::Miracast { name })
}

struct Common {
    server: String,
    bind: Option<String>,
    ca_file: Option<String>,
}

/// Read the optional extra trusted certificates.
fn load_trust(ca_file: Option<&str>) -> Result<TlsTrust, String> {
    let mut trust = TlsTrust::system();
    if let Some(path) = ca_file {
        let pem = std::fs::read(path).map_err(|e| format!("cannot read --ca-file {path}: {e}"))?;
        trust
            .add_pem(&pem)
            .map_err(|e| format!("--ca-file {path}: {e}"))?;
    }
    Ok(trust)
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(cmd) = args.next() else {
        return Ok(Command::Help);
    };
    let mut common = Common {
        server: DEFAULT_SERVER.to_string(),
        bind: None,
        ca_file: None,
    };
    let mut seconds = None;
    let mut positional = Vec::new();
    let mut screen = false;
    let mut decode = false;
    let mut display = None;
    let mut fps = None;
    let mut bitrate_kbps = None;
    match cmd.as_str() {
        "--version" | "-V" => return Ok(Command::Version),
        "--help" | "-h" | "help" => return Ok(Command::Help),
        "airplay" => return parse_airplay(args),
        "airplay-send" => return airplay_send::parse(args).map(Command::AirplaySend),
        "miracast" => return parse_miracast(args),
        "cast" => return Ok(cast::parse(args)?.map_or(Command::Help, Command::Cast)),
        "discover" => return Ok(discover::parse(args)?.map_or(Command::Help, Command::Discover)),
        "docs" => return Ok(docs::parse(args)?.map_or(Command::DocsHelp, Command::Docs)),
        "receive" | "send" => {}
        other => return Err(format!("unknown command {other:?}")),
    }
    while let Some(a) = args.next() {
        match a.as_str() {
            "--server" => common.server = args.next().ok_or("--server needs a value")?,
            "--bind" => common.bind = Some(args.next().ok_or("--bind needs a value")?),
            "--ca-file" => common.ca_file = Some(args.next().ok_or("--ca-file needs a value")?),
            "--seconds" => {
                let v = args.next().ok_or("--seconds needs a value")?;
                let n = v
                    .parse::<u64>()
                    .map_err(|_| format!("bad --seconds value {v:?}"))?;
                if n == 0 {
                    return Err("--seconds must be at least 1".into());
                }
                seconds = Some(n);
            }
            "--screen" => screen = true,
            "--decode" => decode = true,
            "--display" => display = Some(args.next().ok_or("--display needs a value")?),
            "--fps" => {
                let v = args.next().ok_or("--fps needs a value")?;
                match v.parse::<u32>() {
                    Ok(n) if (1..=60).contains(&n) => fps = Some(n),
                    _ => return Err(format!("bad --fps value {v:?} (1 to 60)")),
                }
            }
            "--bitrate-kbps" => {
                let v = args.next().ok_or("--bitrate-kbps needs a value")?;
                match v.parse::<u32>() {
                    Ok(n) if (50..=100_000).contains(&n) => bitrate_kbps = Some(n),
                    _ => return Err(format!("bad --bitrate-kbps value {v:?} (50 to 100000)")),
                }
            }
            "--help" | "-h" => return Ok(Command::Help),
            s if s.starts_with("--") => return Err(format!("unknown option {s:?}")),
            _ => positional.push(a),
        }
    }
    if cmd == "receive" {
        if !positional.is_empty()
            || seconds.is_some()
            || screen
            || display.is_some()
            || fps.is_some()
            || bitrate_kbps.is_some()
        {
            return Err("receive takes only --server, --bind, --ca-file and --decode".into());
        }
        return Ok(Command::Receive { common, decode });
    }
    if decode {
        return Err("--decode belongs to receive".into());
    }
    if !screen && (display.is_some() || fps.is_some() || bitrate_kbps.is_some()) {
        return Err("--display, --fps and --bitrate-kbps need --screen".into());
    }
    let [code] = positional.as_slice() else {
        return Err("send needs exactly one room code".into());
    };
    let code = RoomCode::parse(code).map_err(|e| format!("bad room code: {e}"))?;
    Ok(Command::Send {
        code,
        seconds,
        common,
        screen: screen.then(|| ScreenArgs {
            display,
            fps: fps.unwrap_or(15),
            bitrate_kbps: bitrate_kbps.unwrap_or(2000),
        }),
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let cmd = match parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match cmd {
        Command::Version => {
            println!("cometcast-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Help => {
            println!("{USAGE}\n\n{}", airplay_send::USAGE);
            ExitCode::SUCCESS
        }
        Command::DocsHelp => {
            println!("{}", docs::USAGE);
            ExitCode::SUCCESS
        }
        Command::Docs(d) => docs::run(d),
        Command::Airplay(a) => airplay(a).await,
        Command::AirplaySend(a) => airplay_send::run(a).await,
        Command::Miracast { name } => miracast(name).await,
        Command::Cast(c) => cast::run(c).await,
        Command::Discover(d) => discover::run(d).await,
        Command::Receive { common, decode } => receive(common, decode).await,
        Command::Send {
            code,
            seconds,
            common,
            screen,
        } => send(code, seconds, common, screen).await,
    }
}

/// Miracast receiving exists only on Windows.
#[cfg(not(windows))]
#[allow(clippy::unused_async)]
async fn miracast(_name: Option<String>) -> ExitCode {
    eprintln!(
        "miracast: Windows only. Miracast receiving uses the Windows 11 MiracastReceiver API \
         and is not available on this system. See docs/miracast.md."
    );
    ExitCode::from(2)
}

/// Run the Miracast receiver and print its events until Ctrl-C.
/// COMPILE-CHECKED ONLY: never run on a real Windows machine.
#[cfg(windows)]
async fn miracast(name: Option<String>) -> ExitCode {
    use std::io::IsTerminal;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};

    use cc_miracast::{
        ConnectionId, MiracastConfig, MiracastError, MiracastEvent, MiracastReceiver,
        MiracastStatus, status_after,
    };

    let mut config = MiracastConfig::default();
    if let Some(n) = name {
        config.display_name = n;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = Arc::clone(&stop);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        stop2.store(true, Ordering::SeqCst);
    });
    let join = tokio::task::spawn_blocking(move || {
        let (receiver, events) = match MiracastReceiver::start(config) {
            Ok(v) => v,
            Err(MiracastError::Unsupported) => {
                eprintln!("error: this Windows system reports no Miracast receiver support");
                return ExitCode::from(2);
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        };
        println!("Miracast receiver started (Ctrl-C to quit).");
        // Confirmation is typed by a person. Piped or redirected input (`yes | ...`) must
        // never count as an answer.
        let interactive = std::io::stdin().is_terminal();
        if !interactive {
            println!(
                "stdin is not a terminal: confirming a connection needs an interactive \
                 terminal, so every connection request will be rejected."
            );
        }
        // Answers to connection requests come from stdin lines.
        let (line_tx, lines) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut buf = String::new();
            while std::io::stdin().read_line(&mut buf).is_ok_and(|n| n > 0) {
                if line_tx.send(buf.trim().to_string()).is_err() {
                    break;
                }
                buf.clear();
            }
        });
        let mut status = MiracastStatus::Starting;
        let mut asking: Option<ConnectionId> = None;
        while !stop.load(Ordering::SeqCst) {
            if let Ok(line) = lines.try_recv()
                && let Some(id) = asking.take()
            {
                let accept = matches!(line.to_ascii_lowercase().as_str(), "y" | "yes");
                if let Err(e) = receiver.respond(id, accept) {
                    println!("answer failed: {e}");
                }
            }
            match events.recv_timeout(Duration::from_millis(200)) {
                Ok(ev) => {
                    match &ev {
                        MiracastEvent::ConnectionRequested {
                            id,
                            device_name,
                            pin,
                        } => {
                            println!("event: connection requested by {device_name:?}");
                            if let Some(pin) = pin {
                                println!("PIN to show/enter: {pin}");
                            }
                            if interactive {
                                // Throw away anything typed before this request.
                                while lines.try_recv().is_ok() {}
                                asking = Some(*id);
                                println!("Accept? [y/N] (rejected after 30 seconds)");
                            } else if let Err(e) = receiver.respond(*id, false) {
                                println!("answer failed: {e}");
                            }
                        }
                        MiracastEvent::Disconnected { id } if asking == Some(*id) => asking = None,
                        _ => println!("event: {ev:?}"),
                    }
                    let next = status_after(status, &ev);
                    if next != status {
                        println!("status: {next:?}");
                        status = next;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        println!("Stopping.");
        receiver.stop();
        if status == MiracastStatus::Failed {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    });
    join.await.unwrap_or(ExitCode::FAILURE)
}

/// Run UxPlay under the supervisor and print its events until Ctrl-C (or --seconds).
async fn airplay(args: AirplayArgs) -> ExitCode {
    let mut config = AirplayConfig::default();
    if let Some(n) = args.name {
        config.name = n;
    }
    if let Some(p) = args.uxplay_path {
        config.uxplay_path = p.into();
    }
    config.video_sink = args.video_sink;
    // Ask the installed program which of our flags it does not list in its help text.
    // The probe gets 3 seconds; a program that hangs is killed.
    if let Ok(flags) = config.build_args() {
        let path = config.uxplay_path.clone();
        let help = tokio::task::spawn_blocking(move || help_text(&path))
            .await
            .ok()
            .flatten();
        if let Some(help) = help {
            let missing = cc_airplay::unsupported_flags(&help, &flags);
            if !missing.is_empty() {
                eprintln!("warning: this uxplay does not list these options in -h: {missing:?}");
            }
        }
    }
    let (mut sup, rx) = match Supervisor::start(config, SupervisorOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    // The supervisor uses plain threads; hand its events over to this async code.
    let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            if tx.send(ev).is_err() {
                break;
            }
        }
    });
    println!("Starting UxPlay (Ctrl-C to quit)...");
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    let limit = args
        .seconds
        .map_or(Duration::from_secs(60 * 60 * 24 * 365), Duration::from_secs);
    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);
    let mut status = AirplayStatus::Off;
    let mut failed = false;
    let mut stopping = false;
    loop {
        tokio::select! {
            () = &mut deadline, if !stopping => {
                stopping = true;
                sup.request_stop();
            }
            _ = &mut ctrl_c, if !stopping => {
                println!("Interrupted, stopping UxPlay.");
                stopping = true;
                sup.request_stop();
            }
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                println!("event: {ev:?}");
                let next = cc_airplay::status_after(&status, &ev);
                if next != status {
                    println!("status: {next:?}");
                    status = next;
                }
                match ev {
                    AirplayEvent::Failed(_) => failed = true,
                    AirplayEvent::Stopped => break,
                    _ => {}
                }
            }
        }
    }
    // Waits until the process is gone and reaped.
    sup.stop();
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Run `PROGRAM -h` and return its output, or `None` if it fails or takes over 3 seconds.
fn help_text(path: &std::path::Path) -> Option<String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = std::process::Command::new(path)
        .arg("-h")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let end = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    // The help text is small, so it fits in the pipes while we waited.
    let mut text = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut text);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut text);
    }
    Some(text)
}

/// One line about the STUN/TURN servers the signalling server handed out (nothing if none).
fn print_ice_servers(list: &[IceServerConfig]) {
    if list.is_empty() {
        return;
    }
    let turn = list
        .iter()
        .filter(|s| s.username.is_some() && s.credential.is_some())
        .count();
    println!(
        "ICE servers from the server: {} ({} with TURN credentials)",
        list.len(),
        turn
    );
}

/// Decodes tapped H.264 packets and prints what came out (`receive --decode`).
struct DecodeCheck {
    assembler: cc_media::rtp::H264Assembler,
    decoder: cc_media::H264Decoder,
    units: u64,
    pictures: u64,
    last: Option<cc_media::DecodedFrame>,
}

impl DecodeCheck {
    fn new() -> Result<Self, String> {
        let decoder = cc_media::H264Decoder::new(&cc_media::H264Backend::from_env())
            .map_err(|e| format!("cannot start the H.264 decoder: {e}"))?;
        Ok(Self {
            assembler: cc_media::rtp::H264Assembler::new(),
            decoder,
            units: 0,
            pictures: 0,
            last: None,
        })
    }

    fn push(&mut self, p: &cc_transport::VideoPacket) {
        let Some(au) = self
            .assembler
            .push(&p.payload, p.marker, p.timestamp, p.sequence)
        else {
            return;
        };
        self.units += 1;
        if let Ok(Some(pic)) = self.decoder.decode(&au.data) {
            self.pictures += 1;
            self.last = Some(pic);
        }
    }

    fn report(&self) -> String {
        match &self.last {
            Some(p) => {
                let m = p.mean_rgb();
                format!(
                    "decoded units={} pictures={} last={}x{} mean_rgb=({:.0},{:.0},{:.0})",
                    self.units, self.pictures, p.width, p.height, m[0], m[1], m[2]
                )
            }
            None => format!(
                "decoded units={} pictures={} (no picture yet)",
                self.units, self.pictures
            ),
        }
    }
}

async fn receive(common: Common, decode: bool) -> ExitCode {
    let mut opts = ReceiverOptions::new(common.server);
    let mut checker = None;
    let mut tap_rx = None;
    let mut tap_handle: Option<cc_transport::VideoTap> = None;
    if decode {
        match DecodeCheck::new() {
            Ok(c) => checker = Some(c),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
        // Bounded: if decoding is slower than the packets arrive, packets are dropped and
        // counted instead of piling up in memory.
        let (tx, rx) = cc_transport::VideoTap::channel_default();
        opts.session.video_tap = Some(tx.clone());
        tap_handle = Some(tx);
        tap_rx = Some(rx);
    }
    opts.trust = match load_trust(common.ca_file.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(b) = common.bind {
        opts.session.udp_bind = vec![b];
    }
    let mut core = match ReceiverCore::start(opts).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Room code: {}", core.room_code());
    print_ice_servers(core.ice_servers());
    println!("Waiting for a sender (Ctrl-C to quit)...");
    let mut last_packets = 0;
    let mut got_video = false;
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                println!("Interrupted, closing.");
                core.stop().await;
                return ExitCode::SUCCESS;
            }
            Some(p) = async {
                match tap_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(c) = checker.as_mut() {
                    c.push(&p);
                }
            }
            ev = core.next_event() => match ev {
                Some(ReceiverEvent::PeerJoined) => println!("Sender joined."),
                Some(ReceiverEvent::State(s)) => println!("Connection: {s:?}"),
                Some(ReceiverEvent::VideoTrack { mime, .. }) => {
                    got_video = true;
                    println!("Video track: {mime}");
                }
                Some(ReceiverEvent::Stats(s)) => {
                    println!(
                        "stats packets={} bytes={} frames={} keyframes={} (+{} packets/s)",
                        s.packets, s.bytes, s.frames, s.keyframes, s.packets - last_packets
                    );
                    last_packets = s.packets;
                    if let Some(c) = checker.as_ref() {
                        println!("{}", c.report());
                    }
                    if let Some(dropped) = tap_handle.as_ref().map(|t| t.dropped()).filter(|d| *d > 0) {
                        println!("decoder too slow: {dropped} packets dropped");
                    }
                }
                Some(ReceiverEvent::PeerLeft) => {
                    println!("Sender left.");
                    core.stop().await;
                    return if got_video { ExitCode::SUCCESS } else { ExitCode::from(3) };
                }
                Some(ReceiverEvent::Error(e)) => {
                    eprintln!("error: {e}");
                    core.stop().await;
                    return ExitCode::FAILURE;
                }
                None => return ExitCode::FAILURE,
            }
        }
    }
}

async fn send(
    code: RoomCode,
    seconds: Option<u64>,
    common: Common,
    screen: Option<ScreenArgs>,
) -> ExitCode {
    let mut opts = SenderOptions::new(common.server, code);
    let opts_screen_note = screen.as_ref().map(|a| {
        format!(
            "Sending the screen of display {} as H.264 ({} fps, {} kbit/s).",
            a.display.as_deref().unwrap_or("$DISPLAY"),
            a.fps,
            a.bitrate_kbps
        )
    });
    let screen_mode = screen.is_some();
    opts.screen = screen.map(|a| ScreenOptions {
        display: a.display,
        fps: a.fps,
        bitrate_bps: a.bitrate_kbps.saturating_mul(1000),
        ..ScreenOptions::default()
    });
    opts.duration = seconds.map(Duration::from_secs);
    opts.trust = match load_trust(common.ca_file.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(b) = common.bind {
        opts.session.udp_bind = vec![b];
    }
    let mut core = match SenderCore::start(opts).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Joined the room, offer sent.");
    if let Some(s) = opts_screen_note {
        println!("{s}");
    }
    print_ice_servers(core.ice_servers());
    let mut connected = false;
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                println!("Interrupted, closing.");
                core.stop().await;
                return ExitCode::SUCCESS;
            }
            ev = core.next_event() => match ev {
                Some(SenderEvent::State(s)) => {
                    connected |= s == cc_transport::ConnectionState::Connected;
                    println!("Connection: {s:?}");
                }
                Some(SenderEvent::Streaming { frames_sent }) => {
                    if screen_mode {
                        let st = core.screen_stats();
                        println!(
                            "sent frames={frames_sent} polls={} changed={} unchanged={} refreshes={} keyframes={} key_requests={} bytes={} key_requests_ignored={} skipped={}",
                            st.polls, st.changed, st.unchanged, st.refreshes, st.keyframes, st.keyframe_requests, st.encoded_bytes,
                            st.keyframe_requests_ignored, st.skipped
                        );
                    } else {
                        println!("sent frames={frames_sent}");
                    }
                }
                Some(SenderEvent::Finished { frames_sent }) => {
                    println!("Done, sent {frames_sent} frames.");
                    core.stop().await;
                    return ExitCode::SUCCESS;
                }
                Some(SenderEvent::PeerLeft) => {
                    println!("Receiver left.");
                    core.stop().await;
                    return if connected { ExitCode::SUCCESS } else { ExitCode::from(3) };
                }
                Some(SenderEvent::Error(e)) => {
                    eprintln!("error: {e}");
                    core.stop().await;
                    return ExitCode::FAILURE;
                }
                None => return ExitCode::FAILURE,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Command, String> {
        parse_args(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_receive_and_send() {
        assert!(matches!(
            parse("receive --server a:1"),
            Ok(Command::Receive { common: c, .. }) if c.server == "a:1"
        ));
        let Ok(Command::Send { code, seconds, .. }) = parse("send k7m2qx --seconds 5") else {
            panic!("send did not parse");
        };
        assert_eq!(code.as_str(), "K7M2QX");
        assert_eq!(seconds, Some(5));
    }

    #[test]
    fn parses_screen_and_decode() {
        let Ok(Command::Send { screen, .. }) =
            parse("send k7m2qx --screen --display :99 --fps 10 --bitrate-kbps 800")
        else {
            panic!("send --screen did not parse");
        };
        let screen = screen.expect("screen mode");
        assert_eq!(screen.display.as_deref(), Some(":99"));
        assert_eq!((screen.fps, screen.bitrate_kbps), (10, 800));
        let Ok(Command::Send { screen, .. }) = parse("send k7m2qx --screen") else {
            panic!("send --screen did not parse");
        };
        let screen = screen.unwrap();
        assert_eq!(
            (screen.display, screen.fps, screen.bitrate_kbps),
            (None, 15, 2000)
        );
        assert!(matches!(
            parse("receive --decode"),
            Ok(Command::Receive { decode: true, .. })
        ));
    }

    #[test]
    fn parses_ca_file() {
        assert!(matches!(
            parse("receive --server https://h --ca-file /x/ca.pem"),
            Ok(Command::Receive { common: c, .. }) if c.ca_file.as_deref() == Some("/x/ca.pem")
        ));
        assert!(parse("receive --ca-file").is_err());
        assert!(load_trust(Some("/nonexistent/ca.pem")).is_err());
        assert!(load_trust(None).is_ok());
    }

    #[test]
    fn parses_airplay() {
        let Ok(Command::Airplay(a)) =
            parse("airplay --name Room1 --uxplay-path /x/uxplay --seconds 3")
        else {
            panic!("airplay did not parse");
        };
        assert_eq!(a.name.as_deref(), Some("Room1"));
        assert_eq!(a.uxplay_path.as_deref(), Some("/x/uxplay"));
        assert_eq!(a.seconds, Some(3));
        assert!(parse("airplay --nope").is_err());
        assert!(parse("airplay --name").is_err());
        assert!(parse("airplay --seconds 0").is_err());
    }

    #[test]
    fn parses_miracast() {
        assert!(matches!(
            parse("miracast --name Room1"),
            Ok(Command::Miracast { name: Some(n) }) if n == "Room1"
        ));
        assert!(matches!(
            parse("miracast"),
            Ok(Command::Miracast { name: None })
        ));
        assert!(parse("miracast --name").is_err());
        assert!(parse("miracast --nope").is_err());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("send").is_err());
        assert!(parse("send ABC").is_err());
        assert!(parse("send K7M2QX --seconds 0").is_err());
        assert!(parse("receive extra").is_err());
        assert!(parse("receive --nope").is_err());
        assert!(parse("dance").is_err());
        assert!(parse("receive --screen").is_err());
        assert!(parse("send K7M2QX --decode").is_err());
        assert!(parse("send K7M2QX --display :1").is_err());
        assert!(parse("send K7M2QX --screen --fps 0").is_err());
        assert!(parse("send K7M2QX --screen --fps 61").is_err());
        assert!(parse("send K7M2QX --screen --bitrate-kbps 1").is_err());
    }
}
