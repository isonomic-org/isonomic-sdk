// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data carried by the `blobs` interface: byte objects an app stores by
//! handle, such as Chat's attachments.
//!
//! Bytes travel as base64 text inside the JSON frame. A frame is at most
//! 1 MiB, so one call carries at most [`CHUNK_BYTES`]; a larger blob is
//! created with its size and written in order, chunk by chunk, and read
//! back the same way. The client does the chunking.

use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

/// Most raw bytes in one `blobs.put`, `blobs.write` or `blobs.get`. As
/// base64 this stays well inside a frame.
pub const CHUNK_BYTES: u64 = 512 * 1024;

/// A stored blob. `written == size` once every byte has arrived; only then
/// can it be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobInfo {
    /// Opaque, chosen by the engine, valid only for the app that stored it.
    pub handle: String,
    /// A media type such as `image/png`, as the app gave it.
    pub content_type: String,
    /// Total size in bytes.
    pub size: u64,
    /// Bytes received so far.
    pub written: u64,
}

impl BlobInfo {
    pub fn complete(&self) -> bool {
        self.written == self.size
    }
}

/// Raw bytes, as base64 text on the wire.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Bytes(pub Vec<u8>);

impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bytes({} bytes)", self.0.len())
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Base64;
        impl Visitor<'_> for Base64 {
            type Value = Bytes;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("base64 text")
            }
            fn visit_str<E: de::Error>(self, text: &str) -> Result<Bytes, E> {
                decode(text)
                    .map(Bytes)
                    .ok_or_else(|| E::custom("invalid base64"))
            }
        }
        deserializer.deserialize_str(Base64)
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let word = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= n {
                out.push(ALPHABET[((word >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The inverse of [`encode`]. `None` for anything that is not padded
/// base64.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if !text.len().is_multiple_of(4) {
        return None;
    }
    let value = |c: u8| ALPHABET.iter().position(|&a| a == c).map(|v| v as u32);
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for (index, quad) in text.chunks(4).enumerate() {
        let last = index == text.len() / 4 - 1;
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut word = 0u32;
        for &c in &quad[..4 - pad] {
            word = (word << 6) | value(c)?;
        }
        word <<= 6 * pad as u32;
        let bytes = [(word >> 16) as u8, (word >> 8) as u8, word as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length() {
        for len in 0..40 {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let text = encode(&bytes);
            assert_eq!(decode(&text), Some(bytes), "length {len}");
        }
        assert_eq!(encode(b"hello"), "aGVsbG8=");
        assert_eq!(encode(b"hi"), "aGk=");
    }

    #[test]
    fn base64_rejects_damage() {
        for bad in ["a", "aGk", "a===", "aG=k", "aGk=aGk=", "a*k="] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }
}
