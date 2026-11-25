// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `sql` interface: query and change the app's own schemas, and keep a
//! live query whose result the engine pushes whenever it changes.

use isonomic_proto::sql::Statement;
use isonomic_proto::{Call, Event, Reply, Rows, SqlValue, Token};

use crate::{Client, ClientError, Subscription};

/// SQL calls, made as the shell or under an app's token, in one schema.
#[derive(Clone)]
pub struct Sql {
    client: Client,
    token: Option<Token>,
    schema: Option<String>,
}

impl Sql {
    pub(crate) fn new(client: Client, token: Option<Token>) -> Self {
        Self {
            client,
            token,
            schema: None,
        }
    }

    /// The same calls in another schema: one of the app's own, or (for
    /// reads) one its manifest lists. By default the app's first schema.
    pub fn in_schema(&self, schema: &str) -> Self {
        Self {
            schema: Some(schema.to_string()),
            ..self.clone()
        }
    }

    async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.client.send(self.token.clone(), call, None).await
    }

    pub async fn query(&self, sql: &str, params: Vec<SqlValue>) -> Result<Rows, ClientError> {
        let call = Call::SqlQuery {
            sql: sql.to_string(),
            params,
            schema: self.schema.clone(),
        };
        match self.call(call).await? {
            Reply::Rows(rows) => Ok(rows),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Run one statement and return the rows it changed.
    pub async fn execute(&self, sql: &str, params: Vec<SqlValue>) -> Result<u64, ClientError> {
        let call = Call::SqlExecute {
            sql: sql.to_string(),
            params,
            schema: self.schema.clone(),
        };
        match self.call(call).await? {
            Reply::Executed { rows_affected } => Ok(rows_affected),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Run every statement in one transaction; all or nothing.
    pub async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<u64>, ClientError> {
        let call = Call::SqlBatch {
            statements,
            schema: self.schema.clone(),
        };
        match self.call(call).await? {
            Reply::ExecutedBatch { rows_affected } => Ok(rows_affected),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// The result now, kept current by the engine.
    pub async fn live(&self, sql: &str, params: Vec<SqlValue>) -> Result<LiveQuery, ClientError> {
        let call = Call::SqlSubscribe {
            sql: sql.to_string(),
            params,
            schema: self.schema.clone(),
        };
        let (reply, events) = self.client.open(self.token.clone(), call).await?;
        let Reply::LiveQuery { rows, .. } = reply else {
            return Err(ClientError::UnexpectedReply);
        };
        Ok(LiveQuery {
            rows,
            events,
            client: self.client.clone(),
            token: self.token.clone(),
        })
    }
}

/// A live query. Dropping it unsubscribes.
pub struct LiveQuery {
    rows: Rows,
    events: Subscription,
    client: Client,
    token: Option<Token>,
}

impl LiveQuery {
    /// The latest result.
    pub fn rows(&self) -> &Rows {
        &self.rows
    }

    pub fn subscription(&self) -> u64 {
        self.events.id()
    }

    /// Wait for the next result. `None` once the connection drops; open the
    /// query again after reconnecting.
    pub async fn changed(&mut self) -> Option<&Rows> {
        loop {
            match self.events.next().await? {
                Event::RowsChanged(rows) => {
                    self.rows = rows;
                    return Some(&self.rows);
                }
                _ => continue,
            }
        }
    }
}

impl std::fmt::Debug for LiveQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LiveQuery({}, {} rows)",
            self.events.id(),
            self.rows.rows.len()
        )
    }
}

impl Drop for LiveQuery {
    fn drop(&mut self) {
        let call = Call::SqlUnsubscribe {
            subscription: self.events.id(),
        };
        self.client.send_now(self.token.clone(), call);
    }
}
