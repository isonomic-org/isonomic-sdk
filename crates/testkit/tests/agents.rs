// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The agents interface against the testkit's host (harness-host's
//! `MemoryHost`): a scripted turn streamed to the app, resuming a stream,
//! isolation between apps, approvals that need the person, client tools,
//! presets and purge.

use std::time::Duration;

use isonomic_client::{AppClient, ClientError, SessionStream, SessionUpdate};
use isonomic_proto::agents::schema::{Concurrency, Effect, Event, ToolKind, ToolOutcome, Value};
use isonomic_proto::agents::{ClientToolResult, DecisionInput, SessionEvent, ToolSpec};
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

fn app(engine: &TestEngine, id: &str) -> AppClient {
    let token = engine.grant(&manifest(id, true));
    engine.client().scoped(token)
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

/// Durable events up to and including the first of type `last`.
async fn until(stream: &mut SessionStream, last: &str) -> Vec<SessionEvent> {
    let mut seen = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
        match next
            .expect("no event within 5 s")
            .expect("the stream ended")
        {
            SessionUpdate::Event(event) => {
                let done = event.event.type_name() == last;
                seen.push(*event);
                if done {
                    return seen;
                }
            }
            SessionUpdate::Delta(_) => {}
            SessionUpdate::Gap { after, next } => panic!("gap from {after} to {next}"),
            SessionUpdate::Ended(reason) => panic!("ended: {reason:?}"),
        }
    }
}

fn types(events: &[SessionEvent]) -> Vec<&str> {
    events.iter().map(|e| e.event.type_name()).collect()
}

fn tool(name: &str, effects: Vec<Effect>) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("The {name} tool."),
        input_schema: Value::object([("type", Value::from("object"))]),
        kind: ToolKind::Client,
        effects,
        concurrency: Concurrency::Serial,
    }
}

#[tokio::test]
async fn a_scripted_turn_streams_to_the_app_in_order() {
    let engine = TestEngine::new();
    let chat = app(&engine, "com.isonomic.chat");
    let agents = chat.agents();
    engine.agents().reply("Hello there.");
    let session = agents.create("test").await.unwrap();
    let mut stream = agents.subscribe(&session, 0).await.unwrap();
    let queued = agents.send_text(&session, "Hi").await.unwrap();
    assert!(!queued);
    let events = until(&mut stream, "turn.end").await;
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, (1..=events.len() as u64).collect::<Vec<_>>());
    let names = types(&events);
    assert_eq!(names[0], "session.start");
    assert!(names.contains(&"assistant"), "{names:?}");
    let reply = events.iter().find(|e| e.event.type_name() == "assistant");
    assert!(format!("{reply:?}").contains("Hello there."));
    assert_eq!(stream.last_seq(), events.last().unwrap().seq);
    let snapshot = agents.open(&session).await.unwrap();
    assert_eq!(snapshot.summary.app.to_string(), "com.isonomic.chat");
    assert_eq!(snapshot.turn_open, None);
}

#[tokio::test]
async fn a_stream_resumes_after_the_last_seq_seen() {
    let engine = TestEngine::new();
    let agents = app(&engine, "com.isonomic.chat").agents();
    let session = agents.create("test").await.unwrap();
    agents.send_text(&session, "Hi").await.unwrap();
    let stored = engine.agents().log(&session);
    let from = stored[2].seq;
    let mut stream = agents.subscribe(&session, from).await.unwrap();
    let rest = until(&mut stream, stored.last().unwrap().event.type_name()).await;
    assert_eq!(rest.first().unwrap().seq, from + 1);
    assert_eq!(rest.len() as u64, stored.last().unwrap().seq - from);
}

#[tokio::test]
async fn an_app_reaches_only_its_own_sessions() {
    let engine = TestEngine::new();
    let owner = app(&engine, "com.isonomic.chat").agents();
    let other = app(&engine, "com.isonomic.notes").agents();
    let session = owner.create("test").await.unwrap();
    assert_eq!(code(other.open(&session).await), ErrorCode::NotFound);
    assert_eq!(
        code(other.send_text(&session, "Hijack").await),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(other.subscribe(&session, 0).await),
        ErrorCode::NotFound
    );
    let (theirs, _) = other.list(Default::default()).await.unwrap();
    assert!(theirs.is_empty());
    let (all, _) = engine
        .client()
        .agents()
        .list(Default::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 1, "the shell sees every app's sessions");
}

#[tokio::test]
async fn agents_calls_need_the_capability() {
    let engine = TestEngine::new();
    let token = engine.grant(&manifest("com.isonomic.plain", false));
    let plain = engine.client().scoped(token).agents();
    assert_eq!(
        code(plain.create("test").await),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(code(plain.presets().await), ErrorCode::CapabilityDenied);
}

#[tokio::test]
async fn an_approval_needs_the_persons_user_action() {
    let engine = TestEngine::new();
    let id = "com.isonomic.chat";
    let agents = app(&engine, id).agents();
    let session = agents.create("test").await.unwrap();
    let specs = vec![tool("post", vec![Effect::ActsOutward])];
    agents.register_client_tools(&session, specs).await.unwrap();
    engine
        .agents()
        .with(|host| host.model.calls(&[("post", "{\"text\":\"hi\"}")]));
    agents.send_text(&session, "Post hi.").await.unwrap();
    let pending = agents.approvals(None).await.unwrap();
    assert_eq!(pending.len(), 1);
    let approval = pending[0].request.approval.clone();
    let refused = agents
        .decide(&session, &approval, DecisionInput::Allow, None)
        .await;
    assert_eq!(code(refused), ErrorCode::MissingUserAction);
    let forged = agents
        .decide(&session, &approval, DecisionInput::Allow, Some("forged"))
        .await;
    assert_eq!(code(forged), ErrorCode::MissingUserAction);
    let person = engine.agents().user_action(Some(id), &session, &approval);
    agents
        .decide(
            &session,
            &approval,
            DecisionInput::Allow,
            Some(person.as_str()),
        )
        .await
        .unwrap();
    let again = agents
        .decide(
            &session,
            &approval,
            DecisionInput::Allow,
            Some(person.as_str()),
        )
        .await;
    assert_eq!(code(again), ErrorCode::AlreadyDecided);
    let log = engine.agents().log(&session);
    assert!(types(&log).contains(&"approval.decision"));
}

#[tokio::test]
async fn a_client_tool_call_reaches_the_app_and_its_result_closes_the_step() {
    let engine = TestEngine::new();
    let agents = app(&engine, "com.isonomic.chat").agents();
    let session = agents.create("test").await.unwrap();
    let specs = vec![tool("insert_block", vec![])];
    agents.register_client_tools(&session, specs).await.unwrap();
    let mut stream = agents.subscribe(&session, 0).await.unwrap();
    engine
        .agents()
        .with(|host| host.model.calls(&[("insert_block", "{}")]));
    engine.agents().reply("Added.");
    agents.send_text(&session, "Add a heading.").await.unwrap();
    let events = until(&mut stream, "tool.call").await;
    let Event::ToolCall(call) = &events.last().unwrap().event else {
        panic!("not a tool call");
    };
    assert_eq!(call.kind, ToolKind::Client);
    let result = ClientToolResult {
        session: session.clone(),
        call: call.call.clone(),
        content: vec![],
        error: None,
    };
    agents.tool_result(result).await.unwrap();
    let events = until(&mut stream, "turn.end").await;
    let closed = events
        .iter()
        .any(|e| matches!(&e.event, Event::ToolResult(r) if r.outcome == ToolOutcome::Ok));
    assert!(closed, "{:?}", types(&events));
}

#[tokio::test]
async fn presets_list_the_built_ins_and_refuse_invalid_text() {
    let engine = TestEngine::new();
    let agents = app(&engine, "com.isonomic.chat").agents();
    let presets = agents.presets().await.unwrap();
    assert!(presets.iter().any(|p| p.name == "Chat"), "{presets:?}");
    let chat = agents.preset("chat").await.unwrap();
    assert!(chat.text.contains("class = \"interactive\""));
    let refused = agents
        .put_preset("id = \"broken\"\nversion = \"one\"")
        .await;
    assert_eq!(code(refused), ErrorCode::InvalidPreset);
}

#[tokio::test]
async fn purge_removes_only_the_callers_sessions() {
    let engine = TestEngine::new();
    let chat = app(&engine, "com.isonomic.chat").agents();
    let notes = app(&engine, "com.isonomic.notes").agents();
    let mine = chat.create("test").await.unwrap();
    chat.send_text(&mine, "Hi").await.unwrap();
    let theirs = notes.create("test").await.unwrap();
    let report = chat.purge(None).await.unwrap();
    assert_eq!(report.sessions, 1);
    assert!(report.events > 0);
    assert_eq!(code(chat.open(&mine).await), ErrorCode::NotFound);
    notes.open(&theirs).await.unwrap();
}

#[tokio::test]
async fn unsubscribing_ends_only_the_callers_own_stream() {
    let engine = TestEngine::new();
    let agents = app(&engine, "com.isonomic.chat").agents();
    let session = agents.create("test").await.unwrap();
    let stream = agents.subscribe(&session, 0).await.unwrap();
    let id = stream.subscription();
    let other = app(&engine, "com.isonomic.notes");
    let theirs = other
        .call(isonomic_proto::Call::SessionsUnsubscribe { subscription: id })
        .await;
    assert_eq!(code(theirs), ErrorCode::UnknownSubscription);
    drop(stream);
}

#[tokio::test]
async fn an_approval_is_named_by_its_session_as_well_as_its_id() {
    let engine = TestEngine::new();
    let id = "com.isonomic.chat";
    let agents = app(&engine, id).agents();
    let held = agents.create("test").await.unwrap();
    let other = agents.create("test").await.unwrap();
    let specs = vec![tool("post", vec![Effect::ActsOutward])];
    agents.register_client_tools(&held, specs).await.unwrap();
    engine
        .agents()
        .with(|host| host.model.calls(&[("post", "{}")]));
    agents.send_text(&held, "Post.").await.unwrap();
    let approval = agents.approvals(None).await.unwrap()[0]
        .request
        .approval
        .clone();
    let person = engine.agents().user_action(Some(id), &held, &approval);
    let elsewhere = agents
        .decide(
            &other,
            &approval,
            DecisionInput::Allow,
            Some(person.as_str()),
        )
        .await;
    assert_eq!(code(elsewhere), ErrorCode::NotFound);
    let answered = isonomic_proto::agents::AnswerQuestion {
        session: other.clone(),
        question: "q-1".into(),
        selected: vec![],
        text: Some("Yes".into()),
        skipped: false,
    };
    assert_eq!(code(agents.answer(answered).await), ErrorCode::NotFound);
    agents
        .decide(
            &held,
            &approval,
            DecisionInput::Allow,
            Some(person.as_str()),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn only_the_shell_names_another_app() {
    use isonomic_proto::agents::{ListApprovals, PurgeSessions};
    use isonomic_proto::{Call, Reply};
    let engine = TestEngine::new();
    let chat = app(&engine, "com.isonomic.chat");
    let notes = app(&engine, "com.isonomic.notes");
    let mine = chat.agents().create("test").await.unwrap();
    notes.agents().create("test").await.unwrap();
    let named = Some("com.isonomic.chat".into());
    let reach = Call::SessionsPurge(PurgeSessions { app: named.clone() });
    assert_eq!(code(notes.call(reach).await), ErrorCode::CapabilityDenied);
    let everything = Call::SessionsPurge(PurgeSessions { app: None });
    let refused = engine.client().call(everything).await;
    assert_eq!(code(refused), ErrorCode::BadRequest);
    let listed = Call::ApprovalsList(ListApprovals { app: named.clone() });
    let shell = engine.client();
    let reply = shell.call(listed).await.unwrap();
    assert!(matches!(reply, Reply::Approvals(list) if list.is_empty()));
    let purge = Call::SessionsPurge(PurgeSessions { app: named });
    let Reply::Purged(report) = shell.call(purge).await.unwrap() else {
        panic!("not a purge report");
    };
    assert_eq!(report.sessions, 1);
    assert_eq!(code(chat.agents().open(&mine).await), ErrorCode::NotFound);
    let (left, _) = shell.agents().list(Default::default()).await.unwrap();
    assert_eq!(left.len(), 1, "the other app's session stays");
}

#[tokio::test]
async fn equal_approval_ids_in_two_apps_stay_apart() {
    let engine = TestEngine::new();
    let (chat_id, notes_id) = ("com.isonomic.chat", "com.isonomic.notes");
    let mut pending = vec![];
    for id in [chat_id, notes_id] {
        let agents = app(&engine, id).agents();
        let session = agents.create("test").await.unwrap();
        let specs = vec![tool("post", vec![Effect::ActsOutward])];
        agents.register_client_tools(&session, specs).await.unwrap();
        engine
            .agents()
            .with(|host| host.model.calls(&[("post", "{}")]));
        agents.send_text(&session, "Post.").await.unwrap();
        let approval = agents.approvals(None).await.unwrap()[0]
            .request
            .approval
            .clone();
        pending.push((agents, session, approval));
    }
    assert_eq!(pending[0].2, pending[1].2, "ids are minted per session");
    let (all, _) = (engine.client().agents().approvals(None).await.unwrap(), ());
    assert_eq!(all.len(), 2, "the shell sees both");
    let (chat, session, approval) = &pending[0];
    let person = engine
        .agents()
        .user_action(Some(chat_id), session, approval);
    chat.decide(
        session,
        approval,
        DecisionInput::Allow,
        Some(person.as_str()),
    )
    .await
    .unwrap();
    let (notes, _, _) = &pending[1];
    assert_eq!(
        notes.approvals(None).await.unwrap().len(),
        1,
        "the other app's waits"
    );
}

#[tokio::test]
async fn a_purged_sessions_stream_ends() {
    let engine = TestEngine::new();
    let agents = app(&engine, "com.isonomic.chat").agents();
    let session = agents.create("test").await.unwrap();
    let mut stream = agents.subscribe(&session, 0).await.unwrap();
    until(&mut stream, "session.start").await;
    agents.purge(None).await.unwrap();
    let last = tokio::time::timeout(Duration::from_secs(5), async {
        let mut last = None;
        while let Some(update) = stream.next().await {
            last = Some(update);
        }
        last
    })
    .await
    .expect("the stream did not end after purge");
    let purged = isonomic_proto::agents::EndReason::Purged;
    assert_eq!(
        last,
        Some(SessionUpdate::Ended(purged)),
        "told apart from a dropped connection"
    );
}

#[tokio::test]
async fn the_window_mints_the_persons_token_and_apps_cannot() {
    let engine = TestEngine::new();
    let id = "com.isonomic.chat";
    let agents = app(&engine, id).agents();
    let session = agents.create("test").await.unwrap();
    let specs = vec![tool("post", vec![Effect::ActsOutward])];
    agents.register_client_tools(&session, specs).await.unwrap();
    engine
        .agents()
        .with(|host| host.model.calls(&[("post", "{}")]));
    agents.send_text(&session, "Post.").await.unwrap();
    let approval = agents.approvals(None).await.unwrap()[0]
        .request
        .approval
        .clone();
    let refused = agents.user_action(&session, &approval, Some(id)).await;
    assert_eq!(code(refused), ErrorCode::CapabilityDenied);
    let window = engine.client().agents();
    let token = window
        .user_action(&session, &approval, Some(id))
        .await
        .unwrap();
    agents
        .decide(
            &session,
            &approval,
            DecisionInput::Allow,
            Some(token.as_str()),
        )
        .await
        .unwrap();
}
