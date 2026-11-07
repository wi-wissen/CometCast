// SPDX-License-Identifier: GPL-3.0-or-later
//! The Wayland capture backend (xdg-desktop-portal ScreenCast + PipeWire) behind a small C
//! interface, so that the app can load it with `dlopen` (decision N57 of
//! docs/design/first-run-and-display.md). The host side is `cc_capture::pwplugin`.
//!
//! The interface (version 1): every function returns a status; frames are BGRA.
//!
//! ```text
//! u32 ccpw_abi_version(void)
//! i32 ccpw_open(u32 fps, const char *app_id /* nullable */, void **handle)
//! i32 ccpw_start(void *handle)                  /* blocks while the system picker is open */
//! i32 ccpw_next_frame(void *handle, CcPwFrame *out)   /* data valid until the next call */
//! void ccpw_stop(void *handle)
//! void ccpw_close(void *handle)
//! usize ccpw_message(void *handle /* nullable: of the last open */, char *buf, usize len)
//! ```
//!
//! Statuses: 0 ok / a frame, 1 unchanged, 2 ended, 10 denied (cancelled or refused), 11 no
//! portal (or PipeWire), 12 failed (see `ccpw_message`).

#![cfg(all(target_os = "linux", feature = "plugin"))]
// The C interface passes raw pointers. Every `unsafe` block says why it is sound.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};

use cc_capture::{
    Capture, CaptureError, CaptureSource, PICKER_SOURCE_ID, PickerOptions, RawFrame, ScreenSource,
    wayland::WaylandSource,
};

/// Version of this interface; the host refuses another one.
pub const ABI_VERSION: u32 = 1;

/// A picture (or the end of the stream) was delivered.
pub const ST_OK: i32 = 0;
/// Nothing changed since the last picture.
pub const ST_UNCHANGED: i32 = 1;
/// The capture ended (window closed, share stopped in the system UI).
pub const ST_ENDED: i32 = 2;
/// The picker was cancelled or the permission refused.
pub const ST_DENIED: i32 = 10;
/// No ScreenCast portal or no PipeWire.
pub const ST_NO_PORTAL: i32 = 11;
/// Anything else.
pub const ST_FAILED: i32 = 12;

/// One picture. `data` points to `len` bytes owned by the handle.
#[repr(C)]
pub struct CcPwFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// BGRA bytes.
    pub data: *const u8,
    /// Number of bytes.
    pub len: usize,
}

struct Handle {
    source: WaylandSource,
    frame: Option<RawFrame>,
    message: String,
}

thread_local! {
    static OPEN_MESSAGE: RefCell<String> = const { RefCell::new(String::new()) };
}

fn status(e: &CaptureError) -> i32 {
    match e {
        CaptureError::Denied => ST_DENIED,
        CaptureError::NoPortal(_) | CaptureError::Unsupported(_) => ST_NO_PORTAL,
        _ => ST_FAILED,
    }
}

fn guard(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(ST_FAILED)
}

/// The version of this interface.
#[unsafe(no_mangle)]
pub extern "C" fn ccpw_abi_version() -> u32 {
    ABI_VERSION
}

/// Connect to the portal. On success `*handle` is a new handle for the other calls.
///
/// # Safety
/// `app_id` is null or a NUL-terminated string; `handle` is a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ccpw_open(
    fps: u32,
    app_id: *const c_char,
    handle: *mut *mut c_void,
) -> i32 {
    guard(|| {
        if handle.is_null() {
            return ST_FAILED;
        }
        // SAFETY: the caller passes null or a NUL-terminated string.
        let app_id = (!app_id.is_null()).then(|| {
            unsafe { CStr::from_ptr(app_id) }
                .to_string_lossy()
                .into_owned()
        });
        let opts = PickerOptions {
            fps: fps.clamp(1, 60),
            app_id,
            ..PickerOptions::default()
        };
        match WaylandSource::connect(&opts) {
            Ok(source) => {
                let h = Box::new(Handle {
                    source,
                    frame: None,
                    message: String::new(),
                });
                // SAFETY: `handle` is a valid pointer (checked above).
                unsafe { *handle = Box::into_raw(h).cast() };
                ST_OK
            }
            Err(e) => {
                OPEN_MESSAGE.with(|m| *m.borrow_mut() = e.to_string());
                status(&e)
            }
        }
    })
}

fn with_handle(handle: *mut c_void, f: impl FnOnce(&mut Handle) -> i32) -> i32 {
    if handle.is_null() {
        return ST_FAILED;
    }
    // SAFETY: a non-null handle came from `ccpw_open` and is not used from two threads at once
    // (the host owns it exclusively).
    f(unsafe { &mut *handle.cast::<Handle>() })
}

/// Show the system picker and start the stream; blocks until the person chose or cancelled.
///
/// # Safety
/// `handle` is null or a handle of `ccpw_open` that was not closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ccpw_start(handle: *mut c_void) -> i32 {
    guard(|| {
        with_handle(handle, |h| {
            let src = CaptureSource {
                id: PICKER_SOURCE_ID.into(),
                name: "system picker".into(),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            };
            match h.source.start(&src) {
                Ok(()) => ST_OK,
                Err(e) => {
                    h.message = e.to_string();
                    status(&e)
                }
            }
        })
    })
}

/// Wait for the next frame time and fill `out`.
///
/// # Safety
/// `handle` is null or a handle of `ccpw_open` that was not closed; `out` is a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ccpw_next_frame(handle: *mut c_void, out: *mut CcPwFrame) -> i32 {
    guard(|| {
        if out.is_null() {
            return ST_FAILED;
        }
        with_handle(handle, |h| match h.source.next_frame() {
            Ok(Capture::Frame(f)) => {
                let frame = h.frame.insert(f);
                // SAFETY: `out` is a valid pointer; the bytes live in the handle until the next
                // call.
                unsafe {
                    *out = CcPwFrame {
                        width: frame.width,
                        height: frame.height,
                        data: frame.bgra.as_ptr(),
                        len: frame.bgra.len(),
                    };
                }
                ST_OK
            }
            Ok(Capture::Unchanged) => ST_UNCHANGED,
            Ok(Capture::Ended) => ST_ENDED,
            Err(e) => {
                h.message = e.to_string();
                status(&e)
            }
        })
    })
}

/// Stop the capture (the handle stays valid until `ccpw_close`).
///
/// # Safety
/// `handle` is null or a handle of `ccpw_open` that was not closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ccpw_stop(handle: *mut c_void) {
    let _ = guard(|| {
        with_handle(handle, |h| {
            h.source.stop();
            ST_OK
        })
    });
}

/// Free a handle.
///
/// # Safety
/// `handle` is null or a handle of `ccpw_open` that was not closed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ccpw_close(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: the handle came from `Box::into_raw` in `ccpw_open` and is freed once.
        drop(unsafe { Box::from_raw(handle.cast::<Handle>()) });
    }
}

/// Copy the last error message (UTF-8, not NUL-terminated) of `handle`, or of the last
/// `ccpw_open` on this thread when `handle` is null, into `buf`; returns the bytes written.
///
/// # Safety
/// `handle` is null or a live handle; `buf` is valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ccpw_message(handle: *mut c_void, buf: *mut c_char, len: usize) -> usize {
    if buf.is_null() || len == 0 {
        return 0;
    }
    let text = if handle.is_null() {
        OPEN_MESSAGE.with(|m| m.borrow().clone())
    } else {
        // SAFETY: a non-null handle is live (the caller's promise).
        unsafe { &*handle.cast::<Handle>() }.message.clone()
    };
    let n = text.len().min(len);
    // SAFETY: `buf` is valid for `len >= n` bytes and does not overlap `text`.
    unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), buf.cast::<u8>(), n) };
    n
}
