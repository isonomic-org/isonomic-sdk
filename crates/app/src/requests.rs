//! What an app asks the window to do for it: things only the window may
//! do, such as installing a model, which is the shell's alone on the wire.

use std::sync::{Arc, Mutex};

/// A request from an app to the window. More kinds will come; match with a
/// wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowRequest {
    /// Install this local model: the window runs its download step with
    /// the person. The app learns the outcome from `models.info` (`stopped`
    /// once installed), not from the request.
    InstallModel { model: String },
}

/// The window's side: it decides what to do with each request. `true` when
/// it took the request on, `false` when it cannot (it does not handle that
/// kind, or the person is mid-way through something else).
pub trait WindowRequests: Send + Sync + 'static {
    fn request(&self, app: &str, request: WindowRequest) -> bool;
}

/// Every request, recorded, each taken on: the test host's window.
#[derive(Debug, Default)]
pub struct RecordedRequests {
    seen: Mutex<Vec<(String, WindowRequest)>>,
}

impl RecordedRequests {
    /// Every request so far, oldest first, with the app that made it.
    pub fn all(&self) -> Vec<(String, WindowRequest)> {
        self.seen.lock().unwrap().clone()
    }
}

impl WindowRequests for RecordedRequests {
    fn request(&self, app: &str, request: WindowRequest) -> bool {
        self.seen.lock().unwrap().push((app.to_string(), request));
        true
    }
}

pub(crate) type Window = Option<Arc<dyn WindowRequests>>;
