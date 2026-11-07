// SPDX-License-Identifier: GPL-3.0-or-later
//! The `extern "C"` functions. See `docs/miracast.md`, section "C interface", for the table.
//!
//! Rules: no panic crosses the boundary (every body runs in `catch_unwind`), no pointer is
//! kept after a call returns, every pointer argument is checked for null and every length is
//! checked before it is used. Return codes: `>= 0` success (some functions return a length),
//! negative or the listed constants on failure.
#![allow(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use cc_miracast::{MiracastConfig, probe};
use serde::Deserialize;

use crate::controller::{CallError, Controller, FrameTake, Starter, parse_pairing, probe_json};

/// Version of this interface. Bumped when a function or the JSON changes incompatibly.
pub const ABI_VERSION: u32 = 1;

/// Success.
pub const CC_OK: i32 = 0;
/// A receiver is already starting, running or stopping.
pub const CC_ERR_BUSY: i32 = -1;
/// Bad argument (null pointer, bad JSON, unknown pairing name, text too long).
pub const CC_ERR_ARG: i32 = -2;
/// The output buffer is too small.
pub const CC_ERR_BUFFER: i32 = -3;
/// No receiver is running.
pub const CC_ERR_NOT_RUNNING: i32 = -4;
/// No request with this id is waiting.
pub const CC_ERR_NO_REQUEST: i32 = -5;
/// Windows refused or failed.
pub const CC_ERR_FAILED: i32 = -6;
/// An internal panic was caught.
pub const CC_ERR_PANIC: i32 = -7;

/// Longest start JSON we read, in bytes.
const MAX_JSON_BYTES: usize = 4096;

fn controller() -> &'static Controller {
    static C: OnceLock<Controller> = OnceLock::new();
    C.get_or_init(Controller::new)
}

fn guarded(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(CC_ERR_PANIC)
}

fn call_error(e: CallError) -> i32 {
    match e {
        CallError::Busy => CC_ERR_BUSY,
        CallError::NotRunning => CC_ERR_NOT_RUNNING,
        CallError::NoSuchRequest => CC_ERR_NO_REQUEST,
        CallError::Failed => CC_ERR_FAILED,
    }
}

/// Copy `text` into the caller's buffer; returns its length, or [`CC_ERR_BUFFER`].
///
/// # Safety
/// `out` must be null or point to `cap` writable bytes.
unsafe fn write_text(text: &str, out: *mut u8, cap: usize) -> i32 {
    if out.is_null() {
        return CC_ERR_ARG;
    }
    let bytes = text.as_bytes();
    if bytes.len() > cap || i32::try_from(bytes.len()).is_err() {
        return CC_ERR_BUFFER;
    }
    // SAFETY: `out` is non-null and the caller promises `cap >= bytes.len()` writable bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    i32::try_from(bytes.len()).unwrap_or(CC_ERR_BUFFER)
}

/// Version of this interface ([`ABI_VERSION`]).
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_abi_version() -> u32 {
    ABI_VERSION
}

/// Allocate `len` bytes (zeroed) that the caller may pass to the functions below. Free with
/// [`cc_miracast_free`] and the same `len`. Returns null for 0 or on failure.
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_alloc(len: usize) -> *mut u8 {
    if len == 0 || len > (1 << 28) {
        return std::ptr::null_mut();
    }
    let boxed: Box<[u8]> = vec![0u8; len].into_boxed_slice();
    Box::into_raw(boxed).cast::<u8>()
}

/// Free a block from [`cc_miracast_alloc`].
///
/// # Safety
/// `ptr` must come from `cc_miracast_alloc(len)` with the same `len` and not be freed twice;
/// null is ignored.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_miracast_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: by the contract, `ptr`/`len` describe a boxed slice made by `cc_miracast_alloc`.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
}

/// Look at the system without starting anything. Writes JSON; returns its length.
///
/// # Safety
/// `out` must point to `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_miracast_probe(out: *mut u8, cap: usize) -> i32 {
    // SAFETY: forwarded contract.
    guarded(|| unsafe { write_text(&probe_json(&probe()), out, cap) })
}

#[derive(Deserialize)]
struct StartJson {
    name: String,
    pairing: String,
}

/// Start the receiver. `json` is `{"name":"Comet Cast","pairing":"confirm"}` (pairing:
/// `confirm`, `pin_if_requested` or `pin_required`). Returns at once; poll the state.
///
/// # Safety
/// `json` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_miracast_start(json: *const u8, len: usize) -> i32 {
    guarded(|| {
        if json.is_null() || len == 0 || len > MAX_JSON_BYTES {
            return CC_ERR_ARG;
        }
        // SAFETY: non-null, and the caller promises `len` readable bytes.
        let bytes = unsafe { std::slice::from_raw_parts(json, len) };
        let Ok(parsed) = serde_json::from_slice::<StartJson>(bytes) else {
            return CC_ERR_ARG;
        };
        let Some(pairing) = parse_pairing(&parsed.pairing) else {
            return CC_ERR_ARG;
        };
        let config = MiracastConfig {
            display_name: parsed.name,
            pairing,
            ..MiracastConfig::default()
        };
        if config.validate().is_err() {
            return CC_ERR_ARG;
        }
        let starter: Starter = Box::new(|config| {
            let (receiver, events) = cc_miracast::MiracastReceiver::start(config)?;
            Ok((
                Box::new(receiver) as Box<dyn cc_miracast::ReceiverBackend>,
                events,
            ))
        });
        match controller().start(config, probe(), starter) {
            Ok(()) => CC_OK,
            Err(e) => call_error(e),
        }
    })
}

/// Stop the receiver (on a thread of its own). Safe when nothing runs.
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_stop() -> i32 {
    guarded(|| {
        controller().stop();
        CC_OK
    })
}

/// The state as JSON. Returns its length.
///
/// # Safety
/// `out` must point to `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_miracast_state(out: *mut u8, cap: usize) -> i32 {
    // SAFETY: forwarded contract.
    guarded(|| unsafe { write_text(&controller().state_json(), out, cap) })
}

/// Answer the waiting connection request: `accept` is 1 for yes, anything else for no.
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_respond(id: u64, accept: i32) -> i32 {
    guarded(|| match controller().respond(id, accept == 1) {
        Ok(()) => CC_OK,
        Err(e) => call_error(e),
    })
}

/// End the active connection and keep listening.
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_disconnect() -> i32 {
    guarded(|| match controller().disconnect() {
        Ok(()) => CC_OK,
        Err(e) => call_error(e),
    })
}

/// Largest picture size handed out by `cc_miracast_poll_frame` (larger pictures are scaled
/// down, aspect ratio kept).
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_set_max_size(width: u32, height: u32) -> i32 {
    guarded(|| {
        controller().set_max_frame_size(width, height);
        CC_OK
    })
}

/// Mute (1) or unmute (0) the audio of the stream.
#[unsafe(no_mangle)]
pub extern "C" fn cc_miracast_set_muted(muted: i32) -> i32 {
    guarded(|| {
        controller().set_muted(muted != 0);
        CC_OK
    })
}

/// Result of [`cc_miracast_poll_frame`].
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CcFrameInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Sequence number of the picture (grows by one per published picture).
    pub seq: u64,
}

/// Take the newest picture into the caller's buffer (BGRA, rows without padding,
/// `width * height * 4` bytes). Returns 1 if a picture was copied (`info` is filled), 0 if
/// there is none, [`CC_ERR_BUFFER`] if `cap` is too small (`info` then holds the size that
/// is needed; the picture stays in the mailbox), or another negative code.
///
/// # Safety
/// `buf` must point to `cap` writable bytes and `info` to a writable `CcFrameInfo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cc_miracast_poll_frame(
    buf: *mut u8,
    cap: usize,
    info: *mut CcFrameInfo,
) -> i32 {
    guarded(|| {
        if buf.is_null() || info.is_null() {
            return CC_ERR_ARG;
        }
        // SAFETY: non-null; the caller promises `cap` writable bytes.
        let slice = unsafe { std::slice::from_raw_parts_mut(buf, cap) };
        match controller().take_frame_into(slice) {
            FrameTake::None => 0,
            FrameTake::TooSmall { width, height } => {
                // SAFETY: `info` is non-null and writable by the contract.
                unsafe {
                    info.write(CcFrameInfo {
                        width,
                        height,
                        seq: 0,
                    })
                };
                CC_ERR_BUFFER
            }
            FrameTake::Frame { width, height, seq } => {
                // SAFETY: as above.
                unsafe { info.write(CcFrameInfo { width, height, seq }) };
                1
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_alloc_roundtrip() {
        assert_eq!(cc_miracast_abi_version(), ABI_VERSION);
        let p = cc_miracast_alloc(64);
        assert!(!p.is_null());
        // SAFETY: allocated just above with the same length.
        unsafe { cc_miracast_free(p, 64) };
        assert!(cc_miracast_alloc(0).is_null());
        // SAFETY: null is allowed.
        unsafe { cc_miracast_free(std::ptr::null_mut(), 5) };
    }

    #[test]
    fn bad_arguments_are_refused() {
        // SAFETY: null pointers are checked by the functions.
        unsafe {
            assert_eq!(cc_miracast_start(std::ptr::null(), 10), CC_ERR_ARG);
            assert_eq!(cc_miracast_state(std::ptr::null_mut(), 10), CC_ERR_ARG);
            assert_eq!(
                cc_miracast_poll_frame(std::ptr::null_mut(), 0, std::ptr::null_mut()),
                CC_ERR_ARG
            );
        }
        let bad = br#"{"name":"x","pairing":"everyone"}"#;
        // SAFETY: valid slice.
        assert_eq!(
            unsafe { cc_miracast_start(bad.as_ptr(), bad.len()) },
            CC_ERR_ARG
        );
        let empty_name = br#"{"name":"  ","pairing":"confirm"}"#;
        // SAFETY: valid slice.
        assert_eq!(
            unsafe { cc_miracast_start(empty_name.as_ptr(), empty_name.len()) },
            CC_ERR_ARG
        );
    }

    #[test]
    fn small_buffer_is_reported() {
        let mut b = [0u8; 4];
        // SAFETY: valid buffer.
        assert_eq!(
            unsafe { cc_miracast_state(b.as_mut_ptr(), b.len()) },
            CC_ERR_BUFFER
        );
    }
}
