//! The `sql` interface of the in-memory engine: a scripted database. It
//! runs no SQL. Tests say what a query returns (`respond`), how many rows a
//! statement changes (`affect`) and which statements fail (`fail`), then
//! read back every statement an app ran (`statements`). Changing a query's
//! result pushes it to every live query on it, as the real engine does when
//! a write changes the result.

use std::collections::HashMap;
use std::sync::Arc;

use isonomic_proto::auth::authorize_schema;
use isonomic_proto::sql::{normalize, Statement};
use isonomic_proto::{
    Call, ErrorCode, Event, Manifest, Reply, Rows, ServerMessage, SqlValue, Token, WireError,
};
use tokio::sync::broadcast;

use crate::conn::{failure, result, Conn};
use crate::State;

/// One statement an app ran, for assertions.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlCall {
    /// The calling app's id; `None` for the shell.
    pub app: Option<String>,
    pub schema: String,
    /// As sent, with its spacing normalized.
    pub sql: String,
    pub params: Vec<SqlValue>,
}

type Key = (String, String);

pub(crate) struct SqlState {
    results: HashMap<Key, Rows>,
    affected: HashMap<Key, u64>,
    failures: HashMap<String, String>,
    log: Vec<SqlCall>,
    changes: broadcast::Sender<(Key, Rows)>,
}

impl Default for SqlState {
    fn default() -> Self {
        Self {
            results: HashMap::new(),
            affected: HashMap::new(),
            failures: HashMap::new(),
            log: Vec::new(),
            changes: broadcast::channel(256).0,
        }
    }
}

fn key(schema: &str, sql: &str) -> Key {
    (schema.to_string(), normalize(sql))
}

/// Script and inspect the fake database.
#[derive(Clone)]
pub struct FakeSql {
    pub(crate) state: Arc<State>,
}

impl FakeSql {
    /// What `sql` returns in `schema` from now on. Every live query on it
    /// receives the new result.
    pub fn respond(&self, schema: &str, sql: &str, rows: Rows) {
        let mut db = self.state.sql.lock().unwrap();
        db.results.insert(key(schema, sql), rows.clone());
        let _ = db.changes.send((key(schema, sql), rows));
    }

    /// How many rows `sql` reports changing in `schema` (0 until set).
    pub fn affect(&self, schema: &str, sql: &str, rows: u64) {
        let mut db = self.state.sql.lock().unwrap();
        db.affected.insert(key(schema, sql), rows);
    }

    /// Make `sql` fail with `sql_error` and this message, in any schema.
    pub fn fail(&self, sql: &str, message: &str) {
        let mut db = self.state.sql.lock().unwrap();
        db.failures.insert(normalize(sql), message.to_string());
    }

    /// Every statement run so far, oldest first. A failed batch adds none.
    pub fn statements(&self) -> Vec<SqlCall> {
        self.state.sql.lock().unwrap().log.clone()
    }
}

fn sql_error(message: &str) -> WireError {
    WireError::new(ErrorCode::SqlError, message)
}

impl SqlState {
    fn check(&self, statement: &Statement) -> Result<(), WireError> {
        match self.failures.get(&normalize(&statement.sql)) {
            Some(message) => Err(sql_error(message)),
            None => Ok(()),
        }
    }

    fn run(&mut self, app: Option<&str>, schema: &str, statement: Statement) -> u64 {
        let rows = self.affected.get(&key(schema, &statement.sql)).copied();
        self.log.push(SqlCall {
            app: app.map(str::to_string),
            schema: schema.to_string(),
            sql: normalize(&statement.sql),
            params: statement.params,
        });
        rows.unwrap_or(0)
    }

    fn rows(&self, schema: &str, sql: &str) -> Rows {
        self.results
            .get(&key(schema, sql))
            .cloned()
            .unwrap_or_default()
    }
}

pub(crate) async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    manifest: Option<&Manifest>,
    token: Option<Token>,
    call: Call,
) -> Option<ServerMessage> {
    let app = manifest.map(|m| m.id.as_str());
    let answer = |r: Result<Reply, WireError>| match r {
        Ok(reply) => result(id, reply),
        Err(error) => failure(id, error),
    };
    match call {
        Call::SqlQuery {
            sql,
            params,
            schema,
        } => Some(answer((|| {
            let schema = authorize_schema(manifest, schema.as_deref(), false)?;
            let mut db = state.sql.lock().unwrap();
            let statement = Statement { sql, params };
            db.check(&statement)?;
            let rows = db.rows(&schema, &statement.sql);
            db.run(app, &schema, statement);
            Ok(Reply::Rows(rows))
        })())),
        Call::SqlExecute {
            sql,
            params,
            schema,
        } => Some(answer((|| {
            let schema = authorize_schema(manifest, schema.as_deref(), true)?;
            let mut db = state.sql.lock().unwrap();
            let statement = Statement { sql, params };
            db.check(&statement)?;
            let rows_affected = db.run(app, &schema, statement);
            Ok(Reply::Executed { rows_affected })
        })())),
        Call::SqlBatch { statements, schema } => Some(answer((|| {
            let schema = authorize_schema(manifest, schema.as_deref(), true)?;
            let mut db = state.sql.lock().unwrap();
            for statement in &statements {
                db.check(statement)?;
            }
            let rows_affected = statements
                .into_iter()
                .map(|s| db.run(app, &schema, s))
                .collect();
            Ok(Reply::ExecutedBatch { rows_affected })
        })())),
        Call::SqlSubscribe {
            sql,
            params,
            schema,
        } => {
            let opened = (|| {
                let schema = authorize_schema(manifest, schema.as_deref(), false)?;
                let mut db = state.sql.lock().unwrap();
                let statement = Statement { sql, params };
                db.check(&statement)?;
                let watched = key(&schema, &statement.sql);
                // Listen before reading, so no change falls in between.
                let changes = db.changes.subscribe();
                let rows = db.rows(&schema, &statement.sql);
                db.run(app, &schema, statement);
                Ok((watched, changes, rows))
            })();
            let (watched, mut changes, rows) = match opened {
                Ok(opened) => opened,
                Err(error) => return Some(failure(id, error)),
            };
            let subscription = conn.open_subscription();
            let reply = result(id, Reply::LiveQuery { subscription, rows });
            let _ = conn.out.send(reply).await;
            let out = conn.out.clone();
            let task = tokio::spawn(async move {
                loop {
                    let rows = match changes.recv().await {
                        Ok((changed, rows)) if changed == watched => rows,
                        Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => return,
                    };
                    let event = ServerMessage::Event {
                        subscription,
                        event: Event::RowsChanged(rows),
                    };
                    if out.send(event).await.is_err() {
                        return;
                    }
                }
            });
            conn.track(subscription, task, None, token);
            None
        }
        Call::SqlUnsubscribe { subscription } => {
            Some(match conn.close_owned(subscription, token.as_ref()) {
                true => result(id, Reply::Unsubscribed {}),
                false => failure(
                    id,
                    WireError::new(ErrorCode::UnknownSubscription, "no such subscription"),
                ),
            })
        }
        other => Some(failure(
            id,
            WireError::new(
                ErrorCode::UnknownMethod,
                format!("{} is not a sql method", other.method()),
            ),
        )),
    }
}
