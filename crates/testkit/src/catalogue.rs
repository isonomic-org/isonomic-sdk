// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! `models.catalogue` and `models.remove` in the in-memory engine: the
//! window's model setup, derived from the fake models.
//!
//! The catalogue lists every local fake model, installed or not, with the
//! state `models.info` gives it. A model fits unless the memory gate
//! refuses it or its file is larger than the machine's memory.

use isonomic_proto::models::{CatalogueEntry, ModelDetail, ModelState};
use isonomic_proto::{Call, ErrorCode, Reply, WireError};

use crate::downloads::{recommended_id, state_of};
use crate::models::FakeModels;
use crate::State;

impl FakeModels {
    /// The line and publisher `models.catalogue` shows for a model. Unset,
    /// both are absent.
    pub fn describe(&self, id: &str, about: Option<&str>, publisher: Option<&str>) {
        self.with_model(id, |model| {
            model.about = about.map(str::to_string);
            model.publisher = publisher.map(str::to_string);
        });
    }
}

fn catalogue(state: &State) -> Reply {
    let recommended = recommended_id(state);
    let memory = state.downloads.lock().unwrap().profile.total_memory_bytes;
    let models = state.models.lock().unwrap();
    let entries = models
        .models
        .iter()
        .filter(|m| !m.info.remote)
        .map(|m| CatalogueEntry {
            detail: ModelDetail {
                info: crate::models::shown(m, recommended.as_deref()),
                size_bytes: m.size_bytes,
                license: Some("Test License".into()),
                state: state_of(state, m),
            },
            about: m.about.clone(),
            publisher: m.publisher.clone(),
            fits: fits(m.refusal.is_none(), m.size_bytes, memory),
            fit_reason: Some(fit_reason(m.refusal.is_none(), m.size_bytes, memory)),
        })
        .collect();
    Reply::Catalogue(entries)
}

fn fits(admitted: bool, size: Option<u64>, memory: u64) -> bool {
    admitted && size.is_none_or(|size| size <= memory)
}

const GIB: u64 = 1 << 30;

/// A sentence on whether this machine suits the model.
fn fit_reason(admitted: bool, size: Option<u64>, memory: u64) -> String {
    let have = memory.div_ceil(GIB);
    match size {
        _ if !admitted => format!("This computer's {have} GB of memory can't run it."),
        Some(size) if size > memory => format!(
            "It needs about {} GB of memory, and this computer has {have} GB.",
            size.div_ceil(GIB)
        ),
        _ => format!("It fits in this computer's {have} GB of memory."),
    }
}

fn remove(state: &State, id: &str) -> Result<Reply, WireError> {
    let mut models = state.models.lock().unwrap();
    let Some(model) = models.models.iter_mut().find(|m| m.info.id == id) else {
        return Err(WireError::new(
            ErrorCode::UnknownModel,
            format!("no model {id}"),
        ));
    };
    if model.info.remote {
        let message = format!("{id} is a remote model; there is no file to remove");
        return Err(WireError::new(ErrorCode::Unsupported, message));
    }
    match state_of(state, model) {
        ModelState::Downloading => Err(WireError::new(
            ErrorCode::DownloadRunning,
            format!("{id} is downloading; cancel the download first"),
        )),
        ModelState::NotInstalled => Err(WireError::new(
            ErrorCode::NotInstalled,
            format!("{id} is not installed"),
        )),
        _ => {
            model.installed = false;
            Ok(Reply::Done {})
        }
    }
}

/// Answer `models.catalogue` or `models.remove`. The token check has
/// already kept apps out.
pub(crate) fn answer(state: &State, call: Call) -> Result<Reply, WireError> {
    match call {
        Call::ModelsCatalogue(_) => Ok(catalogue(state)),
        Call::ModelsRemove(request) => remove(state, &request.id),
        other => Err(WireError::new(
            ErrorCode::UnknownMethod,
            format!("the testkit has no method {}", other.method()),
        )),
    }
}
