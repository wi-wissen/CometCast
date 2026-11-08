// SPDX-License-Identifier: GPL-3.0-or-later
//! The C ABI (`include/cc_ios.h`). Every function is a thin wrapper around [`crate::engine`];
//! the unsafe part is only turning C pointers into Rust values.
//!
//! Rules for the caller (the Swift extension): strings are NUL-terminated UTF-8 and are copied
//! before the call returns; byte buffers are copied; the status callback runs on a background
//! thread and the `CcIosStatus` it receives (and its `message`) is valid only during the call.
#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use std::sync::Arc;

use crate::engine::{self, Config, ErrorCode, Sink, Status};
use crate::share::{self, Command, Item, ShareConfig};

/// What the status callback receives. Layout is part of the ABI (see `cc_ios.h`).
#[repr(C)]
pub struct CcIosStatus {
    /// `State` as an integer (0 idle .. 7 pending).
    pub state: i32,
    /// `ErrorCode` as an integer.
    pub error: i32,
    /// Check number (10..99) of a join that waits for the host; 0 = none.
    pub check: i32,
    /// Seconds the host has to decide, with `check`; 0 = none.
    pub timeout_secs: i32,
    /// Access units handed to the sender.
    pub pushed: u64,
    /// Access units written to the transport.
    pub sent: u64,
    /// Access units dropped.
    pub dropped: u64,
    /// Bytes written to the transport.
    pub bytes_sent: u64,
    /// Key frames written to the transport.
    pub keyframes_sent: u64,
    /// Key frame requests.
    pub keyframe_requests: u64,
    /// Technical English text, NUL-terminated, valid during the callback only.
    pub message: *const c_char,
    /// Room name the display gave, NUL-terminated, empty = none; valid during the callback only.
    pub room_name: *const c_char,
    /// Connection check code (`123 456`), NUL-terminated, empty until connected; valid during
    /// the callback only.
    pub sas: *const c_char,
}

/// Signature of the status callback.
pub type CcIosStatusCallback =
    Option<unsafe extern "C" fn(status: *const CcIosStatus, user: *mut c_void)>;

/// The user pointer travels to the callback thread; the caller guarantees it stays valid until
/// `cc_ios_stop` returned (documented in the header).
struct UserData(*mut c_void);
// SAFETY: the pointer is only passed back to the caller's callback, never dereferenced here.
unsafe impl Send for UserData {}
unsafe impl Sync for UserData {}
impl UserData {
    // A method (not `.0`) so the closure captures the whole wrapper, which is `Send`.
    fn get(&self) -> *mut c_void {
        self.0
    }
}

fn sink(callback: CcIosStatusCallback, user: *mut c_void) -> Sink {
    let user = UserData(user);
    Arc::new(move |s: &Status| {
        let Some(cb) = callback else { return };
        let cstr = |t: &str| CString::new(t.replace('\0', " ")).unwrap_or_default();
        let message = cstr(&s.message);
        let room_name = cstr(&s.room_name);
        let sas = cstr(&s.sas);
        let c = CcIosStatus {
            state: s.state as i32,
            error: s.error as i32,
            check: i32::from(s.check),
            timeout_secs: i32::try_from(s.timeout_secs).unwrap_or(i32::MAX),
            pushed: s.pushed,
            sent: s.sent,
            dropped: s.dropped,
            bytes_sent: s.bytes_sent,
            keyframes_sent: s.keyframes_sent,
            keyframe_requests: s.keyframe_requests,
            message: message.as_ptr(),
            room_name: room_name.as_ptr(),
            sas: sas.as_ptr(),
        };
        // SAFETY: `cb` is the caller's function; `c` and `message` live until it returns.
        unsafe { cb(&c, user.get()) };
    })
}

unsafe fn text(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the caller passes a NUL-terminated string (header contract).
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

/// Start the broadcast sender. Returns 0 if it was started (progress comes through `callback`),
/// else an `ErrorCode` (1 bad room code, 2 bad relay, 7 internal, 8 already running).
/// `app_group_path` is the directory for `cc-ios.log` (may be NULL). `callback` may be NULL.
/// `name` is the person's name for the display's request and `handover` the single-use token
/// the app got from the display (native-apps.md 20.5); both may be NULL or empty. `device_key`
/// is the secret of the app's device key, 43 characters of base64url (NULL, empty or invalid =
/// the join carries no device proof, docs/design/presenter-identity.md).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_start(
    room_code: *const c_char,
    relay_url: *const c_char,
    app_group_path: *const c_char,
    name: *const c_char,
    handover: *const c_char,
    device_key: *const c_char,
    callback: CcIosStatusCallback,
    user: *mut c_void,
) -> i32 {
    // SAFETY: header contract, see `text`.
    let (code, relay, dir, name, handover, device_key) = unsafe {
        (
            text(room_code),
            text(relay_url),
            text(app_group_path),
            text(name),
            text(handover),
            text(device_key),
        )
    };
    let Some(code) = code else {
        return ErrorCode::InvalidRoomCode as i32;
    };
    let Some(relay) = relay else {
        return ErrorCode::InvalidRelay as i32;
    };
    let config = Config {
        room_code: code,
        relay_url: relay,
        log_dir: dir.filter(|d| !d.is_empty()).map(PathBuf::from),
        udp_bind: None,
        name: name.filter(|n| !n.trim().is_empty()),
        handover: handover.filter(|h| !h.is_empty()),
        device_key: device_key.filter(|k| !k.is_empty()),
    };
    match engine::start(config, sink(callback, user)) {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

/// Give the codec configuration: Annex B with SPS/PPS, or an `avcC` record. Returns 1 if it was
/// accepted, 0 otherwise (nothing runs, or no SPS/PPS in the data).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_set_config(data: *const u8, len: usize) -> i32 {
    if data.is_null() || len == 0 {
        return 0;
    }
    // SAFETY: the caller passes `len` readable bytes.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    i32::from(engine::set_config(bytes))
}

/// Give one H.264 access unit. Returns a `PushCode` (0 queued, 1 not streaming, 2 queue full,
/// 3 waiting for key frame, 4 missing parameter sets, 5 invalid, 6 no sender). Never blocks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_push_frame(
    data: *const u8,
    len: usize,
    is_key: bool,
    pts_us: i64,
) -> i32 {
    if data.is_null() || len == 0 {
        return engine::PushCode::Invalid as i32;
    }
    // SAFETY: the caller passes `len` readable bytes.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    engine::push_frame(bytes, is_key, pts_us) as i32
}

/// Ask the display to hold a new hand-over token for the way back to the app (native-apps.md
/// 20.5, step 5). `token` is 1 to 64 base64url characters (the extension makes 128 random bits).
/// Blocks for at most about four seconds. Returns 1 when the display answered, 0 otherwise
/// (nothing runs, the token is not valid, an old or a web display that does not answer). Call it
/// before `cc_ios_stop`, not from the status callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_request_handover(token: *const c_char) -> i32 {
    // SAFETY: header contract, see `text`.
    let Some(token) = (unsafe { text(token) }) else {
        return 0;
    };
    i32::from(engine::request_handover(&token))
}

/// Returns true once per key frame request; the encoder should then make a sync frame.
#[unsafe(no_mangle)]
pub extern "C" fn cc_ios_keyframe_requested() -> bool {
    engine::keyframe_requested()
}

/// Stop the broadcast and release threads and sockets. Safe to call when nothing runs, and
/// after a terminal status. Blocks for at most about four seconds. Do not call it from the
/// status callback.
#[unsafe(no_mangle)]
pub extern "C" fn cc_ios_stop() {
    engine::stop();
}

/// Start the share extension's session (native-apps.md 21.3). `config_json` is a JSON object
/// `{"code": "K7QM2W", "server": "host:port", "name": "Anna", "log_dir": "/path"}` (`name` and
/// `log_dir` optional). Returns 0 if it was started (events come through
/// `cc_ios_share_poll`), else an `ErrorCode` (1 bad room code, 2 bad relay, 7 internal, 8 already
/// running).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_share_start(config_json: *const c_char) -> i32 {
    // SAFETY: header contract, see `text`.
    let Some(json) = (unsafe { text(config_json) }) else {
        return ErrorCode::InvalidRoomCode as i32;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
        return ErrorCode::InvalidRoomCode as i32;
    };
    let field = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    let config = ShareConfig {
        room_code: field("code").unwrap_or_default(),
        relay_url: field("server").unwrap_or_default(),
        name: field("name"),
        log_dir: field("log_dir")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
        udp_bind: None,
    };
    match share::start(config) {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

/// Show an item: `{"file": "/path"}` or `{"link": "https://..."}`. Returns 1 if the request was
/// queued, 0 if no session runs or the JSON is not one of the two shapes. The checks run
/// afterwards; their result is an event (`content_prepared` or `content_rejected`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_share_present(json: *const c_char) -> i32 {
    // SAFETY: header contract, see `text`.
    let Some(json) = (unsafe { text(json) }) else {
        return 0;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
        return 0;
    };
    let item = if let Some(p) = v.get("file").and_then(|x| x.as_str()) {
        Item::File(PathBuf::from(p))
    } else if let Some(u) = v.get("link").and_then(|x| x.as_str()) {
        Item::Link(u.to_owned())
    } else {
        return 0;
    };
    i32::from(share::present(item))
}

/// A page command: `{"cmd": "prev"}` or `{"cmd": "next"}`. Returns 1 if queued, else 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_share_cmd(json: *const c_char) -> i32 {
    // SAFETY: header contract, see `text`.
    let Some(json) = (unsafe { text(json) }) else {
        return 0;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
        return 0;
    };
    let c = match v.get("cmd").and_then(|x| x.as_str()) {
        Some("prev") => Command::Prev,
        Some("next") => Command::Next,
        _ => return 0,
    };
    i32::from(share::command(c))
}

/// Copy the next event (one JSON object, UTF-8, NUL-terminated) into `buf`. Returns its length
/// (more than 0), 0 if no event waits, or the negative size needed if `len` is too small (the
/// event stays queued). Call it a few times a second; it never blocks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_share_poll(buf: *mut u8, len: usize) -> i32 {
    if buf.is_null() || len == 0 {
        return 0;
    }
    // SAFETY: the caller passes `len` writable bytes.
    let out = unsafe { std::slice::from_raw_parts_mut(buf, len) };
    share::poll_into(out)
}

/// "Stop and close" (and a sheet that was swiped away): stop showing and leave the room. Blocks
/// for at most about four seconds. Safe when nothing runs.
#[unsafe(no_mangle)]
pub extern "C" fn cc_ios_share_stop() {
    share::stop();
}

/// Version of this C ABI. Bumped when a signature or number changes (3: the share functions; 4: `device_key` in `cc_ios_start`).
#[unsafe(no_mangle)]
pub extern "C" fn cc_ios_abi_version() -> i32 {
    4
}

#[cfg(test)]
mod tests {
    /// The header must declare what is exported (cheap guard against drift; the layout of
    /// `CcIosStatus` is checked by hand, see docs/ios.md).
    #[test]
    fn header_matches_the_exported_functions() {
        let header = include_str!("../include/cc_ios.h");
        for name in [
            "cc_ios_start",
            "cc_ios_set_config",
            "cc_ios_push_frame",
            "cc_ios_keyframe_requested",
            "cc_ios_request_handover",
            "cc_ios_share_start",
            "cc_ios_share_present",
            "cc_ios_share_cmd",
            "cc_ios_share_poll",
            "cc_ios_share_stop",
            "cc_ios_stop",
            "cc_ios_abi_version",
        ] {
            assert!(header.contains(name), "{name} missing in cc_ios.h");
        }
        assert!(header.contains("#define CC_IOS_ABI_VERSION 4"));
        assert_eq!(super::cc_ios_abi_version(), 4);
        // 4 ints, 6 counters and 3 pointers (64-bit layout, as on every iOS device).
        assert_eq!(
            std::mem::size_of::<super::CcIosStatus>(),
            4 * 4 + 6 * 8 + 3 * 8
        );
    }
}
