// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The client an app is given: every call names the app's token, so the
//! engine checks it against the app's manifest.

use isonomic_proto::{Call, Reply, Token};
use tokio::sync::watch;

use crate::{Agents, Blobs, Client, ClientError, Events, Live, Models, Sql, Subscription};

/// A [`Client`] scoped to one app's capability token. Cheap to clone.
#[derive(Clone)]
pub struct AppClient {
    client: Client,
    token: Token,
}

impl AppClient {
    pub(crate) fn new(client: Client, token: Token) -> Self {
        Self { client, token }
    }

    pub fn token(&self) -> &Token {
        &self.token
    }

    /// Whether the engine is reachable, for an app's own "reconnecting"
    /// state. Await `changed()` on it; never poll.
    pub fn live(&self) -> watch::Receiver<Live> {
        self.client.live()
    }

    /// The `models` interface, under this app's token.
    pub fn models(&self) -> Models {
        Models::new(self.client.clone(), Some(self.token.clone()))
    }

    /// The `blobs` interface, under this app's token.
    pub fn blobs(&self) -> Blobs {
        Blobs::new(self.client.clone(), Some(self.token.clone()))
    }

    /// The `events` interface, scoped to this app.
    pub fn events(&self) -> Events {
        Events::new(self.client.clone(), Some(self.token.clone()))
    }

    /// The agents interface: this app's own sessions, presets and artifacts.
    pub fn agents(&self) -> Agents {
        Agents::new(self.client.clone(), Some(self.token.clone()))
    }

    /// The `sql` interface, in this app's schemas.
    pub fn sql(&self) -> Sql {
        Sql::new(self.client.clone(), Some(self.token.clone()))
    }

    /// Make one call under this app's token.
    pub async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.client.send(Some(self.token.clone()), call, None).await
    }

    /// Make one call under this app's token whose reply opens a
    /// subscription.
    pub async fn subscribe(&self, call: Call) -> Result<(Reply, Subscription), ClientError> {
        self.client.open(Some(self.token.clone()), call).await
    }
}
