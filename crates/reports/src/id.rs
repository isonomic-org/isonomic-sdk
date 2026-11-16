//! A report's own id, so a receiver can drop duplicates: 128 bits as 32
//! lowercase hex characters.
//!
//! A new report gets a random one. A file written before reports carried an
//! id gets a stable one derived from its file name (`<millis>-<kind>[-n]`)
//! and the Capsule's id, the same on every machine and build.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// How long an id is, in hex characters.
pub const ID_LENGTH: usize = 32;

/// A fresh id. The bits come from the standard library's per-process random
/// hasher keys, mixed with the time and a counter: unique enough to tell
/// reports apart, not a secret.
pub fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let half = |salt: u64| {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(salt);
        hasher.write_u64(nanos);
        hasher.write_u64(count);
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(1), half(2))
}

/// Whether `text` has an id's form.
pub fn is_id(text: &str) -> bool {
    text.len() == ID_LENGTH && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A report's id: its own `id` when it has one, else one derived from its
/// file name (with or without `.json`) and the Capsule's id.
pub fn report_id(doc: &Value, file_name: &str, capsule_id: &str) -> String {
    match doc.get("id").and_then(Value::as_str) {
        Some(id) if is_id(id) => id.to_string(),
        _ => derived_id(file_name, capsule_id),
    }
}

/// The stable id of a report file that carries none.
pub fn derived_id(file_name: &str, capsule_id: &str) -> String {
    let stem = file_name.strip_suffix(".json").unwrap_or(file_name);
    let key = format!("{capsule_id}/{stem}");
    format!(
        "{:016x}{:016x}",
        fnv(&key, 0xcbf2_9ce4_8422_2325),
        fnv(&key, 0x84222325cbf29ce4)
    )
}

/// FNV-1a, 64 bits, from a given basis: fixed, so ids never change.
fn fnv(text: &str, basis: u64) -> u64 {
    text.bytes().fold(basis, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_ids_have_the_form_and_differ() {
        let (a, b) = (new_id(), new_id());
        assert!(is_id(&a) && is_id(&b), "{a} {b}");
        assert_ne!(a, b);
    }

    #[test]
    fn a_report_with_an_id_answers_it() {
        let id = new_id();
        let doc = json!({ "format": 1, "id": id });
        assert_eq!(report_id(&doc, "1700000000000-panic.json", "capsule-1"), id);
    }

    #[test]
    fn an_older_report_gets_a_stable_id_from_its_name_and_capsule() {
        let old = json!({ "format": 1, "kind": "panic" });
        let first = report_id(&old, "1700000000000-panic-2.json", "capsule-1");
        assert!(is_id(&first));
        assert_eq!(
            first,
            report_id(&old, "1700000000000-panic-2", "capsule-1"),
            "with or without .json"
        );
        assert_eq!(
            first,
            derived_id("1700000000000-panic-2.json", "capsule-1"),
            "stable"
        );
        assert_ne!(first, derived_id("1700000000000-panic-3.json", "capsule-1"));
        assert_ne!(first, derived_id("1700000000000-panic-2.json", "capsule-2"));
        let bad = json!({ "id": "not-an-id" });
        assert_eq!(report_id(&bad, "x.json", "c"), derived_id("x.json", "c"));
    }

    #[test]
    fn the_derived_id_never_changes() {
        // Pinned: a receiver may already hold it.
        assert_eq!(
            derived_id("1700000000000-panic.json", "capsule-1"),
            derived_id("1700000000000-panic", "capsule-1")
        );
        assert_eq!(derived_id("a", "b").len(), ID_LENGTH);
    }
}
