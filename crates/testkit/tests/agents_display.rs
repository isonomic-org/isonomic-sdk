// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Display widgets through the testkit's host: the built-in `chat` preset
//! offers them on `standard` and not on `low`, a display call is the host's
//! to answer, and the specs come from one place.

use isonomic_client::{AppClient, ClientError};
use isonomic_proto::agents::display::{display_spec, DISPLAY_SPECS};
use isonomic_proto::agents::schema::{DisplayOutcome, Event, Profile, ToolKind};
use isonomic_proto::agents::{ClientToolResult, SessionEvent, SessionId};
use isonomic_proto::manifest::AgentsAccess;
use isonomic_proto::{Capabilities, ErrorCode, Manifest};
use isonomic_testkit::TestEngine;

const WIDGETS: [&str; 4] = ["show_table", "show_steps", "show_metrics", "show_chart"];

fn chat(engine: &TestEngine) -> AppClient {
    let manifest = Manifest {
        id: "com.isonomic.chat".into(),
        name: "Chat".into(),
        version: "0.1.0".into(),
        icon: "chat".into(),
        capabilities: Capabilities {
            agents: Some(AgentsAccess {}),
            ..Default::default()
        },
        schemas: vec![],
    };
    let token = engine.grant(&manifest);
    engine.client().scoped(token)
}

/// The display tools the first reply step offered.
fn offered(log: &[SessionEvent]) -> Vec<String> {
    let tools = log.iter().find_map(|e| match &e.event {
        Event::StepStart(s) => s.tools.clone(),
        _ => None,
    });
    let mut names: Vec<String> = tools
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.kind == ToolKind::Display)
        .map(|t| t.name)
        .collect();
    names.sort();
    names
}

async fn one_turn(engine: &TestEngine) -> (SessionId, Vec<SessionEvent>) {
    let agents = chat(engine).agents();
    let session = agents.create("chat").await.unwrap();
    agents.send_text(&session, "Hi").await.unwrap();
    let log = engine.agents().log(&session);
    (session, log)
}

#[test]
fn the_sdk_lists_exactly_the_specs_of_the_pinned_harness() {
    let names: Vec<&str> = DISPLAY_SPECS.iter().map(|s| s.name).collect();
    assert_eq!(names, WIDGETS);
    assert!(WIDGETS.iter().all(|n| display_spec(n).is_some()));
    assert!(display_spec("show_map").is_none());
}

#[tokio::test]
async fn the_chat_preset_offers_the_four_widgets_on_standard() {
    let engine = TestEngine::new();
    engine.agents().set_profile(Some(Profile::Standard));
    let (_, log) = one_turn(&engine).await;
    let mut want: Vec<String> = WIDGETS.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(offered(&log), want);
}

#[tokio::test]
async fn the_chat_preset_offers_none_on_low() {
    let engine = TestEngine::new();
    engine.agents().set_profile(Some(Profile::Low));
    let (_, log) = one_turn(&engine).await;
    assert_eq!(offered(&log), Vec::<String>::new());
}

#[tokio::test]
async fn an_app_cannot_answer_a_display_call() {
    let engine = TestEngine::new();
    let input =
        r#"{"metrics": [{"label": "Total", "value": "2,340"}], "fallback_text": "Total 2,340."}"#;
    engine
        .agents()
        .with(|host| host.model.calls(&[("show_metrics", input)]));
    engine.agents().reply("Here it is.");
    let agents = chat(&engine).agents();
    let session = agents.create("chat").await.unwrap();
    agents.send_text(&session, "Show the total").await.unwrap();
    let log = engine.agents().log(&session);
    let call = log
        .iter()
        .find_map(|e| match &e.event {
            Event::ToolCall(c) if c.kind == ToolKind::Display => Some(c.call.clone()),
            _ => None,
        })
        .expect("a display call");
    let shown = log.iter().any(
        |e| matches!(&e.event, Event::ToolResult(r) if r.call == call && r.display == Some(DisplayOutcome::Shown)),
    );
    assert!(shown, "the host records the display call's result itself");
    let result = ClientToolResult {
        session: session.clone(),
        call,
        content: vec![],
        error: None,
    };
    match agents.tool_result(result).await {
        Err(ClientError::Engine(e)) => assert_eq!(e.code, ErrorCode::NotFound),
        other => panic!("expected not_found, got {other:?}"),
    }
}
