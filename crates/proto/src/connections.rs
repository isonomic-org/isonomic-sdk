// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The `connections` interface: tool servers the person connects, shell
//! only.
//!
//! A connection is a remote tool server reached at a URL, or a local one
//! the engine starts as a command. The engine owns them: it mints their
//! ids, keeps their sign-ins, and says when one changes state with a
//! `connection` event on the shell's status subscription
//! (`system.subscribe`). Apps never reach this interface.

use serde::{Deserialize, Serialize};

/// Where a tool server runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Server {
    /// A server reached over the network.
    Remote { url: String },
    /// A server the engine starts on this computer.
    Local {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// A kind from a newer engine.
    #[serde(other)]
    Unrecognized,
}

/// Where a connection stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConnectionState {
    /// The server wants the person to sign in: `connections.sign_in`.
    NeedsSignIn,
    Connecting,
    /// Its tools are available.
    Ready,
    /// It could not connect; the connection's `error` says why.
    Failed,
    /// A state from a newer engine.
    #[serde(other)]
    Unrecognized,
}

/// One tool server the person connected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    /// Minted by the engine; stable for the connection's life.
    pub id: String,
    /// Shown to the person.
    pub name: String,
    /// The window's own catalogue key for it (picks the logo), if it came
    /// from the catalogue. The engine keeps it as given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub server: Server,
    pub state: ConnectionState,
    /// With `failed`: plain words about what went wrong, to show as given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// How many tools it offers once ready.
    #[serde(default)]
    pub tools: u32,
}

/// One tool a connection offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// A bearer token or API key the person pasted for a server that takes
/// one.
///
/// - **`Debug` is redacted:** it prints `[secret]`, so the `Debug` of a
///   call that holds one is safe to log.
/// - **`Serialize` is not:** it is transparent, as the wire needs. Never
///   log a call as JSON.
/// - **`Deserialize` errors are fixed text:** they never quote what was
///   sent.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ConnectionSecret(String);

/// What a malformed secret is answered with. Never the input.
const NOT_A_SECRET: &str = "a connection secret must be a string";

impl ConnectionSecret {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// The secret itself, to hand to the server. Never log it.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ConnectionSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[secret]")
    }
}

impl<'de> Deserialize<'de> for ConnectionSecret {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // `deserialize_any`, so every other input reaches a visitor below
        // that refuses it in fixed text, never serde's default message.
        deserializer.deserialize_any(SecretVisitor).map(Self)
    }
}

struct SecretVisitor;

impl<'de> serde::de::Visitor<'de> for SecretVisitor {
    type Value = String;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(NOT_A_SECRET)
    }

    fn visit_str<E: serde::de::Error>(self, secret: &str) -> Result<String, E> {
        Ok(secret.to_string())
    }

    fn visit_string<E: serde::de::Error>(self, secret: String) -> Result<String, E> {
        Ok(secret)
    }

    fn visit_bytes<E: serde::de::Error>(self, _: &[u8]) -> Result<String, E> {
        Err(E::custom(NOT_A_SECRET))
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<String, E> {
        Err(E::custom(NOT_A_SECRET))
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<String, E> {
        Err(E::custom(NOT_A_SECRET))
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<String, E> {
        Err(E::custom(NOT_A_SECRET))
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, _: A) -> Result<String, A::Error> {
        Err(serde::de::Error::custom(NOT_A_SECRET))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, _: A) -> Result<String, A::Error> {
        Err(serde::de::Error::custom(NOT_A_SECRET))
    }
}

/// Params of `connections.list`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListConnections {}

/// Params of `connections.add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddConnection {
    /// Shown to the person.
    pub name: String,
    /// The window's catalogue key, if it came from the catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub server: Server,
    /// For a server that takes a pasted token or key instead of a sign-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<ConnectionSecret>,
}

/// Params of `connections.sign_in`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignIn {
    pub id: String,
}

/// Answer to `connections.sign_in`: the page the window opens in the
/// person's browser. The outcome arrives as a `connection` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignInStarted {
    pub url: String,
}

/// Params of `connections.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveConnection {
    pub id: String,
}

/// Params of `connections.tools`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListConnectionTools {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-conformance-7f3a";

    #[test]
    fn debug_never_prints_the_secret() {
        let add = AddConnection {
            name: "Files".into(),
            key: None,
            server: Server::Remote {
                url: "https://tools.example/api".into(),
            },
            secret: Some(ConnectionSecret::new(KEY)),
        };
        let text = format!("{add:?}");
        assert!(!text.contains(KEY), "{text}");
        assert!(text.contains("[secret]"), "{text}");
    }

    #[test]
    fn a_malformed_secret_is_never_quoted() {
        for secret in [
            format!("[\"{KEY}\"]"),
            format!("{{\"k\":\"{KEY}\"}}"),
            "73316".to_string(),
        ] {
            let json = format!(
                r#"{{"name":"x","server":{{"kind":"remote","url":"u"}},"secret":{secret}}}"#
            );
            let error = serde_json::from_str::<AddConnection>(&json).unwrap_err();
            let text = error.to_string();
            assert!(!text.contains(KEY) && !text.contains("73316"), "{text}");
        }
    }

    #[test]
    fn an_unknown_server_kind_still_decodes() {
        let server: Server = serde_json::from_str(r#"{"kind":"socket","path":"/x"}"#).unwrap();
        assert_eq!(server, Server::Unrecognized);
        let state: ConnectionState = serde_json::from_str(r#""paused""#).unwrap();
        assert_eq!(state, ConnectionState::Unrecognized);
    }
}
