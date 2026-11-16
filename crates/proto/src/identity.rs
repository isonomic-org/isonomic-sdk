//! The `identity` interface: the person's Isonomic ID, shell only.
//!
//! The engine creates the ID the first time it is needed and keeps it; the
//! window shows it. Version 1 carries nothing else about identity: no
//! grants, logs or keys.

use serde::{Deserialize, Serialize};

/// Params of `identity.get`. Empty for now; a struct so fields can come.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetIdentity {}

/// The person's ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// The form a person reads and copies. Opaque: show it as given.
    pub id: String,
    /// This call made it; every later call answers `false`.
    pub created: bool,
}
