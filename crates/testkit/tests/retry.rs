// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! `system.retry`: a degraded service starts again, the outcome arrives as
//! status events, and anything else is refused.

use std::time::Duration;

use isonomic_client::{ClientError, Subscription};
use isonomic_proto::{Call, Capabilities, ErrorCode, Event, Manifest, ServiceState};
use isonomic_testkit::{healthy, TestEngine};

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

async fn next_state(events: &mut Subscription, service: &str) -> ServiceState {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), events.next())
            .await
            .expect("a status event within 5 s")
            .expect("open");
        if let Event::ServiceStatus(status) = event {
            if status.name == service {
                return status.state;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_degraded_service_starts_again() {
    let engine = TestEngine::new();
    engine.set_service(healthy("search", 2));
    engine.set_state("search", ServiceState::Degraded);
    engine.on_retry("search", ServiceState::Healthy);
    let client = engine.client();
    let (_, mut events) = client.subscribe(Call::SystemSubscribe).await.unwrap();

    client.retry("search").await.unwrap();
    assert_eq!(engine.retries(), ["search"]);
    assert_eq!(
        next_state(&mut events, "search").await,
        ServiceState::Starting
    );
    assert_eq!(
        next_state(&mut events, "search").await,
        ServiceState::Healthy
    );
    let status = client.status().await.unwrap();
    assert_eq!(status.aggregate, ServiceState::Healthy);
}

#[tokio::test(flavor = "multi_thread")]
async fn unscripted_it_stays_starting_until_the_test_moves_it() {
    let engine = TestEngine::new();
    engine.set_state("postgres", ServiceState::Degraded);
    let client = engine.client();
    let (_, mut events) = client.subscribe(Call::SystemSubscribe).await.unwrap();
    client.retry("postgres").await.unwrap();
    assert_eq!(
        next_state(&mut events, "postgres").await,
        ServiceState::Starting
    );
    engine.set_state("postgres", ServiceState::Degraded);
    assert_eq!(
        next_state(&mut events, "postgres").await,
        ServiceState::Degraded
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn only_degraded_services_and_only_the_shell() {
    let engine = TestEngine::new();
    let client = engine.client();
    assert_eq!(code(client.retry("postgres").await), ErrorCode::NotDegraded);
    assert_eq!(code(client.retry("nope").await), ErrorCode::UnknownService);
    assert!(engine.retries().is_empty());

    engine.set_state("postgres", ServiceState::Degraded);
    let manifest = Manifest {
        id: "com.isonomic.hello".into(),
        name: "Hello".into(),
        version: "0.1.0".into(),
        icon: "sparkle".into(),
        capabilities: Capabilities::default(),
        schemas: vec![],
    };
    let app = engine.app_client(&manifest);
    let call = Call::SystemRetry {
        service: "postgres".into(),
    };
    assert_eq!(code(app.call(call).await), ErrorCode::CapabilityDenied);
    assert!(engine.retries().is_empty());
}
