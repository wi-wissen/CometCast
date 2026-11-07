// SPDX-License-Identifier: GPL-3.0-or-later
//! The Wayland backend as a plug-in library, loaded at run time (decision N57 of
//! docs/design/first-run-and-display.md).
//!
//! The portal and PipeWire code (`wayland`, feature `wayland`) links `libpipewire`, and a
//! program that links it does not start on a machine without it, X11 users included. So the app
//! is built without that feature and ships `libcc_capture_pw.so` (`crates/cc-capture-pw`, which
//! has the same code behind a small C interface) next to itself. [`PluginSource`] loads it with
//! `dlopen` when the person taps Screen on a Wayland session. When the library or libpipewire
//! is missing the answer is [`CaptureError::NoPortal`] (app code `capture_no_portal`); an app
//! without the plug-in still starts and still captures X11.
//!
//! The decisions that need no library (where to look, what a status means) are plain functions
//! and tested here; the real calls were checked against the real library in
//! `tests/pwplugin.rs` where PipeWire development files exist.

// `dlopen` and the C calls are `unsafe`. Every `unsafe` block below says why it is sound; the
// crate denies `unsafe_code` everywhere else.
#![allow(unsafe_code)]

use std::ffi::{CString, c_char, c_void};
use std::path::{Path, PathBuf};

use libloading::Library;

use crate::{
    Capture, CaptureError, CaptureSource, PICKER_SOURCE_ID, PickerOptions, RawFrame, ScreenSource,
};

/// File name of the plug-in.
pub const LIBRARY_NAME: &str = "libcc_capture_pw.so";
/// Environment variable with the full path of the plug-in (tests, development).
pub const LIBRARY_ENV: &str = "COMETCAST_CAPTURE_PW_LIB";
/// The interface version this host speaks (`ABI_VERSION` of `cc-capture-pw`).
pub const ABI_VERSION: u32 = 1;

const ST_OK: i32 = 0;
const ST_UNCHANGED: i32 = 1;
const ST_ENDED: i32 = 2;
const ST_DENIED: i32 = 10;
const ST_NO_PORTAL: i32 = 11;

/// Where to look for the plug-in, in this order: the variable, `lib/` next to the program (the
/// Flutter bundle: the program is in the bundle folder, the libraries in `lib/`), the folder of
/// the program, then the system's own search.
pub fn candidate_paths(env: Option<&str>, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(e) = env.filter(|e| !e.is_empty()) {
        v.push(PathBuf::from(e));
    }
    if let Some(dir) = exe.and_then(Path::parent) {
        v.push(dir.join("lib").join(LIBRARY_NAME));
        v.push(dir.join(LIBRARY_NAME));
    }
    v.push(PathBuf::from(LIBRARY_NAME));
    v
}

/// What a status of the plug-in means for the caller.
pub fn map_status(code: i32, message: &str) -> Result<(), CaptureError> {
    match code {
        ST_OK => Ok(()),
        ST_DENIED => Err(CaptureError::Denied),
        ST_NO_PORTAL => Err(CaptureError::NoPortal(
            "no ScreenCast desktop portal or no PipeWire on this desktop",
        )),
        _ => Err(CaptureError::Display(message.to_owned())),
    }
}

#[repr(C)]
struct CFrame {
    width: u32,
    height: u32,
    data: *const u8,
    len: usize,
}

type AbiFn = unsafe extern "C" fn() -> u32;
type OpenFn = unsafe extern "C" fn(u32, *const c_char, *mut *mut c_void) -> i32;
type HandleFn = unsafe extern "C" fn(*mut c_void) -> i32;
type NextFn = unsafe extern "C" fn(*mut c_void, *mut CFrame) -> i32;
type VoidFn = unsafe extern "C" fn(*mut c_void);
type MsgFn = unsafe extern "C" fn(*mut c_void, *mut c_char, usize) -> usize;

/// The Wayland backend in the plug-in library.
pub struct PluginSource {
    handle: *mut c_void,
    start: HandleFn,
    next: NextFn,
    stop: VoidFn,
    close: VoidFn,
    message: MsgFn,
    // Last: the library is unloaded after the handle was closed in `drop`.
    _lib: Library,
}

// SAFETY: the handle is owned by this value alone and every call goes through `&mut self` (or
// `&self` without touching it), so the library never sees two threads at once.
unsafe impl Send for PluginSource {}

impl std::fmt::Debug for PluginSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginSource").finish_non_exhaustive()
    }
}

fn load(paths: &[PathBuf]) -> Result<Library, CaptureError> {
    for p in paths {
        // SAFETY: loading a library runs its initialisers; this is the plug-in of the app (or a
        // library the administrator put in the search path), as with every dlopen.
        if let Ok(l) = unsafe { Library::new(p) } {
            return Ok(l);
        }
    }
    // A library that is missing, or whose own dependency (libpipewire) is missing.
    Err(CaptureError::NoPortal(
        "the PipeWire capture library cannot be loaded (libpipewire missing?)",
    ))
}

impl PluginSource {
    /// Load the plug-in from `paths` and connect to the portal. Shows nothing.
    pub fn connect_from(paths: &[PathBuf], opts: &PickerOptions) -> Result<Self, CaptureError> {
        let lib = load(paths)?;
        // SAFETY: the symbols have the signatures of crates/cc-capture-pw (`ABI_VERSION` is
        // checked first); the function pointers are only used while `_lib` is alive.
        let (open, start, next, stop, close, message) = unsafe {
            let abi: AbiFn = *lib
                .get::<AbiFn>(b"ccpw_abi_version\0")
                .map_err(|_| CaptureError::NoPortal("not a Comet Cast capture library"))?;
            if abi() != ABI_VERSION {
                return Err(CaptureError::NoPortal(
                    "the PipeWire capture library is of another version",
                ));
            }
            let bad = |_| CaptureError::NoPortal("the PipeWire capture library is incomplete");
            (
                *lib.get::<OpenFn>(b"ccpw_open\0").map_err(bad)?,
                *lib.get::<HandleFn>(b"ccpw_start\0").map_err(bad)?,
                *lib.get::<NextFn>(b"ccpw_next_frame\0").map_err(bad)?,
                *lib.get::<VoidFn>(b"ccpw_stop\0").map_err(bad)?,
                *lib.get::<VoidFn>(b"ccpw_close\0").map_err(bad)?,
                *lib.get::<MsgFn>(b"ccpw_message\0").map_err(bad)?,
            )
        };
        let app_id = opts.app_id.as_deref().and_then(|a| CString::new(a).ok());
        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: `app_id` is null or NUL-terminated and outlives the call; `handle` is valid.
        let code = unsafe {
            open(
                opts.fps,
                app_id.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
                &mut handle,
            )
        };
        if code != ST_OK || handle.is_null() {
            let msg = read_message(message, std::ptr::null_mut());
            let code = if code == ST_OK { 12 } else { code };
            map_status(code, &msg)?;
            return Err(CaptureError::Display(msg));
        }
        Ok(Self {
            handle,
            start,
            next,
            stop,
            close,
            message,
            _lib: lib,
        })
    }

    /// Load the plug-in from the usual places ([`candidate_paths`]) and connect to the portal.
    pub fn connect(opts: &PickerOptions) -> Result<Self, CaptureError> {
        let env = std::env::var(LIBRARY_ENV).ok();
        let exe = std::env::current_exe().ok();
        Self::connect_from(&candidate_paths(env.as_deref(), exe.as_deref()), opts)
    }
}

fn read_message(f: MsgFn, handle: *mut c_void) -> String {
    let mut buf = [0u8; 512];
    // SAFETY: `buf` is valid for its length; `handle` is null or live.
    let n = unsafe { f(handle, buf.as_mut_ptr().cast(), buf.len()) };
    String::from_utf8_lossy(&buf[..n.min(buf.len())]).into_owned()
}

impl ScreenSource for PluginSource {
    fn list_sources(&self) -> Result<Vec<CaptureSource>, CaptureError> {
        Ok(vec![CaptureSource {
            id: PICKER_SOURCE_ID.into(),
            name: "system picker".into(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }])
    }

    fn start(&mut self, _source: &CaptureSource) -> Result<(), CaptureError> {
        // SAFETY: the handle is live until `drop`.
        let code = unsafe { (self.start)(self.handle) };
        map_status(code, &read_message(self.message, self.handle))
    }

    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        let mut out = CFrame {
            width: 0,
            height: 0,
            data: std::ptr::null(),
            len: 0,
        };
        // SAFETY: the handle is live; `out` is valid.
        let code = unsafe { (self.next)(self.handle, &mut out) };
        match code {
            ST_OK if !out.data.is_null() => {
                // SAFETY: the library keeps `len` bytes at `data` until the next call.
                let bytes = unsafe { std::slice::from_raw_parts(out.data, out.len) }.to_vec();
                let frame = RawFrame {
                    width: out.width,
                    height: out.height,
                    bgra: bytes,
                };
                Ok(Capture::Frame(frame))
            }
            ST_UNCHANGED => Ok(Capture::Unchanged),
            ST_ENDED => Ok(Capture::Ended),
            other => {
                map_status(other, &read_message(self.message, self.handle))?;
                Ok(Capture::Unchanged)
            }
        }
    }

    fn stop(&mut self) {
        // SAFETY: the handle is live until `drop`.
        unsafe { (self.stop)(self.handle) };
    }
}

impl Drop for PluginSource {
    fn drop(&mut self) {
        // SAFETY: the handle is live and closed exactly once; the library is still loaded
        // (`_lib` is dropped after this body).
        unsafe {
            (self.stop)(self.handle);
            (self.close)(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_order_is_variable_bundle_folder_system() {
        let p = candidate_paths(Some("/x/lib.so"), Some(Path::new("/app/cometcast")));
        assert_eq!(
            p,
            vec![
                PathBuf::from("/x/lib.so"),
                PathBuf::from("/app/lib/libcc_capture_pw.so"),
                PathBuf::from("/app/libcc_capture_pw.so"),
                PathBuf::from("libcc_capture_pw.so"),
            ]
        );
        assert_eq!(candidate_paths(Some(""), None).len(), 1);
    }

    #[test]
    fn statuses_map_to_the_capture_errors() {
        assert!(map_status(0, "").is_ok());
        assert_eq!(map_status(10, ""), Err(CaptureError::Denied));
        assert!(matches!(map_status(11, ""), Err(CaptureError::NoPortal(_))));
        assert_eq!(
            map_status(12, "boom"),
            Err(CaptureError::Display("boom".into()))
        );
    }

    #[test]
    fn a_missing_library_is_no_portal_not_a_crash() {
        let r = PluginSource::connect_from(
            &[PathBuf::from("/nonexistent/libcc_capture_pw.so")],
            &PickerOptions::default(),
        );
        assert!(matches!(r, Err(CaptureError::NoPortal(_))), "{r:?}");
    }

    #[test]
    fn a_file_that_is_no_library_is_no_portal_too() {
        let dir = std::env::temp_dir().join(format!("ccpw-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("libcc_capture_pw.so");
        std::fs::write(&f, b"not an ELF file").unwrap();
        let r = PluginSource::connect_from(&[f], &PickerOptions::default());
        assert!(matches!(r, Err(CaptureError::NoPortal(_))), "{r:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
