// SPDX-License-Identifier: GPL-3.0-or-later
//! Content transfer between sender and receiver: images, links, PDF documents and page
//! commands, over the `control` data channel.
//!
//! * [`wire`]: the messages (the format of the web client, docs/web-m2.md, plus the
//!   `document` kinds), validation of every field, link rules.
//! * [`outgoing`]: the sender side, turns an image, PDF, link or command into messages.
//! * [`receiver`]: the receiver side, a state machine with hard limits that reassembles
//!   transfers, hands PDFs to `cc-docs` and reports [`ContentEvent`]s.
//!
//! This crate has no network code; `cc-transport` moves the messages, `cc-sender` and
//! `cc-receiver` connect the two. See docs/web-m2.md and docs/transport.md.
//!
//! UNCERTAIN: the `document` message kinds are defined by the native clients only; the web
//! client does not render PDFs yet and answers a document with a notice.

pub mod outgoing;
pub mod receiver;
pub mod wire;

pub use outgoing::{
    Outgoing, SendError, Transfer, cmd_message, document_transfer, image_transfer, link_message,
};
pub use receiver::{
    Blob, ContentError, ContentEvent, ContentOptions, ContentReceiver, DEFAULT_MAX_VIOLATIONS,
    DEFAULT_TARGET_WIDTH, MAX_SKIPPED_BYTES, Next, Work, WorkDone,
};
pub use wire::{
    CHUNK_SIZE, ImageMime, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, MAX_LINK_LENGTH, MAX_NAME_LENGTH,
    MAX_TEXT_CHARS, Message, ParseError, TransferKind, announced_transfer, validate_link,
};
