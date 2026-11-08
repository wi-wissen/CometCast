// SPDX-License-Identifier: GPL-3.0-or-later
//! `Session`: one WebRTC peer connection (direct route only), wrapping the `webrtc` crate.
//!
//! The rest of Comet Cast should not need to know the `webrtc` API. A session
//! turns library callbacks into [`SessionEvent`]s on a channel and counts incoming
//! video in [`MediaStats`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use rtc::interceptor::Registry;
use rtc::media::Sample;
use rtc::peer_connection::configuration::interceptor_registry::register_default_interceptors;
use rtc::peer_connection::configuration::media_engine::{
    MIME_TYPE_H264, MIME_TYPE_VP8, MediaEngine,
};
use rtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use tokio::sync::{Mutex, OnceCell, mpsc};
use webrtc::data_channel::RTCDataChannelInit;
use webrtc::media_stream::MediaStreamTrack;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_local::{TrackLocal, TrackLocalEvent};
use webrtc::media_stream::track_remote::{TrackRemote, TrackRemoteEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
    RTCIceCandidateInit, RTCIceServer, RTCPeerConnectionIceEvent, RTCPeerConnectionState,
    RTCSessionDescription,
};
use webrtc::rtp_transceiver::RtpSender;

use crate::Route;
use crate::control::{CONTROL_LABEL, ControlChannel, SEND_BUFFER_LIMIT};
use crate::error::TransportError;
use crate::ice::IceServerConfig;
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
    /// STUN/TURN servers (from the signalling server's `/ice-servers`, see [`crate::ice`]).
    /// Empty = host candidates only. Entries that are not usable (no URL, or `turn:` without
    /// user name and credential) are skipped.
    pub ice_servers: Vec<IceServerConfig>,
    /// If set, every RTP packet of incoming video is also sent to this channel (payload, marker
    /// bit, RTP timestamp). Meant for tests and tools that decode what arrives; the receiver
    /// apps do not use it. The channel is bounded: when the reader is too slow, packets are
    /// dropped and counted ([`VideoTap::dropped`]).
    pub video_tap: Option<VideoTap>,
}

/// Default size of the channel made by [`VideoTap::channel_default`], in packets.
/// UNCERTAIN: placeholder (about 3 MB of 1200-byte packets).
pub const VIDEO_TAP_CAPACITY: usize = 2048;

/// The sending side of a bounded copy of the incoming video packets (see
/// [`SessionConfig::video_tap`]). Cheap to clone.
#[derive(Debug, Clone)]
pub struct VideoTap {
    tx: mpsc::Sender<VideoPacket>,
    dropped: Arc<AtomicU64>,
}

impl VideoTap {
    /// A tap with room for `capacity` packets (at least 1) and the receiver to read them from.
    pub fn channel(capacity: usize) -> (Self, mpsc::Receiver<VideoPacket>) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        (
            Self {
                tx,
                dropped: Arc::new(AtomicU64::new(0)),
            },
            rx,
        )
    }

    /// [`Self::channel`] with [`VIDEO_TAP_CAPACITY`].
    pub fn channel_default() -> (Self, mpsc::Receiver<VideoPacket>) {
        Self::channel(VIDEO_TAP_CAPACITY)
    }

    /// Packets dropped because the reader was too slow (or gone).
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// A shared handle on the drop counter, to read it after the tap itself was moved away.
    pub fn dropped_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.dropped)
    }

    fn offer(&self, packet: VideoPacket) {
        if self.tx.try_send(packet).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// One incoming RTP video packet, as handed to [`SessionConfig::video_tap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoPacket {
    /// RTP payload (codec payload format, e.g. RFC 6184 for H.264).
    pub payload: Bytes,
    /// RTP marker bit: set on the last packet of a frame.
    pub marker: bool,
    /// RTP timestamp (90 kHz clock for video).
    pub timestamp: u32,
    /// RTP sequence number.
    pub sequence: u16,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            route: Route::Direct,
            udp_bind: vec!["0.0.0.0:0".to_string()],
            video_codecs: None,
            ice_servers: Vec::new(),
            video_tap: None,
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
    /// The peer opened a data channel labelled `control`. The receiver side gets this; the
    /// side that creates the channel ([`Session::create_control_channel`]) does not. Only the
    /// first such channel is reported, others are closed.
    ControlChannel(ControlChannel),
}

/// One peer connection.
pub struct Session {
    pc: Arc<dyn PeerConnection>,
    stats: Arc<MediaStats>,
    ice: Mutex<IceQueue>,
    /// The incoming video track and its SSRC, once it exists (for key frame requests).
    remote_video: RemoteVideo,
    /// The final local and remote SDP, kept for the connection check code ([`crate::sas`]).
    sdps: std::sync::Mutex<Sdps>,
}

#[derive(Default)]
struct Sdps {
    local: Option<String>,
    remote: Option<String>,
}

/// The incoming video track and its SSRC, once it exists.
type RemoteVideo = Arc<Mutex<Option<(Arc<dyn TrackRemote>, u32)>>>;

/// Most remote candidates that wait for the remote description (more are dropped). A real
/// peer gathers a handful per network interface; the queue only has to bridge the short time
/// until the offer or answer is applied.
const MAX_PENDING_CANDIDATES: usize = 64;

/// Longest `candidate` line accepted. The fields of RFC 8839 (section 5.1) add up to a few
/// hundred characters even with two 253-character host names; browsers send about 100 to 200.
const MAX_CANDIDATE_LEN: usize = 1024;

/// Longest `RTCIceCandidateInit` JSON text accepted (the candidate plus `sdpMid`,
/// `sdpMLineIndex`, `usernameFragment` and the JSON around them).
const MAX_CANDIDATE_JSON_LEN: usize = 2 * MAX_CANDIDATE_LEN;

/// Remote candidates may arrive before the remote description; they wait here (at most
/// [`MAX_PENDING_CANDIDATES`]).
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
        // Key frame requests (PLI/FIR) must reach the sending application.
        let registry = registry.with(
            crate::keyframe::slot(),
            crate::keyframe::KeyframeForwarder::default(),
        );

        // UNCERTAIN: relaying through TURN was not tried against a real coturn; only that the
        // list reaches the library configuration is tested (docs/transport.md).
        let rtc_config = RTCConfigurationBuilder::new()
            .with_ice_servers(to_rtc_ice_servers(&config.ice_servers))
            .build();

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let stats = Arc::new(MediaStats::default());
        let remote_video = Arc::new(Mutex::new(None));
        let handler = Arc::new(Handler {
            control_taken: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            remote_video: Arc::clone(&remote_video),
            events: events_tx,
            stats: Arc::clone(&stats),
            tap: config.video_tap.clone(),
        });
        let pc = PeerConnectionBuilder::new()
            .with_configuration(rtc_config)
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_handler(handler)
            .with_udp_addrs(config.udp_bind.clone())
            // Back-pressure for the control data channel (see control.rs).
            .with_data_channel_send_buffer_limit(SEND_BUFFER_LIMIT)
            .build()
            .await?;
        Ok((
            Self {
                pc: Arc::new(pc),
                stats,
                ice: Mutex::new(IceQueue::default()),
                remote_video,
                sdps: std::sync::Mutex::new(Sdps::default()),
            },
            events_rx,
        ))
    }

    /// Add a video track that this side will send. Call before [`Session::create_offer`].
    /// VP8 and H.264 can be sent. H.264 is offered as `packetization-mode=1` with
    /// `profile-level-id=42e01f` (Constrained Baseline, level 3.1), the profile browsers
    /// announce for WebRTC. Frames are given to [`VideoSender::send_frame`] in Annex-B form.
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
            rtcp_events: Mutex::new(()),
        })
    }

    /// Create the `control` data channel (ordered, reliable) as the web sender does. Call
    /// before [`Session::create_offer`], so that the offer contains it. The channel can be
    /// used once [`ControlChannel::wait_open`] returns.
    pub async fn create_control_channel(&self) -> Result<ControlChannel, TransportError> {
        let dc = self
            .pc
            .create_data_channel(CONTROL_LABEL, Some(RTCDataChannelInit::default()))
            .await?;
        Ok(ControlChannel::new(dc).await)
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
        self.set_sdp(Some(local.sdp.clone()), None);
        Ok(local.sdp)
    }

    /// Take the peer's offer and return our answer SDP.
    pub async fn accept_offer(&self, sdp: String) -> Result<String, TransportError> {
        self.set_sdp(None, Some(sdp.clone()));
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
        self.set_sdp(Some(local.sdp.clone()), None);
        Ok(local.sdp)
    }

    /// Take the peer's answer to our offer.
    pub async fn accept_answer(&self, sdp: String) -> Result<(), TransportError> {
        self.set_sdp(None, Some(sdp.clone()));
        let answer = RTCSessionDescription::answer(sdp)?;
        self.pc.set_remote_description(answer).await?;
        self.flush_pending_candidates().await;
        Ok(())
    }

    fn set_sdp(&self, local: Option<String>, remote: Option<String>) {
        let mut g = self.sdps.lock().unwrap_or_else(|p| p.into_inner());
        if local.is_some() {
            g.local = local;
        }
        if remote.is_some() {
            g.remote = remote;
        }
    }

    /// The connection check code (`"123 456"`): both sides show the same digits when nobody
    /// swapped the DTLS certificates on the way (see [`crate::sas`]). `None` until both
    /// descriptions are known, or when one has no SHA-256 fingerprint.
    pub fn connection_code(&self) -> Option<String> {
        let g = self.sdps.lock().unwrap_or_else(|p| p.into_inner());
        crate::sas::connection_code(g.local.as_deref()?, g.remote.as_deref()?)
    }

    /// Add a remote ICE candidate given as the JSON text of an `RTCIceCandidateInit`.
    /// Candidates that arrive before the remote description are queued, at most
    /// [`MAX_PENDING_CANDIDATES`] of them; over-long candidates and candidates beyond that
    /// are refused (the peer is not trusted to keep within reason).
    pub async fn add_remote_candidate(&self, json: &str) -> Result<(), TransportError> {
        if json.len() > MAX_CANDIDATE_JSON_LEN {
            return Err(TransportError::Invalid("ICE candidate too long".into()));
        }
        let init: RTCIceCandidateInit =
            serde_json::from_str(json).map_err(|e| TransportError::Invalid(e.to_string()))?;
        if init.candidate.is_empty() {
            return Ok(()); // end-of-candidates marker
        }
        if init.candidate.len() > MAX_CANDIDATE_LEN {
            return Err(TransportError::Invalid("ICE candidate too long".into()));
        }
        let mut q = self.ice.lock().await;
        if q.remote_set {
            drop(q);
            self.pc.add_ice_candidate(init).await?;
        } else if q.pending.len() < MAX_PENDING_CANDIDATES {
            q.pending.push(init);
        } else {
            return Err(TransportError::Invalid(
                "too many ICE candidates before the remote description".into(),
            ));
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

    /// Ask the sender for a new key frame (RTCP PLI), for example after a decoding error.
    /// Fails if no video track has arrived yet.
    pub async fn request_keyframe(&self) -> Result<(), TransportError> {
        let guard = self.remote_video.lock().await;
        let Some((track, media_ssrc)) = guard.as_ref() else {
            return Err(TransportError::Invalid(
                "no incoming video track yet".into(),
            ));
        };
        let pli = PictureLossIndication {
            sender_ssrc: 0,
            media_ssrc: *media_ssrc,
        };
        track.write_rtcp(vec![Box::new(pli)]).await?;
        Ok(())
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
    /// Only one task at a time may wait for RTCP events.
    rtcp_events: Mutex<()>,
}

impl VideoSender {
    /// Wait for the next key frame request (RTCP PLI or FIR) from the receiver. Returns
    /// `false` when no more feedback can arrive (the connection closed, or the track is not
    /// connected yet: call this only after the connection state is `Connected`).
    pub async fn next_keyframe_request(&self) -> bool {
        let _one_waiter = self.rtcp_events.lock().await;
        while let Some(event) = self.track.poll().await {
            // Only requests for a key frame get through the interceptor, see `keyframe.rs`.
            let TrackLocalEvent::OnRtcpPacket(packets) = event else {
                continue;
            };
            if !packets.is_empty() {
                return true;
            }
        }
        false
    }

    /// Send one encoded frame (a whole VP8 frame, or one H.264 access unit in Annex-B form). `duration` is the time this frame lasts;
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

/// Convert the server's list to the library type. Unusable entries are dropped: the library
/// refuses the whole configuration for one bad entry, and a missing TURN server should not
/// stop a connection that could work directly.
fn to_rtc_ice_servers(list: &[IceServerConfig]) -> Vec<RTCIceServer> {
    list.iter()
        .filter(|s| !s.urls.is_empty())
        .filter(|s| {
            let needs_auth = s
                .urls
                .iter()
                .any(|u| u.starts_with("turn:") || u.starts_with("turns:"));
            !needs_auth
                || (s.username.as_deref().is_some_and(|u| !u.is_empty())
                    && s.credential.as_deref().is_some_and(|c| !c.is_empty()))
        })
        .map(|s| RTCIceServer {
            urls: s.urls.clone(),
            username: s.username.clone().unwrap_or_default(),
            credential: s.credential.clone().unwrap_or_default(),
        })
        .collect()
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
        VideoCodec::H264 => Ok(RTCRtpCodecParameters {
            rtp_codec: RTCRtpCodec {
                mime_type: MIME_TYPE_H264.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line:
                    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
                        .to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: 102,
        }),
        _ => Err(TransportError::Unsupported(
            "only VP8 and H.264 can be registered explicitly; use default codecs to receive others",
        )),
    }
}

/// Library callbacks -> events and counters.
struct Handler {
    /// Set when the first `control` channel of the peer was taken.
    control_taken: Arc<std::sync::atomic::AtomicBool>,
    remote_video: RemoteVideo,
    events: mpsc::UnboundedSender<SessionEvent>,
    stats: Arc<MediaStats>,
    tap: Option<VideoTap>,
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

    async fn on_data_channel(&self, data_channel: Arc<dyn webrtc::data_channel::DataChannel>) {
        // The library's driver waits for this callback to return, and `label()` asks the
        // driver, so the work moves to its own task.
        let events = self.events.clone();
        let taken = Arc::clone(&self.control_taken);
        tokio::spawn(async move {
            if data_channel.label().await.as_deref() != Ok(CONTROL_LABEL)
                || taken.swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                // Other labels are not ours; a second `control` channel is not allowed.
                let _ = data_channel.close().await;
                return;
            }
            let channel = ControlChannel::new(data_channel).await;
            let _ = events.send(SessionEvent::ControlChannel(channel));
        });
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
        let tap = self.tap.clone();

        // Ask for a key frame now and every 2 s until one arrived, so a late joiner
        // does not wait for the sender's next regular key frame.
        // UNCERTAIN: the 2 s interval is a placeholder.
        if let (true, Some(media_ssrc)) = (is_video, ssrc) {
            *self.remote_video.lock().await = Some((Arc::clone(&track), media_ssrc));
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
                    if let Some(tap) = &tap {
                        tap.offer(VideoPacket {
                            payload: p.payload.clone(),
                            marker: p.header.marker,
                            timestamp: p.header.timestamp,
                            sequence: p.header.sequence_number,
                        });
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_video_tap_is_bounded_and_counts_drops() {
        let (tap, mut rx) = VideoTap::channel(3);
        let packet = |n: u16| VideoPacket {
            payload: Bytes::from_static(b"x"),
            marker: false,
            timestamp: 0,
            sequence: n,
        };
        for n in 0..10 {
            tap.offer(packet(n));
        }
        assert_eq!(tap.dropped(), 7);
        assert_eq!(tap.clone().dropped(), 7, "clones share the counter");
        let got: Vec<u16> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|p| p.sequence)
            .collect();
        assert_eq!(got, [0, 1, 2], "the oldest packets are kept");
        // A reader that is gone also counts as dropped, and never blocks.
        drop(rx);
        tap.offer(packet(11));
        assert_eq!(tap.dropped(), 8);
    }

    fn cfg(urls: &[&str], user: Option<&str>, cred: Option<&str>) -> IceServerConfig {
        IceServerConfig {
            urls: urls.iter().map(|u| (*u).to_string()).collect(),
            username: user.map(str::to_string),
            credential: cred.map(str::to_string),
        }
    }

    #[test]
    fn ice_servers_are_converted_and_unusable_ones_dropped() {
        let list = vec![
            cfg(&["stun:s.example.org:3478"], None, None),
            cfg(
                &["turn:t.example.org:3478?transport=udp"],
                Some("u"),
                Some("c"),
            ),
            cfg(&["turn:no-auth.example.org"], None, None),
            cfg(&[], Some("u"), Some("c")),
        ];
        let out = to_rtc_ice_servers(&list);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].username, "u");
        assert_eq!(out[1].credential, "c");
        assert_eq!(out[0].username, "");
    }

    #[tokio::test]
    async fn early_candidates_are_bounded_in_number_and_length() {
        let config = SessionConfig {
            udp_bind: vec!["127.0.0.1:0".into()],
            ..SessionConfig::default()
        };
        let (session, _events) = Session::new(config).await.unwrap();
        let json = |c: &str| {
            serde_json::json!({ "candidate": c, "sdpMid": null, "sdpMLineIndex": 0 }).to_string()
        };
        let good = "candidate:1 1 udp 2122260223 192.0.2.1 54321 typ host generation 0";
        for _ in 0..MAX_PENDING_CANDIDATES {
            session.add_remote_candidate(&json(good)).await.unwrap();
        }
        assert!(
            session.add_remote_candidate(&json(good)).await.is_err(),
            "the queue is full"
        );
        assert_eq!(
            session.ice.lock().await.pending.len(),
            MAX_PENDING_CANDIDATES
        );
        let long = format!("{good} {}", "x".repeat(MAX_CANDIDATE_LEN));
        assert!(session.add_remote_candidate(&json(&long)).await.is_err());
        let padded = format!(
            r#"{{"candidate":"{good}","sdpMid":"{}","sdpMLineIndex":0}}"#,
            "m".repeat(MAX_CANDIDATE_JSON_LEN)
        );
        assert!(session.add_remote_candidate(&padded).await.is_err());
        // The end-of-candidates marker is still fine.
        session.add_remote_candidate(&json("")).await.unwrap();
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn session_accepts_turn_configuration() {
        // The library validates TURN URLs and credentials when the connection is built.
        let config = SessionConfig {
            udp_bind: vec!["127.0.0.1:0".into()],
            ice_servers: vec![
                cfg(&["stun:127.0.0.1:3478"], None, None),
                cfg(
                    &["turn:127.0.0.1:3478?transport=udp"],
                    Some("1:abc"),
                    Some("secret"),
                ),
            ],
            ..SessionConfig::default()
        };
        let (session, _events) = Session::new(config)
            .await
            .expect("session with TURN config");
        session.close().await.unwrap();
    }
}
