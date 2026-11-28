// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The person's token for an approval: an app asks the window for one when
//! the person presses Allow or Deny in the app's own view, and passes it to
//! `approvals.decide`.
//!
//! Only the window may mint a token (`approvals.user_action` is shell
//! only on the wire). The window owns the policy through
//! [`UserActionSource`]: it answers only while the asking app's tab is on
//! screen and focused and the person's click or key press is being
//! handled. That keeps model output from obtaining one on its own; the app
//! keeps its side by calling it only from its own Allow and Deny controls,
//! for the approval that control answers. The token is bound to one app,
//! session and approval, and the engine still checks it.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use isonomic_client::{Agents, ClientError};
use isonomic_proto::agents::{ApprovalId, SessionId};
use isonomic_proto::ErrorCode;

/// The window's answer to [`AppContext::user_action`](crate::AppContext::user_action).
pub type UserActionFuture = Pin<Box<dyn Future<Output = Result<String, UserActionError>> + Send>>;

/// Why no token came. Deny-safe either way: without a token the app sends
/// nothing and the approval stays pending.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UserActionError {
    /// The person was not acting in this app just then: its tab was not
    /// focused, or no click or key press was being handled. Ask again from
    /// the person's next click. Code `no_user_input`.
    #[error("no_user_input: the person was not acting in this app")]
    NoUserInput,
    /// The window gives no tokens (an app opened without one). Code
    /// `unavailable`.
    #[error("unavailable: this window gives no user-action tokens")]
    Unavailable,
    /// The engine refused to mint it: the approval is unknown or already
    /// decided, say. `code` is the engine's (`not_found`, ...). Code
    /// `engine`.
    #[error("engine: {code:?}: {message}")]
    Engine { code: ErrorCode, message: String },
    /// The window lost the engine, or the engine answered something else.
    /// Code `disconnected`.
    #[error("disconnected: {0}")]
    Disconnected(String),
}

impl UserActionError {
    /// The stable code to match on, for example to show "Allow again from
    /// this window" for `no_user_input`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoUserInput => "no_user_input",
            Self::Unavailable => "unavailable",
            Self::Engine { .. } => "engine",
            Self::Disconnected(_) => "disconnected",
        }
    }
}

impl From<ClientError> for UserActionError {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::Engine(wire) => Self::Engine {
                code: wire.code,
                message: wire.message,
            },
            other => Self::Disconnected(other.to_string()),
        }
    }
}

/// The window's side: it decides whether the person is acting in `app` now,
/// and if so mints the token for `app` to decide `approval` in `session`.
///
/// The check is made when `user_action` is called, not when the future is
/// polled: it runs inside the app's input handler, and the future may run
/// later.
pub trait UserActionSource: Send + Sync + 'static {
    /// The token for `app` to decide `approval` in `session`, or why not.
    /// Decide here, synchronously; the future only waits for the mint.
    fn user_action(
        &self,
        app: &str,
        session: &SessionId,
        approval: &ApprovalId,
    ) -> UserActionFuture;
}

/// A ready-made source for the window: `acting(app)` says whether the
/// person is acting in `app`'s focused tab right now, and the shell's
/// `agents` interface mints the token (`approvals.user_action` with `app`).
///
/// `acting` should be true for one ask per input event: the window clears
/// its flag once it has answered, so one click mints one token.
pub struct ShellUserActions {
    agents: Agents,
    acting: Box<dyn Fn(&str) -> bool + Send + Sync>,
}

impl ShellUserActions {
    /// `agents` must be the shell's (`Client::agents`), not an app's: the
    /// engine refuses `approvals.user_action` under an app token.
    pub fn new(agents: Agents, acting: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        Self {
            agents,
            acting: Box::new(acting),
        }
    }
}

impl UserActionSource for ShellUserActions {
    fn user_action(
        &self,
        app: &str,
        session: &SessionId,
        approval: &ApprovalId,
    ) -> UserActionFuture {
        if !(self.acting)(app) {
            return Box::pin(async { Err(UserActionError::NoUserInput) });
        }
        let (agents, app) = (self.agents.clone(), app.to_string());
        let (session, approval) = (session.clone(), approval.clone());
        Box::pin(async move {
            let token = agents.user_action(&session, &approval, Some(&app)).await?;
            Ok(token)
        })
    }
}

pub(crate) type Source = Option<Arc<dyn UserActionSource>>;
