//! The agents interface: agent sessions the engine's host runs for the app.
//! One method per host call; the session's events arrive on a
//! [`SessionStream`].

use isonomic_proto::agents::*;
use isonomic_proto::{Call, Event, Reply, Token};

use crate::{Client, ClientError, Subscription};

/// Agent session calls, made as the shell or under an app's token.
#[derive(Clone)]
pub struct Agents {
    client: Client,
    token: Option<Token>,
}

impl Agents {
    pub(crate) fn new(client: Client, token: Option<Token>) -> Self {
        Self { client, token }
    }

    async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.client.send(self.token.clone(), call, None).await
    }

    async fn done(&self, call: Call) -> Result<(), ClientError> {
        match self.call(call).await? {
            Reply::Done {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    async fn created(&self, call: Call) -> Result<SessionId, ClientError> {
        match self.call(call).await? {
            Reply::SessionCreated { session } => Ok(session),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// A new session from a preset; its class comes from the preset.
    pub async fn create(&self, preset: &str) -> Result<SessionId, ClientError> {
        self.create_with(CreateSession {
            preset: PresetId::new(preset),
            title: None,
            location_opt_in: false,
        })
        .await
    }

    pub async fn create_with(&self, request: CreateSession) -> Result<SessionId, ClientError> {
        self.created(Call::SessionsCreate(request)).await
    }

    /// One page of sessions, and the cursor for the next.
    pub async fn list(
        &self,
        filter: ListSessions,
    ) -> Result<(Vec<SessionSummary>, Option<String>), ClientError> {
        match self.call(Call::SessionsList(filter)).await? {
            Reply::Sessions { items, next } => Ok((items, next)),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// The session's summary and what is pending in it.
    pub async fn open(&self, session: &SessionId) -> Result<SessionSnapshot, ClientError> {
        let call = Call::SessionsOpen(SessionRef {
            session: session.clone(),
        });
        match self.call(call).await? {
            Reply::SessionSnapshot(snapshot) => Ok(*snapshot),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// A new session sharing this one's history up to a turn boundary.
    pub async fn fork(
        &self,
        session: &SessionId,
        at: ForkPoint,
        reason: Option<ForkReason>,
    ) -> Result<SessionId, ClientError> {
        let call = Call::SessionsFork(ForkSession {
            session: session.clone(),
            at,
            reason,
        });
        self.created(call).await
    }

    pub async fn archive(&self, session: &SessionId, archived: bool) -> Result<(), ClientError> {
        let session = session.clone();
        self.done(Call::SessionsArchive(ArchiveSession { session, archived }))
            .await
    }

    pub async fn rename(&self, session: &SessionId, title: &str) -> Result<(), ClientError> {
        let call = Call::SessionsRename(RenameSession {
            session: session.clone(),
            title: title.to_string(),
        });
        self.done(call).await
    }

    /// Send input. `true` when it waits behind a running turn.
    pub async fn send(
        &self,
        session: &SessionId,
        content: Vec<Block>,
    ) -> Result<bool, ClientError> {
        let call = Call::SessionsSend(SendInput {
            session: session.clone(),
            content,
            client_ref: None,
        });
        match self.call(call).await? {
            Reply::Accepted { queued } => Ok(queued),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    pub async fn send_text(&self, session: &SessionId, text: &str) -> Result<bool, ClientError> {
        self.send(session, vec![Block::text(text)]).await
    }

    /// Extend the last sealed partial reply.
    pub async fn continue_reply(&self, session: &SessionId) -> Result<(), ClientError> {
        let session = session.clone();
        self.done(Call::SessionsContinue(SessionRef { session }))
            .await
    }

    /// Stop the running turn. Safe to repeat.
    pub async fn cancel(&self, session: &SessionId) -> Result<(), ClientError> {
        let session = session.clone();
        self.done(Call::SessionsCancel(SessionRef { session }))
            .await
    }

    /// Delete every session of this app, and what they own.
    pub async fn purge(&self) -> Result<PurgeReport, ClientError> {
        match self.call(Call::SessionsPurge).await? {
            Reply::Purged(report) => Ok(report),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// The session's durable events after `from_seq` (0 for all), then live
    /// ones, with no gap or duplicate between them.
    pub async fn subscribe(
        &self,
        session: &SessionId,
        from_seq: Seq,
    ) -> Result<SessionStream, ClientError> {
        let call = Call::SessionsSubscribe(SubscribeSession {
            session: session.clone(),
            from_seq,
        });
        let (reply, events) = self.client.open(self.token.clone(), call).await?;
        let Reply::SessionStream { .. } = reply else {
            return Err(ClientError::UnexpectedReply);
        };
        Ok(SessionStream {
            events,
            client: self.client.clone(),
            token: self.token.clone(),
            last_seq: from_seq,
        })
    }

    /// Approvals waiting for a decision in the caller's sessions.
    pub async fn approvals(&self) -> Result<Vec<PendingApproval>, ClientError> {
        match self.call(Call::ApprovalsList).await? {
            Reply::Approvals(pending) => Ok(pending),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Decide an approval. `user_action` is the token the window minted when
    /// the person acted; without it the engine refuses.
    pub async fn decide(
        &self,
        approval: &ApprovalId,
        decision: DecisionInput,
        user_action: Option<&str>,
    ) -> Result<(), ClientError> {
        let call = Call::ApprovalsDecide(DecideApproval {
            approval: approval.clone(),
            decision,
            user_action: user_action.map(str::to_string),
        });
        self.done(call).await
    }

    pub async fn answer(&self, answer: AnswerQuestion) -> Result<(), ClientError> {
        self.done(Call::QuestionsAnswer(answer)).await
    }

    /// This app's client tools for a session, replacing any registered
    /// before. Calls to them arrive as `tool.call` events.
    pub async fn register_client_tools(
        &self,
        session: &SessionId,
        specs: Vec<ToolSpec>,
    ) -> Result<(), ClientError> {
        let session = session.clone();
        self.done(Call::ToolsRegisterClient(RegisterClientTools {
            session,
            specs,
        }))
        .await
    }

    /// What a client tool returned.
    pub async fn tool_result(&self, result: ClientToolResult) -> Result<(), ClientError> {
        self.done(Call::ToolsResult(result)).await
    }

    /// The app's presets and the built-ins.
    pub async fn presets(&self) -> Result<Vec<PresetSummary>, ClientError> {
        match self.call(Call::PresetsList).await? {
            Reply::Presets(presets) => Ok(presets),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    pub async fn preset(&self, preset: &str) -> Result<PresetDetail, ClientError> {
        let call = Call::PresetsGet(GetPreset {
            preset: PresetId::new(preset),
        });
        match self.call(call).await? {
            Reply::Preset(detail) => Ok(*detail),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Store a preset's TOML text as its next version.
    pub async fn put_preset(&self, text: &str) -> Result<PresetRef, ClientError> {
        let call = Call::PresetsPut(PutPreset {
            text: text.to_string(),
        });
        match self.call(call).await? {
            Reply::PresetSaved { reference } => Ok(reference),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// An artifact version, latest when `version` is `None`.
    pub async fn artifact(
        &self,
        artifact: &ArtifactId,
        version: Option<u32>,
    ) -> Result<ArtifactVersion, ClientError> {
        let call = Call::ArtifactsGet(GetArtifact {
            artifact: artifact.clone(),
            version,
        });
        match self.call(call).await? {
            Reply::Artifact(found) => Ok(*found),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    pub async fn artifacts(&self, session: &SessionId) -> Result<Vec<ArtifactEvent>, ClientError> {
        let session = session.clone();
        match self
            .call(Call::ArtifactsList(SessionRef { session }))
            .await?
        {
            Reply::Artifacts(list) => Ok(list),
            _ => Err(ClientError::UnexpectedReply),
        }
    }
}

/// One thing a session stream delivers.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionUpdate {
    /// A durable event, in sequence order. Events this build does not know
    /// arrive as `schema::Event::Unknown`; render them generically.
    Event(Box<SessionEvent>),
    /// Reply text as it streams. Drop deltas once the `assistant` event for
    /// their step arrives.
    Delta(LiveDelta),
}

/// A session's events. Dropping it unsubscribes.
pub struct SessionStream {
    events: Subscription,
    client: Client,
    token: Option<Token>,
    last_seq: Seq,
}

impl SessionStream {
    pub fn subscription(&self) -> u64 {
        self.events.id()
    }

    /// The last durable sequence number delivered: subscribe from it after
    /// reconnecting to miss nothing.
    pub fn last_seq(&self) -> Seq {
        self.last_seq
    }

    /// The next update. `None` once the connection drops or the session is
    /// purged.
    pub async fn next(&mut self) -> Option<SessionUpdate> {
        loop {
            match self.events.next().await? {
                Event::SessionEvent(event) => {
                    self.last_seq = event.seq;
                    return Some(SessionUpdate::Event(event));
                }
                Event::LiveDelta(delta) => return Some(SessionUpdate::Delta(delta)),
                _ => continue,
            }
        }
    }
}

impl std::fmt::Debug for SessionStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SessionStream({}, after {})",
            self.events.id(),
            self.last_seq
        )
    }
}

impl Drop for SessionStream {
    fn drop(&mut self) {
        let call = Call::SessionsUnsubscribe {
            subscription: self.events.id(),
        };
        self.client.send_now(self.token.clone(), call);
    }
}
