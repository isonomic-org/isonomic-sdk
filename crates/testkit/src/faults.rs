// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Slow and failing calls, per interface, for any interface.

use std::time::Duration;

use isonomic_proto::auth::Interface;
use isonomic_proto::WireError;

use crate::State;

/// What happens to every call on one interface until it is cleared.
#[derive(Debug, Clone, Default)]
pub(crate) struct Fault {
    pub(crate) delay: Option<Duration>,
    pub(crate) error: Option<WireError>,
}

/// Wait out the interface's delay, then fail with its error, if any.
pub(crate) async fn apply(state: &State, interface: Interface) -> Result<(), WireError> {
    let fault = state.faults.lock().unwrap().get(&interface).cloned();
    let Some(fault) = fault else { return Ok(()) };
    if let Some(delay) = fault.delay {
        tokio::time::sleep(delay).await;
    }
    match fault.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
