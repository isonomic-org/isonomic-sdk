// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Typed errors: a stable code and a readable message.

use serde::{Deserialize, Serialize};

use crate::MemoryRefusal;

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
    /// `system.retry` on a service that is not degraded.
    NotDegraded,
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
    /// The database refused the statement; the message says why.
    SqlError,
    /// No such session, approval, question, preset or artifact for this
    /// caller; another app's ids answer this too.
    NotFound,
    /// The preset did not validate; the message lists why.
    InvalidPreset,
    /// The host runs interactive sessions only, for now.
    ClassNotSupported,
    /// A fork point that is not a turn boundary.
    NotAtTurnBoundary,
    /// There is nothing to continue, or the model cannot continue it.
    ContinueRefused,
    /// Views and third-party code may not decide approvals.
    NotAllowedToDecide,
    /// Deciding an approval needs a user-action token.
    MissingUserAction,
    AlreadyDecided,
    /// The session failed and takes no more input.
    SessionFailed,
    /// Client tool specs must be client tools with free names.
    InvalidTools,
    /// `models.download_cancel` for a model with no download running.
    NotDownloading,
    /// The engine could not make or read the person's ID (the key store
    /// refused, say). The message is plain words for the window to show.
    IdentityUnavailable,
    /// A recovery kit call the engine refused: an export the person did
    /// not ask for, words that are not the kit's, a damaged or spent kit,
    /// or a computer whose ID is already in use. The message is plain words for
    /// the window to show; nothing was changed.
    RecoveryRefused,
    /// `models.remove` for a local model whose file is not on this
    /// machine.
    NotInstalled,
    /// `models.remove` while the model's download runs: cancel it first.
    DownloadRunning,
    /// No connection with that id.
    ConnectionNotFound,
    /// The engine could not start a server's sign-in (a server with no
    /// browser sign-in, or one that did not answer). The message is plain
    /// words for the window to show.
    SignInFailed,
    /// A code from a newer engine.
    #[serde(other)]
    Unrecognized,
}
