// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `permissions` interface: what the person lets an app's agent
//! sessions reach. A linked folder, a kit switch ("may this app use the
//! web") and a remembered approval are all grants the engine keeps and
//! checks; this interface is how the window issues and ends them, and how
//! an app reads its own.
//!
//! `permissions.grant` and `permissions.revoke` are the window's alone: it
//! calls them after the person acts. An app may list its own permissions,
//! and may ask the window to put a question to the person with
//! `permissions.request`, which needs a user-action token, so nothing a
//! model writes can raise a question.
//!
//! Everything a client decodes here is tolerant: a value from a newer
//! engine decodes as `unrecognized`, or as harness-core's `Other`.

use serde::{Deserialize, Serialize};

use crate::agents::kits::{Effect, Remember, Scope};
use crate::agents::{AppId, SessionId};

/// The kit a linked folder belongs to.
pub const FILES_KIT: &str = "files";

/// What an app may do in a linked folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FolderAccess {
    /// Read without asking.
    Read,
    /// Read without asking; a change asks first, until the person
    /// remembers an answer.
    ReadWrite,
    /// An access from a newer engine.
    #[serde(other)]
    Unrecognized,
}

/// Where a kit's first question stands for an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum KitState {
    /// The person was never asked.
    NotAsked,
    /// The person said yes.
    On,
    /// The person said no, or turned it off.
    Off,
    /// A state from a newer engine.
    #[serde(other)]
    Unrecognized,
}

/// One folder the person linked to an app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedFolder {
    /// Minted by the engine; the scope id of the folder's grants.
    pub id: String,
    /// What the person and the model call it: `Design/tokens.css` is the
    /// file `tokens.css` in the folder named `Design`.
    pub name: String,
    /// The path to show the person.
    pub path: String,
    pub access: FolderAccess,
    /// Whether the engine can open it now (a drive may be unplugged).
    pub reachable: bool,
}

/// One kit the app's manifest names, and the person's answer for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitSwitch {
    pub kit: String,
    pub state: KitState,
}

/// An approval the person chose to remember: an effect on a scope, for a
/// session or until revoked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RememberedApproval {
    /// Minted by the engine, for `permissions.revoke`.
    pub id: String,
    pub effect: Effect,
    pub scope: Scope,
    /// `session` or `always`.
    pub remember: Remember,
    /// The session it holds for, with `session`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
}

/// Everything one app holds. Answer to `permissions.list` and
/// `permissions.grant`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    pub app: AppId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<LinkedFolder>,
    /// One switch per kit the manifest names, in its order, apart from
    /// `files`: that kit's permissions are the linked folders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kits: Vec<KitSwitch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<RememberedApproval>,
}

/// A folder to link, as the person chose it in the window's picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkFolder {
    pub path: String,
    pub access: FolderAccess,
}

/// What `permissions.grant` grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grantable {
    /// Link a folder to the app.
    Folder(LinkFolder),
    /// Turn a kit on for the app. Not `files`: link a folder.
    Kit(String),
}

impl Grantable {
    /// The kit the manifest must name for this grant.
    pub fn kit(&self) -> &str {
        match self {
            Grantable::Folder(_) => FILES_KIT,
            Grantable::Kit(kit) => kit,
        }
    }
}

/// Params of `permissions.grant`, shell only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantPermission {
    pub app: AppId,
    pub grant: Grantable,
    /// Hold it for this session only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// End it at this time, in milliseconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,
}

/// Params of `permissions.list`. An app's `{}` means its own; the shell
/// must name the app.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListPermissions {
    /// Whose permissions. Only the shell names another app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<AppId>,
}

/// What `permissions.revoke` ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Revocable {
    /// A linked folder, by its id.
    Folder(String),
    /// A kit, by name: its switch reads `off` from then on. This is also
    /// how the window records a no to a kit's first question.
    Kit(String),
    /// A remembered approval, by its id.
    Approval(String),
}

/// Params of `permissions.revoke`, shell only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokePermission {
    pub app: AppId,
    pub permission: Revocable,
}

/// What an app asks the window to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asked {
    /// The folder picker.
    Folder,
    /// A kit's first question.
    Kit(String),
}

impl Asked {
    /// The kit the app's manifest must name to ask this.
    pub fn kit(&self) -> &str {
        match self {
            Asked::Folder => FILES_KIT,
            Asked::Kit(kit) => kit,
        }
    }
}

/// Params of `permissions.request`. Needs a user-action token; the window
/// decides what to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPermission {
    pub ask: Asked,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_action: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::kits::ScopeKind;

    #[test]
    fn values_from_a_newer_engine_still_decode() {
        let json = r#"{"app":"com.isonomic.chat",
            "folders":[{"id":"folder-1","name":"Design","path":"~/Design",
                "access":"read_append","reachable":true,"bookmark":"x"}],
            "kits":[{"kit":"telepathy","state":"snoozed"}],
            "approvals":[{"id":"approval-1","effect":"rewrites_history",
                "scope":{"kind":"galaxy","id":"m31","label":"Andromeda"},
                "remember":"forever"}],
            "later":true}"#;
        let got: Permissions = serde_json::from_str(json).unwrap();
        assert_eq!(got.folders[0].access, FolderAccess::Unrecognized);
        assert_eq!(got.kits[0].state, KitState::Unrecognized);
        let approval = &got.approvals[0];
        assert_eq!(approval.effect, Effect::Other("rewrites_history".into()));
        assert_eq!(approval.scope.kind, ScopeKind::Other("galaxy".into()));
        assert_eq!(approval.remember, Remember::Other("forever".into()));
    }

    #[test]
    fn a_folder_is_the_files_kit() {
        let link = Grantable::Folder(LinkFolder {
            path: "/tmp/x".into(),
            access: FolderAccess::Read,
        });
        assert_eq!(link.kit(), "files");
        assert_eq!(Grantable::Kit("web".into()).kit(), "web");
        assert_eq!(Asked::Folder.kit(), "files");
        assert_eq!(Asked::Kit("web".into()).kit(), "web");
    }
}
