//! What the window gives an app: a scoped client, settings, attention.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use isonomic_client::AppClient;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::watch;

/// Whether an app wants the user to look at it. Drives the dot on the rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Attention {
    #[default]
    None,
    /// Something new; no count.
    Dot,
    /// A number of things waiting, such as unread replies.
    Count(u32),
}

/// Where app settings are kept. The window supplies one; it keys every
/// value by app id, so one app never sees another's settings.
pub trait SettingsStore: Send + Sync + 'static {
    fn get(&self, app: &str, key: &str) -> Option<Value>;
    fn set(&self, app: &str, key: &str, value: Value);
    fn remove(&self, app: &str, key: &str);
}

/// A settings store in memory, for tests and until the engine stores
/// settings.
#[derive(Default)]
pub struct MemorySettings {
    values: Mutex<HashMap<(String, String), Value>>,
}

impl SettingsStore for MemorySettings {
    fn get(&self, app: &str, key: &str) -> Option<Value> {
        let key = (app.to_string(), key.to_string());
        self.values.lock().unwrap().get(&key).cloned()
    }

    fn set(&self, app: &str, key: &str, value: Value) {
        let key = (app.to_string(), key.to_string());
        self.values.lock().unwrap().insert(key, value);
    }

    fn remove(&self, app: &str, key: &str) {
        let key = (app.to_string(), key.to_string());
        self.values.lock().unwrap().remove(&key);
    }
}

/// One app's settings namespace. Values are plain JSON.
#[derive(Clone)]
pub struct Settings {
    app: Arc<str>,
    store: Arc<dyn SettingsStore>,
}

impl Settings {
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let value = self.store.get(&self.app, key)?;
        serde_json::from_value(value).ok()
    }

    pub fn set<T: Serialize>(&self, key: &str, value: &T) {
        if let Ok(value) = serde_json::to_value(value) {
            self.store.set(&self.app, key, value);
        }
    }

    pub fn remove(&self, key: &str) {
        self.store.remove(&self.app, key);
    }
}

/// Everything an app is given. Cheap to clone; pass it into views.
#[derive(Clone)]
pub struct AppContext {
    id: Arc<str>,
    client: AppClient,
    settings: Settings,
    attention: Arc<watch::Sender<Attention>>,
    window: crate::requests::Window,
    /// When this app's last report was taken, shared by every clone.
    reported: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
}

impl AppContext {
    /// Made by the window for one app. The receiver is the window's side
    /// of the attention signal.
    pub fn new(
        id: &str,
        client: AppClient,
        store: Arc<dyn SettingsStore>,
    ) -> (Self, watch::Receiver<Attention>) {
        let (attention, watched) = watch::channel(Attention::None);
        let id: Arc<str> = Arc::from(id);
        let settings = Settings {
            app: Arc::clone(&id),
            store,
        };
        let cx = Self {
            id,
            client,
            settings,
            attention: Arc::new(attention),
            window: None,
            reported: Arc::default(),
        };
        (cx, watched)
    }

    /// The window's handler for [`AppContext::request`]. Without one, every
    /// request answers `false`.
    pub fn with_window(mut self, window: Arc<dyn crate::WindowRequests>) -> Self {
        self.window = Some(window);
        self
    }

    /// Ask the window to do something only it may, such as installing a
    /// model. `true` when the window took it on. The outcome shows where it
    /// always does: a model's state in `models.info`, say.
    /// Report a handled problem: the window adds the app's id and version,
    /// scrubs it and shows the person the problem-report notice. `true` when
    /// the window took it. At most one a [`REPORT_INTERVAL`](crate::REPORT_INTERVAL):
    /// a second within it answers `false` and reaches nothing. One the
    /// window refused does not count.
    pub fn report(&self, problem: crate::Problem) -> bool {
        let mut last = self.reported.lock().unwrap_or_else(|p| p.into_inner());
        if last.is_some_and(|at| at.elapsed() < crate::REPORT_INTERVAL) {
            return false;
        }
        let taken = self.request(crate::WindowRequest::Report(problem));
        if taken {
            *last = Some(std::time::Instant::now());
        }
        taken
    }

    pub fn request(&self, request: crate::WindowRequest) -> bool {
        match &self.window {
            Some(window) => window.request(&self.id, request),
            None => false,
        }
    }

    /// The app's id, from its manifest.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The engine, under this app's token. Every call is checked against
    /// the app's manifest.
    pub fn client(&self) -> &AppClient {
        &self.client
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Ask for (or stop asking for) the user's attention.
    pub fn set_attention(&self, attention: Attention) {
        self.attention.send_replace(attention);
    }

    pub fn attention(&self) -> Attention {
        *self.attention.borrow()
    }
}
