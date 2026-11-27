// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The window's settings against the in-memory engine: tool server
//! connections (add, sign in, tools, remove, the `connection` event) and
//! the model setup (`models.catalogue`, `models.remove`), all the shell's
//! alone.

use std::time::Duration;

use isonomic_client::{Client, ClientError};
use isonomic_proto::connections::{
    AddConnection, Connection, ConnectionSecret, ConnectionState, ConnectionTool, Server,
};
use isonomic_proto::manifest::ModelAccess;
use isonomic_proto::models::{ModelState, RemoveModel};
use isonomic_proto::{Call, Capabilities, ErrorCode, HardwareProfile, Manifest, ModelInfo};
use isonomic_testkit::{DownloadStep, Gate, TestEngine, SIGN_IN_PAGE};

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

fn remote(name: &str, secret: Option<&str>) -> AddConnection {
    AddConnection {
        name: name.into(),
        key: Some("files".into()),
        server: Server::Remote {
            url: "https://tools.example.com/api".into(),
        },
        secret: secret.map(ConnectionSecret::new),
    }
}

/// A shell client whose status subscription is open, so no event is
/// missed.
async fn shell(engine: &TestEngine) -> Client {
    let client = engine.client();
    let mut live = client.live();
    let open = live.wait_for(|l| l.system.is_some());
    tokio::time::timeout(Duration::from_secs(5), open)
        .await
        .expect("no status within 5 s")
        .expect("the client stopped");
    client
}

/// Wait until the client has heard `id` reach `state`.
async fn heard(client: &Client, id: &str, state: ConnectionState) -> Connection {
    let mut changes = client.connections().changes();
    let reached = changes.wait_for(|s| s.get(id).is_some_and(|c| c.state == state));
    let states = tokio::time::timeout(Duration::from_secs(5), reached)
        .await
        .unwrap_or_else(|_| panic!("{id} did not reach {state:?} within 5 s"))
        .expect("the client stopped");
    states[id].clone()
}

#[tokio::test]
async fn a_remote_server_signs_in_and_the_window_hears_it() {
    let engine = TestEngine::new();
    let shell = shell(&engine).await;
    let connections = shell.connections();
    assert!(connections.list().await.unwrap().is_empty());
    let added = connections.add(remote("Files", None)).await.unwrap();
    assert_eq!(added.state, ConnectionState::NeedsSignIn);
    assert_eq!((added.key.as_deref(), added.tools), (Some("files"), 0));
    let started = connections.sign_in(&added.id).await.unwrap();
    assert_eq!(started.url, format!("{SIGN_IN_PAGE}{}", added.id));
    assert!(started.url.starts_with("http://127.0.0.1/"));
    let ready = heard(&shell, &added.id, ConnectionState::Ready).await;
    assert_eq!(ready.error, None);
    assert_eq!(connections.list().await.unwrap(), vec![ready]);
}

#[tokio::test]
async fn a_secret_or_a_local_server_is_ready_at_once() {
    let engine = TestEngine::new();
    let connections = shell(&engine).await.connections();
    let keyed = connections
        .add(remote("Tracker", Some("test-token-0001")))
        .await
        .unwrap();
    assert_eq!(keyed.state, ConnectionState::Ready);
    assert_eq!(
        engine.connections().secret(&keyed.id).as_deref(),
        Some("test-token-0001")
    );
    let local = AddConnection {
        name: "Notes".into(),
        key: None,
        server: Server::Local {
            command: "notes-server".into(),
            args: vec!["--root".into(), "notes".into()],
        },
        secret: None,
    };
    let local = connections.add(local).await.unwrap();
    assert_eq!(local.state, ConnectionState::Ready);
    assert_ne!(local.id, keyed.id);
    let no_sign_in = connections.sign_in(&local.id).await;
    assert_eq!(code(no_sign_in), ErrorCode::SignInFailed);
    let blank = connections.add(remote(" ", None)).await;
    assert_eq!(code(blank), ErrorCode::BadRequest);
}

#[tokio::test]
async fn tools_remove_and_unknown_ids() {
    let engine = TestEngine::new();
    let connections = shell(&engine).await.connections();
    let added = connections
        .add(remote("Files", Some("test-token-0001")))
        .await
        .unwrap();
    assert!(connections.tools(&added.id).await.unwrap().is_empty());
    let tool = ConnectionTool {
        name: "search".into(),
        description: "Find files".into(),
    };
    engine
        .connections()
        .set_tools(&added.id, vec![tool.clone()]);
    assert_eq!(connections.tools(&added.id).await.unwrap(), vec![tool]);
    assert_eq!(connections.list().await.unwrap()[0].tools, 1);
    connections.remove(&added.id).await.unwrap();
    assert!(connections.list().await.unwrap().is_empty());
    let gone = ErrorCode::ConnectionNotFound;
    assert_eq!(code(connections.remove(&added.id).await), gone);
    assert_eq!(code(connections.tools(&added.id).await), gone);
    assert_eq!(code(connections.sign_in("conn-9").await), gone);
}

#[tokio::test]
async fn a_failed_sign_in_and_a_scripted_change_arrive_as_events() {
    let engine = TestEngine::new();
    let shell = shell(&engine).await;
    let connections = shell.connections();
    let added = connections.add(remote("Files", None)).await.unwrap();
    let words = "The server turned the sign-in down.";
    engine.connections().fail_sign_ins(Some(words));
    connections.sign_in(&added.id).await.unwrap();
    let failed = heard(&shell, &added.id, ConnectionState::Failed).await;
    assert_eq!(failed.error.as_deref(), Some(words));
    engine
        .connections()
        .set_state(&added.id, ConnectionState::Connecting, None);
    heard(&shell, &added.id, ConnectionState::Connecting).await;
}

#[tokio::test]
async fn apps_never_reach_connections_or_the_model_setup() {
    let engine = TestEngine::new();
    let manifest = Manifest {
        id: "com.isonomic.hello".into(),
        name: "Hello".into(),
        version: "0.0.1".into(),
        icon: "hand".into(),
        capabilities: Capabilities {
            models: Some(ModelAccess {
                any: true,
                remote: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        schemas: vec![],
    };
    let app = engine.app_client(&manifest);
    let calls = [
        Call::ConnectionsList(Default::default()),
        Call::ConnectionsAdd(remote("Files", Some("test-token-0001"))),
        Call::ModelsCatalogue(Default::default()),
        Call::ModelsRemove(RemoveModel {
            id: "chat-small".into(),
        }),
    ];
    for call in calls {
        let method = call.method();
        assert_eq!(
            code(app.call(call).await),
            ErrorCode::CapabilityDenied,
            "{method}"
        );
    }
    assert!(engine.connections().list().is_empty());
    assert!(engine.models().installed("chat-small"));
}

#[tokio::test]
async fn the_catalogue_lists_local_models_installed_or_not() {
    let engine = TestEngine::new();
    let mut large = ModelInfo {
        id: "chat-large".into(),
        name: "Chat Large".into(),
        context_length: Some(131072),
        capabilities: vec!["chat".into()],
        remote: false,
        recommended: false,
    };
    engine.models().add(large.clone());
    engine.models().set_size("chat-large", Some(64 << 30));
    engine.models().uninstall("chat-large");
    engine
        .models()
        .describe("chat-large", Some("A large model."), Some("Test Lab"));
    large.id = "chat-remote".into();
    large.remote = true;
    engine.models().add(large);
    let models = engine.client().models();
    let entries = models.catalogue().await.unwrap();
    let ids: Vec<&str> = entries.iter().map(|e| e.detail.info.id.as_str()).collect();
    assert_eq!(ids, ["chat-small", "embed-small", "chat-large"]);
    let small = &entries[0];
    assert_eq!(small.detail.state, ModelState::Stopped);
    assert!(small.fits && small.detail.info.recommended);
    let large = &entries[2];
    assert_eq!(large.detail.state, ModelState::NotInstalled);
    assert!(!large.fits, "64 GiB on a 16 GiB machine");
    let reason = large.fit_reason.as_deref().unwrap();
    assert!(
        reason.contains("64 GB") && reason.contains("16 GB"),
        "{reason}"
    );
    assert_eq!(large.about.as_deref(), Some("A large model."));
    assert_eq!(large.publisher.as_deref(), Some("Test Lab"));
    engine.set_profile(HardwareProfile {
        profile: "high".into(),
        total_memory_bytes: 128 << 30,
        available_memory_bytes: 96 << 30,
    });
    assert!(models.catalogue().await.unwrap()[2].fits);
}

#[tokio::test]
async fn removing_a_model_makes_it_not_installed() {
    let engine = TestEngine::new();
    let models = engine.client().models();
    models.remove("chat-small").await.unwrap();
    assert!(!engine.models().installed("chat-small"));
    assert_eq!(
        models.info("chat-small").await.unwrap().state,
        ModelState::NotInstalled
    );
    assert_eq!(
        code(models.remove("chat-small").await),
        ErrorCode::NotInstalled
    );
    assert_eq!(
        code(models.remove("chat-huge").await),
        ErrorCode::UnknownModel
    );
}

#[tokio::test]
async fn a_running_download_must_be_cancelled_first() {
    let engine = TestEngine::new();
    engine.models().uninstall("chat-small");
    let gate = Gate::new();
    let steps = vec![DownloadStep::Progress(1 << 20), DownloadStep::Hold(gate)];
    engine.models().script_download("chat-small", steps);
    let models = engine.client().models();
    let mut download = models.download("chat-small").await.unwrap();
    assert_eq!(
        code(models.remove("chat-small").await),
        ErrorCode::DownloadRunning
    );
    models.cancel_download("chat-small").await.unwrap();
    while let Some(event) = download.next().await {
        if event.unwrap().outcome.is_some() {
            break;
        }
    }
    assert_eq!(
        code(models.remove("chat-small").await),
        ErrorCode::NotInstalled
    );
}
