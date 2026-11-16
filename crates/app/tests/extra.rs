//! The extra components reach an app through `isonomic_app::extra`: the
//! wordmark, drawn in a headless window.

#![cfg(all(feature = "extra", feature = "test-support"))]

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
