// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The client against the in-memory engine: requests, the live view, and
//! reconnecting after the engine goes away.

use std::time::Duration;

use isonomic_client::{Client, ClientError, Live};
use isonomic_proto::{ErrorCode, ServiceState};
use isonomic_testkit::{healthy, TestEngine};

/// Wait until the live view satisfies `want`, without polling.
async fn until(client: &Client, want: impl Fn(&Live) -> bool) -> Live {
    let mut live = client.live();
    let seen = tokio::time::timeout(Duration::from_secs(5), live.wait_for(|view| want(view)))
        .await
        .expect("live view never reached the wanted state")
        .unwrap()
        .clone();
    seen
}

fn state_of(view: &Live, name: &str) -> Option<ServiceState> {
    let system = view.system.as_ref()?;
    system
        .services
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.state)
}

#[tokio::test(flavor = "multi_thread")]
async fn requests_are_answered_and_engine_errors_come_back_typed() {
    let engine = TestEngine::new();
    engine.set_logs("postgres", &["one", "two", "three"]);
    let client = engine.client();
    let view = until(&client, |view| view.system.is_some()).await;
    assert!(view.connected);
    assert_eq!(view.engine.as_deref(), Some("testkit"));

    let status = client.status().await.unwrap();
    assert_eq!(status.aggregate, ServiceState::Healthy);
    assert_eq!(status.cold_start_ms, Some(150));
    assert_eq!(client.profile().await.unwrap().profile, "standard");
    assert_eq!(
        client.logs("postgres", Some(2)).await.unwrap(),
        ["two", "three"]
    );
    match client.logs("nope", None).await {
        Err(ClientError::Engine(error)) => assert_eq!(error.code, ErrorCode::UnknownService),
        other => panic!("expected an engine error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_live_view_follows_every_change_without_polling() {
    let engine = TestEngine::new();
    let client = engine.client();
    until(&client, |view| view.system.is_some()).await;

    engine.set_state("postgres", ServiceState::Starting);
    let restarting = until(&client, |view| {
        state_of(view, "postgres") == Some(ServiceState::Starting)
    })
    .await;
    assert_eq!(restarting.system.unwrap().aggregate, ServiceState::Starting);

    engine.set_state("postgres", ServiceState::Healthy);
    let recovered = until(&client, |view| {
        state_of(view, "postgres") == Some(ServiceState::Healthy)
    })
    .await;
    let system = recovered.system.unwrap();
    assert_eq!(system.aggregate, ServiceState::Healthy);
    assert_eq!(system.services[0].restart_count, 1);

    // A service the client has never seen is added to the view.
    engine.set_service(healthy("cache", 1));
    let grown = until(&client, |view| state_of(view, "cache").is_some()).await;
    assert_eq!(grown.system.unwrap().services.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn it_reconnects_after_the_engine_goes_away_and_resubscribes() {
    let engine = TestEngine::new();
    let client = engine.client();
    until(&client, |view| view.system.is_some()).await;

    engine.go_offline();
    let down = until(&client, |view| !view.connected).await;
    assert_eq!(down.system, None);
    assert!(matches!(
        client.status().await,
        Err(ClientError::Disconnected)
    ));

    // The engine changed while the client was away.
    engine.set_state("postgres", ServiceState::Degraded);
    engine.go_online();
    let back = until(&client, |view| view.connected && view.system.is_some()).await;
    assert_eq!(back.system.unwrap().aggregate, ServiceState::Degraded);

    // And the new connection is subscribed: later changes still arrive.
    engine.set_state("postgres", ServiceState::Healthy);
    until(&client, |view| {
        state_of(view, "postgres") == Some(ServiceState::Healthy)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_started_before_the_engine_connects_once_it_is_up() {
    let engine = TestEngine::new();
    engine.go_offline();
    let client = engine.client();
    assert!(matches!(
        client.status().await,
        Err(ClientError::Disconnected)
    ));
    engine.go_online();
    until(&client, |view| view.connected && view.system.is_some()).await;
    assert!(client.status().await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_reaches_the_engine() {
    let engine = TestEngine::new();
    let client = engine.client();
    until(&client, |view| view.connected).await;
    assert!(!engine.shutdown_requested());
    client.shutdown().await.unwrap();
    assert!(engine.shutdown_requested());
}
