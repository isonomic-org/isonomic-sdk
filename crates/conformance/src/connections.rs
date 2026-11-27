// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The shell's model setup and tool server connections. Apps are kept out
//! by `tokens.shell_only`, which sends every fixture call; these check what
//! the shell gets.
//!
//! The one connection added is a remote server at an unused local port,
//! removed again at once. No check signs in or names a real server.

use std::collections::HashSet;

use isonomic_proto::connections::{AddConnection, Server};
use isonomic_proto::ErrorCode;

use crate::{ensure, ok, refused, Outcome, Target};

const NO_SUCH: &str = "conformance-no-such-connection";

/// A connection the shell adds is listed with the name and key it gave,
/// and is gone once removed.
pub async fn round_trip(target: Target) -> Outcome {
    let connections = target.shell.connections();
    let add = AddConnection {
        name: "Conformance".into(),
        key: Some("conformance".into()),
        server: Server::Remote {
            url: "http://127.0.0.1:9/conformance".into(),
        },
        secret: None,
    };
    let added = ok(connections.add(add).await, "add")?;
    ensure(!added.id.is_empty(), || "an empty id".into())?;
    ensure(
        added.name == "Conformance" && added.key.as_deref() == Some("conformance"),
        || format!("add answered {added:?}"),
    )?;
    let listed = ok(connections.list().await, "list")?;
    ensure(listed.iter().any(|c| c.id == added.id), || {
        format!("{} is not listed", added.id)
    })?;
    ok(connections.remove(&added.id).await, "remove")?;
    let listed = ok(connections.list().await, "list after remove")?;
    ensure(listed.iter().all(|c| c.id != added.id), || {
        format!("{} is still listed", added.id)
    })?;
    refused(
        connections.remove(&added.id).await,
        ErrorCode::ConnectionNotFound,
        "remove twice",
    )
}

/// An id the engine never minted: `connection_not_found` everywhere.
pub async fn unknown_id(target: Target) -> Outcome {
    let connections = target.shell.connections();
    let found = ErrorCode::ConnectionNotFound;
    refused(connections.sign_in(NO_SUCH).await, found, "sign_in")?;
    refused(connections.remove(NO_SUCH).await, found, "remove")?;
    refused(connections.tools(NO_SUCH).await, found, "tools")
}

/// The catalogue holds local models only, each once.
pub async fn catalogue_local(target: Target) -> Outcome {
    let entries = ok(target.shell.models().catalogue().await, "catalogue")?;
    let mut seen = HashSet::new();
    for entry in &entries {
        let id = &entry.detail.info.id;
        ensure(!entry.detail.info.remote, || format!("{id} is remote"))?;
        ensure(seen.insert(id.clone()), || format!("{id} twice"))?;
    }
    Ok(())
}

/// Removing a model the engine does not know: `unknown_model`.
pub async fn remove_unknown(target: Target) -> Outcome {
    refused(
        target
            .shell
            .models()
            .remove("conformance-no-such-model")
            .await,
        ErrorCode::UnknownModel,
        "models.remove",
    )
}
