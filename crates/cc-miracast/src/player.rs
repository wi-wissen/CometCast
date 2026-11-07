// SPDX-License-Identifier: GPL-3.0-or-later
//! Plays the incoming `MediaSource` and turns its pictures into BGRA frames for the UI.
//!
//! Design (see docs/miracast.md, "Video path"): a `Windows.Media.Playback.MediaPlayer` in
//! FRAME SERVER mode (`IsVideoFrameServerEnabled`). The player decodes and plays the audio
//! itself; for each picture it raises `VideoFrameAvailable`, and we copy that picture with
//! `CopyFrameToVideoSurface` into a Direct3D 11 texture, read it back as a `SoftwareBitmap`
//! (BGRA), scale it down to the size the UI asked for and put it into the [`FrameMailbox`].
//!
//! COMPILE-CHECKED ONLY. Never run: every statement about what Windows does at run time is
//! UNCERTAIN until `docs/miracast-device-test-plan.md` part E has been done. Speed is
//! unmeasured: the path copies each picture GPU -> CPU -> scaled CPU buffer -> Dart, which may
//! be too slow at 1080p60 on a weak mini PC (measurement point 5; fallback: lower the size
//! with `set_max_size`, or a GPU shared texture in a native Flutter plugin, not built).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::DirectX::Direct3D11::IDirect3DSurface;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Core::MediaSource;
use windows::Media::Playback::{MediaPlayer, MediaPlayerFailedEventArgs};
use windows::Storage::Streams::{Buffer, DataReader, IBuffer};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice, ID3D11Device,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGISurface};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11SurfaceFromDXGISurface;
use windows::core::{IInspectable, Interface};

use crate::frame::{
    DEFAULT_MAX_HEIGHT, DEFAULT_MAX_WIDTH, FrameMailbox, HARD_MAX_DIMENSION, fit_size,
    scale_bgra_nearest,
};

/// At most one picture per this long is copied (about 60 per second). Pictures that arrive
/// faster are skipped, so a 120 Hz source cannot make the copy path run flat out.
// UNCERTAIN: placeholder, not tuned.
const MIN_FRAME_INTERVAL: Duration = Duration::from_millis(15);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A Direct3D object that is moved between the threads of the Windows thread pool.
struct ThreadSafe<T>(T);
// SAFETY: only `ID3D11Device`, a texture created by it, and the WinRT surface wrapped around
// that texture are put in here. A Direct3D 11 device is free-threaded unless it was created
// with `D3D11_CREATE_DEVICE_SINGLETHREADED`, which we do not do, and its immediate context is
// never used. Access to the value is serialised by a `Mutex` anyway.
#[allow(unsafe_code)]
unsafe impl<T> Send for ThreadSafe<T> {}

/// Texture and WinRT surface for a given picture size.
struct Gpu {
    width: u32,
    height: u32,
    // Kept alive: the surface refers to the texture, the texture to the device.
    _device: ID3D11Device,
    _texture: ID3D11Texture2D,
    surface: IDirect3DSurface,
}

#[allow(unsafe_code)]
fn create_device() -> windows::core::Result<ID3D11Device> {
    let make = |kind: D3D_DRIVER_TYPE| -> windows::core::Result<ID3D11Device> {
        let mut device: Option<ID3D11Device> = None;
        // SAFETY: all pointers refer to locals that live for the call; `device` receives a
        // new reference on success. BGRA support is required for B8G8R8A8 textures.
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
                None,
            )?;
        }
        device.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))
    };
    // UNCERTAIN: the WARP (software) fallback is for machines without a usable GPU driver.
    make(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| make(D3D_DRIVER_TYPE_WARP))
}

#[allow(unsafe_code)]
fn create_gpu(width: u32, height: u32) -> windows::core::Result<Gpu> {
    let device = create_device()?;
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    // SAFETY: `desc` and `texture` are locals that outlive the call; no initial data.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture))? };
    let texture =
        texture.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))?;
    let dxgi: IDXGISurface = texture.cast()?;
    // SAFETY: `dxgi` is a valid DXGI surface of the device created above.
    let inspectable: IInspectable = unsafe { CreateDirect3D11SurfaceFromDXGISurface(&dxgi)? };
    let surface: IDirect3DSurface = inspectable.cast()?;
    Ok(Gpu {
        width,
        height,
        _device: device,
        _texture: texture,
        surface,
    })
}

/// State shared between the player and its event handlers.
struct PlayerShared {
    mailbox: Arc<FrameMailbox>,
    gpu: Mutex<Option<ThreadSafe<Gpu>>>,
    /// A picture is being copied; an overlapping `VideoFrameAvailable` is skipped.
    busy: AtomicBool,
    closed: AtomicBool,
    max_w: AtomicU32,
    max_h: AtomicU32,
    last_copy: Mutex<Option<Instant>>,
}

impl PlayerShared {
    fn on_frame(&self, player: &MediaPlayer) -> windows::core::Result<()> {
        if self.closed.load(Ordering::SeqCst) || self.busy.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let result = self.copy_frame(player);
        self.busy.store(false, Ordering::SeqCst);
        if result.is_err() {
            self.mailbox.note_failure();
        }
        result
    }

    fn copy_frame(&self, player: &MediaPlayer) -> windows::core::Result<()> {
        {
            let mut last = lock(&self.last_copy);
            let now = Instant::now();
            if let Some(t) = *last
                && now.saturating_duration_since(t) < MIN_FRAME_INTERVAL
            {
                return Ok(());
            }
            *last = Some(now);
        }
        let session = player.PlaybackSession()?;
        let (w, h) = (session.NaturalVideoWidth()?, session.NaturalVideoHeight()?);
        if w == 0 || h == 0 || w > HARD_MAX_DIMENSION * 2 || h > HARD_MAX_DIMENSION * 2 {
            // Size not known yet (first picture) or absurd.
            return Ok(());
        }
        let surface = {
            let mut gpu = lock(&self.gpu);
            let stale = gpu
                .as_ref()
                .is_none_or(|g| g.0.width != w || g.0.height != h);
            if stale {
                *gpu = Some(ThreadSafe(create_gpu(w, h)?));
            }
            // `gpu` was just filled if it was stale.
            match gpu.as_ref() {
                Some(g) => g.0.surface.clone(),
                None => return Ok(()),
            }
        };
        // UNCERTAIN: CopyFrameToVideoSurface on an unpackaged desktop process with a stream
        // from MiracastReceiver (protected content, e.g. HDCP, may give a black picture).
        player.CopyFrameToVideoSurface(&surface)?;
        // The documented way to read a Direct3D surface on the CPU without Win2D.
        let mut bitmap = SoftwareBitmap::CreateCopyFromSurfaceAsync(&surface)?.join()?;
        if bitmap.BitmapPixelFormat()? != BitmapPixelFormat::Bgra8 {
            bitmap = SoftwareBitmap::Convert(&bitmap, BitmapPixelFormat::Bgra8)?;
        }
        let (bw, bh) = (bitmap.PixelWidth()?, bitmap.PixelHeight()?);
        let (bw, bh) = (
            u32::try_from(bw).unwrap_or(0),
            u32::try_from(bh).unwrap_or(0),
        );
        let len = (bw as usize) * (bh as usize) * 4;
        let buffer =
            Buffer::Create(u32::try_from(len).map_err(|_| {
                windows::core::Error::from(windows::Win32::Foundation::E_OUTOFMEMORY)
            })?)?;
        let ibuffer: IBuffer = buffer.cast()?;
        bitmap.CopyToBuffer(&ibuffer)?;
        let mut bytes = vec![0u8; buffer.Length()? as usize];
        DataReader::FromBuffer(&ibuffer)?.ReadBytes(&mut bytes)?;
        // UNCERTAIN: rows are assumed to have no padding (stride == width * 4); the copy
        // length check in `scale_bgra_nearest` refuses a picture where that is wrong.
        let (dw, dh) = fit_size(
            bw,
            bh,
            self.max_w.load(Ordering::Relaxed),
            self.max_h.load(Ordering::Relaxed),
        );
        match scale_bgra_nearest(&bytes, bw, bh, dw, dh) {
            Some(out) => {
                self.mailbox.publish(dw, dh, out);
            }
            None => self.mailbox.note_failure(),
        }
        Ok(())
    }
}

/// The MediaPlayer and its handlers. Dropping it stops playback.
pub(crate) struct Player {
    player: MediaPlayer,
    shared: Arc<PlayerShared>,
    frame_token: Option<i64>,
    failed_token: Option<i64>,
}

impl Player {
    /// Create the player (no source yet). `on_failed` is called with the error text when
    /// playback of the stream fails.
    pub(crate) fn new(
        mailbox: Arc<FrameMailbox>,
        on_failed: impl Fn(String) + Send + Sync + 'static,
    ) -> windows::core::Result<Self> {
        let player = MediaPlayer::new()?;
        // The pictures are taken by us, not drawn by the player.
        player.SetIsVideoFrameServerEnabled(true)?;
        // UNCERTAIN: asks for low latency over smooth playback; effect not measured.
        let _ = player.SetRealTimePlayback(true);
        let shared = Arc::new(PlayerShared {
            mailbox,
            gpu: Mutex::new(None),
            busy: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            max_w: AtomicU32::new(DEFAULT_MAX_WIDTH),
            max_h: AtomicU32::new(DEFAULT_MAX_HEIGHT),
            last_copy: Mutex::new(None),
        });
        let s = Arc::clone(&shared);
        let frame_token = player.VideoFrameAvailable(&TypedEventHandler::<
            MediaPlayer,
            IInspectable,
        >::new(move |sender, _| {
            if let Some(p) = sender.as_ref() {
                // Errors are counted in the mailbox; the handler itself never fails.
                let _ = s.on_frame(p);
            }
            Ok(())
        }))?;
        let failed_token = player.MediaFailed(&TypedEventHandler::<
            MediaPlayer,
            MediaPlayerFailedEventArgs,
        >::new(move |_, args| {
            let text = args
                .as_ref()
                .and_then(|a| a.ErrorMessage().ok())
                .map(|h| h.to_string_lossy())
                .unwrap_or_default();
            on_failed(text);
            Ok(())
        }))?;
        Ok(Self {
            player,
            shared,
            frame_token: Some(frame_token),
            failed_token: Some(failed_token),
        })
    }

    /// Start playing the stream. Called INSIDE the `MediaSourceCreated` handler (under a
    /// `Deferral`), so the source is attached before Windows continues.
    pub(crate) fn play(&self, source: &MediaSource) -> windows::core::Result<()> {
        self.shared.mailbox.reset();
        self.player.SetSource(source)?;
        self.player.Play()
    }

    /// Stop playing and forget the last picture.
    pub(crate) fn stop(&self) {
        let _ = self.player.Pause();
        let _ = self
            .player
            .SetSource(None::<&windows::Media::Playback::IMediaPlaybackSource>);
        self.shared.mailbox.clear();
        *lock(&self.shared.gpu) = None;
    }

    /// Largest picture size handed to the UI (clamped by [`fit_size`]).
    pub(crate) fn set_max_size(&self, w: u32, h: u32) {
        self.shared
            .max_w
            .store(w.clamp(1, HARD_MAX_DIMENSION), Ordering::Relaxed);
        self.shared
            .max_h
            .store(h.clamp(1, HARD_MAX_DIMENSION), Ordering::Relaxed);
    }

    /// Mute or unmute the audio of the stream.
    pub(crate) fn set_muted(&self, muted: bool) {
        let _ = self.player.SetIsMuted(muted);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        if let Some(t) = self.frame_token.take() {
            let _ = self.player.RemoveVideoFrameAvailable(t);
        }
        if let Some(t) = self.failed_token.take() {
            let _ = self.player.RemoveMediaFailed(t);
        }
        self.stop();
        let _ = self.player.Close();
    }
}
