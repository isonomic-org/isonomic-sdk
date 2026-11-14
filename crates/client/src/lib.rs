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
//!
//! The background task needs a Tokio runtime. The request methods and the
//! watch do not, so they can be awaited from any executor, including a UI
//! framework's own.

mod actor;

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use isonomic_proto::{Call, HardwareProfile, Reply, SystemStatus, WireError};
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

pub(crate) struct Request {
    pub(crate) call: Call,
    pub(crate) reply: oneshot::Sender<Result<Reply, ClientError>>,
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

    /// The live view. Await `changed()` on it; never poll.
    pub fn live(&self) -> watch::Receiver<Live> {
        self.live.clone()
    }

    /// Make one call and wait for its reply.
    pub async fn call(&self, call: Call) -> Result<Reply, ClientError> {
        let (reply, answer) = oneshot::channel();
        self.requests
            .send(Request { call, reply })
            .await
            .map_err(|_| ClientError::Disconnected)?;
        answer.await.map_err(|_| ClientError::Disconnected)?
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

    /// Ask the engine to stop every service in order, then itself.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        match self.call(Call::SystemShutdown).await? {
            Reply::ShuttingDown {} => Ok(()),
            _ => Err(ClientError::UnexpectedReply),
        }
    }
}
