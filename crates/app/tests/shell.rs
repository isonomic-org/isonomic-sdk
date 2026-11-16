//! The shell header slot: an app that does not host it keeps the rail, a
//! context without a shell draws nothing, and the sidebar's state reaches
//! the window's subscription.

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
