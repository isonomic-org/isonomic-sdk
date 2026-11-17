// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Data carried by the `system` interface.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Stopped,
    /// Starting for the first time, or restarting after a crash.
    Starting,
    Healthy,
    /// Restarted too often; the engine stopped trying.
    Degraded,
}

impl ServiceState {
    /// One light for many services: degraded if any is, healthy only if all
    /// are, starting if any is starting, else stopped.
    pub fn aggregate(states: impl IntoIterator<Item = ServiceState>) -> ServiceState {
        let states: Vec<ServiceState> = states.into_iter().collect();
        if states.contains(&ServiceState::Degraded) {
            ServiceState::Degraded
        } else if !states.is_empty() && states.iter().all(|s| *s == ServiceState::Healthy) {
            ServiceState::Healthy
        } else if states.contains(&ServiceState::Starting) {
            ServiceState::Starting
        } else {
            ServiceState::Stopped
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    /// Startup tier, 0 to 3.
    pub tier: u8,
    pub state: ServiceState,
    /// Restarts since the engine started.
    pub restart_count: u32,
    /// Seconds since the Unix epoch, when it was last seen healthy.
    pub last_healthy_unix: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemStatus {
    /// One light for everything: degraded if any service is, healthy only if
    /// all are.
    pub aggregate: ServiceState,
    pub services: Vec<ServiceStatus>,
    /// How long the core services took to come up. `None` until they have.
    pub cold_start_ms: Option<u64>,
}

impl SystemStatus {
    /// Fold one status event into a snapshot: replace the service of that
    /// name (or add it) and recompute the light.
    pub fn apply(&mut self, change: ServiceStatus) {
        match self.services.iter_mut().find(|s| s.name == change.name) {
            Some(existing) => *existing = change,
            None => self.services.push(change),
        }
        self.aggregate = ServiceState::aggregate(self.services.iter().map(|s| s.state));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// `low`, `standard` or `high`.
    pub profile: String,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
}
