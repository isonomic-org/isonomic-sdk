//! The context an app is given, without a window; the shell header's
//! slot; and the extra components drawn through `isonomic_app::extra`.
//!
//! One test binary on purpose: each links the whole interface stack.

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

// The shell header slot: an app that does not host it keeps the rail, a
// context without a shell draws nothing, and the sidebar's state reaches
// the window's subscription.
mod shell {
    use std::sync::Arc;

    use isonomic_app::{register, App, AppContext, Manifest, MemorySettings, Sidebar};
    use isonomic_proto::Token;
    use isonomic_testkit::TestEngine;

    struct Plain;

    impl App for Plain {
        fn manifest() -> Manifest {
            Manifest {
                id: "com.isonomic.plain".into(),
                name: "Plain".into(),
                version: "0.1.0".into(),
                icon: "sparkle".into(),
                capabilities: Default::default(),
                schemas: vec![],
            }
        }

        fn build(
            _: AppContext,
            _: &mut isonomic_app::ui::Window,
            _: &mut isonomic_app::ui::App,
        ) -> isonomic_app::ui::AnyView {
            unreachable!("never opened here")
        }
    }

    #[test]
    fn an_app_that_does_not_host_the_header_keeps_the_rail() {
        assert!(!register::<Plain>().hosts_shell_header());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_sidebar_reaches_the_window_once_per_change() {
        let engine = TestEngine::new();
        let client = engine.client().scoped(Token::new("t"));
        let (cx, _) = AppContext::new(
            "com.isonomic.plain",
            client,
            Arc::new(MemorySettings::default()),
        );
        let mut window = cx.sidebar_changes();
        assert_eq!(cx.sidebar(), Sidebar::Expanded);
        assert_eq!(
            cx.shell_inset(),
            isonomic_app::ui::px(0.),
            "no shell, no inset"
        );

        cx.clone().set_sidebar(Sidebar::Collapsed);
        window.changed().await.unwrap();
        assert_eq!(*window.borrow_and_update(), Sidebar::Collapsed);
        cx.set_sidebar(Sidebar::Collapsed);
        assert!(
            !window.has_changed().unwrap(),
            "the same state is not sent again"
        );
    }
}

// The extra components reach an app through `isonomic_app::extra`: the
// wordmark, drawn in a headless window.
#[cfg(all(feature = "extra", feature = "test-support"))]
mod extra {
    use isonomic_app::extra::wordmark::{Wordmark, NAME};
    use isonomic_app::ui;

    use ui::{div, IntoElement, ParentElement, Render, TestAppContext};

    struct Shows;

    impl Render for Shows {
        fn render(&mut self, _: &mut ui::Window, _: &mut ui::Context<Self>) -> impl IntoElement {
            div()
                .child(Wordmark::new())
                .child(Wordmark::product("Chat"))
        }
    }

    #[ui::test]
    fn the_wordmark_reaches_apps_and_draws(cx: &mut TestAppContext) {
        assert_eq!(NAME, "Isonomic");
        assert_eq!(Wordmark::product("Chat").text(), "Isonomic Chat");
        cx.update(ui::init);
        let window = cx.add_window(|_, _| Shows);
        window.update(cx, |_, window, _| window.refresh()).unwrap();
        cx.run_until_parked();
    }
}
