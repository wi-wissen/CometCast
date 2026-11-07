// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS capture with ScreenCaptureKit and the system's `SCContentSharingPicker` (macOS 14+),
//! through the `objc2` crates (MIT / Apache-2.0 / Zlib).
//!
//! Flow of `start`: check the system version; on the main queue activate the shared picker,
//! register an observer and present it (the system dialog; windows and displays); the observer
//! reports the chosen `SCContentFilter`, a cancel, or a failure; cancel is
//! [`CaptureError::Denied`]; then an `SCStream` is built from the filter (BGRA, the requested
//! frame interval, cursor shown), started, and every `SCFrameStatus::Complete` sample buffer is
//! copied out of its `CVPixelBuffer` ([`crate::convert::to_bgra`]) into the shared mailbox.
//! `next_frame` takes the newest picture at the requested rate.
//!
//! The decisions that do not need a Mac are in [`crate::sck_logic`] and tested everywhere.
//! **Nothing in this file was run**: it is type-checked for `aarch64-apple-darwin` (the objc2
//! crates need no SDK for that) and docs/macos-capture-test-plan.md lists what to check on a
//! Mac.
//!
//! Permissions (UNCERTAIN, not tried): with the system picker the person's choice is the
//! consent for that content, and Screen Recording (TCC) is not asked up front; a refusal comes
//! as the picker's cancel or as `SCStreamErrorUserDeclined` from the stream start. The picker
//! needs the app's main run loop (it is presented on the main queue), so this backend works
//! inside a Flutter app and not in a plain command line program.

// ObjC classes, blocks and the C functions of CoreVideo are `unsafe` in the bindings. Every
// `unsafe` block has a SAFETY comment; the crate denies `unsafe_code` everywhere else.
#![allow(unsafe_code)]

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, Message, define_class, msg_send};
use objc2_core_foundation::{CFArray, CFDictionary};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSError, NSNumber, NSObject, NSObjectProtocol, NSProcessInfo};
use objc2_screen_capture_kit::{
    SCContentFilter, SCContentSharingPicker, SCContentSharingPickerConfiguration,
    SCContentSharingPickerMode, SCContentSharingPickerObserver, SCFrameStatus, SCStream,
    SCStreamConfiguration, SCStreamDelegate, SCStreamFrameInfoStatus, SCStreamOutput,
    SCStreamOutputType,
};

use crate::convert::{PixelFormat, to_bgra};
use crate::sck_logic::{
    FrameAction, capture_size, frame_action, frame_interval, map_start_error, picker_supported,
    stop_event,
};
use crate::session::{Event, Pacer, Phase, Shared, Taken};
use crate::{
    Capture, CaptureError, CaptureSource, ChangeDetector, PICKER_SOURCE_ID, PickerOptions,
    RawFrame, ScreenSource,
};

/// How long `start` waits for the first picture after the person picked something.
/// UNCERTAIN: placeholder.
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the picker may stay open before `start` gives up. UNCERTAIN: placeholder; the
/// person may leave the dialog open, and nothing else can use the sender meanwhile.
const PICKER_TIMEOUT: Duration = Duration::from_secs(600);

/// Lets an Objective-C object cross threads where the code makes sure it is only used in a
/// way the framework allows (retain, release, pass to an API that is documented thread safe).
struct SendBox<T>(T);
// SAFETY: see the uses: the boxed objects are only retained/released on other threads or
// handed to ScreenCaptureKit calls that are documented as thread safe; main-queue-only calls
// happen inside `DispatchQueue::main().exec_async`.
unsafe impl<T> Send for SendBox<T> {}

/// What the picker observer reports.
enum PickResult {
    Picked(SendBox<Retained<SCContentFilter>>),
    Cancelled,
    Failed(String),
}

struct ObserverIvars {
    tx: Mutex<mpsc::Sender<PickResult>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class has no `Drop`.
    #[unsafe(super(NSObject))]
    #[ivars = ObserverIvars]
    struct PickerObserver;

    unsafe impl NSObjectProtocol for PickerObserver {}

    unsafe impl SCContentSharingPickerObserver for PickerObserver {
        #[unsafe(method(contentSharingPicker:didCancelForStream:))]
        fn did_cancel(&self, _picker: &SCContentSharingPicker, _stream: Option<&SCStream>) {
            let _ = self
                .ivars()
                .tx
                .lock()
                .map(|t| t.send(PickResult::Cancelled));
        }

        #[unsafe(method(contentSharingPicker:didUpdateWithFilter:forStream:))]
        fn did_update(
            &self,
            _picker: &SCContentSharingPicker,
            filter: &SCContentFilter,
            _stream: Option<&SCStream>,
        ) {
            let f = SendBox(filter.retain());
            let _ = self
                .ivars()
                .tx
                .lock()
                .map(|t| t.send(PickResult::Picked(f)));
        }

        #[unsafe(method(contentSharingPickerStartDidFailWithError:))]
        fn start_failed(&self, error: &NSError) {
            let msg = error.localizedDescription().to_string();
            let _ = self
                .ivars()
                .tx
                .lock()
                .map(|t| t.send(PickResult::Failed(msg)));
        }
    }
);

struct StreamIvars {
    shared: Arc<Shared>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class has no `Drop`.
    #[unsafe(super(NSObject))]
    #[ivars = StreamIvars]
    struct StreamHandler;

    unsafe impl NSObjectProtocol for StreamHandler {}

    unsafe impl SCStreamOutput for StreamHandler {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(
            &self,
            _stream: &SCStream,
            sample: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind != SCStreamOutputType::Screen {
                return;
            }
            handle_sample(&self.ivars().shared, sample);
        }
    }

    unsafe impl SCStreamDelegate for StreamHandler {
        #[unsafe(method(stream:didStopWithError:))]
        fn did_stop(&self, _stream: &SCStream, error: &NSError) {
            self.ivars().shared.event(stop_event(error.code() as i64));
        }
    }
);

/// The `SCFrameStatus` of a sample buffer, from its attachments.
fn sample_status(sample: &CMSampleBuffer) -> Option<i64> {
    // SAFETY: plain CoreMedia getter; the array is retained by the returned wrapper.
    let attachments: objc2_core_foundation::CFRetained<CFArray> =
        unsafe { sample.sample_attachments_array(false) }?;
    if attachments.count() < 1 {
        return None;
    }
    // SAFETY: index 0 exists (checked); the elements of this array are CFDictionary.
    let dict = unsafe { &*(attachments.value_at_index(0) as *const CFDictionary) };
    // SAFETY: the key is a valid CFString; the value, when present, is an NSNumber.
    let value = unsafe {
        dict.value(std::ptr::from_ref::<objc2_foundation::NSString>(SCStreamFrameInfoStatus).cast())
    };
    if value.is_null() {
        return None;
    }
    // SAFETY: `SCStreamFrameInfoStatus` maps to an NSNumber (documented).
    let n = unsafe { &*(value as *const NSNumber) };
    Some(n.integerValue() as i64)
}

/// One sample buffer from ScreenCaptureKit.
fn handle_sample(shared: &Shared, sample: &CMSampleBuffer) {
    // A buffer without status is treated as a complete frame (older systems).
    let status = sample_status(sample).unwrap_or(SCFrameStatus::Complete.0 as i64);
    match frame_action(status) {
        FrameAction::Use => {}
        FrameAction::Unchanged | FrameAction::Started => return,
        FrameAction::Pause => {
            shared.event(Event::StreamPaused);
            return;
        }
        FrameAction::End => {
            shared.event(Event::SourceClosed);
            return;
        }
    }
    // SAFETY: plain CoreMedia getter.
    let Some(image) = (unsafe { sample.image_buffer() }) else {
        return;
    };
    let flags = CVPixelBufferLockFlags::ReadOnly;
    // SAFETY: `image` is a pixel buffer for as long as `image` lives; locked read-only here
    // and unlocked below, after the copy.
    if unsafe { CVPixelBufferLockBaseAddress(&image, flags) } != 0 {
        return;
    }
    let width = CVPixelBufferGetWidth(&image);
    let height = CVPixelBufferGetHeight(&image);
    let stride = CVPixelBufferGetBytesPerRow(&image);
    let base = CVPixelBufferGetBaseAddress(&image);
    let result = if base.is_null() {
        None
    } else {
        // SAFETY: while locked, `base` points to `stride * height` readable bytes.
        let bytes = unsafe { std::slice::from_raw_parts(base as *const u8, stride * height) };
        Some(to_bgra(
            PixelFormat::Bgra,
            bytes,
            width as u32,
            height as u32,
            stride,
            None,
        ))
    };
    // SAFETY: locked above with the same flags.
    unsafe { CVPixelBufferUnlockBaseAddress(&image, flags) };
    match result {
        Some(Ok(frame)) => {
            shared.event(Event::StreamResumed);
            shared.push_frame(frame);
        }
        Some(Err(e)) => shared.event(Event::Error(e.to_string())),
        None => {}
    }
}

struct Running {
    stream: SendBox<Retained<SCStream>>,
    // Keeps the handler (output and delegate) alive as long as the stream runs.
    _handler: SendBox<Retained<StreamHandler>>,
    _queue: DispatchRetained<DispatchQueue>,
}

/// macOS screen capture with the system picker.
pub struct SckSource {
    opts: PickerOptions,
    shared: Arc<Shared>,
    running: Option<Running>,
    pacer: Pacer,
    detector: ChangeDetector,
    first: Option<RawFrame>,
}

impl std::fmt::Debug for SckSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SckSource").finish_non_exhaustive()
    }
}

impl SckSource {
    /// Check the macOS version (14 or newer has the picker) and prepare.
    pub fn new(opts: &PickerOptions) -> Result<Self, CaptureError> {
        let major = NSProcessInfo::processInfo()
            .operatingSystemVersion()
            .majorVersion as i64;
        if !picker_supported(major) {
            return Err(CaptureError::Unsupported(
                "the system screen picker needs macOS 14 or newer",
            ));
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

    /// Show the system picker (on the main queue) and wait for the choice.
    fn pick_filter(&self) -> Result<Retained<SCContentFilter>, CaptureError> {
        if MainThreadMarker::new().is_some() {
            return Err(CaptureError::Display(
                "the picker must be started from a worker thread, not the main thread".into(),
            ));
        }
        let (tx, rx) = mpsc::channel::<PickResult>();
        let tx = SendBox(tx);
        // The observer lives until the picker is closed; it is created and released on the
        // main queue.
        DispatchQueue::main().exec_async(move || {
            let tx = tx;
            let observer: Retained<PickerObserver> = {
                let this = PickerObserver::alloc().set_ivars(ObserverIvars {
                    tx: Mutex::new(tx.0),
                });
                // SAFETY: `init` of NSObject on a freshly allocated instance.
                unsafe { msg_send![super(this), init] }
            };
            // SAFETY: main queue; the shared picker is a singleton.
            unsafe {
                let picker = SCContentSharingPicker::sharedPicker();
                let config = SCContentSharingPickerConfiguration::new();
                config.setAllowedPickerModes(
                    SCContentSharingPickerMode::SingleWindow
                        | SCContentSharingPickerMode::SingleDisplay,
                );
                picker.setDefaultConfiguration(&config);
                picker.setActive(true);
                picker.addObserver(ProtocolObject::from_ref(&*observer));
                picker.present();
            }
            // Keep the observer alive for the process life of this pick; it is removed in
            // `finish_picker`.
            OBSERVER
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .replace(SendBox(observer));
        });
        let result = rx.recv_timeout(PICKER_TIMEOUT);
        finish_picker();
        match result {
            Ok(PickResult::Picked(f)) => Ok(f.0),
            Ok(PickResult::Cancelled) => Err(CaptureError::Denied),
            Ok(PickResult::Failed(m)) => Err(CaptureError::Display(format!("picker: {m}"))),
            Err(RecvTimeoutError::Timeout) => Err(CaptureError::Denied),
            Err(RecvTimeoutError::Disconnected) => {
                Err(CaptureError::Display("the picker went away".into()))
            }
        }
    }
}

static OBSERVER: Mutex<Option<SendBox<Retained<PickerObserver>>>> = Mutex::new(None);

/// Deactivate the picker and drop its observer (main queue).
fn finish_picker() {
    DispatchQueue::main().exec_async(|| {
        let observer = OBSERVER.lock().unwrap_or_else(|p| p.into_inner()).take();
        // SAFETY: main queue; the shared picker is a singleton.
        unsafe {
            let picker = SCContentSharingPicker::sharedPicker();
            if let Some(o) = &observer {
                picker.removeObserver(ProtocolObject::from_ref(&*o.0));
            }
            picker.setActive(false);
        }
    });
}

impl ScreenSource for SckSource {
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
        let filter = match self.pick_filter() {
            Ok(f) => f,
            Err(e) => {
                self.shared.event(match &e {
                    CaptureError::Denied => Event::PickerCancelled,
                    other => Event::Error(other.to_string()),
                });
                return Err(e);
            }
        };
        self.shared.event(Event::Picked);

        // SAFETY: plain getters of the filter (macOS 14).
        let (rect, scale) = unsafe { (filter.contentRect(), filter.pointPixelScale()) };
        let (w, h) = capture_size(rect.size.width, rect.size.height, scale);
        let (iv, its) = frame_interval(self.opts.fps);
        // SAFETY: the configuration is a new object configured before it is used.
        let config = unsafe {
            let c = SCStreamConfiguration::new();
            c.setWidth(w);
            c.setHeight(h);
            c.setPixelFormat(kCVPixelFormatType_32BGRA);
            c.setMinimumFrameInterval(CMTime::new(iv, its));
            c.setShowsCursor(true);
            c.setQueueDepth(3);
            c
        };
        let handler: Retained<StreamHandler> = {
            let this = StreamHandler::alloc().set_ivars(StreamIvars {
                shared: Arc::clone(&self.shared),
            });
            // SAFETY: `init` of NSObject on a freshly allocated instance.
            unsafe { msg_send![super(this), init] }
        };
        // SAFETY: filter and configuration are valid; the delegate is kept alive in `Running`.
        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &config,
                Some(ProtocolObject::from_ref(&*handler)),
            )
        };
        let queue = DispatchQueue::new("cometcast.screen-capture", None);
        // SAFETY: the output is kept alive in `Running`; the queue is serial.
        unsafe {
            stream
                .addStreamOutput_type_sampleHandlerQueue_error(
                    ProtocolObject::from_ref(&*handler),
                    SCStreamOutputType::Screen,
                    Some(&queue),
                )
                .map_err(|e| map_start_error(e.code() as i64, "cannot add the stream output"))?;
        }
        let (tx, rx) = mpsc::channel::<Option<(i64, String)>>();
        let block = RcBlock::new(move |err: *mut NSError| {
            // SAFETY: when non-null, `err` is a valid NSError for the duration of the call.
            let info = unsafe { err.as_ref() }
                .map(|e| (e.code() as i64, e.localizedDescription().to_string()));
            let _ = tx.send(info);
        });
        // SAFETY: the block is copied by the framework; the stream is valid.
        unsafe { stream.startCaptureWithCompletionHandler(Some(&block)) };
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(None) => {}
            Ok(Some((code, msg))) => {
                let e = map_start_error(code, &msg);
                self.shared.event(match &e {
                    CaptureError::Denied => Event::PickerCancelled,
                    other => Event::Error(other.to_string()),
                });
                return Err(e);
            }
            Err(_) => {
                return Err(CaptureError::Display(
                    "ScreenCaptureKit did not answer the start request".into(),
                ));
            }
        }
        self.running = Some(Running {
            stream: SendBox(stream),
            _handler: SendBox(handler),
            _queue: queue,
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
            let block = RcBlock::new(|_err: *mut NSError| {});
            // SAFETY: the stream is valid; the completion block is copied by the framework.
            unsafe { r.stream.0.stopCaptureWithCompletionHandler(Some(&block)) };
        }
        self.first = None;
    }
}

impl Drop for SckSource {
    fn drop(&mut self) {
        self.stop();
    }
}
