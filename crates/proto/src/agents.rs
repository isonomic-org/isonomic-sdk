// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data carried by the agents interface: agent sessions run by the engine's
//! host, one to one with its calls (isonomic-harness `docs/host-api.md`).
//!
//! The session event types are harness-core's, re-exported as [`schema`] at
//! the commit the workspace pins, so apps import only the SDK. Arguments
//! and results are the SDK's own: the host's types are not wire types.

use serde::{Deserialize, Serialize};

use crate::Bytes;

/// harness-core, the session event schema: events, blocks, ids and kinds.
pub use harness_core as schema;
pub use harness_core::{
    AppId, ApprovalId, ApprovalRequest, ArtifactEvent, ArtifactId, Block, CallId, ErrorInfo,
    ForkReason, LiveDelta, PresetId, PresetRef, QuestionId, Seq, SessionClass, SessionEvent,
    SessionId, Status, ToolKind, ToolSpec, TurnId,
};

/// Display widgets: tools the model calls to show a component inline. The
/// host validates and records them; apps render them and never answer them
/// with `tools.result`. One source of truth: harness-core's specs.
pub mod display {
    pub use harness_core::display::{
        display_capable, display_spec, display_tool_spec, validate, DisplayError, DisplaySpec,
        DISPLAY_SPECS, QUOTE_MAX, SMALLEST_MODEL,
    };
    pub use harness_core::DisplayOutcome;
}

/// Tool kits: what a kit call touches, what an approval may remember, the
/// table that decides what a session is offered, and the kit tool specs
/// with their error codes. One source of truth: harness-core's types, as
/// for [`display`]. The approval fields (`scope`, `preview`, `reversible`,
/// `remember`) ride on [`ApprovalRequest`].
pub mod kits {
    pub use harness_core::access::{
        grant_effect, Access, GrantAnswer, HeldGrant, PreviewItem, Scope,
    };
    pub use harness_core::kits::{files_errors, kit_spec, KitField, KitSpec, FILES_SPECS};
    pub use harness_core::offer::{
        admit, offer, KitCandidate, KitMember, Offer, KITS, MAX_DISPLAY_TOOLS,
    };
    pub use harness_core::{ApprovalRule, Effect, Remember, ScopeKind};
}

/// The session event schema an engine writes. A client reading a newer
/// revision of the same version renders unknown events generically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub version: u32,
    pub revision: u32,
}

impl SchemaVersion {
    /// The schema this build reads.
    pub const CURRENT: SchemaVersion = SchemaVersion {
        version: harness_core::SCHEMA_VERSION,
        revision: harness_core::SCHEMA_REVISION,
    };

    /// Whether this build can read events written under `self`.
    pub fn readable(&self) -> bool {
        self.version <= Self::CURRENT.version
    }
}

fn no(b: &bool) -> bool {
    !*b
}

fn zero(n: &u64) -> bool {
    *n == 0
}

fn zero32(n: &u32) -> bool {
    *n == 0
}

/// The longest model name `sessions.create` takes, in characters, once
/// trimmed.
pub const MAX_MODEL_NAME: usize = 128;

/// The host's own bound on a model name, in bytes.
pub const MAX_MODEL_NAME_BYTES: usize = 200;

/// The model a `sessions.create` names, trimmed. Blank, longer than
/// [`MAX_MODEL_NAME`] characters or [`MAX_MODEL_NAME_BYTES`] bytes, or
/// holding a control character is `bad_request`. The engine, `proto::auth`
/// and the testkit all check names here, so a name that passes is one the
/// host records.
pub fn model_name(name: &str) -> Result<&str, crate::WireError> {
    let name = name.trim();
    let bad = |why: &str| crate::WireError::new(crate::ErrorCode::BadRequest, why);
    if name.is_empty() {
        return Err(bad("the model name is blank"));
    }
    if name.chars().count() > MAX_MODEL_NAME || name.len() > MAX_MODEL_NAME_BYTES {
        return Err(bad("the model name is too long"));
    }
    if name.chars().any(char::is_control) {
        return Err(bad("the model name holds a control character"));
    }
    Ok(name)
}

/// `sessions.create`: the class comes from the preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateSession {
    pub preset: PresetId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The person opted in to sharing a location with this session.
    #[serde(default, skip_serializing_if = "no")]
    pub location_opt_in: bool,
    /// Run on this model in place of the preset's. An app may name only a
    /// model its `models` capability allows, as for `models.stream`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `sessions.list`. Every field narrows; none means all of the caller's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListSessions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetId>,
    /// Title contains this, ignoring case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Page size; 0 means 50.
    #[serde(default, skip_serializing_if = "zero32")]
    pub limit: u32,
}

/// `sessions.purge`: one app's sessions and what they own. An app's `{}`
/// means its own; the shell must name the app (never "everything").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeSessions {
    /// Whose sessions. Only the shell names another app, as for events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppId>,
}

/// `approvals.list`: every approval the caller may see, or one app's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListApprovals {
    /// One app's approvals. Only the shell names another app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppId>,
}

/// `presets.list`. No filter yet; a struct so one can be added.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListPresets {}

/// One session, for `sessions.open`, `continue` and `cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRef {
    pub session: SessionId,
}

/// Where a fork starts: at a turn boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForkPoint {
    BeforeTurn(TurnId),
    AfterTurn(TurnId),
}

/// `sessions.fork`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkSession {
    pub session: SessionId,
    pub at: ForkPoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ForkReason>,
}

/// `sessions.archive`: `archived: false` restores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveSession {
    pub session: SessionId,
    pub archived: bool,
}

/// `sessions.rename`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameSession {
    pub session: SessionId,
    pub title: String,
}

/// `sessions.send`: text, attachments (blobs the app owns) and mentions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendInput {
    pub session: SessionId,
    pub content: Vec<Block>,
    /// The app's own reference, echoed in the audit record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_ref: Option<String>,
}

/// `sessions.subscribe`: durable events after `from_seq`, then live ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscribeSession {
    pub session: SessionId,
    /// The last sequence number seen; 0 for everything.
    #[serde(default, skip_serializing_if = "zero")]
    pub from_seq: Seq,
}

/// How the person decided an approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionInput {
    Allow,
    Deny,
    /// Words; anything but a clear yes is a denial.
    Text(String),
}

/// `approvals.decide`. Needs a user-action token from the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecideApproval {
    /// Ids are minted per session, so the session names the target too.
    pub session: SessionId,
    pub approval: ApprovalId,
    pub decision: DecisionInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_action: Option<String>,
    /// How long to remember an allow: `session` or `always`. The engine
    /// holds it to what the request's `remember` offers, and keeps it only
    /// for an allow that carried a verified user-action token. Absent means
    /// once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remember: Option<kits::Remember>,
}

/// `questions.answer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerQuestion {
    pub session: SessionId,
    pub question: QuestionId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "no")]
    pub skipped: bool,
}

/// `tools.register_client`: replaces the app's client tools for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterClientTools {
    pub session: SessionId,
    pub specs: Vec<ToolSpec>,
}

/// `tools.result`: what a client tool returned. `error` means it failed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientToolResult {
    pub session: SessionId,
    pub call: CallId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

/// `presets.get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetPreset {
    pub preset: PresetId,
}

/// `presets.put`: a preset's TOML text, stored as a new version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PutPreset {
    pub text: String,
}

/// `artifacts.get`: the latest version unless one is named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetArtifact {
    pub artifact: ArtifactId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// Where to start reading the body. One reply carries at most
    /// `blobs::CHUNK_BYTES`, as for `blobs.get`.
    #[serde(default, skip_serializing_if = "zero")]
    pub offset: u64,
    /// Read fewer bytes than a full chunk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<u64>,
}

/// A fork's parent and the sequence number it forked at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkedFrom {
    pub session: SessionId,
    pub seq: Seq,
}

/// A session as listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub app: AppId,
    pub class: SessionClass,
    pub preset: PresetRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Active, awaiting client or failed.
    pub status: Status,
    #[serde(default, skip_serializing_if = "no")]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkedFrom>,
    /// The model picked at creation; absent when the preset's model runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_seq: Seq,
}

/// `sessions.open`: the summary and what is pending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub summary: SessionSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_open: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<ApprovalId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<QuestionId>,
    /// Client calls waiting for their app to come back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub awaiting_client: Vec<CallId>,
}

/// An approval waiting for a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    pub session: SessionId,
    /// The session's owner app.
    pub app: AppId,
    pub request: ApprovalRequest,
}

/// One finding about a preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// `error` (the preset cannot be used) or `warning`.
    pub severity: String,
    /// A dotted key path, or `line:column`.
    pub at: String,
    pub message: String,
}

/// A preset as listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetSummary {
    pub reference: PresetRef,
    /// The owner app; absent for built-ins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<AppId>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

/// A preset as stored: its TOML text and what validation found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetDetail {
    pub reference: PresetRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<AppId>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

/// An artifact version: its `artifact` event and its body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactVersion {
    pub meta: ArtifactEvent,
    /// The body from `offset`, at most `blobs::CHUNK_BYTES`; `meta.bytes`
    /// is its whole size.
    pub bytes: Bytes,
    #[serde(default, skip_serializing_if = "zero")]
    pub offset: u64,
}

/// `approvals.user_action`, shell only: the window mints the token when the
/// person acts on an approval, and hands it to the app that will decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintUserAction {
    pub session: SessionId,
    pub approval: ApprovalId,
    /// The app that will call `approvals.decide`; absent for the shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppId>,
}

/// Why a session stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// The session was purged.
    Purged,
    #[serde(other)]
    Unrecognized,
}

/// The last event of a session stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionEnded {
    pub session: SessionId,
    pub reason: EndReason,
}

/// What `sessions.purge` deleted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeReport {
    pub sessions: u64,
    pub events: u64,
    pub blobs: u64,
}
