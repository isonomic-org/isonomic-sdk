// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The recovery kit of the in-memory engine: deterministic words and a kit
//! document of its own, so a kit exported from one test engine restores on
//! another.
//!
//! The words come only from the word-list standard's public 256-bit test
//! vectors, one per kit generation, so no test prints words that look like
//! a person's. The kit document is this engine's own small JSON; the real
//! engine's differs, and the window treats both as opaque text.

use isonomic_proto::identity::{KitRestore, KitRestored, KitStatus, RecoveryKit, RecoveryWords};
use isonomic_proto::{ErrorCode, Reply, WireError};
use serde::{Deserialize, Serialize};

use crate::identity::IdentityState;

/// The test engine's clock for kits, in Unix seconds, unless a test sets
/// another.
pub const DEFAULT_NOW: u64 = 1_794_850_000;

/// The public test vectors (entropy 00…, 7f…, 80…, ff…), in that order.
const VECTORS: [&str; 4] = [
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
    "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title",
    "letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless",
    "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote",
];

/// The words of the test engine's kit `generation`: a public test vector,
/// repeating every four generations.
pub fn kit_words(generation: u32) -> RecoveryWords {
    let vector = VECTORS[generation as usize % VECTORS.len()];
    RecoveryWords::new(vector.split(' ').map(String::from).collect())
}

const FORMAT: &str = "isonomic-test-kit";

/// What the test engine says after a restore.
pub const NOTE: &str =
    "Your old recovery kit no longer works. Save these new words and the new kit file now.";

/// What the document says, before its check.
#[derive(Serialize, Deserialize)]
struct Body {
    format: String,
    version: u32,
    id: String,
    generation: u32,
    made: u64,
}

/// The document as saved: the body, and `check`, the SHA-256 (hex) of the
/// body's JSON. A document whose ID or any other field was changed no
/// longer matches its check and is refused as damaged, as the engine's
/// real kit is.
#[derive(Serialize, Deserialize)]
struct Document {
    #[serde(flatten)]
    body: Body,
    check: String,
}

fn check_of(body: &Body) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_vec(body).expect("a kit body serializes");
    Sha256::digest(&json)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The kit side of the person's ID.
pub(crate) struct KitState {
    pub(crate) generation: u32,
    pub(crate) made: Option<u64>,
    pub(crate) saved_at: Option<u64>,
    pub(crate) exported_at: Option<u64>,
    pub(crate) now: u64,
    pub(crate) in_use: bool,
}

impl Default for KitState {
    fn default() -> Self {
        Self {
            generation: 0,
            made: None,
            saved_at: None,
            exported_at: None,
            now: DEFAULT_NOW,
            in_use: false,
        }
    }
}

fn refused(message: &str) -> WireError {
    WireError::new(ErrorCode::RecoveryRefused, message)
}

fn document(id: &str, kit: &KitState) -> String {
    let body = Body {
        format: FORMAT.into(),
        version: 1,
        id: id.into(),
        generation: kit.generation,
        made: kit.made.unwrap_or(kit.now),
    };
    let check = check_of(&body);
    serde_json::to_string(&Document { body, check }).expect("a kit document serializes")
}

pub(crate) fn export(identity: &mut IdentityState, person_asked: bool) -> Result<Reply, WireError> {
    if !person_asked {
        return Err(refused(
            "Isonomic shows your recovery kit only when you ask for it.",
        ));
    }
    identity.made = true;
    let kit = &mut identity.kit;
    kit.made.get_or_insert(kit.now);
    kit.exported_at = Some(kit.now);
    Ok(Reply::RecoveryKit(RecoveryKit {
        id: identity.id.clone(),
        words: kit_words(kit.generation),
        kit: document(&identity.id, kit),
    }))
}

pub(crate) fn restore(
    identity: &mut IdentityState,
    restore: KitRestore,
) -> Result<Reply, WireError> {
    let damaged =
        "This file is not an Isonomic recovery kit, or it is damaged. Nothing was changed.";
    let sent: Document = serde_json::from_str(&restore.kit).map_err(|_| refused(damaged))?;
    if sent.check != check_of(&sent.body) {
        return Err(refused(damaged));
    }
    let saved = sent.body;
    if saved.format != FORMAT || saved.version != 1 {
        return Err(refused(damaged));
    }
    let next = saved
        .generation
        .checked_add(1)
        .ok_or_else(|| refused(damaged))?;
    if isonomic_words::check(restore.words.reveal()).is_err() {
        return Err(refused(
            "These words don't add up. Check each one against your paper and try again. Nothing was changed.",
        ));
    }
    if restore.words != kit_words(saved.generation) {
        return Err(refused(
            "These words are not this kit's. Nothing was changed.",
        ));
    }
    let kit = &identity.kit;
    if identity.made && identity.id == saved.id && kit.generation > saved.generation {
        return Err(refused(
            "This recovery kit was replaced by a newer one. Nothing was changed.",
        ));
    }
    if identity.made && kit.in_use {
        return Err(refused(
            "This computer already has an Isonomic ID with apps and chats. Nothing was changed.",
        ));
    }
    if identity.made && !restore.replace_unused {
        return Err(refused(
            "This computer already has an Isonomic ID. Restoring replaces it only if you choose to. Nothing was changed.",
        ));
    }
    identity.id = saved.id;
    identity.made = true;
    let kit = &mut identity.kit;
    kit.generation = next;
    kit.made = Some(kit.now);
    kit.saved_at = None;
    kit.exported_at = Some(kit.now);
    Ok(Reply::KitRestored(KitRestored {
        id: identity.id.clone(),
        words: kit_words(kit.generation),
        kit: document(&identity.id, kit),
        kit_made: saved.made,
        note: NOTE.into(),
    }))
}

pub(crate) fn saved(identity: &mut IdentityState) -> Result<Reply, WireError> {
    let kit = &mut identity.kit;
    if kit.exported_at.is_none() {
        return Err(refused("There is no recovery kit to mark as saved yet."));
    }
    kit.saved_at = Some(kit.now);
    Ok(Reply::Done {})
}

pub(crate) fn status(identity: &IdentityState) -> Reply {
    Reply::KitStatus(KitStatus {
        saved_at: identity.kit.saved_at,
        exported_at: identity.kit.exported_at,
    })
}
