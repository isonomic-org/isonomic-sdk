// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The background task behind a `Client`: connect, greet, subscribe, route
//! replies and events, fold status into the live view, and reconnect with
//! backoff.

use std::collections::HashMap;

use isonomic_proto::agents::SchemaVersion;
use isonomic_proto::io::{read_frame, write_message};
use isonomic_proto::lenient::{decode_server, FromServer};
use isonomic_proto::{Call, ClientMessage, Event, Outcome, Reply, ServerMessage, PROTOCOL_VERSION};
use tokio::io::ReadHalf;
use tokio::sync::{mpsc, oneshot, watch};

use crate::{BoxedStream, ClientError, ConnectionStates, Connector, Live, Options, Request};

/// What the actor keeps current for views.
pub(crate) struct Watches {
    pub(crate) live: watch::Sender<Live>,
    /// The newest state of each connection, from `connection` events.
    pub(crate) connections: std::sync::Arc<watch::Sender<ConnectionStates>>,
}

/// The status subscription always uses this id; caller requests start at 1.
const SUBSCRIBE_ID: u64 = 0;

struct Pending {
    reply: oneshot::Sender<Result<Reply, ClientError>>,
    events: Option<mpsc::UnboundedSender<Event>>,
}

/// Per-connection routing state. Dropped on disconnect, which ends every
/// open subscription and fails every pending call.
#[derive(Default)]
struct Routes {
    pending: HashMap<u64, Pending>,
    subscriptions: HashMap<u64, mpsc::UnboundedSender<Event>>,
}

/// Connect and complete the version handshake.
async fn open(connector: &Connector, name: &str) -> Option<(BoxedStream, Welcome)> {
    let mut stream = connector().await.ok()?;
    let hello = ClientMessage::Hello {
        versions: vec![PROTOCOL_VERSION],
        client: name.to_string(),
    };
    write_message(&mut stream, &hello).await.ok()?;
    match read_frame(&mut stream)
        .await
        .ok()?
        .map(|body| decode_server(&body))
    {
        Some(FromServer::Message(ServerMessage::Welcome { engine, schema, .. })) => {
            Some((stream, Welcome { engine, schema }))
        }
        _ => None,
    }
}

/// Reading a frame cannot be cancelled halfway without losing bytes, so a
/// dedicated task reads and hands over whole frames. Frames this build
/// cannot read are skipped, except responses, whose callers must hear back.
fn spawn_reader(mut reader: ReadHalf<BoxedStream>) -> mpsc::Receiver<FromServer> {
    let (messages, incoming) = mpsc::channel(64);
    tokio::spawn(async move {
        while let Ok(Some(body)) = read_frame(&mut reader).await {
            let decoded = decode_server(&body);
            if matches!(decoded, FromServer::Skip) {
                continue;
            }
            if messages.send(decoded).await.is_err() {
                return;
            }
        }
    });
    incoming
}

fn route(incoming: FromServer, routes: &mut Routes, watches: &Watches) {
    let live = &watches.live;
    let message = match incoming {
        FromServer::Message(message) => message,
        FromServer::UnreadableResponse { id } => {
            if let Some(pending) = routes.pending.remove(&id) {
                let _ = pending.reply.send(Err(ClientError::UnexpectedReply));
            }
            return;
        }
        FromServer::Skip => return,
    };
    match message {
        ServerMessage::Response {
            id: SUBSCRIBE_ID,
            outcome,
        } => {
            if let Outcome::Result(Reply::Subscribed { snapshot, .. }) = outcome {
                live.send_modify(|view| view.system = Some(snapshot));
            }
        }
        ServerMessage::Response { id, outcome } => {
            let Some(pending) = routes.pending.remove(&id) else {
                return;
            };
            let result = match outcome {
                Outcome::Result(reply) => {
                    // Register before delivering, so no event can slip past.
                    if let (Some(events), Some(subscription)) =
                        (pending.events, reply.opened_subscription())
                    {
                        routes.subscriptions.insert(subscription, events);
                    }
                    Ok(reply)
                }
                Outcome::Error(error) => Err(ClientError::Engine(error)),
            };
            let _ = pending.reply.send(result);
        }
        ServerMessage::Event {
            subscription,
            event,
        } => {
            if let Some(events) = routes.subscriptions.get(&subscription) {
                let last = event.is_final();
                if events.send(event).is_err() || last {
                    routes.subscriptions.remove(&subscription);
                }
            } else {
                // The status subscription: service and connection changes.
                match event {
                    Event::ServiceStatus(change) => live.send_modify(|view| {
                        if let Some(system) = &mut view.system {
                            system.apply(change);
                        }
                    }),
                    Event::Connection(change) => watches.connections.send_modify(|states| {
                        states.insert(change.id.clone(), change);
                    }),
                    _ => {}
                }
            }
        }
        ServerMessage::Welcome { .. } => {}
    }
}

/// What the engine said in `welcome`.
struct Welcome {
    engine: String,
    schema: Option<SchemaVersion>,
}

/// Run one connection until it drops. Returns `true` when every `Client`
/// handle is gone and the task should end.
async fn serve(
    stream: BoxedStream,
    welcome: Welcome,
    inbox: &mut mpsc::Receiver<Request>,
    watches: &Watches,
) -> bool {
    let live = &watches.live;
    let (reader, mut writer) = tokio::io::split(stream);
    let subscribe = ClientMessage::Request {
        id: SUBSCRIBE_ID,
        token: None,
        call: Call::SystemSubscribe,
    };
    if write_message(&mut writer, &subscribe).await.is_err() {
        return false;
    }
    let mut incoming = spawn_reader(reader);
    live.send_modify(|view| {
        view.connected = true;
        view.engine = Some(welcome.engine);
        view.schema = welcome.schema;
    });

    let mut routes = Routes::default();
    let mut next_id = SUBSCRIBE_ID + 1;
    let finished = loop {
        tokio::select! {
            message = incoming.recv() => match message {
                Some(message) => route(message, &mut routes, watches),
                None => break false,
            },
            request = inbox.recv() => match request {
                Some(Request { token, call, reply, events }) => {
                    let id = next_id;
                    next_id += 1;
                    let message = ClientMessage::Request { id, token, call };
                    if write_message(&mut writer, &message).await.is_err() {
                        let _ = reply.send(Err(ClientError::Disconnected));
                        break false;
                    }
                    routes.pending.insert(id, Pending { reply, events });
                }
                None => break true,
            },
        }
    };
    for pending in routes.pending.into_values() {
        let _ = pending.reply.send(Err(ClientError::Disconnected));
    }
    finished
}

pub(crate) async fn run(
    connector: Connector,
    options: Options,
    mut inbox: mpsc::Receiver<Request>,
    watches: Watches,
) {
    let mut delay = options.reconnect_min;
    loop {
        if let Some((stream, welcome)) = open(&connector, &options.name).await {
            delay = options.reconnect_min;
            if serve(stream, welcome, &mut inbox, &watches).await {
                return;
            }
            watches.live.send_modify(|view| {
                view.connected = false;
                view.system = None;
            });
            watches.connections.send_if_modified(|states| {
                let had = !states.is_empty();
                states.clear();
                had
            });
        }
        // Wait before the next attempt. Requests made meanwhile fail fast
        // instead of queueing behind an engine that is not there.
        let wait = tokio::time::sleep(delay);
        tokio::pin!(wait);
        loop {
            tokio::select! {
                _ = &mut wait => break,
                request = inbox.recv() => match request {
                    Some(request) => {
                        let _ = request.reply.send(Err(ClientError::Disconnected));
                    }
                    None => return,
                },
            }
        }
        delay = (delay * 2).min(options.reconnect_max);
    }
}
