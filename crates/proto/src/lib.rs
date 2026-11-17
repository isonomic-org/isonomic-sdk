// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

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

pub mod agents;
pub mod auth;
pub mod blobs;
mod calls;
mod error;
pub mod events;
pub mod frame;
pub mod identity;
#[cfg(feature = "tokio")]
pub mod io;
mod json;
pub mod lenient;
pub mod locate;
pub mod manifest;
pub mod models;
mod reply;
pub mod sql;
pub mod system;

use serde::{Deserialize, Serialize};

pub use blobs::{BlobInfo, Bytes};
pub use error::{ErrorCode, WireError};
pub use events::AppEvent;
pub use json::Json;
pub use manifest::{Capabilities, Manifest, Token};
pub use models::{
    MemoryRefusal, Message, ModelChunk, ModelDetail, ModelInfo, StopReason, StreamRequest,
};
pub use reply::{Event, Reply};
pub use sql::{Rows, SqlValue};
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
    /// Start a degraded service again, clearing its restart window. The
    /// outcome arrives as `service_status` events.
    #[serde(rename = "system.retry")]
    SystemRetry { service: String },
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
    /// Shell only: fetch a local model's file, with progress events.
    #[serde(rename = "models.download")]
    ModelsDownload(models::DownloadModel),
    /// Shell only: stop a model's download, for every subscriber.
    #[serde(rename = "models.download_cancel")]
    ModelsDownloadCancel(models::DownloadModel),
    /// Shell only: the local model to start with on this machine.
    #[serde(rename = "models.recommended")]
    ModelsRecommended(models::RecommendModel),
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
    /// Run a query in `schema` (the app's first own schema when absent).
    #[serde(rename = "sql.query")]
    SqlQuery {
        sql: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        params: Vec<SqlValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<String>,
    },
    #[serde(rename = "sql.execute")]
    SqlExecute {
        sql: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        params: Vec<SqlValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<String>,
    },
    /// Run every statement in one transaction: all or nothing.
    #[serde(rename = "sql.batch")]
    SqlBatch {
        statements: Vec<sql::Statement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<String>,
    },
    /// A live query: the result now, then a `rows_changed` event with the
    /// whole new result whenever it changes.
    #[serde(rename = "sql.subscribe")]
    SqlSubscribe {
        sql: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        params: Vec<SqlValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<String>,
    },
    #[serde(rename = "sql.unsubscribe")]
    SqlUnsubscribe { subscription: u64 },
    // The agents interface: agent sessions run by the engine's host.
    #[serde(rename = "sessions.create")]
    SessionsCreate(agents::CreateSession),
    #[serde(rename = "sessions.list")]
    SessionsList(agents::ListSessions),
    #[serde(rename = "sessions.open")]
    SessionsOpen(agents::SessionRef),
    #[serde(rename = "sessions.fork")]
    SessionsFork(agents::ForkSession),
    #[serde(rename = "sessions.archive")]
    SessionsArchive(agents::ArchiveSession),
    #[serde(rename = "sessions.rename")]
    SessionsRename(agents::RenameSession),
    #[serde(rename = "sessions.send")]
    SessionsSend(agents::SendInput),
    #[serde(rename = "sessions.continue")]
    SessionsContinue(agents::SessionRef),
    #[serde(rename = "sessions.cancel")]
    SessionsCancel(agents::SessionRef),
    /// Delete every session of the calling app, and what they own.
    #[serde(rename = "sessions.purge")]
    SessionsPurge(agents::PurgeSessions),
    #[serde(rename = "sessions.subscribe")]
    SessionsSubscribe(agents::SubscribeSession),
    #[serde(rename = "sessions.unsubscribe")]
    SessionsUnsubscribe { subscription: u64 },
    #[serde(rename = "approvals.list")]
    ApprovalsList(agents::ListApprovals),
    #[serde(rename = "approvals.decide")]
    ApprovalsDecide(agents::DecideApproval),
    #[serde(rename = "questions.answer")]
    QuestionsAnswer(agents::AnswerQuestion),
    #[serde(rename = "tools.register_client")]
    ToolsRegisterClient(agents::RegisterClientTools),
    #[serde(rename = "tools.result")]
    ToolsResult(agents::ClientToolResult),
    #[serde(rename = "presets.list")]
    PresetsList(agents::ListPresets),
    #[serde(rename = "presets.get")]
    PresetsGet(agents::GetPreset),
    #[serde(rename = "presets.put")]
    PresetsPut(agents::PutPreset),
    #[serde(rename = "artifacts.get")]
    ArtifactsGet(agents::GetArtifact),
    #[serde(rename = "artifacts.list")]
    ArtifactsList(agents::SessionRef),
    /// Shell only: the person's Isonomic ID, made on first need.
    #[serde(rename = "identity.get")]
    IdentityGet(identity::GetIdentity),
    /// Shell only: the user-action token for a person's decision.
    #[serde(rename = "approvals.user_action")]
    ApprovalsUserAction(agents::MintUserAction),
}

/// What the engine sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Answer to `hello`.
    Welcome {
        version: u32,
        engine: String,
        /// The agent session event schema the engine writes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<agents::SchemaVersion>,
    },
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
