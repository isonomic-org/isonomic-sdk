//! Wire types for the Isonomic engine API, protocol version 0.
//!
//! One Unix socket carries three kinds of message: requests, responses and
//! events. Each message is one frame: a 4-byte big-endian length followed by
//! that many bytes of UTF-8 JSON, so a person can read the traffic and paste
//! it into a bug report. A binary encoding can replace JSON behind these
//! types without changing them.
//!
//! The first message on a connection is `hello`; the engine answers
//! `welcome` with the version both sides will speak.

pub mod auth;
pub mod blobs;
pub mod events;
pub mod frame;
#[cfg(feature = "tokio")]
pub mod io;
mod json;
pub mod lenient;
pub mod locate;
pub mod manifest;
pub mod models;
pub mod system;

use serde::{Deserialize, Serialize};

pub use blobs::{BlobInfo, Bytes};
pub use events::AppEvent;
pub use json::Json;
pub use manifest::{Capabilities, Manifest, Token};
pub use models::{
    MemoryRefusal, Message, ModelChunk, ModelDetail, ModelInfo, StopReason, StreamRequest,
};
pub use system::{HardwareProfile, ServiceState, ServiceStatus, SystemStatus};

/// The protocol version this crate describes.
pub const PROTOCOL_VERSION: u32 = 0;

/// Largest frame either side accepts.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

/// What a client sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Must be first. Lists every protocol version the client can speak.
    Hello { versions: Vec<u32>, client: String },
    /// One call. `id` is chosen by the client and echoed in the response.
    /// `token` is the calling app's capability token; a call without one
    /// comes from the shell.
    Request {
        id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<Token>,
        #[serde(flatten)]
        call: Call,
    },
}

/// Every method of every interface. Each method is named
/// `<interface>.<verb>`; new methods are added, never changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
#[non_exhaustive]
pub enum Call {
    /// Status of every service.
    #[serde(rename = "system.status")]
    SystemStatus,
    /// Status now, then an event for every change.
    #[serde(rename = "system.subscribe")]
    SystemSubscribe,
    #[serde(rename = "system.unsubscribe")]
    SystemUnsubscribe { subscription: u64 },
    /// The most recent log lines of one service.
    #[serde(rename = "system.logs")]
    SystemLogs { service: String, limit: Option<u32> },
    /// The hardware profile the engine chose.
    #[serde(rename = "system.profile")]
    SystemProfile,
    /// Stop every service in order, then the engine.
    #[serde(rename = "system.shutdown")]
    SystemShutdown,
    /// Shell only: record an app's approved manifest and issue its token.
    #[serde(rename = "apps.grant")]
    AppsGrant { manifest: Box<Manifest> },
    /// The models the caller may use.
    #[serde(rename = "models.list")]
    ModelsList,
    #[serde(rename = "models.info")]
    ModelsInfo { model: String },
    /// Start a chat completion. The reply names a subscription; its events
    /// are `model_chunk`s, the last with `stop` set.
    #[serde(rename = "models.stream")]
    ModelsStream(StreamRequest),
    /// Stop a stream. Its last chunk then arrives with `stop: cancelled`.
    #[serde(rename = "models.cancel")]
    ModelsCancel { subscription: u64 },
    /// Embed each input. `model: None` is the user's default embeddings
    /// model.
    #[serde(rename = "models.embed")]
    ModelsEmbed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        input: Vec<String>,
    },
    /// Store a blob of at most `CHUNK_BYTES` in one call.
    #[serde(rename = "blobs.put")]
    BlobsPut { content_type: String, data: Bytes },
    /// Reserve a larger blob, then send its bytes with `blobs.write`.
    #[serde(rename = "blobs.create")]
    BlobsCreate { content_type: String, size: u64 },
    /// Append the next chunk; `offset` must equal the bytes written so far.
    #[serde(rename = "blobs.write")]
    BlobsWrite {
        handle: String,
        offset: u64,
        data: Bytes,
    },
    /// Read up to `length` bytes (and at most `CHUNK_BYTES`) from `offset`.
    #[serde(rename = "blobs.get")]
    BlobsGet {
        handle: String,
        #[serde(default)]
        offset: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<u64>,
    },
    #[serde(rename = "blobs.info")]
    BlobsInfo { handle: String },
    #[serde(rename = "blobs.delete")]
    BlobsDelete { handle: String },
    /// Send `data` to every current subscriber of `topic` in the app's
    /// scope. Only the shell may name another `app`.
    #[serde(rename = "events.publish")]
    EventsPublish {
        topic: String,
        data: Json,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sender: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
    },
    /// Receive `app_event`s for `topics` (all topics when empty).
    #[serde(rename = "events.subscribe")]
    EventsSubscribe {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        topics: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
    },
    #[serde(rename = "events.unsubscribe")]
    EventsUnsubscribe { subscription: u64 },
}

impl Call {
    /// Every method this build knows, in interface order.
    pub const METHODS: &'static [&'static str] = &[
        "system.status",
        "system.subscribe",
        "system.unsubscribe",
        "system.logs",
        "system.profile",
        "system.shutdown",
        "apps.grant",
        "models.list",
        "models.info",
        "models.stream",
        "models.cancel",
        "models.embed",
        "blobs.put",
        "blobs.create",
        "blobs.write",
        "blobs.get",
        "blobs.info",
        "blobs.delete",
        "events.publish",
        "events.subscribe",
        "events.unsubscribe",
    ];

    /// The method name on the wire, for logs and the Activity tab.
    pub fn method(&self) -> &'static str {
        match self {
            Call::SystemStatus => "system.status",
            Call::SystemSubscribe => "system.subscribe",
            Call::SystemUnsubscribe { .. } => "system.unsubscribe",
            Call::SystemLogs { .. } => "system.logs",
            Call::SystemProfile => "system.profile",
            Call::SystemShutdown => "system.shutdown",
            Call::AppsGrant { .. } => "apps.grant",
            Call::ModelsList => "models.list",
            Call::ModelsInfo { .. } => "models.info",
            Call::ModelsStream(_) => "models.stream",
            Call::ModelsCancel { .. } => "models.cancel",
            Call::ModelsEmbed { .. } => "models.embed",
            Call::BlobsPut { .. } => "blobs.put",
            Call::BlobsCreate { .. } => "blobs.create",
            Call::BlobsWrite { .. } => "blobs.write",
            Call::BlobsGet { .. } => "blobs.get",
            Call::BlobsInfo { .. } => "blobs.info",
            Call::BlobsDelete { .. } => "blobs.delete",
            Call::EventsPublish { .. } => "events.publish",
            Call::EventsSubscribe { .. } => "events.subscribe",
            Call::EventsUnsubscribe { .. } => "events.unsubscribe",
        }
    }

    pub fn interface(&self) -> auth::Interface {
        use auth::Interface;
        match self {
            Call::SystemStatus
            | Call::SystemSubscribe
            | Call::SystemUnsubscribe { .. }
            | Call::SystemLogs { .. }
            | Call::SystemProfile
            | Call::SystemShutdown => Interface::System,
            Call::AppsGrant { .. } => Interface::Apps,
            Call::ModelsList
            | Call::ModelsInfo { .. }
            | Call::ModelsStream(_)
            | Call::ModelsCancel { .. }
            | Call::ModelsEmbed { .. } => Interface::Models,
            Call::BlobsPut { .. }
            | Call::BlobsCreate { .. }
            | Call::BlobsWrite { .. }
            | Call::BlobsGet { .. }
            | Call::BlobsInfo { .. }
            | Call::BlobsDelete { .. } => Interface::Blobs,
            Call::EventsPublish { .. }
            | Call::EventsSubscribe { .. }
            | Call::EventsUnsubscribe { .. } => Interface::Events,
        }
    }
}

/// What the engine sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Answer to `hello`.
    Welcome { version: u32, engine: String },
    Response {
        id: u64,
        #[serde(flatten)]
        outcome: Outcome,
    },
    Event {
        subscription: u64,
        #[serde(flatten)]
        event: Event,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Result(Reply),
    Error(WireError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reply {
    Status(SystemStatus),
    Subscribed {
        subscription: u64,
        snapshot: SystemStatus,
    },
    Unsubscribed {},
    Logs {
        service: String,
        lines: Vec<String>,
    },
    Profile(HardwareProfile),
    ShuttingDown {},
    /// Answer to `apps.grant`.
    Granted {
        app: String,
        token: Token,
    },
    /// Answer to `models.list`.
    Models(Vec<ModelInfo>),
    /// Answer to `models.info`.
    Model(ModelDetail),
    /// Answer to `models.stream`: chunks follow on `subscription`. `model`
    /// is the model the engine chose.
    Stream {
        subscription: u64,
        model: String,
    },
    /// Answer to `models.cancel`.
    Cancelled {},
    /// Answer to `models.embed`: one vector per input, in order.
    Embeddings {
        model: String,
        vectors: Vec<Vec<f32>>,
    },
    /// Answer to `blobs.put`, `blobs.create`, `blobs.write` and
    /// `blobs.info`.
    Blob(BlobInfo),
    /// Answer to `blobs.get`: `data` starts at `offset`.
    BlobData {
        info: BlobInfo,
        offset: u64,
        data: Bytes,
    },
    /// Answer to `blobs.delete`.
    Deleted {},
    /// Answer to `events.publish`.
    Published {},
    /// Answer to `events.subscribe`: `app_event`s follow on `subscription`.
    Listening {
        subscription: u64,
    },
}

impl Reply {
    /// The subscription this reply opened, if it opened one. Events for it
    /// follow on the same connection.
    pub fn opened_subscription(&self) -> Option<u64> {
        match self {
            Reply::Subscribed { subscription, .. }
            | Reply::Stream { subscription, .. }
            | Reply::Listening { subscription } => Some(*subscription),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Event {
    /// One service changed.
    ServiceStatus(ServiceStatus),
    /// Part of a model stream.
    ModelChunk(ModelChunk),
    /// A message published to an `events` subscription's scope.
    AppEvent(AppEvent),
}

impl Event {
    /// Whether this is the last event of its subscription.
    pub fn is_final(&self) -> bool {
        matches!(self, Event::ModelChunk(chunk) if chunk.stop.is_some())
    }
}

/// A typed error: a stable code to branch on and a message a person can
/// read. Interface-specific detail rides in optional fields, so an older
/// client simply ignores detail it does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    pub code: ErrorCode,
    pub message: String,
    /// With `insufficient_memory`: what the memory gate measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryRefusal>,
}

impl WireError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            memory: None,
        }
    }

    /// The memory gate's refusal, with its numbers.
    pub fn memory(refusal: MemoryRefusal) -> Self {
        Self {
            code: ErrorCode::InsufficientMemory,
            message: refusal.reason.clone(),
            memory: Some(refusal),
        }
    }
}

/// Stable error codes. A code this build does not know decodes as
/// `Unrecognized` instead of failing the whole message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCode {
    /// No protocol version in common.
    UnsupportedVersion,
    /// A request arrived before `hello`.
    HelloRequired,
    /// The frame was not a valid message.
    BadRequest,
    UnknownService,
    UnknownSubscription,
    Internal,
    /// The engine does not implement this method (yet).
    UnknownMethod,
    /// The token was never issued, or was revoked.
    UnknownToken,
    /// The token's manifest does not allow this call.
    CapabilityDenied,
    /// The manifest broke a rule in [`Manifest::validate`].
    InvalidManifest,
    /// No model with that id, or no default model of the kind asked for.
    UnknownModel,
    /// The model cannot do this (embeddings from a chat-only model, say).
    Unsupported,
    /// The memory gate refused to load the model; see `WireError::memory`.
    InsufficientMemory,
    /// The model's worker is degraded or crashed.
    ModelUnavailable,
    /// Remote models are switched off by the user.
    RemoteDisabled,
    /// No blob with that handle for this caller.
    UnknownBlob,
    /// Over the manifest's `max_bytes` or the engine's own limit.
    TooLarge,
    /// The blob is still being written.
    Incomplete,
    /// A code from a newer engine.
    #[serde(other)]
    Unrecognized,
}

/// Pick the version to speak: the highest one both sides support.
pub fn negotiate(client_versions: &[u32], engine_versions: &[u32]) -> Option<u32> {
    client_versions
        .iter()
        .copied()
        .filter(|v| engine_versions.contains(v))
        .max()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_apps;
