//! The background task behind a `Client`: connect, greet, subscribe, route
//! replies, fold events into the live view, and reconnect with backoff.

use std::collections::HashMap;

use isonomic_proto::io::{read_message, write_message};
use isonomic_proto::{Call, ClientMessage, Event, Outcome, Reply, ServerMessage, PROTOCOL_VERSION};
use tokio::io::ReadHalf;
use tokio::sync::{mpsc, oneshot, watch};

use crate::{BoxedStream, ClientError, Connector, Live, Options, Request};

/// The subscription request always uses this id; caller requests start at 1.
const SUBSCRIBE_ID: u64 = 0;

type Pending = HashMap<u64, oneshot::Sender<Result<Reply, ClientError>>>;

/// Connect and complete the version handshake.
async fn open(connector: &Connector, name: &str) -> Option<(BoxedStream, String)> {
    let mut stream = connector().await.ok()?;
    let hello = ClientMessage::Hello {
        versions: vec![PROTOCOL_VERSION],
        client: name.to_string(),
    };
    write_message(&mut stream, &hello).await.ok()?;
    match read_message::<_, ServerMessage>(&mut stream).await {
        Ok(Some(ServerMessage::Welcome { engine, .. })) => Some((stream, engine)),
        _ => None,
    }
}

/// Reading a frame cannot be cancelled halfway without losing bytes, so a
/// dedicated task reads and hands over whole messages.
fn spawn_reader(mut reader: ReadHalf<BoxedStream>) -> mpsc::Receiver<ServerMessage> {
    let (messages, incoming) = mpsc::channel(64);
    tokio::spawn(async move {
        while let Ok(Some(message)) = read_message::<_, ServerMessage>(&mut reader).await {
            if messages.send(message).await.is_err() {
                return;
            }
        }
    });
    incoming
}

fn route(message: ServerMessage, pending: &mut Pending, live: &watch::Sender<Live>) {
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
            if let Some(reply) = pending.remove(&id) {
                let _ = reply.send(match outcome {
                    Outcome::Result(result) => Ok(result),
                    Outcome::Error(error) => Err(ClientError::Engine(error)),
                });
            }
        }
        ServerMessage::Event {
            event: Event::ServiceStatus(change),
            ..
        } => live.send_modify(|view| {
            if let Some(system) = &mut view.system {
                system.apply(change);
            }
        }),
        ServerMessage::Welcome { .. } => {}
    }
}

/// Run one connection until it drops. Returns `true` when every `Client`
/// handle is gone and the task should end.
async fn serve(
    stream: BoxedStream,
    engine: String,
    inbox: &mut mpsc::Receiver<Request>,
    live: &watch::Sender<Live>,
) -> bool {
    let (reader, mut writer) = tokio::io::split(stream);
    let subscribe = ClientMessage::Request {
        id: SUBSCRIBE_ID,
        call: Call::SystemSubscribe,
    };
    if write_message(&mut writer, &subscribe).await.is_err() {
        return false;
    }
    let mut incoming = spawn_reader(reader);
    live.send_modify(|view| {
        view.connected = true;
        view.engine = Some(engine);
    });

    let mut pending = Pending::new();
    let mut next_id = SUBSCRIBE_ID + 1;
    let finished = loop {
        tokio::select! {
            message = incoming.recv() => match message {
                Some(message) => route(message, &mut pending, live),
                None => break false,
            },
            request = inbox.recv() => match request {
                Some(Request { call, reply }) => {
                    let id = next_id;
                    next_id += 1;
                    if write_message(&mut writer, &ClientMessage::Request { id, call }).await.is_err() {
                        let _ = reply.send(Err(ClientError::Disconnected));
                        break false;
                    }
                    pending.insert(id, reply);
                }
                None => break true,
            },
        }
    };
    for reply in pending.into_values() {
        let _ = reply.send(Err(ClientError::Disconnected));
    }
    finished
}

pub(crate) async fn run(
    connector: Connector,
    options: Options,
    mut inbox: mpsc::Receiver<Request>,
    live: watch::Sender<Live>,
) {
    let mut delay = options.reconnect_min;
    loop {
        if let Some((stream, engine)) = open(&connector, &options.name).await {
            delay = options.reconnect_min;
            if serve(stream, engine, &mut inbox, &live).await {
                return;
            }
            live.send_modify(|view| {
                view.connected = false;
                view.system = None;
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
