//! Model downloads, the recommended first model and the hardware profile:
//! progress to installed, attaching from a second window, cancel, failure,
//! the shell-only rule, and the memory floor.

use std::time::Duration;

use isonomic_client::{ClientError, DownloadStream};
use isonomic_proto::manifest::ModelAccess;
use isonomic_proto::models::{DownloadOutcome, DownloadProgress, ModelState};
use isonomic_proto::{
    Capabilities, ErrorCode, HardwareProfile, Manifest, Message, ModelInfo, StreamRequest,
    WireError,
};
use isonomic_testkit::{DownloadStep, Gate, TestEngine, MEMORY_FLOOR};

const SIZE: u64 = 2 << 30;

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

async fn next(stream: &mut DownloadStream) -> DownloadProgress {
    let next = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
    next.expect("no event within 5 s")
        .expect("the stream ended")
        .expect("the connection dropped")
}

async fn to_end(stream: &mut DownloadStream) -> Vec<DownloadProgress> {
    let mut seen = vec![];
    loop {
        let event = next(stream).await;
        let last = event.outcome.is_some();
        seen.push(event);
        if last {
            return seen;
        }
    }
}

fn hello() -> StreamRequest {
    StreamRequest {
        messages: vec![Message::user("Hi")],
        ..Default::default()
    }
}

#[tokio::test]
async fn a_download_reports_progress_then_the_model_is_installed() {
    let engine = TestEngine::new();
    engine.models().uninstall("chat-small");
    let models = engine.client().models();
    assert_eq!(
        models.info("chat-small").await.unwrap().state,
        ModelState::NotInstalled
    );
    let refused = models.stream(hello()).await.map(|_| ());
    assert_eq!(code(refused), ErrorCode::ModelUnavailable);
    let mut download = models.download("chat-small").await.unwrap();
    assert_eq!(download.total_bytes(), Some(SIZE));
    let events = to_end(&mut download).await;
    let received: Vec<u64> = events.iter().map(|e| e.received_bytes).collect();
    assert!(received.windows(2).all(|w| w[0] <= w[1]), "{received:?}");
    let last = events.last().unwrap();
    assert_eq!(last.outcome, Some(DownloadOutcome::Installed));
    assert_eq!(last.received_bytes, SIZE);
    assert!(download.next().await.is_none());
    assert!(engine.models().installed("chat-small"));
    assert_eq!(
        models.info("chat-small").await.unwrap().state,
        ModelState::Stopped
    );
    models.stream(hello()).await.unwrap().text().await.unwrap();
}

#[tokio::test]
async fn a_second_window_attaches_to_the_running_download() {
    let engine = TestEngine::new();
    let gate = Gate::new();
    engine.models().uninstall("chat-small");
    let steps = vec![
        DownloadStep::Progress(SIZE / 2),
        DownloadStep::Hold(gate.clone()),
        DownloadStep::Progress(SIZE),
    ];
    engine.models().script_download("chat-small", steps);
    let first = engine.client().models();
    let mut one = first.download("chat-small").await.unwrap();
    while next(&mut one).await.received_bytes < SIZE / 2 {}
    assert_eq!(
        first.info("chat-small").await.unwrap().state,
        ModelState::Downloading
    );
    let second = engine.client().models();
    let mut two = second.download("chat-small").await.unwrap();
    assert_eq!(
        next(&mut two).await.received_bytes,
        SIZE / 2,
        "where it stands"
    );
    gate.open();
    for stream in [&mut one, &mut two] {
        let last = to_end(stream).await.pop().unwrap();
        assert_eq!(last.outcome, Some(DownloadOutcome::Installed));
    }
}

#[tokio::test]
async fn cancel_stops_the_download_for_every_window() {
    let engine = TestEngine::new();
    let gate = Gate::new();
    engine.models().uninstall("chat-small");
    let steps = vec![DownloadStep::Progress(1024), DownloadStep::Hold(gate)];
    engine.models().script_download("chat-small", steps);
    let models = engine.client().models();
    let mut one = models.download("chat-small").await.unwrap();
    let mut two = engine
        .client()
        .models()
        .download("chat-small")
        .await
        .unwrap();
    while next(&mut one).await.received_bytes < 1024 {}
    models.cancel_download("chat-small").await.unwrap();
    for stream in [&mut one, &mut two] {
        let last = to_end(stream).await.pop().unwrap();
        assert_eq!(last.outcome, Some(DownloadOutcome::Cancelled));
    }
    assert!(!engine.models().installed("chat-small"));
    let again = models.cancel_download("chat-small").await;
    assert_eq!(code(again), ErrorCode::NotDownloading);
    assert_eq!(
        models.info("chat-small").await.unwrap().state,
        ModelState::NotInstalled
    );
}

#[tokio::test]
async fn a_failed_download_says_why_and_leaves_the_model_uninstalled() {
    let engine = TestEngine::new();
    engine.models().uninstall("chat-small");
    let cut = WireError::new(ErrorCode::ModelUnavailable, "the source stopped answering");
    let steps = vec![
        DownloadStep::Progress(SIZE / 4),
        DownloadStep::Fail(cut.clone()),
    ];
    engine.models().script_download("chat-small", steps);
    let mut download = engine
        .client()
        .models()
        .download("chat-small")
        .await
        .unwrap();
    let last = to_end(&mut download).await.pop().unwrap();
    assert_eq!(last.outcome, Some(DownloadOutcome::Failed));
    assert_eq!(last.error, Some(cut));
    assert_eq!(last.received_bytes, SIZE / 4);
    assert!(!engine.models().installed("chat-small"));
}

#[tokio::test]
async fn an_installed_model_reports_installed_at_once() {
    let engine = TestEngine::new();
    let mut download = engine
        .client()
        .models()
        .download("chat-small")
        .await
        .unwrap();
    let events = to_end(&mut download).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, Some(DownloadOutcome::Installed));
}

#[tokio::test]
async fn only_local_known_models_download() {
    let engine = TestEngine::new();
    engine.models().add(ModelInfo {
        id: "remote-large".into(),
        name: "Remote".into(),
        context_length: None,
        capabilities: vec!["chat".into()],
        remote: true,
    });
    let models = engine.client().models();
    assert_eq!(
        code(models.download("remote-large").await),
        ErrorCode::Unsupported
    );
    assert_eq!(
        code(models.download("chat-huge").await),
        ErrorCode::UnknownModel
    );
}

#[tokio::test]
async fn apps_may_not_install_models() {
    let engine = TestEngine::new();
    let manifest = Manifest {
        id: "com.isonomic.hello".into(),
        name: "Hello".into(),
        version: "0.1.0".into(),
        icon: "sparkle".into(),
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
    let app = engine.client().scoped(engine.grant(&manifest)).models();
    assert_eq!(
        code(app.download("chat-small").await),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        code(app.cancel_download("chat-small").await),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        code(app.recommended(None).await),
        ErrorCode::CapabilityDenied
    );
}

#[tokio::test]
async fn the_recommendation_follows_the_profile_and_the_memory_floor() {
    let engine = TestEngine::new();
    let models = engine.client().models();
    let standard = models.recommended(None).await.unwrap();
    assert_eq!(
        (standard.model.as_str(), standard.profile.as_str()),
        ("chat-small", "standard")
    );
    assert_eq!(standard.size_bytes, Some(SIZE));
    let small = HardwareProfile {
        profile: "low".into(),
        total_memory_bytes: 8 << 30,
        available_memory_bytes: 4 << 30,
    };
    engine.set_profile(small.clone());
    assert_eq!(engine.client().profile().await.unwrap(), small);
    match models.recommended(None).await {
        Err(ClientError::Engine(error)) => {
            assert_eq!(error.code, ErrorCode::InsufficientMemory);
            let memory = error.memory.expect("the numbers");
            assert_eq!(memory.required_bytes, MEMORY_FLOOR);
            assert_eq!(memory.available_bytes, 8 << 30);
        }
        other => panic!("expected insufficient_memory, got {other:?}"),
    }
    let high = models.recommended(Some("high")).await.unwrap();
    assert_eq!(high.profile, "high");
    engine.models().recommend("low", Some("chat-small"));
    assert_eq!(models.recommended(None).await.unwrap().model, "chat-small");
}

#[tokio::test]
async fn an_unknown_size_is_absent_not_zero() {
    let engine = TestEngine::new();
    engine.models().uninstall("chat-small");
    engine.models().set_size("chat-small", None);
    let models = engine.client().models();
    let mut download = models.download("chat-small").await.unwrap();
    assert_eq!(download.total_bytes(), None);
    let events = to_end(&mut download).await;
    assert!(events.iter().all(|e| e.total_bytes.is_none()));
    let last = events.last().unwrap();
    assert_eq!(last.outcome, Some(DownloadOutcome::Installed));
    assert!(last.received_bytes > 0);
    assert_eq!(models.recommended(None).await.unwrap().size_bytes, None);
}

#[tokio::test]
async fn a_dropped_connection_leaves_the_download_running_to_resume() {
    let engine = TestEngine::new();
    let gate = Gate::new();
    engine.models().uninstall("chat-small");
    let steps = vec![
        DownloadStep::Progress(SIZE / 4),
        DownloadStep::Hold(gate.clone()),
        DownloadStep::Progress(SIZE),
    ];
    engine.models().script_download("chat-small", steps);
    let client = engine.client();
    let models = client.models();
    let mut download = models.download("chat-small").await.unwrap();
    while next(&mut download).await.received_bytes < SIZE / 4 {}
    engine.go_offline();
    let dropped = tokio::time::timeout(Duration::from_secs(5), download.next()).await;
    assert!(
        matches!(dropped, Ok(Some(Err(ClientError::Disconnected)))),
        "{dropped:?}"
    );
    assert!(download.next().await.is_none());
    engine.go_online();
    let mut live = client.live();
    let back = live.wait_for(|view| view.connected);
    tokio::time::timeout(Duration::from_secs(5), back)
        .await
        .expect("reconnected within 5 s")
        .unwrap();
    drop(live);
    let mut resumed = models.download("chat-small").await.unwrap();
    assert_eq!(
        next(&mut resumed).await.received_bytes,
        SIZE / 4,
        "where it stood"
    );
    gate.open();
    let last = to_end(&mut resumed).await.pop().unwrap();
    assert_eq!(last.outcome, Some(DownloadOutcome::Installed));
    assert!(engine.models().installed("chat-small"));
}
