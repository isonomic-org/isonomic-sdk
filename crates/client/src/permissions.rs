// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `permissions` interface: linked folders, kit switches and remembered
//! approvals. The window grants and revokes; an app reads its own and asks
//! the window to put a question to the person.

use isonomic_proto::permissions::{
    Asked, GrantPermission, ListPermissions, Permissions as Held, RequestPermission, Revocable,
    RevokePermission,
};
use isonomic_proto::{Call, Reply, Token};

use crate::{Client, ClientError};

/// Permission calls, made as the shell or under an app's token.
#[derive(Clone)]
pub struct Permissions {
    client: Client,
    token: Option<Token>,
}

impl Permissions {
    pub(crate) fn new(client: Client, token: Option<Token>) -> Self {
        Self { client, token }
    }

    async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.client.send(self.token.clone(), call, None).await
    }

    async fn held(&self, call: Call) -> Result<Held, ClientError> {
        match self.call(call).await? {
            Reply::Permissions(held) => Ok(*held),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    async fn done(&self, call: Call) -> Result<(), ClientError> {
        match self.call(call).await? {
            Reply::Done {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Shell only: link a folder to an app or turn a kit on for it, after
    /// the person acted. Answers everything the app then holds. A folder
    /// that can never be linked is `not_linkable`; a kit the app's manifest
    /// does not name is `capability_denied`.
    pub async fn grant(&self, grant: GrantPermission) -> Result<Held, ClientError> {
        self.held(Call::PermissionsGrant(grant)).await
    }

    /// What an app holds: its own with `None` under an app's token; the
    /// shell names the app.
    pub async fn list(&self, app: Option<&str>) -> Result<Held, ClientError> {
        let app = app.map(Into::into);
        self.held(Call::PermissionsList(ListPermissions { app }))
            .await
    }

    /// Shell only: end one permission of `app`. The next check refuses.
    /// Revoking a kit also records a no to its first question.
    pub async fn revoke(&self, app: &str, permission: Revocable) -> Result<(), ClientError> {
        let app = app.into();
        self.done(Call::PermissionsRevoke(RevokePermission {
            app,
            permission,
        }))
        .await
    }

    /// An app asks the window to show the folder picker or a kit's first
    /// question. `user_action` is the person's token; without it the engine
    /// refuses with `missing_user_action` and nothing is shown.
    pub async fn request(&self, ask: Asked, user_action: Option<&str>) -> Result<(), ClientError> {
        let user_action = user_action.map(str::to_string);
        self.done(Call::PermissionsRequest(RequestPermission {
            ask,
            user_action,
        }))
        .await
    }
}
