//! Hello hosts the window's shell header at the top of its sidebar and
//! tells the window when the sidebar folds.

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
