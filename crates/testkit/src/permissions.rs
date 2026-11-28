// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `permissions` interface of the in-memory engine. Grants live in
//! memory, in the agent host's own `MemoryGrants`, so what the shell grants
//! here is what the host checks when it offers and judges kit tools. A
//! linked folder is also linked, empty, in the host's in-memory file store,
//! which is what the files kit reads and changes.
//!
//! Folders on disk are never opened. A path can be linked only when it lies
//! strictly inside the scratch root ([`FakePermissions::scratch_root`]);
//! anything else, the root included, is `not_linkable`, as the engine
//! answers for the places a person can never link.
//!
//! Not modelled: a grant for one session or with an end time answers
//! `unsupported`, and no remembered approval is listed.

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use harness_toolkit::files::MemoryFs;
use harness_toolkit::{MemoryGrants, Standing};
use isonomic_proto::agents::kits::{Effect, Scope, ScopeKind, KITS};
use isonomic_proto::agents::AppId;
use isonomic_proto::auth::authorize_kit;
use isonomic_proto::permissions::{
    Asked, FolderAccess, GrantPermission, Grantable, KitState, KitSwitch, LinkFolder, LinkedFolder,
    Permissions, Revocable, FILES_KIT,
};
use isonomic_proto::{Call, ErrorCode, Manifest, Reply, WireError};

use crate::State;

/// The kit whose switch stands for two grants: fetch, and act asking first.
const WEB_KIT: &str = "web";

#[derive(Default)]
struct Held {
    folders: Vec<LinkedFolder>,
    kits: BTreeMap<String, KitState>,
}

#[derive(Default)]
pub(crate) struct PermissionsState {
    apps: BTreeMap<String, Held>,
    next_folder: u64,
    next_action: u64,
    /// User-action tokens minted and not yet spent, by app.
    actions: HashSet<(String, String)>,
    /// What the window was asked to show, oldest first.
    requests: Vec<(String, Asked)>,
}

fn error(code: ErrorCode, message: impl Into<String>) -> WireError {
    WireError::new(code, message)
}

fn not_found() -> WireError {
    error(ErrorCode::NotFound, "no such permission for this app")
}

fn folder_scope(folder: &LinkedFolder) -> Scope {
    Scope {
        kind: ScopeKind::Folder,
        id: folder.id.clone(),
        label: folder.name.clone(),
    }
}

/// The grants a linked folder stands for: read without asking, and for
/// read and write a change that asks first.
fn folder_grants(app: &str, folder: &LinkedFolder) -> Vec<Standing> {
    let grant = |effect, ask_first| Standing {
        app: AppId::from(app),
        effect,
        scope: folder_scope(folder),
        ask_first,
    };
    let mut grants = vec![grant(Effect::ReadsPrivateData, false)];
    if folder.access == FolderAccess::ReadWrite {
        grants.push(grant(Effect::ModifiesData, true));
    }
    grants
}

/// The two grants web access stands for: fetch on the web without asking,
/// and act on any site asking first.
fn web_grants(app: &str) -> Vec<Standing> {
    let grant = |effect, kind, label: &str, ask_first| Standing {
        app: AppId::from(app),
        effect,
        scope: Scope {
            kind,
            id: "*".into(),
            label: label.into(),
        },
        ask_first,
    };
    vec![
        grant(
            Effect::IngestsUntrustedContent,
            ScopeKind::Web,
            "The web",
            false,
        ),
        grant(Effect::ActsOutward, ScopeKind::Site, "Any site", true),
    ]
}

fn replace(grants: &MemoryGrants, standing: Vec<Standing>) {
    for grant in &standing {
        grants.revoke(&grant.app, &grant.scope);
    }
    standing.into_iter().for_each(|grant| grants.issue(grant));
}

fn withdraw(grants: &MemoryGrants, standing: Vec<Standing>) {
    for grant in &standing {
        grants.revoke(&grant.app, &grant.scope);
    }
}

/// The scratch root: the only place a folder can be linked from.
fn scratch_root() -> PathBuf {
    std::env::temp_dir()
}

/// Strictly inside the scratch root, by its components alone: nothing is
/// opened or resolved.
fn linkable(path: &str) -> bool {
    let path = Path::new(path);
    let plain = path
        .components()
        .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    let root = scratch_root();
    path.is_absolute() && plain && path.starts_with(&root) && path != root
}

impl PermissionsState {
    fn list(&self, manifest: &Manifest) -> Permissions {
        let held = self.apps.get(&manifest.id);
        let named = manifest.capabilities.agents.as_ref();
        let kits = named.map(|a| a.kits.as_slice()).unwrap_or_default();
        let switch = |kit: &String| KitSwitch {
            kit: kit.clone(),
            state: held
                .and_then(|h| h.kits.get(kit).copied())
                .unwrap_or(KitState::NotAsked),
        };
        Permissions {
            app: AppId::from(manifest.id.as_str()),
            folders: held.map(|h| h.folders.clone()).unwrap_or_default(),
            kits: kits
                .iter()
                .filter(|kit| *kit != FILES_KIT)
                .map(switch)
                .collect(),
            approvals: Vec::new(),
        }
    }

    fn link(
        &mut self,
        app: &str,
        link: LinkFolder,
        (grants, files): (&MemoryGrants, &MemoryFs),
    ) -> Result<(), WireError> {
        if link.access == FolderAccess::Unrecognized {
            return Err(error(ErrorCode::BadRequest, "no such folder access"));
        }
        if !linkable(&link.path) {
            let message = "This folder cannot be linked.";
            return Err(error(ErrorCode::NotLinkable, message));
        }
        let held = self.apps.entry(app.to_string()).or_default();
        let folder = match held.folders.iter_mut().find(|f| f.path == link.path) {
            Some(folder) => {
                folder.access = link.access;
                folder.clone()
            }
            None => {
                self.next_folder += 1;
                let last = Path::new(&link.path).file_name();
                let base = last.unwrap_or_default().to_string_lossy().into_owned();
                // Names are how files are addressed, so each is unique.
                let taken = |name: &str| held.folders.iter().any(|f| f.name == name);
                let name = (1..)
                    .map(|n| match n {
                        1 => base.clone(),
                        n => format!("{base} {n}"),
                    })
                    .find(|name| !taken(name))
                    .unwrap_or_default();
                let folder = LinkedFolder {
                    id: format!("folder-{}", self.next_folder),
                    name,
                    path: link.path,
                    access: link.access,
                    reachable: true,
                };
                held.folders.push(folder.clone());
                folder
            }
        };
        replace(grants, folder_grants(app, &folder));
        files.unlink(app, &folder.id);
        let read_only = folder.access == FolderAccess::Read;
        files.link(app, &folder.id, &folder.name, read_only);
        Ok(())
    }

    fn switch(&mut self, app: &str, kit: &str, on: bool, grants: &MemoryGrants) {
        let state = match on {
            true => KitState::On,
            false => KitState::Off,
        };
        let held = self.apps.entry(app.to_string()).or_default();
        held.kits.insert(kit.to_string(), state);
        match (kit, on) {
            (WEB_KIT, true) => replace(grants, web_grants(app)),
            (WEB_KIT, false) => withdraw(grants, web_grants(app)),
            _ => {}
        }
    }

    fn revoke(
        &mut self,
        manifest: &Manifest,
        permission: Revocable,
        (grants, files): (&MemoryGrants, &MemoryFs),
    ) -> Result<(), WireError> {
        let app = manifest.id.as_str();
        match permission {
            Revocable::Folder(id) => {
                let held = self.apps.get_mut(app).ok_or_else(not_found)?;
                let at = held.folders.iter().position(|f| f.id == id);
                let folder = held.folders.remove(at.ok_or_else(not_found)?);
                withdraw(grants, folder_grants(app, &folder));
                files.unlink(app, &folder.id);
            }
            Revocable::Kit(kit) => {
                if kit == FILES_KIT || authorize_kit(manifest, &kit).is_err() {
                    return Err(not_found());
                }
                self.switch(app, &kit, false, grants);
            }
            Revocable::Approval(_) => return Err(not_found()),
        }
        Ok(())
    }

    /// Give a fresh host every grant the permissions here stand for.
    pub(crate) fn reissue(&self, grants: &MemoryGrants) {
        for (app, held) in &self.apps {
            for folder in &held.folders {
                replace(grants, folder_grants(app, folder));
            }
            if held.kits.get(WEB_KIT) == Some(&KitState::On) {
                replace(grants, web_grants(app));
            }
        }
    }
}

/// The newest manifest the shell approved for `app`.
fn manifest_of(state: &State, app: &str) -> Result<Manifest, WireError> {
    let grants = state.grants.lock().unwrap();
    let found = grants.values().find(|g| g.manifest.id == app);
    found
        .map(|g| g.manifest.clone())
        .ok_or_else(|| error(ErrorCode::NotFound, "no such app"))
}

fn grant(state: &State, request: GrantPermission) -> Result<Reply, WireError> {
    let manifest = manifest_of(state, request.app.as_str())?;
    authorize_kit(&manifest, request.grant.kit())?;
    if request.session.is_some() || request.until.is_some() {
        let message = "the test engine grants neither for one session nor until a time";
        return Err(error(ErrorCode::Unsupported, message));
    }
    let agents = state.agents.lock().unwrap();
    let mut permissions = state.permissions.lock().unwrap();
    let grants = &agents.memory.grants;
    match request.grant {
        Grantable::Folder(link) => permissions.link(&manifest.id, link, (grants, &agents.files))?,
        Grantable::Kit(kit) if kit == FILES_KIT => {
            let message = "the files kit is granted by linking a folder";
            return Err(error(ErrorCode::BadRequest, message));
        }
        Grantable::Kit(kit) if !KITS.contains(&kit.as_str()) => {
            return Err(error(ErrorCode::BadRequest, "no such kit"));
        }
        Grantable::Kit(kit) => permissions.switch(&manifest.id, &kit, true, grants),
    }
    Ok(Reply::Permissions(Box::new(permissions.list(&manifest))))
}

/// Answer one `permissions.*` call. `proto::auth` has already kept apps
/// out of grant and revoke, out of other apps' lists, and away from kits
/// their manifests do not name.
pub(crate) fn answer(state: &State, app: Option<&str>, call: Call) -> Result<Reply, WireError> {
    match call {
        Call::PermissionsGrant(request) => grant(state, request),
        Call::PermissionsList(request) => {
            let named = request.app.as_ref().map(AppId::as_str).or(app);
            let manifest = manifest_of(state, named.unwrap_or_default())?;
            let permissions = state.permissions.lock().unwrap();
            Ok(Reply::Permissions(Box::new(permissions.list(&manifest))))
        }
        Call::PermissionsRevoke(request) => {
            // Another app's permissions, and an app the engine never
            // approved, answer as if they did not exist.
            let manifest = manifest_of(state, request.app.as_str()).map_err(|_| not_found())?;
            let agents = state.agents.lock().unwrap();
            let mut permissions = state.permissions.lock().unwrap();
            let held = (&*agents.memory.grants, &*agents.files);
            permissions.revoke(&manifest, request.permission, held)?;
            Ok(Reply::Done {})
        }
        Call::PermissionsRequest(request) => {
            let app = app.unwrap_or_default().to_string();
            let mut permissions = state.permissions.lock().unwrap();
            let token = request.user_action.unwrap_or_default();
            if !permissions.actions.remove(&(app.clone(), token)) {
                let message = "Asking the person for a permission needs a user action.";
                return Err(error(ErrorCode::MissingUserAction, message));
            }
            permissions.requests.push((app, request.ask));
            Ok(Reply::Done {})
        }
        other => Err(error(
            ErrorCode::UnknownMethod,
            format!("the testkit has no method {}", other.method()),
        )),
    }
}

/// Scripting handle for the fake permissions. Cheap to clone.
#[derive(Clone)]
pub struct FakePermissions {
    pub(crate) state: Arc<State>,
}

impl FakePermissions {
    /// The scratch root: a folder can be linked only from strictly inside
    /// it. The test engine never opens or creates anything there.
    pub fn scratch_root(&self) -> PathBuf {
        scratch_root()
    }

    /// A user-action token for `app`, as the window mints one when the
    /// person acts. Pass it to `permissions.request`; it is spent by the
    /// first request that carries it.
    pub fn user_action(&self, app: &str) -> String {
        let mut permissions = self.state.permissions.lock().unwrap();
        permissions.next_action += 1;
        let token = format!("ua:permissions:{app}:{}", permissions.next_action);
        permissions.actions.insert((app.to_string(), token.clone()));
        token
    }

    /// What the window was asked to show, oldest first: the app and what it
    /// asked for. A refused request is never here.
    pub fn requests(&self) -> Vec<(String, Asked)> {
        self.state.permissions.lock().unwrap().requests.clone()
    }
}
