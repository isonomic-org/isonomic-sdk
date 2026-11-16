//! Hello in a headless window, against the in-memory engine.

use isonomic_app::ui;

use hello_app::{Hello, Phase};
use isonomic_app::testing::{wait_until, TestHost};
use isonomic_app::Attention;
use isonomic_proto::{ErrorCode, ModelChunk, WireError};
use isonomic_testkit::Step;
use ui::TestAppContext;

#[ui::test]
fn it_lists_the_models_and_streams_one_reply(cx: &mut TestAppContext) {
    let host = TestHost::new();
    let open = host.open::<Hello>(cx);
    let hello = open.root::<Hello>(cx);
    wait_until(cx, &hello, |hello| *hello.phase() == Phase::Done);

    hello.read_with(cx, |hello, _| {
        let ids: Vec<&str> = hello.models().iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["chat-small", "embed-small"]);
        assert_eq!(hello.reply(), "You said: Say hello");
    });
    assert_eq!(*open.attention.borrow(), Attention::Dot);

    // It drew, and every call went out under its own token.
    open.window
        .update(cx, |_, window, _| window.refresh())
        .unwrap();
    cx.run_until_parked();
    let calls = host.engine().calls();
    let mine: Vec<&str> = calls
        .iter()
        .filter(|c| c.app.as_deref() == Some("com.isonomic.hello"))
        .map(|c| c.method.as_str())
        .collect();
    assert_eq!(mine, ["models.list", "models.stream"]);
}

#[ui::test]
fn the_reply_arrives_chunk_by_chunk_and_failures_show(cx: &mut TestAppContext) {
    let host = TestHost::new();
    let crashed = WireError::new(ErrorCode::ModelUnavailable, "the model worker crashed");
    host.engine().models().script(
        "chat-small",
        vec![Step::Chunk(ModelChunk::text("Hel")), Step::Fail(crashed)],
    );
    let open = host.open::<Hello>(cx);
    let hello = open.root::<Hello>(cx);
    wait_until(cx, &hello, |hello| {
        matches!(hello.phase(), Phase::Failed(_))
    });
    hello.read_with(cx, |hello, _| {
        assert_eq!(hello.reply(), "Hel");
        assert_eq!(
            *hello.phase(),
            Phase::Failed("the model worker crashed".into())
        );
    });
}

#[ui::test]
fn the_window_routes_search_and_deep_links_to_the_app(cx: &mut TestAppContext) {
    let host = TestHost::new();
    let open = host.open::<Hello>(cx);
    let hello = open.root::<Hello>(cx);
    wait_until(cx, &hello, |hello| *hello.phase() == Phase::Done);

    let tab = open.tab(cx);
    let hits = tab.update(cx, |tab, cx| tab.app().search("chat", cx));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].link, "isonomic://hello/models/chat-small");
    let handled = tab.update(cx, |tab, cx| tab.app().open_link(&hits[0].link, cx));
    assert!(handled);
    assert!(!tab.update(cx, |tab, cx| tab.app().open_link("isonomic://other/x", cx)));
    hello.read_with(cx, |hello, _| {
        assert_eq!(hello.opened(), ["isonomic://hello/models/chat-small"]);
    });
}

#[ui::test]
fn settings_are_kept_per_app(cx: &mut TestAppContext) {
    let host = TestHost::new();
    let (hello, _) = host.context::<Hello>();
    hello.settings().set("greeting", &"hi");
    assert_eq!(
        hello.settings().get::<String>("greeting").as_deref(),
        Some("hi")
    );
    use isonomic_app::SettingsStore;
    assert_eq!(host.settings().get("com.isonomic.other", "greeting"), None);
    hello.settings().remove("greeting");
    assert_eq!(hello.settings().get::<String>("greeting"), None);
    let _ = cx;
}
