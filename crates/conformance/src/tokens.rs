// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Capability tokens: granted only for valid manifests, unknown tokens
//! refused, and a token without a capability refused on every interface.
//! The calls come from the frozen fixtures, so a new method is covered the
//! day its fixture lands.

use std::collections::BTreeSet;

use isonomic_proto::auth::Interface;
use isonomic_proto::manifest::{AgentsAccess, BlobAccess, ModelAccess, SqlAccess};
use isonomic_proto::{Call, Capabilities, ClientMessage, ErrorCode, Json, Token};

use crate::{ensure, manifest, ok, refused, Outcome, Target};

const FIXTURES: &[&str] = &[
    include_str!("../../proto/fixtures/v0/system.jsonl"),
    include_str!("../../proto/fixtures/v0/apps.jsonl"),
    include_str!("../../proto/fixtures/v0/models.jsonl"),
    include_str!("../../proto/fixtures/v0/blobs.jsonl"),
    include_str!("../../proto/fixtures/v0/events.jsonl"),
    include_str!("../../proto/fixtures/v0/sql.jsonl"),
    include_str!("../../proto/fixtures/v0/agents.jsonl"),
    include_str!("../../proto/fixtures/v0/identity.jsonl"),
];

/// Every call in the fixtures, in order.
fn fixture_calls() -> Result<Vec<Call>, String> {
    let mut calls = Vec::new();
    for line in FIXTURES.iter().flat_map(|f| f.lines()) {
        if !line.contains(r#""type":"request""#) {
            continue;
        }
        match serde_json::from_str::<ClientMessage>(line) {
            Ok(ClientMessage::Request { call, .. }) => calls.push(call),
            Ok(_) => {}
            Err(error) => return Err(format!("fixture does not decode ({error}): {line}")),
        }
    }
    Ok(calls)
}

/// Every capability there is, with nothing in reach beyond the app itself.
pub(crate) fn everything() -> Capabilities {
    Capabilities {
        models: Some(ModelAccess {
            any: true,
            remote: true,
            ..Default::default()
        }),
        blobs: Some(BlobAccess::default()),
        sql: Some(SqlAccess::default()),
        ..Default::default()
    }
}

/// The shell grants valid manifests, including one with the agents
/// capability, and refuses invalid ones.
pub async fn grant(target: Target) -> Outcome {
    let valid = manifest("grant", everything(), &["conformance_grant"]);
    let token = ok(target.shell.grant(valid.clone()).await, "a valid manifest")?;
    ensure(!token.as_str().is_empty(), || "an empty token".into())?;

    let mut bad_version = valid.clone();
    bad_version.version = "one".into();
    let result = target.shell.grant(bad_version).await;
    refused(result, ErrorCode::InvalidManifest, "version \"one\"")?;

    let mut bad_id = valid.clone();
    bad_id.id = "Conformance".into();
    let result = target.shell.grant(bad_id).await;
    refused(result, ErrorCode::InvalidManifest, "id \"Conformance\"")?;

    let mut agents = valid;
    agents.capabilities.agents = Some(AgentsAccess::default());
    ok(target.shell.grant(agents).await, "the agents capability").map(|_| ())
}

/// A token the engine never issued: `unknown_token`, on any interface.
pub async fn unknown_token(target: Target) -> Outcome {
    let stranger = target.shell.scoped(Token::new("conformance-never-issued"));
    let calls = [
        Call::ModelsList,
        Call::BlobsInfo {
            handle: "conformance-none".into(),
        },
        Call::EventsPublish {
            topic: "conformance.ping".into(),
            data: Json::default(),
            sender: None,
            app: None,
        },
        Call::SqlQuery {
            sql: "select 1".into(),
            params: vec![],
            schema: None,
        },
    ];
    for call in calls {
        let method = call.method();
        refused(stranger.call(call).await, ErrorCode::UnknownToken, method)?;
    }
    Ok(())
}

/// Even an app with every capability never reaches `system`, `apps`, or
/// the model install calls (`models.download`, `download_cancel`,
/// `recommended`). (`system.shutdown` is checked last, on its own.)
pub async fn shell_only(target: Target) -> Outcome {
    let app = target
        .app(manifest("shell-only", everything(), &[]))
        .await?;
    for call in fixture_calls()? {
        let shell_only = call.interface().shell_only() || call.window_only();
        if !shell_only || matches!(call, Call::SystemShutdown) {
            continue;
        }
        let method = call.method();
        refused(app.call(call).await, ErrorCode::CapabilityDenied, method)?;
    }
    Ok(())
}

/// An app with no capabilities is refused every method of every interface
/// that needs one (all but `events`), and the fixtures cover every such
/// method.
pub async fn every_interface(target: Target) -> Outcome {
    let app = target
        .app(manifest("no-capabilities", Capabilities::default(), &[]))
        .await?;
    let mut covered = BTreeSet::new();
    for call in fixture_calls()? {
        let needs_one = matches!(
            call.interface(),
            Interface::System
                | Interface::Apps
                | Interface::Identity
                | Interface::Models
                | Interface::Blobs
                | Interface::Sql
                | Interface::Agents
        );
        if !needs_one || matches!(call, Call::SystemShutdown) {
            continue;
        }
        let method = call.method();
        covered.insert(method);
        refused(app.call(call).await, ErrorCode::CapabilityDenied, method)?;
    }
    let missing: Vec<&str> = Call::METHODS
        .iter()
        .filter(|m| !m.starts_with("events.") && **m != "system.shutdown")
        .filter(|m| !covered.contains(*m))
        .copied()
        .collect();
    ensure(missing.is_empty(), || {
        format!("no fixture call for {}", missing.join(", "))
    })
}

/// Models access names models: one outside the list is refused.
pub async fn named_model(target: Target) -> Outcome {
    let only = Capabilities {
        models: Some(ModelAccess {
            ids: vec!["conformance-model".into()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let app = target.app(manifest("one-model", only, &[])).await?;
    let result = app.models().info("conformance-other-model").await;
    refused(
        result,
        ErrorCode::CapabilityDenied,
        "a model outside the list",
    )
}

/// Events need no capability, but an app reaches only its own scope.
pub async fn other_scope(target: Target) -> Outcome {
    let app = target
        .app(manifest("scope", Capabilities::default(), &[]))
        .await?;
    let other = Some("com.isonomic.conformance.elsewhere".to_string());
    let publish = Call::EventsPublish {
        topic: "conformance.ping".into(),
        data: Json::default(),
        sender: None,
        app: other.clone(),
    };
    let subscribe = Call::EventsSubscribe {
        topics: vec![],
        app: other,
    };
    refused(
        app.call(publish).await,
        ErrorCode::CapabilityDenied,
        "publish elsewhere",
    )?;
    refused(
        app.call(subscribe).await,
        ErrorCode::CapabilityDenied,
        "listen elsewhere",
    )
}

/// An app cannot stop the engine, and the engine is still there after it
/// tried.
pub async fn shutdown_refused(target: Target) -> Outcome {
    let app = target.app(manifest("shutdown", everything(), &[])).await?;
    let result = app.call(Call::SystemShutdown).await;
    refused(result, ErrorCode::CapabilityDenied, "system.shutdown")?;
    ok(target.shell.status().await, "status after the refusal").map(|_| ())
}
