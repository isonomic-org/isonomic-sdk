//! The context an app is given, without a window.

use std::sync::Arc;

use isonomic_app::{AppContext, Attention, MemorySettings, SettingsStore};
use isonomic_proto::Token;
use isonomic_testkit::TestEngine;

#[tokio::test(flavor = "multi_thread")]
async fn attention_reaches_the_window_and_settings_stay_per_app() {
    let engine = TestEngine::new();
    let store = Arc::new(MemorySettings::default());
    let client = engine.client().scoped(Token::new("t"));
    let (notes, mut rail) = AppContext::new("com.isonomic.notes", client.clone(), store.clone());
    let (chat, _) = AppContext::new("com.isonomic.chat", client, store.clone());

    assert_eq!(notes.id(), "com.isonomic.notes");
    assert_eq!(*rail.borrow(), Attention::None);
    notes.set_attention(Attention::Count(3));
    rail.changed().await.unwrap();
    assert_eq!(*rail.borrow_and_update(), Attention::Count(3));
    assert_eq!(notes.clone().attention(), Attention::Count(3));

    notes.settings().set("width", &320u32);
    chat.settings().set("width", &640u32);
    assert_eq!(notes.settings().get::<u32>("width"), Some(320));
    assert_eq!(chat.settings().get::<u32>("width"), Some(640));
    assert_eq!(
        notes.settings().get::<String>("width"),
        None,
        "wrong type reads as absent"
    );
    assert_eq!(
        store.get("com.isonomic.notes", "width"),
        Some(serde_json::json!(320))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_asks_the_window_and_the_window_decides() {
    use isonomic_app::{RecordedRequests, WindowRequest};
    let engine = TestEngine::new();
    let store = Arc::new(MemorySettings::default());
    let client = engine.client().scoped(Token::new("t"));
    let (alone, _) = AppContext::new("com.isonomic.chat", client.clone(), store.clone());
    let install = WindowRequest::InstallModel {
        model: "chat-small".into(),
    };
    assert!(!alone.request(install.clone()), "no window, no taker");
    let window = Arc::new(RecordedRequests::default());
    let (chat, _) = AppContext::new("com.isonomic.chat", client, store);
    let chat = chat.with_window(window.clone());
    assert!(chat.request(install.clone()));
    assert_eq!(
        window.all(),
        vec![("com.isonomic.chat".to_string(), install)]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_app_reports_a_problem_at_most_once_a_minute() {
    use isonomic_app::{Problem, RecordedRequests, WindowRequest, WindowRequests};
    let engine = TestEngine::new();
    let store = Arc::new(MemorySettings::default());
    let client = engine.client().scoped(Token::new("t"));
    let problem = |summary: &str| Problem {
        summary: summary.into(),
        detail: Some("step: sync".into()),
    };
    let (alone, _) = AppContext::new("com.isonomic.notes", client.clone(), store.clone());
    assert!(!alone.report(problem("no window")), "no window, no taker");
    let window = Arc::new(RecordedRequests::default());
    let (notes, _) = AppContext::new("com.isonomic.notes", client.clone(), store.clone());
    let notes = notes.with_window(window.clone());
    assert!(notes.report(problem("sync failed")));
    assert!(
        !notes.clone().report(problem("again")),
        "one a minute, across clones"
    );
    let (chat, _) = AppContext::new("com.isonomic.chat", client.clone(), store.clone());
    assert!(
        chat.with_window(window.clone())
            .report(problem("send failed")),
        "per app"
    );
    assert_eq!(
        window.reports(),
        vec![
            ("com.isonomic.notes".to_string(), problem("sync failed")),
            ("com.isonomic.chat".to_string(), problem("send failed")),
        ]
    );

    /// A window that refuses reports.
    struct Busy;
    impl WindowRequests for Busy {
        fn request(&self, _app: &str, request: WindowRequest) -> bool {
            !matches!(request, WindowRequest::Report(_))
        }
    }
    let (busy, _) = AppContext::new("com.isonomic.files", client, store);
    let busy = busy.with_window(Arc::new(Busy));
    assert!(!busy.report(problem("refused")));
    let recorded = Arc::new(RecordedRequests::default());
    let busy = busy.with_window(recorded.clone());
    assert!(
        busy.report(problem("taken")),
        "a refusal does not use up the minute"
    );
}
