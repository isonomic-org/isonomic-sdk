// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Capability tokens against the in-memory engine: an app reaches only what
//! its manifest allows, on every interface.

use isonomic_client::ClientError;
use isonomic_proto::manifest::ModelAccess;
use isonomic_proto::{Call, Capabilities, ErrorCode, Manifest, Reply, Token};
use isonomic_testkit::{CallRecord, TestEngine};

fn manifest() -> Manifest {
    Manifest {
        id: "com.isonomic.hello".into(),
        name: "Hello".into(),
        version: "0.1.0".into(),
        icon: "sparkle".into(),
        capabilities: Capabilities {
            models: Some(ModelAccess {
                any: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        schemas: vec![],
    }
}

fn code(result: Result<Reply, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shell_grants_a_token_over_the_wire() {
    let engine = TestEngine::new();
    let shell = engine.client();
    let token = shell.grant(manifest()).await.unwrap();
    assert!(token.as_str().contains("com.isonomic.hello"));

    let mut broken = manifest();
    broken.version = "one".into();
    match shell.grant(broken).await {
        Err(ClientError::Engine(error)) => assert_eq!(error.code, ErrorCode::InvalidManifest),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_token_never_reaches_a_shell_interface() {
    let engine = TestEngine::new();
    let app = engine.app_client(&manifest());
    for call in [
        Call::SystemStatus,
        Call::SystemProfile,
        Call::SystemShutdown,
        Call::AppsGrant {
            manifest: Box::new(manifest()),
        },
    ] {
        assert_eq!(code(app.call(call).await), ErrorCode::CapabilityDenied);
    }
    assert!(!engine.shutdown_requested());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_or_revoked_token_is_refused() {
    let engine = TestEngine::new();
    let forged = engine.client().scoped(Token::new("forged"));
    assert_eq!(
        code(forged.call(Call::SystemStatus).await),
        ErrorCode::UnknownToken
    );

    let app = engine.app_client(&manifest());
    engine.revoke_all();
    assert_eq!(
        code(app.call(Call::SystemStatus).await),
        ErrorCode::UnknownToken
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn every_call_is_recorded_with_its_caller() {
    let engine = TestEngine::new();
    let shell = engine.client();
    shell.status().await.unwrap();
    let app = shell.scoped(engine.grant(&manifest()));
    let _ = app.call(Call::SystemShutdown).await;
    let calls = engine.calls();
    assert!(calls.contains(&CallRecord {
        app: None,
        method: "system.status".into()
    }));
    assert!(calls.contains(&CallRecord {
        app: Some("com.isonomic.hello".into()),
        method: "system.shutdown".into()
    }));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscription_delivers_its_own_events() {
    let engine = TestEngine::new();
    let shell = engine.client();
    let (reply, mut events) = shell.subscribe(Call::SystemSubscribe).await.unwrap();
    assert!(matches!(reply, Reply::Subscribed { .. }));
    engine.set_state("postgres", isonomic_proto::ServiceState::Starting);
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), events.next())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        isonomic_proto::Event::ServiceStatus(ref s) if s.name == "postgres"
    ));
    // A call that opens nothing cannot be used as a subscription.
    assert!(matches!(
        shell.subscribe(Call::SystemStatus).await,
        Err(ClientError::UnexpectedReply)
    ));
}
