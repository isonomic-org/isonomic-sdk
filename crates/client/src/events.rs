// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `events` interface: publish to the app's own scope and listen to it,
//! so an app's tabs and windows stay in step.

use isonomic_proto::{AppEvent, Call, Event, Json, Reply, Token};

use crate::{Client, ClientError, Subscription};

/// Event calls, made as the shell or under an app's token.
#[derive(Clone)]
pub struct Events {
    client: Client,
    token: Option<Token>,
}

impl Events {
    pub(crate) fn new(client: Client, token: Option<Token>) -> Self {
        Self { client, token }
    }

    /// Send `data` to every current subscriber of `topic`. `sender` comes
    /// back on each delivered event, so a tab can skip its own messages.
    pub async fn publish(
        &self,
        topic: &str,
        data: Json,
        sender: Option<&str>,
    ) -> Result<(), ClientError> {
        let call = Call::EventsPublish {
            topic: topic.to_string(),
            data,
            sender: sender.map(str::to_string),
            app: None,
        };
        match self.client.send(self.token.clone(), call, None).await? {
            Reply::Published {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Listen to `topics`, or to every topic when it is empty.
    pub async fn subscribe(&self, topics: &[&str]) -> Result<EventStream, ClientError> {
        let call = Call::EventsSubscribe {
            topics: topics.iter().map(|t| t.to_string()).collect(),
            app: None,
        };
        let (_, events) = self.client.open(self.token.clone(), call).await?;
        Ok(EventStream {
            events,
            client: self.client.clone(),
            token: self.token.clone(),
        })
    }
}

/// Live app events. Dropping it unsubscribes.
pub struct EventStream {
    events: Subscription,
    client: Client,
    token: Option<Token>,
}

impl EventStream {
    pub fn subscription(&self) -> u64 {
        self.events.id()
    }

    /// The next event; `None` once the connection drops (subscribe again
    /// after reconnecting).
    pub async fn next(&mut self) -> Option<AppEvent> {
        loop {
            match self.events.next().await? {
                Event::AppEvent(event) => return Some(event),
                _ => continue,
            }
        }
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventStream({})", self.events.id())
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        let call = Call::EventsUnsubscribe {
            subscription: self.events.id(),
        };
        self.client.send_now(self.token.clone(), call);
    }
}
