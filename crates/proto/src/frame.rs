//! Framing: a 4-byte big-endian length, then that many bytes of JSON.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::MAX_FRAME_BYTES;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame of {0} bytes exceeds the {MAX_FRAME_BYTES}-byte limit")]
    TooLarge(usize),
    /// Why it did not decode, as category and place only. serde's own
    /// message is dropped: it can quote the input, and a client's frame can
    /// hold recovery words.
    #[error("frame is not a valid message ({0})")]
    Json(DecodeFailure),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<serde_json::Error> for FrameError {
    fn from(error: serde_json::Error) -> Self {
        FrameError::Json(DecodeFailure::of(&error))
    }
}

/// What may be said about a JSON failure without quoting the input: serde's
/// category (`syntax`, `data`, `eof` or `io`) and the line and column.
/// `Display` and `Debug` print only these.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DecodeFailure {
    category: &'static str,
    line: usize,
    column: usize,
}

impl DecodeFailure {
    pub fn of(error: &serde_json::Error) -> Self {
        use serde_json::error::Category;
        let category = match error.classify() {
            Category::Io => "io",
            Category::Syntax => "syntax",
            Category::Data => "data",
            Category::Eof => "eof",
        };
        Self {
            category,
            line: error.line(),
            column: error.column(),
        }
    }

    pub fn category(&self) -> &'static str {
        self.category
    }
}

impl std::fmt::Display for DecodeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "{} error", self.category),
            line => write!(
                f,
                "{} error at line {line} column {}",
                self.category, self.column
            ),
        }
    }
}

impl std::fmt::Debug for DecodeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecodeFailure({self})")
    }
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

/// Parse a frame body. A failure says only serde's category and the line
/// and column ([`DecodeFailure`]), never the input. To answer a client's
/// frame, use [`lenient::decode_client`](crate::lenient::decode_client).
pub fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, FrameError> {
    Ok(serde_json::from_slice(body)?)
}
