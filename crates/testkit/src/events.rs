// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `events` interface of the in-memory engine: one broadcast channel
//! per scope (an app's id, or `shell`), shared by every connection, so
//! publishing from one window reaches the app's tabs in every window.

use std::collections::HashMap;
use std::sync::Arc;

use isonomic_proto::events::{matches, valid_topic};
use isonomic_proto::{AppEvent, Call, ErrorCode, Event, Reply, ServerMessage, Token, WireError};
use tokio::sync::broadcast;

use crate::conn::{failure, result, Conn};
use crate::State;

#[derive(Default)]
pub(crate) struct EventsState {
    scopes: HashMap<String, broadcast::Sender<AppEvent>>,
}

impl EventsState {
    fn channel(&mut self, scope: &str) -> broadcast::Sender<AppEvent> {
        self.scopes
            .entry(scope.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}

/// The scope a call reaches: the app named (shell only, checked by
/// `authorize`), else the caller's own.
fn scope(app: Option<&str>, named: Option<String>) -> String {
    named.unwrap_or_else(|| app.unwrap_or("shell").to_string())
}

pub(crate) async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    app: Option<&str>,
    token: Option<Token>,
    call: Call,
) -> Option<ServerMessage> {
    match call {
        Call::EventsPublish {
            topic,
            data,
            sender,
            app: named,
        } => {
            if !valid_topic(&topic) {
                let message = format!(
                    "topic {topic:?} must be lowercase letters, digits, dots, dashes and underscores"
                );
                return Some(failure(id, WireError::new(ErrorCode::BadRequest, message)));
            }
            let channel = state.events.lock().unwrap().channel(&scope(app, named));
            let _ = channel.send(AppEvent {
                topic,
                data,
                sender,
            });
            Some(result(id, Reply::Published {}))
        }
        Call::EventsSubscribe { topics, app: named } => {
            if let Some(bad) = topics.iter().find(|t| !valid_topic(t)) {
                let message = format!("topic {bad:?} is not a valid topic");
                return Some(failure(id, WireError::new(ErrorCode::BadRequest, message)));
            }
            let mut receiver = state
                .events
                .lock()
                .unwrap()
                .channel(&scope(app, named))
                .subscribe();
            let subscription = conn.open_subscription();
            let reply = result(id, Reply::Listening { subscription });
            let _ = conn.out.send(reply).await;
            let out = conn.out.clone();
            let task = tokio::spawn(async move {
                loop {
                    let event = match receiver.recv().await {
                        Ok(event) => event,
                        // Live only: a lagging subscriber misses messages.
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => return,
                    };
                    if !matches(&topics, &event.topic) {
                        continue;
                    }
                    let message = ServerMessage::Event {
                        subscription,
                        event: Event::AppEvent(event),
                    };
                    if out.send(message).await.is_err() {
                        return;
                    }
                }
            });
            conn.track(subscription, task, None, token);
            None
        }
        Call::EventsUnsubscribe { subscription } => {
            Some(match conn.close_owned(subscription, token.as_ref()) {
                true => result(id, Reply::Unsubscribed {}),
                false => failure(
                    id,
                    WireError::new(ErrorCode::UnknownSubscription, "no such subscription"),
                ),
            })
        }
        other => Some(failure(
            id,
            WireError::new(
                ErrorCode::UnknownMethod,
                format!("{} is not an events method", other.method()),
            ),
        )),
    }
}
