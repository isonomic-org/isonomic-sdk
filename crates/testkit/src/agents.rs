//! The agents interface, served by harness-host's `MemoryHost`: the same
//! host logic the engine links, over in-memory stores, a scripted model and
//! a manual clock. Every granted app is first-party, and the token check
//! comes first, through `proto::auth`, as for every interface.
//!
//! The host runs no runtime of its own. Each call drives it to completion
//! (`MemoryHost::block_on` runs whatever the call started), so a test sees
//! a scripted turn finished by the time `sessions.send` answers. Timers move
//! only with [`FakeAgents::advance`].

use std::collections::HashSet;
use std::sync::Arc;

use harness_host::{
    AppKind, Caller as HostCaller, CapabilityToken, HostError, MemoryHost, UserActionToken,
};
use isonomic_proto::agents::schema::Event as LogEvent;
use isonomic_proto::agents::{AppId, ApprovalId, ForkReason, SessionEvent, SessionId};
use isonomic_proto::auth::Caller;
use isonomic_proto::{Call, ErrorCode, Event, Reply, ServerMessage, Token, WireError};

use crate::agents_map as map;
use crate::conn::{failure, result, Conn};
use crate::State;

/// The host, and which apps it has been told about.
pub(crate) struct AgentsState {
    memory: MemoryHost,
    known: HashSet<String>,
}

impl Default for AgentsState {
    fn default() -> Self {
        Self {
            memory: MemoryHost::new(),
            known: HashSet::new(),
        }
    }
}

impl AgentsState {
    /// The host's caller for an SDK caller. An app is introduced to the
    /// host (and counted as connected) the first time it calls.
    fn caller(&mut self, caller: Caller<'_>) -> HostCaller {
        match caller {
            Caller::Shell => HostCaller::shell(),
            Caller::App { id, .. } => self.app(id),
        }
    }

    fn app(&mut self, id: &str) -> HostCaller {
        let caller = HostCaller::app(id, AppKind::FirstParty);
        if self.known.insert(id.to_string()) {
            let token = CapabilityToken(format!("testkit:{id}"));
            self.memory.insert_token(token, caller.clone());
        }
        caller
    }
}

/// Drive one host call to completion, with whatever it started.
fn run<T>(
    memory: &MemoryHost,
    call: impl std::future::Future<Output = Result<T, HostError>>,
) -> Result<T, HostError> {
    memory.block_on(call)
}

/// Ids are minted per session, and the call named another session.
fn not_in_session(what: &str) -> WireError {
    let message = format!("no such {what} in this session");
    WireError::new(ErrorCode::NotFound, message)
}

/// The caller a call naming an app acts as. `proto::auth` has already
/// refused an app naming another app, and the shell's unnamed purge; the
/// shell acts for the app it names.
fn scoped(caller: &HostCaller, scope: Option<&AppId>) -> Result<HostCaller, WireError> {
    match (scope, caller.scope.app()) {
        (None, _) => Ok(caller.clone()),
        (Some(app), Some(own)) if app == own => Ok(caller.clone()),
        (Some(_), Some(_)) => Err(WireError::new(
            ErrorCode::NotFound,
            "no such app for this caller",
        )),
        (Some(app), None) => Ok(HostCaller::app(&app.to_string(), AppKind::FirstParty)),
    }
}

fn answer(memory: &MemoryHost, caller: &HostCaller, call: Call) -> Result<Reply, WireError> {
    let host = &memory.host;
    let done = |r: Result<(), HostError>| r.map(|()| Reply::Done {});
    let reply = match call {
        Call::SessionsCreate(r) => {
            let (preset, options) = map::create(r);
            run(memory, host.create_session(caller, &preset, options))
                .map(|session| Reply::SessionCreated { session })
        }
        Call::SessionsList(f) => {
            run(memory, host.list_sessions(caller, map::filter(f))).map(|page| Reply::Sessions {
                items: page.items.into_iter().map(map::summary).collect(),
                next: page.next,
            })
        }
        Call::SessionsOpen(r) => run(memory, host.open_session(caller, &r.session))
            .map(|s| Reply::SessionSnapshot(Box::new(map::snapshot(s)))),
        Call::SessionsFork(r) => {
            let reason = r.reason.unwrap_or(ForkReason::Branch);
            let at = map::fork_point(r.at);
            run(memory, host.fork_session(caller, &r.session, at, reason))
                .map(|session| Reply::SessionCreated { session })
        }
        Call::SessionsArchive(r) => done(run(
            memory,
            host.archive_session(caller, &r.session, r.archived),
        )),
        Call::SessionsRename(r) => done(run(
            memory,
            host.rename_session(caller, &r.session, r.title),
        )),
        Call::SessionsSend(r) => {
            let session = r.session.clone();
            run(memory, host.send(caller, &session, map::input(r)))
                .map(|a| Reply::Accepted { queued: a.queued })
        }
        Call::SessionsContinue(r) => done(run(memory, host.continue_reply(caller, &r.session))),
        Call::SessionsCancel(r) => done(run(memory, host.cancel(caller, &r.session))),
        Call::SessionsPurge(r) => {
            let caller = scoped(caller, r.app.as_ref())?;
            run(memory, host.purge(&caller)).map(|r| Reply::Purged(map::purged(r)))
        }
        Call::ApprovalsList(r) => {
            let scope = r.app;
            scoped(caller, scope.as_ref())?;
            run(memory, host.list_approvals(caller)).map(|list| {
                let list = list
                    .into_iter()
                    .filter(|a| scope.as_ref().is_none_or(|app| &a.owner == app));
                Reply::Approvals(list.map(map::approval).collect())
            })
        }
        Call::ApprovalsDecide(r) => {
            run(memory, host.open_session(caller, &r.session)).map_err(map::error)?;
            let asked = memory.log(&r.session).iter().any(
                |e| matches!(&e.event, LogEvent::ApprovalRequest(a) if a.approval == r.approval),
            );
            if !asked {
                return Err(not_in_session("approval"));
            }
            let token = r.user_action.map(UserActionToken);
            let decision = map::decision(r.decision);
            done(run(
                memory,
                host.decide_approval(caller, &r.approval, decision, token),
            ))
        }
        Call::QuestionsAnswer(r) => {
            let open = run(memory, host.open_session(caller, &r.session)).map_err(map::error)?;
            if !open.questions.contains(&r.question) {
                return Err(not_in_session("question"));
            }
            let question = r.question.clone();
            done(run(
                memory,
                host.answer_question(caller, &question, map::answer(r)),
            ))
        }
        Call::ToolsRegisterClient(r) => done(run(
            memory,
            host.register_client_tools(caller, &r.session, r.specs),
        )),
        Call::ToolsResult(r) => {
            run(memory, host.open_session(caller, &r.session)).map_err(map::error)?;
            let asked = memory
                .log(&r.session)
                .iter()
                .any(|e| matches!(&e.event, LogEvent::ToolCall(c) if c.call == r.call));
            if !asked {
                return Err(not_in_session("tool call"));
            }
            let call = r.call.clone();
            done(run(
                memory,
                host.client_tool_result(caller, &call, map::outcome(r)),
            ))
        }
        Call::PresetsList(_) => run(memory, host.list_presets(caller))
            .map(|list| Reply::Presets(list.into_iter().map(map::preset_summary).collect())),
        Call::PresetsGet(r) => run(memory, host.get_preset(caller, &r.preset))
            .map(|p| Reply::Preset(Box::new(map::preset(p)))),
        Call::PresetsPut(r) => run(memory, host.put_preset(caller, r.text))
            .map(|reference| Reply::PresetSaved { reference }),
        Call::ArtifactsGet(r) => run(memory, host.get_artifact(caller, &r.artifact, r.version))
            .map(|a| Reply::Artifact(Box::new(map::artifact(a)))),
        Call::ArtifactsList(r) => {
            run(memory, host.list_artifacts(caller, &r.session)).map(Reply::Artifacts)
        }
        call => {
            let message = format!("{} is not an agents call", call.method());
            return Err(WireError::new(ErrorCode::UnknownMethod, message));
        }
    };
    reply.map_err(map::error)
}

/// Answer one agents call, or `None` when a subscription answered itself.
pub(crate) async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    caller: Caller<'_>,
    token: Option<Token>,
    call: Call,
) -> Option<ServerMessage> {
    match call {
        Call::SessionsSubscribe(r) => {
            let opened = {
                let mut agents = state.agents.lock().unwrap();
                let caller = agents.caller(caller);
                let host = &agents.memory.host;
                agents
                    .memory
                    .block_on(host.subscribe(&caller, &r.session, r.from_seq))
            };
            let mut feed = match opened {
                Ok(feed) => feed,
                Err(e) => return Some(failure(id, map::error(e))),
            };
            let subscription = conn.open_subscription();
            let reply = result(id, Reply::SessionStream { subscription });
            let _ = conn.out.send(reply).await;
            let out = conn.out.clone();
            let task = tokio::spawn(async move {
                while let Some(event) = feed.next().await {
                    // Deltas stream ahead of the event that makes them final.
                    for delta in feed.deltas() {
                        let event = Event::LiveDelta(delta);
                        if out
                            .send(ServerMessage::Event {
                                subscription,
                                event,
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    let event = Event::SessionEvent(Box::new(event));
                    if out
                        .send(ServerMessage::Event {
                            subscription,
                            event,
                        })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });
            conn.track(subscription, task, None, token);
            None
        }
        Call::SessionsUnsubscribe { subscription } => {
            Some(match conn.close_owned(subscription, token.as_ref()) {
                true => result(id, Reply::Unsubscribed {}),
                false => failure(
                    id,
                    WireError::new(ErrorCode::UnknownSubscription, "no such session stream"),
                ),
            })
        }
        call => {
            let mut agents = state.agents.lock().unwrap();
            let caller = agents.caller(caller);
            Some(match answer(&agents.memory, &caller, call) {
                Ok(reply) => result(id, reply),
                Err(error) => failure(id, error),
            })
        }
    }
}

/// The test engine's agent host, for scripting what the engine side does:
/// the model's replies, the clock, a person approving, an app going away.
/// From [`TestEngine::agents`](crate::TestEngine::agents).
#[derive(Clone)]
pub struct FakeAgents {
    pub(crate) state: Arc<State>,
}

impl FakeAgents {
    /// Work with the host directly: its scripted model
    /// (`memory.model.text(..)`, `.calls(..)`, `.push(..)`), its store, its
    /// clients. Do not hold the host across an `.await`.
    pub fn with<T>(&self, f: impl FnOnce(&MemoryHost) -> T) -> T {
        f(&self.state.agents.lock().unwrap().memory)
    }

    /// The model's next reply: this text, then the end of the turn. With
    /// nothing scripted, the model answers "OK.".
    pub fn reply(&self, text: &str) {
        self.with(|memory| memory.model.text(text));
    }

    /// Move the host's clock, then run its timers, its watchdog and what
    /// they start.
    pub fn advance(&self, ms: u64) {
        self.with(|memory| memory.advance(ms));
    }

    /// A user-action token, as the window mints one when a person acts on
    /// `approval`, for the app `app` (the shell when `None`). Pass it to
    /// `Agents::decide`.
    pub fn user_action(&self, app: Option<&str>, approval: &ApprovalId) -> String {
        let mut agents = self.state.agents.lock().unwrap();
        let caller = match app {
            None => HostCaller::shell(),
            Some(app) => agents.app(app),
        };
        agents.memory.user_action(&caller, approval).0
    }

    /// The app goes away: its open client tool calls wait for it, and its
    /// sessions say `awaiting_client`.
    pub fn disconnect(&self, app: &str) {
        let mut agents = self.state.agents.lock().unwrap();
        agents.app(app);
        agents.memory.disconnect(app);
    }

    /// The app comes back.
    pub fn reconnect(&self, app: &str) {
        let mut agents = self.state.agents.lock().unwrap();
        agents.app(app);
        agents.memory.reconnect(app);
    }

    /// A session's events as the host stored them.
    pub fn log(&self, session: &SessionId) -> Vec<SessionEvent> {
        self.with(|memory| memory.log(session))
    }
}
