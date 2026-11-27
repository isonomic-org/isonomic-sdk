// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `connections` interface: tool servers the person connects. Shell
//! only; an app's client has no way to reach it.

use std::collections::BTreeMap;

use isonomic_proto::connections::{
    AddConnection, Connection, ConnectionTool, ListConnectionTools, ListConnections,
    RemoveConnection, SignIn, SignInStarted,
};
use isonomic_proto::{Call, Reply};
use tokio::sync::watch;

use crate::{Client, ClientError};

/// The newest state the engine announced for each connection, by id, since
/// this client connected. Cleared when the connection to the engine drops.
pub type ConnectionStates = BTreeMap<String, Connection>;

/// Connection calls, made as the shell.
#[derive(Clone)]
pub struct Connections {
    client: Client,
}

impl Connections {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    /// Every tool server the person connected.
    pub async fn list(&self) -> Result<Vec<Connection>, ClientError> {
        let call = Call::ConnectionsList(ListConnections {});
        match self.client.call(call).await? {
            Reply::Connections(connections) => Ok(connections),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Connect a tool server. A remote one without a secret usually comes
    /// back `needs_sign_in`: call [`sign_in`](Self::sign_in). The secret's
    /// `Debug` is redacted; never log the call as JSON.
    pub async fn add(&self, add: AddConnection) -> Result<Connection, ClientError> {
        match self.client.call(Call::ConnectionsAdd(add)).await? {
            Reply::Connection(connection) => Ok(connection),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Start a server's browser sign-in: open the page the reply names. The
    /// outcome arrives in [`changes`](Self::changes). `sign_in_failed`
    /// carries plain words to show as given.
    pub async fn sign_in(&self, id: &str) -> Result<SignInStarted, ClientError> {
        let call = Call::ConnectionsSignIn(SignIn { id: id.to_string() });
        match self.client.call(call).await? {
            Reply::SignIn(started) => Ok(started),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Disconnect a tool server; the engine forgets its sign-in. Once the
    /// engine answers, the id leaves [`changes`](Self::changes) too, since
    /// no event says a connection is gone.
    pub async fn remove(&self, id: &str) -> Result<(), ClientError> {
        let call = Call::ConnectionsRemove(RemoveConnection { id: id.to_string() });
        match self.client.call(call).await? {
            Reply::Done {} => {
                self.client
                    .connections_tx
                    .send_if_modified(|states| states.remove(id).is_some());
                Ok(())
            }
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// The tools a connection offers.
    pub async fn tools(&self, id: &str) -> Result<Vec<ConnectionTool>, ClientError> {
        let call = Call::ConnectionsTools(ListConnectionTools { id: id.to_string() });
        match self.client.call(call).await? {
            Reply::ConnectionTools(tools) => Ok(tools),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Every `connection` event, folded by id. Await `changed()`; never
    /// poll. Empty after a reconnect: call [`list`](Self::list) then.
    pub fn changes(&self) -> watch::Receiver<ConnectionStates> {
        self.client.connections_live.clone()
    }
}
