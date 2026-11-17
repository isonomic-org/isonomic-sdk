// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Async frame reading and writing over any byte stream. One implementation
//! for the engine and the client.

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::frame::{self, FrameError};

/// Read one frame's body without decoding it, so a caller can decode
/// leniently (see [`crate::lenient`]). `Ok(None)` means the peer closed the
/// connection cleanly between frames.
pub async fn read_frame<R>(reader: &mut R) -> Result<Option<Vec<u8>>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0u8; 4];
    match reader.read_exact(&mut prefix).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let mut body = vec![0u8; frame::body_len(prefix)?];
    reader.read_exact(&mut body).await?;
    Ok(Some(body))
}

/// Read one message. `Ok(None)` means the peer closed the connection cleanly
/// between frames.
pub async fn read_message<R, T>(reader: &mut R) -> Result<Option<T>, FrameError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    match read_frame(reader).await? {
        Some(body) => frame::decode(&body).map(Some),
        None => Ok(None),
    }
}

/// Write one message and flush it.
pub async fn write_message<W, T>(writer: &mut W, message: &T) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    writer.write_all(&frame::encode(message)?).await?;
    writer.flush().await?;
    Ok(())
}
