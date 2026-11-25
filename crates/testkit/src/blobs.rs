// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `blobs` interface of the in-memory engine. Blobs live in memory,
//! owned by the app that stored them; the shell can read them all. Tests
//! lower the engine's own size limit and read back what an app stored.

use std::collections::HashMap;
use std::sync::Arc;

use isonomic_proto::auth::too_large;
use isonomic_proto::blobs::CHUNK_BYTES;
use isonomic_proto::{BlobInfo, Bytes, Call, ErrorCode, Reply, WireError};

use crate::State;

/// The engine's own limit until a test changes it: 64 MiB.
pub const DEFAULT_LIMIT: u64 = 64 * 1024 * 1024;

struct Stored {
    owner: Option<String>,
    info: BlobInfo,
    data: Vec<u8>,
}

pub(crate) struct BlobsState {
    blobs: HashMap<String, Stored>,
    next: u64,
    limit: u64,
}

impl Default for BlobsState {
    fn default() -> Self {
        Self {
            blobs: HashMap::new(),
            next: 1,
            limit: DEFAULT_LIMIT,
        }
    }
}

/// Script and inspect the fake blob store.
#[derive(Clone)]
pub struct FakeBlobs {
    pub(crate) state: Arc<State>,
}

impl FakeBlobs {
    /// The engine's own limit, which applies on top of each manifest's.
    pub fn set_limit(&self, bytes: u64) {
        self.state.blobs.lock().unwrap().limit = bytes;
    }

    /// The bytes stored under `handle` so far, whoever owns it.
    pub fn stored(&self, handle: &str) -> Option<Vec<u8>> {
        let blobs = self.state.blobs.lock().unwrap();
        blobs.blobs.get(handle).map(|b| b.data.clone())
    }

    /// How many blobs exist, complete or not.
    pub fn count(&self) -> usize {
        self.state.blobs.lock().unwrap().blobs.len()
    }
}

fn unknown(handle: &str) -> WireError {
    WireError::new(ErrorCode::UnknownBlob, format!("no blob {handle}"))
}

impl BlobsState {
    fn create(&mut self, owner: Option<String>, content_type: String, size: u64) -> BlobInfo {
        let handle = format!("blob-{}", self.next);
        self.next += 1;
        let info = BlobInfo {
            handle: handle.clone(),
            content_type,
            size,
            written: 0,
        };
        let stored = Stored {
            owner,
            info: info.clone(),
            data: Vec::new(),
        };
        self.blobs.insert(handle, stored);
        info
    }

    /// The caller's blob. Another app's handle is as good as unknown.
    fn mine(&mut self, app: Option<&str>, handle: &str) -> Result<&mut Stored, WireError> {
        match self.blobs.get_mut(handle) {
            Some(blob) if app.is_none() || blob.owner.as_deref() == app => Ok(blob),
            _ => Err(unknown(handle)),
        }
    }

    fn write(
        &mut self,
        app: Option<&str>,
        handle: &str,
        offset: u64,
        data: Bytes,
    ) -> Result<BlobInfo, WireError> {
        let blob = self.mine(app, handle)?;
        if offset != blob.info.written {
            let message = format!("{handle} expects offset {}", blob.info.written);
            return Err(WireError::new(ErrorCode::BadRequest, message));
        }
        let end = offset + data.0.len() as u64;
        if data.0.len() as u64 > CHUNK_BYTES || end > blob.info.size {
            return Err(too_large(end, blob.info.size.min(offset + CHUNK_BYTES)));
        }
        blob.data.extend_from_slice(&data.0);
        blob.info.written = end;
        Ok(blob.info.clone())
    }
}

pub(crate) fn answer(state: &State, app: Option<&str>, call: Call) -> Result<Reply, WireError> {
    let mut blobs = state.blobs.lock().unwrap();
    let owner = app.map(str::to_string);
    let limit = blobs.limit;
    match call {
        Call::BlobsPut { content_type, data } => {
            let size = data.0.len() as u64;
            if size > limit.min(CHUNK_BYTES) {
                return Err(too_large(size, limit.min(CHUNK_BYTES)));
            }
            let handle = blobs.create(owner, content_type, size).handle;
            blobs.write(app, &handle, 0, data).map(Reply::Blob)
        }
        Call::BlobsCreate { size, .. } if size > limit => Err(too_large(size, limit)),
        Call::BlobsCreate { content_type, size } => {
            Ok(Reply::Blob(blobs.create(owner, content_type, size)))
        }
        Call::BlobsWrite {
            handle,
            offset,
            data,
        } => blobs.write(app, &handle, offset, data).map(Reply::Blob),
        Call::BlobsGet {
            handle,
            offset,
            length,
        } => {
            let blob = blobs.mine(app, &handle)?;
            if !blob.info.complete() {
                let message = format!(
                    "{handle} has {} of {} bytes",
                    blob.info.written, blob.info.size
                );
                return Err(WireError::new(ErrorCode::Incomplete, message));
            }
            let start = offset.min(blob.info.size);
            let take = length.unwrap_or(CHUNK_BYTES).min(CHUNK_BYTES);
            let end = (start + take).min(blob.info.size);
            let data = blob.data[start as usize..end as usize].to_vec();
            Ok(Reply::BlobData {
                info: blob.info.clone(),
                offset: start,
                data: Bytes(data),
            })
        }
        Call::BlobsInfo { handle } => Ok(Reply::Blob(blobs.mine(app, &handle)?.info.clone())),
        Call::BlobsDelete { handle } => {
            blobs.mine(app, &handle)?;
            blobs.blobs.remove(&handle);
            Ok(Reply::Deleted {})
        }
        other => Err(WireError::new(
            ErrorCode::UnknownMethod,
            format!("{} is not a blobs method", other.method()),
        )),
    }
}
