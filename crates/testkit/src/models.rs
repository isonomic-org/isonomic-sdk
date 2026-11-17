// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `models` interface of the in-memory engine: a scriptable fake.
//!
//! By default it has one local chat model, `chat-small`, which echoes the
//! last user message word by word, and one local embeddings model,
//! `embed-small`. Tests add models, script streams step by step (with
//! [`Gate`]s to hold a stream at a chosen point), make the memory gate
//! refuse, make a model fail, and switch remote models on or off.

use std::sync::Arc;

use isonomic_proto::auth::{authorize_model, Caller};
use isonomic_proto::models::{ModelDetail, StreamRequest, Usage};
use isonomic_proto::{
    Call, ErrorCode, Event, MemoryRefusal, ModelChunk, ModelInfo, Reply, ServerMessage, StopReason,
    Token, WireError,
};
use tokio::sync::{oneshot, watch};

use crate::conn::{failure, result, Conn};
use crate::State;

/// A point a scripted stream waits at until the test opens it.
#[derive(Clone)]
pub struct Gate(Arc<watch::Sender<bool>>);

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    pub fn new() -> Self {
        Self(Arc::new(watch::channel(false).0))
    }

    pub fn open(&self) {
        self.0.send_replace(true);
    }

    pub(crate) async fn wait(&self) {
        let _ = self.0.subscribe().wait_for(|open| *open).await;
    }
}

/// One step of a scripted stream.
#[derive(Clone)]
pub enum Step {
    /// Send this chunk. A chunk with `stop` set ends the stream.
    Chunk(ModelChunk),
    /// Wait until the gate opens (or the stream is cancelled).
    Hold(Gate),
    /// End the stream with `stop: error` and this error, like a crashed
    /// worker.
    Fail(WireError),
}

pub(crate) struct FakeModel {
    pub(crate) info: ModelInfo,
    pub(crate) size_bytes: Option<u64>,
    /// Its file is on this machine. `false` until `models.download` ends.
    pub(crate) installed: bool,
    script: Option<Vec<Step>>,
    refusal: Option<MemoryRefusal>,
    failure: Option<WireError>,
}

pub(crate) struct ModelsState {
    pub(crate) models: Vec<FakeModel>,
    default_chat: Option<String>,
    default_embeddings: Option<String>,
    remote_enabled: bool,
    requests: Vec<StreamRequest>,
}

fn info(id: &str, capability: &str) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: id.to_string(),
        context_length: Some(8192),
        capabilities: vec![capability.to_string()],
        remote: false,
        recommended: false,
    }
}

/// The model as the caller sees it: `recommended` set on the one the
/// engine recommends for this machine's profile.
fn shown(model: &FakeModel, recommended: Option<&str>) -> ModelInfo {
    ModelInfo {
        recommended: recommended == Some(model.info.id.as_str()),
        ..model.info.clone()
    }
}

impl Default for ModelsState {
    fn default() -> Self {
        let local = |info: ModelInfo| FakeModel {
            info,
            size_bytes: Some(2 << 30),
            installed: true,
            script: None,
            refusal: None,
            failure: None,
        };
        Self {
            models: vec![
                local(info("chat-small", "chat")),
                local(info("embed-small", "embeddings")),
            ],
            default_chat: Some("chat-small".into()),
            default_embeddings: Some("embed-small".into()),
            remote_enabled: false,
            requests: Vec::new(),
        }
    }
}

/// Scripting handle for the fake models. Cheap to clone.
#[derive(Clone)]
pub struct FakeModels {
    pub(crate) state: Arc<State>,
}

impl FakeModels {
    fn with<R>(&self, f: impl FnOnce(&mut ModelsState) -> R) -> R {
        f(&mut self.state.models.lock().unwrap())
    }

    pub(crate) fn with_model(&self, id: &str, f: impl FnOnce(&mut FakeModel)) {
        self.with(|m| match m.models.iter_mut().find(|m| m.info.id == id) {
            Some(model) => f(model),
            None => panic!("no fake model {id}"),
        })
    }

    /// Add a model (or replace one with the same id).
    pub fn add(&self, info: ModelInfo) {
        self.with(|m| {
            m.models.retain(|existing| existing.info.id != info.id);
            m.models.push(FakeModel {
                size_bytes: (!info.remote).then_some(2 << 30),
                installed: true,
                info,
                script: None,
                refusal: None,
                failure: None,
            });
        });
    }

    /// Every later stream on this model plays these steps.
    pub fn script(&self, id: &str, steps: Vec<Step>) {
        self.with_model(id, |model| model.script = Some(steps));
    }

    /// The memory gate refuses this model until `admit` is called.
    pub fn refuse(&self, id: &str, required_bytes: u64, available_bytes: u64, reason: &str) {
        let refusal = MemoryRefusal {
            required_bytes,
            available_bytes,
            reason: reason.to_string(),
        };
        self.with_model(id, |model| model.refusal = Some(refusal));
    }

    pub fn admit(&self, id: &str) {
        self.with_model(id, |model| model.refusal = None);
    }

    /// Every call on this model fails with `error` until `recover`.
    pub fn fail(&self, id: &str, error: WireError) {
        self.with_model(id, |model| model.failure = Some(error));
    }

    pub fn recover(&self, id: &str) {
        self.with_model(id, |model| model.failure = None);
    }

    /// The user's global switch for remote models. Off by default.
    pub fn set_remote_enabled(&self, enabled: bool) {
        self.with(|m| m.remote_enabled = enabled);
    }

    pub fn set_default_chat(&self, id: Option<&str>) {
        self.with(|m| m.default_chat = id.map(str::to_string));
    }

    /// Every stream request received, oldest first.
    pub fn requests(&self) -> Vec<StreamRequest> {
        self.with(|m| m.requests.clone())
    }

    /// How many streams were cancelled.
    pub fn cancels(&self) -> u32 {
        *self.state.cancels.borrow()
    }

    /// Wait until at least `count` streams were cancelled.
    pub async fn wait_for_cancels(&self, count: u32) {
        let _ = self
            .state
            .cancels
            .subscribe()
            .wait_for(|n| *n >= count)
            .await;
    }
}

/// What a caller may see of a model: the shell everything, an app what its
/// manifest permits.
fn visible(caller: Caller<'_>, info: &ModelInfo) -> bool {
    authorize_model(caller, &info.id, info.remote).is_ok()
}

fn echo(request: &StreamRequest) -> Vec<Step> {
    let last = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == isonomic_proto::models::Role::User)
        .map(|m| m.content.as_str())
        .unwrap_or("nothing");
    let mut steps = Vec::new();
    if request.reasoning == Some(true) {
        steps.push(Step::Chunk(ModelChunk::reasoning("Repeating the message.")));
    }
    let reply = format!("You said: {last}");
    let words: Vec<&str> = reply.split_inclusive(' ').collect();
    for word in &words {
        steps.push(Step::Chunk(ModelChunk::text(*word)));
    }
    let input: usize = request
        .messages
        .iter()
        .map(|m| m.content.split_whitespace().count())
        .sum();
    steps.push(Step::Chunk(ModelChunk {
        usage: Some(Usage {
            input_tokens: input as u32,
            output_tokens: words.len() as u32,
            reasoning_tokens: None,
            cost_micros: Some(0),
        }),
        stop: Some(StopReason::End),
        ..ModelChunk::default()
    }));
    steps
}

/// A vector that depends only on the text, so tests can compare.
fn embedding(text: &str) -> Vec<f32> {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    (0..8)
        .map(|_| {
            for byte in text.bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
            hash = hash.rotate_left(13);
            ((hash & 0xff) as f32 - 128.0) / 128.0
        })
        .collect()
}

async fn play(
    steps: Vec<Step>,
    subscription: u64,
    conn_out: tokio::sync::mpsc::Sender<ServerMessage>,
    mut cancel: oneshot::Receiver<()>,
) {
    let send = |chunk: ModelChunk| {
        let out = conn_out.clone();
        async move {
            let event = ServerMessage::Event {
                subscription,
                event: Event::ModelChunk(chunk),
            };
            out.send(event).await.is_ok()
        }
    };
    for step in steps {
        if cancel.try_recv().is_ok() {
            send(ModelChunk::stop(StopReason::Cancelled)).await;
            return;
        }
        match step {
            Step::Chunk(chunk) => {
                let last = chunk.stop.is_some();
                if !send(chunk).await || last {
                    return;
                }
            }
            Step::Hold(gate) => {
                tokio::select! {
                    _ = &mut cancel => {
                        send(ModelChunk::stop(StopReason::Cancelled)).await;
                        return;
                    }
                    _ = gate.wait() => {}
                }
            }
            Step::Fail(error) => {
                send(ModelChunk {
                    stop: Some(StopReason::Error),
                    error: Some(error),
                    ..ModelChunk::default()
                })
                .await;
                return;
            }
        }
    }
    send(ModelChunk::stop(StopReason::End)).await;
}

/// Resolve the model a call runs on, and refuse what the caller may not
/// use or the model cannot do.
fn resolve(
    state: &State,
    caller: Caller<'_>,
    named: Option<&str>,
    capability: &str,
) -> Result<(ModelInfo, Option<Vec<Step>>), WireError> {
    let models = state.models.lock().unwrap();
    let default = match capability {
        "chat" => &models.default_chat,
        _ => &models.default_embeddings,
    };
    let Some(id) = named.map(str::to_string).or_else(|| default.clone()) else {
        return Err(WireError::new(
            ErrorCode::UnknownModel,
            format!("no default {capability} model is set"),
        ));
    };
    let Some(model) = models.models.iter().find(|m| m.info.id == id) else {
        return Err(WireError::new(
            ErrorCode::UnknownModel,
            format!("no model {id}"),
        ));
    };
    authorize_model(caller, &model.info.id, model.info.remote)?;
    if !model.info.supports(capability) {
        return Err(WireError::new(
            ErrorCode::Unsupported,
            format!("model {id} does not do {capability}"),
        ));
    }
    if model.info.remote && !models.remote_enabled {
        return Err(WireError::new(
            ErrorCode::RemoteDisabled,
            "remote models are switched off",
        ));
    }
    if !model.installed {
        return Err(WireError::new(
            ErrorCode::ModelUnavailable,
            format!("model {id} is not installed; download it first"),
        ));
    }
    if let Some(error) = &model.failure {
        return Err(error.clone());
    }
    if let Some(refusal) = &model.refusal {
        return Err(WireError::memory(refusal.clone()));
    }
    Ok((model.info.clone(), model.script.clone()))
}

/// May a session run on `id` for this caller? The checks `models.stream`
/// makes, for `sessions.create { model }`.
pub(crate) fn session_model(state: &State, caller: Caller<'_>, id: &str) -> Result<(), WireError> {
    // The manifest first, so an app learns nothing about models it may not
    // use; whether the model is remote needs the model, so that comes after.
    authorize_model(caller, id, false)?;
    let models = state.models.lock().unwrap();
    let Some(model) = models.models.iter().find(|m| m.info.id == id) else {
        return Err(WireError::new(
            ErrorCode::UnknownModel,
            format!("no model {id}"),
        ));
    };
    authorize_model(caller, &model.info.id, model.info.remote)?;
    if !model.info.supports("chat") {
        let message = format!("model {id} does not do chat");
        return Err(WireError::new(ErrorCode::Unsupported, message));
    }
    if !model.installed {
        let message = format!("model {id} is not installed; download it first");
        return Err(WireError::new(ErrorCode::ModelUnavailable, message));
    }
    Ok(())
}

/// Answer one `models.*` call. `None` when the answer was already sent.
pub(crate) async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    caller: Caller<'_>,
    token: Option<Token>,
    call: Call,
) -> Option<ServerMessage> {
    let answer = match call {
        Call::ModelsList => {
            let recommended = crate::downloads::recommended_id(state);
            let models = state.models.lock().unwrap();
            let list = models
                .models
                .iter()
                .map(|m| shown(m, recommended.as_deref()))
                .filter(|info| visible(caller, info))
                .collect();
            Ok(Reply::Models(list))
        }
        Call::ModelsInfo { model } => {
            let recommended = crate::downloads::recommended_id(state);
            let models = state.models.lock().unwrap();
            match models.models.iter().find(|m| m.info.id == model) {
                None => Err(WireError::new(
                    ErrorCode::UnknownModel,
                    format!("no model {model}"),
                )),
                Some(found) => {
                    authorize_model(caller, &found.info.id, found.info.remote).map(|()| {
                        Reply::Model(ModelDetail {
                            info: shown(found, recommended.as_deref()),
                            size_bytes: found.size_bytes,
                            license: (!found.info.remote).then(|| "Test License".to_string()),
                            state: crate::downloads::state_of(state, found),
                        })
                    })
                }
            }
        }
        Call::ModelsEmbed { model, input } => {
            resolve(state, caller, model.as_deref(), "embeddings").map(|(info, _)| {
                Reply::Embeddings {
                    model: info.id,
                    vectors: input.iter().map(|text| embedding(text)).collect(),
                }
            })
        }
        Call::ModelsCancel { subscription } => match conn.cancel(subscription, token.as_ref()) {
            true => {
                state.cancels.send_modify(|n| *n += 1);
                Ok(Reply::Cancelled {})
            }
            false => Err(WireError::new(
                ErrorCode::UnknownSubscription,
                format!("no running stream {subscription}"),
            )),
        },
        Call::ModelsStream(request) => {
            match resolve(state, caller, request.model.as_deref(), "chat") {
                Err(error) => Err(error),
                Ok((info, script)) => {
                    let steps = script.unwrap_or_else(|| echo(&request));
                    state.models.lock().unwrap().requests.push(request);
                    let subscription = conn.open_subscription();
                    let reply = Reply::Stream {
                        subscription,
                        model: info.id,
                    };
                    // The reply goes out before the first chunk.
                    let _ = conn.out.send(result(id, reply)).await;
                    let (stop, cancel) = oneshot::channel();
                    let task = tokio::spawn(play(steps, subscription, conn.out.clone(), cancel));
                    conn.track(subscription, task, Some(stop), token);
                    return None;
                }
            }
        }
        Call::ModelsDownload(_) | Call::ModelsDownloadCancel(_) | Call::ModelsRecommended(_) => {
            return crate::downloads::dispatch(state, conn, id, token, call).await;
        }
        other => Err(WireError::new(
            ErrorCode::UnknownMethod,
            format!("the testkit has no method {}", other.method()),
        )),
    };
    Some(match answer {
        Ok(reply) => result(id, reply),
        Err(error) => failure(id, error),
    })
}
