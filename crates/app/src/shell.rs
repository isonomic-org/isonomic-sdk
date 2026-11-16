//! The window's part of an app's sidebar.
//!
//! The window draws no rail of its own beside an app that hosts the shell
//! header. Instead, the app places [`AppContext::shell_header`] at the top
//! of its sidebar, next to its sidebar toggle, and tells the window whether
//! the sidebar is collapsed with [`AppContext::set_sidebar`]. The window
//! fills the slot: the plain wordmark with one app ([`ProductWordmark`]),
//! its app switcher with more.
//!
//! An app opts in with [`App::hosts_shell_header`](crate::App::hosts_shell_header).
//! An app that does not keeps the window's rail.
//!
//! [`AppContext::shell_header`]: crate::AppContext::shell_header
//! [`AppContext::set_sidebar`]: crate::AppContext::set_sidebar

#[cfg(feature = "extra")]
use isonomic_ui::IntoElement;
use isonomic_ui::{self as gpui, px, AnyElement, Pixels, Window};

/// How an app's sidebar stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sidebar {
    /// Open: the toggle at the left of the top row, the shell header beside
    /// it.
    #[default]
    Expanded,
    /// Folded to a narrow rail: the shell header (the app's icon) at the
    /// top, the toggle under it.
    Collapsed,
}

/// The window's side of the shell header. The window supplies one with
/// [`AppContext::with_shell`](crate::AppContext::with_shell).
pub trait Shell: Send + Sync + 'static {
    /// The element for the top of `app`'s sidebar, drawn for its current
    /// state. Called on every render of the app's sidebar.
    fn header(
        &self,
        app: &str,
        sidebar: Sidebar,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> AnyElement;

    /// The room the app leaves for the window's own controls (the traffic
    /// lights): at the left of the top row when expanded, above the header
    /// when collapsed. None by default.
    fn inset(&self, _sidebar: Sidebar) -> Pixels {
        px(0.)
    }
}

/// The header for a window with one app: "Isonomic" and the app's name
/// (Isonomic UI's `Wordmark::product`) when expanded, the app's icon when
/// collapsed. No menu. The window may use it as is; the test host does.
/// Feature `extra`.
#[cfg(feature = "extra")]
#[derive(Debug, Clone)]
pub struct ProductWordmark {
    name: String,
    icon: String,
}

#[cfg(feature = "extra")]
impl ProductWordmark {
    /// For an app with this manifest name ("Chat") and icon ("sparkle").
    pub fn new(name: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            icon: icon.into(),
        }
    }

    /// For the app this manifest describes.
    pub fn of(manifest: &isonomic_proto::Manifest) -> Self {
        Self::new(manifest.name.clone(), manifest.icon.clone())
    }

    /// What the expanded header reads: "Isonomic Chat".
    pub fn text(&self) -> String {
        format!("Isonomic {}", self.name.trim())
    }
}

#[cfg(feature = "extra")]
impl Shell for ProductWordmark {
    fn header(&self, _: &str, sidebar: Sidebar, _: &mut Window, _: &mut gpui::App) -> AnyElement {
        use isonomic_ui::component::Icon;
        use isonomic_ui_extra::wordmark::Wordmark;
        match sidebar {
            Sidebar::Expanded => Wordmark::product(self.name.clone()).into_any_element(),
            Sidebar::Collapsed => Icon::default()
                .path(format!("icons/{}.svg", self.icon))
                .into_any_element(),
        }
    }
}

#[cfg(all(test, feature = "extra"))]
mod tests {
    use super::*;

    #[test]
    fn the_one_app_header_reads_the_product_form() {
        let header = ProductWordmark::new("Chat", "message-circle");
        assert_eq!(header.text(), "Isonomic Chat");
        assert_eq!(header.inset(Sidebar::Collapsed), px(0.));
        assert_eq!(Sidebar::default(), Sidebar::Expanded);
    }
}
