// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Hello in a headless window, against the in-memory engine; its shell
//! header; and the bundled fonts.
//!
//! One test binary on purpose: each links the whole interface stack.

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

// Hello hosts the window's shell header at the top of its sidebar and
// tells the window when the sidebar folds.
mod shell {
    use isonomic_app::testing::{wait_until, TestHost};
    use isonomic_app::{ui, Sidebar};

    use hello_app::{Hello, Phase};
    use ui::TestAppContext;

    const HELLO: &str = "com.isonomic.hello";

    #[ui::test]
    fn the_sidebar_hosts_the_shell_header_and_reports_folding(cx: &mut TestAppContext) {
        let host = TestHost::new();
        let mut open = host.open::<Hello>(cx);
        assert!(
            open.hosts_shell_header,
            "the window would leave out its rail"
        );
        let hello = open.root::<Hello>(cx);
        wait_until(cx, &hello, |hello| *hello.phase() == Phase::Done);
        open.window
            .update(cx, |_, window, _| window.refresh())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            host.shell_headers().last(),
            Some(&(HELLO.into(), Sidebar::Expanded))
        );
        assert_eq!(*open.sidebar.borrow_and_update(), Sidebar::Expanded);

        hello.update(cx, |hello, cx| hello.toggle_sidebar(cx));
        cx.run_until_parked();
        assert!(open.sidebar.has_changed().unwrap());
        assert_eq!(*open.sidebar.borrow_and_update(), Sidebar::Collapsed);
        assert_eq!(
            host.shell_headers().last(),
            Some(&(HELLO.into(), Sidebar::Collapsed))
        );

        hello.update(cx, |hello, cx| hello.toggle_sidebar(cx));
        cx.run_until_parked();
        assert_eq!(*open.sidebar.borrow_and_update(), Sidebar::Expanded);
    }
}

// The bundled serif reaches an app through `isonomic_app::ui` alone: the
// theme names it, and the font files Isonomic UI's `init` registers carry
// it beside the sans (Light included) and the mono.
//
// The headless test platform's text system lists no fonts, so this checks
// the files `init` hands to `add_fonts` rather than the platform's list.
mod fonts {
    use isonomic_app::ui;

    use ui::component::ActiveTheme;
    use ui::TestAppContext;

    /// Whether a font file names `family`, in its name table's single-byte or
    /// UTF-16 form.
    fn names(file: &[u8], family: &str) -> bool {
        let wide: Vec<u8> = family.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let has = |needle: &[u8]| file.windows(needle.len()).any(|w| w == needle);
        has(family.as_bytes()) || has(&wide)
    }

    #[test]
    fn the_bundled_files_carry_every_family() {
        let files = ui::assets::fonts();
        for family in [ui::assets::SANS, ui::assets::MONO, ui::assets::SERIF] {
            let count = files.iter().filter(|f| names(f, family)).count();
            assert!(count >= 2, "{family}: {count} files");
        }
    }

    /// The wordmark's product word is the sans in Light: one of the files is it.
    #[test]
    fn the_bundled_files_carry_the_sans_in_light() {
        let light = format!("{} Light", ui::assets::SANS);
        let files = ui::assets::fonts();
        assert!(files.iter().any(|f| names(f, &light)), "no {light} file");
    }

    #[ui::test]
    fn the_theme_names_the_bundled_serif(cx: &mut TestAppContext) {
        cx.update(|cx| {
            ui::init(cx);
            assert_eq!(cx.theme().serif_font_family.as_ref(), ui::assets::SERIF);
        });
    }
}
