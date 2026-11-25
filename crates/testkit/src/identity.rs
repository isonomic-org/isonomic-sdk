// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `identity` interface of the in-memory engine: a scripted ID, made on
//! the first `identity.get`, a refusal a test can switch on, and the
//! recovery kit (`crate::recovery`).

use isonomic_proto::identity::Identity;
use isonomic_proto::{Call, ErrorCode, Reply, WireError};

use crate::recovery::{self, KitState};
use crate::State;

/// The ID the test engine makes unless a test sets another.
pub const DEFAULT_ID: &str = "isonomic:test-person-0001";

pub(crate) struct IdentityState {
    pub(crate) id: String,
    pub(crate) made: bool,
    refusal: Option<String>,
    pub(crate) kit: KitState,
}

impl Default for IdentityState {
    fn default() -> Self {
        Self {
            id: DEFAULT_ID.into(),
            made: false,
            refusal: None,
            kit: KitState::default(),
        }
    }
}

/// Answer an `identity.*` call. `identity.get`'s `created` is true only on
/// the call that makes the ID.
pub(crate) fn answer(state: &State, call: Call) -> Result<Reply, WireError> {
    let mut identity = state.identity.lock().unwrap();
    if let Some(message) = &identity.refusal {
        return Err(WireError::new(
            ErrorCode::IdentityUnavailable,
            message.clone(),
        ));
    }
    match call {
        Call::IdentityKitExport(export) => {
            return recovery::export(&mut identity, export.person_asked)
        }
        Call::IdentityKitRestore(restore) => return recovery::restore(&mut identity, restore),
        Call::IdentityKitSaved(_) => return recovery::saved(&mut identity),
        Call::IdentityKitStatus(_) => return Ok(recovery::status(&identity)),
        _ => {}
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
        let mut identity = self.state.identity.lock().unwrap();
        identity.made = false;
        identity.kit = KitState::default();
    }

    /// The kit's clock, in Unix seconds: what `made`, `saved_at` and
    /// `exported_at` read. [`DEFAULT_KIT_NOW`](crate::DEFAULT_KIT_NOW) by
    /// default.
    pub fn set_now(&self, unix_secs: u64) {
        self.state.identity.lock().unwrap().kit.now = unix_secs;
    }

    /// Whether this computer's ID has apps and chats: then `kit_restore` is
    /// refused even with `replace_unused`.
    pub fn set_in_use(&self, in_use: bool) {
        self.state.identity.lock().unwrap().kit.in_use = in_use;
    }

    /// Which kit is current: 0 until the first restore, one more on each.
    /// Its words are [`kit_words`](crate::kit_words) of this.
    pub fn kit_generation(&self) -> u32 {
        self.state.identity.lock().unwrap().kit.generation
    }

    /// Every `identity.*` call answers `identity_unavailable` with this plain
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
