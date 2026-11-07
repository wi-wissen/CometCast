// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows capture with Windows.Graphics.Capture and the system's `GraphicsCapturePicker`.
//!
//! Flow of `start`: initialise the Windows Runtime on the calling thread (MTA); show the
//! picker (the system dialog; screens and windows); on cancel return [`CaptureError::Denied`];
//! create a Direct3D 11 device, a free-threaded frame pool (BGRA, 2 buffers) and a capture
//! session for the picked item; each `FrameArrived` copies the texture into a CPU-readable
//! staging texture, converts it with [`crate::convert::to_bgra`] and puts it into the shared
//! mailbox, from which `next_frame` takes the newest picture at the requested rate.
//!
//! The decisions that do not need Windows (error codes, size changes, crop) are in
//! [`crate::winlogic`] and tested everywhere. **Nothing in this file was run**: it is compiled
//! for `x86_64-pc-windows-gnu` only; docs/windows-capture-test-plan.md lists what to check on a
//! machine.
//!
//! What the system does that this code does not touch: the yellow capture border (privacy
//! indicator; hiding it needs a capability and extra consent) and the stop control.
//! Cursor capture is requested. Pictures are in physical pixels regardless of DPI scaling.

// The Windows Runtime and Direct3D calls are `unsafe` in the bindings. Every `unsafe` block
// below has a SAFETY comment; the crate denies `unsafe_code` everywhere else.
#![allow(unsafe_code)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCapturePicker, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::Shell::IInitializeWithWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetClassNameW, GetForegroundWindow, GetWindow, GetWindowThreadProcessId,
    IsWindowVisible,
};
use windows::core::{BOOL, IInspectable, Interface};

use crate::convert::{PixelFormat, to_bgra};
use crate::session::{Event, Pacer, Phase, Shared, Taken};
use crate::winlogic::{SizeTracker, content_crop, map_pick_error};
use crate::{
    Capture, CaptureError, CaptureSource, ChangeDetector, PICKER_SOURCE_ID, PickerOptions,
    ScreenSource,
};

/// The window class of the Flutter runner (`apps/flutter/windows/runner/win32_window.cpp`).
const RUNNER_CLASS: &str = "FLUTTER_RUNNER_WIN32_WINDOW";

struct OwnWindows {
    pid: u32,
    /// The first visible top-level window of this process without an owner.
    any: Option<isize>,
    /// The same, with the runner's window class.
    runner: Option<isize>,
}

unsafe extern "system" fn collect_own(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the address of the `OwnWindows` that `own_window` keeps alive for the
    // whole `EnumWindows` call.
    let found = unsafe { &mut *(lparam.0 as *mut OwnWindows) };
    let mut pid = 0u32;
    // SAFETY: `hwnd` comes from the enumeration; `pid` is a valid pointer.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    // SAFETY: plain calls with the window handle of the enumeration.
    let top = pid == found.pid
        && unsafe { IsWindowVisible(hwnd) }.as_bool()
        && unsafe { GetWindow(hwnd, GW_OWNER) }.is_err_or_null();
    if top {
        found.any.get_or_insert(hwnd.0 as isize);
        let mut name = [0u16; 64];
        // SAFETY: the buffer is valid for its length.
        let n = unsafe { GetClassNameW(hwnd, &mut name) } as usize;
        if String::from_utf16_lossy(&name[..n.min(64)]) == RUNNER_CLASS {
            found.runner.get_or_insert(hwnd.0 as isize);
        }
    }
    BOOL(1)
}

trait OwnerCheck {
    fn is_err_or_null(&self) -> bool;
}

impl OwnerCheck for windows::core::Result<HWND> {
    fn is_err_or_null(&self) -> bool {
        self.as_ref().map_or(true, |h| h.0.is_null())
    }
}

/// The handle of this program's own main window (the Flutter runner's window class, else the
/// first visible top-level window of the process), or `None` for a program without one.
pub fn own_window() -> Option<u64> {
    let mut found = OwnWindows {
        // SAFETY: plain call without arguments.
        pid: unsafe { GetCurrentProcessId() },
        any: None,
        runner: None,
    };
    // SAFETY: the callback only reads and writes `found`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect_own), LPARAM(&mut found as *mut _ as isize)) };
    found.runner.or(found.any).map(|h| h as usize as u64)
}

/// How long `start` waits for the first picture after the person picked an item.
/// UNCERTAIN: placeholder.
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);

/// Frames in the pool. Two is the usual minimum for a consumer that copies at once.
const POOL_BUFFERS: i32 = 2;

fn werr(context: &'static str) -> impl Fn(windows::core::Error) -> CaptureError {
    move |e| CaptureError::Display(format!("{context}: {e}"))
}

/// Create a Direct3D 11 device with BGRA support; WARP (software) if no hardware device.
/// UNCERTAIN: the software fallback is for machines without a usable GPU driver.
fn create_device() -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
    let make = |kind| -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        // SAFETY: all pointers refer to locals that live for the call; the out values receive
        // new references on success.
        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                kind,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        match (device, context) {
            (Some(d), Some(c)) => Ok((d, c)),
            _ => Err(windows::core::Error::from(
                windows::Win32::Foundation::E_POINTER,
            )),
        }
    };
    make(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| make(D3D_DRIVER_TYPE_WARP))
}

/// The Direct3D state used by the frame handler. Only the handler thread(s) touch it, behind
/// a mutex (the immediate context is not thread safe).
struct Gpu {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt_device: IDirect3DDevice,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
    size: SizeTracker,
}

/// Wrapper that lets the COM objects move between threads.
struct ThreadSafe<T>(T);
// SAFETY: the Direct3D 11 device is free-threaded (we do not create it with
// `D3D11_CREATE_DEVICE_SINGLETHREADED`); the immediate context and the staging texture are
// only used while holding the `Mutex` that owns this wrapper. Same reasoning as in
// `cc-miracast`'s player.
unsafe impl<T> Send for ThreadSafe<T> {}

impl Gpu {
    /// Copy the texture of `frame_surface` into the staging texture and return its pixels as
    /// a tight BGRA picture, cut to `content`.
    fn read(
        &mut self,
        texture: &ID3D11Texture2D,
        content: (u32, u32),
    ) -> Result<crate::RawFrame, CaptureError> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: `desc` is a local that outlives the call.
        unsafe { texture.GetDesc(&mut desc) };
        let (w, h) = (desc.Width, desc.Height);
        let needs_new = !matches!(&self.staging, Some((_, sw, sh)) if (*sw, *sh) == (w, h));
        if needs_new {
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                ..desc
            };
            let mut tex: Option<ID3D11Texture2D> = None;
            // SAFETY: `staging_desc` and `tex` are locals that outlive the call; no initial data.
            unsafe {
                self.device
                    .CreateTexture2D(&staging_desc, None, Some(&mut tex))
                    .map_err(werr("cannot create the staging texture"))?;
            }
            let tex = tex.ok_or_else(|| CaptureError::Display("no staging texture".into()))?;
            self.staging = Some((tex, w, h));
        }
        let Some((staging, _, _)) = &self.staging else {
            return Err(CaptureError::Display("no staging texture".into()));
        };
        let dst: ID3D11Resource = staging.cast().map_err(werr("texture cast"))?;
        let src: ID3D11Resource = texture.cast().map_err(werr("texture cast"))?;
        // SAFETY: both resources belong to this device and have the same size and format.
        unsafe { self.context.CopyResource(&dst, &src) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: `dst` is a staging texture with CPU read access; `mapped` is a local.
        unsafe {
            self.context
                .Map(&dst, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(werr("cannot map the staging texture"))?;
        }
        let stride = mapped.RowPitch as usize;
        let len = stride * h as usize;
        // SAFETY: after a successful `Map`, `pData` points to `RowPitch * Height` readable bytes
        // that stay valid until `Unmap`, which happens after the copy below.
        let result = unsafe {
            let bytes = std::slice::from_raw_parts(mapped.pData as *const u8, len);
            to_bgra(
                PixelFormat::Bgra,
                bytes,
                w,
                h,
                stride,
                content_crop((w, h), content),
            )
        };
        // SAFETY: the resource was mapped above.
        unsafe { self.context.Unmap(&dst, 0) };
        result
    }
}

struct Running {
    item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    frame_token: i64,
    closed_token: i64,
    // Keeps the device alive for the WinRT objects.
    _gpu: Arc<Mutex<ThreadSafe<Gpu>>>,
}

/// Windows screen capture with the system picker.
pub struct WgcSource {
    opts: PickerOptions,
    shared: Arc<Shared>,
    running: Option<Running>,
    pacer: Pacer,
    detector: ChangeDetector,
    first: Option<crate::RawFrame>,
}

// SAFETY: the WinRT objects in `Running` are agile (free-threaded frame pool, item, session)
// and are only used from the thread that owns the `WgcSource` or through their own thread
// safe methods (`Close`, event unregistration).
unsafe impl Send for WgcSource {}

impl std::fmt::Debug for WgcSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgcSource").finish_non_exhaustive()
    }
}

impl WgcSource {
    /// Check that Windows.Graphics.Capture exists (Windows 10 1803 or newer) and prepare.
    pub fn new(opts: &PickerOptions) -> Result<Self, CaptureError> {
        // SAFETY: plain call; S_FALSE / RPC_E_CHANGED_MODE (already initialised) are fine.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        match GraphicsCaptureSession::IsSupported() {
            Ok(true) => {}
            Ok(false) => {
                return Err(CaptureError::Unsupported(
                    "Windows.Graphics.Capture is not supported on this Windows version",
                ));
            }
            Err(e) => {
                return Err(CaptureError::Display(format!(
                    "cannot ask for Graphics Capture support: {e}"
                )));
            }
        }
        Ok(Self {
            opts: opts.clone(),
            shared: Arc::new(Shared::new()),
            running: None,
            pacer: Pacer::new(opts.fps),
            detector: ChangeDetector::new(),
            first: None,
        })
    }

    /// Show the system picker and return the chosen item.
    fn pick_item(&self) -> Result<GraphicsCaptureItem, CaptureError> {
        let picker = GraphicsCapturePicker::new().map_err(werr("cannot create the picker"))?;
        // Decision N55: the picker is owned by the Comet Cast window, never by whatever window
        // is in front. An explicit handle wins; else the app's own top-level window; the
        // foreground window only where this process has none (a command-line program).
        let owner = match self.opts.owner_window.or_else(own_window) {
            Some(h) => HWND(h as usize as *mut _),
            // SAFETY: plain call without arguments.
            None => unsafe { GetForegroundWindow() },
        };
        let init: IInitializeWithWindow = picker.cast().map_err(werr("picker window init"))?;
        // SAFETY: `owner` is a window handle (possibly null, which the call rejects).
        unsafe { init.Initialize(owner) }.map_err(werr("cannot set the picker owner window"))?;
        let op = picker
            .PickSingleItemAsync()
            .map_err(|e| map_pick_error(e.code().0, "cannot open the picker"))?;
        op.join()
            .map_err(|e| map_pick_error(e.code().0, "the picker failed"))
    }
}

impl ScreenSource for WgcSource {
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
        if self.running.is_some() {
            return Err(CaptureError::Display("capture is already running".into()));
        }
        self.shared = Arc::new(Shared::new());
        self.shared.event(Event::PickerShown);
        let item = match self.pick_item() {
            Ok(i) => i,
            Err(e) => {
                self.shared.event(match &e {
                    CaptureError::Denied => Event::PickerCancelled,
                    other => Event::Error(other.to_string()),
                });
                return Err(e);
            }
        };
        self.shared.event(Event::Picked);

        let (device, context) = create_device().map_err(werr("cannot create a Direct3D device"))?;
        let dxgi: IDXGIDevice = device.cast().map_err(werr("DXGI device"))?;
        // SAFETY: `dxgi` is a valid DXGI device of the device created above.
        let inspectable: IInspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
            .map_err(werr("cannot create the WinRT device"))?;
        let winrt_device: IDirect3DDevice = inspectable.cast().map_err(werr("WinRT device"))?;

        let size = item.Size().map_err(werr("cannot read the item size"))?;
        let (w, h) = (size.Width.max(1) as u32, size.Height.max(1) as u32);
        let gpu = Arc::new(Mutex::new(ThreadSafe(Gpu {
            device,
            context,
            winrt_device: winrt_device.clone(),
            staging: None,
            size: SizeTracker::new(w, h),
        })));

        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            POOL_BUFFERS,
            size,
        )
        .map_err(werr("cannot create the frame pool"))?;

        let handler_gpu = Arc::clone(&gpu);
        let handler_shared = Arc::clone(&self.shared);
        let frame_token = pool
            .FrameArrived(&TypedEventHandler::new(
                move |pool: windows::core::Ref<'_, Direct3D11CaptureFramePool>,
                      _: windows::core::Ref<'_, IInspectable>| {
                    let Some(pool) = pool.as_ref() else {
                        return Ok(());
                    };
                    on_frame(pool, &handler_gpu, &handler_shared);
                    Ok(())
                },
            ))
            .map_err(werr("cannot subscribe to frames"))?;

        let closed_shared = Arc::clone(&self.shared);
        let closed_token = item
            .Closed(&TypedEventHandler::new(
                move |_: windows::core::Ref<'_, GraphicsCaptureItem>,
                      _: windows::core::Ref<'_, IInspectable>| {
                    // The window was closed or the monitor went away.
                    closed_shared.event(Event::SourceClosed);
                    Ok(())
                },
            ))
            .map_err(werr("cannot subscribe to the item"))?;

        let session = pool
            .CreateCaptureSession(&item)
            .map_err(werr("cannot create the capture session"))?;
        // Cursor on (needs Windows 10 2004; the call is ignored when it fails). The yellow
        // border is left to the system.
        let _ = session.SetIsCursorCaptureEnabled(true);
        session
            .StartCapture()
            .map_err(werr("cannot start the capture"))?;

        self.running = Some(Running {
            item,
            pool,
            session,
            frame_token,
            closed_token,
            _gpu: gpu,
        });
        match self.shared.mailbox.take_timeout(FIRST_FRAME_TIMEOUT) {
            Taken::Item(f) => {
                self.first = Some(f);
                Ok(())
            }
            Taken::Empty => {
                self.stop();
                Err(CaptureError::Display("no picture arrived".into()))
            }
            Taken::Closed => {
                let err =
                    self.shared.phase().start_error().unwrap_or_else(|| {
                        CaptureError::Display("the capture ended at once".into())
                    });
                self.stop();
                Err(err)
            }
        }
    }

    fn next_frame(&mut self) -> Result<Capture, CaptureError> {
        if self.running.is_none() {
            return Err(CaptureError::NotStarted);
        }
        let wait = self.pacer.wait(Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        self.pacer.looked(Instant::now());
        let item = match self.first.take() {
            Some(f) => Taken::Item(f),
            None => self.shared.mailbox.take_timeout(Duration::ZERO),
        };
        match item {
            Taken::Item(f) => {
                if self.detector.update(f.width, f.height, &f.bgra) {
                    Ok(Capture::Frame(f))
                } else {
                    Ok(Capture::Unchanged)
                }
            }
            Taken::Empty => Ok(Capture::Unchanged),
            Taken::Closed => match self.shared.phase() {
                Phase::Failed(e) => Err(CaptureError::Display(e)),
                _ => Ok(Capture::Ended),
            },
        }
    }

    fn stop(&mut self) {
        self.shared.event(Event::StopRequested);
        if let Some(r) = self.running.take() {
            let _ = r.pool.RemoveFrameArrived(r.frame_token);
            let _ = r.item.RemoveClosed(r.closed_token);
            let _ = r.session.Close();
            let _ = r.pool.Close();
        }
        self.first = None;
    }
}

impl Drop for WgcSource {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `FrameArrived`: take the newest frame, follow size changes, copy it to the mailbox.
fn on_frame(
    pool: &Direct3D11CaptureFramePool,
    gpu: &Arc<Mutex<ThreadSafe<Gpu>>>,
    shared: &Arc<Shared>,
) {
    let Ok(frame) = pool.TryGetNextFrame() else {
        return;
    };
    let result = (|| -> Result<crate::RawFrame, CaptureError> {
        let content = frame.ContentSize().map_err(werr("content size"))?;
        let content = (content.Width.max(0) as u32, content.Height.max(0) as u32);
        let surface = frame.Surface().map_err(werr("frame surface"))?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(werr("surface cast"))?;
        // SAFETY: the surface of a capture frame is a Direct3D 11 texture.
        let texture: ID3D11Texture2D =
            unsafe { access.GetInterface() }.map_err(werr("frame texture"))?;
        let mut g = gpu.lock().unwrap_or_else(|p| p.into_inner());
        let g = &mut g.0;
        let picture = g.read(&texture, content)?;
        // Window resized: rebuild the pool for the new size (the next frame has it).
        if let Some((nw, nh)) = g.size.observe(content.0, content.1) {
            let _ = pool.Recreate(
                &g.winrt_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                POOL_BUFFERS,
                SizeInt32 {
                    Width: nw as i32,
                    Height: nh as i32,
                },
            );
        }
        Ok(picture)
    })();
    match result {
        Ok(f) => shared.push_frame(f),
        Err(e) => shared.event(Event::Error(e.to_string())),
    }
}
