//! The SDK's contract as checks that run against any engine connection.
//!
//! This repo's CI runs them against the testkit (`tests/testkit.rs`). The
//! engine runs the same checks against its own socket, from its own tests:
//!
//! ```no_run
//! # async fn demo() {
//! let socket = isonomic_client::unix_connector("/tmp/isonomic-test/engine.sock");
//! let report = isonomic_conformance::run(socket).await;
//! println!("{report}");
//! report.assert_passed();
//! # }
//! ```
//!
//! Every check speaks the wire alone. It grants its own test apps through
//! `apps.grant` (ids under `com.isonomic.conformance`), so it needs no
//! engine-side setup. What needs scripted engine state, such as a model's
//! output or a query's rows, stays in the testkit's own tests.
//!
//! Run them against a test engine, never a user's: the checks store and
//! delete small blobs, publish events, create and purge agent sessions and
//! grant test apps, and the last one asks the engine to shut down with an
//! app's token, which it must refuse.

mod agents;
mod interfaces;
mod recovery;
mod tokens;
mod wire;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use isonomic_client::{AppClient, Client, ClientError, Connector, Options};
use isonomic_proto::manifest::Schema;
use isonomic_proto::{Capabilities, ErrorCode, Manifest};

/// How long one check may take before it fails.
pub const CHECK_LIMIT: Duration = Duration::from_secs(10);

/// What a check returns: why it failed, if it did.
pub type Outcome = Result<(), String>;

type Run = fn(Target) -> Pin<Box<dyn Future<Output = Outcome>>>;

/// One named check.
pub struct Check {
    pub name: &'static str,
    run: Run,
}

macro_rules! check {
    ($name:literal, $f:path) => {
        Check {
            name: $name,
            run: |target| Box::pin($f(target)),
        }
    };
}

/// Every check, in the order `run` takes them. The shutdown refusal is last,
/// so an engine that wrongly obeys it fails nothing else.
pub const CHECKS: &[Check] = &[
    check!("wire.welcome", wire::welcome),
    check!("wire.unsupported_version", wire::unsupported_version),
    check!("wire.hello_required", wire::hello_required),
    check!("wire.unknown_method", wire::unknown_method),
    check!("wire.unknown_fields", wire::unknown_fields),
    check!("wire.bad_frame", wire::bad_frame),
    check!("identity.stable", wire::identity_stable),
    check!("identity.kit_apps_refused", recovery::apps_refused),
    check!(
        "identity.kit_export_needs_person",
        recovery::export_needs_person
    ),
    check!("tokens.grant", tokens::grant),
    check!("tokens.unknown_token", tokens::unknown_token),
    check!("tokens.shell_only", tokens::shell_only),
    check!("tokens.every_interface", tokens::every_interface),
    check!("tokens.named_model", tokens::named_model),
    check!("tokens.other_scope", tokens::other_scope),
    check!("blobs.round_trip", interfaces::blobs_round_trip),
    check!("blobs.chunked", interfaces::blobs_chunked),
    check!("blobs.private", interfaces::blobs_private),
    check!("blobs.max_bytes", interfaces::blobs_max_bytes),
    check!("events.round_trip", interfaces::events_round_trip),
    check!("events.scoped", interfaces::events_scoped),
    check!(
        "events.shell_reaches_app",
        interfaces::events_shell_reaches_app
    ),
    check!("subscriptions.owner_only", interfaces::owner_only),
    check!("sql.schemas", interfaces::sql_schemas),
    check!("agents.presets", agents::presets),
    check!("agents.registry", agents::registry),
    check!("agents.replay", agents::replay),
    check!("agents.isolated", agents::isolated),
    check!("agents.purge_own", agents::purge_own),
    check!("agents.unknown_subscription", agents::unknown_subscription),
    check!("agents.model_rules", agents::model_rules),
    check!("agents.session_ended", agents::session_ended),
    check!("tokens.shutdown_refused", tokens::shutdown_refused),
];

/// What the checks share: the way in, and one shell connection.
#[derive(Clone)]
pub struct Target {
    connector: Connector,
    shell: Client,
}

impl Target {
    /// A token for `manifest`, granted by the shell over the wire, used on
    /// the shell's connection.
    async fn app(&self, manifest: Manifest) -> Result<AppClient, String> {
        let id = manifest.id.clone();
        let token = ok(self.shell.grant(manifest).await, &format!("grant {id}"))?;
        Ok(self.shell.scoped(token))
    }
}

/// The result of every check that ran.
#[derive(Debug)]
pub struct Report {
    pub results: Vec<(&'static str, Outcome)>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.results.iter().all(|(_, outcome)| outcome.is_ok())
    }

    /// The checks that failed, with why.
    pub fn failures(&self) -> Vec<(&'static str, &str)> {
        let failed = self
            .results
            .iter()
            .filter_map(|(name, outcome)| outcome.as_ref().err().map(|why| (*name, why.as_str())));
        failed.collect()
    }

    /// Panic, listing every failure, unless all passed.
    pub fn assert_passed(&self) {
        assert!(self.passed(), "conformance failed:\n{self}");
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, outcome) in &self.results {
            match outcome {
                Ok(()) => writeln!(f, "ok    {name}")?,
                Err(why) => writeln!(f, "FAIL  {name}: {why}")?,
            }
        }
        let failed = self.failures().len();
        write!(f, "{} passed, {failed} failed", self.results.len() - failed)
    }
}

/// Run every check against the engine behind `connector`. Needs a Tokio
/// runtime.
pub async fn run(connector: Connector) -> Report {
    run_only(connector, |_| true).await
}

/// Run the checks whose names pass `filter`, such as `|n| n.starts_with("blobs.")`.
pub async fn run_only(connector: Connector, filter: impl Fn(&str) -> bool) -> Report {
    let mut options = Options::new("isonomic-conformance");
    options.reconnect_min = Duration::from_millis(10);
    options.reconnect_max = Duration::from_millis(200);
    let shell = Client::start(
        &tokio::runtime::Handle::current(),
        connector.clone(),
        options,
    );
    let mut live = shell.live();
    let waiting = tokio::time::timeout(CHECK_LIMIT, live.wait_for(|l| l.connected));
    // Never bind the borrow `wait_for` returns: held, it blocks the client's
    // own updates to the live view.
    let connected = matches!(waiting.await, Ok(Ok(_)));
    if !connected {
        let why = format!("no connection within {CHECK_LIMIT:?}");
        return Report {
            results: vec![("connect", Err(why))],
        };
    }
    let target = Target { connector, shell };
    let mut results = Vec::new();
    for check in CHECKS.iter().filter(|c| filter(c.name)) {
        let outcome = tokio::time::timeout(CHECK_LIMIT, (check.run)(target.clone()))
            .await
            .unwrap_or_else(|_| Err(format!("no result within {CHECK_LIMIT:?}")));
        results.push((check.name, outcome));
    }
    Report { results }
}

/// A test app's manifest. Ids are `com.isonomic.conformance.<name>`.
fn manifest(name: &str, capabilities: Capabilities, schemas: &[&str]) -> Manifest {
    Manifest {
        id: format!("com.isonomic.conformance.{name}"),
        name: format!("Conformance {name}"),
        version: "0.0.1".into(),
        icon: "check".into(),
        capabilities,
        schemas: schemas
            .iter()
            .map(|s| Schema {
                name: s.to_string(),
            })
            .collect(),
    }
}

/// The value, or why the call failed.
fn ok<T>(result: Result<T, ClientError>, what: &str) -> Result<T, String> {
    result.map_err(|error| format!("{what}: {error}"))
}

/// The call failed with `code`.
fn refused<T: fmt::Debug>(result: Result<T, ClientError>, code: ErrorCode, what: &str) -> Outcome {
    match result {
        Err(ClientError::Engine(error)) if error.code == code => Ok(()),
        Err(ClientError::Engine(error)) => Err(format!(
            "{what}: expected {code:?}, got {:?} ({})",
            error.code, error.message
        )),
        other => Err(format!("{what}: expected {code:?}, got {other:?}")),
    }
}

fn ensure(condition: bool, what: impl FnOnce() -> String) -> Outcome {
    if condition {
        Ok(())
    } else {
        Err(what())
    }
}
