// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The app platform: the trait every first-party Isonomic app implements,
//! and what the window gives each app.
//!
//! An app is a crate. It describes itself with a [`Manifest`] (identity,
//! icon, capabilities, schemas) and builds its root view from an
//! [`AppContext`]: a client scoped to the app's capability token, the app's
//! own settings, and a way to ask for the user's attention. The window links
//! app crates and registers them with [`register`]; it never hands an app
//! anything else.
//!
//! ```ignore
//! impl isonomic_app::App for Notes {
//!     fn manifest() -> Manifest { /* id, name, version, icon, capabilities */ }
//!     fn build(cx: AppContext, window: &mut Window, app: &mut gpui::App) -> AnyView {
//!         app.new(|view| Notes::new(cx, window, view)).into()
//!     }
//! }
//! ```
//!
//! This is the only SDK crate that depends on Isonomic UI, and it hands it
//! on as [`ui`]: apps and the window take every component from there and
//! never depend on isonomic-ui themselves. The SDK's `UI_REF` is the one pin.
//! The shared toolkits ride the same way: [`editor`] and [`node_graph`],
//! behind features of those names.

mod context;
mod host;
mod requests;
pub mod shell;
#[cfg(feature = "test-support")]
pub mod testing;
mod user_action;

use isonomic_ui as gpui;

/// Isonomic UI, the components every app's front end is made of, at the
/// commit in the SDK's `UI_REF`. Its macros find it here by themselves: an
/// app that depends on `isonomic-app` alone needs no alias.
///
/// ```ignore
/// use isonomic_app::ui::{self, div, IntoElement, ParentElement, Render};
///
/// #[derive(IntoElement)]
/// struct Badge;
///
/// #[ui::test]
/// fn renders(cx: &mut ui::TestAppContext) {}
/// ```
///
/// The app's workspace root needs the same `gpui-pre-macros` patch line as
/// the SDK's. The older `use isonomic_app::ui as gpui;` still works.
pub use isonomic_ui as ui;

/// Isonomic UI's extra components: the prompt composer, the thinking
/// indicator, approval and tool cards, and more. Feature `extra`.
#[cfg(feature = "extra")]
pub use isonomic_ui_extra as extra;

/// The editor toolkit, reached through the SDK. Reserved: empty until the
/// toolkit exists, so turning the feature on later breaks no app.
#[cfg(feature = "editor")]
pub mod editor {}

/// The node-graph toolkit, reached through the SDK. Reserved: empty until
/// the toolkit exists, so turning the feature on later breaks no app.
#[cfg(feature = "node-graph")]
pub mod node_graph {}

pub use context::{AppContext, Attention, MemorySettings, Settings, SettingsStore};
pub use host::{register, LoadedApp, Registration, Tab};
pub use isonomic_client::{AppClient, ClientError, ModelStream, Models};
pub use isonomic_proto::manifest::{BlobAccess, ConnectionAccess, ModelAccess, Schema, SqlAccess};
pub use isonomic_proto::{Capabilities, Manifest};
pub use requests::{Problem, RecordedRequests, WindowRequest, WindowRequests, REPORT_INTERVAL};
pub use shell::{Shell, Sidebar};
pub use user_action::{ShellUserActions, UserActionError, UserActionFuture, UserActionSource};

/// One result for the window's global search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub title: String,
    /// A second line: where the hit is, or a snippet.
    pub detail: Option<String>,
    /// The deep link that opens it, handed back to [`App::open_link`].
    pub link: String,
}

/// The trait every first-party app implements.
///
/// `build` returns the app's root view. When that view is an
/// `Entity<Self>`, the window routes the hooks below to it; an app whose
/// root is some other view simply gets the defaults.
pub trait App: 'static {
    /// Identity, icon and capabilities. The window shows it to the user,
    /// and the engine derives the app's token from it.
    fn manifest() -> Manifest;

    /// Build the root view. Called on first open, and again after the tab
    /// was unloaded.
    fn build(cx: AppContext, window: &mut gpui::Window, app: &mut gpui::App) -> gpui::AnyView;

    /// Open a deep link addressed to this app. `true` if it was handled.
    fn open_link(&mut self, _url: &str) -> bool {
        false
    }

    /// Whether the app places the window's shell header
    /// ([`AppContext::shell_header`]) at the top of its sidebar and reports
    /// its sidebar with [`AppContext::set_sidebar`]. `true` and the window
    /// draws no rail beside the app; `false`, the default, and it keeps it.
    fn hosts_shell_header() -> bool {
        false
    }

    /// Answer the window's global search.
    fn search(&mut self, _q: &str) -> Vec<SearchHit> {
        vec![]
    }

    /// The tab is about to be unloaded to free memory. Everything the user
    /// did is already in the engine; drop caches and subscriptions here.
    fn on_unload(&mut self) {}
}
