// SPDX-License-Identifier: GPL-3.0-or-later
//! `Session`: one WebRTC peer connection (direct route only), wrapping the `webrtc` crate.
//!
//! The rest of Comet Cast should not need to know the `webrtc` API. A session
//! turns library callbacks into [`SessionEvent`]s on a channel and counts incoming
//! video in [`MediaStats`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use rtc::interceptor::Registry;
use rtc::media::Sample;
use rtc::peer_connection::configuration::interceptor_registry::register_default_interceptors;
use rtc::peer_connection::configuration::media_engine::{MIME_TYPE_VP8, MediaEngine};
use rtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use tokio::sync::{Mutex, OnceCell, mpsc};
use webrtc::media_stream::MediaStreamTrack;
use webrtc::media_stream::track_local::TrackLocal;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_remote::{TrackRemote, TrackRemoteEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
    RTCIceCandidateInit, RTCPeerConnectionIceEvent, RTCPeerConnectionState, RTCSessionDescription,
};
use webrtc::rtp_transceiver::RtpSender;

use crate::Route;
use crate::error::TransportError;
use crate::rtp::{MediaSnapshot, MediaStats, VideoCodec, starts_key_frame};

/// Settings for a [`Session`].
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Which route to use. Only [`Route::Direct`] works today.
    pub route: Route,
    /// Local UDP addresses to bind, e.g. `"0.0.0.0:0"` (all interfaces, any port).
    /// Tests on one machine use `"127.0.0.1:0"`.
    pub udp_bind: Vec<String>,
    /// Video codecs to offer or accept. `None` means all codecs the library registers by default
    /// (so the receiver can answer whatever a browser offers). `Some(vec![Vp8])` is what the
    /// synthetic sender uses.
    pub video_codecs: Option<Vec<VideoCodec>>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            route: Route::Direct,
            udp_bind: vec!["0.0.0.0:0".to_string()],
            video_codecs: None,
        }
    }
}

/// Connection state, simplified from the library's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not started.
    New,
    /// ICE / DTLS in progress.
    Connecting,
    /// Media can flow.
    Connected,
    /// Temporarily lost.
    /// UNCERTAIN: may recover; callers should not treat it as final.
    Disconnected,
    /// Gave up.
    Failed,
    /// Closed.
    Closed,
}

/// What a session reports to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// A local ICE candidate to send to the peer. The string is the JSON text of an
    /// `RTCIceCandidateInit` (same as browsers, see docs/protocol.md).
    LocalIceCandidate(String),
    /// The connection state changed.
    State(ConnectionState),
    /// The peer started sending a video track.
    VideoTrack {
        /// MIME type from the negotiated codec, e.g. `video/VP8`.
        mime: String,
        /// Parsed codec if it is one we know.
        codec: Option<VideoCodec>,
    },
}

/// One peer connection.
pub struct Session {
    pc: Arc<dyn PeerConnection>,
    stats: Arc<MediaStats>,
    ice: Mutex<IceQueue>,
}

/// Remote candidates may arrive before the remote description; they wait here.
#[derive(Default)]
struct IceQueue {
    remote_set: bool,
    pending: Vec<RTCIceCandidateInit>,
}

impl Session {
    /// Create a session and the channel its events arrive on.
    pub async fn new(
        config: SessionConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<SessionEvent>), TransportError> {
        if config.route != Route::Direct {
            return Err(TransportError::Unsupported(
                "only the direct route exists yet",
            ));
        }
        let mut media_engine = MediaEngine::default();
        match &config.video_codecs {
            None => media_engine.register_default_codecs()?,
            Some(list) => {
                for codec in list {
                    media_engine.register_codec(codec_parameters(*codec)?, RtpCodecKind::Video)?;
                }
            }
        }
        let registry = register_default_interceptors(Registry::new(), &mut media_engine)?;

        // UNCERTAIN: no STUN/TURN servers, so only direct / same-network connections work.
        // STUN/TURN belongs to milestone M8 and measurement point 1 (docs/measurement-points.md).
        let rtc_config = RTCConfigurationBuilder::new().build();

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let stats = Arc::new(MediaStats::default());
        let handler = Arc::new(Handler {
            events: events_tx,
            stats: Arc::clone(&stats),
        });
        let pc = PeerConnectionBuilder::new()
            .with_configuration(rtc_config)
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_handler(handler)
            .with_udp_addrs(config.udp_bind.clone())
            .build()
            .await?;
        Ok((
            Self {
                pc: Arc::new(pc),
                stats,
                ice: Mutex::new(IceQueue::default()),
            },
            events_rx,
        ))
    }

    /// Add a video track that this side will send. Call before [`Session::create_offer`].
    /// Only VP8 can be sent for now.
    pub async fn add_video_track(&self, codec: VideoCodec) -> Result<VideoSender, TransportError> {
        let params = codec_parameters(codec)?;
        let ssrc: u32 = rand_u32();
        let track = TrackLocalStaticSample::new(
            Instant::now(),
            MediaStreamTrack::new(
                "cometcast-stream".to_owned(),
                "cometcast-video".to_owned(),
                "cometcast-video".to_owned(),
                RtpCodecKind::Video,
                vec![RTCRtpEncodingParameters {
                    rtp_coding_parameters: RTCRtpCodingParameters {
                        ssrc: Some(ssrc),
                        ..Default::default()
                    },
                    codec: params.rtp_codec.clone(),
                    ..Default::default()
                }],
            ),
        )?;
        let track = Arc::new(track);
        let sender = self
            .pc
            .add_track(Arc::clone(&track) as Arc<dyn TrackLocal>)
            .await?;
        Ok(VideoSender {
            track,
            sender,
            ssrc,
            payload_type: OnceCell::new(),
        })
    }

    /// Create an offer, set it as local description and return its SDP text.
    pub async fn create_offer(&self) -> Result<String, TransportError> {
        let offer = self.pc.create_offer(None).await?;
        self.pc.set_local_description(offer).await?;
        let local = self
            .pc
            .local_description()
            .await
            .ok_or_else(|| TransportError::WebRtc("no local description".into()))?;
        Ok(local.sdp)
    }

    /// Take the peer's offer and return our answer SDP.
    pub async fn accept_offer(&self, sdp: String) -> Result<String, TransportError> {
        let offer = RTCSessionDescription::offer(sdp)?;
        self.pc.set_remote_description(offer).await?;
        self.flush_pending_candidates().await;
        let answer = self.pc.create_answer(None).await?;
        self.pc.set_local_description(answer).await?;
        let local = self
            .pc
            .local_description()
            .await
            .ok_or_else(|| TransportError::WebRtc("no local description".into()))?;
        Ok(local.sdp)
    }

    /// Take the peer's answer to our offer.
    pub async fn accept_answer(&self, sdp: String) -> Result<(), TransportError> {
        let answer = RTCSessionDescription::answer(sdp)?;
        self.pc.set_remote_description(answer).await?;
        self.flush_pending_candidates().await;
        Ok(())
    }

    /// Add a remote ICE candidate given as the JSON text of an `RTCIceCandidateInit`.
    /// Candidates that arrive before the remote description are queued.
    pub async fn add_remote_candidate(&self, json: &str) -> Result<(), TransportError> {
        let init: RTCIceCandidateInit =
            serde_json::from_str(json).map_err(|e| TransportError::Invalid(e.to_string()))?;
        if init.candidate.is_empty() {
            return Ok(()); // end-of-candidates marker
        }
        let mut q = self.ice.lock().await;
        if q.remote_set {
            drop(q);
            self.pc.add_ice_candidate(init).await?;
        } else {
            q.pending.push(init);
        }
        Ok(())
    }

    async fn flush_pending_candidates(&self) {
        let pending = {
            let mut q = self.ice.lock().await;
            q.remote_set = true;
            std::mem::take(&mut q.pending)
        };
        for c in pending {
            // A bad single candidate must not stop the connection; others may still work.
            let _ = self.pc.add_ice_candidate(c).await;
        }
    }

    /// Counters of incoming video.
    pub fn stats(&self) -> MediaSnapshot {
        self.stats.snapshot()
    }

    /// Close the connection.
    pub async fn close(&self) -> Result<(), TransportError> {
        self.pc.close().await?;
        Ok(())
    }
}

/// Sends encoded video frames on a track added with [`Session::add_video_track`].
pub struct VideoSender {
    track: Arc<TrackLocalStaticSample>,
    sender: Arc<dyn RtpSender>,
    ssrc: u32,
    payload_type: OnceCell<u8>,
}

impl VideoSender {
    /// Send one encoded frame (a whole VP8 frame). `duration` is the time this frame lasts;
    /// it sets the RTP timestamp step. Fails until the offer/answer exchange is done.
    pub async fn send_frame(&self, data: Bytes, duration: Duration) -> Result<(), TransportError> {
        let pt = *self
            .payload_type
            .get_or_try_init(|| async {
                // The payload type is only known after negotiation.
                self.sender
                    .get_parameters()
                    .await
                    .map_err(TransportError::from)?
                    .rtp_parameters
                    .codecs
                    .first()
                    .map(|c| c.payload_type)
                    .ok_or_else(|| TransportError::WebRtc("no negotiated codec yet".into()))
            })
            .await?;
        self.track
            .sample_writer(self.ssrc, pt)
            .write_sample(&Sample {
                data,
                duration,
                ..Sample::new(Instant::now())
            })
            .await?;
        Ok(())
    }
}

fn rand_u32() -> u32 {
    // No `rand` dependency needed: the std hasher is randomly seeded per process.
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u8(1);
    h.finish() as u32
}

fn codec_parameters(codec: VideoCodec) -> Result<RTCRtpCodecParameters, TransportError> {
    match codec {
        VideoCodec::Vp8 => Ok(RTCRtpCodecParameters {
            rtp_codec: RTCRtpCodec {
                mime_type: MIME_TYPE_VP8.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: String::new(),
                rtcp_feedback: vec![],
            },
            payload_type: 96,
        }),
        _ => Err(TransportError::Unsupported(
            "only VP8 can be registered explicitly; use default codecs to receive others",
        )),
    }
}

/// Library callbacks -> events and counters.
struct Handler {
    events: mpsc::UnboundedSender<SessionEvent>,
    stats: Arc<MediaStats>,
}

#[async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        let Ok(mut init) = event.candidate.to_json() else {
            return;
        };
        // The library fills `sdpMid` with "". Browsers reject a mid that does not exist, and
        // our SDPs have one transport (bundle), so index 0 alone is enough.
        // UNCERTAIN: fine for one video m-line; revisit when audio/data lines are added.
        init.sdp_mid = None;
        init.sdp_mline_index = Some(0);
        if let Ok(json) = serde_json::to_string(&init) {
            let _ = self.events.send(SessionEvent::LocalIceCandidate(json));
        }
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        let s = match state {
            RTCPeerConnectionState::New | RTCPeerConnectionState::Unspecified => {
                ConnectionState::New
            }
            RTCPeerConnectionState::Connecting => ConnectionState::Connecting,
            RTCPeerConnectionState::Connected => ConnectionState::Connected,
            RTCPeerConnectionState::Disconnected => ConnectionState::Disconnected,
            RTCPeerConnectionState::Failed => ConnectionState::Failed,
            RTCPeerConnectionState::Closed => ConnectionState::Closed,
            // The library enum may grow; treat unknown states as "connecting".
            _ => ConnectionState::Connecting,
        };
        let _ = self.events.send(SessionEvent::State(s));
    }

    async fn on_track(&self, track: Arc<dyn TrackRemote>) {
        let kind = track.kind().await;
        let ssrc = track.ssrcs().await.first().copied();
        let mime = match ssrc {
            Some(s) => track
                .codec(s)
                .await
                .map(|c| c.mime_type)
                .unwrap_or_default(),
            None => String::new(),
        };
        let codec = VideoCodec::from_mime(&mime);
        let is_video = kind == RtpCodecKind::Video;
        if is_video {
            let _ = self.events.send(SessionEvent::VideoTrack { mime, codec });
        }
        let stats = Arc::clone(&self.stats);

        // Ask for a key frame now and every 2 s until one arrived, so a late joiner
        // does not wait for the sender's next regular key frame.
        // UNCERTAIN: the 2 s interval is a placeholder.
        if let (true, Some(media_ssrc)) = (is_video, ssrc) {
            let t = Arc::clone(&track);
            let st = Arc::clone(&stats);
            tokio::spawn(async move {
                while st.snapshot().keyframes == 0 {
                    let pli = PictureLossIndication {
                        sender_ssrc: 0,
                        media_ssrc,
                    };
                    if t.write_rtcp(vec![Box::new(pli)]).await.is_err() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        }

        // The track must be read, otherwise the library's queue fills up. Audio is read and dropped.
        tokio::spawn(async move {
            while let Some(evt) = track.poll().await {
                if let TrackRemoteEvent::OnRtpPacket(p) = evt
                    && is_video
                {
                    let key = codec.is_some_and(|c| starts_key_frame(c, &p.payload));
                    stats.record(p.payload.len(), p.header.marker, key);
                }
            }
        });
    }
}
