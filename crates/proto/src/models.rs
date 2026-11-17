// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Data carried by the `models` interface.
//!
//! These mirror the model types of the inference layer, seen from an app:
//! a message list in, a stream of chunks out. The chunk carries text, an
//! optional reasoning channel (decision D4), tool-call deltas, usage and a
//! stop reason. Where the inference layer says `delta` and
//! `finish_reason`, this says `text` and `stop`.
//!
//! JSON Schemas and tool arguments travel as JSON text, not nested objects,
//! so every type stays plain enough to describe in WIT later.

use serde::{Deserialize, Serialize};

use crate::WireError;

fn is_false(value: &bool) -> bool {
    !*value
}

/// A model the caller may use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Stable identifier, used in every call.
    pub id: String,
    /// Name shown to a person.
    pub name: String,
    pub context_length: Option<u32>,
    /// `chat`, `embeddings`, and later others. Strings, so a newer engine's
    /// capabilities never fail to decode.
    pub capabilities: Vec<String>,
    /// Runs at a remote provider rather than on this machine.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remote: bool,
    /// The model the engine recommends for this machine's profile, so an
    /// app can show it first. At most one model has it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub recommended: bool,
}

impl ModelInfo {
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

/// Where a model's worker stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
    /// Not loaded. The first request loads it, if the memory gate admits.
    Stopped,
    Starting,
    Ready,
    /// Crashed too often; the engine stopped trying.
    Degraded,
    /// A local model whose file is not on this machine yet:
    /// `models.download` fetches it.
    NotInstalled,
    /// Its file is being fetched; `models.download` attaches to it.
    Downloading,
    #[serde(other)]
    Unrecognized,
}

/// Everything `models.info` knows about one model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDetail {
    #[serde(flatten)]
    pub info: ModelInfo,
    /// Size of the model file. `None` for remote models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// License name, where one is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    pub state: ModelState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// One message of a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Tools the assistant called in this message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For a `tool` message: the call this is the result of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    fn plain(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::plain(Role::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::plain(Role::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::plain(Role::Assistant, content)
    }

    pub fn tool_result(call: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: Some(call.into()),
            ..Self::plain(Role::Tool, content)
        }
    }
}

/// A tool the model may call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments, as JSON text.
    pub schema: String,
}

/// A complete tool call, as sent back in an assistant message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// The arguments, as JSON text.
    pub arguments: String,
}

/// Params of `models.stream`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StreamRequest {
    /// `None` asks for the user's default chat model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Ask the model to reason before it answers, where it can. `None`
    /// leaves it to the engine, which keeps it off on slow machines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

/// Part of one tool call, streamed. Deltas with the same `index` belong to
/// the same call: the first carries `id` and `name`, the rest append to
/// `arguments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub arguments: String,
}

/// Tokens used, and the cost the engine metered for this app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    /// In millionths of the platform's stable-dollar unit. 0 for local
    /// models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_micros: Option<u64>,
}

/// Why a stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The model finished its answer.
    End,
    /// The `max_tokens` limit was reached.
    MaxTokens,
    /// The model wants the caller to run the tools it called.
    ToolCalls,
    /// The caller cancelled, or the user's remote toggle cut the stream.
    Cancelled,
    /// The model failed mid-stream; the chunk's `error` says why.
    Error,
    #[serde(other)]
    Unrecognized,
}

/// One event of a model stream. Every field is optional; the last chunk
/// has `stop` set and ends the subscription.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChunk {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopReason>,
    /// With `stop: error`: what went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}

impl ModelChunk {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    pub fn reasoning(text: impl Into<String>) -> Self {
        Self {
            reasoning: Some(text.into()),
            ..Self::default()
        }
    }

    pub fn stop(reason: StopReason) -> Self {
        Self {
            stop: Some(reason),
            ..Self::default()
        }
    }
}

/// Why the memory gate refused to load a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRefusal {
    /// Bytes the load needs, including the gate's headroom.
    pub required_bytes: u64,
    /// Bytes available when the gate decided.
    pub available_bytes: u64,
    /// Readable reason, with what to try instead.
    pub reason: String,
}

/// Params of `models.download`: fetch a local model's file. The engine owns
/// the download; calling again while it runs attaches to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadModel {
    pub model: String,
}

/// How a download ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadOutcome {
    /// The file is in place; the model is `stopped`, ready to load.
    Installed,
    /// The download failed; the event's `error` says why.
    Failed,
    /// `models.download_cancel`.
    Cancelled,
    #[serde(other)]
    Unrecognized,
}

/// One event of a download. The last one has `outcome` set and ends the
/// subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model: String,
    pub received_bytes: u64,
    /// Absent when the engine does not know the size yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<DownloadOutcome>,
    /// With `outcome: failed`: what went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}

/// Params of `models.recommended`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendModel {
    /// Ask for another profile than this machine's (`low`, `standard`,
    /// `high`), to show what a profile would get.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// The local model to start with on a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recommendation {
    pub model: String,
    /// The profile it was chosen for.
    pub profile: String,
    /// Its file size, to show before the download starts; absent when
    /// unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}
