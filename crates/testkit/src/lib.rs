//! An in-memory Isonomic engine for tests.
//!
//! It speaks the real protocol over in-memory streams and runs no services,
//! so a client, or a whole window, can be tested with no socket, no engine
//! binary and no database. Tests script it: change a service's state, take
//! the engine offline and bring it back, and check whether shutdown was
//! requested.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use isonomic_client::{BoxedStream, Client, Connector, Options};
use isonomic_proto::io::{read_message, write_message};
use isonomic_proto::{
    negotiate, Call, ClientMessage, ErrorCode, Event, HardwareProfile, Outcome, Reply,
    ServerMessage, ServiceState, ServiceStatus, SystemStatus, WireError, PROTOCOL_VERSION,
};
use tokio::io::DuplexStream;
use tokio::sync::{broadcast, mpsc, watch};

struct State {
    system: Mutex<SystemStatus>,
    logs: Mutex<HashMap<String, Vec<String>>>,
    changes: broadcast::Sender<ServiceStatus>,
    online: watch::Sender<bool>,
    shutdown_requested: AtomicBool,
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
                online: watch::channel(true).0,
                shutdown_requested: AtomicBool::new(false),
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
                let (client_side, engine_side) = tokio::io::duplex(64 * 1024);
                tokio::spawn(serve(engine_side, state));
                Ok(Box::new(client_side) as BoxedStream)
            })
        })
    }

    /// A client connected to this engine, with fast reconnects. Must be
    /// called inside a Tokio runtime.
    pub fn client(&self) -> Client {
        let mut options = Options::new("test");
        options.reconnect_min = std::time::Duration::from_millis(5);
        options.reconnect_max = std::time::Duration::from_millis(20);
        Client::start(
            &tokio::runtime::Handle::current(),
            self.connector(),
            options,
        )
    }

    /// Replace (or add) one service and tell every subscriber.
    pub fn set_service(&self, status: ServiceStatus) {
        self.state.system.lock().unwrap().apply(status.clone());
        let _ = self.state.changes.send(status);
    }

    /// Change one existing service's state, counting a restart when it comes
    /// back to healthy from anything else.
    pub fn set_state(&self, name: &str, state: ServiceState) {
        let current = self
            .state
            .system
            .lock()
            .unwrap()
            .services
            .iter()
            .find(|s| s.name == name)
            .cloned();
        let Some(mut status) = current else { return };
        if state == ServiceState::Healthy && status.state != ServiceState::Healthy {
            status.restart_count += 1;
        }
        status.state = state;
        self.set_service(status);
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

fn failure(id: u64, code: ErrorCode, message: &str) -> ServerMessage {
    ServerMessage::Response {
        id,
        outcome: Outcome::Error(WireError {
            code,
            message: message.to_string(),
        }),
    }
}

fn answer(state: &State, call: Call) -> Outcome {
    match call {
        Call::SystemStatus => Outcome::Result(Reply::Status(state.system.lock().unwrap().clone())),
        Call::SystemLogs { service, limit } => {
            let known = state
                .system
                .lock()
                .unwrap()
                .services
                .iter()
                .any(|s| s.name == service);
            if !known {
                return Outcome::Error(WireError {
                    code: ErrorCode::UnknownService,
                    message: format!("no service named {service}"),
                });
            }
            let mut lines = state
                .logs
                .lock()
                .unwrap()
                .get(&service)
                .cloned()
                .unwrap_or_default();
            let keep = limit.map(|n| n as usize).unwrap_or(50);
            lines.drain(..lines.len().saturating_sub(keep));
            Outcome::Result(Reply::Logs { service, lines })
        }
        Call::SystemProfile => Outcome::Result(Reply::Profile(HardwareProfile {
            profile: "standard".to_string(),
            total_memory_bytes: 16 << 30,
            available_memory_bytes: 8 << 30,
        })),
        Call::SystemShutdown => {
            state.shutdown_requested.store(true, Ordering::SeqCst);
            Outcome::Result(Reply::ShuttingDown {})
        }
        Call::SystemSubscribe | Call::SystemUnsubscribe { .. } => {
            unreachable!("handled by the connection")
        }
    }
}

async fn serve(stream: DuplexStream, state: Arc<State>) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (out, mut outbox) = mpsc::channel::<ServerMessage>(64);
    let writing = tokio::spawn(async move {
        while let Some(message) = outbox.recv().await {
            if write_message(&mut writer, &message).await.is_err() {
                return;
            }
        }
    });
    let mut online = state.online.subscribe();
    let mut forwarders = HashMap::new();
    let mut next_subscription = 1u64;
    let mut greeted = false;
    loop {
        // Dropping a half-read frame is fine here: going offline ends the
        // connection anyway.
        let message = tokio::select! {
            _ = online.wait_for(|up| !*up) => break,
            message = read_message::<_, ClientMessage>(&mut reader) => match message {
                Ok(Some(message)) => message,
                _ => break,
            },
        };
        let reply = match message {
            ClientMessage::Hello { versions, .. } => {
                match negotiate(&versions, &[PROTOCOL_VERSION]) {
                    Some(version) => {
                        greeted = true;
                        ServerMessage::Welcome {
                            version,
                            engine: "testkit".to_string(),
                        }
                    }
                    None => failure(0, ErrorCode::UnsupportedVersion, "no common version"),
                }
            }
            ClientMessage::Request { id, .. } if !greeted => {
                failure(id, ErrorCode::HelloRequired, "send hello first")
            }
            ClientMessage::Request {
                id,
                call: Call::SystemSubscribe,
            } => {
                let subscription = next_subscription;
                next_subscription += 1;
                let mut changes = state.changes.subscribe();
                let snapshot = state.system.lock().unwrap().clone();
                let _ = out
                    .send(ServerMessage::Response {
                        id,
                        outcome: Outcome::Result(Reply::Subscribed {
                            subscription,
                            snapshot,
                        }),
                    })
                    .await;
                let events = out.clone();
                forwarders.insert(
                    subscription,
                    tokio::spawn(async move {
                        while let Ok(change) = changes.recv().await {
                            let event = ServerMessage::Event {
                                subscription,
                                event: Event::ServiceStatus(change),
                            };
                            if events.send(event).await.is_err() {
                                return;
                            }
                        }
                    }),
                );
                continue;
            }
            ClientMessage::Request {
                id,
                call: Call::SystemUnsubscribe { subscription },
            } => match forwarders.remove(&subscription) {
                Some(task) => {
                    task.abort();
                    ServerMessage::Response {
                        id,
                        outcome: Outcome::Result(Reply::Unsubscribed {}),
                    }
                }
                None => failure(id, ErrorCode::UnknownSubscription, "no such subscription"),
            },
            ClientMessage::Request { id, call } => ServerMessage::Response {
                id,
                outcome: answer(&state, call),
            },
        };
        let _ = out.send(reply).await;
    }
    for task in forwarders.into_values() {
        task.abort();
    }
    writing.abort();
}
