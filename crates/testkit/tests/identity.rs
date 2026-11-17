// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The person's Isonomic ID: made once, the same ever after, refusable,
//! and the shell's alone.

use isonomic_client::ClientError;
use isonomic_proto::{Call, Capabilities, ErrorCode, Manifest};
use isonomic_testkit::{TestEngine, DEFAULT_IDENTITY};

#[tokio::test]
async fn the_id_is_made_once_and_the_same_after() {
    let engine = TestEngine::new();
    let shell = engine.client();
    assert!(!engine.identity().made());
    let first = shell.identity().await.unwrap();
    assert_eq!((first.id.as_str(), first.created), (DEFAULT_IDENTITY, true));
    let again = shell.identity().await.unwrap();
    assert_eq!(
        (again.id.as_str(), again.created),
        (DEFAULT_IDENTITY, false)
    );
    assert_eq!(
        engine.client().identity().await.unwrap(),
        again,
        "any window sees it"
    );
    engine.identity().forget();
    assert!(shell.identity().await.unwrap().created, "a first run again");
}

#[tokio::test]
async fn a_scripted_id_and_a_refusal_in_plain_words() {
    let engine = TestEngine::new();
    engine.identity().set_id("isonomic:7Q4K-M2XD-9PLV");
    let shell = engine.client();
    let words = "The key store did not let Isonomic save your ID.";
    engine.identity().refuse(words);
    match shell.identity().await {
        Err(ClientError::Engine(error)) => {
            assert_eq!(error.code, ErrorCode::IdentityUnavailable);
            assert_eq!(error.message, words, "shown as written");
        }
        other => panic!("expected identity_unavailable, got {other:?}"),
    }
    assert!(!engine.identity().made(), "a refusal makes nothing");
    engine.identity().recover();
    let made = shell.identity().await.unwrap();
    assert_eq!(
        (made.id.as_str(), made.created),
        ("isonomic:7Q4K-M2XD-9PLV", true)
    );
}

#[tokio::test]
async fn apps_never_see_the_id() {
    let engine = TestEngine::new();
    let manifest = Manifest {
        id: "com.isonomic.chat".into(),
        name: "Chat".into(),
        version: "0.1.0".into(),
        icon: "chat".into(),
        capabilities: Capabilities::default(),
        schemas: vec![],
    };
    let app = engine.client().scoped(engine.grant(&manifest));
    match app.call(Call::IdentityGet(Default::default())).await {
        Err(ClientError::Engine(error)) => assert_eq!(error.code, ErrorCode::CapabilityDenied),
        other => panic!("expected capability_denied, got {other:?}"),
    }
    assert!(!engine.identity().made(), "an app's call makes nothing");
}
