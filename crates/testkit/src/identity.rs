// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `identity` interface of the in-memory engine: a scripted ID, made on
//! the first `identity.get`, and a refusal a test can switch on.

use isonomic_proto::identity::Identity;
use isonomic_proto::{ErrorCode, Reply, WireError};

use crate::State;

/// The ID the test engine makes unless a test sets another.
pub const DEFAULT_ID: &str = "isonomic:test-person-0001";

pub(crate) struct IdentityState {
    id: String,
    made: bool,
    refusal: Option<String>,
}

impl Default for IdentityState {
    fn default() -> Self {
        Self {
            id: DEFAULT_ID.into(),
            made: false,
            refusal: None,
        }
    }
}

/// Answer `identity.get`: `created` is true only on the call that makes it.
pub(crate) fn answer(state: &State) -> Result<Reply, WireError> {
    let mut identity = state.identity.lock().unwrap();
    if let Some(message) = &identity.refusal {
        return Err(WireError::new(
            ErrorCode::IdentityUnavailable,
            message.clone(),
        ));
    }
    let created = !identity.made;
    identity.made = true;
    Ok(Reply::Identity(Identity {
        id: identity.id.clone(),
        created,
    }))
}

/// Scripting handle for the person's ID. From
/// [`TestEngine::identity`](crate::TestEngine::identity).
#[derive(Clone)]
pub struct FakeIdentity {
    pub(crate) state: std::sync::Arc<State>,
}

impl FakeIdentity {
    /// The ID the engine makes (or has made).
    pub fn set_id(&self, id: &str) {
        self.state.identity.lock().unwrap().id = id.into();
    }

    /// Start again as on a first run: the next `identity.get` makes the ID
    /// and answers `created: true`.
    pub fn forget(&self) {
        self.state.identity.lock().unwrap().made = false;
    }

    /// Every `identity.get` answers `identity_unavailable` with this plain
    /// message, as when the key store refuses, until `recover`.
    pub fn refuse(&self, message: &str) {
        self.state.identity.lock().unwrap().refusal = Some(message.into());
    }

    pub fn recover(&self) {
        self.state.identity.lock().unwrap().refusal = None;
    }

    /// Whether the ID has been made.
    pub fn made(&self) -> bool {
        self.state.identity.lock().unwrap().made
    }
}
