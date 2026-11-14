//! One in-memory connection: the handshake, then each request checked
//! against its token and answered by the interface it names.

use std::collections::HashMap;
use std::sync::Arc;

use isonomic_proto::auth::{authorize, Caller};
use isonomic_proto::io::{read_frame, write_message};
use isonomic_proto::lenient::{decode_client, FromClient};
use isonomic_proto::{
    negotiate, Call, ClientMessage, ErrorCode, Event, Outcome, Reply, ServerMessage, Token,
    WireError, PROTOCOL_VERSION,
};
use tokio::io::DuplexStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{system, CallRecord, Grant, State};

/// What a connection owns: its outbox and its open subscriptions.
pub(crate) struct Conn {
    pub(crate) out: mpsc::Sender<ServerMessage>,
    tasks: HashMap<u64, JoinHandle<()>>,
    next_subscription: u64,
}

impl Conn {
    /// Reserve the next subscription id.
    pub(crate) fn open_subscription(&mut self) -> u64 {
        let id = self.next_subscription;
        self.next_subscription += 1;
        id
    }

    /// Keep the task that feeds a subscription, so it can be stopped.
    pub(crate) fn track(&mut self, subscription: u64, task: JoinHandle<()>) {
        self.tasks.retain(|_, task| !task.is_finished());
        self.tasks.insert(subscription, task);
    }

    /// Stop a subscription's task. `false` if there was none.
    pub(crate) fn close(&mut self, subscription: u64) -> bool {
        match self.tasks.remove(&subscription) {
            Some(task) => {
                task.abort();
                true
            }
            None => false,
        }
    }
}

pub(crate) fn failure(id: u64, error: WireError) -> ServerMessage {
    ServerMessage::Response {
        id,
        outcome: Outcome::Error(error),
    }
}

fn result(id: u64, reply: Reply) -> ServerMessage {
    ServerMessage::Response {
        id,
        outcome: Outcome::Result(reply),
    }
}

/// Answer one request, or `None` when the interface answered it itself.
async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    token: Option<Token>,
    call: Call,
) -> Option<ServerMessage> {
    let grant: Option<Grant> = match &token {
        None => None,
        Some(token) => match state.grants.lock().unwrap().get(token).cloned() {
            Some(grant) => Some(grant),
            None => {
                let error = WireError::new(ErrorCode::UnknownToken, "no app holds this token");
                return Some(failure(id, error));
            }
        },
    };
    let caller = match &grant {
        None => Caller::Shell,
        Some(grant) => Caller::App {
            id: &grant.manifest.id,
            capabilities: &grant.manifest.capabilities,
        },
    };
    state.calls.lock().unwrap().push(CallRecord {
        app: grant.as_ref().map(|g| g.manifest.id.clone()),
        method: call.method().to_string(),
    });
    if let Err(error) = authorize(caller, &call) {
        return Some(failure(id, error));
    }
    Some(match call {
        Call::SystemSubscribe => {
            let subscription = conn.open_subscription();
            let mut changes = state.changes.subscribe();
            let snapshot = state.system.lock().unwrap().clone();
            let _ = conn
                .out
                .send(result(
                    id,
                    Reply::Subscribed {
                        subscription,
                        snapshot,
                    },
                ))
                .await;
            let events = conn.out.clone();
            let task = tokio::spawn(async move {
                while let Ok(change) = changes.recv().await {
                    let event = ServerMessage::Event {
                        subscription,
                        event: Event::ServiceStatus(change),
                    };
                    if events.send(event).await.is_err() {
                        return;
                    }
                }
            });
            conn.track(subscription, task);
            return None;
        }
        Call::SystemUnsubscribe { subscription } => match conn.close(subscription) {
            true => result(id, Reply::Unsubscribed {}),
            false => failure(
                id,
                WireError::new(ErrorCode::UnknownSubscription, "no such subscription"),
            ),
        },
        Call::AppsGrant { manifest } => match state.grant(*manifest) {
            Ok((app, token)) => result(id, Reply::Granted { app, token }),
            Err(error) => failure(id, error),
        },
        call => match system::answer(state, call) {
            Ok(reply) => result(id, reply),
            Err(error) => failure(id, error),
        },
    })
}

pub(crate) async fn serve(stream: DuplexStream, state: Arc<State>) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (out, mut outbox) = mpsc::channel::<ServerMessage>(256);
    let writing = tokio::spawn(async move {
        while let Some(message) = outbox.recv().await {
            if write_message(&mut writer, &message).await.is_err() {
                return;
            }
        }
    });
    let mut online = state.online.subscribe();
    let mut conn = Conn {
        out,
        tasks: HashMap::new(),
        next_subscription: 1,
    };
    let mut greeted = false;
    loop {
        // Dropping a half-read frame is fine here: going offline ends the
        // connection anyway.
        let body = tokio::select! {
            _ = online.wait_for(|up| !*up) => break,
            body = read_frame(&mut reader) => match body {
                Ok(Some(body)) => body,
                _ => break,
            },
        };
        let reply = match decode_client(&body) {
            FromClient::Garbage(error) => Some(failure(0, error)),
            FromClient::Rejected { id, error } => Some(failure(id, error)),
            FromClient::Message(ClientMessage::Hello { versions, .. }) => {
                match negotiate(&versions, &[PROTOCOL_VERSION]) {
                    Some(version) => {
                        greeted = true;
                        Some(ServerMessage::Welcome {
                            version,
                            engine: "testkit".to_string(),
                        })
                    }
                    None => Some(failure(
                        0,
                        WireError::new(ErrorCode::UnsupportedVersion, "no common version"),
                    )),
                }
            }
            FromClient::Message(ClientMessage::Request { id, .. }) if !greeted => Some(failure(
                id,
                WireError::new(ErrorCode::HelloRequired, "send hello first"),
            )),
            FromClient::Message(ClientMessage::Request { id, token, call }) => {
                dispatch(&state, &mut conn, id, token, call).await
            }
        };
        if let Some(reply) = reply {
            let _ = conn.out.send(reply).await;
        }
    }
    for task in conn.tasks.into_values() {
        task.abort();
    }
    writing.abort();
}
