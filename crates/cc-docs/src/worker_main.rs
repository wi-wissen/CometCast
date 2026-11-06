// SPDX-License-Identifier: GPL-3.0-or-later
//! The entry point of the render worker process: read one request from stdin, parse and
//! render, write one reply to stdout, exit.
//!
//! The process is started by the parent in its own process group with an empty environment
//! (see `process.rs`). Right after the fixed-size request head, and **before the PDF is
//! read**, it restricts itself (`hardening.rs`: resource limits, Landlock, seccomp). This
//! code only does the work and keeps stdout clean. Nothing here prints anything but the
//! reply.

use std::io::{Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::document::Limits;
use crate::error::DocError;
use crate::hardening::{self, Plan};
use crate::prescan::ScanLevel;
use crate::protection::Mask;
use crate::wire::{
    self, KIND_ERROR, KIND_INFO, KIND_PAGE, OP_INFO, OP_RENDER, REQ_HEAD, ReplyHead, Request,
};
use crate::worker::{Job, parse, render_one};

/// Stack of the thread that parses and renders inside the worker process. Smaller than the
/// 64 MiB of the in-process engine so that the address-space limit can be low.
// UNCERTAIN: 16 MiB was enough for the deep-nesting tests in `tests/hostile.rs`; it is not
// derived from hayro's recursion limits. The measured lowest working limit is in
// `docs/documents.md`.
const PROCESS_STACK: usize = 16 * 1024 * 1024;

enum Done {
    Info(Vec<crate::document::PageSize>),
    Page(crate::document::RenderedPage),
}

/// Run the render worker. Returns the process exit code (0 when a reply, also an error
/// reply, was written). Call it from `main` of the worker program and exit with the result.
///
/// On `wasm32` there is no worker: it returns 2.
pub fn worker_main() -> i32 {
    let mut stdin = std::io::stdin().lock();
    let mut head = [0u8; REQ_HEAD];
    if stdin.read_exact(&mut head).is_err() {
        return 3;
    }
    let mut mask = Mask::default();
    let result = wire::decode_request(&head).and_then(|req| {
        // First restrict this process, then touch the hostile bytes. A mandatory limit that
        // cannot be set ends the job here.
        mask = hardening::apply(Plan {
            memory_bytes: req.limits.max_worker_memory_bytes,
            cpu_secs: req.cpu_secs,
            require_os_sandbox: req.limits.require_os_sandbox,
        })?;
        if req.pdf_len > req.limits.max_file_bytes as u64 {
            return Err(DocError::TooLarge {
                size: usize::try_from(req.pdf_len).unwrap_or(usize::MAX),
                max: req.limits.max_file_bytes,
            });
        }
        // Grow while reading: a lying length costs only what was really sent.
        let mut pdf = Vec::new();
        let n = stdin
            .by_ref()
            .take(req.pdf_len)
            .read_to_end(&mut pdf)
            .map_err(|_| DocError::Internal("cannot read the document"))?;
        if n as u64 != req.pdf_len {
            return Err(DocError::Internal("document ended early"));
        }
        compute(req, pdf)
    });
    write_reply(result, mask)
}

fn compute(req: Request, pdf: Vec<u8>) -> Result<Done, DocError> {
    let handle = std::thread::Builder::new()
        .name("cc-docs-render".into())
        .stack_size(PROCESS_STACK)
        .spawn(move || {
            catch_unwind(AssertUnwindSafe(|| compute_inner(&req, pdf)))
                .unwrap_or(Err(DocError::Panicked))
        })
        .map_err(|_| DocError::Internal("cannot start thread"))?;
    handle.join().unwrap_or(Err(DocError::Panicked))
}

fn compute_inner(req: &Request, pdf: Vec<u8>) -> Result<Done, DocError> {
    let bytes = Arc::new(pdf);
    let limits: &Limits = &req.limits;
    match req.op {
        OP_INFO => {
            let (_, sizes) = parse(&bytes, limits, ScanLevel::Full)?;
            Ok(Done::Info(sizes))
        }
        OP_RENDER => {
            // The same bytes passed the full scan at open; the structure scan is cheap.
            let (pdf, sizes) = parse(&bytes, limits, ScanLevel::Structure)?;
            let index = usize::try_from(req.index).unwrap_or(usize::MAX);
            if index >= sizes.len() {
                return Err(DocError::PageOutOfRange {
                    index,
                    count: sizes.len(),
                });
            }
            let side = limits.max_output_side.min(u32::from(u16::MAX));
            let (w, h) = (req.width, req.height);
            if w == 0 || h == 0 {
                return Err(DocError::InvalidWidth);
            }
            if w > side || h > side || u64::from(w) * u64::from(h) > limits.max_output_pixels {
                return Err(DocError::OutputTooLarge {
                    width: u64::from(w),
                    height: u64::from(h),
                });
            }
            let settings = hayro::hayro_interpret::InterpreterSettings::default();
            let cache = hayro::RenderCache::new();
            let job = Job {
                index,
                width: w,
                height: h,
            };
            Ok(Done::Page(render_one(&pdf, &cache, &settings, job)?))
        }
        _ => Err(DocError::Internal("unknown operation")),
    }
}

fn write_reply(result: Result<Done, DocError>, mask: Mask) -> i32 {
    let peak_kib = hardening::own_peak_kib();
    let mut out = std::io::stdout().lock();
    let written = match result {
        Ok(Done::Info(sizes)) => {
            let head = wire::encode_head(&ReplyHead {
                kind: KIND_INFO,
                code: 0,
                peak_kib,
                x: sizes.len() as u64,
                y: 0,
                z: mask.0,
            });
            let mut payload = Vec::with_capacity(sizes.len() * 8);
            for s in &sizes {
                payload.extend_from_slice(&s.width_pt.to_le_bytes());
                payload.extend_from_slice(&s.height_pt.to_le_bytes());
            }
            out.write_all(&head).and_then(|()| out.write_all(&payload))
        }
        Ok(Done::Page(p)) => {
            let head = wire::encode_head(&ReplyHead {
                kind: KIND_PAGE,
                code: 0,
                peak_kib,
                x: u64::from(p.width),
                y: u64::from(p.height),
                z: mask.0,
            });
            out.write_all(&head).and_then(|()| out.write_all(&p.rgba))
        }
        Err(e) => {
            let (code, x, y) = wire::error_to_wire(&e);
            let head = wire::encode_head(&ReplyHead {
                kind: KIND_ERROR,
                code,
                peak_kib,
                x,
                y,
                z: mask.0,
            });
            out.write_all(&head)
        }
    };
    match written.and_then(|()| out.flush()) {
        Ok(()) => 0,
        Err(_) => 4,
    }
}
