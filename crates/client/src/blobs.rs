//! The `blobs` interface: store bytes, get a handle, read them back. Large
//! blobs are split into chunks here, so callers pass and receive whole
//! byte vectors.

use isonomic_proto::blobs::CHUNK_BYTES;
use isonomic_proto::{BlobInfo, Bytes, Call, Reply, Token};

use crate::{Client, ClientError};

/// Blob calls, made as the shell or under an app's token.
#[derive(Clone)]
pub struct Blobs {
    client: Client,
    token: Option<Token>,
}

impl Blobs {
    pub(crate) fn new(client: Client, token: Option<Token>) -> Self {
        Self { client, token }
    }

    async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.client.send(self.token.clone(), call, None).await
    }

    async fn info_of(&self, call: Call) -> Result<BlobInfo, ClientError> {
        match self.call(call).await? {
            Reply::Blob(info) => Ok(info),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Store `bytes` and return its handle and size. One call when it fits
    /// in a chunk; otherwise a create and one write per chunk, in order.
    pub async fn put(&self, content_type: &str, bytes: &[u8]) -> Result<BlobInfo, ClientError> {
        let content_type = content_type.to_string();
        if bytes.len() as u64 <= CHUNK_BYTES {
            let data = Bytes(bytes.to_vec());
            return self.info_of(Call::BlobsPut { content_type, data }).await;
        }
        let size = bytes.len() as u64;
        let mut info = self
            .info_of(Call::BlobsCreate { content_type, size })
            .await?;
        for (index, chunk) in bytes.chunks(CHUNK_BYTES as usize).enumerate() {
            let call = Call::BlobsWrite {
                handle: info.handle.clone(),
                offset: index as u64 * CHUNK_BYTES,
                data: Bytes(chunk.to_vec()),
            };
            info = self.info_of(call).await?;
        }
        Ok(info)
    }

    /// Read a whole blob, chunk by chunk.
    pub async fn get(&self, handle: &str) -> Result<(BlobInfo, Vec<u8>), ClientError> {
        let mut bytes = Vec::new();
        loop {
            let call = Call::BlobsGet {
                handle: handle.to_string(),
                offset: bytes.len() as u64,
                length: None,
            };
            let Reply::BlobData { info, data, .. } = self.call(call).await? else {
                return Err(ClientError::UnexpectedReply);
            };
            let progressed = !data.0.is_empty();
            bytes.extend_from_slice(&data.0);
            if bytes.len() as u64 >= info.size || !progressed {
                return Ok((info, bytes));
            }
        }
    }

    pub async fn info(&self, handle: &str) -> Result<BlobInfo, ClientError> {
        let handle = handle.to_string();
        self.info_of(Call::BlobsInfo { handle }).await
    }

    pub async fn delete(&self, handle: &str) -> Result<(), ClientError> {
        let handle = handle.to_string();
        match self.call(Call::BlobsDelete { handle }).await? {
            Reply::Deleted {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }
}
