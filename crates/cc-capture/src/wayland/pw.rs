// SPDX-License-Identifier: GPL-3.0-or-later
//! The PipeWire side: one thread runs the PipeWire main loop, connects a stream to the node the
//! portal gave us (through the file descriptor of the portal's PipeWire remote), negotiates a
//! raw video format and hands every buffer to [`super::ingest`].
//!
//! This file is the thin glue to the `pipewire` crate (MIT, safe API). All decisions are in
//! `negotiate.rs` and `mod.rs`, which are tested without PipeWire.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::thread::JoinHandle;

use pipewire as pw;
use pw::spa;
use spa::param::video::{VideoFormat, VideoInfoRaw};
use spa::pod::{ChoiceValue, Object, Pod, Property, Value, serialize::PodSerializer};
use spa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Id, Rectangle, SpaTypes};

use super::negotiate::{self, OFFERED_FORMATS};
use super::{Shared, StreamFormat};
use crate::CaptureError;
use crate::convert::{MAX_SIDE, PixelFormat};
use crate::session::Event;

/// Handle of the PipeWire thread.
pub(super) struct PwThread {
    quit: pw::channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl PwThread {
    /// Start the thread. Failures that happen inside it are reported to `shared` as
    /// `Event::Error`.
    pub(super) fn spawn(
        fd: OwnedFd,
        node_id: u32,
        shared: Arc<Shared>,
    ) -> Result<Self, CaptureError> {
        let (quit, quit_rx) = pw::channel::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("cometcast-pipewire".into())
            .spawn(move || {
                if let Err(e) = run(fd, node_id, &shared, quit_rx) {
                    shared.event(Event::Error(format!("PipeWire: {e}")));
                }
                // If the loop ended on its own, the stream is over.
                shared.event(Event::SystemStopped);
                shared.mailbox.close();
            })
            .map_err(|e| CaptureError::Display(format!("cannot start the PipeWire thread: {e}")))?;
        Ok(Self {
            quit,
            thread: Some(thread),
        })
    }

    /// Ask the loop to end and wait for the thread.
    pub(super) fn stop(&mut self) {
        let _ = self.quit.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for PwThread {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spa_format(f: PixelFormat) -> VideoFormat {
    match f {
        PixelFormat::Bgra => VideoFormat::BGRA,
        PixelFormat::Bgrx => VideoFormat::BGRx,
        PixelFormat::Rgba => VideoFormat::RGBA,
        PixelFormat::Rgbx => VideoFormat::RGBx,
        PixelFormat::Argb => VideoFormat::ARGB,
        PixelFormat::Xrgb => VideoFormat::xRGB,
        PixelFormat::Abgr => VideoFormat::ABGR,
        PixelFormat::Xbgr => VideoFormat::xBGR,
        PixelFormat::Rgb => VideoFormat::RGB,
        PixelFormat::Bgr => VideoFormat::BGR,
    }
}

fn pixel_format(f: VideoFormat) -> Option<PixelFormat> {
    OFFERED_FORMATS.into_iter().find(|p| spa_format(*p) == f)
}

/// The `EnumFormat` parameter: raw video, the offered pixel formats, any size, any rate.
fn enum_format_pod() -> Vec<u8> {
    use spa::param::ParamType;
    use spa::param::format::{FormatProperties as P, MediaSubtype, MediaType};
    let formats: Vec<Id> = OFFERED_FORMATS
        .iter()
        .map(|f| Id(spa_format(*f).as_raw()))
        .collect();
    let obj = Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: vec![
            Property::new(
                P::MediaType.as_raw(),
                Value::Id(Id(MediaType::Video.as_raw())),
            ),
            Property::new(
                P::MediaSubtype.as_raw(),
                Value::Id(Id(MediaSubtype::Raw.as_raw())),
            ),
            Property::new(
                P::VideoFormat.as_raw(),
                Value::Choice(ChoiceValue::Id(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Enum {
                        default: formats[0],
                        alternatives: formats.clone(),
                    },
                ))),
            ),
            Property::new(
                P::VideoSize.as_raw(),
                Value::Choice(ChoiceValue::Rectangle(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: Rectangle {
                            width: 1920,
                            height: 1080,
                        },
                        min: Rectangle {
                            width: 1,
                            height: 1,
                        },
                        max: Rectangle {
                            width: MAX_SIDE,
                            height: MAX_SIDE,
                        },
                    },
                ))),
            ),
            Property::new(
                P::VideoFramerate.as_raw(),
                Value::Choice(ChoiceValue::Fraction(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: Fraction { num: 60, denom: 1 },
                        min: Fraction { num: 0, denom: 1 },
                        max: Fraction {
                            num: 1000,
                            denom: 1,
                        },
                    },
                ))),
            ),
        ],
    };
    serialize(obj)
}

/// The `Buffers` parameter: a few buffers of one block each, and only data types this code
/// can read. The size is left to the producer (it knows the picture size).
fn buffers_pod() -> Vec<u8> {
    let mask = negotiate::buffer_type_mask(false) as i32;
    let obj = Object {
        type_: SpaTypes::ObjectParamBuffers.as_raw(),
        id: spa::param::ParamType::Buffers.as_raw(),
        properties: vec![
            Property::new(
                spa::sys::SPA_PARAM_BUFFERS_buffers,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: 8,
                        min: 2,
                        max: 16,
                    },
                ))),
            ),
            Property::new(spa::sys::SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
            // A flags choice, not a plain Int: PipeWire intersects flags with the producer's
            // mask, while two plain values must be equal (found with the loopback test).
            Property::new(
                spa::sys::SPA_PARAM_BUFFERS_dataType,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Flags {
                        default: mask,
                        flags: vec![mask],
                    },
                ))),
            ),
        ],
    };
    serialize(obj)
}

fn serialize(obj: Object) -> Vec<u8> {
    PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
        .map(|(c, _)| c.into_inner())
        .unwrap_or_default()
}

struct Ud {
    shared: Arc<Shared>,
    info: VideoInfoRaw,
    fmt: Option<StreamFormat>,
    had_frames: bool,
}

fn run(
    fd: OwnedFd,
    node_id: u32,
    shared: &Arc<Shared>,
    quit_rx: pw::channel::Receiver<()>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;

    // The node of the shared window or monitor disappearing is the end of the share: the
    // stream itself only goes to "paused" then, which looks like a minimised window.
    let registry = core.get_registry_rc()?;
    let gone = Arc::clone(shared);
    let ml_gone = mainloop.clone();
    let _registry_listener = registry
        .add_listener_local()
        .global_remove(move |id| {
            if id == node_id {
                gone.event(Event::SourceClosed);
                ml_gone.quit();
            }
        })
        .register();

    let ml = mainloop.clone();
    let _quit = quit_rx.attach(mainloop.loop_(), move |()| ml.quit());

    let stream = pw::stream::StreamBox::new(
        &core,
        "cometcast-screen",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let ud = Ud {
        shared: Arc::clone(shared),
        info: VideoInfoRaw::default(),
        fmt: None,
        had_frames: false,
    };
    let ml_end = mainloop.clone();
    let _listener = stream
        .add_local_listener_with_user_data(ud)
        .state_changed(move |_, ud, _old, new| {
            use pw::stream::StreamState as S;
            match new {
                S::Paused if ud.had_frames => ud.shared.event(Event::StreamPaused),
                S::Streaming if ud.had_frames => ud.shared.event(Event::StreamResumed),
                S::Unconnected if ud.fmt.is_some() => {
                    // The node went away (window closed, monitor removed).
                    ud.shared.event(Event::SourceClosed);
                    ml_end.quit();
                }
                S::Error(e) => {
                    // After streaming started an error means the source is gone.
                    let ev = if ud.had_frames {
                        Event::SourceClosed
                    } else {
                        Event::Error(format!("stream error: {e}"))
                    };
                    ud.shared.event(ev);
                    ml_end.quit();
                }
                _ => {}
            }
        })
        .param_changed(|stream, ud, id, param| {
            let Some(param) = param else { return };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Ok((mt, st)) = spa::param::format_utils::parse_format(param) else {
                return;
            };
            if mt != spa::param::format::MediaType::Video
                || st != spa::param::format::MediaSubtype::Raw
            {
                return;
            }
            if ud.info.parse(param).is_err() {
                ud.shared
                    .event(Event::Error("cannot parse the video format".into()));
                return;
            }
            let Some(format) = pixel_format(ud.info.format()) else {
                ud.shared.event(Event::Error(format!(
                    "unsupported pixel format {:?}",
                    ud.info.format()
                )));
                return;
            };
            let size = ud.info.size();
            ud.fmt = Some(StreamFormat {
                format,
                width: size.width,
                height: size.height,
                crop: None,
            });
            ud.shared.set_size(size.width, size.height);
            let bytes = buffers_pod();
            if let Some(pod) = Pod::from_bytes(&bytes) {
                let _ = stream.update_params(&mut [pod]);
            }
        })
        .process(|stream, ud| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let Some(fmt) = ud.fmt else { return };
            let datas = buffer.datas_mut();
            let Some(d) = datas.first_mut() else { return };
            let data_type = d.type_().as_raw();
            let (offset, size, stride, flags) = {
                let c = d.chunk();
                (c.offset(), c.size(), c.stride(), c.flags().bits())
            };
            let mapped = d.data().map(|s| &*s);
            super::ingest(
                &ud.shared, &fmt, data_type, mapped, offset, size, stride, flags,
            );
            if matches!(
                ud.shared.phase(),
                crate::session::Phase::Streaming | crate::session::Phase::Paused
            ) {
                ud.had_frames = true;
            }
        })
        .register()?;

    let bytes = enum_format_pod();
    let mut params = [Pod::from_bytes(&bytes).ok_or(pw::Error::CreationFailed)?];
    stream.connect(
        spa::utils::Direction::Input,
        Some(node_id),
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;
    mainloop.run();
    let _ = stream.disconnect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_format_round_trips() {
        for f in OFFERED_FORMATS {
            assert_eq!(pixel_format(spa_format(f)), Some(f));
        }
        assert_eq!(pixel_format(VideoFormat::NV12), None);
    }

    #[test]
    fn pods_serialize_and_parse() {
        let b = enum_format_pod();
        assert!(!b.is_empty());
        assert!(Pod::from_bytes(&b).is_some());
        let b = buffers_pod();
        assert!(Pod::from_bytes(&b).is_some());
    }

    #[test]
    fn spa_constants_match_the_ones_used_in_negotiate() {
        assert_eq!(negotiate::spa_data::MEM_PTR, spa::sys::SPA_DATA_MemPtr);
        assert_eq!(negotiate::spa_data::MEM_FD, spa::sys::SPA_DATA_MemFd);
        assert_eq!(negotiate::spa_data::DMA_BUF, spa::sys::SPA_DATA_DmaBuf);
        assert_eq!(
            negotiate::CHUNK_CORRUPTED,
            spa::sys::SPA_CHUNK_FLAG_CORRUPTED as i32
        );
    }

    // ---- A real PipeWire daemon, a fake screen producer, our consumer -------------------
    //
    // Needs a running `pipewire` + `wireplumber` (see docs/linux-wayland-test-plan.md, "Headless
    // PipeWire loopback"). Without a socket in $XDG_RUNTIME_DIR the test says so and passes.
    // It does not test the portal; the producer stands in for the compositor.

    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::Duration;

    const W: u32 = 64;
    const H: u32 = 48;

    fn socket_path() -> Option<std::path::PathBuf> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
        let name = std::env::var_os("PIPEWIRE_REMOTE").unwrap_or_else(|| "pipewire-0".into());
        let p = std::path::Path::new(&dir).join(name);
        p.exists().then_some(p)
    }

    fn fixed_format_pod() -> Vec<u8> {
        use spa::param::ParamType;
        use spa::param::format::{FormatProperties as P, MediaSubtype, MediaType};
        serialize(Object {
            type_: SpaTypes::ObjectParamFormat.as_raw(),
            id: ParamType::EnumFormat.as_raw(),
            properties: vec![
                Property::new(
                    P::MediaType.as_raw(),
                    Value::Id(Id(MediaType::Video.as_raw())),
                ),
                Property::new(
                    P::MediaSubtype.as_raw(),
                    Value::Id(Id(MediaSubtype::Raw.as_raw())),
                ),
                Property::new(
                    P::VideoFormat.as_raw(),
                    Value::Id(Id(VideoFormat::BGRx.as_raw())),
                ),
                Property::new(
                    P::VideoSize.as_raw(),
                    Value::Rectangle(Rectangle {
                        width: W,
                        height: H,
                    }),
                ),
                Property::new(
                    P::VideoFramerate.as_raw(),
                    Value::Fraction(Fraction { num: 30, denom: 1 }),
                ),
            ],
        })
    }

    fn producer_buffers_pod() -> Vec<u8> {
        let int = |k: u32, v: u32| Property::new(k, Value::Int(v as i32));
        serialize(Object {
            type_: SpaTypes::ObjectParamBuffers.as_raw(),
            id: spa::param::ParamType::Buffers.as_raw(),
            properties: vec![
                int(spa::sys::SPA_PARAM_BUFFERS_buffers, 4),
                int(spa::sys::SPA_PARAM_BUFFERS_blocks, 1),
                int(spa::sys::SPA_PARAM_BUFFERS_size, W * H * 4),
                int(spa::sys::SPA_PARAM_BUFFERS_stride, W * 4),
                Property::new(
                    spa::sys::SPA_PARAM_BUFFERS_dataType,
                    Value::Choice(ChoiceValue::Int(Choice(
                        ChoiceFlags::empty(),
                        ChoiceEnum::Flags {
                            default: 1 << spa::sys::SPA_DATA_MemFd,
                            flags: vec![1 << spa::sys::SPA_DATA_MemFd],
                        },
                    ))),
                ),
            ],
        })
    }

    /// A fake compositor: a video source node that draws a flat colour whose blue value is the
    /// current `level`, with a row stride equal to the width.
    fn produce(
        node_tx: std::sync::mpsc::Sender<u32>,
        level: Arc<AtomicU32>,
        stop: Arc<AtomicBool>,
    ) -> Result<(), pw::Error> {
        pw::init();
        let ml = pw::main_loop::MainLoopRc::new(None)?;
        let ctx = pw::context::ContextRc::new(&ml, None)?;
        let core = ctx.connect_rc(None)?;
        let stream = pw::stream::StreamRc::new(
            core,
            "cc-test-source",
            pw::properties::properties! {
                *pw::keys::MEDIA_CLASS => "Video/Source",
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_ROLE => "Screen",
                *pw::keys::NODE_NAME => "cc-test-source",
            },
        )?;
        let tx = node_tx;
        let _l = stream
            .add_local_listener_with_user_data(())
            .state_changed(move |s, _, _, new| {
                if matches!(new, pw::stream::StreamState::Paused) {
                    let _ = tx.send(s.node_id());
                }
            })
            .param_changed(|s, _, id, p| {
                if p.is_some() && id == spa::param::ParamType::Format.as_raw() {
                    let b = producer_buffers_pod();
                    if let Some(pod) = Pod::from_bytes(&b) {
                        let _ = s.update_params(&mut [pod]);
                    }
                }
            })
            .process(move |s, _| {
                if let Some(mut buf) = s.dequeue_buffer() {
                    let d = &mut buf.datas_mut()[0];
                    if let Some(bytes) = d.data() {
                        let blue = level.load(Ordering::Relaxed) as u8;
                        for px in bytes
                            .as_chunks_mut::<4>()
                            .0
                            .iter_mut()
                            .take((W * H) as usize)
                        {
                            px.copy_from_slice(&[blue, 7, 9, 0]);
                        }
                    }
                    let c = d.chunk_mut();
                    *c.offset_mut() = 0;
                    *c.stride_mut() = (W * 4) as i32;
                    *c.size_mut() = W * H * 4;
                }
            })
            .register()?;
        let bytes = fixed_format_pod();
        let mut params = [Pod::from_bytes(&bytes).ok_or(pw::Error::CreationFailed)?];
        stream.connect(
            spa::utils::Direction::Output,
            None,
            pw::stream::StreamFlags::DRIVER | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )?;
        let s2 = stream.clone();
        let tick = ml.loop_().add_timer(move |_| {
            let _ = s2.trigger_process();
        });
        tick.update_timer(
            Some(Duration::from_millis(5)),
            Some(Duration::from_millis(20)),
        );
        let ml2 = ml.clone();
        let watch = ml.loop_().add_timer(move |_| {
            if stop.load(Ordering::Relaxed) {
                ml2.quit();
            }
        });
        watch.update_timer(
            Some(Duration::from_millis(20)),
            Some(Duration::from_millis(20)),
        );
        ml.run();
        Ok(())
    }

    fn next_picture(shared: &Shared, secs: u64) -> Option<crate::RawFrame> {
        match shared.mailbox.take_timeout(Duration::from_secs(secs)) {
            crate::session::Taken::Item(f) => Some(f),
            _ => None,
        }
    }

    #[test]
    fn frames_flow_from_a_real_pipewire_node_through_the_backend_code() {
        let Some(sock) = socket_path() else {
            eprintln!("skipped: no PipeWire socket in $XDG_RUNTIME_DIR (see the test plan)");
            return;
        };
        let level = Arc::new(AtomicU32::new(10));
        let stop = Arc::new(AtomicBool::new(false));
        let (ntx, nrx) = std::sync::mpsc::channel();
        let prod = {
            let (l, s) = (Arc::clone(&level), Arc::clone(&stop));
            std::thread::spawn(move || produce(ntx, l, s))
        };
        let node = nrx
            .recv_timeout(Duration::from_secs(10))
            .expect("the producer node did not appear");

        let fd: OwnedFd = UnixStream::connect(sock)
            .expect("connect to PipeWire")
            .into();
        let shared = Arc::new(Shared::new());
        shared.event(Event::PickerShown);
        shared.event(Event::Picked);
        let mut consumer = PwThread::spawn(fd, node, Arc::clone(&shared)).unwrap();

        let first = next_picture(&shared, 15)
            .unwrap_or_else(|| panic!("no picture arrived; phase = {:?}", shared.phase()));
        assert_eq!((first.width, first.height), (W, H));
        assert_eq!(&first.bgra[0..4], &[10, 7, 9, 255], "BGRx became BGRA");
        assert_eq!(shared.phase(), crate::session::Phase::Streaming);

        // A change of the producer's picture arrives.
        level.store(200, Ordering::Relaxed);
        let mut seen = false;
        for _ in 0..50 {
            if let Some(f) = next_picture(&shared, 1)
                && f.bgra[0] == 200
            {
                seen = true;
                break;
            }
        }
        assert!(seen, "the changed picture never arrived");

        // The source disappears: the session ends without a failure.
        stop.store(true, Ordering::Relaxed);
        let _ = prod.join();
        let mut ended = false;
        for _ in 0..100 {
            if shared.phase().is_final() {
                ended = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        eprintln!("phase after the source went away: {:?}", shared.phase());
        assert!(ended, "the session did not end when the node went away");
        assert!(
            matches!(shared.phase(), crate::session::Phase::Ended(_)),
            "{:?}",
            shared.phase()
        );
        consumer.stop();
    }
}
