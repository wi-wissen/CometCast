// SPDX-License-Identifier: GPL-3.0-or-later
//! Interop test against the REAL UxPlay (a separate GPL program, started as a child process;
//! nothing of it is linked or copied). Skips with a loud note if `uxplay` or `Xvfb` is not
//! installed or UxPlay does not start. mDNS is not used: UxPlay is started with `-p PORT`
//! and the test connects to that TCP port directly.
//!
//! What it checks: dialect detection (UxPlay answers only RTSP/1.0), `pair-pin-start`,
//! legacy PIN pairing with the fixed PIN given by `-pin`, rejection of a wrong PIN,
//! pair-verify with the stored credentials, and that `/play` is acknowledged. UxPlay 1.68
//! does not implement `/play` (it answers 200 with an empty body and does nothing), so the
//! test does NOT claim that anything is played.
#![allow(missing_docs)]

use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use cc_airplay_sender::{AuthKind, Device, Proto, SenderError, Session, legacy};
use cc_capture::testutil::TestXServer;

struct Uxplay(Child);

impl Drop for Uxplay {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[test]
fn pairs_with_the_real_uxplay_and_gets_play_acknowledged() {
    let Some(x) = TestXServer::start(640, 480) else {
        eprintln!("NOTE: SKIPPING UXPLAY INTEROP TEST, `Xvfb` is not installed");
        return;
    };
    // UxPlay's -p N listens on TCP N (and uses N+1, N+2 and UDP ports N-1000+... for others).
    let port = free_port();
    let child = Command::new("uxplay")
        .args([
            "-n",
            "CCInteropTest",
            "-vs",
            "fakesink",
            "-as",
            "0",
            "-pin",
            "4711",
            "-p",
        ])
        .arg(port.to_string())
        .env("DISPLAY", &x.display)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(child) = child else {
        eprintln!("NOTE: SKIPPING UXPLAY INTEROP TEST, `uxplay` is not installed");
        return;
    };
    let _guard = Uxplay(child);
    // Depending on the UxPlay build, `-p N` listens on N or on N+1 (observed: N+1 with 1.68.2
    // from Ubuntu). Use whichever answers.
    let end = Instant::now() + Duration::from_secs(10);
    let port = loop {
        if let Some(p) = [port, port + 1]
            .into_iter()
            .find(|p| TcpStream::connect(("127.0.0.1", *p)).is_ok())
        {
            break p;
        }
        if Instant::now() > end {
            eprintln!("NOTE: SKIPPING UXPLAY INTEROP TEST, uxplay did not open port {port}");
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    let dev = Device::from_address(&format!("127.0.0.1:{port}")).unwrap();

    let mut s = Session::connect(&dev).unwrap();
    assert_eq!(s.proto(), Proto::Rtsp, "UxPlay answers only RTSP/1.0");
    legacy::pair_pin_start(s.conn_mut()).unwrap();
    let wrong = legacy::pair_finish(s.conn_mut(), "0000");
    assert!(matches!(wrong, Err(SenderError::Auth(_))), "{wrong:?}");

    drop(s);
    let mut s = Session::connect(&dev).unwrap();
    legacy::pair_pin_start(s.conn_mut()).unwrap();
    let creds = legacy::pair_finish(s.conn_mut(), "4711").unwrap();

    drop(s);
    let mut s = Session::connect(&dev).unwrap();
    assert_eq!(
        s.authenticate(Some(&creds)).unwrap(),
        AuthKind::LegacyVerify
    );
    s.play("http://192.0.2.1:8080/live.m3u8").unwrap();
    let info = s.playback_info().unwrap();
    // UxPlay 1.68 has no media player: no duration, no position.
    assert_eq!(info.duration, None);
    s.stop().unwrap();
}
