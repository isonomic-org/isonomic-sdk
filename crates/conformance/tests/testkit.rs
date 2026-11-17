// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The testkit passes every check, and an engine that breaks the contract
//! does not: the checks are not vacuous.

use std::collections::HashSet;
use std::sync::Arc;

use isonomic_client::{BoxedStream, Connector};
use isonomic_conformance::{run, run_only, CHECKS};
use isonomic_proto::io::read_frame;
use isonomic_testkit::TestEngine;
use tokio::io::AsyncWriteExt;

#[tokio::test(flavor = "multi_thread")]
async fn the_testkit_passes_every_check() {
    let engine = TestEngine::new();
    let report = run(engine.connector()).await;
    println!("{report}");
    report.assert_passed();
    assert_eq!(report.results.len(), CHECKS.len());
    assert!(!engine.shutdown_requested(), "an app shut the engine down");
}

#[test]
fn check_names_are_unique_and_namespaced() {
    let mut seen = HashSet::new();
    for check in CHECKS {
        assert!(seen.insert(check.name), "{} twice", check.name);
        assert!(check.name.contains('.'), "{} has no group", check.name);
    }
    assert_eq!(CHECKS.last().unwrap().name, "tokens.shutdown_refused");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_filter_runs_only_its_checks() {
    let engine = TestEngine::new();
    let report = run_only(engine.connector(), |name| name.starts_with("blobs.")).await;
    report.assert_passed();
    let names: Vec<&str> = report.results.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        [
            "blobs.round_trip",
            "blobs.chunked",
            "blobs.private",
            "blobs.max_bytes"
        ]
    );
}

/// An engine that ignores tokens: a proxy in front of the testkit that
/// strips the token from every request, so each app call runs as the shell.
fn ignores_tokens(engine: &TestEngine) -> Connector {
    let inner = engine.connector();
    Arc::new(move || {
        let inner = inner.clone();
        Box::pin(async move {
            let upstream = inner().await?;
            let (client_side, proxy) = tokio::io::duplex(1 << 20);
            let (mut from_client, mut to_client) = tokio::io::split(proxy);
            let (mut from_engine, mut to_engine) = tokio::io::split(upstream);
            tokio::spawn(async move {
                let _ = tokio::io::copy(&mut from_engine, &mut to_client).await;
            });
            tokio::spawn(async move {
                while let Ok(Some(body)) = read_frame(&mut from_client).await {
                    let body = match serde_json::from_slice::<serde_json::Value>(&body) {
                        Ok(mut message) => {
                            if let Some(map) = message.as_object_mut() {
                                map.remove("token");
                            }
                            serde_json::to_vec(&message).unwrap()
                        }
                        Err(_) => body,
                    };
                    let mut frame = (body.len() as u32).to_be_bytes().to_vec();
                    frame.extend_from_slice(&body);
                    if to_engine.write_all(&frame).await.is_err() {
                        return;
                    }
                }
            });
            Ok(Box::new(client_side) as BoxedStream)
        })
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn an_engine_that_ignores_tokens_fails() {
    let engine = TestEngine::new();
    let report = run(ignores_tokens(&engine)).await;
    println!("{report}");
    let failed: HashSet<&str> = report.failures().into_iter().map(|(n, _)| n).collect();
    for name in [
        "tokens.unknown_token",
        "tokens.shell_only",
        "tokens.every_interface",
        "tokens.named_model",
        "tokens.other_scope",
        "blobs.private",
        "blobs.max_bytes",
        "events.scoped",
        "sql.schemas",
        "agents.isolated",
        "tokens.shutdown_refused",
    ] {
        assert!(failed.contains(name), "{name} passed:\n{report}");
    }
    for name in ["wire.welcome", "wire.unknown_method", "blobs.round_trip"] {
        assert!(!failed.contains(name), "{name} failed:\n{report}");
    }
}
