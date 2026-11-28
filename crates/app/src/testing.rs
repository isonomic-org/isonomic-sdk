// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! A headless host for app tests: the in-memory engine, a token granted
//! from the app's own manifest, and a GPUI test window.
//!
//! The testkit enforces the token, so an app cannot pass its tests while
//! calling something its manifest does not allow.
//!
//! ```ignore
//! use isonomic_app::ui;
//!
//! #[ui::test]
//! fn shows_the_models(cx: &mut TestAppContext) {
//!     let host = TestHost::new();
//!     let open = host.open::<Hello>(cx);
//!     let hello = open.root::<Hello>(cx);
//!     wait_until(cx, &hello, |hello| !hello.models().is_empty());
//! }
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use isonomic_client::Client;
use isonomic_testkit::TestEngine;
use isonomic_ui::{self as gpui, AnyElement, Entity, TestAppContext, Window, WindowHandle};
use tokio::sync::watch;

use crate::shell::{ProductWordmark, Shell, Sidebar};
use crate::{
    register, App, AppContext, Attention, MemorySettings, RecordedRequests, Tab, UserActionError,
    UserActionFuture, UserActionSource, WindowRequest,
};

/// How long [`wait_until`] waits before failing the test.
pub const WAIT: Duration = Duration::from_secs(10);

/// An engine, a shell client on its own Tokio runtime, and app settings in
/// memory. The UI side runs on GPUI's test executor, as in the window.
pub struct TestHost {
    engine: TestEngine,
    client: Client,
    settings: Arc<MemorySettings>,
    requests: Arc<RecordedRequests>,
    headers: Arc<Mutex<Vec<(String, Sidebar)>>>,
    user_actions: Arc<TestUserActions>,
    runtime: tokio::runtime::Runtime,
}

/// The test host's token source: the testkit mints as the window would,
/// only while the person is "acting" (inside [`TestHost::user_input`]);
/// otherwise it refuses with `no_user_input`, as the window does.
struct TestUserActions {
    engine: TestEngine,
    acting: AtomicBool,
    asked: Mutex<Vec<String>>,
}

impl UserActionSource for TestUserActions {
    fn user_action(
        &self,
        app: &str,
        session: &isonomic_proto::agents::SessionId,
        approval: &isonomic_proto::agents::ApprovalId,
    ) -> UserActionFuture {
        self.asked.lock().unwrap().push(app.to_string());
        if !self.acting.load(Ordering::SeqCst) {
            return Box::pin(async { Err(UserActionError::NoUserInput) });
        }
        let token = self
            .engine
            .agents()
            .user_action(Some(app), session, approval);
        Box::pin(async move { Ok(token) })
    }
}

/// The test host's shell: the window's one-app header
/// ([`ProductWordmark`]), with every draw recorded.
struct TestShell {
    header: ProductWordmark,
    drawn: Arc<Mutex<Vec<(String, Sidebar)>>>,
}

impl Shell for TestShell {
    fn header(
        &self,
        app: &str,
        sidebar: Sidebar,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> AnyElement {
        self.drawn.lock().unwrap().push((app.to_string(), sidebar));
        self.header.header(app, sidebar, window, cx)
    }
}

impl Default for TestHost {
    fn default() -> Self {
        Self::new()
    }
}

impl TestHost {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("a Tokio runtime for the client");
        let engine = runtime.block_on(async { TestEngine::new() });
        let client = engine.client_on(runtime.handle());
        let user_actions = Arc::new(TestUserActions {
            engine: engine.clone(),
            acting: AtomicBool::new(false),
            asked: Mutex::default(),
        });
        Self {
            engine,
            user_actions,
            client,
            settings: Arc::new(MemorySettings::default()),
            requests: Arc::new(RecordedRequests::default()),
            headers: Arc::default(),
            runtime,
        }
    }

    /// Script the engine through this.
    pub fn engine(&self) -> &TestEngine {
        &self.engine
    }

    pub fn runtime(&self) -> &tokio::runtime::Handle {
        self.runtime.handle()
    }

    pub fn settings(&self) -> &Arc<MemorySettings> {
        &self.settings
    }

    /// Every request apps made of the window, with the app that made it.
    /// The test host takes each one on.
    pub fn requests(&self) -> Vec<(String, WindowRequest)> {
        self.requests.all()
    }

    /// Every problem apps reported, with the app that reported it.
    pub fn reports(&self) -> Vec<(String, crate::Problem)> {
        self.requests.reports()
    }

    /// Run `act` as the window runs an app's handler for the person's click
    /// or key press: a user-action token asked for inside it is given;
    /// one asked for anywhere else, including inside a future that `act`
    /// starts but that runs later, is refused with `no_user_input`.
    pub fn user_input<T>(&self, act: impl FnOnce() -> T) -> T {
        let before = self.user_actions.acting.swap(true, Ordering::SeqCst);
        let out = act();
        self.user_actions.acting.store(before, Ordering::SeqCst);
        out
    }

    /// Count the person as acting (`true`) or not (`false`, the default)
    /// until changed, for tests that do not model the click.
    pub fn set_user_input(&self, acting: bool) {
        self.user_actions.acting.store(acting, Ordering::SeqCst);
    }

    /// Every app that asked for a user-action token, oldest first, whether
    /// or not it got one.
    pub fn user_actions_asked(&self) -> Vec<String> {
        self.user_actions.asked.lock().unwrap().clone()
    }

    /// Every time an app drew the shell header, oldest first: the app and
    /// the sidebar state it was drawn for.
    pub fn shell_headers(&self) -> Vec<(String, Sidebar)> {
        self.headers.lock().unwrap().clone()
    }

    /// The context the window would give `A`: a token granted from
    /// `A::manifest()`, its settings, and the window's side of its
    /// attention signal, with the one-app shell header.
    pub fn context<A: App>(&self) -> (AppContext, watch::Receiver<Attention>) {
        let manifest = A::manifest();
        let token = self.engine.grant(&manifest);
        let (cx, attention) = AppContext::new(
            &manifest.id,
            self.client.scoped(token),
            self.settings.clone(),
        );
        let shell = TestShell {
            header: ProductWordmark::of(&manifest),
            drawn: self.headers.clone(),
        };
        let cx = cx
            .with_window(self.requests.clone())
            .with_shell(Arc::new(shell))
            .with_user_actions(self.user_actions.clone());
        (cx, attention)
    }

    /// Open `A` in a headless window, as the window would open its tab,
    /// with Isonomic UI initialised as the window does.
    pub fn open<A: App>(&self, cx: &mut TestAppContext) -> OpenApp {
        // Isonomic UI as the window sets it up: theme, bundled fonts and the
        // components' state, once per test.
        cx.update(|app| {
            if !app.has_global::<isonomic_ui::component::Theme>() {
                isonomic_ui::init(app);
            }
        });
        let (context, attention) = self.context::<A>();
        let sidebar = context.sidebar_changes();
        let registration = register::<A>();
        let hosts_shell_header = registration.hosts_shell_header();
        let window = cx.add_window(|window, tab| Tab::open(&registration, context, window, tab));
        OpenApp {
            window,
            attention,
            sidebar,
            hosts_shell_header,
        }
    }
}

/// An app open in a test window.
pub struct OpenApp {
    pub window: WindowHandle<Tab>,
    /// What the rail would show for the app.
    pub attention: watch::Receiver<Attention>,
    /// The app's sidebar as the window sees it.
    pub sidebar: watch::Receiver<Sidebar>,
    /// Whether the window would leave out its rail for the app.
    pub hosts_shell_header: bool,
}

impl OpenApp {
    /// The tab that hosts the app.
    pub fn tab(&self, cx: &mut TestAppContext) -> Entity<Tab> {
        self.window.root(cx).expect("the test window is open")
    }

    /// The app's root view as its own type.
    pub fn root<A: 'static>(&self, cx: &mut TestAppContext) -> Entity<A> {
        let view = self.tab(cx).read_with(cx, |tab, _| tab.app().view());
        view.downcast::<A>()
            .unwrap_or_else(|_| panic!("the app's root view is not the type asked for"))
    }
}

/// Run GPUI and the engine until `done` holds for `entity`. Event-driven:
/// it wakes on the entity's notifications. Fails the test after [`WAIT`].
pub fn wait_until<V: 'static>(
    cx: &mut TestAppContext,
    entity: &Entity<V>,
    mut done: impl FnMut(&V) -> bool,
) {
    let (wake, mut woken) = tokio::sync::mpsc::unbounded_channel::<bool>();
    let notified = wake.clone();
    let _observer = cx.update(|app| {
        app.observe(entity, move |_, _| {
            let _ = notified.send(true);
        })
    });
    std::thread::spawn(move || {
        std::thread::sleep(WAIT);
        let _ = wake.send(false);
    });
    cx.executor().allow_parking();
    loop {
        cx.run_until_parked();
        if entity.read_with(cx, |view, _| done(view)) {
            return;
        }
        let executor = cx.foreground_executor().clone();
        match executor.block_test(woken.recv()) {
            Some(true) => continue,
            _ => panic!("the view did not reach the expected state within {WAIT:?}"),
        }
    }
}
