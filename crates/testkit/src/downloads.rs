//! Model downloads, the recommended first model and the hardware profile:
//! the window's first-run flow against the in-memory engine.
//!
//! A download belongs to the engine, not to a connection: one task plays
//! it and broadcasts its progress, and every `models.download` for the
//! model attaches a subscription to it, starting with where it stands.
//! Without a script it arrives in four quarters.

use std::collections::HashMap;
use std::sync::Arc;

use isonomic_proto::models::{
    DownloadOutcome, DownloadProgress, ModelState, RecommendModel, Recommendation,
};
use isonomic_proto::{
    Call, ErrorCode, Event, HardwareProfile, MemoryRefusal, Reply, ServerMessage, Token, WireError,
};
use tokio::sync::{broadcast, watch};

use crate::conn::{failure, result, Conn};
use crate::models::{FakeModel, FakeModels, Gate};
use crate::State;

/// The smallest machine a local model runs on.
pub const MEMORY_FLOOR: u64 = 16 << 30;

/// One step of a scripted download.
#[derive(Clone)]
pub enum DownloadStep {
    /// So many bytes have arrived, in all.
    Progress(u64),
    /// Wait until the gate opens (or the download is cancelled).
    Hold(Gate),
    /// End the download as failed, with this error.
    Fail(WireError),
}

struct Running {
    received: u64,
    events: broadcast::Sender<DownloadProgress>,
    cancel: watch::Sender<bool>,
}

pub(crate) struct DownloadsState {
    scripts: HashMap<String, Vec<DownloadStep>>,
    running: HashMap<String, Running>,
    recommended: HashMap<String, String>,
    pub(crate) profile: HardwareProfile,
}

impl Default for DownloadsState {
    fn default() -> Self {
        let recommended = [("standard", "chat-small"), ("high", "chat-small")]
            .into_iter()
            .map(|(p, m)| (p.to_string(), m.to_string()))
            .collect();
        Self {
            scripts: HashMap::new(),
            running: HashMap::new(),
            recommended,
            profile: HardwareProfile {
                profile: "standard".into(),
                total_memory_bytes: MEMORY_FLOOR,
                available_memory_bytes: 8 << 30,
            },
        }
    }
}

/// What `models.info` reports for a model.
pub(crate) fn state_of(state: &State, model: &FakeModel) -> ModelState {
    if model.installed {
        ModelState::Stopped
    } else if state
        .downloads
        .lock()
        .unwrap()
        .running
        .contains_key(&model.info.id)
    {
        ModelState::Downloading
    } else {
        ModelState::NotInstalled
    }
}

impl FakeModels {
    /// The model's file is not on this machine: `models.info` says
    /// `not_installed`, streams refuse it, and `models.download` fetches it.
    pub fn uninstall(&self, id: &str) {
        self.with_model(id, |model| model.installed = false);
    }

    /// The model's file size, `None` for unknown: downloads then report no
    /// `total_bytes`.
    pub fn set_size(&self, id: &str, size_bytes: Option<u64>) {
        self.with_model(id, |model| model.size_bytes = size_bytes);
    }

    pub fn installed(&self, id: &str) -> bool {
        let models = self.state.models.lock().unwrap();
        models.models.iter().any(|m| m.info.id == id && m.installed)
    }

    /// Every later download of this model plays these steps, then ends
    /// installed unless a step failed.
    pub fn script_download(&self, id: &str, steps: Vec<DownloadStep>) {
        let mut downloads = self.state.downloads.lock().unwrap();
        downloads.scripts.insert(id.to_string(), steps);
    }

    /// The model `models.recommended` names for a profile (`low`,
    /// `standard`, `high`). With none, the profile is below every local
    /// model and the call answers `insufficient_memory`.
    pub fn recommend(&self, profile: &str, id: Option<&str>) {
        let mut downloads = self.state.downloads.lock().unwrap();
        match id {
            Some(id) => downloads.recommended.insert(profile.into(), id.into()),
            None => downloads.recommended.remove(profile),
        };
    }
}

/// The file size, `None` when the fake does not know it.
fn size_of(state: &State, id: &str) -> Result<Option<u64>, WireError> {
    let models = state.models.lock().unwrap();
    let Some(model) = models.models.iter().find(|m| m.info.id == id) else {
        return Err(WireError::new(
            ErrorCode::UnknownModel,
            format!("no model {id}"),
        ));
    };
    if model.info.remote {
        let message = format!("{id} is a remote model; there is nothing to download");
        return Err(WireError::new(ErrorCode::Unsupported, message));
    }
    Ok(model.size_bytes)
}

fn installed(state: &State, id: &str) -> bool {
    let models = state.models.lock().unwrap();
    models.models.iter().any(|m| m.info.id == id && m.installed)
}

fn progress(model: &str, received: u64, total: Option<u64>) -> DownloadProgress {
    DownloadProgress {
        model: model.to_string(),
        received_bytes: received,
        total_bytes: total,
        outcome: None,
        error: None,
    }
}

/// Play one download to its end, then forget it.
async fn play(state: Arc<State>, model: String, total: Option<u64>, steps: Vec<DownloadStep>) {
    let (events, mut cancel) = {
        let downloads = state.downloads.lock().unwrap();
        let running = &downloads.running[&model];
        (running.events.clone(), running.cancel.subscribe())
    };
    let mut received = 0;
    let end = |received: u64, outcome, error| {
        let mut last = progress(&model, received, total);
        last.outcome = Some(outcome);
        last.error = error;
        if outcome == DownloadOutcome::Installed {
            last.received_bytes = total.unwrap_or(received);
            let mut models = state.models.lock().unwrap();
            if let Some(m) = models.models.iter_mut().find(|m| m.info.id == model) {
                m.installed = true;
            }
        }
        // Forget it before the last event goes out, so a new download
        // starts afresh rather than attaching to one that has ended.
        state.downloads.lock().unwrap().running.remove(&model);
        let _ = events.send(last);
    };
    for step in steps {
        if *cancel.borrow() {
            return end(received, DownloadOutcome::Cancelled, None);
        }
        match step {
            DownloadStep::Progress(bytes) => {
                received = total.map_or(bytes, |total| bytes.min(total));
                if let Some(r) = state.downloads.lock().unwrap().running.get_mut(&model) {
                    r.received = received;
                }
                let _ = events.send(progress(&model, received, total));
                tokio::task::yield_now().await;
            }
            DownloadStep::Hold(gate) => {
                tokio::select! {
                    _ = cancel.wait_for(|stop| *stop) => {
                        return end(received, DownloadOutcome::Cancelled, None);
                    }
                    _ = gate.wait() => {}
                }
            }
            DownloadStep::Fail(error) => {
                return end(received, DownloadOutcome::Failed, Some(error))
            }
        }
    }
    end(received, DownloadOutcome::Installed, None);
}

/// The default script: four quarters, or four 256 MiB steps when the size
/// is unknown.
fn quarters(total: Option<u64>) -> Vec<DownloadStep> {
    let step = total.map_or(256 << 20, |total| total / 4);
    (1..=4).map(|q| DownloadStep::Progress(step * q)).collect()
}

/// Start the model's download unless it runs, and attach to it: where it
/// stands now, then what follows.
fn attach(
    state: &Arc<State>,
    model: &str,
    total: Option<u64>,
) -> broadcast::Receiver<DownloadProgress> {
    let mut downloads = state.downloads.lock().unwrap();
    if let Some(running) = downloads.running.get(model) {
        return running.events.subscribe();
    }
    let steps = downloads
        .scripts
        .get(model)
        .cloned()
        .unwrap_or_else(|| quarters(total));
    let events = broadcast::channel(64).0;
    let receiver = events.subscribe();
    let running = Running {
        received: 0,
        events,
        cancel: watch::channel(false).0,
    };
    downloads.running.insert(model.to_string(), running);
    tokio::spawn(play(Arc::clone(state), model.to_string(), total, steps));
    receiver
}

fn recommended(state: &State, request: RecommendModel) -> Result<Reply, WireError> {
    let (profile, model, total) = {
        let downloads = state.downloads.lock().unwrap();
        let profile = request
            .profile
            .unwrap_or_else(|| downloads.profile.profile.clone());
        let model = downloads.recommended.get(&profile).cloned();
        (profile, model, downloads.profile.total_memory_bytes)
    };
    let Some(model) = model else {
        let reason = "this machine has less memory than any local model needs";
        return Err(WireError::memory(MemoryRefusal {
            required_bytes: MEMORY_FLOOR,
            // The machine's total physical memory: the floor is about the
            // machine, not what is free now.
            available_bytes: total,
            reason: reason.into(),
        }));
    };
    let size_bytes = size_of(state, &model)?;
    Ok(Reply::Recommended(Recommendation {
        model,
        profile,
        size_bytes,
    }))
}

/// Answer `models.download`, `download_cancel` and `recommended`. The
/// token check has already kept apps out.
pub(crate) async fn dispatch(
    state: &Arc<State>,
    conn: &mut Conn,
    id: u64,
    token: Option<Token>,
    call: Call,
) -> Option<ServerMessage> {
    let answer = match call {
        Call::ModelsRecommended(request) => recommended(state, request),
        Call::ModelsDownloadCancel(request) => {
            let downloads = state.downloads.lock().unwrap();
            match downloads.running.get(&request.model) {
                Some(running) => {
                    running.cancel.send_replace(true);
                    Ok(Reply::Cancelled {})
                }
                None => Err(WireError::new(
                    ErrorCode::NotDownloading,
                    format!("no download of {} is running", request.model),
                )),
            }
        }
        Call::ModelsDownload(request) => {
            let model = request.model;
            let total = match size_of(state, &model) {
                Ok(total) => total,
                Err(error) => return Some(failure(id, error)),
            };
            let subscription = conn.open_subscription();
            let reply = Reply::Downloading {
                subscription,
                model: model.clone(),
                total_bytes: total,
            };
            let _ = conn.out.send(result(id, reply)).await;
            let out = conn.out.clone();
            let send = move |data: DownloadProgress| {
                let out = out.clone();
                async move {
                    let event = Event::DownloadProgress(data);
                    let message = ServerMessage::Event {
                        subscription,
                        event,
                    };
                    out.send(message).await.is_ok()
                }
            };
            let task = if installed(state, &model) {
                let mut done = progress(&model, total.unwrap_or(0), total);
                done.outcome = Some(DownloadOutcome::Installed);
                tokio::spawn(async move {
                    send(done).await;
                })
            } else {
                let mut events = attach(state, &model, total);
                let now = state
                    .downloads
                    .lock()
                    .unwrap()
                    .running
                    .get(&model)
                    .map(|r| r.received);
                let state = Arc::clone(state);
                tokio::spawn(async move {
                    if let Some(received) = now {
                        if !send(progress(&model, received, total)).await {
                            return;
                        }
                    }
                    loop {
                        match events.recv().await {
                            Ok(event) => {
                                let last = event.outcome.is_some();
                                if !send(event).await || last {
                                    return;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => {
                                // Ended before this subscriber caught the
                                // last event: say how it stands.
                                let mut last = progress(&model, total.unwrap_or(0), total);
                                last.outcome = Some(if installed(&state, &model) {
                                    DownloadOutcome::Installed
                                } else {
                                    DownloadOutcome::Failed
                                });
                                send(last).await;
                                return;
                            }
                        }
                    }
                })
            };
            conn.track(subscription, task, None, token);
            return None;
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
