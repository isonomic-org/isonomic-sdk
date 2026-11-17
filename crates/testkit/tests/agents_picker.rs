// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Sessions on a model the app picks: the capability rule, refusals of
//! unknown, non-chat, uninstalled and malformed names, and the model
//! reaching the host and the summary.

use isonomic_client::ClientError;
use isonomic_proto::manifest::AgentsAccess;
use isonomic_proto::{Capabilities, ErrorCode, Manifest};
use isonomic_testkit::TestEngine;

fn manifest(id: &str, agents: bool) -> Manifest {
    Manifest {
        id: id.into(),
        name: "Agents app".into(),
        version: "0.1.0".into(),
        icon: "chat".into(),
        capabilities: Capabilities {
            agents: agents.then_some(AgentsAccess {}),
            ..Default::default()
        },
        schemas: vec![],
    }
}

fn app(engine: &TestEngine, id: &str) -> isonomic_client::AppClient {
    let token = engine.grant(&manifest(id, true));
    engine.client().scoped(token)
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_session_may_name_a_model_its_app_may_use() {
    use isonomic_proto::agents::CreateSession;
    use isonomic_proto::manifest::ModelAccess;
    let engine = TestEngine::new();
    let mut chat = manifest("com.isonomic.chat", true);
    chat.capabilities.models = Some(ModelAccess {
        ids: vec![
            "chat-small".into(),
            "chat-huge".into(),
            "embed-small".into(),
        ],
        ..Default::default()
    });
    let agents = engine.client().scoped(engine.grant(&chat)).agents();
    let on = |model: &str| CreateSession {
        preset: "test".into(),
        title: None,
        location_opt_in: false,
        model: Some(model.into()),
    };
    agents.create_with(on("chat-small")).await.unwrap();
    assert_eq!(
        code(agents.create_with(on("chat-large")).await),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        code(agents.create_with(on("chat-huge")).await),
        ErrorCode::UnknownModel
    );
    assert_eq!(
        code(agents.create_with(on("embed-small")).await),
        ErrorCode::Unsupported
    );
    engine.models().uninstall("chat-small");
    assert_eq!(
        code(agents.create_with(on("chat-small")).await),
        ErrorCode::ModelUnavailable
    );
    let plain = app(&engine, "com.isonomic.notes").agents();
    assert_eq!(
        code(plain.create_with(on("chat-small")).await),
        ErrorCode::CapabilityDenied
    );
}

#[tokio::test]
async fn the_picked_model_runs_the_session_and_shows_on_its_summary() {
    use isonomic_proto::agents::CreateSession;
    use isonomic_proto::manifest::ModelAccess;
    let engine = TestEngine::new();
    let mut chat = manifest("com.isonomic.chat", true);
    chat.capabilities.models = Some(ModelAccess {
        ids: vec!["chat-small".into()],
        ..Default::default()
    });
    let agents = engine.client().scoped(engine.grant(&chat)).agents();
    let on = |model: &str| CreateSession {
        preset: "test".into(),
        title: None,
        location_opt_in: false,
        model: Some(model.into()),
    };
    let picked = agents.create_with(on("  chat-small ")).await.unwrap();
    agents.send_text(&picked, "Hi").await.unwrap();
    let asked = engine
        .agents()
        .with(|host| host.model.requests.lock().unwrap().last().cloned());
    assert_eq!(asked.expect("a model request").model, "chat-small");
    let summary = agents.open(&picked).await.unwrap().summary;
    assert_eq!(summary.model.as_deref(), Some("chat-small"));
    let preset = agents.create("test").await.unwrap();
    assert_eq!(agents.open(&preset).await.unwrap().summary.model, None);
    assert_eq!(
        code(agents.create_with(on("   ")).await),
        ErrorCode::BadRequest
    );
    let long = "m".repeat(isonomic_proto::agents::MAX_MODEL_NAME + 1);
    assert_eq!(
        code(agents.create_with(on(&long)).await),
        ErrorCode::BadRequest
    );
    let wide = "é".repeat(isonomic_proto::agents::MAX_MODEL_NAME);
    let refused = agents.create_with(on(&wide)).await;
    assert_eq!(code(refused), ErrorCode::BadRequest, "over 200 bytes");
    let control = agents.create_with(on("chat\u{7}small")).await;
    assert_eq!(code(control), ErrorCode::BadRequest);
}

/// A preset of the app's own that runs on `model`.
fn preset_on(id: &str, model: &str) -> String {
    preset_with(id, model, None)
}

/// The same, with a title model, and the title step on so it would run.
fn preset_with(id: &str, model: &str, title_model: Option<&str>) -> String {
    let title = title_model.map_or(String::new(), |t| format!("title_model = \"{t}\"\n"));
    let post = if title_model.is_some() {
        "\n[post_turn]\ntitle = \"model\"\n"
    } else {
        ""
    };
    format!(
        "id = \"{id}\"\nversion = 1\nname = \"Own\"\nclass = \"interactive\"\n\n[persona]\nprompt = \"Be brief.\"\n\n[model]\nid = \"{model}\"\nmax_output_tokens = 256\nreasoning = \"off\"\n{title}\n[limits]\ntokens = 20000\nsteps = 8\nwall_ms = 60000\n{post}"
    )
}

#[tokio::test]
async fn an_apps_own_preset_cannot_reach_a_model_its_manifest_does_not_allow() {
    use isonomic_proto::manifest::ModelAccess;
    let engine = TestEngine::new();
    engine.models().add(isonomic_proto::ModelInfo {
        id: "chat-large".into(),
        name: "Chat Large".into(),
        context_length: Some(32768),
        capabilities: vec!["chat".into()],
        remote: false,
        recommended: false,
    });
    let mut chat = manifest("com.isonomic.chat", true);
    chat.capabilities.models = Some(ModelAccess {
        ids: vec!["chat-small".into()],
        ..Default::default()
    });
    let agents = engine.client().scoped(engine.grant(&chat)).agents();
    let large = agents
        .put_preset(&preset_on("large", "chat-large"))
        .await
        .unwrap();
    let refused = agents.create(&large.id.to_string()).await;
    assert_eq!(code(refused), ErrorCode::CapabilityDenied);
    let small = agents
        .put_preset(&preset_on("small", "chat-small"))
        .await
        .unwrap();
    agents.create(&small.id.to_string()).await.unwrap();
    agents.create("test").await.unwrap();
}

#[tokio::test]
async fn a_stored_presets_title_model_is_held_to_the_rule_too() {
    use isonomic_proto::manifest::ModelAccess;
    let engine = TestEngine::new();
    let mut chat = manifest("com.isonomic.chat", true);
    chat.capabilities.models = Some(ModelAccess {
        ids: vec!["chat-small".into()],
        ..Default::default()
    });
    let agents = engine.client().scoped(engine.grant(&chat)).agents();
    let text = preset_with("titled", "chat-small", Some("chat-large"));
    let stored = agents.put_preset(&text).await.unwrap();
    let refused = agents.create(&stored.id.to_string()).await;
    assert_eq!(code(refused), ErrorCode::CapabilityDenied);
    let fine = preset_with("titled-ok", "chat-small", Some("chat-small"));
    let stored = agents.put_preset(&fine).await.unwrap();
    agents.create(&stored.id.to_string()).await.unwrap();
}
