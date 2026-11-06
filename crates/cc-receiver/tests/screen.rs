// SPDX-License-Identifier: GPL-3.0-or-later
//! Real screen content end to end: a private Xvfb with a coloured window, the X11 capture, the
//! H.264 encoder, WebRTC over loopback UDP through a real cc-server in this process, and the
//! native receiver. The received RTP packets are put back together and decoded with the
//! test-only OpenH264 decoder, and the decoded picture is compared with what was drawn.
//!
//! Needs `Xvfb` on PATH; without it the tests print a note and return (see
//! `cc_capture::testutil`).

use std::time::{Duration, Instant};

use cc_capture::testutil::{TestWindow, TestXServer};
use cc_media::rtp::H264Assembler;
use cc_media::{DecodedFrame, H264Backend, H264Decoder};
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{ScreenOptions, SenderCore, SenderEvent, SenderOptions};
use cc_server::{ServerConfig, run};
use cc_transport::{SessionConfig, VideoCodec, VideoPacket};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run(
        listener,
        ServerConfig::default(),
        std::future::pending(),
    ));
    addr.to_string()
}

fn local_session() -> SessionConfig {
    SessionConfig {
        udp_bind: vec!["127.0.0.1:0".into()],
        ..SessionConfig::default()
    }
}

/// Turns tapped RTP packets into decoded pictures.
struct Viewer {
    rx: mpsc::UnboundedReceiver<VideoPacket>,
    assembler: H264Assembler,
    decoder: H264Decoder,
    units: u64,
    last: Option<DecodedFrame>,
    /// Codec reported by the receiver's `VideoTrack` event.
    codec: Option<(Option<VideoCodec>, String)>,
}

impl Viewer {
    fn new(rx: mpsc::UnboundedReceiver<VideoPacket>) -> Self {
        Self {
            rx,
            assembler: H264Assembler::new(),
            decoder: H264Decoder::new(&H264Backend::Source).unwrap(),
            units: 0,
            last: None,
            codec: None,
        }
    }

    /// Decode everything that arrived so far.
    fn drain(&mut self) {
        while let Ok(p) = self.rx.try_recv() {
            if let Some(au) = self
                .assembler
                .push(&p.payload, p.marker, p.timestamp, p.sequence)
            {
                self.units += 1;
                if let Ok(Some(pic)) = self.decoder.decode(&au.data) {
                    self.last = Some(pic);
                }
            }
        }
    }
}

/// Keep both cores moving (their event channels must be read) and decode, until `done`
/// returns true or the time is up.
async fn pump(
    receiver: &mut ReceiverCore,
    sender: &mut SenderCore,
    viewer: &mut Viewer,
    limit: Duration,
    mut done: impl FnMut(&Viewer, &SenderCore) -> bool,
) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        tokio::select! {
            ev = receiver.next_event() => match ev {
                Some(ReceiverEvent::Error(e)) => panic!("receiver error: {e}"),
                None => panic!("receiver ended"),
                Some(ReceiverEvent::VideoTrack { codec, mime }) => viewer.codec = Some((codec, mime)),
                Some(_) => {}
            },
            ev = sender.next_event() => match ev {
                Some(SenderEvent::Error(e)) => panic!("sender error: {e}"),
                None => panic!("sender ended"),
                Some(_) => {}
            },
            () = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        viewer.drain();
        if done(viewer, sender) {
            return true;
        }
    }
    false
}

fn near(pic: &DecodedFrame, rgb: [u8; 3]) -> bool {
    pic.share_near(rgb, 25) > 0.9
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn x11_screen_reaches_the_receiver_as_h264_and_goes_quiet_when_static() {
    let Some(x) = TestXServer::start(640, 480) else {
        return;
    };
    const RED: [u8; 3] = [200, 30, 40];
    const BLUE: [u8; 3] = [20, 160, 220];
    let window = TestWindow::open(&x.display, (0, 0, 640, 480), RED).unwrap();

    let server = start_server().await;
    let (tap_tx, tap_rx) = mpsc::unbounded_channel();
    let mut ropts = ReceiverOptions::new(&server);
    ropts.session = local_session();
    ropts.session.video_tap = Some(tap_tx);
    let mut receiver = ReceiverCore::start(ropts).await.expect("receiver start");

    let mut sopts = SenderOptions::new(&server, receiver.room_code().clone());
    sopts.session = local_session();
    sopts.screen = Some(ScreenOptions {
        display: Some(x.display.clone()),
        fps: 15,
        ..ScreenOptions::default()
    });
    let mut sender = SenderCore::start(sopts).await.expect("sender start");
    let mut viewer = Viewer::new(tap_rx);

    // 1. The first picture arrives, is H.264, and has the colour of the window.
    let got = pump(
        &mut receiver,
        &mut sender,
        &mut viewer,
        Duration::from_secs(20),
        |v, _| v.last.as_ref().is_some_and(|p| near(p, RED)),
    )
    .await;
    assert!(
        got,
        "no red picture decoded; units={} mean={:?}",
        viewer.units,
        viewer.last.as_ref().map(DecodedFrame::mean_rgb)
    );
    let pic = viewer.last.clone().unwrap();
    assert_eq!((pic.width, pic.height), (640, 480));
    let mean = pic.mean_rgb();
    for (m, w) in mean.iter().zip(RED) {
        assert!((m - f64::from(w)).abs() < 12.0, "mean {mean:?} vs {RED:?}");
    }

    // The receiver announced H.264.
    // (The event may still be queued: read events for a moment.)
    let end = Instant::now() + Duration::from_secs(3);
    while viewer.codec.is_none() && Instant::now() < end {
        if let Ok(Some(ReceiverEvent::VideoTrack { codec, mime })) =
            tokio::time::timeout(Duration::from_millis(50), receiver.next_event()).await
        {
            viewer.codec = Some((codec, mime));
        }
    }
    let (codec, mime) = viewer.codec.clone().expect("a VideoTrack event");
    assert_eq!(codec, Some(VideoCodec::H264), "mime {mime}");

    // 2. A change on screen shows up in the received picture.
    window.set_colour(BLUE).unwrap();
    let got = pump(
        &mut receiver,
        &mut sender,
        &mut viewer,
        Duration::from_secs(20),
        |v, _| v.last.as_ref().is_some_and(|p| near(p, BLUE)),
    )
    .await;
    assert!(got, "the colour change did not arrive");

    // 3. The screen is static now. Let the static refresh run out, then count: the sender must
    // not produce frames any more.
    pump(
        &mut receiver,
        &mut sender,
        &mut viewer,
        Duration::from_millis(1500),
        |_, _| false,
    )
    .await;
    let quiet_start = sender.screen_stats();
    let units_start = viewer.units;
    pump(
        &mut receiver,
        &mut sender,
        &mut viewer,
        Duration::from_millis(2500),
        |_, _| false,
    )
    .await;
    let quiet_end = sender.screen_stats();
    eprintln!("screen stats at quiet start: {quiet_start:?}");
    eprintln!("screen stats at quiet end:   {quiet_end:?}");
    eprintln!(
        "received access units: {} (+{} during the quiet window)",
        viewer.units,
        viewer.units - units_start
    );
    assert_eq!(
        quiet_end.encoded, quiet_start.encoded,
        "frames were encoded while the screen did not change"
    );
    assert_eq!(viewer.units, units_start, "frames arrived while static");
    assert!(
        quiet_end.unchanged > quiet_start.unchanged + 10,
        "the screen should still have been looked at"
    );
    // Static refresh happened, but only a limited number of times per change.
    assert!(quiet_end.refreshes >= 1, "{quiet_end:?}");
    assert!(
        quiet_end.refreshes <= 3 * quiet_end.changed,
        "{quiet_end:?}"
    );
    assert!(quiet_end.keyframes >= 1);
    // 4. A key frame request from the receiver is answered although nothing changed on screen.
    let before = sender.screen_stats();
    let units_before = viewer.units;
    receiver.request_keyframe().await.expect("PLI can be sent");
    let got = pump(
        &mut receiver,
        &mut sender,
        &mut viewer,
        Duration::from_secs(10),
        |v, s| v.units > units_before && s.screen_stats().keyframes > before.keyframes,
    )
    .await;
    assert!(
        got,
        "no key frame after the request: {:?}",
        sender.screen_stats()
    );
    let after = sender.screen_stats();
    assert!(
        after.keyframe_requests > before.keyframe_requests,
        "{after:?}"
    );
    assert!(near(viewer.last.as_ref().unwrap(), BLUE));
    eprintln!("after the key frame request: {after:?}");

    sender.stop().await;
    receiver.stop().await;
}
