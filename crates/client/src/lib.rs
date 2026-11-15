//! Async client for the Isonomic engine.
//!
//! One connection per process, multiplexed. A background task owns it: it
//! connects, negotiates the version, subscribes to service status, and
//! reconnects with backoff whenever the engine goes away. Callers get two
//! things:
//!
//! - `Client::live()`: a watch on the connection and the latest system
//!   status, kept current from the subscription. Views await changes on it;
//!   nothing polls.
//! - request methods (`status`, `logs`, `profile`, `shutdown`), which fail
//!   fast with `Disconnected` while the engine is unreachable.
//! - `Client::scoped(token)`: an [`AppClient`] that names one app's
//!   capability token on every call. This is the only client an app sees.
//! - subscriptions: a call whose reply opens a subscription hands back a
//!   [`Subscription`] that yields its events. Events are routed before the
//!   reply is delivered, so none can be missed.
//!
//! The background task needs a Tokio runtime. The request methods and the
//! watch do not, so they can be awaited from any executor, including a UI
//! framework's own.

mod actor;
mod agents;
mod blobs;
mod events;
mod models;
mod scoped;
mod sql;

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use isonomic_proto::agents::SchemaVersion;
use isonomic_proto::{
    Call, Event, HardwareProfile, Manifest, Reply, SystemStatus, Token, WireError,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};

/// A connected byte stream of any kind.
pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}
pub type BoxedStream = Box<dyn AsyncStream>;
pub type Connecting = Pin<Box<dyn Future<Output = io::Result<BoxedStream>> + Send>>;

/// How to open a connection. Called again for every reconnect.
pub type Connector = Arc<dyn Fn() -> Connecting + Send + Sync>;

/// Connect to the engine's Unix socket.
pub fn unix_connector(socket: impl Into<PathBuf>) -> Connector {
    let socket = socket.into();
    Arc::new(move || {
        let socket = socket.clone();
        Box::pin(async move {
            let stream = tokio::net::UnixStream::connect(socket).await?;
            Ok(Box::new(stream) as BoxedStream)
        })
    })
}

#[derive(Debug, Clone)]
pub struct Options {
    /// Sent in `hello`, for the engine's logs.
    pub name: String,
    /// First wait before reconnecting; doubles up to `reconnect_max`.
    pub reconnect_min: Duration,
    pub reconnect_max: Duration,
}

impl Options {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            reconnect_min: Duration::from_millis(100),
            reconnect_max: Duration::from_secs(2),
        }
    }
}

/// What a view needs to draw: are we connected, and what is running.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Live {
    pub connected: bool,
    /// Engine version from the last `welcome`.
    pub engine: Option<String>,
    /// `None` while disconnected or before the first snapshot arrives.
    pub system: Option<SystemStatus>,
    /// The agent session event schema the engine writes, from `welcome`.
    /// A newer revision still renders: unknown events decode as unknown.
    pub schema: Option<SchemaVersion>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("not connected to the engine")]
    Disconnected,
    #[error("engine error {:?}: {}", .0.code, .0.message)]
    Engine(WireError),
    #[error("the engine sent a reply of the wrong kind")]
    UnexpectedReply,
}

pub use agents::{Agents, SessionStream, SessionUpdate};
pub use blobs::Blobs;
pub use events::{EventStream, Events};
pub use models::{DownloadStream, ModelStream, Models};
pub use scoped::AppClient;
pub use sql::{LiveQuery, Sql};

pub(crate) struct Request {
    pub(crate) token: Option<Token>,
    pub(crate) call: Call,
    pub(crate) reply: oneshot::Sender<Result<Reply, ClientError>>,
    /// Where to deliver events if the reply opens a subscription.
    pub(crate) events: Option<mpsc::UnboundedSender<Event>>,
}

/// The events of one subscription, in order. Ends (`None`) when the
/// subscription's last event has arrived or the connection drops.
#[derive(Debug)]
pub struct Subscription {
    id: u64,
    events: mpsc::UnboundedReceiver<Event>,
}

impl Subscription {
    /// The engine's id for this subscription.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.events.recv().await
    }
}

/// A handle to the connection. Cheap to clone; the connection closes when
/// the last handle is dropped.
#[derive(Clone)]
pub struct Client {
    requests: mpsc::Sender<Request>,
    live: watch::Receiver<Live>,
}

impl Client {
    /// Start the background task on `runtime` and return at once. The first
    /// connection attempt happens in the background.
    pub fn start(runtime: &tokio::runtime::Handle, connector: Connector, options: Options) -> Self {
        let (requests, inbox) = mpsc::channel(64);
        let (live_tx, live) = watch::channel(Live::default());
        runtime.spawn(actor::run(connector, options, inbox, live_tx));
        Self { requests, live }
    }

    /// The live view. Await `changed()` on it; never poll. Copy what you
    /// need out of `borrow()` or `wait_for()` and drop the guard at once:
    /// while one is held, the client cannot update the view, and calls wait.
    pub fn live(&self) -> watch::Receiver<Live> {
        self.live.clone()
    }

    /// Make one call as the shell and wait for its reply.
    pub async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        self.send(None, call, None).await
    }

    /// Make one call as the shell whose reply opens a subscription.
    pub async fn subscribe(&self, call: Call) -> Result<(Reply, Subscription), ClientError> {
        self.open(None, call).await
    }

    /// The `models` interface, as the shell.
    pub fn models(&self) -> Models {
        Models::new(self.clone(), None)
    }

    /// The `blobs` interface, as the shell.
    pub fn blobs(&self) -> Blobs {
        Blobs::new(self.clone(), None)
    }

    /// The `events` interface, as the shell.
    pub fn events(&self) -> Events {
        Events::new(self.clone(), None)
    }

    /// The `sql` interface, as the shell (which must name each schema).
    pub fn sql(&self) -> Sql {
        Sql::new(self.clone(), None)
    }

    /// The agents interface, as the shell: every app's sessions.
    pub fn agents(&self) -> Agents {
        Agents::new(self.clone(), None)
    }

    /// A client that names `token` on every call: what an app is given.
    pub fn scoped(&self, token: Token) -> AppClient {
        AppClient::new(self.clone(), token)
    }

    /// Shell only: record an app's approved manifest and get its token.
    pub async fn grant(&self, manifest: Manifest) -> Result<Token, ClientError> {
        match self
            .call(Call::AppsGrant {
                manifest: Box::new(manifest),
            })
            .await?
        {
            Reply::Granted { token, .. } => Ok(token),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    pub(crate) async fn send(
        &self,
        token: Option<Token>,
        call: Call,
        events: Option<mpsc::UnboundedSender<Event>>,
    ) -> Result<Reply, ClientError> {
        let (reply, answer) = oneshot::channel();
        let request = Request {
            token,
            call,
            reply,
            events,
        };
        self.requests
            .send(request)
            .await
            .map_err(|_| ClientError::Disconnected)?;
        answer.await.map_err(|_| ClientError::Disconnected)?
    }

    pub(crate) async fn open(
        &self,
        token: Option<Token>,
        call: Call,
    ) -> Result<(Reply, Subscription), ClientError> {
        let (sender, events) = mpsc::unbounded_channel();
        let reply = self.send(token, call, Some(sender)).await?;
        let id = reply
            .opened_subscription()
            .ok_or(ClientError::UnexpectedReply)?;
        Ok((reply, Subscription { id, events }))
    }

    /// Queue a call without waiting, where awaiting is impossible (a
    /// `Drop`). Lost if the queue is full or the client is gone.
    pub(crate) fn send_now(&self, token: Option<Token>, call: Call) {
        let (reply, _) = oneshot::channel();
        let _ = self.requests.try_send(Request {
            token,
            call,
            reply,
            events: None,
        });
    }

    pub async fn status(&self) -> Result<SystemStatus, ClientError> {
        match self.call(Call::SystemStatus).await? {
            Reply::Status(status) => Ok(status),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    pub async fn profile(&self) -> Result<HardwareProfile, ClientError> {
        match self.call(Call::SystemProfile).await? {
            Reply::Profile(profile) => Ok(profile),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// The most recent log lines of one service.
    pub async fn logs(
        &self,
        service: &str,
        limit: Option<u32>,
    ) -> Result<Vec<String>, ClientError> {
        let call = Call::SystemLogs {
            service: service.to_string(),
            limit,
        };
        match self.call(call).await? {
            Reply::Logs { lines, .. } => Ok(lines),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Start a degraded service again. The outcome arrives as
    /// `service_status` events; refused with `not_degraded` otherwise.
    pub async fn retry(&self, service: &str) -> Result<(), ClientError> {
        let call = Call::SystemRetry {
            service: service.to_string(),
        };
        match self.call(call).await? {
            Reply::Retrying {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }

    /// Ask the engine to stop every service in order, then itself.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        match self.call(Call::SystemShutdown).await? {
            Reply::ShuttingDown {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }
}
