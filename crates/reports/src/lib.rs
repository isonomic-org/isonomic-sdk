//! Problem reports in format 1, shared by the window and the engine.
//!
//! A report is one small JSON document. Both sides write the same document
//! with the same scrubbing, from this crate; neither sends anything. This
//! crate builds without Isonomic UI.
//!
//! - [`Report`]: the document, with the build and system it came from.
//! - [`scrub`]: what never goes in a report (home paths, long quoted text,
//!   keys and IDs, overlong messages).
//! - [`backtrace`]: backtraces as functions and crate-relative places.
//! - [`filter`]: the stricter rules for service logs.
//! - [`id`]: a report's own id, for dropping duplicates.
//!
//! Where a report is kept, the setting that allows it, and the panic hook
//! stay with each side: they differ.

pub mod backtrace;
pub mod filter;
pub mod id;
pub mod scrub;

use serde_json::{json, Map, Value};

/// The report format this crate writes.
pub const FORMAT: u32 = 1;
/// How many log lines a report carries.
pub const LOG_LINES: usize = 50;

/// What a report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The window or the engine panicked.
    Panic,
    /// The window stopped answering.
    Hang,
    /// An app failed at a call the window made, and was contained.
    AppFailed,
    /// An app reported a handled problem.
    AppError,
    /// A bundled service failed too often and was given up on.
    ServiceFailed,
    /// A model worker failed too often and was given up on.
    ModelServerFailed,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Panic => "panic",
            Kind::Hang => "hang",
            Kind::AppFailed => "app_failed",
            Kind::AppError => "app_error",
            Kind::ServiceFailed => "service_failed",
            Kind::ModelServerFailed => "model_server_failed",
        }
    }
}

/// The build a report comes from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Build {
    pub version: String,
    pub build: String,
    pub arch: String,
    /// The SDK, Chat and engine commits the build carries.
    pub sdk: String,
    pub chat: String,
    pub engine: String,
}

/// The machine a report comes from, as far as the writer knows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct System {
    /// The operating system's version. Format 1 writes it as `macos`.
    pub os_version: String,
    /// `low`, `standard` or `high`, once known.
    pub profile: Option<String>,
    pub memory_gb: Option<u64>,
    /// The Capsule's format number.
    pub capsule_format: Option<u32>,
}

/// Which part failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Component {
    /// The window: `window`.
    Window,
    /// An app, with its version: its id.
    App { id: String, version: String },
    /// The engine itself: `engine`.
    Engine,
    /// A bundled service: `engine/<name>`, with a `service` object.
    Service {
        name: String,
        version: String,
        restarts: u32,
        last_exit: Option<String>,
    },
    /// A model worker: `engine/<runtime>`, with a `model` object.
    ModelServer {
        model: String,
        runtime: String,
        version: String,
        restarts: u32,
        last_exit: Option<String>,
    },
}

/// One report, before it is written.
#[derive(Debug, Clone)]
pub struct Report {
    /// The report's own id (32 hex characters), made by [`Report::new`].
    /// Written as the top-level `id`, never scrubbed. `None` writes none,
    /// like a file from before ids.
    pub id: Option<String>,
    pub kind: Kind,
    pub component: Component,
    pub message: String,
    pub backtrace: Vec<String>,
    /// Filtered for its source where it has one; scrubbed again when written.
    pub log_tail: Vec<String>,
    /// The tab on screen when it happened (the window only).
    pub tab: Option<String>,
    pub uptime_secs: u64,
    /// More top-level fields, written as given.
    pub extra: Map<String, Value>,
}

impl Report {
    pub fn new(kind: Kind, component: Component, message: impl Into<String>) -> Self {
        Self {
            id: Some(id::new_id()),
            kind,
            component,
            message: message.into(),
            backtrace: Vec::new(),
            log_tail: Vec::new(),
            tab: None,
            uptime_secs: 0,
            extra: Map::new(),
        }
    }

    /// The document as written: every text field scrubbed against `home`.
    pub fn to_json(
        &self,
        build: &Build,
        system: &System,
        home: Option<&str>,
        unix_secs: u64,
    ) -> Value {
        let line = |l: &String| scrub::line(l, home);
        let exit = |e: &Option<String>| e.as_ref().map(line);
        let mut extra = Map::new();
        let component = match &self.component {
            Component::Window => json!({ "id": "window" }),
            Component::App { id, version } => json!({ "id": id, "version": version }),
            Component::Engine => json!({ "id": "engine", "version": build.version }),
            Component::Service {
                name,
                version,
                restarts,
                last_exit,
            } => {
                extra.insert(
                    "service".into(),
                    json!({ "name": name, "version": version, "restarts": restarts,
                            "last_exit": exit(last_exit) }),
                );
                json!({ "id": format!("engine/{name}"), "version": version })
            }
            Component::ModelServer {
                model,
                runtime,
                version,
                restarts,
                last_exit,
            } => {
                extra.insert(
                    "model".into(),
                    json!({ "id": model, "runtime": runtime, "restarts": restarts,
                            "last_exit": exit(last_exit) }),
                );
                json!({ "id": format!("engine/{runtime}"), "version": version })
            }
        };
        let mut doc = json!({
            "format": FORMAT,
            "kind": self.kind.as_str(),
            "component": component,
            "product": {
                "version": build.version,
                "build": build.build,
                "arch": build.arch,
                "pins": { "sdk": build.sdk, "chat": build.chat, "engine": build.engine },
            },
            "system": {
                "macos": system.os_version,
                "profile": system.profile,
                "memory_gb": system.memory_gb,
                "capsule_format": system.capsule_format,
            },
            "when": { "unix_secs": unix_secs, "uptime_secs": self.uptime_secs },
            "message": scrub::message(&self.message, home),
            "backtrace": self.backtrace.iter().map(line).collect::<Vec<_>>(),
            "log_tail": self.log_tail.iter().map(line).collect::<Vec<_>>(),
            "tab": self.tab,
        });
        if let Value::Object(map) = &mut doc {
            if let Some(id) = &self.id {
                map.insert("id".into(), Value::String(id.clone()));
            }
            map.extend(extra);
            for (key, value) in &self.extra {
                map.insert(key.clone(), value.clone());
            }
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> Build {
        Build {
            version: "0.9.0".into(),
            build: "214".into(),
            arch: "x86_64".into(),
            sdk: "abe8fe4".into(),
            chat: "9b58161".into(),
            engine: "506097d".into(),
        }
    }

    fn system() -> System {
        System {
            os_version: "15.1".into(),
            profile: Some("standard".into()),
            memory_gb: Some(16),
            capsule_format: Some(1),
        }
    }

    #[test]
    fn a_window_report_names_its_build_pins_system_and_tab() {
        let app = Component::App {
            id: "com.isonomic.chat".into(),
            version: "0.1.0".into(),
        };
        let mut report = Report::new(
            Kind::AppFailed,
            app,
            "build failed at /Users/omar/Library/x",
        );
        report.tab = Some("com.isonomic.chat".into());
        let doc = report.to_json(&build(), &system(), Some("/Users/omar"), 1_700_000_000);
        assert_eq!(doc["format"], FORMAT);
        assert_eq!(doc["kind"], "app_failed");
        assert_eq!(doc["component"]["id"], "com.isonomic.chat");
        assert_eq!(doc["product"]["pins"]["chat"], "9b58161");
        assert_eq!(doc["system"]["macos"], "15.1");
        assert_eq!(doc["system"]["capsule_format"], 1);
        assert_eq!(doc["message"], "build failed at ~/Library/x");
        assert_eq!(doc["tab"], "com.isonomic.chat");
    }

    #[test]
    fn a_service_report_names_the_service_and_keeps_the_windows_fields() {
        let service = Component::Service {
            name: "database".into(),
            version: "18.4".into(),
            restarts: 6,
            last_exit: Some("exit code 1".into()),
        };
        let report = Report::new(
            Kind::ServiceFailed,
            service,
            "database stopped at /Users/omar/x",
        );
        let doc = report.to_json(&build(), &system(), Some("/Users/omar"), 1_700_000_000);
        assert_eq!(doc["kind"], "service_failed");
        assert_eq!(doc["component"]["id"], "engine/database");
        assert_eq!(doc["service"]["restarts"], 6);
        assert_eq!(doc["service"]["last_exit"], "exit code 1");
        assert_eq!(doc["message"], "database stopped at ~/x");
        assert!(doc["tab"].is_null());
    }

    #[test]
    fn a_model_server_report_carries_its_model() {
        let worker = Component::ModelServer {
            model: "chat-small".into(),
            runtime: "model-server".into(),
            version: "b12387".into(),
            restarts: 5,
            last_exit: Some("signal 9".into()),
        };
        let doc = Report::new(Kind::ModelServerFailed, worker, "stopped").to_json(
            &build(),
            &system(),
            None,
            1,
        );
        assert_eq!(doc["component"]["id"], "engine/model-server");
        assert_eq!(doc["model"]["id"], "chat-small");
        assert_eq!(doc["model"]["last_exit"], "signal 9");
    }

    #[test]
    fn a_report_carries_its_own_id_unscrubbed() {
        let report = Report::new(Kind::Panic, Component::Window, "fell over");
        let id = report.id.clone().expect("an id");
        assert!(id::is_id(&id));
        let doc = report.to_json(&build(), &system(), Some("/Users/omar"), 1);
        assert_eq!(doc["id"], id.as_str(), "not [key removed]");
        assert_eq!(id::report_id(&doc, "1-panic.json", "capsule-1"), id);
        let mut older = report.clone();
        older.id = None;
        assert!(older
            .to_json(&build(), &system(), None, 1)
            .get("id")
            .is_none());
    }
}
