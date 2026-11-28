// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The permissions interface and kits under `proto::auth`: grant and revoke
//! are the window's alone, an app reaches only its own permissions and only
//! the kits its manifest names, and a client decodes what a newer engine
//! sends.

use isonomic_proto::agents::kits::{Remember, ScopeKind};
use isonomic_proto::auth::{authorize, authorize_kit, Caller};
use isonomic_proto::lenient::{decode_server, FromServer};
use isonomic_proto::manifest::AgentsAccess;
use isonomic_proto::permissions::{
    Asked, FolderAccess, GrantPermission, Grantable, LinkFolder, ListPermissions,
    RequestPermission, Revocable, RevokePermission,
};
use isonomic_proto::{Call, Capabilities, ErrorCode, Manifest, Outcome, Reply, ServerMessage};

const CHAT: &str = "com.isonomic.chat";

fn manifest(kits: Option<&[&str]>) -> Manifest {
    Manifest {
        id: CHAT.into(),
        name: "Chat".into(),
        version: "0.1.0".into(),
        icon: "chat".into(),
        capabilities: Capabilities {
            agents: kits.map(|kits| AgentsAccess {
                kits: kits.iter().map(|k| k.to_string()).collect(),
            }),
            ..Default::default()
        },
        schemas: vec![],
    }
}

fn as_app(manifest: &Manifest, call: &Call) -> Result<(), ErrorCode> {
    let caller = Caller::App {
        id: &manifest.id,
        capabilities: &manifest.capabilities,
    };
    authorize(caller, call).map_err(|e| e.code)
}

fn grant(what: Grantable) -> Call {
    Call::PermissionsGrant(GrantPermission {
        app: CHAT.into(),
        grant: what,
        session: None,
        until: None,
    })
}

fn revoke(what: Revocable) -> Call {
    Call::PermissionsRevoke(RevokePermission {
        app: CHAT.into(),
        permission: what,
    })
}

fn list(app: Option<&str>) -> Call {
    Call::PermissionsList(ListPermissions {
        app: app.map(Into::into),
    })
}

fn request(ask: Asked) -> Call {
    Call::PermissionsRequest(RequestPermission {
        ask,
        user_action: Some("ua-1".into()),
    })
}

#[test]
fn an_app_token_never_grants_or_revokes() {
    let app = manifest(Some(&["files", "web", "shell"]));
    let folder = Grantable::Folder(LinkFolder {
        path: "/tmp/design".into(),
        access: FolderAccess::ReadWrite,
    });
    for call in [
        grant(folder),
        grant(Grantable::Kit("web".into())),
        revoke(Revocable::Folder("folder-1".into())),
        revoke(Revocable::Kit("web".into())),
        revoke(Revocable::Approval("approval-1".into())),
    ] {
        assert!(call.window_only(), "{}", call.method());
        assert_eq!(as_app(&app, &call), Err(ErrorCode::CapabilityDenied));
        assert!(authorize(Caller::Shell, &call).is_ok());
    }
}

#[test]
fn an_app_reaches_only_its_own_permissions() {
    let app = manifest(Some(&["web"]));
    assert_eq!(as_app(&app, &list(None)), Ok(()));
    assert_eq!(as_app(&app, &list(Some(CHAT))), Ok(()));
    let other = list(Some("com.isonomic.notes"));
    assert_eq!(as_app(&app, &other), Err(ErrorCode::CapabilityDenied));
    // Kits belong to the agents capability.
    let none = manifest(None);
    assert_eq!(as_app(&none, &list(None)), Err(ErrorCode::CapabilityDenied));
    let ask = request(Asked::Kit("web".into()));
    assert_eq!(as_app(&none, &ask), Err(ErrorCode::CapabilityDenied));
}

#[test]
fn an_app_asks_only_for_kits_its_manifest_names() {
    let web = manifest(Some(&["web"]));
    assert_eq!(as_app(&web, &request(Asked::Kit("web".into()))), Ok(()));
    for ask in [Asked::Kit("shell".into()), Asked::Folder] {
        let refused = as_app(&web, &request(ask));
        assert_eq!(refused, Err(ErrorCode::CapabilityDenied));
    }
    let files = manifest(Some(&["files"]));
    assert_eq!(as_app(&files, &request(Asked::Folder)), Ok(()));

    // The check the engine makes before it records anything for an app.
    assert!(authorize_kit(&web, "web").is_ok());
    for (manifest, kit) in [(&web, "files"), (&manifest(Some(&[])), "web")] {
        let error = authorize_kit(manifest, kit).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityDenied);
    }
    let error = authorize_kit(&manifest(None), "web").unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityDenied);
}

#[test]
fn the_shell_names_the_app_and_never_asks() {
    let shell = |call: &Call| authorize(Caller::Shell, call).map_err(|e| e.code);
    assert_eq!(shell(&list(None)), Err(ErrorCode::BadRequest));
    assert_eq!(shell(&list(Some(CHAT))), Ok(()));
    let ask = request(Asked::Kit("web".into()));
    assert_eq!(shell(&ask), Err(ErrorCode::BadRequest));
}

#[test]
fn a_manifest_without_kits_keeps_its_bytes() {
    let old = r#"{"id":"com.isonomic.chat","name":"Chat","version":"0.1.0","icon":"chat","capabilities":{"agents":{}}}"#;
    let manifest: Manifest = serde_json::from_str(old).unwrap();
    assert_eq!(manifest.capabilities.agents, Some(AgentsAccess::default()));
    assert_eq!(serde_json::to_string(&manifest).unwrap(), old);
}

/// A newer engine's remember and scope values reach the client as the
/// schema's `Other`, not as a decoding failure.
#[test]
fn an_approval_with_unknown_remember_and_scope_decodes() {
    let frame = r#"{"type":"response","id":7,"result":{"approvals":[{"session":"s-1","app":"com.isonomic.chat","request":{"approval":"a-6","summary":"x","rule":"grant","scope":{"kind":"galaxy","id":"m31","label":"Andromeda"},"remember":"forever"}}]}}"#;
    let FromServer::Message(ServerMessage::Response { outcome, .. }) =
        decode_server(frame.as_bytes())
    else {
        panic!("the frame did not decode as a response");
    };
    let Outcome::Result(Reply::Approvals(pending)) = outcome else {
        panic!("not an approvals reply: {outcome:?}");
    };
    let request = &pending[0].request;
    assert_eq!(request.remember, Some(Remember::Other("forever".into())));
    let scope = request.scope.as_ref().unwrap();
    assert_eq!(scope.kind, ScopeKind::Other("galaxy".into()));
    // An older `approvals.decide` still decodes, with nothing remembered.
    let decide = r#"{"session":"s-1","approval":"a-1","decision":"allow","user_action":"ua-77"}"#;
    let decide: isonomic_proto::agents::DecideApproval = serde_json::from_str(decide).unwrap();
    assert_eq!(decide.remember, None);
}

/// The kit specs are harness-core's own, re-exported: an app renders a
/// files call or its error from the same table the engine offers from.
#[test]
fn the_files_kit_specs_are_the_engines() {
    use isonomic_proto::agents::kits::{files_errors, kit_spec, FILES_SPECS, KITS};
    use isonomic_proto::agents::SchemaVersion;
    use isonomic_proto::permissions::FILES_KIT;
    assert_eq!(FILES_SPECS.len(), 7);
    for spec in FILES_SPECS {
        assert_eq!(spec.kit, FILES_KIT);
        assert!(KITS.contains(&spec.kit));
        assert_eq!(spec.since_revision, SchemaVersion::CURRENT.revision);
        assert_eq!(spec.tool_spec().scope, Some(ScopeKind::Folder));
        assert_eq!(kit_spec(spec.name).map(|s| s.name), Some(spec.name));
    }
    assert!(kit_spec("show_table").is_none());
    assert_eq!(files_errors::NOT_LINKED, "files.not_linked");
}
