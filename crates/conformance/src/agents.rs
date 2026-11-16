//! The agents interface for an app that holds the capability: presets, the
//! session registry, replay from a sequence number, isolation between apps
//! and purge.
//!
//! No check sends input. Sending runs the engine's model, whose words a
//! check cannot know; scripted turns stay in the testkit's own tests. Every
//! check purges what it created.

use std::time::Duration;

use isonomic_client::{Agents, SessionStream, SessionUpdate};
use isonomic_proto::agents::{CreateSession, ListSessions, SessionEvent};
use isonomic_proto::manifest::AgentsAccess;
use isonomic_proto::{Call, Capabilities, ErrorCode};

use crate::{ensure, manifest, ok, refused, Outcome, Target};

/// The built-in preset every host ships.
const PRESET: &str = "chat";

async fn agents(target: &Target, name: &str) -> Result<Agents, String> {
    let capabilities = Capabilities {
        agents: Some(AgentsAccess {}),
        ..Default::default()
    };
    let app = target.app(manifest(name, capabilities, &[])).await?;
    Ok(app.agents())
}

async fn event(stream: &mut SessionStream) -> Result<SessionEvent, String> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(SessionUpdate::Event(event))) => return Ok(*event),
            Ok(Some(SessionUpdate::Delta(_))) => continue,
            Ok(Some(SessionUpdate::Gap { after, next })) => {
                return Err(format!("the stream skipped from seq {after} to {next}"))
            }
            Ok(None) => return Err("the session stream closed".into()),
            Err(_) => return Err("no session event within 5 s".into()),
        }
    }
}

async fn purge(agents: &Agents) -> Outcome {
    ok(agents.purge(None).await, "purge").map(|_| ())
}

/// The built-in preset is listed and readable; invalid text is refused with
/// its diagnostics.
pub async fn presets(target: Target) -> Outcome {
    let agents = agents(&target, "agents-presets").await?;
    let listed = ok(agents.presets().await, "presets.list")?;
    ensure(
        listed.iter().any(|p| p.reference.id.to_string() == PRESET),
        || format!("presets.list has no {PRESET}: {listed:?}"),
    )?;
    let chat = ok(agents.preset(PRESET).await, "presets.get")?;
    ensure(!chat.text.is_empty(), || "presets.get: empty text".into())?;
    let broken = agents
        .put_preset("id = \"broken\"\nversion = \"one\"")
        .await;
    refused(
        broken,
        ErrorCode::InvalidPreset,
        "presets.put of invalid text",
    )
}

/// Create, list, open, rename, archive and restore, each visible in the next
/// read.
pub async fn registry(target: Target) -> Outcome {
    let agents = agents(&target, "agents-registry").await?;
    let request = CreateSession {
        preset: PRESET.into(),
        title: Some("Conformance".into()),
        location_opt_in: false,
        model: None,
    };
    let session = ok(agents.create_with(request).await, "sessions.create")?;
    let result = async {
        let (listed, _) = ok(agents.list(ListSessions::default()).await, "list")?;
        ensure(listed.iter().any(|s| s.id == session), || {
            "sessions.list leaves out the new session".into()
        })?;
        ok(agents.rename(&session, "Renamed").await, "sessions.rename")?;
        let opened = ok(agents.open(&session).await, "sessions.open")?;
        ensure(opened.summary.title.as_deref() == Some("Renamed"), || {
            format!("title after rename: {:?}", opened.summary.title)
        })?;
        ok(agents.archive(&session, true).await, "sessions.archive")?;
        let archived = ListSessions {
            archived: Some(true),
            ..Default::default()
        };
        let (listed, _) = ok(agents.list(archived).await, "list archived")?;
        ensure(listed.iter().any(|s| s.id == session && s.archived), || {
            "an archived session is not listed as archived".into()
        })?;
        ok(agents.archive(&session, false).await, "restore")?;
        let opened = ok(agents.open(&session).await, "open after restore")?;
        ensure(!opened.summary.archived, || {
            "a restored session is still archived".into()
        })
    }
    .await;
    purge(&agents).await?;
    result
}

/// A new session's log starts with `session.start` at seq 1, and a
/// subscriber resuming from a seq gets the next one first.
pub async fn replay(target: Target) -> Outcome {
    let agents = agents(&target, "agents-replay").await?;
    let session = ok(agents.create(PRESET).await, "sessions.create")?;
    let result = async {
        let mut all = ok(agents.subscribe(&session, 0).await, "subscribe from 0")?;
        let first = event(&mut all).await?;
        ensure(first.seq == 1, || format!("first seq is {}", first.seq))?;
        ensure(first.event.type_name() == "session.start", || {
            format!("first event is {}", first.event.type_name())
        })?;
        let second = event(&mut all).await?;
        let mut resumed = ok(agents.subscribe(&session, 1).await, "subscribe from 1")?;
        let again = event(&mut resumed).await?;
        ensure(again == second, || {
            format!("from 1, got seq {} instead of {}", again.seq, second.seq)
        })
    }
    .await;
    purge(&agents).await?;
    result
}

/// Another app's session answers `not_found` everywhere, as if it did not
/// exist, and is not listed.
pub async fn isolated(target: Target) -> Outcome {
    let owner = agents(&target, "agents-owner").await?;
    let other = agents(&target, "agents-other").await?;
    let session = ok(owner.create(PRESET).await, "sessions.create")?;
    let result = async {
        refused(other.open(&session).await, ErrorCode::NotFound, "open")?;
        refused(
            other.subscribe(&session, 0).await,
            ErrorCode::NotFound,
            "subscribe",
        )?;
        let renamed = other.rename(&session, "Taken").await;
        refused(renamed, ErrorCode::NotFound, "rename")?;
        let sent = other.send_text(&session, "Hijack").await;
        refused(sent, ErrorCode::NotFound, "send")?;
        let owner_id = "com.isonomic.conformance.agents-owner";
        let purged = other.purge(Some(owner_id)).await;
        refused(
            purged,
            ErrorCode::CapabilityDenied,
            "purge naming the owner",
        )?;
        let pending = other.approvals(Some(owner_id)).await;
        refused(
            pending,
            ErrorCode::CapabilityDenied,
            "approvals naming the owner",
        )?;
        ok(owner.open(&session).await, "the owner's session after that")?;
        let (listed, _) = ok(other.list(ListSessions::default()).await, "list")?;
        ensure(listed.iter().all(|s| s.id != session), || {
            "another app's session is listed".into()
        })
    }
    .await;
    purge(&owner).await?;
    result
}

/// Purge deletes the caller's sessions and leaves every other app's.
pub async fn purge_own(target: Target) -> Outcome {
    let mine = agents(&target, "agents-purge").await?;
    let theirs = agents(&target, "agents-kept").await?;
    let gone = ok(mine.create(PRESET).await, "create mine")?;
    let kept = ok(theirs.create(PRESET).await, "create theirs")?;
    let report = ok(mine.purge(None).await, "purge")?;
    let result = async {
        ensure(report.sessions >= 1, || {
            format!("purge reported {report:?}")
        })?;
        refused(
            mine.open(&gone).await,
            ErrorCode::NotFound,
            "open after purge",
        )?;
        ok(
            theirs.open(&kept).await,
            "the other app's session after purge",
        )
        .map(|_| ())
    }
    .await;
    purge(&theirs).await?;
    result
}

/// Unsubscribing from a stream that is not open is refused.
pub async fn unknown_subscription(target: Target) -> Outcome {
    let capabilities = Capabilities {
        agents: Some(AgentsAccess {}),
        ..Default::default()
    };
    let app = target
        .app(manifest("agents-unsubscribe", capabilities, &[]))
        .await?;
    let call = Call::SessionsUnsubscribe {
        subscription: u64::MAX,
    };
    let what = "sessions.unsubscribe";
    refused(app.call(call).await, ErrorCode::UnknownSubscription, what)
}
