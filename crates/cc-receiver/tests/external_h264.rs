// SPDX-License-Identifier: GPL-3.0-or-later
//! The `ExternalH264` input end to end: a host (this test, standing in for the Android app)
//! pushes real H.264 access units made by OpenH264 from synthetic pictures; they travel through
//! the sender's RTP packetiser, WebRTC over loopback UDP and a real cc-server in this process to
//! the native receiver, where they are put back together and decoded. No X server is needed.
//!
//! Both input formats a `MediaCodec` might produce are covered (UNCERTAIN which one real devices
//! use, docs/android.md): Annex B with in-band SPS/PPS, and AVCC with the parameter sets only in
//! a separate `avcC` config record.

use std::time::{Duration, Instant};

use cc_media::rtp::H264Assembler;
use cc_media::{
    BgraFrame, DecodedFrame, H264Backend, H264Config, H264Decoder, H264Encoder, annexb_nal_types,
};
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{ExternalInput, PushOutcome, SenderCore, SenderEvent, SenderOptions};
use cc_server::{ServerConfig, run};
use cc_transport::{SessionConfig, VideoCodec, VideoPacket, VideoTap};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const RED: [u8; 3] = [200, 30, 40];
const BLUE: [u8; 3] = [20, 160, 220];

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

fn solid(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        v.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 255]);
    }
    v
}

/// NAL units of an Annex B access unit (without start codes).
fn nals(annex_b: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= annex_b.len() {
        if annex_b[i..i + 3] == [0, 0, 1] {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (n, &s) in starts.iter().enumerate() {
        let mut e = starts.get(n + 1).map_or(annex_b.len(), |x| x - 3);
        while e > s && annex_b[e - 1] == 0 {
            e -= 1;
        }
        out.push(&annex_b[s..e]);
    }
    out
}

/// How the host presents its encoder output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Format {
    /// Start codes, SPS/PPS in front of every IDR frame (what OpenH264 produces).
    AnnexB,
    /// 4-byte lengths, SPS/PPS removed from the frames and given once as an `avcC` record.
    AvccWithConfigRecord,
}

fn to_avcc(annex_b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for n in nals(annex_b) {
        if matches!(n[0] & 0x1f, 7 | 8) {
            continue;
        }
        out.extend_from_slice(&(n.len() as u32).to_be_bytes());
        out.extend_from_slice(n);
    }
    out
}

fn avcc_record(annex_b_key_frame: &[u8]) -> Vec<u8> {
    let all = nals(annex_b_key_frame);
    let sps = all.iter().find(|n| n[0] & 0x1f == 7).expect("SPS");
    let pps = all.iter().find(|n| n[0] & 0x1f == 8).expect("PPS");
    let mut v = vec![1, sps[1], sps[2], sps[3], 0xff, 0xe1];
    v.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    v.extend_from_slice(sps);
    v.push(1);
    v.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    v.extend_from_slice(pps);
    v
}

/// Plays the part of the Android app: an encoder plus the key frame request handling.
struct Host {
    input: ExternalInput,
    encoder: H264Encoder,
    format: Format,
    colour: [u8; 3],
    ts_us: u64,
    config_sent: bool,
    outcomes: Vec<PushOutcome>,
}

impl Host {
    fn new(input: ExternalInput, format: Format) -> Self {
        let encoder = H264Encoder::new(&H264Config {
            backend: H264Backend::Source,
            ..H264Config::default()
        })
        .unwrap();
        Self {
            input,
            encoder,
            format,
            colour: RED,
            ts_us: 0,
            config_sent: false,
            outcomes: Vec::new(),
        }
    }

    /// Encode and push one picture (called about every 60 ms), honouring key frame requests.
    fn tick(&mut self) {
        if self.input.keyframe_requested() {
            self.encoder.force_keyframe();
        }
        let pic = solid(160, 120, self.colour);
        self.ts_us += 66_000;
        let Some(frame) = self
            .encoder
            .encode_bgra(
                BgraFrame {
                    width: 160,
                    height: 120,
                    bgra: &pic,
                },
                self.ts_us,
            )
            .unwrap()
        else {
            return;
        };
        let data = match self.format {
            Format::AnnexB => frame.data.clone(),
            Format::AvccWithConfigRecord => {
                if frame.is_keyframe && !self.config_sent {
                    assert!(self.input.set_config(&avcc_record(&frame.data)));
                    self.config_sent = true;
                }
                to_avcc(&frame.data)
            }
        };
        let outcome = self.input.push(&data, frame.is_keyframe, Some(self.ts_us));
        self.outcomes.push(outcome);
    }
}

struct Viewer {
    rx: mpsc::Receiver<VideoPacket>,
    assembler: H264Assembler,
    decoder: H264Decoder,
    units: u64,
    last: Option<DecodedFrame>,
    codec: Option<VideoCodec>,
}

impl Viewer {
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

struct Rig {
    receiver: ReceiverCore,
    sender: SenderCore,
    viewer: Viewer,
    host: Host,
}

async fn rig(format: Format) -> Rig {
    let server = start_server().await;
    let (tap_tx, tap_rx) = VideoTap::channel_default();
    let mut ropts = ReceiverOptions::new(&server);
    ropts.session = local_session();
    ropts.session.video_tap = Some(tap_tx);
    let receiver = ReceiverCore::start(ropts).await.expect("receiver start");

    let input = ExternalInput::default();
    let mut sopts = SenderOptions::new(&server, receiver.room_code().clone());
    sopts.session = local_session();
    sopts.external = Some(input.clone());
    let sender = SenderCore::start(sopts).await.expect("sender start");
    Rig {
        receiver,
        sender,
        viewer: Viewer {
            rx: tap_rx,
            assembler: H264Assembler::new(),
            decoder: H264Decoder::new(&H264Backend::Source).unwrap(),
            units: 0,
            last: None,
            codec: None,
        },
        host: Host::new(input, format),
    }
}

impl Rig {
    /// Run host, cores and decoder until `done` or the time is up.
    async fn pump(&mut self, limit: Duration, mut done: impl FnMut(&Rig) -> bool) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            tokio::select! {
                ev = self.receiver.next_event() => match ev {
                    Some(ReceiverEvent::Error(e)) => panic!("receiver error: {e}"),
                    None => panic!("receiver ended"),
                    Some(ReceiverEvent::VideoTrack { codec, .. }) => self.viewer.codec = codec,
                    Some(_) => {}
                },
                ev = self.sender.next_event() => match ev {
                    Some(SenderEvent::Error(e)) => panic!("sender error: {e}"),
                    None => panic!("sender ended"),
                    Some(_) => {}
                },
                () = tokio::time::sleep(Duration::from_millis(60)) => {}
            }
            self.host.tick();
            self.viewer.drain();
            if done(self) {
                return true;
            }
        }
        false
    }
}

fn near(pic: &DecodedFrame, rgb: [u8; 3]) -> bool {
    pic.share_near(rgb, 25) > 0.9
}

async fn run_format(format: Format) {
    let mut r = rig(format).await;

    // 1. Before a peer is connected the host's frames are refused; once it is, the first
    // request for a key frame comes from the core, and a picture with the right colour arrives.
    let got = r
        .pump(Duration::from_secs(20), |r| {
            r.viewer.last.as_ref().is_some_and(|p| near(p, RED))
        })
        .await;
    assert!(
        got,
        "{format:?}: no red picture; stats={:?} units={}",
        r.sender.external_stats(),
        r.viewer.units
    );
    let pic = r.viewer.last.clone().unwrap();
    assert_eq!((pic.width, pic.height), (160, 120));
    r.pump(Duration::from_secs(3), |r| r.viewer.codec.is_some())
        .await;
    assert_eq!(r.viewer.codec, Some(VideoCodec::H264), "receiver saw H.264");
    let s = r.sender.external_stats();
    assert!(s.keyframes_sent >= 1 && s.sent >= 1, "{s:?}");
    assert!(s.keyframe_requests >= 1, "{s:?}");
    assert!(
        r.host.outcomes.contains(&PushOutcome::Queued),
        "{:?}",
        r.host.outcomes
    );

    // 2. A change of the content arrives.
    r.host.colour = BLUE;
    let got = r
        .pump(Duration::from_secs(20), |r| {
            r.viewer.last.as_ref().is_some_and(|p| near(p, BLUE))
        })
        .await;
    assert!(got, "{format:?}: colour change did not arrive");

    // 3. A PLI from the receiver reaches the host as a key frame request, and the host's answer
    // (a new IDR frame) arrives. The rate limit (1 s) may delay it; wait for it.
    let before = r.sender.external_stats();
    let units_before = r.viewer.units;
    // Let the rate-limit window of the start pass.
    r.pump(Duration::from_millis(1200), |_| false).await;
    r.receiver
        .request_keyframe()
        .await
        .expect("PLI can be sent");
    let got = r
        .pump(Duration::from_secs(10), |r| {
            r.sender.external_stats().keyframes_sent > before.keyframes_sent
                && r.viewer.units > units_before
        })
        .await;
    let after = r.sender.external_stats();
    assert!(got, "{format:?}: no key frame after the PLI: {after:?}");
    assert!(
        after.keyframe_requests > before.keyframe_requests,
        "{after:?}"
    );
    assert!(near(r.viewer.last.as_ref().unwrap(), BLUE));

    // The bounded queue was never exceeded.
    assert!(r.host.input.queue_len() <= 3);
    eprintln!("{format:?}: {after:?}");
    r.sender.stop().await;
    r.receiver.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn annex_b_access_units_reach_the_receiver_and_decode() {
    run_format(Format::AnnexB).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn avcc_access_units_with_a_config_record_reach_the_receiver_and_decode() {
    run_format(Format::AvccWithConfigRecord).await;
}

#[test]
fn the_encoder_output_used_by_the_tests_has_in_band_parameter_sets() {
    // Guards the assumption of the Annex B variant: OpenH264's IDR frames start with SPS, PPS.
    let mut enc = H264Encoder::new(&H264Config {
        backend: H264Backend::Source,
        ..H264Config::default()
    })
    .unwrap();
    let pic = solid(160, 120, RED);
    let f = enc
        .encode_bgra(
            BgraFrame {
                width: 160,
                height: 120,
                bgra: &pic,
            },
            0,
        )
        .unwrap()
        .unwrap();
    let types = annexb_nal_types(&f.data);
    assert!(types.contains(&7) && types.contains(&8) && types.contains(&5));
}
