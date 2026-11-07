// SPDX-License-Identifier: GPL-3.0-or-later
//! AirPlay sender without FairPlay (milestone M7, light).
//!
//! What it does:
//!
//! 1. [`discovery`]: finds AirPlay receivers through mDNS (`_airplay._tcp`).
//! 2. [`legacy`] and [`hap`]: pairing and authentication (legacy PIN pairing like UxPlay's,
//!    HomeKit-style transient and PIN pairing) with SRP-6a, HKDF-SHA-512, ChaCha20-Poly1305
//!    and Ed25519/X25519.
//! 3. [`client`]: plays a media link (`/play`, `/playback-info`, `/scrub`, `/stop`) and
//!    shows a photo (`/photo`).
//! 4. [`screen`] with [`hls`] and [`ts`]: shows the screen as a live HLS video stream that
//!    our own small HTTP server serves; the receiver is told to play that URL. The delay is
//!    several seconds.
//!
//! What it does not do, on purpose: real AirPlay screen mirroring. That needs the FairPlay
//! handshake (`fp-setup`), and the project owner forbids taking over FairPlay material.
//! [`mirror::MirroringProvider`] is the empty socket for it and [`mirror::Unsupported`]
//! says why. See `docs/airplay-sender.md` for what was tested and what is UNCERTAIN.
//!
//! Everything a receiver sends is untrusted: sizes are capped, waits have deadlines, property
//! lists are checked for nesting and declared lengths before the `plist` crate sees them, and
//! the code is written not to panic on bad input (tested for the paths in `docs/airplay-sender.md`,
//! not proven for every dependency).

pub mod client;
pub mod creds;
mod cryptoutil;
pub mod discovery;
pub mod error;
pub mod framed;
pub mod hap;
pub mod hls;
pub mod http;
pub mod legacy;
pub mod mirror;
pub mod photo;
mod plistx;
pub mod screen;
pub mod srp;
pub mod tlv;
pub mod ts;
pub mod watch;

#[cfg(test)]
mod testfake;
#[cfg(test)]
mod tests;

pub use client::{AuthKind, PlaybackInfo, Proto, Session};
pub use creds::{Credentials, PairingStore};
pub use discovery::Device;
pub use error::{Result, SenderError};
pub use mirror::{MirroringError, MirroringProvider, Unsupported};
