//! The models interface against the fake: streams with every chunk kind,
//! cancel, refusals, failures, disconnects, and token checks.

use std::time::Duration;

use isonomic_client::{AppClient, ClientError};
use isonomic_proto::manifest::ModelAccess;
use isonomic_proto::models::{ToolCallDelta, Usage};
use isonomic_proto::{
    Capabilities, ErrorCode, Manifest, Message, ModelChunk, ModelInfo, StopReason, StreamRequest,
    WireError,
};
use isonomic_testkit::{Gate, Step, TestEngine};

fn app(models: Option<ModelAccess>) -> Manifest {
    Manifest {
        id: "com.isonomic.hello".into(),
        name: "Hello".into(),
        version: "0.1.0".into(),
        icon: "sparkle".into(),
        capabilities: Capabilities {
            models,
            ..Default::default()
        },
        schemas: vec![],
    }
}

fn any() -> Option<ModelAccess> {
    Some(ModelAccess {
        any: true,
        ..Default::default()
    })
}

fn ask(text: &str) -> StreamRequest {
    StreamRequest {
        messages: vec![Message::user(text)],
        ..Default::default()
    }
}

fn remote_model() -> ModelInfo {
    ModelInfo {
        id: "chat-remote".into(),
        name: "Chat Remote".into(),
        context_length: Some(200_000),
        capabilities: vec!["chat".into()],
        remote: true,
        recommended: false,
    }
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

async fn within<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("timed out")
}

async fn hello(engine: &TestEngine) -> AppClient {
    engine.app_client(&app(any()))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_default_model_streams_an_answer_word_by_word() {
    let engine = TestEngine::new();
    let models = hello(&engine).await.models();
    let list = models.list().await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list
        .iter()
        .any(|m| m.id == "chat-small" && m.supports("chat")));

    let mut stream = models.stream(ask("hello there")).await.unwrap();
    assert_eq!(stream.model(), "chat-small");
    let mut chunks = Vec::new();
    while let Some(chunk) = within(stream.next()).await {
        chunks.push(chunk.unwrap());
    }
    let text: String = chunks.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "You said: hello there");
    let last = chunks.last().unwrap();
    assert_eq!(last.stop, Some(StopReason::End));
    assert_eq!(last.usage.unwrap().output_tokens, 4);
    assert_eq!(engine.models().requests().len(), 1);

    let detail = models.info("chat-small").await.unwrap();
    assert_eq!(detail.info.context_length, Some(8192));
    assert_eq!(
        code(models.info("chat-huge").await),
        ErrorCode::UnknownModel
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scripted_stream_carries_reasoning_tool_calls_and_usage_in_order() {
    let engine = TestEngine::new();
    let call = |args: &str, first: bool| ToolCallDelta {
        index: 0,
        id: first.then(|| "call-1".into()),
        name: first.then(|| "clock".into()),
        arguments: args.into(),
    };
    engine.models().script(
        "chat-small",
        vec![
            Step::Chunk(ModelChunk::reasoning("Needs the clock.")),
            Step::Chunk(ModelChunk {
                tool_calls: vec![call("{\"city\":", true)],
                ..Default::default()
            }),
            Step::Chunk(ModelChunk {
                tool_calls: vec![call("\"Lisbon\"}", false)],
                ..Default::default()
            }),
            Step::Chunk(ModelChunk {
                usage: Some(Usage {
                    input_tokens: 9,
                    output_tokens: 7,
                    reasoning_tokens: Some(3),
                    cost_micros: Some(0),
                }),
                stop: Some(StopReason::ToolCalls),
                ..Default::default()
            }),
        ],
    );
    let models = hello(&engine).await.models();
    let mut request = ask("time in Lisbon?");
    request.reasoning = Some(true);
    let mut stream = models.stream(request).await.unwrap();
    let mut got = Vec::new();
    while let Some(chunk) = within(stream.next()).await {
        got.push(chunk.unwrap());
    }
    assert_eq!(got.len(), 4);
    assert_eq!(got[0].reasoning.as_deref(), Some("Needs the clock."));
    let arguments: String = got
        .iter()
        .flat_map(|c| c.tool_calls.iter())
        .map(|d| d.arguments.as_str())
        .collect();
    assert_eq!(arguments, "{\"city\":\"Lisbon\"}");
    assert_eq!(got[3].stop, Some(StopReason::ToolCalls));
    assert_eq!(got[3].usage.unwrap().reasoning_tokens, Some(3));
    assert_eq!(engine.models().requests()[0].reasoning, Some(true));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_stream_waits_and_cancel_ends_it_with_cancelled() {
    let engine = TestEngine::new();
    let gate = Gate::new();
    engine.models().script(
        "chat-small",
        vec![
            Step::Chunk(ModelChunk::text("Hel")),
            Step::Hold(gate.clone()),
            Step::Chunk(ModelChunk::text("lo")),
        ],
    );
    let models = hello(&engine).await.models();
    let mut stream = models.stream(ask("hi")).await.unwrap();
    assert_eq!(within(stream.next()).await.unwrap().unwrap().text, "Hel");
    // Held: nothing more arrives until the gate opens or we cancel.
    assert!(
        tokio::time::timeout(Duration::from_millis(50), stream.next())
            .await
            .is_err()
    );
    stream.cancel().await.unwrap();
    let last = within(stream.next()).await.unwrap().unwrap();
    assert_eq!(last.stop, Some(StopReason::Cancelled));
    assert!(stream.next().await.is_none());
    assert_eq!(engine.models().cancels(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn opening_the_gate_lets_a_slow_stream_finish() {
    let engine = TestEngine::new();
    let gate = Gate::new();
    engine.models().script(
        "chat-small",
        vec![
            Step::Hold(gate.clone()),
            Step::Chunk(ModelChunk::text("late")),
        ],
    );
    let stream = hello(&engine)
        .await
        .models()
        .stream(ask("hi"))
        .await
        .unwrap();
    let text = tokio::spawn(stream.text());
    gate.open();
    assert_eq!(within(text).await.unwrap().unwrap(), "late");
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_a_stream_cancels_it_at_the_engine() {
    let engine = TestEngine::new();
    engine
        .models()
        .script("chat-small", vec![Step::Hold(Gate::new())]);
    let stream = hello(&engine)
        .await
        .models()
        .stream(ask("hi"))
        .await
        .unwrap();
    drop(stream);
    within(engine.models().wait_for_cancels(1)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_memory_gate_refusal_carries_its_numbers() {
    let engine = TestEngine::new();
    engine
        .models()
        .refuse("chat-small", 20 << 30, 6 << 30, "not enough free memory");
    let models = hello(&engine).await.models();
    match models.stream(ask("hi")).await {
        Err(ClientError::Engine(error)) => {
            assert_eq!(error.code, ErrorCode::InsufficientMemory);
            let memory = error.memory.unwrap();
            assert_eq!(memory.required_bytes, 20 << 30);
            assert_eq!(memory.available_bytes, 6 << 30);
            assert_eq!(memory.reason, "not enough free memory");
        }
        other => panic!("expected a refusal, got {:?}", other.err()),
    }
    engine.models().admit("chat-small");
    assert!(models.stream(ask("hi")).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_calls_and_mid_stream_failures_come_back_typed() {
    let engine = TestEngine::new();
    let crashed = WireError::new(ErrorCode::ModelUnavailable, "the model worker crashed");
    engine.models().fail("chat-small", crashed.clone());
    let models = hello(&engine).await.models();
    assert_eq!(
        code(models.stream(ask("hi")).await.map(|_| ())),
        ErrorCode::ModelUnavailable
    );

    engine.models().recover("chat-small");
    engine.models().script(
        "chat-small",
        vec![Step::Chunk(ModelChunk::text("par")), Step::Fail(crashed)],
    );
    let stream = models.stream(ask("hi")).await.unwrap();
    assert_eq!(
        code(within(stream.text()).await),
        ErrorCode::ModelUnavailable
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_disconnect_ends_the_stream_and_the_client_recovers() {
    let engine = TestEngine::new();
    engine
        .models()
        .script("chat-small", vec![Step::Hold(Gate::new())]);
    let app = hello(&engine).await;
    let mut stream = app.models().stream(ask("hi")).await.unwrap();
    engine.go_offline();
    assert!(matches!(
        within(stream.next()).await,
        Some(Err(ClientError::Disconnected))
    ));
    assert!(stream.next().await.is_none());
    let mut live = app.live();
    within(live.wait_for(|view| !view.connected)).await.unwrap();

    engine.go_online();
    engine.models().script("chat-small", vec![]);
    within(live.wait_for(|view| view.connected)).await.unwrap();
    let text = within(app.models().stream(ask("hi")).await.unwrap().text()).await;
    assert_eq!(text.unwrap(), "");
}

#[tokio::test(flavor = "multi_thread")]
async fn embeddings_are_one_vector_per_input_and_need_an_embeddings_model() {
    let engine = TestEngine::new();
    let models = hello(&engine).await.models();
    let vectors = models
        .embed(None, vec!["blue".into(), "green".into(), "blue".into()])
        .await
        .unwrap();
    assert_eq!(vectors.len(), 3);
    assert_eq!(vectors[0], vectors[2]);
    assert_ne!(vectors[0], vectors[1]);
    assert_eq!(
        code(models.embed(Some("chat-small"), vec!["x".into()]).await),
        ErrorCode::Unsupported
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_reaches_only_the_models_its_manifest_names() {
    let engine = TestEngine::new();
    engine.models().add(remote_model());

    // No models capability: refused on every method.
    let none = engine.app_client(&app(None)).models();
    assert_eq!(code(none.list().await), ErrorCode::CapabilityDenied);
    assert_eq!(
        code(none.info("chat-small").await),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        code(none.stream(ask("hi")).await.map(|_| ())),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        code(none.embed(None, vec!["x".into()]).await),
        ErrorCode::CapabilityDenied
    );

    // One local model only: it sees and uses that one.
    let one = engine
        .app_client(&app(Some(ModelAccess {
            ids: vec!["chat-small".into()],
            ..Default::default()
        })))
        .models();
    let seen: Vec<String> = one
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(seen, ["chat-small"]);
    assert!(one.stream(ask("hi")).await.is_ok());
    let mut other = ask("hi");
    other.model = Some("chat-remote".into());
    assert_eq!(
        code(one.stream(other.clone()).await.map(|_| ())),
        ErrorCode::CapabilityDenied
    );
    // The default embeddings model is not among its ids either.
    assert_eq!(
        code(one.embed(None, vec!["x".into()]).await),
        ErrorCode::CapabilityDenied
    );

    // Any model, but local only: the remote one is hidden and refused.
    let local = hello(&engine).await.models();
    assert!(local.list().await.unwrap().iter().all(|m| !m.remote));
    assert_eq!(
        code(local.stream(other.clone()).await.map(|_| ())),
        ErrorCode::CapabilityDenied
    );

    // Remote allowed by the manifest, but the user's switch is off.
    let remote = engine
        .app_client(&app(Some(ModelAccess {
            any: true,
            remote: true,
            ..Default::default()
        })))
        .models();
    assert_eq!(
        code(remote.stream(other.clone()).await.map(|_| ())),
        ErrorCode::RemoteDisabled
    );
    engine.models().set_remote_enabled(true);
    assert!(remote.stream(other).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn one_app_cannot_cancel_another_apps_stream() {
    let engine = TestEngine::new();
    engine
        .models()
        .script("chat-small", vec![Step::Hold(Gate::new())]);
    let shell = engine.client();
    let first = shell.scoped(engine.grant(&app(any())));
    let mut second_manifest = app(any());
    second_manifest.id = "com.isonomic.other".into();
    let second = shell.scoped(engine.grant(&second_manifest));

    let stream = first.models().stream(ask("hi")).await.unwrap();
    let theirs = isonomic_proto::Call::ModelsCancel {
        subscription: stream.subscription(),
    };
    match second.call(theirs).await {
        Err(ClientError::Engine(error)) => assert_eq!(error.code, ErrorCode::UnknownSubscription),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(engine.models().cancels(), 0);
    stream.cancel().await.unwrap();
}
