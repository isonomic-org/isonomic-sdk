// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `connections` interface of the in-memory engine: tool servers kept
//! in memory, with no network.
//!
//! `connections.add` answers `needs_sign_in` for a remote server without a
//! secret and `ready` (with no tools) otherwise. `connections.sign_in`
//! answers a fixed local test page, then publishes the connection `ready`
//! on every shell status subscription, or `failed` when a test asked for
//! that with [`FakeConnections::fail_sign_ins`].

use std::sync::Arc;

use isonomic_proto::connections::{
    AddConnection, Connection, ConnectionState, ConnectionTool, Server, SignInStarted,
};
use isonomic_proto::{Call, ErrorCode, Reply, ServerMessage, WireError};

use crate::conn::{failure, result, Conn};
use crate::State;

/// The page `connections.sign_in` answers, followed by the connection's id.
pub const SIGN_IN_PAGE: &str = "http://127.0.0.1/sign-in/";

struct Held {
    connection: Connection,
    secret: Option<String>,
    tools: Vec<ConnectionTool>,
}

#[derive(Default)]
pub(crate) struct ConnectionsState {
    held: Vec<Held>,
    next: u64,
    sign_in_failure: Option<String>,
}

/// Scripting handle for the fake connections. Cheap to clone.
#[derive(Clone)]
pub struct FakeConnections {
    pub(crate) state: Arc<State>,
}

impl FakeConnections {
    /// Every connection, as `connections.list` gives them.
    pub fn list(&self) -> Vec<Connection> {
        let connections = self.state.connections.lock().unwrap();
        connections
            .held
            .iter()
            .map(|h| h.connection.clone())
            .collect()
    }

    /// The secret the shell added a connection with, to check it arrived.
    pub fn secret(&self, id: &str) -> Option<String> {
        let connections = self.state.connections.lock().unwrap();
        let held = connections.held.iter().find(|h| h.connection.id == id);
        held.and_then(|h| h.secret.clone())
    }

    /// Move a connection to `state` (with `error` for `failed`) and tell
    /// every shell status subscription. Panics on an unknown id.
    pub fn set_state(&self, id: &str, state: ConnectionState, error: Option<&str>) {
        let found = publish(&self.state, id, |c| {
            c.state = state;
            c.error = error.map(str::to_string);
        });
        assert!(found, "no fake connection {id}");
    }

    /// The tools `connections.tools` lists for a connection, and its count.
    /// Panics on an unknown id.
    pub fn set_tools(&self, id: &str, tools: Vec<ConnectionTool>) {
        let mut connections = self.state.connections.lock().unwrap();
        let Some(held) = connections.held.iter_mut().find(|h| h.connection.id == id) else {
            panic!("no fake connection {id}");
        };
        held.connection.tools = tools.len() as u32;
        held.tools = tools;
    }

    /// Every later sign-in ends `failed` with this message; `None` makes
    /// them succeed again.
    pub fn fail_sign_ins(&self, message: Option<&str>) {
        let mut connections = self.state.connections.lock().unwrap();
        connections.sign_in_failure = message.map(str::to_string);
    }
}

/// Change one connection and send its new state to the shell. `false`
/// when there is no such connection (any more).
fn publish(state: &State, id: &str, change: impl FnOnce(&mut Connection)) -> bool {
    let connection = {
        let mut connections = state.connections.lock().unwrap();
        let Some(held) = connections.held.iter_mut().find(|h| h.connection.id == id) else {
            return false;
        };
        change(&mut held.connection);
        held.connection.clone()
    };
    let _ = state.connection_changes.send(connection);
    true
}

fn not_found(id: &str) -> WireError {
    WireError::new(ErrorCode::ConnectionNotFound, format!("no connection {id}"))
}

fn add(state: &State, request: AddConnection) -> Result<Reply, WireError> {
    if request.name.trim().is_empty() {
        let message = "a connection needs a name";
        return Err(WireError::new(ErrorCode::BadRequest, message));
    }
    let needs_sign_in = match &request.server {
        Server::Remote { .. } => request.secret.is_none(),
        Server::Local { .. } => false,
        _ => {
            let message = "this engine does not know that kind of tool server";
            return Err(WireError::new(ErrorCode::BadRequest, message));
        }
    };
    let mut connections = state.connections.lock().unwrap();
    connections.next += 1;
    let connection = Connection {
        id: format!("conn-{}", connections.next),
        name: request.name,
        key: request.key,
        server: request.server,
        state: match needs_sign_in {
            true => ConnectionState::NeedsSignIn,
            false => ConnectionState::Ready,
        },
        error: None,
        tools: 0,
    };
    connections.held.push(Held {
        connection: connection.clone(),
        secret: request.secret.map(|s| s.reveal().to_string()),
        tools: Vec::new(),
    });
    Ok(Reply::Connection(connection))
}

/// The page to open, or why there is none. `Ok` also says how the sign-in
/// will end: `None` for ready, the failure message otherwise.
fn sign_in(state: &State, id: &str) -> Result<(String, Option<String>), WireError> {
    let connections = state.connections.lock().unwrap();
    let Some(held) = connections.held.iter().find(|h| h.connection.id == id) else {
        return Err(not_found(id));
    };
    if !matches!(held.connection.server, Server::Remote { .. }) {
        let message = "This tool server has no sign-in.";
        return Err(WireError::new(ErrorCode::SignInFailed, message));
    }
    Ok((
        format!("{SIGN_IN_PAGE}{id}"),
        connections.sign_in_failure.clone(),
    ))
}

fn answer(state: &State, call: Call) -> Result<Reply, WireError> {
    let mut connections = state.connections.lock().unwrap();
    let find = |held: &[Held], id: &str| {
        let found = held.iter().position(|h| h.connection.id == id);
        found.ok_or_else(|| not_found(id))
    };
    match call {
        Call::ConnectionsList(_) => Ok(Reply::Connections(
            connections
                .held
                .iter()
                .map(|h| h.connection.clone())
                .collect(),
        )),
        Call::ConnectionsRemove(request) => {
            let at = find(&connections.held, &request.id)?;
            connections.held.remove(at);
            Ok(Reply::Done {})
        }
        Call::ConnectionsTools(request) => {
            let at = find(&connections.held, &request.id)?;
            Ok(Reply::ConnectionTools(connections.held[at].tools.clone()))
        }
        other => Err(WireError::new(
            ErrorCode::UnknownMethod,
            format!("the testkit has no method {}", other.method()),
        )),
    }
}

/// Answer one `connections.*` call. `None` when the answer was already
/// sent. The token check has already kept apps out.
pub(crate) async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    call: Call,
) -> Option<ServerMessage> {
    let answer = match call {
        Call::ConnectionsAdd(request) => add(state, request),
        Call::ConnectionsSignIn(request) => match sign_in(state, &request.id) {
            Err(error) => Err(error),
            Ok((url, failed)) => {
                // The page first, then the outcome on the status
                // subscription.
                let reply = Reply::SignIn(SignInStarted { url });
                let _ = conn.out.send(result(id, reply)).await;
                // Removed meanwhile: nothing to say.
                publish(state, &request.id, |c| match failed {
                    None => {
                        c.state = ConnectionState::Ready;
                        c.error = None;
                    }
                    Some(message) => {
                        c.state = ConnectionState::Failed;
                        c.error = Some(message);
                    }
                });
                return None;
            }
        },
        call => answer(state, call),
    };
    Some(match answer {
        Ok(reply) => result(id, reply),
        Err(error) => failure(id, error),
    })
}
