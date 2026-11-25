// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

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
    /// Report a handled problem. The window adds the app's id and version,
    /// scrubs it with `isonomic-reports`, and shows the person the
    /// problem-report notice. Made by [`AppContext::report`](crate::AppContext::report),
    /// which keeps to one a minute.
    Report(Problem),
}

/// A handled problem an app reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// One line: what went wrong, in plain words. No content the person
    /// typed.
    pub summary: String,
    /// Optional detail, such as an error code or the step that failed.
    pub detail: Option<String>,
}

/// An app reports at most one problem in this long.
pub const REPORT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

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

    /// The problems reported so far, oldest first, with the app.
    pub fn reports(&self) -> Vec<(String, Problem)> {
        let seen = self.seen.lock().unwrap();
        let reported = seen.iter().filter_map(|(app, request)| match request {
            WindowRequest::Report(problem) => Some((app.clone(), problem.clone())),
            _ => None,
        });
        reported.collect()
    }
}

impl WindowRequests for RecordedRequests {
    fn request(&self, app: &str, request: WindowRequest) -> bool {
        self.seen.lock().unwrap().push((app.to_string(), request));
        true
    }
}

pub(crate) type Window = Option<Arc<dyn WindowRequests>>;
