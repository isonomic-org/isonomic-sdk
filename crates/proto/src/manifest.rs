// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! What an app is and what it may do: the manifest, its capabilities, and
//! the token the engine issues for it.
//!
//! The engine stores the manifest, shows it to the user, and derives the
//! app's capability token from it. Every call an app makes names that token,
//! and the engine checks the call against these capabilities (see
//! [`crate::auth`]).
//!
//! Every field beyond the identity is optional on the wire, so a manifest
//! written for an older engine still parses, and an older engine ignores
//! capabilities it does not know.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A first-party app's identity and needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Reverse-DNS identifier, for example `com.isonomic.chat`. Stable for
    /// the life of the app; settings and data are keyed by it.
    pub id: String,
    /// Name shown on the rail and in the Store.
    pub name: String,
    /// Semantic version, `major.minor.patch`.
    pub version: String,
    /// Name of an icon in the Isonomic UI icon set. The window resolves it.
    pub icon: String,
    #[serde(default, skip_serializing_if = "Capabilities::is_empty")]
    pub capabilities: Capabilities,
    /// The SQL schemas this app creates and owns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<Schema>,
}

/// Everything an app may reach. Absent means denied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Schemas beyond its own that the app may query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql: Option<SqlAccess>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<ModelAccess>,
    /// Linked accounts, by provider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<ConnectionAccess>,
    /// Hosts the app may reach through the egress gateway, once the user
    /// approves them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub egress: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blobs: Option<BlobAccess>,
    /// Agent sessions: the app's own sessions, presets and artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<AgentsAccess>,
}

impl Capabilities {
    pub fn is_empty(&self) -> bool {
        self == &Capabilities::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlAccess {
    /// Other apps' schemas this app may read. Its own schemas are implied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<String>,
}

/// Which models an app may use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAccess {
    /// Any model the user has installed or enabled.
    #[serde(default, skip_serializing_if = "is_false")]
    pub any: bool,
    /// Specific model ids, used when `any` is false.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// Whether the app may use remote providers at all. The user's global
    /// toggle still applies on top.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remote: bool,
}

impl ModelAccess {
    /// May the app use this model? `remote` says where the model runs.
    pub fn permits(&self, model: &str, remote: bool) -> bool {
        (self.any || self.ids.iter().any(|id| id == model)) && (!remote || self.remote)
    }

    /// May the app name this model at all, wherever it runs?
    pub fn names(&self, model: &str) -> bool {
        self.any || self.ids.iter().any(|id| id == model)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionAccess {
    /// Provider identifier, for example `mail` or `calendar`.
    pub provider: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobAccess {
    /// Largest single blob the app may store. `None` is the engine's limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
}

/// Access to agent sessions. Which sessions an app reaches is the host's
/// rule (its own only); fields that narrow it may come later.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsAccess {
    /// The tool kits the app may ever offer its sessions (`files`, `web`
    /// and so on), so the shell sees them when it approves the app. A kit
    /// that is not named here is never granted to the app and so never
    /// offered, whatever a preset says (`docs/interfaces/permissions.md`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kits: Vec<String>,
}

impl AgentsAccess {
    /// Whether the manifest names this kit.
    pub fn names_kit(&self, kit: &str) -> bool {
        self.kits.iter().any(|k| k == kit)
    }
}

/// A SQL schema an app creates. More fields (migrations) come later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// An opaque capability token. The engine issues it from an approved
/// manifest; the client attaches it to every call the app makes. `Debug`
/// never prints it.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(..)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("app id {0:?} is not reverse-DNS (lowercase letters, digits, dots and dashes, at least three parts)")]
    BadId(String),
    #[error("app {0} has an empty name")]
    EmptyName(String),
    #[error("version {0:?} is not major.minor.patch")]
    BadVersion(String),
    #[error("schema name {0:?} must be lowercase letters, digits and underscores")]
    BadSchema(String),
}

impl Manifest {
    /// Check the rules every engine applies before granting a token.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let parts: Vec<&str> = self.id.split('.').collect();
        let part_ok = |p: &&str| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        };
        if parts.len() < 3 || !parts.iter().all(part_ok) {
            return Err(ManifestError::BadId(self.id.clone()));
        }
        if self.name.trim().is_empty() {
            return Err(ManifestError::EmptyName(self.id.clone()));
        }
        let numbers: Vec<&str> = self.version.split('.').collect();
        let number_ok = |n: &&str| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit());
        if numbers.len() != 3 || !numbers.iter().all(number_ok) {
            return Err(ManifestError::BadVersion(self.version.clone()));
        }
        for schema in &self.schemas {
            let ok = !schema.name.is_empty()
                && schema
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !ok {
                return Err(ManifestError::BadSchema(schema.name.clone()));
            }
        }
        Ok(())
    }
}
