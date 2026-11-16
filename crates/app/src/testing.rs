//! A headless host for app tests: the in-memory engine, a token granted
//! from the app's own manifest, and a GPUI test window.
//!
//! The testkit enforces the token, so an app cannot pass its tests while
//! calling something its manifest does not allow.
//!
//! ```ignore
//! use isonomic_app::ui as gpui;
//!
//! #[gpui::test]
//! fn shows_the_models(cx: &mut TestAppContext) {
//!     let host = TestHost::new();
//!     let open = host.open::<Hello>(cx);
//!     let hello = open.root::<Hello>(cx);
//!     wait_until(cx, &hello, |hello| !hello.models().is_empty());
//! }
//! ```

use std::sync::Arc;
use std::time::Duration;

use isonomic_client::Client;
use isonomic_testkit::TestEngine;
use isonomic_ui::{Entity, TestAppContext, WindowHandle};
use tokio::sync::watch;

use crate::{
    register, App, AppContext, Attention, MemorySettings, RecordedRequests, Tab, WindowRequest,
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
    runtime: tokio::runtime::Runtime,
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
        Self {
            engine,
            client,
            settings: Arc::new(MemorySettings::default()),
            requests: Arc::new(RecordedRequests::default()),
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

    /// The context the window would give `A`: a token granted from
    /// `A::manifest()`, its settings, and the window's side of its
    /// attention signal.
    pub fn context<A: App>(&self) -> (AppContext, watch::Receiver<Attention>) {
        let manifest = A::manifest();
        let token = self.engine.grant(&manifest);
        let (cx, attention) = AppContext::new(
            &manifest.id,
            self.client.scoped(token),
            self.settings.clone(),
        );
        (cx.with_window(self.requests.clone()), attention)
    }

    /// Open `A` in a headless window, as the window would open its tab.
    pub fn open<A: App>(&self, cx: &mut TestAppContext) -> OpenApp {
        let (context, attention) = self.context::<A>();
        let registration = register::<A>();
        let window = cx.add_window(|window, tab| Tab::open(&registration, context, window, tab));
        OpenApp { window, attention }
    }
}

/// An app open in a test window.
pub struct OpenApp {
    pub window: WindowHandle<Tab>,
    /// What the rail would show for the app.
    pub attention: watch::Receiver<Attention>,
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
