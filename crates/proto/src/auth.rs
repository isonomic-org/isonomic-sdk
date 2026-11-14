//! Capability checks, shared by the engine and the testkit so both refuse
//! exactly the same calls.
//!
//! A call made without a token comes from the shell (the window itself) on
//! its own connection. A call made with an app's token is checked against
//! the capabilities in that app's approved manifest. Shell-only interfaces
//! (`system`, `apps`) are refused to every app token.

use crate::manifest::Capabilities;
use crate::{Call, ErrorCode, WireError};

/// The namespace a method belongs to: the part of its name before the dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Interface {
    System,
    Apps,
    Models,
}

impl Interface {
    pub fn name(self) -> &'static str {
        match self {
            Interface::System => "system",
            Interface::Apps => "apps",
            Interface::Models => "models",
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

/// May this caller make this call? Checks what the call itself names. A
/// call that leaves a choice to the engine (a default model, say) is
/// checked again by the engine once it has chosen.
pub fn authorize(caller: Caller<'_>, call: &Call) -> Result<(), WireError> {
    let Caller::App { id, capabilities } = caller else {
        return Ok(());
    };
    let interface = call.interface();
    if interface.shell_only() {
        return Err(denied(id, format_args!("call {}", call.method())));
    }
    match interface {
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
        _ => Ok(()),
    }
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
