//! The `models` interface: list, inspect, stream and embed.

use isonomic_proto::models::{ModelChunk, ModelDetail, ModelInfo, StopReason, StreamRequest};
use isonomic_proto::{Call, Event, Reply, Token, WireError};

use crate::{Client, ClientError, Subscription};

/// Model calls, made as the shell or under an app's token.
#[derive(Clone)]
pub struct Models {
    client: Client,
    token: Option<Token>,
}

impl Models {
    pub(crate) fn new(client: Client, token: Option<Token>) -> Self {
        Self { client, token }
    }

    async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.client.send(self.token.clone(), call, None).await
    }

    /// The models the caller may use. An app sees only what its manifest
    /// allows.
    pub async fn list(&self) -> Result<Vec<ModelInfo>, ClientError> {
        match self.call(Call::ModelsList).await? {
            Reply::Models(models) => Ok(models),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    pub async fn info(&self, model: &str) -> Result<ModelDetail, ClientError> {
        let call = Call::ModelsInfo {
            model: model.to_string(),
        };
        match self.call(call).await? {
            Reply::Model(detail) => Ok(detail),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Start a chat completion. Chunks arrive on the returned stream.
    pub async fn stream(&self, request: StreamRequest) -> Result<ModelStream, ClientError> {
        let (reply, events) = self
            .client
            .open(self.token.clone(), Call::ModelsStream(request))
            .await?;
        let Reply::Stream { model, .. } = reply else {
            return Err(ClientError::UnexpectedReply);
        };
        Ok(ModelStream {
            events,
            model,
            client: self.client.clone(),
            token: self.token.clone(),
            done: false,
        })
    }

    /// One vector per input, in order. `model: None` is the user's default
    /// embeddings model.
    pub async fn embed(
        &self,
        model: Option<&str>,
        input: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, ClientError> {
        let call = Call::ModelsEmbed {
            model: model.map(str::to_string),
            input,
        };
        match self.call(call).await? {
            Reply::Embeddings { vectors, .. } => Ok(vectors),
            _ => Err(ClientError::UnexpectedReply),
        }
    }
}

/// A running chat completion. Dropping it before the last chunk cancels it.
pub struct ModelStream {
    events: Subscription,
    model: String,
    client: Client,
    token: Option<Token>,
    done: bool,
}

impl ModelStream {
    /// The model the engine chose.
    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn subscription(&self) -> u64 {
        self.events.id()
    }

    /// The next chunk. The last one has `stop` set; after it, `None`. A
    /// dropped connection ends the stream with `Disconnected`.
    pub async fn next(&mut self) -> Option<Result<ModelChunk, ClientError>> {
        if self.done {
            return None;
        }
        loop {
            match self.events.next().await {
                Some(Event::ModelChunk(chunk)) => {
                    self.done = chunk.stop.is_some();
                    return Some(Ok(chunk));
                }
                Some(_) => continue,
                None => {
                    self.done = true;
                    return Some(Err(ClientError::Disconnected));
                }
            }
        }
    }

    /// Ask the engine to stop. The stream still ends with its last chunk,
    /// whose `stop` is `cancelled`.
    pub async fn cancel(&self) -> Result<(), ClientError> {
        let call = Call::ModelsCancel {
            subscription: self.events.id(),
        };
        match self.client.send(self.token.clone(), call, None).await? {
            Reply::Cancelled {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Read to the end and return the text. A stream that stopped with an
    /// error returns that error.
    pub async fn text(mut self) -> Result<String, ClientError> {
        let mut text = String::new();
        while let Some(chunk) = self.next().await {
            let chunk = chunk?;
            text.push_str(&chunk.text);
            if chunk.stop == Some(StopReason::Error) {
                let error = chunk.error.unwrap_or_else(|| {
                    WireError::new(isonomic_proto::ErrorCode::Internal, "the model failed")
                });
                return Err(ClientError::Engine(error));
            }
        }
        Ok(text)
    }
}

impl Drop for ModelStream {
    fn drop(&mut self) {
        if !self.done {
            let call = Call::ModelsCancel {
                subscription: self.events.id(),
            };
            self.client.send_now(self.token.clone(), call);
        }
    }
}
