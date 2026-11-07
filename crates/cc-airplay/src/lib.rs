// SPDX-License-Identifier: GPL-3.0-or-later
//! Supervisor for UxPlay, the open-source AirPlay receiver (milestone M3).
//!
//! UxPlay is GPL-3.0 and runs here as a SEPARATE PROCESS. This crate only starts and
//! watches the program: it builds its command line, reads its text output and restarts it.
//! No UxPlay code is linked or copied. See `docs/airplay.md`.
//!
//! Parts:
//! * [`config`]: typed settings and the safe argument list.
//! * [`parser`]: recognises the few interesting output lines (tolerant, display only).
//! * [`supervisor`]: start, watch, restart with backoff, stop without leaving a zombie.
//! * [`policy`]: Comet Cast's own rules (admission by join mode, one session, Comet Cast
//!   first, confirmation hook, status enum for the UI).
//! * [`receive`]: the display's AirPlay receiver as the app uses it (join mode, request, PIN,
//!   trusted "mirroring started" signal, Locked).

pub mod config;
pub mod parser;
pub mod policy;
pub mod receive;
pub mod supervisor;

pub use config::{AirplayConfig, ConfigError, PinFlag, PinMode, PortMode, unsupported_flags};
pub use parser::{MIRRORING_STARTED_LINE, Recognized, parse_line};
pub use policy::{
    AcceptAll, Admission, AirplayStatus, ConfirmHook, ConfirmRequest, Decision, JoinMode,
    PolicyState, RejectReason, ServiceAction, admit_new_session, service_action,
};
pub use receive::{
    AirplayReceiver, ReceiveEvent, ReceiveOptions, has_mirroring_signal, locate_uxplay,
};
pub use supervisor::{AirplayEvent, Backoff, Supervisor, SupervisorOptions, status_after};
