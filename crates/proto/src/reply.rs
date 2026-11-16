//! What the engine answers (`Reply`) and what it pushes on a subscription
//! (`Event`).

use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reply {
    Status(SystemStatus),
    Subscribed {
        subscription: u64,
        snapshot: SystemStatus,
    },
    Unsubscribed {},
    Logs {
        service: String,
        lines: Vec<String>,
    },
    Profile(HardwareProfile),
    ShuttingDown {},
    /// Answer to `system.retry`: the service is starting again.
    Retrying {},
    /// Answer to `apps.grant`.
    Granted {
        app: String,
        token: Token,
    },
    /// Answer to `models.list`.
    Models(Vec<ModelInfo>),
    /// Answer to `models.info`.
    Model(ModelDetail),
    /// Answer to `models.stream`: chunks follow on `subscription`. `model`
    /// is the model the engine chose.
    Stream {
        subscription: u64,
        model: String,
    },
    /// Answer to `models.cancel`.
    Cancelled {},
    /// Answer to `models.download`: `download_progress` events follow.
    Downloading {
        subscription: u64,
        model: String,
        /// Absent when the engine does not know the size yet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total_bytes: Option<u64>,
    },
    /// Answer to `models.recommended`.
    Recommended(models::Recommendation),
    /// Answer to `models.embed`: one vector per input, in order.
    Embeddings {
        model: String,
        vectors: Vec<Vec<f32>>,
    },
    /// Answer to `blobs.put`, `blobs.create`, `blobs.write` and
    /// `blobs.info`.
    Blob(BlobInfo),
    /// Answer to `blobs.get`: `data` starts at `offset`.
    BlobData {
        info: BlobInfo,
        offset: u64,
        data: Bytes,
    },
    /// Answer to `blobs.delete`.
    Deleted {},
    /// Answer to `events.publish`.
    Published {},
    /// Answer to `events.subscribe`: `app_event`s follow on `subscription`.
    Listening {
        subscription: u64,
    },
    /// Answer to `sql.query`.
    Rows(Rows),
    /// Answer to `sql.execute`.
    Executed {
        rows_affected: u64,
    },
    /// Answer to `sql.batch`: one count per statement, in order.
    ExecutedBatch {
        rows_affected: Vec<u64>,
    },
    /// Answer to `sql.subscribe`: the result now; changes follow.
    LiveQuery {
        subscription: u64,
        rows: Rows,
    },
    /// Answer to `sessions.create` and `sessions.fork`.
    SessionCreated {
        session: agents::SessionId,
    },
    /// Answer to `sessions.list`.
    Sessions {
        items: Vec<agents::SessionSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next: Option<String>,
    },
    SessionSnapshot(Box<agents::SessionSnapshot>),
    /// Answer to a call that only acts.
    Done {},
    /// Answer to `sessions.send`: `queued` when a turn is already running.
    Accepted {
        queued: bool,
    },
    Purged(agents::PurgeReport),
    /// Answer to `sessions.subscribe`: `session_event` and `live_delta`
    /// events follow.
    SessionStream {
        subscription: u64,
    },
    Approvals(Vec<agents::PendingApproval>),
    Presets(Vec<agents::PresetSummary>),
    Preset(Box<agents::PresetDetail>),
    PresetSaved {
        reference: agents::PresetRef,
    },
    Artifact(Box<agents::ArtifactVersion>),
    Artifacts(Vec<agents::ArtifactEvent>),
    /// Answer to `identity.get`.
    Identity(identity::Identity),
    /// Answer to `approvals.user_action`.
    UserAction {
        token: String,
    },
}

impl Reply {
    /// The subscription this reply opened, if it opened one. Events for it
    /// follow on the same connection.
    pub fn opened_subscription(&self) -> Option<u64> {
        match self {
            Reply::Subscribed { subscription, .. }
            | Reply::Stream { subscription, .. }
            | Reply::Downloading { subscription, .. }
            | Reply::Listening { subscription }
            | Reply::LiveQuery { subscription, .. }
            | Reply::SessionStream { subscription } => Some(*subscription),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Event {
    /// One service changed.
    ServiceStatus(ServiceStatus),
    /// Part of a model stream.
    ModelChunk(ModelChunk),
    /// Progress of a model download; the one with `outcome` is the last.
    DownloadProgress(models::DownloadProgress),
    /// A message published to an `events` subscription's scope.
    AppEvent(AppEvent),
    /// A live query's whole new result.
    RowsChanged(Rows),
    /// One durable agent session event, in sequence order.
    SessionEvent(Box<agents::SessionEvent>),
    /// Reply text as it streams; lossy, replaced by the `assistant` event.
    LiveDelta(agents::LiveDelta),
    /// A session stream's last event: the session is gone.
    SessionEnded(agents::SessionEnded),
}

impl Event {
    /// Whether this is the last event of its subscription.
    pub fn is_final(&self) -> bool {
        match self {
            Event::ModelChunk(chunk) => chunk.stop.is_some(),
            Event::DownloadProgress(progress) => progress.outcome.is_some(),
            Event::SessionEnded(_) => true,
            _ => false,
        }
    }
}
