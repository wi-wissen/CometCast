// SPDX-License-Identifier: GPL-3.0-or-later
//! A Chromecast / Google Cast compatible **sender** (BETA, never tried on a real device).
//!
//! It finds Cast receivers with mDNS, talks the Cast v2 channel protocol over TLS (port 8009),
//! launches Google's mirroring receiver app, negotiates a mirroring session (`OFFER` /
//! `ANSWER`) and sends the screen as encrypted H.264 over RTP/UDP.
//!
//! Reference: the public, BSD-licensed Open Screen Library, commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef` (see `docs/cast.md` for the files that were
//! read and `THIRD_PARTY_NOTICES.md` for the notice). Nothing of this crate comes from a
//! proprietary product; no keys or certificates are embedded (see [`device_auth`]).
//! Receiving Cast (acting as a Cast receiver) is out of scope.
//!
//! Everything not verified against the real source or a device is marked `UNCERTAIN` in the
//! module documentation.
//!
//! Modules, in the order of a session:
//! * [`discovery`]: mDNS `_googlecast._tcp`, TXT parsing.
//! * [`proto`], [`framing`]: `CastMessage` protobuf and its length-prefixed framing.
//! * [`tls`], [`device_auth`]: TLS to a self-signed device certificate and the challenge.
//! * [`channel`]: reader/writer tasks and the heartbeat.
//! * [`session`], [`offer`]: connect, `GET_STATUS`, `LAUNCH`, `OFFER` / `ANSWER`.
//! * [`rtp`], [`streamer`], [`media`]: frame encryption, RTP/RTCP, the capture-to-network loop.

pub mod channel;
pub mod device_auth;
pub mod discovery;
pub mod error;
pub mod framing;
pub mod media;
pub mod offer;
pub mod proto;
pub mod rtp;
pub mod session;
pub mod streamer;
pub mod tls;

pub use device_auth::{DeviceAuth, TrustAnchors};
pub use discovery::{CastDevice, discover, find_by_name};
pub use error::{CastError, sanitize_display};
pub use media::{MirrorConfig, mirror_screen};
pub use session::{MIRRORING_APP_ID, SessionConfig, negotiate};
