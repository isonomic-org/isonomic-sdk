//! The `identity` interface: the person's Isonomic ID, shell only.
//!
//! The engine creates the ID the first time it is needed and keeps it; the
//! window shows it. The recovery kit (`identity.kit_*`) lets the person
//! carry the ID to another Mac: 24 words on paper and a kit document saved
//! as a file. No grants, logs or keys are on the wire otherwise.

use serde::{Deserialize, Serialize};

/// Params of `identity.get`. Empty for now; a struct so fields can come.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetIdentity {}

/// The person's ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// The form a person reads and copies. Opaque: show it as given.
    pub id: String,
    /// This call made it; every later call answers `false`.
    pub created: bool,
}

/// The 24 recovery words, as the wire carries them: a JSON array of
/// strings. `Debug` never prints them, so a call, reply or error that
/// holds them can be logged. Read them with [`reveal`](Self::reveal) only
/// to show the person or to send them back.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecoveryWords(Vec<String>);

impl RecoveryWords {
    pub fn new(words: Vec<String>) -> Self {
        Self(words)
    }

    /// The words themselves.
    pub fn reveal(&self) -> &[String] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for RecoveryWords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RecoveryWords(<{} words redacted>)", self.0.len())
    }
}

/// Params of `identity.kit_export`. Refused (`recovery_refused`) unless the
/// person asked, in the window, for the kit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitExport {
    pub person_asked: bool,
}

/// Answer to `identity.kit_export`: the words to write down and the kit
/// document to save as a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryKit {
    /// The person's ID, as `identity.get` gives it.
    pub id: String,
    /// The 24 words. Never in `kit`.
    pub words: RecoveryWords,
    /// The kit document: UTF-8 JSON text with the ID, the key log and a
    /// fingerprint of the recovery key. Opaque: save it as given.
    pub kit: String,
}

/// Params of `identity.kit_restore`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitRestore {
    /// The saved kit document, as `kit_export` gave it.
    pub kit: String,
    /// The words the person wrote down.
    pub words: RecoveryWords,
    /// Replace an ID this Mac made but has not used (no app data yet).
    pub replace_unused: bool,
}

/// Answer to `identity.kit_restore`. A restore spends the old kit: these
/// new words and document replace it, and the window asks the person to
/// save them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitRestored {
    pub id: String,
    pub words: RecoveryWords,
    pub kit: String,
    /// When the restored kit was made (Unix seconds). Later changes made
    /// on another Mac are set aside.
    pub kit_made: u64,
    /// The engine's plain words about what the restore did: that the old
    /// kit no longer works, and anything set aside. Show it as written.
    pub note: String,
}

/// Params of `identity.kit_saved`: the person confirmed they saved the kit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitSaved {}

/// Params of `identity.kit_status`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetKitStatus {}

/// Answer to `identity.kit_status`, in Unix seconds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitStatus {
    /// When the person last confirmed saving the current kit.
    pub saved_at: Option<u64>,
    /// When the current kit's words last left the engine.
    pub exported_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_words() {
        let words = RecoveryWords::new(vec!["abandon".into(); 24]);
        let restore = KitRestore {
            kit: "{}".into(),
            words: words.clone(),
            replace_unused: false,
        };
        for text in [format!("{words:?}"), format!("{restore:?}")] {
            assert!(!text.contains("abandon"), "{text}");
            assert!(text.contains("<24 words redacted>"), "{text}");
        }
        assert_eq!(words.reveal().len(), 24);
    }
}
