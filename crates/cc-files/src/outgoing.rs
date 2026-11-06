// SPDX-License-Identifier: GPL-3.0-or-later
//! The sender side: turns an image, a PDF, a link or a command into the messages to put on
//! the control data channel. Pure and synchronous; the caller sends the items one by one and
//! waits when the channel is full (`cc-sender` does that with the back-pressure of
//! `cc-transport`).

use cc_proto::ControlCmd;

use crate::wire::{
    CHUNK_SIZE, ImageMime, MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, MAX_NAME_LENGTH, Message,
    validate_link,
};

/// One item to send on the data channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing<'a> {
    /// A text message.
    Text(String),
    /// A binary chunk (exactly [`CHUNK_SIZE`] bytes, except the last one of a transfer).
    Binary(&'a [u8]),
}

/// Why something cannot be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// No bytes.
    #[error("the file is empty")]
    Empty,
    /// Larger than the format allows.
    #[error("the file is too large")]
    TooLarge,
    /// A PDF must start with `%PDF-` in its first 1024 bytes.
    #[error("not a PDF file")]
    NotPdf,
    /// The link is not an acceptable http(s) URL.
    #[error("the link must be an http:// or https:// address")]
    BadLink,
}

/// The messages of one transfer: the start message, the chunks, the end message.
#[derive(Debug, Clone)]
pub struct Transfer<'a> {
    start: Option<Message>,
    data: &'a [u8],
    pos: usize,
    end: Option<Message>,
}

impl<'a> Iterator for Transfer<'a> {
    type Item = Outgoing<'a>;

    fn next(&mut self) -> Option<Outgoing<'a>> {
        if let Some(m) = self.start.take() {
            return Some(Outgoing::Text(m.to_json()));
        }
        if self.pos < self.data.len() {
            let end = (self.pos + CHUNK_SIZE).min(self.data.len());
            let chunk = &self.data[self.pos..end];
            self.pos = end;
            return Some(Outgoing::Binary(chunk));
        }
        self.end.take().map(|m| Outgoing::Text(m.to_json()))
    }
}

fn clip_name(name: &str, fallback: &str) -> String {
    let n: String = name.chars().take(MAX_NAME_LENGTH).collect();
    let n = crate::wire::sanitize_name(&n);
    if n.is_empty() { fallback.to_string() } else { n }
}

/// The messages that send an image.
pub fn image_transfer<'a>(
    name: &str,
    mime: ImageMime,
    data: &'a [u8],
) -> Result<Transfer<'a>, SendError> {
    if data.is_empty() {
        return Err(SendError::Empty);
    }
    if data.len() as u64 > MAX_IMAGE_BYTES {
        return Err(SendError::TooLarge);
    }
    Ok(Transfer {
        start: Some(Message::Image {
            name: clip_name(name, "image"),
            mime,
            size: data.len() as u64,
        }),
        data,
        pos: 0,
        end: Some(Message::ImageEnd),
    })
}

/// The messages that send a PDF document.
pub fn document_transfer<'a>(name: &str, data: &'a [u8]) -> Result<Transfer<'a>, SendError> {
    if data.is_empty() {
        return Err(SendError::Empty);
    }
    if data.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(SendError::TooLarge);
    }
    let head = &data[..data.len().min(1024)];
    if !head.windows(5).any(|w| w == b"%PDF-") {
        return Err(SendError::NotPdf);
    }
    Ok(Transfer {
        start: Some(Message::Document {
            name: clip_name(name, "document.pdf"),
            size: data.len() as u64,
        }),
        data,
        pos: 0,
        end: Some(Message::DocumentEnd),
    })
}

/// The message that sends a link (validated and normalised first).
pub fn link_message(url: &str) -> Result<String, SendError> {
    let url = validate_link(url).ok_or(SendError::BadLink)?;
    Ok(Message::Link { url }.to_json())
}

/// The message that sends a remote-control command.
pub fn cmd_message(cmd: ControlCmd) -> String {
    Message::Cmd(cmd).to_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_follow_the_rule() {
        let data = vec![7u8; CHUNK_SIZE * 2 + 100];
        let items: Vec<_> = image_transfer("a.png", ImageMime::Png, &data).unwrap().collect();
        assert_eq!(items.len(), 1 + 3 + 1);
        let sizes: Vec<usize> = items
            .iter()
            .filter_map(|i| match i {
                Outgoing::Binary(b) => Some(b.len()),
                Outgoing::Text(_) => None,
            })
            .collect();
        assert_eq!(sizes, [CHUNK_SIZE, CHUNK_SIZE, 100]);
        assert!(matches!(&items[0], Outgoing::Text(t) if t.contains("\"image\"") && t.contains("\"size\":32868")));
        assert_eq!(items[4], Outgoing::Text(r#"{"kind":"image_end"}"#.into()));
    }

    #[test]
    fn an_exact_multiple_has_no_short_chunk() {
        let data = vec![1u8; CHUNK_SIZE * 2];
        let n = document_transfer("x.pdf", &[b"%PDF-1.4\n".as_slice(), &data].concat())
            .unwrap()
            .filter(|i| matches!(i, Outgoing::Binary(_)))
            .count();
        assert_eq!(n, 3);
        let n = image_transfer("a.png", ImageMime::Png, &data)
            .unwrap()
            .filter(|i| matches!(i, Outgoing::Binary(b) if b.len() == CHUNK_SIZE))
            .count();
        assert_eq!(n, 2);
    }

    #[test]
    fn refusals() {
        assert_eq!(image_transfer("a", ImageMime::Png, &[]).unwrap_err(), SendError::Empty);
        let big = vec![0u8; MAX_IMAGE_BYTES as usize + 1];
        assert_eq!(image_transfer("a", ImageMime::Png, &big).unwrap_err(), SendError::TooLarge);
        assert_eq!(document_transfer("a.pdf", b"hello").unwrap_err(), SendError::NotPdf);
        assert_eq!(document_transfer("a.pdf", &[]).unwrap_err(), SendError::Empty);
        assert_eq!(link_message("javascript:1").unwrap_err(), SendError::BadLink);
        assert_eq!(
            link_message("https://example.org").unwrap(),
            r#"{"kind":"link","url":"https://example.org/"}"#
        );
        assert_eq!(cmd_message(ControlCmd::NextPage), r#"{"cmd":"next_page","kind":"cmd"}"#);
    }

    #[test]
    fn names_are_clipped() {
        let long = "n".repeat(400);
        let first = image_transfer(&long, ImageMime::Gif, b"GIF89a").unwrap().next().unwrap();
        let Outgoing::Text(t) = first else { panic!() };
        assert!(crate::wire::parse_message(&t, MAX_IMAGE_BYTES, MAX_DOCUMENT_BYTES).is_ok());
        let first = image_transfer("\u{202e}", ImageMime::Gif, b"GIF89a").unwrap().next().unwrap();
        let Outgoing::Text(t) = first else { panic!() };
        assert!(t.contains("\"image\""), "falls back to a default name");
    }
}
