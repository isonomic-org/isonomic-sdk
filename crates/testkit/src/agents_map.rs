//! Between the SDK's agents types and harness-host's, both ways, and the
//! host's errors as wire errors. The event schema is shared and needs none.

use harness_host as host;
use isonomic_proto::agents as sdk;
use isonomic_proto::blobs::CHUNK_BYTES;
use isonomic_proto::{Bytes, ErrorCode, WireError};

pub(crate) fn error(e: host::HostError) -> WireError {
    use host::HostError as E;
    let (code, message) = match e {
        E::NotFound => (ErrorCode::NotFound, "no such thing for this caller".into()),
        E::Unauthorized => (ErrorCode::CapabilityDenied, "not allowed".into()),
        E::ClassNotSupported(class) => (
            ErrorCode::ClassNotSupported,
            format!("{class:?} sessions are not run yet"),
        ),
        E::InvalidPreset(found) => {
            let found: Vec<String> = found
                .iter()
                .map(|d| format!("{}: {}", d.path, d.message))
                .collect();
            (ErrorCode::InvalidPreset, found.join("; "))
        }
        E::NotAtTurnBoundary => (ErrorCode::NotAtTurnBoundary, "not a turn boundary".into()),
        E::ContinueRefused(why) => (ErrorCode::ContinueRefused, why),
        E::NotAllowedToDecide => (
            ErrorCode::NotAllowedToDecide,
            "this caller may not decide approvals".into(),
        ),
        E::MissingUserAction => (
            ErrorCode::MissingUserAction,
            "a decision needs the person's user-action token".into(),
        ),
        E::AlreadyDecided => (ErrorCode::AlreadyDecided, "already decided".into()),
        E::SessionFailed => (ErrorCode::SessionFailed, "the session failed".into()),
        E::InvalidTools(why) => (ErrorCode::InvalidTools, why),
        E::Store(e) => (ErrorCode::Internal, format!("store: {e:?}")),
        E::Model(e) => (ErrorCode::ModelUnavailable, e.message),
    };
    WireError::new(code, message)
}

pub(crate) fn create(r: sdk::CreateSession) -> (sdk::PresetId, host::CreateOptions) {
    let options = host::CreateOptions {
        title: r.title,
        location_opt_in: r.location_opt_in,
        limits: None,
        model: r.model,
    };
    (r.preset, options)
}

pub(crate) fn filter(f: sdk::ListSessions) -> host::SessionFilter {
    host::SessionFilter {
        archived: f.archived,
        preset: f.preset,
        text: f.text,
        cursor: f.cursor,
        limit: f.limit,
    }
}

pub(crate) fn summary(s: host::SessionSummary) -> sdk::SessionSummary {
    sdk::SessionSummary {
        id: s.id,
        app: s.app,
        class: s.class,
        preset: s.preset,
        title: s.title,
        status: s.status,
        archived: s.archived,
        forked_from: s
            .forked_from
            .map(|(session, seq)| sdk::ForkedFrom { session, seq }),
        model: s.model,
        created_at: s.created_at,
        updated_at: s.updated_at,
        last_seq: s.last_seq,
    }
}

pub(crate) fn snapshot(s: host::SessionSnapshot) -> sdk::SessionSnapshot {
    sdk::SessionSnapshot {
        summary: summary(s.summary),
        turn_open: s.turn_open,
        approvals: s.approvals,
        questions: s.questions,
        awaiting_client: s.awaiting_client,
    }
}

pub(crate) fn fork_point(at: sdk::ForkPoint) -> host::ForkPoint {
    match at {
        sdk::ForkPoint::BeforeTurn(turn) => host::ForkPoint::BeforeTurn(turn),
        sdk::ForkPoint::AfterTurn(turn) => host::ForkPoint::AfterTurn(turn),
    }
}

pub(crate) fn input(i: sdk::SendInput) -> host::Input {
    host::Input {
        content: i.content,
        client_ref: i.client_ref,
    }
}

pub(crate) fn decision(d: sdk::DecisionInput) -> host::DecisionInput {
    match d {
        sdk::DecisionInput::Allow => host::DecisionInput::Allow,
        sdk::DecisionInput::Deny => host::DecisionInput::Deny,
        sdk::DecisionInput::Text(text) => host::DecisionInput::Text(text),
    }
}

pub(crate) fn answer(a: sdk::AnswerQuestion) -> host::AnswerInput {
    host::AnswerInput {
        selected: a.selected,
        text: a.text,
        skipped: a.skipped,
    }
}

pub(crate) fn outcome(r: sdk::ClientToolResult) -> host::ClientOutcome {
    match r.error {
        Some(error) => host::ClientOutcome::Error(error, r.content),
        None => host::ClientOutcome::Ok(r.content),
    }
}

pub(crate) fn approval(a: host::PendingApproval) -> sdk::PendingApproval {
    sdk::PendingApproval {
        session: a.session,
        app: a.owner,
        request: a.request,
    }
}

/// harness-preset's diagnostic, through the fields the host exposes.
fn diagnostics<D: std::fmt::Debug>(
    found: &[D],
    path: impl Fn(&D) -> (&str, &str, String),
) -> Vec<sdk::Diagnostic> {
    found
        .iter()
        .map(|d| {
            let (at, message, severity) = path(d);
            sdk::Diagnostic {
                severity,
                at: at.to_string(),
                message: message.to_string(),
            }
        })
        .collect()
}

macro_rules! preset_diagnostics {
    ($found:expr) => {
        diagnostics(&$found, |d| {
            (
                d.path.as_str(),
                d.message.as_str(),
                format!("{:?}", d.severity).to_lowercase(),
            )
        })
    };
}

pub(crate) fn preset_summary(p: host::PresetSummary) -> sdk::PresetSummary {
    sdk::PresetSummary {
        diagnostics: preset_diagnostics!(p.diagnostics),
        reference: p.reference,
        owner: p.owner,
        name: p.name,
    }
}

pub(crate) fn preset(p: host::StoredPreset) -> sdk::PresetDetail {
    sdk::PresetDetail {
        diagnostics: preset_diagnostics!(p.diagnostics),
        reference: p.reference,
        owner: p.owner,
        text: p.text,
    }
}

/// One chunk of an artifact's body: at most `CHUNK_BYTES` from `offset`,
/// fewer if `length` asks for fewer.
pub(crate) fn artifact(
    a: host::ArtifactVersion,
    offset: u64,
    length: Option<u64>,
) -> sdk::ArtifactVersion {
    let chunk = length.map_or(CHUNK_BYTES, |l| l.min(CHUNK_BYTES)) as usize;
    let start = (offset as usize).min(a.bytes.len());
    let end = start.saturating_add(chunk).min(a.bytes.len());
    sdk::ArtifactVersion {
        meta: a.meta,
        bytes: Bytes(a.bytes[start..end].to_vec()),
        offset,
    }
}

pub(crate) fn purged(r: host::PurgeReport) -> sdk::PurgeReport {
    sdk::PurgeReport {
        sessions: r.sessions,
        events: r.events,
        blobs: r.blobs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(len: usize) -> host::ArtifactVersion {
        let meta: sdk::ArtifactEvent = serde_json::from_value(serde_json::json!({
            "artifact": "r-1", "version": 1, "op": "create", "title": "Notes",
            "media_type": "text/plain", "blob": "b-1", "bytes": len, "call": "c-1"
        }))
        .unwrap();
        host::ArtifactVersion {
            meta,
            bytes: (0..len).map(|i| (i % 251) as u8).collect(),
        }
    }

    #[test]
    fn an_artifact_body_comes_in_chunks() {
        let len = CHUNK_BYTES as usize + 10;
        let first = artifact(version(len), 0, None);
        assert_eq!(first.bytes.0.len() as u64, CHUNK_BYTES);
        let rest = artifact(version(len), CHUNK_BYTES, None);
        assert_eq!((rest.offset, rest.bytes.0.len()), (CHUNK_BYTES, 10));
        let small = artifact(version(len), 4, Some(3));
        assert_eq!(small.bytes.0, vec![4, 5, 6]);
        let past = artifact(version(len), len as u64 + 5, None);
        assert!(past.bytes.0.is_empty());
    }
}
