//! Wire types for the Isonomic engine API, protocol version 0.
//!
//! One Unix socket carries three kinds of message: requests, responses and
//! events. Each message is one frame: a 4-byte big-endian length followed by
//! that many bytes of UTF-8 JSON, so a person can read the traffic and paste
//! it into a bug report. A binary encoding can replace JSON behind these
//! types without changing them.
//!
//! The first message on a connection is `hello`; the engine answers
//! `welcome` with the version both sides will speak.

pub mod frame;
#[cfg(feature = "tokio")]
pub mod io;
pub mod locate;
pub mod system;

use serde::{Deserialize, Serialize};

pub use system::{HardwareProfile, ServiceState, ServiceStatus, SystemStatus};

/// The protocol version this crate describes.
pub const PROTOCOL_VERSION: u32 = 0;

/// Largest frame either side accepts.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

/// What a client sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Must be first. Lists every protocol version the client can speak.
    Hello { versions: Vec<u32>, client: String },
    /// One call. `id` is chosen by the client and echoed in the response.
    Request {
        id: u64,
        #[serde(flatten)]
        call: Call,
    },
}

/// Every method of every interface. Milestone 0 has `system` only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Call {
    /// Status of every service.
    #[serde(rename = "system.status")]
    SystemStatus,
    /// Status now, then an event for every change.
    #[serde(rename = "system.subscribe")]
    SystemSubscribe,
    #[serde(rename = "system.unsubscribe")]
    SystemUnsubscribe { subscription: u64 },
    /// The most recent log lines of one service.
    #[serde(rename = "system.logs")]
    SystemLogs { service: String, limit: Option<u32> },
    /// The hardware profile the engine chose.
    #[serde(rename = "system.profile")]
    SystemProfile,
    /// Stop every service in order, then the engine.
    #[serde(rename = "system.shutdown")]
    SystemShutdown,
}

/// What the engine sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Answer to `hello`.
    Welcome { version: u32, engine: String },
    Response {
        id: u64,
        #[serde(flatten)]
        outcome: Outcome,
    },
    Event {
        subscription: u64,
        #[serde(flatten)]
        event: Event,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Result(Reply),
    Error(WireError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum Event {
    /// One service changed.
    ServiceStatus(ServiceStatus),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No protocol version in common.
    UnsupportedVersion,
    /// A request arrived before `hello`.
    HelloRequired,
    /// The frame was not a valid message.
    BadRequest,
    UnknownService,
    UnknownSubscription,
    Internal,
}

/// Pick the version to speak: the highest one both sides support.
pub fn negotiate(client_versions: &[u32], engine_versions: &[u32]) -> Option<u32> {
    client_versions
        .iter()
        .copied()
        .filter(|v| engine_versions.contains(v))
        .max()
}

#[cfg(test)]
mod tests;
