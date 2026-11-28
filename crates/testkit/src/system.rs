// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `system` interface of the in-memory engine.

use std::sync::atomic::Ordering;

use isonomic_proto::{Call, ErrorCode, Reply, ServiceState, WireError};

use crate::State;

pub(crate) fn answer(state: &State, call: Call) -> Result<Reply, WireError> {
    match call {
        Call::SystemStatus => Ok(Reply::Status(state.system.lock().unwrap().clone())),
        Call::SystemLogs { service, limit } => {
            let known = state
                .system
                .lock()
                .unwrap()
                .services
                .iter()
                .any(|s| s.name == service);
            if !known {
                return Err(WireError::new(
                    ErrorCode::UnknownService,
                    format!("no service named {service}"),
                ));
            }
            let mut lines = state
                .logs
                .lock()
                .unwrap()
                .get(&service)
                .cloned()
                .unwrap_or_default();
            let keep = limit.map(|n| n as usize).unwrap_or(50);
            lines.drain(..lines.len().saturating_sub(keep));
            Ok(Reply::Logs { service, lines })
        }
        Call::SystemProfile => Ok(Reply::Profile(
            state.downloads.lock().unwrap().profile.clone(),
        )),
        Call::SystemRetry { service } => {
            let services = state.system.lock().unwrap().services.clone();
            let Some(current) = services.iter().find(|s| s.name == service) else {
                return Err(WireError::new(
                    ErrorCode::UnknownService,
                    format!("no service named {service}"),
                ));
            };
            if current.state != ServiceState::Degraded {
                return Err(WireError::new(
                    ErrorCode::NotDegraded,
                    format!("{service} is not degraded"),
                ));
            }
            state.retries.lock().unwrap().push(service.clone());
            state.set_state(&service, ServiceState::Starting);
            let outcome = state.retry_outcomes.lock().unwrap().get(&service).copied();
            if let Some(outcome) = outcome {
                state.set_state(&service, outcome);
            }
            Ok(Reply::Retrying {})
        }
        Call::SystemShutdown => {
            state.shutdown_requested.store(true, Ordering::SeqCst);
            Ok(Reply::ShuttingDown {})
        }
        other => Err(WireError::new(
            ErrorCode::UnknownMethod,
            format!("the testkit has no method {}", other.method()),
        )),
    }
}
