// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed settings for UxPlay and the safe argument list built from them.
//!
//! We never pass free text from outside to the command line. Every option is a typed field,
//! and the arguments are given to the program one by one (no shell), so a name like
//! `"; rm -rf /"` is just a (rejected or harmless) name.

use std::path::PathBuf;

use thiserror::Error;

/// Longest receiver name we accept. UNCERTAIN: placeholder, UxPlay's own limit is not checked.
pub const MAX_NAME_LEN: usize = 64;

/// How clients must prove they may connect.
///
/// There is deliberately no "no PIN" mode: a device must never be accepted automatically.
/// We also never pass `-reg` to UxPlay: with `-reg`, UxPlay remembers paired devices and
/// lets them in later without a new PIN (see the UxPlay help text in the test fixtures).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PinMode {
    /// A new random 4-digit PIN for every connection (the default).
    #[default]
    PerConnection,
    /// A fixed 4-digit PIN (exactly 4 ASCII digits). Weaker: the PIN is reused.
    Fixed(String),
}

/// Which UxPlay flag turns the PIN on.
///
/// UNCERTAIN: the installed UxPlay 1.68 has `-pin[xxxx]` (checked with its `-h` text). The
/// project plan describes `-pw` for newer UxPlay versions (1.73 and later), which could not
/// be checked here. Use [`unsupported_flags`](crate::unsupported_flags) against `uxplay -h`
/// of the real binary to find out which one it knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinFlag {
    /// `-pin` (UxPlay 1.68, verified against its help text).
    #[default]
    Pin,
    /// `-pw` (named in the plan for newer versions; not verified).
    Pw,
}

/// Which network ports UxPlay uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortMode {
    /// `-p`: the fixed legacy ports (UDP 6000, 6001, 7011; TCP 7000, 7001, 7100).
    /// Fixed ports are needed for a firewall rule. This is the default.
    #[default]
    Legacy,
    /// `-p N`: three consecutive ports starting at N (1024 to 65535 per the help text).
    Base(u16),
    /// No `-p`: UxPlay chooses ports itself (they change on every start).
    Random,
}

/// All settings of the UxPlay process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirplayConfig {
    /// Path of the `uxplay` program. A bare name is looked up in `PATH`.
    pub uxplay_path: PathBuf,
    /// Name shown on iPhone, iPad and Mac (`-n`). The host name is not appended (`-nh`).
    pub name: String,
    /// PIN mode.
    pub pin: PinMode,
    /// Which flag spells the PIN option.
    pub pin_flag: PinFlag,
    /// Full-screen window (`-fs`). UxPlay says this only works with X11, Wayland and VAAPI.
    pub fullscreen: bool,
    /// Ask for H.265 (`-h265`). UNCERTAIN: measurement point A3 expects iOS 27 to need it,
    /// but UxPlay 1.68 does not list this flag, and no iOS 27 device was available. Off by
    /// default here; the plan wants it on only after measurement A3.
    pub h265: bool,
    /// Ports.
    pub ports: PortMode,
    /// GStreamer video sink (`-vs`), for example `fakesink` in tests. `None` = UxPlay default.
    pub video_sink: Option<String>,
    /// Turn audio off (`-as 0`). Normally false.
    pub audio_off: bool,
    /// Debug logging (`-d`). The receiving app needs it: the end of a mirroring is UxPlay's debug
    /// line `Open connections: 0` (see [`crate::MIRRORING_STARTED_LINE`]). Off by default for the
    /// CLI's experiments, on for [`crate::AirplayReceiver`].
    pub debug_log: bool,
    /// The X11 display UxPlay opens its window on (the `DISPLAY` variable of the process), so the
    /// full-screen mirror lands on the screen of the Comet Cast window. `None` = inherit.
    /// UxPlay has no option to choose a monitor of one X screen.
    pub x11_display: Option<String>,
}

impl Default for AirplayConfig {
    fn default() -> Self {
        Self {
            uxplay_path: PathBuf::from("uxplay"),
            name: "Comet Cast".to_string(),
            pin: PinMode::default(),
            pin_flag: PinFlag::default(),
            fullscreen: true,
            h265: false,
            ports: PortMode::default(),
            video_sink: None,
            audio_off: false,
            debug_log: false,
            x11_display: None,
        }
    }
}

/// A setting that cannot be used.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// The receiver name is empty, too long, starts with `-` or has control characters.
    #[error("bad receiver name: {0}")]
    BadName(&'static str),
    /// The fixed PIN is not exactly 4 digits.
    #[error("a fixed PIN must be exactly 4 digits")]
    BadPin,
    /// The base port is below 1024 (or so high that three ports do not fit).
    #[error("base port must be between 1024 and 65533")]
    BadPort,
    /// The video sink is empty, starts with `-` or has control characters.
    #[error("bad video sink name")]
    BadSink,
    /// The X11 display is empty, starts with `-` or has control characters.
    #[error("bad X11 display name")]
    BadDisplay,
}

fn clean_text(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-') && !s.chars().any(char::is_control)
}

impl AirplayConfig {
    /// Check the settings. Called by [`build_args`](Self::build_args).
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.name.is_empty() {
            return Err(ConfigError::BadName("empty"));
        }
        if self.name.chars().count() > MAX_NAME_LEN {
            return Err(ConfigError::BadName("too long"));
        }
        if !clean_text(&self.name) {
            return Err(ConfigError::BadName(
                "starts with '-' or has control characters",
            ));
        }
        if let PinMode::Fixed(p) = &self.pin
            && !(p.len() == 4 && p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(ConfigError::BadPin);
        }
        if let PortMode::Base(p) = self.ports
            && !(1024..=65533).contains(&p)
        {
            return Err(ConfigError::BadPort);
        }
        if let Some(s) = &self.video_sink
            && !clean_text(s)
        {
            return Err(ConfigError::BadSink);
        }
        if let Some(d) = &self.x11_display
            && !clean_text(d)
        {
            return Err(ConfigError::BadDisplay);
        }
        Ok(())
    }

    /// The argument list for the `uxplay` program (without the program name).
    ///
    /// Never contains `-reg`, `-nohold`, `-restrict no`, or anything that would accept
    /// devices without a PIN. `-nohold` is left out on purpose: UxPlay then keeps the
    /// running connection and does not drop it when another device connects (see the help
    /// text in the fixtures); this matches the "one active session" decision.
    pub fn build_args(&self) -> Result<Vec<String>, ConfigError> {
        self.validate()?;
        let mut a: Vec<String> = Vec::new();
        a.push("-n".into());
        a.push(self.name.clone());
        a.push("-nh".into());
        let pin_flag = match self.pin_flag {
            PinFlag::Pin => "-pin",
            PinFlag::Pw => "-pw",
        };
        match &self.pin {
            PinMode::PerConnection => a.push(pin_flag.into()),
            // UxPlay's help: "-pin[xxxx]" (no space between flag and digits).
            PinMode::Fixed(p) => a.push(format!("{pin_flag}{p}")),
        }
        if self.debug_log {
            a.push("-d".into());
        }
        if self.fullscreen {
            a.push("-fs".into());
        }
        if self.h265 {
            a.push("-h265".into());
        }
        match self.ports {
            PortMode::Legacy => a.push("-p".into()),
            PortMode::Base(p) => {
                a.push("-p".into());
                a.push(p.to_string());
            }
            PortMode::Random => {}
        }
        if let Some(s) = &self.video_sink {
            a.push("-vs".into());
            a.push(s.clone());
        }
        if self.audio_off {
            a.push("-as".into());
            a.push("0".into());
        }
        Ok(a)
    }
}

/// Which of our arguments are not mentioned in a `uxplay -h` text.
///
/// Used to warn when a flag (for example `-h265`) is unknown to the installed UxPlay.
/// This is a rough text check: a flag counts as known when some help line starts with it.
/// Option values (the fixed PIN digits, the name, ports) are skipped.
pub fn unsupported_flags(help_text: &str, args: &[String]) -> Vec<String> {
    let known = |flag: &str| {
        help_text.lines().any(|l| {
            let l = l.trim_start();
            l.strip_prefix(flag).is_some_and(|rest| {
                // The flag must end here: space, '[' (optional value) or end of line.
                rest.is_empty() || rest.starts_with([' ', '[', '\t'])
            })
        })
    };
    let mut out = Vec::new();
    let mut skip_value = false;
    for arg in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        if matches!(arg.as_str(), "-n" | "-vs" | "-as") {
            skip_value = true;
        }
        if !arg.starts_with('-') {
            continue; // a value such as the port number after -p
        }
        // "-pin1234" -> "-pin"
        let flag = if arg.starts_with("-pin") {
            "-pin"
        } else if arg.starts_with("-pw") {
            "-pw"
        } else {
            arg.as_str()
        };
        if !known(flag) && !out.iter().any(|f| f == flag) {
            out.push(flag.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELP_168: &str = include_str!("../tests/fixtures/uxplay-1.68-real-help.txt");

    #[test]
    fn default_args_are_safe() {
        let a = AirplayConfig::default().build_args().unwrap();
        assert_eq!(a, ["-n", "Comet Cast", "-nh", "-pin", "-fs", "-p"]);
        for bad in ["-reg", "-nohold", "-restrict", "-h265"] {
            assert!(!a.iter().any(|x| x == bad), "{bad} must not be in {a:?}");
        }
    }

    #[test]
    fn options_map_to_flags() {
        let c = AirplayConfig {
            name: "Room 204".into(),
            pin: PinMode::Fixed("1234".into()),
            pin_flag: PinFlag::Pw,
            fullscreen: false,
            h265: true,
            ports: PortMode::Base(7100),
            video_sink: Some("fakesink".into()),
            audio_off: true,
            debug_log: true,
            ..AirplayConfig::default()
        };
        assert_eq!(
            c.build_args().unwrap(),
            [
                "-n", "Room 204", "-nh", "-pw1234", "-d", "-h265", "-p", "7100", "-vs", "fakesink",
                "-as", "0"
            ]
        );
    }

    #[test]
    fn bad_values_are_rejected() {
        let base = AirplayConfig::default();
        let name = |n: &str| AirplayConfig {
            name: n.into(),
            ..base.clone()
        };
        assert!(name("").validate().is_err());
        assert!(name("-evil").validate().is_err());
        assert!(name("a\nb").validate().is_err());
        assert!(name(&"x".repeat(MAX_NAME_LEN + 1)).validate().is_err());
        assert!(name("Raum 204 \u{e4}").validate().is_ok());
        for p in ["123", "12345", "12a4", ""] {
            let c = AirplayConfig {
                pin: PinMode::Fixed(p.into()),
                ..base.clone()
            };
            assert_eq!(c.validate(), Err(ConfigError::BadPin), "pin {p:?}");
        }
        let c = AirplayConfig {
            ports: PortMode::Base(80),
            ..base.clone()
        };
        assert_eq!(c.validate(), Err(ConfigError::BadPort));
        let c = AirplayConfig {
            video_sink: Some("--x".into()),
            ..base
        };
        assert_eq!(c.validate(), Err(ConfigError::BadSink));
        let c = AirplayConfig {
            x11_display: Some("-x".into()),
            ..AirplayConfig::default()
        };
        assert_eq!(c.validate(), Err(ConfigError::BadDisplay));
    }

    #[test]
    fn pin_is_always_requested_and_never_remembered() {
        // N87: a PIN for every connection; `-reg` (remember paired devices) never.
        let a = AirplayConfig::default().build_args().unwrap();
        assert!(a.iter().any(|x| x == "-pin"));
        assert!(!a.iter().any(|x| x.starts_with("-reg")));
        // `-d` is known to the real 1.68 help text.
        let c = AirplayConfig {
            debug_log: true,
            ..AirplayConfig::default()
        };
        assert!(unsupported_flags(HELP_168, &c.build_args().unwrap()).is_empty());
    }

    #[test]
    fn help_check_against_real_168_text() {
        let mut c = AirplayConfig {
            h265: true,
            pin: PinMode::Fixed("4321".into()),
            ports: PortMode::Base(8000),
            ..AirplayConfig::default()
        };
        c.video_sink = Some("fakesink".into());
        let args = c.build_args().unwrap();
        // Real UxPlay 1.68 does not list -h265 (checked with its -h output).
        assert_eq!(unsupported_flags(HELP_168, &args), ["-h265"]);
        // -pw is not in 1.68 either.
        let args = AirplayConfig {
            pin_flag: PinFlag::Pw,
            ..AirplayConfig::default()
        }
        .build_args()
        .unwrap();
        assert_eq!(unsupported_flags(HELP_168, &args), ["-pw"]);
    }
}
