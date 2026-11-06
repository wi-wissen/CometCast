// SPDX-License-Identifier: GPL-3.0-or-later
//! Cast v2 framing: a 4-byte big-endian length, then the protobuf `CastMessage`.
//!
//! The size limit (64 KiB body) and the header size follow
//! `cast/common/channel/message_framer.cc` of Open Screen (commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef`, BSD-3-Clause, Copyright 2019 The Chromium
//! Authors).

use prost::Message;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::CastError;
use crate::proto::CastMessage;

/// Largest allowed message body (Open Screen: "Cast specifies a max message body size of 64 KiB").
pub const MAX_BODY: usize = 65_536;

/// Serialise one message with its length prefix.
pub fn encode(message: &CastMessage) -> Result<Vec<u8>, CastError> {
    let body = message.encode_to_vec();
    if body.is_empty() || body.len() > MAX_BODY {
        return Err(CastError::Protocol(format!(
            "message body of {} bytes is not allowed (1 to {MAX_BODY})",
            body.len()
        )));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Try to take one message off the front of `buf`. `Ok(None)` means more bytes are needed.
pub fn try_decode(buf: &mut Vec<u8>) -> Result<Option<CastMessage>, CastError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_BODY {
        return Err(CastError::Protocol(format!(
            "message of {len} bytes is too large"
        )));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let msg = CastMessage::decode(&buf[4..4 + len])
        .map_err(|e| CastError::Protocol(format!("bad CastMessage: {e}")))?;
    buf.drain(..4 + len);
    Ok(Some(msg))
}

/// Write one message.
pub async fn write_message<W: AsyncWrite + Unpin>(
    w: &mut W,
    message: &CastMessage,
) -> Result<(), CastError> {
    let bytes = encode(message)?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}

/// Read one message (waits for the whole frame).
pub async fn read_message<R: AsyncRead + Unpin>(r: &mut R) -> Result<CastMessage, CastError> {
    let mut head = [0u8; 4];
    r.read_exact(&mut head).await?;
    let len = u32::from_be_bytes(head) as usize;
    if len > MAX_BODY {
        return Err(CastError::Protocol(format!(
            "message of {len} bytes is too large"
        )));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    CastMessage::decode(body.as_slice())
        .map_err(|e| CastError::Protocol(format!("bad CastMessage: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg() -> CastMessage {
        CastMessage::text(
            "sender-0",
            "receiver-0",
            "urn:x-cast:com.google.cast.tp.heartbeat",
            "{\"type\":\"PING\"}".into(),
        )
    }

    #[test]
    fn encode_has_big_endian_length() {
        let bytes = encode(&msg()).unwrap();
        let len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(len, bytes.len() - 4);
    }

    #[test]
    fn decoder_handles_split_and_back_to_back_frames() {
        let one = encode(&msg()).unwrap();
        let mut stream = one.clone();
        stream.extend_from_slice(&one);
        let mut buf = Vec::new();
        // Feed one byte at a time: no message until the frame is complete.
        let mut got = 0;
        for b in &stream {
            buf.push(*b);
            while let Some(m) = try_decode(&mut buf).unwrap() {
                assert_eq!(m, msg());
                got += 1;
            }
        }
        assert_eq!(got, 2);
        assert!(buf.is_empty());
    }

    #[test]
    fn oversized_length_is_rejected() {
        let mut buf = vec![0, 2, 0, 0, 1];
        assert!(try_decode(&mut buf).is_err());
    }

    #[test]
    fn garbage_body_is_rejected() {
        let mut buf = vec![0, 0, 0, 2, 0xff, 0xff];
        assert!(try_decode(&mut buf).is_err());
    }

    #[tokio::test]
    async fn async_round_trip() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_message(&mut a, &msg()).await.unwrap();
        assert_eq!(read_message(&mut b).await.unwrap(), msg());
    }
}
