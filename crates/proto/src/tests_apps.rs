// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Manifests, tokens, capability checks and lenient decoding.

use super::*;
use crate::auth::{authorize, Caller};
use crate::lenient::{decode_client, decode_server, FromClient, FromServer};
use crate::manifest::{AgentsAccess, ManifestError, ModelAccess, Schema};

pub(crate) fn hello() -> Manifest {
    Manifest {
        id: "com.isonomic.hello".into(),
        name: "Hello".into(),
        version: "0.1.0".into(),
        icon: "sparkle".into(),
        capabilities: Capabilities {
            models: Some(ModelAccess {
                any: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        schemas: vec![],
    }
}

#[test]
fn a_manifest_omits_what_it_does_not_ask_for() {
    let json = serde_json::to_string(&hello()).unwrap();
    assert_eq!(
        json,
        r#"{"id":"com.isonomic.hello","name":"Hello","version":"0.1.0","icon":"sparkle","capabilities":{"models":{"any":true}}}"#
    );
    let bare = r#"{"id":"com.isonomic.x","name":"X","version":"1.0.0","icon":"x"}"#;
    let parsed: Manifest = serde_json::from_str(bare).unwrap();
    assert!(parsed.capabilities.is_empty());
    assert_eq!(serde_json::to_string(&parsed).unwrap(), bare);
}

#[test]
fn a_manifest_from_a_newer_build_still_parses() {
    let newer = r#"{"id":"com.isonomic.x","name":"X","version":"1.0.0","icon":"x","capabilities":{"telepathy":{}},"colour":"red"}"#;
    let parsed: Manifest = serde_json::from_str(newer).unwrap();
    assert!(parsed.capabilities.is_empty());
}

#[test]
fn validation_applies_the_engine_rules() {
    assert_eq!(hello().validate(), Ok(()));
    let bad = |edit: fn(&mut Manifest)| {
        let mut manifest = hello();
        edit(&mut manifest);
        manifest.validate().unwrap_err()
    };
    assert!(matches!(
        bad(|m| m.id = "hello".into()),
        ManifestError::BadId(_)
    ));
    assert!(matches!(
        bad(|m| m.id = "com.Isonomic.x".into()),
        ManifestError::BadId(_)
    ));
    assert!(matches!(
        bad(|m| m.id = "com..x".into()),
        ManifestError::BadId(_)
    ));
    assert!(matches!(
        bad(|m| m.name = " ".into()),
        ManifestError::EmptyName(_)
    ));
    assert!(matches!(
        bad(|m| m.version = "1.2".into()),
        ManifestError::BadVersion(_)
    ));
    assert!(matches!(
        bad(|m| m.schemas = vec![Schema {
            name: "My-Schema".into()
        }]),
        ManifestError::BadSchema(_)
    ));
    // The agents capability is open: a manifest may set it.
    let mut agents = hello();
    agents.capabilities.agents = Some(AgentsAccess {});
    assert_eq!(agents.validate(), Ok(()));
}

#[test]
fn model_access_checks_the_id_and_where_it_runs() {
    let some = ModelAccess {
        any: false,
        ids: vec!["small".into()],
        remote: false,
    };
    assert!(some.permits("small", false));
    assert!(!some.permits("large", false));
    assert!(!some.permits("small", true));
    let any_remote = ModelAccess {
        any: true,
        ids: vec![],
        remote: true,
    };
    assert!(any_remote.permits("anything", true));
    assert!(!ModelAccess::default().names("x"));
}

#[test]
fn tokens_never_show_in_debug_output() {
    let token = Token::new("secret-value");
    assert_eq!(format!("{token:?}"), "Token(..)");
    assert_eq!(serde_json::to_string(&token).unwrap(), r#""secret-value""#);
}

#[test]
fn the_shell_may_call_anything_and_apps_never_reach_shell_interfaces() {
    let manifest = hello();
    let app = Caller::App {
        id: &manifest.id,
        capabilities: &manifest.capabilities,
    };
    for call in [
        Call::SystemStatus,
        Call::SystemShutdown,
        Call::AppsGrant {
            manifest: Box::new(hello()),
        },
    ] {
        assert!(authorize(Caller::Shell, &call).is_ok());
        let error = authorize(app, &call).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityDenied);
        assert!(error.message.contains(call.method()), "{}", error.message);
    }
}

#[test]
fn an_unknown_error_code_is_not_fatal() {
    let wire = r#"{"type":"response","id":3,"error":{"code":"quota_exceeded","message":"later"}}"#;
    let message: ServerMessage = serde_json::from_str(wire).unwrap();
    let ServerMessage::Response {
        outcome: Outcome::Error(error),
        ..
    } = message
    else {
        panic!("expected an error response");
    };
    assert_eq!(error.code, ErrorCode::Unrecognized);
}

#[test]
fn the_engine_can_answer_a_method_it_does_not_know() {
    let unknown = br#"{"type":"request","id":9,"method":"tables.query","params":{}}"#;
    match decode_client(unknown) {
        FromClient::Rejected { id, error } => {
            assert_eq!(id, 9);
            assert_eq!(error.code, ErrorCode::UnknownMethod);
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
    let bad_params = br#"{"type":"request","id":4,"method":"system.logs","params":{}}"#;
    match decode_client(bad_params) {
        FromClient::Rejected { id, error } => {
            assert_eq!(id, 4);
            assert_eq!(error.code, ErrorCode::BadRequest);
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
    assert!(matches!(decode_client(b"{nope"), FromClient::Garbage(_)));
    let known = br#"{"type":"request","id":1,"token":"t","method":"system.status"}"#;
    assert!(matches!(
        decode_client(known),
        FromClient::Message(ClientMessage::Request {
            id: 1,
            token: Some(_),
            ..
        })
    ));
}

#[test]
fn the_client_can_skip_what_a_newer_engine_sends() {
    let event = br#"{"type":"event","subscription":2,"event":"weather","data":{"sunny":true}}"#;
    assert!(matches!(decode_server(event), FromServer::Skip));
    let reply = br#"{"type":"response","id":5,"result":{"forecast":{}}}"#;
    assert!(matches!(
        decode_server(reply),
        FromServer::UnreadableResponse { id: 5 }
    ));
    let extra = br#"{"type":"welcome","version":0,"engine":"9","motd":"hi"}"#;
    assert!(matches!(
        decode_server(extra),
        FromServer::Message(ServerMessage::Welcome { .. })
    ));
}

#[test]
fn model_calls_need_the_models_capability_and_a_named_model() {
    use crate::auth::authorize_model;
    let mut bare = hello();
    bare.capabilities.models = None;
    let none = Caller::App {
        id: &bare.id,
        capabilities: &bare.capabilities,
    };
    let stream = |model: Option<&str>| {
        Call::ModelsStream(StreamRequest {
            model: model.map(str::to_string),
            messages: vec![Message::user("hi")],
            ..Default::default()
        })
    };
    for call in [
        Call::ModelsList,
        stream(None),
        Call::ModelsCancel { subscription: 1 },
    ] {
        assert_eq!(
            authorize(none, &call).unwrap_err().code,
            ErrorCode::CapabilityDenied
        );
    }
    let mut picky = hello();
    picky.capabilities.models = Some(ModelAccess {
        ids: vec!["small".into()],
        ..Default::default()
    });
    let picky = Caller::App {
        id: &picky.id,
        capabilities: &picky.capabilities,
    };
    assert!(authorize(picky, &stream(Some("small"))).is_ok());
    assert!(authorize(picky, &stream(None)).is_ok(), "resolved later");
    assert!(authorize(picky, &stream(Some("large"))).is_err());
    assert!(authorize_model(picky, "small", false).is_ok());
    assert!(authorize_model(picky, "small", true).is_err());
    assert!(authorize_model(picky, "large", false).is_err());
    assert!(authorize_model(Caller::Shell, "large", true).is_ok());
}

#[test]
fn only_the_last_chunk_is_final() {
    assert!(!Event::ModelChunk(ModelChunk::text("a")).is_final());
    assert!(Event::ModelChunk(ModelChunk::stop(StopReason::End)).is_final());
    let reply = Reply::Stream {
        subscription: 4,
        model: "m".into(),
    };
    assert_eq!(reply.opened_subscription(), Some(4));
    let newer = r#"{"stop":"paused","text":"x"}"#;
    let chunk: ModelChunk = serde_json::from_str(newer).unwrap();
    assert_eq!(chunk.stop, Some(StopReason::Unrecognized));
}

#[test]
fn a_session_event_from_a_newer_schema_decodes_as_unknown() {
    use crate::agents::schema;
    let line = r#"{"type":"event","subscription":21,"event":"session_event","data":{"session":"s-1","seq":15,"at":1700000000015,"turn":"t-3","type":"reaction","emoji":"thumbs_up","to_seq":13}}"#;
    let message: ServerMessage = serde_json::from_str(line).unwrap();
    let ServerMessage::Event {
        event: Event::SessionEvent(event),
        ..
    } = message
    else {
        panic!("not a session event: {message:?}");
    };
    assert_eq!(event.seq, 15);
    let schema::Event::Unknown(unknown) = &event.event else {
        panic!("decoded as a known event: {:?}", event.event);
    };
    assert_eq!(unknown.kind.0, "reaction");
    assert!(crate::agents::SchemaVersion::CURRENT.readable());
}

#[test]
fn only_the_shell_names_another_apps_agent_sessions() {
    use crate::agents::{ListApprovals, PurgeSessions};
    let mut chat = hello();
    chat.capabilities.agents = Some(AgentsAccess {});
    let app = Caller::App {
        id: &chat.id,
        capabilities: &chat.capabilities,
    };
    let purge = |app: Option<&str>| {
        Call::SessionsPurge(PurgeSessions {
            app: app.map(Into::into),
        })
    };
    let approvals = |app: Option<&str>| {
        Call::ApprovalsList(ListApprovals {
            app: app.map(Into::into),
        })
    };
    for call in [
        purge(None),
        purge(Some("com.isonomic.hello")),
        approvals(None),
    ] {
        assert!(authorize(app, &call).is_ok(), "{call:?}");
    }
    for call in [
        purge(Some("com.isonomic.notes")),
        approvals(Some("com.isonomic.notes")),
    ] {
        let error = authorize(app, &call).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityDenied, "{call:?}");
    }
    assert!(authorize(Caller::Shell, &purge(Some("com.isonomic.notes"))).is_ok());
    assert!(authorize(Caller::Shell, &approvals(None)).is_ok());
    let everything = authorize(Caller::Shell, &purge(None)).unwrap_err();
    assert_eq!(everything.code, ErrorCode::BadRequest);
}

#[test]
fn installing_models_is_the_shells_alone() {
    use crate::models::{DownloadModel, RecommendModel};
    let mut every = hello();
    every.capabilities.models = Some(ModelAccess {
        any: true,
        remote: true,
        ..Default::default()
    });
    let app = Caller::App {
        id: &every.id,
        capabilities: &every.capabilities,
    };
    let model = || DownloadModel {
        model: "chat-medium".into(),
    };
    for call in [
        Call::ModelsDownload(model()),
        Call::ModelsDownloadCancel(model()),
        Call::ModelsRecommended(RecommendModel::default()),
    ] {
        assert!(call.installs_models());
        let error = authorize(app, &call).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityDenied, "{call:?}");
        assert!(authorize(Caller::Shell, &call).is_ok());
    }
    assert!(!Call::ModelsList.installs_models());
}

#[test]
fn only_the_download_event_with_an_outcome_is_final() {
    use crate::models::{DownloadOutcome, DownloadProgress};
    let progress = |outcome| {
        Event::DownloadProgress(DownloadProgress {
            model: "chat-medium".into(),
            received_bytes: 1,
            total_bytes: Some(2),
            outcome,
            error: None,
        })
    };
    assert!(!progress(None).is_final());
    assert!(progress(Some(DownloadOutcome::Installed)).is_final());
    assert!(progress(Some(DownloadOutcome::Cancelled)).is_final());
}

#[test]
fn a_session_on_a_named_model_needs_that_model() {
    use crate::agents::CreateSession;
    let create = |model: Option<&str>| {
        Call::SessionsCreate(CreateSession {
            preset: "chat".into(),
            title: None,
            location_opt_in: false,
            model: model.map(str::to_string),
        })
    };
    let mut chat = hello();
    chat.capabilities.agents = Some(AgentsAccess {});
    chat.capabilities.models = Some(ModelAccess {
        ids: vec!["chat-small".into()],
        ..Default::default()
    });
    let app = Caller::App {
        id: &chat.id,
        capabilities: &chat.capabilities,
    };
    assert!(authorize(app, &create(None)).is_ok());
    assert!(authorize(app, &create(Some("chat-small"))).is_ok());
    let other = authorize(app, &create(Some("chat-large"))).unwrap_err();
    assert_eq!(other.code, ErrorCode::CapabilityDenied);
    chat.capabilities.models = None;
    let bare = Caller::App {
        id: &chat.id,
        capabilities: &chat.capabilities,
    };
    assert!(authorize(bare, &create(None)).is_ok());
    let none = authorize(bare, &create(Some("chat-small"))).unwrap_err();
    assert_eq!(none.code, ErrorCode::CapabilityDenied);
    assert!(authorize(Caller::Shell, &create(Some("chat-large"))).is_ok());
}
