// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The permissions interface against the test engine: the window grants
//! and revokes, the grants reach the agent host, apps are kept to their own
//! permissions and to the kits their manifests name, and a request needs
//! the person.

use harness_toolkit::Grants;
use isonomic_client::{AppClient, ClientError};
use isonomic_proto::agents::kits::{
    kit_spec, Effect, GrantAnswer, Remember, Scope, ScopeKind, FILES_SPECS,
};
use isonomic_proto::agents::schema::{Concurrency, Event, Profile, ToolKind, Value};
use isonomic_proto::agents::{AppId, DecideApproval, DecisionInput, SessionId, ToolSpec};
use isonomic_proto::manifest::AgentsAccess;
use isonomic_proto::permissions::{
    Asked, FolderAccess, GrantPermission, Grantable, KitState, LinkFolder, Permissions, Revocable,
};
use isonomic_proto::{Capabilities, ErrorCode, Manifest};
use isonomic_testkit::TestEngine;

const CHAT: &str = "com.isonomic.chat";
const NOTES: &str = "com.isonomic.notes";

fn manifest(id: &str, kits: &[&str]) -> Manifest {
    Manifest {
        id: id.into(),
        name: "Kits app".into(),
        version: "0.1.0".into(),
        icon: "chat".into(),
        capabilities: Capabilities {
            agents: Some(AgentsAccess {
                kits: kits.iter().map(|k| k.to_string()).collect(),
            }),
            ..Default::default()
        },
        schemas: vec![],
    }
}

fn app(engine: &TestEngine, id: &str, kits: &[&str]) -> AppClient {
    engine.client().scoped(engine.grant(&manifest(id, kits)))
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

fn path(engine: &TestEngine, name: &str) -> String {
    let root = engine.permissions().scratch_root();
    root.join(name).to_string_lossy().into_owned()
}

fn folder(app: &str, path: String, access: FolderAccess) -> GrantPermission {
    GrantPermission {
        app: app.into(),
        grant: Grantable::Folder(LinkFolder { path, access }),
        session: None,
        until: None,
    }
}

fn kit(app: &str, kit: &str) -> GrantPermission {
    GrantPermission {
        app: app.into(),
        grant: Grantable::Kit(kit.into()),
        session: None,
        until: None,
    }
}

/// The host's own grant check, as it makes it when a kit tool is called.
fn check(engine: &TestEngine, app: &str, effect: Effect, scope: &Scope) -> GrantAnswer {
    let (app, session) = (AppId::from(app), SessionId::from("s-any"));
    engine
        .agents()
        .with(|host| host.grants.check(&app, &session, &effect, scope))
}

fn held(engine: &TestEngine, app: &str) -> usize {
    let (app, session) = (AppId::from(app), SessionId::from("s-any"));
    engine
        .agents()
        .with(|host| host.grants.held(&app, &session).len())
}

fn scope_of(held: &Permissions, name: &str) -> Scope {
    let folder = held.folders.iter().find(|f| f.name == name).unwrap();
    Scope {
        kind: ScopeKind::Folder,
        id: folder.id.clone(),
        label: folder.name.clone(),
    }
}

#[tokio::test]
async fn a_linked_folder_is_a_grant_until_it_is_revoked() {
    let engine = TestEngine::new();
    let chat = app(&engine, CHAT, &["files", "web"]);
    let shell = engine.client().permissions();
    let link = folder(CHAT, path(&engine, "Design"), FolderAccess::ReadWrite);
    let granted = shell.grant(link).await.unwrap();
    assert_eq!(granted.folders.len(), 1);
    let linked = &granted.folders[0];
    assert_eq!((linked.name.as_str(), linked.reachable), ("Design", true));
    assert_eq!(linked.access, FolderAccess::ReadWrite);
    // The app reads the same, and the shell reads it by naming the app.
    assert_eq!(chat.permissions().list(None).await.unwrap(), granted);
    assert_eq!(shell.list(Some(CHAT)).await.unwrap(), granted);

    let design = scope_of(&granted, "Design");
    let read = || check(&engine, CHAT, Effect::ReadsPrivateData, &design);
    let write = || check(&engine, CHAT, Effect::ModifiesData, &design);
    assert_eq!((read(), write()), (GrantAnswer::Allow, GrantAnswer::Ask));
    // Linking the same folder again changes its access, not its id.
    let again = folder(CHAT, path(&engine, "Design"), FolderAccess::Read);
    let narrowed = shell.grant(again).await.unwrap();
    assert_eq!(narrowed.folders[0].id, design.id);
    assert_eq!(read(), GrantAnswer::Allow);
    assert!(matches!(write(), GrantAnswer::Deny(_)));

    // A fresh host (another hardware profile) still holds the grant.
    engine.agents().set_profile(Some(Profile::Intel));
    assert_eq!(read(), GrantAnswer::Allow);

    // After a revoke the next check refuses, and the folder is gone.
    let gone = Revocable::Folder(design.id.clone());
    shell.revoke(CHAT, gone.clone()).await.unwrap();
    assert!(matches!(read(), GrantAnswer::Deny(_)));
    assert!(shell.list(Some(CHAT)).await.unwrap().folders.is_empty());
    assert_eq!(code(shell.revoke(CHAT, gone).await), ErrorCode::NotFound);
}

#[tokio::test]
async fn two_folders_never_share_a_name() {
    let engine = TestEngine::new();
    let _chat = app(&engine, CHAT, &["files"]);
    let shell = engine.client().permissions();
    for at in ["a/Design", "b/Design"] {
        let link = folder(CHAT, path(&engine, at), FolderAccess::Read);
        shell.grant(link).await.unwrap();
    }
    let held = shell.list(Some(CHAT)).await.unwrap();
    let names: Vec<&str> = held.folders.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Design", "Design 2"]);
}

#[tokio::test]
async fn web_access_is_a_switch_over_two_grants() {
    let engine = TestEngine::new();
    let chat = app(&engine, CHAT, &["files", "web", "shell"]);
    let shell = engine.client().permissions();
    let state = |held: &Permissions, kit: &str| {
        let switch = held.kits.iter().find(|k| k.kit == kit);
        switch.map(|k| k.state)
    };
    let before = chat.permissions().list(None).await.unwrap();
    // One switch per kit the manifest names, apart from files.
    assert_eq!(before.kits.len(), 2);
    assert_eq!(state(&before, "web"), Some(KitState::NotAsked));
    assert_eq!(state(&before, "files"), None);

    let on = shell.grant(kit(CHAT, "web")).await.unwrap();
    assert_eq!(state(&on, "web"), Some(KitState::On));
    assert_eq!(state(&on, "shell"), Some(KitState::NotAsked));
    let scope = |kind, id: &str, label: &str| Scope {
        kind,
        id: id.into(),
        label: label.into(),
    };
    let pages = scope(ScopeKind::Web, "pages", "Pages");
    let site = scope(ScopeKind::Site, "example.org", "example.org");
    let fetch = || check(&engine, CHAT, Effect::IngestsUntrustedContent, &pages);
    let act = || check(&engine, CHAT, Effect::ActsOutward, &site);
    assert_eq!((fetch(), act()), (GrantAnswer::Allow, GrantAnswer::Ask));

    // Revoking the kit records a no, and both grants go.
    shell
        .revoke(CHAT, Revocable::Kit("web".into()))
        .await
        .unwrap();
    let off = shell.list(Some(CHAT)).await.unwrap();
    assert_eq!(state(&off, "web"), Some(KitState::Off));
    assert!(matches!(fetch(), GrantAnswer::Deny(_)));
    assert!(matches!(act(), GrantAnswer::Deny(_)));
    // The files kit is a linked folder, never a switch.
    let files = shell.grant(kit(CHAT, "files")).await;
    assert_eq!(code(files), ErrorCode::BadRequest);
}

#[tokio::test]
async fn an_app_token_never_grants_or_revokes() {
    let engine = TestEngine::new();
    let chat = app(&engine, CHAT, &["files", "web"]);
    let own = chat.permissions();
    let link = folder(CHAT, path(&engine, "Design"), FolderAccess::Read);
    assert_eq!(code(own.grant(link).await), ErrorCode::CapabilityDenied);
    let web = own.grant(kit(CHAT, "web")).await;
    assert_eq!(code(web), ErrorCode::CapabilityDenied);
    let revoke = own.revoke(CHAT, Revocable::Kit("web".into())).await;
    assert_eq!(code(revoke), ErrorCode::CapabilityDenied);
    // Nothing was recorded, and the host holds nothing for the app.
    let held = own.list(None).await.unwrap();
    assert!(held.folders.is_empty());
    assert_eq!(held.kits[0].state, KitState::NotAsked);
    assert_eq!(self::held(&engine, CHAT), 0);
}

/// Every tool the model was offered since request `from`, by name, and
/// how many requests the engine has made.
fn offered(engine: &TestEngine, from: usize) -> (Vec<String>, usize) {
    engine.agents().with(|host| {
        let requests = host.model.requests.lock().unwrap();
        let tools = requests[from..].iter().flat_map(|r| r.tools.iter());
        (tools.map(|t| t.name.clone()).collect(), requests.len())
    })
}

/// A kit the manifest does not name is never granted, so the host never
/// holds a grant for it, and its offer rule admits no tool without one:
/// the session's tool list carries no kit tool, whatever the preset says.
#[tokio::test]
async fn a_kit_the_manifest_does_not_name_is_never_offered() {
    let engine = TestEngine::new();
    let notes = app(&engine, NOTES, &["web"]);
    let shell = engine.client().permissions();
    let link = folder(NOTES, path(&engine, "Notes"), FolderAccess::ReadWrite);
    assert_eq!(code(shell.grant(link).await), ErrorCode::CapabilityDenied);
    let shell_kit = shell.grant(kit(NOTES, "shell")).await;
    assert_eq!(code(shell_kit), ErrorCode::CapabilityDenied);
    let person = engine.permissions().user_action(NOTES);
    for ask in [Asked::Folder, Asked::Kit("shell".into())] {
        let asked = notes.permissions().request(ask, Some(&person)).await;
        assert_eq!(code(asked), ErrorCode::CapabilityDenied);
    }
    // Both built-in presets name the files kit; the manifest does not.
    for preset in ["test", "chat"] {
        let session = notes.agents().create(preset).await.unwrap();
        notes.agents().send_text(&session, "Hello.").await.unwrap();
    }
    let (tools, requests) = offered(&engine, 0);
    assert!(requests >= 2 && tools.contains(&"ask_user".to_string()));
    assert_eq!(tools.iter().find(|name| kit_spec(name).is_some()), None);
    assert_eq!(held(&engine, NOTES), 0);
    assert!(engine.permissions().requests().is_empty());
    let listed = notes.permissions().list(None).await.unwrap();
    assert!(listed.folders.is_empty() && listed.kits.len() == 1);
    // An app with no agents capability has no permissions to read.
    let mut plain = manifest("com.isonomic.plain", &[]);
    plain.capabilities.agents = None;
    let plain = engine.client().scoped(engine.grant(&plain));
    let refused = plain.permissions().list(None).await;
    assert_eq!(code(refused), ErrorCode::CapabilityDenied);
}

/// The other side of the same rule: an app whose manifest names the files
/// kit is offered its tools once a folder is linked, and only then; a
/// folder linked for reading offers the read tools alone.
#[tokio::test]
async fn a_named_kit_is_offered_once_a_folder_is_linked() {
    let engine = TestEngine::new();
    let chat = app(&engine, CHAT, &["files"]);
    let shell = engine.client().permissions();
    let names = |core: bool| {
        let specs = FILES_SPECS.iter().filter(move |s| s.core == core);
        specs.map(|s| s.name.to_string()).collect::<Vec<_>>()
    };
    let turn = || {
        let (chat, engine) = (&chat, &engine);
        async move {
            let (_, before) = offered(engine, 0);
            let session = chat.agents().create("test").await.unwrap();
            chat.agents().send_text(&session, "Hello.").await.unwrap();
            let (tools, _) = offered(engine, before);
            assert!(tools.contains(&"ask_user".to_string()));
            let has = |names: Vec<String>| names.iter().all(|name| tools.contains(name));
            let any = tools.iter().any(|name| kit_spec(name).is_some());
            (any, has(names(true)), has(names(false)))
        }
    };
    assert_eq!(turn().await, (false, false, false), "nothing is linked");
    let link = folder(CHAT, path(&engine, "Design"), FolderAccess::Read);
    shell.grant(link).await.unwrap();
    assert_eq!(turn().await, (true, true, false), "linked for reading");
    let link = folder(CHAT, path(&engine, "Design"), FolderAccess::ReadWrite);
    let held = shell.grant(link).await.unwrap();
    assert_eq!(turn().await, (true, true, true), "linked for changes");
    let id = held.folders[0].id.clone();
    shell.revoke(CHAT, Revocable::Folder(id)).await.unwrap();
    assert_eq!(turn().await, (false, false, false), "revoked");
}

#[tokio::test]
async fn another_apps_permissions_look_as_if_they_did_not_exist() {
    let engine = TestEngine::new();
    let _chat = app(&engine, CHAT, &["files", "web"]);
    let notes = app(&engine, NOTES, &["files", "web"]);
    let shell = engine.client().permissions();
    let link = folder(CHAT, path(&engine, "Design"), FolderAccess::ReadWrite);
    let granted = shell.grant(link).await.unwrap();
    shell.grant(kit(CHAT, "web")).await.unwrap();
    let design = scope_of(&granted, "Design");

    let theirs = notes.permissions().list(None).await.unwrap();
    assert!(theirs.folders.is_empty() && theirs.approvals.is_empty());
    assert_eq!(theirs.kits[0].state, KitState::NotAsked);
    let named = notes.permissions().list(Some(CHAT)).await;
    assert_eq!(code(named), ErrorCode::CapabilityDenied);
    // The id of one app's folder is nothing under another app's name.
    let wrong = shell.revoke(NOTES, Revocable::Folder(design.id.clone()));
    assert_eq!(code(wrong.await), ErrorCode::NotFound);
    let read = check(&engine, NOTES, Effect::ReadsPrivateData, &design);
    assert!(matches!(read, GrantAnswer::Deny(_)));
    assert_eq!(shell.list(Some(CHAT)).await.unwrap().folders.len(), 1);
    // An app the engine never approved has nothing either.
    let unknown = "com.isonomic.nobody";
    assert_eq!(code(shell.list(Some(unknown)).await), ErrorCode::NotFound);
    let revoke = shell.revoke(unknown, Revocable::Kit("web".into())).await;
    assert_eq!(code(revoke), ErrorCode::NotFound);
}

#[tokio::test]
async fn only_a_folder_inside_the_scratch_root_is_linkable() {
    let engine = TestEngine::new();
    let _chat = app(&engine, CHAT, &["files"]);
    let shell = engine.client().permissions();
    let root = engine.permissions().scratch_root();
    let root = root.to_string_lossy().into_owned();
    let climbing = format!("{root}/Design/../..");
    for bad in [root.as_str(), "/", "Design", "~/Design", &climbing, ""] {
        let link = folder(CHAT, bad.to_string(), FolderAccess::Read);
        let refused = shell.grant(link).await;
        assert_eq!(code(refused), ErrorCode::NotLinkable, "{bad:?}");
    }
    assert!(shell.list(Some(CHAT)).await.unwrap().folders.is_empty());
    assert_eq!(held(&engine, CHAT), 0);
    // Neither modelled here: a grant for one session, or until a time.
    let mut timed = folder(CHAT, path(&engine, "Design"), FolderAccess::Read);
    timed.until = Some(1_800_000_000_000);
    assert_eq!(code(shell.grant(timed).await), ErrorCode::Unsupported);
}

#[tokio::test]
async fn a_request_needs_the_person_and_reaches_the_window_once() {
    let engine = TestEngine::new();
    let chat = app(&engine, CHAT, &["files", "web"]).permissions();
    let notes = app(&engine, NOTES, &["web"]).permissions();
    let web = || Asked::Kit("web".into());
    let none = chat.request(web(), None).await;
    assert_eq!(code(none), ErrorCode::MissingUserAction);
    let forged = chat.request(Asked::Folder, Some("forged")).await;
    assert_eq!(code(forged), ErrorCode::MissingUserAction);
    // A token minted for one app is nothing in another's hands.
    let person = engine.permissions().user_action(CHAT);
    let stolen = notes.request(web(), Some(&person)).await;
    assert_eq!(code(stolen), ErrorCode::MissingUserAction);
    assert!(engine.permissions().requests().is_empty());

    chat.request(web(), Some(&person)).await.unwrap();
    assert_eq!(engine.permissions().requests(), [(CHAT.to_string(), web())]);
    // Spent by the request that carried it.
    let again = chat.request(Asked::Folder, Some(&person)).await;
    assert_eq!(code(again), ErrorCode::MissingUserAction);
    assert_eq!(engine.permissions().requests().len(), 1);
    // Asking changes nothing by itself: the window answers with a grant.
    let held = chat.list(None).await.unwrap();
    assert_eq!(held.kits[0].state, KitState::NotAsked);
    // The window never asks itself.
    let shell = engine.client().permissions();
    let asked = shell.request(web(), Some(&person)).await;
    assert_eq!(code(asked), ErrorCode::BadRequest);
}

/// `remember` rides on `approvals.decide`. The host holds it to what the
/// request offers (once, for a tool with no scope) and needs the person's
/// token as for any decision.
#[tokio::test]
async fn remember_on_a_decision_needs_the_person_and_never_grows() {
    let engine = TestEngine::new();
    let agents = app(&engine, CHAT, &["files"]).agents();
    let session = agents.create("test").await.unwrap();
    let post = ToolSpec {
        name: "post".into(),
        description: "The post tool.".into(),
        input_schema: Value::object([("type", Value::from("object"))]),
        kind: ToolKind::Client,
        effects: vec![Effect::ActsOutward],
        concurrency: Concurrency::Serial,
        scope: None,
    };
    let specs = vec![post];
    agents.register_client_tools(&session, specs).await.unwrap();
    engine
        .agents()
        .with(|host| host.model.calls(&[("post", "{\"text\":\"hi\"}")]));
    agents.send_text(&session, "Post hi.").await.unwrap();
    let pending = agents.approvals(None).await.unwrap();
    assert_eq!(pending[0].request.remember, None);
    let approval = pending[0].request.approval.clone();
    let decide = |user_action: Option<String>| DecideApproval {
        session: session.clone(),
        approval: approval.clone(),
        decision: DecisionInput::Allow,
        user_action,
        remember: Some(Remember::Always),
    };
    let unsigned = agents.decide_with(decide(None)).await;
    assert_eq!(code(unsigned), ErrorCode::MissingUserAction);
    assert_eq!(held(&engine, CHAT), 0);
    let person = engine.agents().user_action(Some(CHAT), &session, &approval);
    agents.decide_with(decide(Some(person))).await.unwrap();
    let log = engine.agents().log(&session);
    let decided = log.iter().find_map(|e| match &e.event {
        Event::ApprovalDecision(decision) => Some(decision.remember.clone()),
        _ => None,
    });
    assert_eq!(decided, Some(None), "an allow for once records no remember");
    assert_eq!(held(&engine, CHAT), 0);
}
