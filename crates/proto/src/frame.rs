//! Framing: a 4-byte big-endian length, then that many bytes of JSON.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::MAX_FRAME_BYTES;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame of {0} bytes exceeds the {MAX_FRAME_BYTES}-byte limit")]
    TooLarge(usize),
    #[error("frame is not a valid message: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Serialize one message into a complete frame.
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, FrameError> {
    let body = serde_json::to_vec(message)?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(body.len()));
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Read the length prefix, refusing frames over the limit.
pub fn body_len(prefix: [u8; 4]) -> Result<usize, FrameError> {
    let len = u32::from_be_bytes(prefix) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    Ok(len)
}

/// Parse a frame body. Its error carries serde's message, which can quote
/// the input. Never send it to a peer, or log it, for a frame a client
/// sent: the recovery kit's calls carry the person's words. Answer client
/// frames through [`lenient::decode_client`](crate::lenient::decode_client),
/// which never repeats a kit request's content.
pub fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, FrameError> {
    Ok(serde_json::from_slice(body)?)
}
