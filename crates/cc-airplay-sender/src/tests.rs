// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests of the sender against the TEST FAKE receiver (`testfake.rs`) and against
//! hostile servers. The tests that need `Xvfb` skip with a loud note if it is missing.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::client::{AuthKind, Session};
use crate::creds::Credentials;
use crate::discovery::feature;
use crate::error::SenderError;
use crate::http::Conn;
use crate::testfake::{FakeMode, FakeReceiver};
use crate::{hap, legacy};

fn hap_device(fake: &FakeReceiver) -> crate::Device {
    let mut d = fake.device();
    d.features = 1 << feature::SYSTEM_PAIRING;
    d
}

fn media_roundtrip(s: &mut Session, fake: &FakeReceiver) {
    s.play("http://192.0.2.1:8080/movie.mp4").unwrap();
    assert_eq!(
        fake.state.lock().unwrap().played_url.as_deref(),
        Some("http://192.0.2.1:8080/movie.mp4")
    );
    let info = s.playback_info().unwrap();
    assert_eq!(info.duration, Some(60.0));
    assert_eq!(info.ready_to_play, Some(true));
    s.scrub(12.5).unwrap();
    assert_eq!(s.scrub_position().unwrap().0, Some(60.0));
    assert!(s.scrub_position().unwrap().1.unwrap() >= 12.5);
    s.rate(0.0).unwrap();
    s.stop().unwrap();
    assert_eq!(s.playback_info().unwrap().duration, None);
    s.photo(vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]).unwrap();
    assert_eq!(
        fake.state.lock().unwrap().photo.as_deref(),
        Some(&[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3][..])
    );
}

#[test]
fn legacy_pin_pairing_verify_and_media() {
    let fake = FakeReceiver::start(FakeMode::Legacy, "4711", false);
    let dev = fake.device();

    // Without pairing the fake refuses media requests.
    let mut s = Session::connect(&dev).unwrap();
    assert!(matches!(
        s.play("http://192.0.2.1/a.mp4"),
        Err(SenderError::Status { status: 470, .. })
    ));

    // Wrong PIN.
    let mut s = Session::connect(&dev).unwrap();
    legacy::pair_pin_start(s.conn_mut()).unwrap();
    let err = legacy::pair_finish(s.conn_mut(), "0000").unwrap_err();
    assert!(matches!(err, SenderError::Auth(_)), "{err:?}");

    // Right PIN, then a new connection verifies with the stored credentials.
    let mut s = Session::connect(&dev).unwrap();
    legacy::pair_pin_start(s.conn_mut()).unwrap();
    let creds = legacy::pair_finish(s.conn_mut(), "4711").unwrap();
    assert!(fake.state.lock().unwrap().legacy_client_key.is_some());
    let mut s = Session::connect(&dev).unwrap();
    assert_eq!(
        s.authenticate(Some(&creds)).unwrap(),
        AuthKind::LegacyVerify
    );
    media_roundtrip(&mut s, &fake);

    // Credentials with a different key are refused by verify.
    let Credentials::Legacy { client_id, .. } = &creds else {
        panic!("legacy credentials expected")
    };
    let forged = Credentials::Legacy {
        client_id: client_id.clone(),
        seed: "07".repeat(32),
    };
    let mut s = Session::connect(&dev).unwrap();
    assert!(s.authenticate(Some(&forged)).is_err());
    assert!(fake.log().iter().any(|l| l == "POST /pair-verify"));
}

#[test]
fn dialect_probe_finds_http_and_rtsp_receivers() {
    use crate::client::Proto;
    let http = FakeReceiver::start(FakeMode::Open, "", false);
    assert_eq!(
        Session::connect(&http.device()).unwrap().proto(),
        Proto::Http
    );

    // Like UxPlay: only RTSP/1.0 is answered. Pairing and media then run over RTSP.
    let rtsp = FakeReceiver::start_with(FakeMode::Legacy, "4711", false, true);
    let dev = rtsp.device();
    let mut s = Session::connect(&dev).unwrap();
    assert_eq!(s.proto(), Proto::Rtsp);
    legacy::pair_pin_start(s.conn_mut()).unwrap();
    let creds = legacy::pair_finish(s.conn_mut(), "4711").unwrap();
    let mut s = Session::connect(&dev).unwrap();
    s.authenticate(Some(&creds)).unwrap();
    media_roundtrip(&mut s, &rtsp);
    // Forcing HTTP against it fails instead of hanging for long.
    let start = Instant::now();
    let mut s = Session::connect_with(&dev, Proto::Http).unwrap();
    assert!(s.server_info().is_err());
    assert!(start.elapsed() < Duration::from_secs(15));
}

#[test]
fn raop_port_is_the_fallback_when_the_advertised_port_refuses() {
    // Like UxPlay 1.68: the advertised port (here: a closed one) is not the listening one.
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let fake = FakeReceiver::start(FakeMode::Open, "", false);
    let mut dev = fake.device();
    let real = dev.port;
    dev.port = closed;
    assert!(Session::connect(&dev).is_err());
    dev.raop_port = Some(real);
    let mut s = Session::connect(&dev).unwrap();
    s.play("http://192.0.2.1/a.mp4").unwrap();
}

#[test]
fn hap_transient_pairing_then_encrypted_media() {
    let fake = FakeReceiver::start(FakeMode::Hap, "1234", false);
    let dev = hap_device(&fake);

    let mut s = Session::connect(&dev).unwrap();
    assert!(matches!(
        s.play("http://192.0.2.1/a.mp4"),
        Err(SenderError::Status { status: 470, .. })
    ));

    let mut s = Session::connect(&dev).unwrap();
    assert_eq!(s.authenticate(None).unwrap(), AuthKind::HapTransient);
    assert!(s.conn_mut().framed_mut().is_encrypted());
    media_roundtrip(&mut s, &fake);
}

#[test]
fn hap_pin_pairing_then_verify() {
    let fake = FakeReceiver::start(FakeMode::Hap, "9021", false);
    let dev = hap_device(&fake);

    // Wrong PIN: the fake answers with an error item.
    let mut s = Session::connect(&dev).unwrap();
    let pending = hap::setup_pin_start(s.conn_mut()).unwrap();
    let err = hap::setup_pin_finish(s.conn_mut(), pending, "0000").unwrap_err();
    assert!(matches!(err, SenderError::Auth(_)), "{err:?}");

    let mut s = Session::connect(&dev).unwrap();
    let pending = hap::setup_pin_start(s.conn_mut()).unwrap();
    let (creds, accessory_signature_ok) =
        hap::setup_pin_finish(s.conn_mut(), pending, "9021").unwrap();
    assert!(accessory_signature_ok);
    assert!(fake.state.lock().unwrap().hap_client_key.is_some());

    let mut s = Session::connect(&dev).unwrap();
    assert_eq!(s.authenticate(Some(&creds)).unwrap(), AuthKind::HapVerify);
    media_roundtrip(&mut s, &fake);

    // A receiver whose key differs from the stored one is rejected.
    let Credentials::Hap {
        pairing_id,
        seed,
        device_id,
        ..
    } = creds
    else {
        panic!("hap credentials expected")
    };
    let wrong_key = Credentials::Hap {
        pairing_id,
        seed,
        device_id,
        device_key: crate::cryptoutil::hex(
            &ed25519_dalek::SigningKey::from_bytes(&[5; 32])
                .verifying_key()
                .to_bytes(),
        ),
    };
    let mut s = Session::connect(&dev).unwrap();
    assert!(matches!(
        s.authenticate(Some(&wrong_key)),
        Err(SenderError::Auth(_))
    ));
}

#[test]
fn open_receiver_needs_no_pairing() {
    let fake = FakeReceiver::start(FakeMode::Open, "", false);
    let mut s = Session::connect(&fake.device()).unwrap();
    assert_eq!(s.authenticate(None).unwrap(), AuthKind::None);
    let info = s.server_info().unwrap();
    assert!(info.contains_key("model"));
    media_roundtrip(&mut s, &fake);
}

/// A server that answers each request with the next canned byte string.
fn evil_server(answers: Vec<Vec<u8>>) -> crate::Device {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            for a in answers {
                let mut buf = [0u8; 65536];
                if s.read(&mut buf).unwrap_or(0) == 0 {
                    return;
                }
                if s.write_all(&a).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    });
    crate::Device::from_address(&addr.to_string()).unwrap()
}

fn http_ok(body: &[u8]) -> Vec<u8> {
    let mut v = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    v.extend_from_slice(body);
    v
}

#[test]
fn hostile_receivers_give_errors_not_panics() {
    // Garbage instead of HTTP.
    let dev = evil_server(vec![b"\x00\x01\x02garbage".to_vec()]);
    let mut s = Session::connect_with(&dev, crate::client::Proto::Http).unwrap();
    assert!(s.server_info().is_err());

    // Endless body announced.
    let dev = evil_server(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 99999999999\r\n\r\n".to_vec(),
    ]);
    let mut s = Session::connect_with(&dev, crate::client::Proto::Http).unwrap();
    assert!(matches!(
        s.server_info(),
        Err(SenderError::TooLarge(_)) | Err(SenderError::Protocol(_))
    ));

    // Legacy pairing: B = 0 and a bogus salt.
    let mut d = plist::Dictionary::new();
    d.insert("pk".into(), plist::Value::Data(vec![0; 256]));
    d.insert("salt".into(), plist::Value::Data(vec![1; 16]));
    let body = crate::plistx::encode(d).unwrap();
    let dev = evil_server(vec![http_ok(b""), http_ok(&body)]);
    let mut s = Session::connect_with(&dev, crate::client::Proto::Http).unwrap();
    legacy::pair_pin_start(s.conn_mut()).unwrap();
    assert!(legacy::pair_finish(s.conn_mut(), "1234").is_err());

    // A plist that is not a dictionary, and a plist bomb of nested arrays.
    let mut nested = plist::Value::Array(vec![]);
    for _ in 0..200 {
        nested = plist::Value::Array(vec![nested]);
    }
    let mut bomb = Vec::new();
    let _ = nested.to_writer_binary(&mut bomb);
    let dev = evil_server(vec![http_ok(&bomb)]);
    let mut s = Session::connect_with(&dev, crate::client::Proto::Http).unwrap();
    assert!(s.server_info().is_err());

    // HomeKit pairing: TLV with an oversize/truncated item.
    let dev = evil_server(vec![http_ok(b""), http_ok(&[6, 1, 2, 3, 200, 1])]);
    let mut s = Session::connect_with(&dev, crate::client::Proto::Http).unwrap();
    assert!(hap::setup_pin_start(s.conn_mut()).is_err());

    // Server that closes at once.
    let dev = evil_server(vec![]);
    let mut s = Session::connect_with(&dev, crate::client::Proto::Http).unwrap();
    assert!(s.server_info().is_err());
}

#[test]
fn silent_peer_times_out() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let t = std::thread::spawn(move || {
        let (s, _) = l.accept().unwrap();
        std::thread::sleep(Duration::from_secs(2));
        drop(s);
    });
    let s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let mut c = Conn::new(s);
    let start = Instant::now();
    let r = c.exchange(&crate::http::Request::new("GET", "/server-info"));
    assert!(matches!(r, Err(SenderError::Timeout(_))), "{r:?}");
    assert!(start.elapsed() < Duration::from_secs(2));
    t.join().unwrap();
}

/// The screen as HLS: Xvfb with a red window, capture, OpenH264, TS segments, our HLS
/// server, and the fake receiver fetching what `/play` pointed it to. If `ffmpeg` is there,
/// the fetched segments are decoded and the picture is checked.
#[test]
fn screen_as_hls_stream_reaches_the_fake_receiver() {
    use cc_capture::testutil::{TestWindow, TestXServer};
    let Some(x) = TestXServer::start(640, 480) else {
        return;
    };
    let _win = TestWindow::open(&x.display, (0, 0, 640, 480), [200, 30, 30]).unwrap();

    let fake = FakeReceiver::start(FakeMode::Open, "", true);
    let mut s = Session::connect(&fake.device()).unwrap();
    s.authenticate(None).unwrap();
    let opts = crate::screen::ScreenOptions {
        display: Some(x.display.clone()),
        segment_ms: 1000,
        ..Default::default()
    };
    let mut cast = crate::screen::start_cast(&mut s, &opts).unwrap();
    assert!(cast.url.ends_with("/live.m3u8"));
    let end = Instant::now() + Duration::from_secs(12);
    while fake.state.lock().unwrap().fetched.len() < 3 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(200));
    }
    cast.stop();
    let fetched = fake.state.lock().unwrap().fetched.clone();
    assert!(
        fetched.len() >= 3,
        "only {} segments fetched",
        fetched.len()
    );
    let mut all = Vec::new();
    for (name, data) in &fetched {
        assert!(name.starts_with("seg") && name.ends_with(".ts"));
        assert_eq!(data.len() % 188, 0, "{name} is not whole TS packets");
        assert!(data.chunks(188).all(|p| p[0] == 0x47), "{name} lost sync");
        all.extend_from_slice(data);
    }
    assert!(
        cast.streamer
            .stats
            .frames
            .load(std::sync::atomic::Ordering::Relaxed)
            > 10
    );

    check_with_ffmpeg(&all);
}

fn check_with_ffmpeg(ts: &[u8]) {
    let dir = std::env::temp_dir().join(format!("cc-airplay-sender-ff-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("out.ts");
    std::fs::write(&file, ts).unwrap();
    let probe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(&file)
        .output();
    let Ok(probe) = probe else {
        eprintln!("NOTE: SKIPPING ffprobe CHECK, ffprobe is not installed");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let text = String::from_utf8_lossy(&probe.stdout).to_string();
    assert!(text.contains("h264,640,480"), "ffprobe said {text:?}");
    let raw = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&file)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .unwrap();
    assert!(
        raw.status.success(),
        "ffmpeg failed: {}",
        String::from_utf8_lossy(&raw.stderr)
    );
    assert_eq!(raw.stdout.len(), 640 * 480 * 3);
    let mid = (240 * 640 + 320) * 3;
    let px = &raw.stdout[mid..mid + 3];
    assert!(
        px[0] > 150 && px[1] < 90 && px[2] < 90,
        "decoded picture is not the red window: {px:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = Arc::new(());
}
