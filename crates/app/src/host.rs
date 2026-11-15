//! The window's side: registering app crates, opening them as tabs, and
//! routing hooks to them.

use isonomic_proto::Manifest;
use isonomic_ui::{self as gpui, AnyView, AppContext as _, Context, IntoElement, Render, Window};

use crate::{App, AppContext, SearchHit};

type Build = fn(AppContext, &mut Window, &mut gpui::App) -> AnyView;
type OpenLink = fn(&AnyView, &str, &mut gpui::App) -> bool;
type Search = fn(&AnyView, &str, &mut gpui::App) -> Vec<SearchHit>;
type Unload = fn(&AnyView, &mut gpui::App);

/// An app crate linked into the window. Make one with [`register`].
#[derive(Clone)]
pub struct Registration {
    manifest: Manifest,
    build: Build,
    open_link: OpenLink,
    search: Search,
    unload: Unload,
}

fn open_link<A: App>(view: &AnyView, url: &str, cx: &mut gpui::App) -> bool {
    match view.clone().downcast::<A>() {
        Ok(app) => app.update(cx, |app, _| app.open_link(url)),
        Err(_) => false,
    }
}

fn search<A: App>(view: &AnyView, q: &str, cx: &mut gpui::App) -> Vec<SearchHit> {
    match view.clone().downcast::<A>() {
        Ok(app) => app.update(cx, |app, _| app.search(q)),
        Err(_) => vec![],
    }
}

fn unload<A: App>(view: &AnyView, cx: &mut gpui::App) {
    if let Ok(app) = view.clone().downcast::<A>() {
        app.update(cx, |app, _| app.on_unload());
    }
}

/// Register an app crate with the window.
pub fn register<A: App>() -> Registration {
    Registration {
        manifest: A::manifest(),
        build: A::build,
        open_link: open_link::<A>,
        search: search::<A>,
        unload: unload::<A>,
    }
}

impl Registration {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Build the app's root view, with the context the window made for it
    /// (its token, settings and attention).
    pub fn open(&self, cx: AppContext, window: &mut Window, app: &mut gpui::App) -> LoadedApp {
        LoadedApp {
            id: self.manifest.id.clone(),
            view: (self.build)(cx, window, app),
            open_link: self.open_link,
            search: self.search,
            unload: self.unload,
        }
    }
}

/// An app that is open in a tab.
pub struct LoadedApp {
    id: String,
    view: AnyView,
    open_link: OpenLink,
    search: Search,
    unload: Unload,
}

impl LoadedApp {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The root view, to place in the tab.
    pub fn view(&self) -> AnyView {
        self.view.clone()
    }

    pub fn open_link(&self, url: &str, cx: &mut gpui::App) -> bool {
        (self.open_link)(&self.view, url, cx)
    }

    pub fn search(&self, q: &str, cx: &mut gpui::App) -> Vec<SearchHit> {
        (self.search)(&self.view, q, cx)
    }

    /// Tell the app, then drop it. Reopening builds it again.
    pub fn unload(self, cx: &mut gpui::App) {
        (self.unload)(&self.view, cx);
    }
}

/// The simplest host for one app: a view that draws the app's root. The
/// window's tab host does the same, with the rail around it.
pub struct Tab {
    app: LoadedApp,
}

impl Tab {
    /// Open `registration` in a new entity.
    pub fn open(
        registration: &Registration,
        cx: AppContext,
        window: &mut Window,
        tab: &mut Context<Self>,
    ) -> Self {
        Self {
            app: registration.open(cx, window, tab),
        }
    }

    pub fn app(&self) -> &LoadedApp {
        &self.app
    }

    /// A new tab entity for `registration`, as for a popped-out window.
    pub fn new_entity(
        registration: &Registration,
        cx: AppContext,
        window: &mut Window,
        app: &mut gpui::App,
    ) -> gpui::Entity<Self> {
        app.new(|tab| Self::open(registration, cx, window, tab))
    }
}

impl Render for Tab {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.app.view()
    }
}
