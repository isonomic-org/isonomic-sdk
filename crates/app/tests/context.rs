// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

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

// The person's token for an approval: the window decides, through its
// source, whether the person is acting in the app; the app passes the
// token to `decide`.
mod user_action {
    use std::sync::{Arc, Mutex};

    use isonomic_app::{
        AppContext, MemorySettings, ShellUserActions, UserActionError, UserActionSource,
    };
    use isonomic_client::AppClient;
    use isonomic_proto::agents::schema::{Concurrency, Effect, ToolKind, Value};
    use isonomic_proto::agents::{ApprovalId, DecisionInput, SessionId, ToolSpec};
    use isonomic_proto::manifest::AgentsAccess;
    use isonomic_proto::{Capabilities, Manifest};
    use isonomic_testkit::TestEngine;

    const CHAT: &str = "com.isonomic.chat";

    fn manifest() -> Manifest {
        Manifest {
            id: CHAT.into(),
            name: "Chat".into(),
            version: "0.1.0".into(),
            icon: "chat".into(),
            capabilities: Capabilities {
                agents: Some(AgentsAccess {}),
                ..Default::default()
            },
            schemas: vec![],
        }
    }

    /// Chat's client and a session with one approval waiting.
    async fn waiting(engine: &TestEngine) -> (AppClient, SessionId, ApprovalId) {
        let client = engine.client().scoped(engine.grant(&manifest()));
        let agents = client.agents();
        let session = agents.create("test").await.unwrap();
        let post = ToolSpec {
            name: "post".into(),
            description: "The post tool.".into(),
            input_schema: Value::object([("type", Value::from("object"))]),
            kind: ToolKind::Client,
            effects: vec![Effect::ActsOutward],
            concurrency: Concurrency::Serial,
        };
        agents
            .register_client_tools(&session, vec![post])
            .await
            .unwrap();
        engine
            .agents()
            .with(|host| host.model.calls(&[("post", "{\"text\":\"hi\"}")]));
        agents.send_text(&session, "Post hi.").await.unwrap();
        let pending = agents.approvals(None).await.unwrap();
        let approval = pending[0].request.approval.clone();
        (client, session, approval)
    }

    fn context(client: AppClient, source: Option<Arc<dyn UserActionSource>>) -> AppContext {
        let (cx, _) = AppContext::new(CHAT, client, Arc::new(MemorySettings::default()));
        match source {
            Some(source) => cx.with_user_actions(source),
            None => cx,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_window_mints_only_while_the_person_acts_in_the_app() {
        let engine = TestEngine::new();
        let (client, session, approval) = waiting(&engine).await;
        let acting = Arc::new(Mutex::new(false));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let (now, seen) = (acting.clone(), asked.clone());
        let source = ShellUserActions::new(engine.client().agents(), move |app| {
            seen.lock().unwrap().push(app.to_string());
            *now.lock().unwrap()
        });
        let cx = context(client.clone(), Some(Arc::new(source)));

        let refused = cx.user_action(&session, &approval).await;
        assert_eq!(refused, Err(UserActionError::NoUserInput));
        assert_eq!(refused.unwrap_err().code(), "no_user_input");

        *acting.lock().unwrap() = true;
        let token = cx.user_action(&session, &approval).await.unwrap();
        assert_eq!(
            *asked.lock().unwrap(),
            [CHAT, CHAT],
            "the window is told the app"
        );
        client
            .agents()
            .decide(&session, &approval, DecisionInput::Allow, Some(&token))
            .await
            .expect("the minted token decides");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_window_decides_when_asked_not_when_awaited() {
        let engine = TestEngine::new();
        let (client, session, approval) = waiting(&engine).await;
        let acting = Arc::new(Mutex::new(true));
        let now = acting.clone();
        let source = ShellUserActions::new(engine.client().agents(), move |_| *now.lock().unwrap());
        let cx = context(client, Some(Arc::new(source)));
        let asked = cx.user_action(&session, &approval);
        *acting.lock().unwrap() = false;
        assert!(
            asked.await.is_ok(),
            "asked during the input, answered after"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_refused_mint_keeps_the_engines_code() {
        let engine = TestEngine::new();
        let (client, session, approval) = waiting(&engine).await;
        // An app's agents, not the shell's: the engine refuses the mint.
        let source = ShellUserActions::new(client.agents(), |_| true);
        let cx = context(client, Some(Arc::new(source)));
        let refused = cx.user_action(&session, &approval).await.unwrap_err();
        assert_eq!(refused.code(), "engine");
        assert!(
            matches!(refused, UserActionError::Engine { .. }),
            "{refused:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn without_a_source_no_token_comes() {
        let engine = TestEngine::new();
        let (client, session, approval) = waiting(&engine).await;
        let cx = context(client, None);
        let none = cx.user_action(&session, &approval).await;
        assert_eq!(none, Err(UserActionError::Unavailable));
        assert_eq!(none.unwrap_err().code(), "unavailable");
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn the_test_host_stands_in_for_the_window_and_can_refuse() {
        use isonomic_app::testing::TestHost;

        struct Chat;
        impl isonomic_app::App for Chat {
            fn manifest() -> Manifest {
                manifest()
            }
            fn build(
                _: AppContext,
                _: &mut isonomic_app::ui::Window,
                _: &mut isonomic_app::ui::App,
            ) -> isonomic_app::ui::AnyView {
                unreachable!("never opened here")
            }
        }

        let host = TestHost::new();
        let (cx, _) = host.context::<Chat>();
        host.runtime().block_on(async {
            let agents = cx.client().agents();
            let session = agents.create("test").await.unwrap();
            let post = ToolSpec {
                name: "post".into(),
                description: "The post tool.".into(),
                input_schema: Value::object([("type", Value::from("object"))]),
                kind: ToolKind::Client,
                effects: vec![Effect::ActsOutward],
                concurrency: Concurrency::Serial,
            };
            agents
                .register_client_tools(&session, vec![post])
                .await
                .unwrap();
            host.engine()
                .agents()
                .with(|h| h.model.calls(&[("post", "{\"text\":\"hi\"}")]));
            agents.send_text(&session, "Post hi.").await.unwrap();
            let approval = agents.approvals(None).await.unwrap()[0]
                .request
                .approval
                .clone();

            let outside = cx.user_action(&session, &approval).await;
            assert_eq!(
                outside,
                Err(UserActionError::NoUserInput),
                "no click, no token"
            );

            let late = host.user_input(|| {
                let (cx, session, approval) = (cx.clone(), session.clone(), approval.clone());
                async move { cx.user_action(&session, &approval).await }
            });
            assert_eq!(
                late.await,
                Err(UserActionError::NoUserInput),
                "asked after the handler returned"
            );

            let token = host
                .user_input(|| cx.user_action(&session, &approval))
                .await
                .unwrap();
            agents
                .decide(&session, &approval, DecisionInput::Allow, Some(&token))
                .await
                .expect("the test host's token decides");
        });
        assert_eq!(host.user_actions_asked(), [CHAT, CHAT, CHAT]);
    }
}
