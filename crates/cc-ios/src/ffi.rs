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

/// What the status callback receives. Layout is part of the ABI (see `cc_ios.h`).
#[repr(C)]
pub struct CcIosStatus {
    /// `State` as an integer (0 idle .. 6 failed).
    pub state: i32,
    /// `ErrorCode` as an integer.
    pub error: i32,
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
        let message = CString::new(s.message.replace('\0', " ")).unwrap_or_default();
        let c = CcIosStatus {
            state: s.state as i32,
            error: s.error as i32,
            pushed: s.pushed,
            sent: s.sent,
            dropped: s.dropped,
            bytes_sent: s.bytes_sent,
            keyframes_sent: s.keyframes_sent,
            keyframe_requests: s.keyframe_requests,
            message: message.as_ptr(),
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_ios_start(
    room_code: *const c_char,
    relay_url: *const c_char,
    app_group_path: *const c_char,
    callback: CcIosStatusCallback,
    user: *mut c_void,
) -> i32 {
    // SAFETY: header contract, see `text`.
    let (code, relay, dir) = unsafe { (text(room_code), text(relay_url), text(app_group_path)) };
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

/// Version of this C ABI. Bumped when a signature or number changes.
#[unsafe(no_mangle)]
pub extern "C" fn cc_ios_abi_version() -> i32 {
    1
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
            "cc_ios_stop",
            "cc_ios_abi_version",
        ] {
            assert!(header.contains(name), "{name} missing in cc_ios.h");
        }
        assert!(header.contains("#define CC_IOS_ABI_VERSION 1"));
        assert_eq!(super::cc_ios_abi_version(), 1);
        assert_eq!(std::mem::size_of::<super::CcIosStatus>(), 8 + 6 * 8 + 8);
    }
}
