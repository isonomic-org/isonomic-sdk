//! Capability checks, shared by the engine and the testkit so both refuse
//! exactly the same calls.
//!
//! A call made without a token comes from the shell (the window itself) on
//! its own connection. A call made with an app's token is checked against
//! the capabilities in that app's approved manifest. Shell-only interfaces
//! (`system`, `apps`) are refused to every app token.

use crate::manifest::{Capabilities, Manifest};
use crate::{agents, Call, ErrorCode, WireError};

/// The namespace a method belongs to: the part of its name before the dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Interface {
    System,
    Apps,
    Models,
    Blobs,
    Events,
    Sql,
    /// Agent sessions: `sessions`, `approvals`, `questions`, `tools`,
    /// `presets` and `artifacts`.
    Agents,
}

impl Interface {
    pub fn name(self) -> &'static str {
        match self {
            Interface::System => "system",
            Interface::Apps => "apps",
            Interface::Models => "models",
            Interface::Blobs => "blobs",
            Interface::Events => "events",
            Interface::Sql => "sql",
            Interface::Agents => "agents",
        }
    }

    /// Only the shell may call it; no app token is ever enough.
    pub fn shell_only(self) -> bool {
        matches!(self, Interface::System | Interface::Apps)
    }
}

/// Who is making a call.
#[derive(Debug, Clone, Copy)]
pub enum Caller<'a> {
    /// The window, on its own connection, with no token.
    Shell,
    /// A first-party app, through the token issued for its manifest.
    App {
        id: &'a str,
        capabilities: &'a Capabilities,
    },
}

fn denied(app: &str, what: impl std::fmt::Display) -> WireError {
    WireError::new(
        ErrorCode::CapabilityDenied,
        format!("app {app} may not {what}"),
    )
}

/// The model a `sessions.create` names, if any.
fn session_model(call: &Call) -> Option<&str> {
    match call {
        Call::SessionsCreate(create) => create.model.as_deref(),
        _ => None,
    }
}

/// May this caller make this call? Checks what the call itself names. A
/// call that leaves a choice to the engine (a default model, say) is
/// checked again by the engine once it has chosen.
pub fn authorize(caller: Caller<'_>, call: &Call) -> Result<(), WireError> {
    let Caller::App { id, capabilities } = caller else {
        return match call {
            Call::SessionsPurge(purge) if purge.app.is_none() => Err(WireError::new(
                ErrorCode::BadRequest,
                "the shell must name the app whose sessions to purge",
            )),
            _ => Ok(()),
        };
    };
    let interface = call.interface();
    if interface.shell_only() {
        return Err(denied(id, format_args!("call {}", call.method())));
    }
    match interface {
        // Installing models is the window's first-run job, not an app's.
        Interface::Models if call.installs_models() => {
            Err(denied(id, format_args!("call {}", call.method())))
        }
        Interface::Models => {
            let Some(models) = &capabilities.models else {
                return Err(denied(id, "use models"));
            };
            let named = match call {
                Call::ModelsInfo { model } => Some(model.as_str()),
                Call::ModelsStream(request) => request.model.as_deref(),
                Call::ModelsEmbed { model, .. } => model.as_deref(),
                _ => None,
            };
            match named {
                Some(model) if !models.names(model) => {
                    Err(denied(id, format_args!("use model {model}")))
                }
                _ => Ok(()),
            }
        }
        Interface::Blobs => {
            let Some(blobs) = &capabilities.blobs else {
                return Err(denied(id, "store blobs"));
            };
            let size = match call {
                Call::BlobsPut { data, .. } => Some(data.0.len() as u64),
                Call::BlobsCreate { size, .. } => Some(*size),
                _ => None,
            };
            match (size, blobs.max_bytes) {
                (Some(size), Some(max)) if size > max => Err(too_large(size, max)),
                _ => Ok(()),
            }
        }
        // An app's own scope needs no capability; another app's is the
        // shell's alone.
        Interface::Events => match call {
            Call::EventsPublish { app: Some(app), .. }
            | Call::EventsSubscribe { app: Some(app), .. }
                if app != id =>
            {
                Err(denied(id, format_args!("reach the events of {app}")))
            }
            _ => Ok(()),
        },
        Interface::Sql if capabilities.sql.is_none() => Err(denied(id, "use sql")),
        Interface::Agents if capabilities.agents.is_none() => Err(denied(id, "use agents")),
        // A session on a model of the app's choosing: the same rule as
        // `models.stream`. Whether it is remote is checked by the engine.
        Interface::Agents if session_model(call).is_some() => {
            let model = session_model(call).unwrap_or_default();
            match &capabilities.models {
                Some(models) if models.names(model) => Ok(()),
                _ => Err(denied(id, format_args!("use model {model}"))),
            }
        }
        // As for events: an app's own scope, or the shell's naming any app.
        // Which sessions, presets and artifacts an app reaches beyond that
        // is the host's decision: another app's ids answer `not_found`.
        Interface::Agents => match call {
            Call::SessionsPurge(agents::PurgeSessions { app: Some(app) })
            | Call::ApprovalsList(agents::ListApprovals { app: Some(app) })
                if app.as_str() != id =>
            {
                Err(denied(
                    id,
                    format_args!("reach the agent sessions of {app}"),
                ))
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

/// Which schema a sql call runs in, and whether this app may use it there:
/// its own schemas for anything, the schemas its manifest lists for reads
/// only. `named` absent means the app's first own schema. The shell
/// (`manifest` absent) may use any schema it names.
pub fn authorize_schema(
    manifest: Option<&Manifest>,
    named: Option<&str>,
    write: bool,
) -> Result<String, WireError> {
    let Some(manifest) = manifest else {
        return named
            .map(str::to_string)
            .ok_or_else(|| WireError::new(ErrorCode::BadRequest, "the shell must name a schema"));
    };
    let own = |s: &str| manifest.schemas.iter().any(|o| o.name == s);
    let schema = match named {
        Some(s) => s.to_string(),
        None => match manifest.schemas.first() {
            Some(first) => first.name.clone(),
            None => return Err(denied(&manifest.id, "use sql without a schema of its own")),
        },
    };
    let listed =
        (manifest.capabilities.sql.as_ref()).is_some_and(|sql| sql.schemas.contains(&schema));
    match (own(&schema), listed, write) {
        (true, _, _) | (false, true, false) => Ok(schema),
        (false, true, true) => Err(denied(&manifest.id, format_args!("write to {schema}"))),
        _ => Err(denied(&manifest.id, format_args!("use schema {schema}"))),
    }
}

/// A blob over a limit: the manifest's, or the engine's own.
pub fn too_large(size: u64, max: u64) -> WireError {
    WireError::new(
        ErrorCode::TooLarge,
        format!("{size} bytes is over the limit of {max}"),
    )
}

/// The check the engine makes once it knows which model a call runs on:
/// the default it chose, and whether that model is remote.
pub fn authorize_model(caller: Caller<'_>, model: &str, remote: bool) -> Result<(), WireError> {
    let Caller::App { id, capabilities } = caller else {
        return Ok(());
    };
    match &capabilities.models {
        Some(access) if access.permits(model, remote) => Ok(()),
        Some(access) if access.names(model) => Err(denied(id, "use remote models")),
        _ => Err(denied(id, format_args!("use model {model}"))),
    }
}
