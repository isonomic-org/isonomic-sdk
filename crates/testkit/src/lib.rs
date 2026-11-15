//! An in-memory Isonomic engine for tests.
//!
//! It speaks the real protocol over in-memory streams and runs no services,
//! so a client, an app, or a whole window can be tested with no socket, no
//! engine binary and no database. Tests script it: change a service's
//! state, take the engine offline and bring it back, check whether shutdown
//! was requested, and see every call each app made.
//!
//! It enforces capability tokens exactly as the engine does (through
//! `isonomic_proto::auth`), so an app cannot pass its tests while calling
//! something its manifest does not allow.

mod agents;
mod agents_map;
mod blobs;
mod conn;
mod downloads;
mod events;
mod faults;
mod models;
mod sql;
mod system;

pub use agents::FakeAgents;
pub use blobs::{FakeBlobs, DEFAULT_LIMIT as DEFAULT_BLOB_LIMIT};
/// harness-host, whose `MemoryHost` serves the agents interface here.
pub use harness_host;
pub use downloads::{DownloadStep, MEMORY_FLOOR};
pub use isonomic_proto::auth::Interface;
pub use models::{FakeModels, Gate, Step};
pub use sql::{FakeSql, SqlCall};

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use isonomic_client::{AppClient, BoxedStream, Client, Connector, Options};
use isonomic_proto::{
    ErrorCode, HardwareProfile, Manifest, ServiceState, ServiceStatus, SystemStatus, Token,
    WireError,
};
use tokio::sync::{broadcast, watch};

/// One app's approved manifest, as the engine holds it.
#[derive(Debug, Clone)]
pub(crate) struct Grant {
    pub(crate) manifest: Manifest,
}

/// One call the engine received, for assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRecord {
    /// The calling app's id; `None` for the shell.
    pub app: Option<String>,
    pub method: String,
}

pub(crate) struct State {
    pub(crate) system: Mutex<SystemStatus>,
    pub(crate) logs: Mutex<HashMap<String, Vec<String>>>,
    pub(crate) changes: broadcast::Sender<ServiceStatus>,
    pub(crate) retries: Mutex<Vec<String>>,
    pub(crate) retry_outcomes: Mutex<HashMap<String, ServiceState>>,
    pub(crate) online: watch::Sender<bool>,
    pub(crate) shutdown_requested: AtomicBool,
    pub(crate) grants: Mutex<HashMap<Token, Grant>>,
    pub(crate) calls: Mutex<Vec<CallRecord>>,
    pub(crate) models: Mutex<models::ModelsState>,
    pub(crate) cancels: watch::Sender<u32>,
    pub(crate) blobs: Mutex<blobs::BlobsState>,
    pub(crate) events: Mutex<events::EventsState>,
    pub(crate) sql: Mutex<sql::SqlState>,
    pub(crate) downloads: Mutex<downloads::DownloadsState>,
    pub(crate) faults: Mutex<HashMap<Interface, faults::Fault>>,
    pub(crate) agents: Mutex<agents::AgentsState>,
    next_token: AtomicU64,
}

impl State {
    fn set_service(&self, status: ServiceStatus) {
        self.system.lock().unwrap().apply(status.clone());
        let _ = self.changes.send(status);
    }

    fn set_state(&self, name: &str, state: ServiceState) {
        let services = self.system.lock().unwrap().services.clone();
        let Some(mut status) = services.into_iter().find(|s| s.name == name) else {
            return;
        };
        if state == ServiceState::Healthy && status.state != ServiceState::Healthy {
            status.restart_count += 1;
        }
        status.state = state;
        self.set_service(status);
    }

    /// Validate a manifest and issue a token for it. Granting the same app
    /// again replaces its manifest and keeps earlier tokens working, like an
    /// engine whose tokens are derived from the manifest.
    pub(crate) fn grant(&self, manifest: Manifest) -> Result<(String, Token), WireError> {
        manifest
            .validate()
            .map_err(|e| WireError::new(ErrorCode::InvalidManifest, e.to_string()))?;
        let n = self.next_token.fetch_add(1, Ordering::SeqCst);
        let token = Token::new(format!("tok-{}-{n}", manifest.id));
        let app = manifest.id.clone();
        let mut grants = self.grants.lock().unwrap();
        for grant in grants.values_mut() {
            if grant.manifest.id == app {
                grant.manifest = manifest.clone();
            }
        }
        grants.insert(token.clone(), Grant { manifest });
        Ok((app, token))
    }
}

/// The scripted engine. Cheap to clone.
#[derive(Clone)]
pub struct TestEngine {
    state: Arc<State>,
}

impl Default for TestEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A healthy service with no restarts.
pub fn healthy(name: &str, tier: u8) -> ServiceStatus {
    ServiceStatus {
        name: name.to_string(),
        tier,
        state: ServiceState::Healthy,
        restart_count: 0,
        last_healthy_unix: Some(1_700_000_000),
    }
}

impl TestEngine {
    /// An online engine with one healthy `postgres` service.
    pub fn new() -> Self {
        let system = SystemStatus {
            aggregate: ServiceState::Healthy,
            services: vec![healthy("postgres", 1)],
            cold_start_ms: Some(150),
        };
        Self {
            state: Arc::new(State {
                system: Mutex::new(system),
                logs: Mutex::new(HashMap::new()),
                changes: broadcast::channel(64).0,
                retries: Mutex::new(Vec::new()),
                retry_outcomes: Mutex::new(HashMap::new()),
                online: watch::channel(true).0,
                shutdown_requested: AtomicBool::new(false),
                grants: Mutex::new(HashMap::new()),
                calls: Mutex::new(Vec::new()),
                models: Mutex::new(models::ModelsState::default()),
                cancels: watch::channel(0).0,
                blobs: Mutex::new(blobs::BlobsState::default()),
                events: Mutex::new(events::EventsState::default()),
                sql: Mutex::new(sql::SqlState::default()),
                downloads: Mutex::new(downloads::DownloadsState::default()),
                faults: Mutex::new(HashMap::new()),
                agents: Mutex::new(agents::AgentsState::default()),
                next_token: AtomicU64::new(1),
            }),
        }
    }

    /// How a client reaches this engine. Refused while it is offline.
    pub fn connector(&self) -> Connector {
        let state = Arc::clone(&self.state);
        Arc::new(move || {
            let state = Arc::clone(&state);
            Box::pin(async move {
                if !*state.online.borrow() {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        "engine offline",
                    ));
                }
                let (client_side, engine_side) = tokio::io::duplex(256 * 1024);
                tokio::spawn(conn::serve(engine_side, state));
                Ok(Box::new(client_side) as BoxedStream)
            })
        })
    }

    /// A shell client connected to this engine, with fast reconnects. Must
    /// be called inside a Tokio runtime.
    pub fn client(&self) -> Client {
        self.client_on(&tokio::runtime::Handle::current())
    }

    /// The same, with the background task on a given runtime. Use this from
    /// a UI test, whose own executor is not Tokio.
    pub fn client_on(&self, runtime: &tokio::runtime::Handle) -> Client {
        let mut options = Options::new("test");
        options.reconnect_min = std::time::Duration::from_millis(5);
        options.reconnect_max = std::time::Duration::from_millis(20);
        Client::start(runtime, self.connector(), options)
    }

    /// Approve a manifest and issue its token, as the shell would through
    /// `apps.grant`. Panics on an invalid manifest; use the wire call to
    /// test refusals.
    pub fn grant(&self, manifest: &Manifest) -> Token {
        match self.state.grant(manifest.clone()) {
            Ok((_, token)) => token,
            Err(error) => panic!("invalid manifest: {}", error.message),
        }
    }

    /// A client scoped to a freshly granted token for `manifest`.
    pub fn app_client(&self, manifest: &Manifest) -> AppClient {
        self.client().scoped(self.grant(manifest))
    }

    /// The fake models: add, script, refuse, fail.
    pub fn models(&self) -> FakeModels {
        FakeModels {
            state: Arc::clone(&self.state),
        }
    }

    /// The fake blob store: its limit and what apps stored.
    pub fn blobs(&self) -> FakeBlobs {
        FakeBlobs {
            state: Arc::clone(&self.state),
        }
    }

    /// The agent host (harness-host's `MemoryHost`): script the model, move
    /// the clock, mint the person's user-action tokens.
    pub fn agents(&self) -> FakeAgents {
        FakeAgents {
            state: Arc::clone(&self.state),
        }
    }

    /// The fake database: scripted results and the statements apps ran.
    pub fn sql(&self) -> FakeSql {
        FakeSql {
            state: Arc::clone(&self.state),
        }
    }

    /// The machine `system.profile` describes and `models.recommended`
    /// chooses for. `standard` at the 16 GiB floor by default.
    pub fn set_profile(&self, profile: HardwareProfile) {
        self.state.downloads.lock().unwrap().profile = profile;
    }

    /// Make every call on `interface` wait `delay` before it is answered.
    pub fn slow(&self, interface: Interface, delay: Duration) {
        let mut faults = self.state.faults.lock().unwrap();
        faults.entry(interface).or_default().delay = Some(delay);
    }

    /// Answer every call on `interface` with `error`, after any delay.
    pub fn fail(&self, interface: Interface, error: WireError) {
        let mut faults = self.state.faults.lock().unwrap();
        faults.entry(interface).or_default().error = Some(error);
    }

    /// Clear `slow` and `fail` for `interface`.
    pub fn recover(&self, interface: Interface) {
        self.state.faults.lock().unwrap().remove(&interface);
    }

    /// Withdraw every token, like an engine that lost its grants.
    pub fn revoke_all(&self) {
        self.state.grants.lock().unwrap().clear();
    }

    /// Every call received so far, oldest first.
    pub fn calls(&self) -> Vec<CallRecord> {
        self.state.calls.lock().unwrap().clone()
    }

    /// Replace (or add) one service and tell every subscriber.
    pub fn set_service(&self, status: ServiceStatus) {
        self.state.set_service(status);
    }

    /// Change one existing service's state, counting a restart when it comes
    /// back to healthy from anything else.
    pub fn set_state(&self, name: &str, state: ServiceState) {
        self.state.set_state(name, state);
    }

    /// The services `system.retry` restarted, oldest first.
    pub fn retries(&self) -> Vec<String> {
        self.state.retries.lock().unwrap().clone()
    }

    /// What a retry of `service` leads to after `starting`: `Healthy`, or
    /// `Degraded` again. Unscripted, the service stays `starting` until the
    /// test calls `set_state`.
    pub fn on_retry(&self, service: &str, outcome: ServiceState) {
        let mut outcomes = self.state.retry_outcomes.lock().unwrap();
        outcomes.insert(service.to_string(), outcome);
    }

    pub fn set_logs(&self, service: &str, lines: &[&str]) {
        let lines = lines.iter().map(|l| l.to_string()).collect();
        self.state
            .logs
            .lock()
            .unwrap()
            .insert(service.to_string(), lines);
    }

    pub fn system(&self) -> SystemStatus {
        self.state.system.lock().unwrap().clone()
    }

    /// Drop every connection and refuse new ones, like a crashed engine.
    pub fn go_offline(&self) {
        self.state.online.send_replace(false);
    }

    pub fn go_online(&self) {
        self.state.online.send_replace(true);
    }

    pub fn shutdown_requested(&self) -> bool {
        self.state.shutdown_requested.load(Ordering::SeqCst)
    }
}
