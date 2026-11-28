// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The events interface against the fake: delivery across windows, topic
//! filters, scopes, the shell reaching an app, unsubscribing, and refusals.

use std::time::Duration;

use isonomic_client::{ClientError, EventStream};
use isonomic_proto::{AppEvent, Call, Capabilities, ErrorCode, Json, Manifest};
use isonomic_testkit::TestEngine;
use serde_json::json;

fn app(id: &str) -> Manifest {
    Manifest {
        id: id.into(),
        name: "Events app".into(),
        version: "0.1.0".into(),
        icon: "bell".into(),
        capabilities: Capabilities::default(),
        schemas: vec![],
    }
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

async fn next(stream: &mut EventStream) -> AppEvent {
    tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("an event within 5 s")
        .expect("the stream is open")
}

/// Nothing arrives for a while.
async fn quiet(stream: &mut EventStream) {
    let waited = tokio::time::timeout(Duration::from_millis(150), stream.next()).await;
    assert!(waited.is_err(), "unexpected event: {waited:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tab_in_one_window_hears_a_tab_in_another() {
    let engine = TestEngine::new();
    let chat = app("com.isonomic.chat");
    // Two windows: two connections, each with the app's token.
    let first = engine.app_client(&chat).events();
    let second = engine.app_client(&chat).events();
    let mut stream = second.subscribe(&[]).await.unwrap();
    let data = Json::new(&json!({"thread": "t-42"})).unwrap();
    first
        .publish("chat.thread-opened", data.clone(), Some("tab-1"))
        .await
        .unwrap();
    let event = next(&mut stream).await;
    assert_eq!(event.topic, "chat.thread-opened");
    assert_eq!(event.data, data);
    assert_eq!(event.sender.as_deref(), Some("tab-1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn topics_filter_and_order_is_kept() {
    let engine = TestEngine::new();
    let events = engine.app_client(&app("com.isonomic.chat")).events();
    let mut opened = events.subscribe(&["chat.opened"]).await.unwrap();
    let mut all = events.subscribe(&[]).await.unwrap();
    for (topic, n) in [("chat.opened", 1), ("chat.closed", 2), ("chat.opened", 3)] {
        events
            .publish(topic, Json::new(&n).unwrap(), None)
            .await
            .unwrap();
    }
    let seen: Vec<u32> = [next(&mut opened).await, next(&mut opened).await]
        .iter()
        .map(|e| e.data.decode().unwrap())
        .collect();
    assert_eq!(seen, [1, 3]);
    for n in 1..=3u32 {
        assert_eq!(next(&mut all).await.data.decode::<u32>().unwrap(), n);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn apps_never_hear_each_other_but_the_shell_can_reach_one() {
    let engine = TestEngine::new();
    let chat = engine.app_client(&app("com.isonomic.chat")).events();
    let docs = engine.app_client(&app("com.isonomic.docs")).events();
    let mut chat_stream = chat.subscribe(&[]).await.unwrap();
    docs.publish("x", Json::new(&1).unwrap(), None)
        .await
        .unwrap();
    quiet(&mut chat_stream).await;

    let shell = engine.client();
    let call = Call::EventsPublish {
        topic: "settings.changed".into(),
        data: Json::new(&["theme"]).unwrap(),
        sender: None,
        app: Some("com.isonomic.chat".into()),
    };
    shell.call(call).await.unwrap();
    assert_eq!(next(&mut chat_stream).await.topic, "settings.changed");

    // An app may name only itself.
    let docs_client = engine.app_client(&app("com.isonomic.docs"));
    let reach = Call::EventsSubscribe {
        topics: vec![],
        app: Some("com.isonomic.chat".into()),
    };
    assert_eq!(
        code(docs_client.call(reach).await),
        ErrorCode::CapabilityDenied
    );
    let own = Call::EventsSubscribe {
        topics: vec![],
        app: Some("com.isonomic.docs".into()),
    };
    assert!(docs_client.call(own).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_a_stream_unsubscribes_and_no_one_else_can() {
    let engine = TestEngine::new();
    let chat = app("com.isonomic.chat");
    let client = engine.app_client(&chat);
    let stream = client.events().subscribe(&[]).await.unwrap();
    let subscription = stream.subscription();
    // Another app on the same connection cannot end it.
    let other = engine
        .client()
        .scoped(engine.grant(&app("com.isonomic.docs")));
    let call = Call::EventsUnsubscribe { subscription };
    assert_eq!(
        code(other.call(call.clone()).await),
        ErrorCode::UnknownSubscription
    );
    drop(stream);
    // The drop's unsubscribe is queued before this call on the same
    // connection, so by now the subscription is gone.
    let again = client.call(call).await;
    assert_eq!(code(again), ErrorCode::UnknownSubscription);
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_topics_and_unknown_tokens_are_refused() {
    let engine = TestEngine::new();
    let events = engine.app_client(&app("com.isonomic.chat")).events();
    let bad = events.publish("Not A Topic", Json::default(), None).await;
    assert_eq!(code(bad), ErrorCode::BadRequest);
    assert_eq!(
        code(events.subscribe(&["a b"]).await),
        ErrorCode::BadRequest
    );
    engine.revoke_all();
    let gone = events.publish("x", Json::default(), None).await;
    assert_eq!(code(gone), ErrorCode::UnknownToken);
}
