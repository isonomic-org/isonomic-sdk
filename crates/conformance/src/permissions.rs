// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The permissions interface: only the window grants and revokes, an app
//! sees only its own permissions, and a request needs the person.
//!
//! One check links a folder. It is an empty folder the check makes under
//! the system's temporary directory and removes again; nothing is read
//! from or written into it. Every check revokes what it granted.
//!
//! No check sends input, so the checks that need a kit tool called
//! (`permissions.revoked`, `kits.remember`, `kits.offered`) are not here
//! yet: see `docs/conformance.md`.

use std::path::PathBuf;

use isonomic_client::{AppClient, Permissions};
use isonomic_proto::manifest::AgentsAccess;
use isonomic_proto::permissions::{
    Asked, FolderAccess, GrantPermission, Grantable, KitState, LinkFolder, Revocable,
};
use isonomic_proto::{Capabilities, ErrorCode};

use crate::{ensure, manifest, ok, refused, Outcome, Target};

const WEB: &str = "web";

/// A test app whose manifest names the files and web kits.
async fn app(target: &Target, name: &str) -> Result<(AppClient, String), String> {
    let capabilities = Capabilities {
        agents: Some(AgentsAccess {
            kits: vec!["files".into(), WEB.into()],
        }),
        ..Default::default()
    };
    let manifest = manifest(name, capabilities, &[]);
    let id = manifest.id.clone();
    Ok((target.app(manifest).await?, id))
}

fn kit(app: &str) -> GrantPermission {
    GrantPermission {
        app: app.into(),
        grant: Grantable::Kit(WEB.into()),
        session: None,
        until: None,
    }
}

fn link(app: &str, folder: &ScratchFolder) -> GrantPermission {
    let path = folder.0.to_string_lossy().into_owned();
    let access = FolderAccess::Read;
    GrantPermission {
        app: app.into(),
        grant: Grantable::Folder(LinkFolder { path, access }),
        session: None,
        until: None,
    }
}

/// An empty folder under the temporary directory, removed on drop.
struct ScratchFolder(PathBuf);

impl ScratchFolder {
    fn new(name: &str) -> Result<Self, String> {
        let unique = format!("isonomic-conformance-{name}-{}", std::process::id());
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).map_err(|e| format!("a scratch folder: {e}"))?;
        Ok(Self(path))
    }
}

impl Drop for ScratchFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.0);
    }
}

async fn web_state(permissions: &Permissions, app: Option<&str>) -> Result<KitState, String> {
    let held = ok(permissions.list(app).await, "permissions.list")?;
    let switch = held.kits.iter().find(|k| k.kit == WEB);
    switch
        .map(|k| k.state)
        .ok_or_else(|| format!("no switch for the {WEB} kit: {held:?}"))
}

/// An app token is refused `permissions.grant` and `permissions.revoke`,
/// whatever its manifest names, and nothing is recorded.
pub async fn shell_only(target: Target) -> Outcome {
    let (app, id) = app(&target, "permissions-shell-only").await?;
    let folder = ScratchFolder::new("shell-only")?;
    let own = app.permissions();
    let denied = ErrorCode::CapabilityDenied;
    refused(own.grant(kit(&id)).await, denied, "grant a kit")?;
    refused(own.grant(link(&id, &folder)).await, denied, "link a folder")?;
    let revoke = own.revoke(&id, Revocable::Kit(WEB.into())).await;
    refused(revoke, denied, "revoke a kit")?;
    let held = ok(own.list(None).await, "permissions.list")?;
    ensure(held.folders.is_empty(), || {
        format!("a folder was linked by an app: {held:?}")
    })?;
    let state = web_state(&target.shell.permissions(), Some(&id)).await?;
    ensure(state == KitState::NotAsked, || {
        format!("the app's own call left the kit {state:?}")
    })
}

/// What the shell grants one app is in that app's list and in no other's:
/// another app's folders and switches look as if they did not exist.
pub async fn scoped(target: Target) -> Outcome {
    let (owner, owner_id) = app(&target, "permissions-owner").await?;
    let (other, other_id) = app(&target, "permissions-other").await?;
    let shell = target.shell.permissions();
    let folder = ScratchFolder::new("scoped")?;
    let granted = ok(shell.grant(link(&owner_id, &folder)).await, "link a folder")?;
    let linked = granted.folders.first().map(|f| f.id.clone());
    let linked = linked.ok_or_else(|| format!("grant answered no folder: {granted:?}"))?;
    let result = async {
        ok(shell.grant(kit(&owner_id)).await, "turn the kit on")?;
        let own = ok(owner.permissions().list(None).await, "the owner's list")?;
        ensure(own.folders.iter().any(|f| f.id == linked), || {
            format!("the owner's list leaves out its folder: {own:?}")
        })?;
        let state = web_state(&owner.permissions(), None).await?;
        ensure(state == KitState::On, || {
            format!("the owner's kit is {state:?}")
        })?;

        let theirs = ok(other.permissions().list(None).await, "the other's list")?;
        ensure(
            theirs.folders.is_empty() && theirs.approvals.is_empty(),
            || format!("another app's permissions are listed: {theirs:?}"),
        )?;
        let state = web_state(&other.permissions(), None).await?;
        ensure(state == KitState::NotAsked, || {
            format!("another app's answer shows as {state:?}")
        })?;
        let named = other.permissions().list(Some(&owner_id)).await;
        refused(named, ErrorCode::CapabilityDenied, "list another app")?;
        // The owner's folder id is nothing under the other app's name.
        let wrong = shell.revoke(&other_id, Revocable::Folder(linked.clone()));
        refused(wrong.await, ErrorCode::NotFound, "revoke across apps")?;
        let own = ok(shell.list(Some(&owner_id)).await, "the shell's list")?;
        ensure(own.folders.iter().any(|f| f.id == linked), || {
            "a revoke under another app's name ended the folder".into()
        })
    }
    .await;
    let unlink = shell.revoke(&owner_id, Revocable::Folder(linked)).await;
    let off = shell.revoke(&owner_id, Revocable::Kit(WEB.into())).await;
    result?;
    ok(unlink, "revoke the folder")?;
    ok(off, "revoke the kit")
}

/// `permissions.request` without the person's user-action token, or with
/// one the engine never minted, is refused, and no answer is recorded.
pub async fn request_needs_person(target: Target) -> Outcome {
    let (app, id) = app(&target, "permissions-request").await?;
    let own = app.permissions();
    let missing = ErrorCode::MissingUserAction;
    let kit = || Asked::Kit(WEB.into());
    refused(own.request(kit(), None).await, missing, "no token")?;
    refused(own.request(Asked::Folder, None).await, missing, "no token")?;
    let forged = own.request(kit(), Some("conformance-never-minted")).await;
    refused(forged, missing, "a token never minted")?;
    let state = web_state(&target.shell.permissions(), Some(&id)).await?;
    ensure(state == KitState::NotAsked, || {
        format!("a refused request left the kit {state:?}")
    })
}
