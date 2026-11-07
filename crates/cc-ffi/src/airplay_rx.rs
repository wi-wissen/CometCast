// SPDX-License-Identifier: GPL-3.0-or-later
//! AirPlay receive of the display app (native-apps.md 21.4): `cc-airplay`'s [`AirplayReceiver`]
//! behind one process-wide slot, its events turned into [`AirplayUiEvent`]. Desktops only.

use std::sync::Mutex;

use cc_airplay::{
    AirplayConfig, AirplayReceiver, ReceiveEvent, ReceiveOptions, has_mirroring_signal,
    locate_uxplay,
};

use crate::api::AirplayUiEvent;
use crate::frb_generated::StreamSink;

static RECEIVER: Mutex<Option<AirplayReceiver>> = Mutex::new(None);

fn slot() -> std::sync::MutexGuard<'static, Option<AirplayReceiver>> {
    RECEIVER.lock().unwrap_or_else(|p| p.into_inner())
}

/// The UxPlay program that can give the mirroring signal, if the computer has one.
pub(crate) fn program() -> Option<std::path::PathBuf> {
    locate_uxplay().filter(|p| has_mirroring_signal(p))
}

pub(crate) fn available() -> bool {
    program().is_some()
}

/// The UI event for one of the receiver's.
pub(crate) fn ui_event(ev: ReceiveEvent) -> AirplayUiEvent {
    match ev {
        ReceiveEvent::Advertising => AirplayUiEvent::Advertising,
        ReceiveEvent::NotAnnouncing => AirplayUiEvent::NotAnnouncing,
        ReceiveEvent::Request => AirplayUiEvent::Request,
        ReceiveEvent::Pin { pin } => AirplayUiEvent::Pin { pin },
        ReceiveEvent::PinDone => AirplayUiEvent::PinDone,
        ReceiveEvent::Mirroring { active } => AirplayUiEvent::Mirroring { active },
        ReceiveEvent::Failed { code } => AirplayUiEvent::Failed { code },
        ReceiveEvent::Stopped => AirplayUiEvent::Stopped,
    }
}

/// The receiver name: the room name cleaned, or "Comet Cast".
pub(crate) fn receiver_name(room_name: &str) -> String {
    let n = cc_discovery::address::clean_room_name(room_name);
    // UxPlay's `-n` must not start with a dash (config check): such a name is not used.
    if n.is_empty() || n.starts_with('-') {
        "Comet Cast".to_owned()
    } else {
        n
    }
}

/// Start (replacing a running receiver) and forward its events until it stops.
pub(crate) fn start(room_name: &str, ask_first: bool, events: StreamSink<AirplayUiEvent>) {
    stop();
    let Some(path) = program() else {
        let _ = events.add(AirplayUiEvent::Failed {
            code: "not_installed".into(),
        });
        return;
    };
    let config = AirplayConfig {
        uxplay_path: path,
        name: receiver_name(room_name),
        fullscreen: true,
        ..AirplayConfig::default()
    };
    let mut options = ReceiveOptions::new(config);
    options.ask_first = ask_first;
    let (receiver, rx) = match AirplayReceiver::start(options) {
        Ok(x) => x,
        Err(_) => {
            let _ = events.add(AirplayUiEvent::Failed {
                code: "bad_config".into(),
            });
            return;
        }
    };
    *slot() = Some(receiver);
    let spawned = std::thread::Builder::new()
        .name("cc-airplay-events".into())
        .spawn(move || {
            for ev in rx {
                let last = ev == ReceiveEvent::Stopped;
                if events.add(ui_event(ev)).is_err() {
                    // Dart stopped listening: stop UxPlay.
                    stop();
                    break;
                }
                if last {
                    break;
                }
            }
        });
    if spawned.is_err() {
        stop();
    }
}

/// Run `f` on the receiver; false if none runs.
pub(crate) fn with(f: impl FnOnce(&AirplayReceiver)) -> bool {
    match slot().as_ref() {
        Some(r) => {
            f(r);
            true
        }
        None => false,
    }
}

pub(crate) fn answer(allow: bool) -> bool {
    slot().as_ref().is_some_and(|r| r.answer(allow))
}

pub(crate) fn stop() {
    let r = slot().take();
    if let Some(r) = r {
        r.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_receiver_name_is_the_room_name_or_comet_cast() {
        assert_eq!(receiver_name("Raum 204"), "Raum 204");
        assert_eq!(receiver_name("   "), "Comet Cast");
        assert_eq!(receiver_name(""), "Comet Cast");
        assert_eq!(receiver_name("-x"), "Comet Cast");
    }

    #[test]
    fn events_map_one_to_one() {
        assert_eq!(
            ui_event(ReceiveEvent::Mirroring { active: true }),
            AirplayUiEvent::Mirroring { active: true }
        );
        assert_eq!(
            ui_event(ReceiveEvent::Pin { pin: "1234".into() }),
            AirplayUiEvent::Pin { pin: "1234".into() }
        );
        assert_eq!(ui_event(ReceiveEvent::Stopped), AirplayUiEvent::Stopped);
        assert_eq!(ui_event(ReceiveEvent::PinDone), AirplayUiEvent::PinDone);
    }

    #[test]
    fn commands_need_a_running_receiver() {
        stop();
        assert!(!with(|r| r.set_locked(true)));
        assert!(!answer(true));
    }
}
